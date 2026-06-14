// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod chunk;
mod codec;
mod range_summary;
mod retention;
mod summary_node;

use crate::{Direction, Entry, Summary};
use alloc::vec::Vec;
use core::mem;

use self::chunk::{ChunkRef, ClosedChunk, OpenChunk};
use self::summary_node::SummaryNode;

pub use self::chunk::ChunkSummary;
pub use self::codec::{ChunkCodec, GorillaF64Codec, RawCodec};
pub use self::range_summary::RangeSummary;
pub use self::retention::RetentionPolicy;
pub use self::summary_node::SUMMARY_FANOUT;

/// Default number of entries stored in each chronology chunk.
pub const DEFAULT_CHUNK_CAPACITY: usize = 4_096;

/// Convenience alias for a chronology that seals chunks with [`GorillaF64Codec`].
pub type GorillaF64Chronology<S> = Chronology<f64, S, GorillaF64Codec>;

/// Error returned when entries cannot be inserted into a [`Chronology`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertError {
    /// The next timestamp was not greater than the previous timestamp.
    NonMonotonicTimestamp {
        /// Last accepted timestamp in the chronology or current batch.
        previous: u64,
        /// Rejected timestamp.
        next: u64,
    },
}

/// A stream of values over time for a single variable.
///
/// A chronology stores timestamped values where the timestamp
/// is when the new value was set.
///
/// ## Values
///
/// Values stored within a [`Chronology`] must implement the [`Copy`]
/// trait. Values are copied when they are stored within the
/// [`Chronology`]. For this reason, it is typically advisable to
/// keep them simple and easy to copy if you're dealing with
/// large numbers of values and need the highest levels of
/// performance.
///
/// ## Nature of Timestamps
///
/// Timestamps are represented as unsigned 64 bit integer values.
/// The exact interpretation of this value is up to the producer
/// and consumer of the data.
///
/// Typical interpretations might be:
///
/// * 1 unit is 1 second.
/// * 1 unit is 1 millisecond.
/// * 1 unit is 1 nanosecond.
/// * 1 unit is 10 nanoseconds.
/// * 1 unit is 100 picoseconds.
///
/// Timestamps may also be interpreted as being an absolute point
/// in time or a relative point in time, again up to the application
/// producing and consuming the data.
///
/// Some applications may be happy tracking number of seconds since
/// 1900. Others are using timestamps that correspond to the number
/// of nanoseconds since the application started or the CPU was
/// powered on.
///
/// ## Inserting Values
///
/// Values are inserted in monotonic timestamp order via
/// [`Chronology::insert_values`]. [`Entry`] wraps values along
/// with their timestamp.
///
/// ```
/// use chronostore::{Chronology, Entry, NullSummary};
///
/// let mut chrono = Chronology::<f32, NullSummary<f32>>::new();
/// chrono.insert_values(&[Entry::new(0, 0.3),
///                        Entry::new(5, 0.5)])
///       .expect("timestamps are monotonic");
/// ```
///
/// ## Querying Values
///
/// A [`Chronology`] can be [queried](Chronology::find_nearest_value) for
/// the current value at any point in time. It will find either the last
/// value set at or prior to the point in time by searching with
/// [`Direction::Backward`] or the next value that has been set at or
/// after the point in time by searching with [`Direction::Forward`].
///
/// ```
/// use chronostore::{Chronology, Direction, Entry, NullSummary};
///
/// let mut chrono = Chronology::<f32, NullSummary<f32>>::new();
/// chrono.insert_values(&[Entry::new(0, 0.3),
///                        Entry::new(5, 0.5)])
///       .expect("timestamps are monotonic");
///
/// assert_eq!(chrono.find_nearest_value(4, Direction::Forward),
///            Some(Entry::new(5, 0.5)));
/// ```
pub struct Chronology<V: Copy, S: Summary<V>, C: ChunkCodec<V> = RawCodec> {
    sealed_chunks: Vec<ClosedChunk<V, S, C>>,
    open_chunk: OpenChunk<V, S>,
    summary_levels: Vec<Vec<SummaryNode<S>>>,
    summary: S,
    chunk_capacity: usize,
    retention_policy: RetentionPolicy,
    len: usize,
}

impl<V: Copy, S: Summary<V>, C: ChunkCodec<V>> Default for Chronology<V, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Copy, S: Summary<V>, C: ChunkCodec<V>> Chronology<V, S, C> {
    /// Create a new [`Chronology`] with [`DEFAULT_CHUNK_CAPACITY`].
    pub fn new() -> Self {
        Self::with_chunk_capacity(DEFAULT_CHUNK_CAPACITY)
    }

    /// Create a new [`Chronology`] with a custom chunk capacity.
    ///
    /// # Panics
    ///
    /// Panics when `chunk_capacity` is zero.
    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        Self::with_chunk_capacity_and_retention(chunk_capacity, RetentionPolicy::unbounded())
    }

    /// Create a new [`Chronology`] with a retention policy.
    pub fn with_retention_policy(retention_policy: RetentionPolicy) -> Self {
        Self::with_chunk_capacity_and_retention(DEFAULT_CHUNK_CAPACITY, retention_policy)
    }

    /// Create a new [`Chronology`] with a custom chunk capacity and retention policy.
    ///
    /// # Panics
    ///
    /// Panics when `chunk_capacity` is zero.
    pub fn with_chunk_capacity_and_retention(
        chunk_capacity: usize,
        retention_policy: RetentionPolicy,
    ) -> Self {
        assert!(chunk_capacity > 0, "chunk capacity must be non-zero");
        Chronology {
            sealed_chunks: Vec::new(),
            open_chunk: OpenChunk::with_capacity(chunk_capacity),
            summary_levels: Vec::new(),
            summary: S::default(),
            chunk_capacity,
            retention_policy,
            len: 0,
        }
    }

    /// Return the number of entries stored in this chronology.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return `true` when this chronology has no entries.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the configured entry capacity for each chunk.
    pub fn chunk_capacity(&self) -> usize {
        self.chunk_capacity
    }

    /// Return the current retention policy.
    pub fn retention_policy(&self) -> RetentionPolicy {
        self.retention_policy
    }

    /// Set the retention policy and immediately enforce it.
    pub fn set_retention_policy(&mut self, retention_policy: RetentionPolicy) {
        self.retention_policy = retention_policy;
        self.enforce_retention();
    }

    /// Return the number of sealed chunks.
    ///
    /// The currently open chunk is not counted here.
    pub fn sealed_chunk_count(&self) -> usize {
        self.sealed_chunks.len()
    }

    /// Return the total encoded payload size for sealed chunks, in bytes.
    ///
    /// This excludes allocator capacity, chunk metadata, open chunk storage,
    /// summary tiles, and summary-pyramid nodes. It is intended for codec
    /// comparisons.
    pub fn sealed_encoded_size(&self) -> usize {
        self.sealed_chunks
            .iter()
            .map(ClosedChunk::encoded_size)
            .sum()
    }

    /// Return the number of chunks with entries.
    ///
    /// This includes the currently open chunk when it is non-empty.
    pub fn chunk_count(&self) -> usize {
        self.sealed_chunks.len() + usize::from(!self.open_chunk.is_empty())
    }

    /// Return borrowed summary metadata for a chunk.
    pub fn chunk_summary(&self, index: usize) -> Option<ChunkSummary<'_, S>> {
        if index < self.sealed_chunks.len() {
            Some(self.sealed_chunks[index].summary())
        } else if index == self.sealed_chunks.len() && !self.open_chunk.is_empty() {
            Some(self.open_chunk.summary())
        } else {
            None
        }
    }

    /// Return the number of summary-pyramid levels currently populated.
    ///
    /// Level 0 contains one node per sealed chunk. Higher levels summarize
    /// groups of [`SUMMARY_FANOUT`] lower-level nodes.
    pub fn summary_level_count(&self) -> usize {
        self.summary_levels.len()
    }

    /// Return the number of summary nodes in a summary-pyramid level.
    pub fn summary_node_count(&self, level: usize) -> Option<usize> {
        self.summary_levels.get(level).map(Vec::len)
    }

    /// Return the summary for all entries in this chronology.
    pub fn summary(&self) -> &S {
        &self.summary
    }

    /// Return a summary for entries whose timestamps are in `start..end`.
    ///
    /// The range is half-open: `start` is included and `end` is excluded.
    pub fn range_summary(&self, start: u64, end: u64) -> RangeSummary<S> {
        let mut range_summary = RangeSummary::empty(start, end);
        if start >= end {
            return range_summary;
        }

        let Some(mut chunk_index) = self.first_chunk_with_end_at_least(start) else {
            return range_summary;
        };

        while chunk_index < self.chunk_count() {
            let chunk = self.chunk(chunk_index);
            if chunk.start_timestamp() >= end {
                break;
            }

            if chunk_index < self.sealed_chunks.len()
                && chunk.start_timestamp() >= start
                && chunk.end_timestamp() < end
            {
                let sealed_start = chunk_index;
                chunk_index += 1;
                while chunk_index < self.sealed_chunks.len() {
                    let chunk = self.chunk(chunk_index);
                    if chunk.start_timestamp() < start || chunk.end_timestamp() >= end {
                        break;
                    }
                    chunk_index += 1;
                }
                self.add_sealed_chunk_range(sealed_start, chunk_index, &mut range_summary);
                continue;
            }

            if chunk.start_timestamp() >= start && chunk.end_timestamp() < end {
                range_summary.add_summary::<V>(chunk.len(), chunk.summary_value());
            } else {
                chunk.add_range_summary(start, end, &mut range_summary);
            }

            chunk_index += 1;
        }

        range_summary
    }

    /// Visit exact entries whose timestamps are in `start..end`.
    ///
    /// The range is half-open: `start` is included and `end` is excluded. This
    /// method does not allocate and is intended for callers that need exact
    /// samples for inspection, export, or display algorithms such as LTTB.
    ///
    /// ```
    /// use chronostore::{Chronology, Entry, NullSummary};
    ///
    /// let mut chrono = Chronology::<u64, NullSummary<u64>>::new();
    /// chrono.insert_values(&[Entry::new(0, 10),
    ///                        Entry::new(5, 20),
    ///                        Entry::new(9, 30)])
    ///       .expect("timestamps are monotonic");
    ///
    /// let mut values = Vec::new();
    /// chrono.visit_range_entries(1, 10, |entry| values.push(entry.value));
    /// assert_eq!(values, vec![20, 30]);
    /// ```
    pub fn visit_range_entries<F>(&self, start: u64, end: u64, mut visit: F)
    where
        F: FnMut(Entry<V>),
    {
        if start >= end {
            return;
        }

        let Some(mut chunk_index) = self.first_chunk_with_end_at_least(start) else {
            return;
        };

        while chunk_index < self.chunk_count() {
            let chunk = self.chunk(chunk_index);
            if chunk.start_timestamp() >= end {
                break;
            }

            chunk.visit_range_entries(start, end, &mut visit);
            chunk_index += 1;
        }
    }

    /// Return exact entries whose timestamps are in `start..end`.
    ///
    /// The range is half-open: `start` is included and `end` is excluded. Use
    /// [`Chronology::visit_range_entries`] when the caller can consume entries
    /// without allocating.
    pub fn entries_in_range(&self, start: u64, end: u64) -> Vec<Entry<V>> {
        let mut entries = Vec::new();
        self.visit_range_entries(start, end, |entry| entries.push(entry));
        entries
    }

    /// Return bucketed summaries for a viewport-style range query.
    ///
    /// The range is half-open: `start` is included and `end` is excluded. At
    /// most `target_buckets` summaries are returned. When the timestamp span is
    /// smaller than `target_buckets`, one bucket per timestamp unit is returned.
    /// With a min/max summary, the returned buckets can be used as a
    /// visualization envelope that preserves spikes without decoding every
    /// sample in the visible range.
    pub fn summarize_range(
        &self,
        start: u64,
        end: u64,
        target_buckets: usize,
    ) -> Vec<RangeSummary<S>> {
        if start >= end || target_buckets == 0 {
            return Vec::new();
        }

        let span = end - start;
        let bucket_count = if span < target_buckets as u64 {
            span as usize
        } else {
            target_buckets
        };
        let mut summaries = Vec::with_capacity(bucket_count);

        for bucket in 0..bucket_count {
            let bucket_start = bucket_boundary(start, span, bucket, bucket_count);
            let bucket_end = bucket_boundary(start, span, bucket + 1, bucket_count);
            summaries.push(self.range_summary(bucket_start, bucket_end));
        }

        summaries
    }

    /// Find the nearest value in time.
    pub fn find_nearest_value(&self, timestamp: u64, direction: Direction) -> Option<Entry<V>> {
        match direction {
            Direction::Backward => self.find_backward(timestamp),
            Direction::Forward => self.find_forward(timestamp),
        }
    }

    /// Record a single value and its timestamp.
    pub fn insert_value(&mut self, entry: Entry<V>) -> Result<(), InsertError> {
        if let Some(previous) = self.last_timestamp() {
            if entry.timestamp <= previous {
                return Err(InsertError::NonMonotonicTimestamp {
                    previous,
                    next: entry.timestamp,
                });
            }
        }

        self.push_entry(entry);
        Ok(())
    }

    /// Record a set of values and their timestamps.
    ///
    /// Entries must have strictly increasing timestamps and must follow any
    /// entries already stored in this chronology. The chronology is not mutated
    /// when validation fails.
    pub fn insert_values(&mut self, values: &[Entry<V>]) -> Result<(), InsertError> {
        self.validate_insert_batch(values)?;

        for entry in values {
            self.push_entry(*entry);
        }

        Ok(())
    }

    fn validate_insert_batch(&self, values: &[Entry<V>]) -> Result<(), InsertError> {
        let mut previous = self.last_timestamp();

        for entry in values {
            if let Some(previous_timestamp) = previous {
                if entry.timestamp <= previous_timestamp {
                    return Err(InsertError::NonMonotonicTimestamp {
                        previous: previous_timestamp,
                        next: entry.timestamp,
                    });
                }
            }
            previous = Some(entry.timestamp);
        }

        Ok(())
    }

    fn push_entry(&mut self, entry: Entry<V>) {
        self.open_chunk.push(entry);
        self.summary.update(&entry);
        self.len += 1;

        if self.open_chunk.len() == self.chunk_capacity {
            self.seal_open_chunk();
        }
    }

    fn seal_open_chunk(&mut self) {
        let next_open_chunk = OpenChunk::with_capacity(self.chunk_capacity);
        let open_chunk = mem::replace(&mut self.open_chunk, next_open_chunk);
        let sealed_chunk = open_chunk.seal::<C>();
        let summary_node = sealed_chunk.summary_node();
        self.sealed_chunks.push(sealed_chunk);
        self.push_summary_node(0, summary_node);
        self.enforce_retention();
    }

    fn find_forward(&self, timestamp: u64) -> Option<Entry<V>> {
        let chunk_index = self.first_chunk_with_end_at_least(timestamp)?;
        let chunk = self.chunk(chunk_index);
        chunk.entry_at_or_after(timestamp)
    }

    fn find_backward(&self, timestamp: u64) -> Option<Entry<V>> {
        let chunk_index = self.last_chunk_with_start_at_most(timestamp)?;
        let chunk = self.chunk(chunk_index);
        chunk.entry_at_or_before(timestamp)
    }

    fn first_chunk_with_end_at_least(&self, timestamp: u64) -> Option<usize> {
        let chunk_count = self.chunk_count();
        let mut left = 0;
        let mut right = chunk_count;

        while left < right {
            let mid = left + ((right - left) / 2);
            if self.chunk(mid).end_timestamp() < timestamp {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        (left < chunk_count).then_some(left)
    }

    fn last_chunk_with_start_at_most(&self, timestamp: u64) -> Option<usize> {
        let chunk_count = self.chunk_count();
        let mut left = 0;
        let mut right = chunk_count;

        while left < right {
            let mid = left + ((right - left) / 2);
            if self.chunk(mid).start_timestamp() <= timestamp {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        left.checked_sub(1)
    }

    fn last_timestamp(&self) -> Option<u64> {
        self.open_chunk
            .last_timestamp()
            .or_else(|| self.sealed_chunks.last().map(ClosedChunk::end_timestamp))
    }

    fn chunk(&self, index: usize) -> ChunkRef<'_, V, S, C> {
        if index < self.sealed_chunks.len() {
            ChunkRef::Closed(&self.sealed_chunks[index])
        } else {
            ChunkRef::Open(&self.open_chunk)
        }
    }

    fn push_summary_node(&mut self, level: usize, node: SummaryNode<S>) {
        if self.summary_levels.len() == level {
            self.summary_levels.push(Vec::new());
        }

        self.summary_levels[level].push(node);

        if self.summary_levels[level]
            .len()
            .is_multiple_of(SUMMARY_FANOUT)
        {
            let child_count = self.summary_levels[level].len();
            let parent = SummaryNode::merge_nodes::<V>(
                &self.summary_levels[level][child_count - SUMMARY_FANOUT..],
            );
            self.push_summary_node(level + 1, parent);
        }
    }

    fn enforce_retention(&mut self) {
        let Some(max_sealed_chunks) = self.retention_policy.sealed_chunk_limit() else {
            return;
        };
        let evict_count = self.sealed_chunks.len().saturating_sub(max_sealed_chunks);
        if evict_count == 0 {
            return;
        }

        let evicted_len = self
            .sealed_chunks
            .iter()
            .take(evict_count)
            .map(ClosedChunk::len)
            .sum::<usize>();

        self.sealed_chunks.drain(0..evict_count);
        self.len -= evicted_len;
        self.rebuild_summary_state();
    }

    fn rebuild_summary_state(&mut self) {
        self.summary_levels.clear();
        for index in 0..self.sealed_chunks.len() {
            let node = self.sealed_chunks[index].summary_node();
            self.push_summary_node(0, node);
        }

        let mut summary = S::default();
        for chunk in &self.sealed_chunks {
            summary.merge(&chunk.summary);
        }
        if !self.open_chunk.is_empty() {
            summary.merge(&self.open_chunk.summary);
        }
        self.summary = summary;
    }

    fn add_sealed_chunk_range(
        &self,
        mut start: usize,
        end: usize,
        range_summary: &mut RangeSummary<S>,
    ) {
        while start < end {
            let mut level = 0;
            let mut span: usize = 1;

            while let Some(next_span) = span.checked_mul(SUMMARY_FANOUT) {
                let next_level = level + 1;
                let node_index = start / next_span;
                let Some(nodes) = self.summary_levels.get(next_level) else {
                    break;
                };

                if !start.is_multiple_of(next_span)
                    || start + next_span > end
                    || node_index >= nodes.len()
                {
                    break;
                }

                level = next_level;
                span = next_span;
            }

            let node = &self.summary_levels[level][start / span];
            range_summary.add_summary::<V>(node.len, &node.summary);
            start += span;
        }
    }
}

fn bucket_boundary(start: u64, span: u64, bucket: usize, bucket_count: usize) -> u64 {
    start + ((u128::from(span) * bucket as u128) / bucket_count as u128) as u64
}
