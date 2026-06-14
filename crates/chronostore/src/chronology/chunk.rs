// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::codec::{ChunkCodec, CodecBlock};
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
#[derive(Clone, Copy, Debug)]
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

pub(super) struct OpenEntries<'a, V: Copy> {
    timestamps: &'a [u64],
    values: &'a [V],
    index: usize,
    end: usize,
}

impl<'a, V: Copy> OpenEntries<'a, V> {
    fn new<S>(chunk: &'a OpenChunk<V, S>, start: u64, end: u64) -> Self
    where
        S: Summary<V>,
    {
        let range = chunk.range_indices(start, end);
        OpenEntries {
            timestamps: &chunk.timestamps,
            values: &chunk.values,
            index: range.start,
            end: range.end,
        }
    }
}

impl<V: Copy> Iterator for OpenEntries<'_, V> {
    type Item = Entry<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.end {
            return None;
        }

        let entry = Entry::new(self.timestamps[self.index], self.values[self.index]);
        self.index += 1;
        Some(entry)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.index;
        (remaining, Some(remaining))
    }
}

impl<V: Copy> ExactSizeIterator for OpenEntries<'_, V> {}

impl<V: Copy, S: Summary<V>> OpenChunk<V, S> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
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
        debug_assert!(
            !self.timestamps.is_empty(),
            "only non-empty open chunks can be sealed"
        );

        let start = self.timestamps[0];
        let end = self.timestamps[self.timestamps.len() - 1];
        let len = self.timestamps.len();
        let encode_plan = C::plan(&self.timestamps, &self.values);
        let summary_tiles =
            build_summary_tiles::<V, S>(&self.timestamps, &self.values, encode_plan.as_ref());
        let encoded = C::encode(self.timestamps, self.values, encode_plan);

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

    pub(super) fn entries(&self, start: u64, end: u64) -> OpenEntries<'_, V> {
        OpenEntries::new(self, start, end)
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

    pub(super) fn entries(&self, start: u64, end: u64) -> C::Entries<'_> {
        C::entries(&self.encoded, start, end)
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

pub(super) enum ChunkEntries<'a, V, C>
where
    V: Copy + 'a,
    C: ChunkCodec<V> + 'a,
{
    Closed(C::Entries<'a>),
    Open(OpenEntries<'a, V>),
}

impl<V, C> Iterator for ChunkEntries<'_, V, C>
where
    V: Copy,
    C: ChunkCodec<V>,
{
    type Item = Entry<V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ChunkEntries::Closed(entries) => entries.next(),
            ChunkEntries::Open(entries) => entries.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            ChunkEntries::Closed(entries) => entries.size_hint(),
            ChunkEntries::Open(entries) => entries.size_hint(),
        }
    }
}

fn build_summary_tiles<V, S>(
    timestamps: &[u64],
    values: &[V],
    blocks: &[CodecBlock],
) -> Vec<SummaryNode<S>>
where
    V: Copy,
    S: Summary<V>,
{
    debug_assert_eq!(
        timestamps.len(),
        values.len(),
        "summary tiles require matching timestamp and value columns"
    );
    debug_assert!(
        !blocks.is_empty(),
        "summary tiles require at least one codec block"
    );

    let mut tiles = Vec::with_capacity(blocks.len());
    let mut expected_start_index = 0;

    for block in blocks {
        debug_assert_eq!(
            block.start_index, expected_start_index,
            "codec blocks must exactly cover the chunk in order"
        );
        debug_assert!(
            block.end_index > block.start_index,
            "codec blocks must be non-empty"
        );
        debug_assert!(
            block.end_index <= timestamps.len(),
            "codec blocks must stay inside the chunk"
        );
        debug_assert_eq!(
            block.start, timestamps[block.start_index],
            "codec block start timestamp must match its first sample"
        );
        debug_assert_eq!(
            block.end,
            timestamps[block.end_index - 1],
            "codec block end timestamp must match its last sample"
        );

        let mut summary = S::default();

        for index in block.start_index..block.end_index {
            summary.update(&Entry::new(timestamps[index], values[index]));
        }

        tiles.push(SummaryNode {
            start: block.start,
            end: block.end,
            len: block.end_index - block.start_index,
            summary,
        });

        expected_start_index = block.end_index;
    }
    debug_assert_eq!(
        expected_start_index,
        timestamps.len(),
        "codec blocks must cover every sample"
    );

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

    pub(super) fn entries(&self, start: u64, end: u64) -> ChunkEntries<'a, V, C> {
        match self {
            ChunkRef::Closed(chunk) => ChunkEntries::Closed(chunk.entries(start, end)),
            ChunkRef::Open(chunk) => ChunkEntries::Open(chunk.entries(start, end)),
        }
    }

    pub(super) fn summary_value(&self) -> &S {
        match self {
            ChunkRef::Closed(chunk) => &chunk.summary,
            ChunkRef::Open(chunk) => &chunk.summary,
        }
    }
}
