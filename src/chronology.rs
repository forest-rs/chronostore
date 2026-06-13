// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Direction, Entry, Summary};
use alloc::vec::Vec;
use core::mem;

/// Default number of entries stored in each raw chronology chunk.
pub const DEFAULT_CHUNK_CAPACITY: usize = 4_096;

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
pub struct Chronology<V: Copy, S: Default + Summary<V>> {
    sealed_chunks: Vec<Chunk<V, S>>,
    open_chunk: Chunk<V, S>,
    summary: S,
    chunk_capacity: usize,
    len: usize,
}

impl<V: Copy, S: Default + Summary<V>> Default for Chronology<V, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Copy, S: Default + Summary<V>> Chronology<V, S> {
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
        assert!(chunk_capacity > 0, "chunk capacity must be non-zero");
        Chronology {
            sealed_chunks: Vec::new(),
            open_chunk: Chunk::with_capacity(chunk_capacity),
            summary: S::default(),
            chunk_capacity,
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

    /// Return the number of sealed chunks.
    ///
    /// The currently open chunk is not counted here.
    pub fn sealed_chunk_count(&self) -> usize {
        self.sealed_chunks.len()
    }

    /// Return the summary for all entries in this chronology.
    pub fn summary(&self) -> &S {
        &self.summary
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
        let next_open_chunk = Chunk::with_capacity(self.chunk_capacity);
        let sealed_chunk = mem::replace(&mut self.open_chunk, next_open_chunk);
        self.sealed_chunks.push(sealed_chunk);
    }

    fn find_forward(&self, timestamp: u64) -> Option<Entry<V>> {
        let chunk_index = self.first_chunk_with_end_at_least(timestamp)?;
        let chunk = self.chunk(chunk_index);
        let value_index = chunk.first_index_at_least(timestamp)?;
        Some(chunk.entry(value_index))
    }

    fn find_backward(&self, timestamp: u64) -> Option<Entry<V>> {
        let chunk_index = self.last_chunk_with_start_at_most(timestamp)?;
        let chunk = self.chunk(chunk_index);
        let value_index = chunk.last_index_at_most(timestamp)?;
        Some(chunk.entry(value_index))
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
            .or_else(|| self.sealed_chunks.last().map(Chunk::end_timestamp))
    }

    fn chunk_count(&self) -> usize {
        self.sealed_chunks.len() + usize::from(!self.open_chunk.is_empty())
    }

    fn chunk(&self, index: usize) -> &Chunk<V, S> {
        if index < self.sealed_chunks.len() {
            &self.sealed_chunks[index]
        } else {
            &self.open_chunk
        }
    }
}

struct Chunk<V: Copy, S: Default + Summary<V>> {
    timestamps: Vec<u64>,
    values: Vec<V>,
    summary: S,
}

impl<V: Copy, S: Default + Summary<V>> Chunk<V, S> {
    fn with_capacity(capacity: usize) -> Self {
        Chunk {
            timestamps: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            summary: S::default(),
        }
    }

    fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    fn len(&self) -> usize {
        self.timestamps.len()
    }

    fn push(&mut self, entry: Entry<V>) {
        self.timestamps.push(entry.timestamp);
        self.values.push(entry.value);
        self.summary.update(&entry);
    }

    fn start_timestamp(&self) -> u64 {
        self.timestamps[0]
    }

    fn end_timestamp(&self) -> u64 {
        self.timestamps[self.timestamps.len() - 1]
    }

    fn last_timestamp(&self) -> Option<u64> {
        self.timestamps.last().copied()
    }

    fn first_index_at_least(&self, timestamp: u64) -> Option<usize> {
        match self.timestamps.binary_search(&timestamp) {
            Ok(index) => Some(index),
            Err(index) => (index < self.timestamps.len()).then_some(index),
        }
    }

    fn last_index_at_most(&self, timestamp: u64) -> Option<usize> {
        match self.timestamps.binary_search(&timestamp) {
            Ok(index) => Some(index),
            Err(index) => index.checked_sub(1),
        }
    }

    fn entry(&self, index: usize) -> Entry<V> {
        Entry::new(self.timestamps[index], self.values[index])
    }
}
