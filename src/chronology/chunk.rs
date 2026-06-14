// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::codec::ChunkCodec;
use super::{RangeSummary, SummaryNode};
use crate::{Entry, Summary};
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::Range;

/// Borrowed summary metadata for one chronology chunk.
///
/// `ChunkSummary` is returned by [`Chronology::chunk_summary`](crate::Chronology::chunk_summary)
/// for diagnostics and storage inspection. It borrows the chunk's summary from
/// the chronology, so it is a view into existing storage rather than an owned
/// aggregate.
#[derive(Clone, Copy)]
pub struct ChunkSummary<'a, S> {
    /// Start timestamp for the chunk.
    pub start: u64,
    /// End timestamp for the chunk.
    pub end: u64,
    /// Number of entries stored in the chunk.
    pub len: usize,
    /// Summary for the chunk.
    pub summary: &'a S,
}

pub(super) struct OpenChunk<V: Copy, S: Summary<V>> {
    timestamps: Vec<u64>,
    values: Vec<V>,
    pub(super) summary: S,
}

impl<V: Copy, S: Summary<V>> OpenChunk<V, S> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        OpenChunk {
            timestamps: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            summary: S::default(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.timestamps.len()
    }

    pub(super) fn push(&mut self, entry: Entry<V>) {
        self.timestamps.push(entry.timestamp);
        self.values.push(entry.value);
        self.summary.update(&entry);
    }

    pub(super) fn seal<C>(self) -> ClosedChunk<V, S, C>
    where
        C: ChunkCodec<V>,
    {
        debug_assert!(!self.timestamps.is_empty());

        let start = self.timestamps[0];
        let end = self.timestamps[self.timestamps.len() - 1];
        let len = self.timestamps.len();
        let summary_tiles =
            build_summary_tiles::<V, S>(&self.timestamps, &self.values, C::SUMMARY_TILE_CAPACITY);
        let encoded = C::encode(self.timestamps, self.values);

        ClosedChunk {
            start,
            end,
            len,
            summary: self.summary,
            summary_tiles,
            encoded,
            codec: PhantomData,
        }
    }

    pub(super) fn summary(&self) -> ChunkSummary<'_, S> {
        ChunkSummary {
            start: self.start_timestamp(),
            end: self.end_timestamp(),
            len: self.len(),
            summary: &self.summary,
        }
    }

    pub(super) fn start_timestamp(&self) -> u64 {
        self.timestamps[0]
    }

    pub(super) fn end_timestamp(&self) -> u64 {
        self.timestamps[self.timestamps.len() - 1]
    }

    pub(super) fn last_timestamp(&self) -> Option<u64> {
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

    pub(super) fn entry_at_or_after(&self, timestamp: u64) -> Option<Entry<V>> {
        let index = self.first_index_at_least(timestamp)?;
        Some(self.entry(index))
    }

    pub(super) fn entry_at_or_before(&self, timestamp: u64) -> Option<Entry<V>> {
        let index = self.last_index_at_most(timestamp)?;
        Some(self.entry(index))
    }

    pub(super) fn add_range_summary(
        &self,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    ) {
        let indices = self.range_indices(start, end);
        if indices.is_empty() {
            return;
        }

        let mut summary = S::default();
        let mut len = 0;

        for index in indices {
            summary.update(&self.entry(index));
            len += 1;
        }

        range_summary.add_summary::<V>(len, &summary);
    }

    pub(super) fn visit_range_entries<F>(&self, start: u64, end: u64, visit: &mut F)
    where
        F: FnMut(Entry<V>),
    {
        for index in self.range_indices(start, end) {
            visit(self.entry(index));
        }
    }

    pub(super) fn entry(&self, index: usize) -> Entry<V> {
        Entry::new(self.timestamps[index], self.values[index])
    }

    fn range_indices(&self, start: u64, end: u64) -> Range<usize> {
        let start_index = match self.timestamps.binary_search(&start) {
            Ok(index) | Err(index) => index,
        };
        let end_index = match self.timestamps.binary_search(&end) {
            Ok(index) | Err(index) => index,
        };

        start_index..end_index
    }
}

pub(super) struct ClosedChunk<V: Copy, S: Summary<V>, C: ChunkCodec<V>> {
    start: u64,
    end: u64,
    len: usize,
    pub(super) summary: S,
    summary_tiles: Vec<SummaryNode<S>>,
    encoded: C::Encoded,
    codec: PhantomData<(V, C)>,
}

impl<V: Copy, S: Summary<V>, C: ChunkCodec<V>> ClosedChunk<V, S, C> {
    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn summary(&self) -> ChunkSummary<'_, S> {
        ChunkSummary {
            start: self.start,
            end: self.end,
            len: self.len,
            summary: &self.summary,
        }
    }

    pub(super) fn summary_node(&self) -> SummaryNode<S> {
        SummaryNode {
            start: self.start,
            end: self.end,
            len: self.len,
            summary: self.summary.clone(),
        }
    }

    pub(super) fn start_timestamp(&self) -> u64 {
        self.start
    }

    pub(super) fn end_timestamp(&self) -> u64 {
        self.end
    }

    pub(super) fn encoded_size(&self) -> usize {
        C::encoded_size(&self.encoded)
    }

    pub(super) fn entry_at_or_after(&self, timestamp: u64) -> Option<Entry<V>> {
        C::entry_at_or_after(&self.encoded, timestamp)
    }

    pub(super) fn entry_at_or_before(&self, timestamp: u64) -> Option<Entry<V>> {
        C::entry_at_or_before(&self.encoded, timestamp)
    }

    pub(super) fn visit_range_entries<F>(&self, start: u64, end: u64, visit: &mut F)
    where
        F: FnMut(Entry<V>),
    {
        C::visit_range_entries(&self.encoded, start, end, visit);
    }

    pub(super) fn add_range_summary(
        &self,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    ) {
        let Some(mut tile_index) = self.first_tile_with_end_at_least(start) else {
            return;
        };

        while let Some(tile) = self.summary_tiles.get(tile_index) {
            if tile.start >= end {
                break;
            }

            if tile.start >= start && tile.end < end {
                range_summary.add_summary::<V>(tile.len, &tile.summary);
            } else {
                let partial_start = start.max(tile.start);
                let partial_end = end.min(tile.end.saturating_add(1));
                C::add_range_summary(&self.encoded, partial_start, partial_end, range_summary);
            }

            tile_index += 1;
        }
    }

    fn first_tile_with_end_at_least(&self, timestamp: u64) -> Option<usize> {
        let mut left = 0;
        let mut right = self.summary_tiles.len();

        while left < right {
            let mid = left + ((right - left) / 2);
            if self.summary_tiles[mid].end < timestamp {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        (left < self.summary_tiles.len()).then_some(left)
    }
}

fn build_summary_tiles<V, S>(
    timestamps: &[u64],
    values: &[V],
    tile_capacity: usize,
) -> Vec<SummaryNode<S>>
where
    V: Copy,
    S: Summary<V>,
{
    debug_assert_eq!(timestamps.len(), values.len());
    debug_assert!(tile_capacity > 0);

    let tile_capacity = tile_capacity.max(1);
    let tile_count = timestamps.len().div_ceil(tile_capacity);
    let mut tiles = Vec::with_capacity(tile_count);
    let mut start_index = 0;

    while start_index < timestamps.len() {
        let end_index = start_index
            .saturating_add(tile_capacity)
            .min(timestamps.len());
        let mut summary = S::default();

        for index in start_index..end_index {
            summary.update(&Entry::new(timestamps[index], values[index]));
        }

        tiles.push(SummaryNode {
            start: timestamps[start_index],
            end: timestamps[end_index - 1],
            len: end_index - start_index,
            summary,
        });

        start_index = end_index;
    }

    tiles
}

#[derive(Clone, Copy)]
pub(super) enum ChunkRef<'a, V: Copy, S: Summary<V>, C: ChunkCodec<V>> {
    Closed(&'a ClosedChunk<V, S, C>),
    Open(&'a OpenChunk<V, S>),
}

impl<'a, V: Copy, S: Summary<V>, C: ChunkCodec<V>> ChunkRef<'a, V, S, C> {
    pub(super) fn len(&self) -> usize {
        match self {
            ChunkRef::Closed(chunk) => chunk.len(),
            ChunkRef::Open(chunk) => chunk.len(),
        }
    }

    pub(super) fn start_timestamp(&self) -> u64 {
        match self {
            ChunkRef::Closed(chunk) => chunk.start_timestamp(),
            ChunkRef::Open(chunk) => chunk.start_timestamp(),
        }
    }

    pub(super) fn end_timestamp(&self) -> u64 {
        match self {
            ChunkRef::Closed(chunk) => chunk.end_timestamp(),
            ChunkRef::Open(chunk) => chunk.end_timestamp(),
        }
    }

    pub(super) fn entry_at_or_after(&self, timestamp: u64) -> Option<Entry<V>> {
        match self {
            ChunkRef::Closed(chunk) => chunk.entry_at_or_after(timestamp),
            ChunkRef::Open(chunk) => chunk.entry_at_or_after(timestamp),
        }
    }

    pub(super) fn entry_at_or_before(&self, timestamp: u64) -> Option<Entry<V>> {
        match self {
            ChunkRef::Closed(chunk) => chunk.entry_at_or_before(timestamp),
            ChunkRef::Open(chunk) => chunk.entry_at_or_before(timestamp),
        }
    }

    pub(super) fn add_range_summary(
        &self,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    ) {
        match self {
            ChunkRef::Closed(chunk) => chunk.add_range_summary(start, end, range_summary),
            ChunkRef::Open(chunk) => chunk.add_range_summary(start, end, range_summary),
        }
    }

    pub(super) fn visit_range_entries<F>(&self, start: u64, end: u64, visit: &mut F)
    where
        F: FnMut(Entry<V>),
    {
        match self {
            ChunkRef::Closed(chunk) => chunk.visit_range_entries(start, end, visit),
            ChunkRef::Open(chunk) => chunk.visit_range_entries(start, end, visit),
        }
    }

    pub(super) fn summary_value(&self) -> &S {
        match self {
            ChunkRef::Closed(chunk) => &chunk.summary,
            ChunkRef::Open(chunk) => &chunk.summary,
        }
    }
}
