// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::codec::{ChunkCodec, RawCodec};
use super::{RangeSummary, SummaryNode};
use crate::{Entry, Summary};
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::Range;

/// Borrowed summary metadata for one chronology chunk.
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

pub(super) type ClosedRawChunk<V, S> = ClosedChunk<V, S, RawCodec>;

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
        let encoded = C::encode(self.timestamps, self.values);

        ClosedChunk {
            start,
            end,
            len,
            summary: self.summary,
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

    pub(super) fn first_index_at_least(&self, timestamp: u64) -> Option<usize> {
        match self.timestamps.binary_search(&timestamp) {
            Ok(index) => Some(index),
            Err(index) => (index < self.timestamps.len()).then_some(index),
        }
    }

    pub(super) fn last_index_at_most(&self, timestamp: u64) -> Option<usize> {
        match self.timestamps.binary_search(&timestamp) {
            Ok(index) => Some(index),
            Err(index) => index.checked_sub(1),
        }
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

    pub(super) fn first_index_at_least(&self, timestamp: u64) -> Option<usize> {
        C::first_index_at_least(&self.encoded, timestamp)
    }

    pub(super) fn last_index_at_most(&self, timestamp: u64) -> Option<usize> {
        C::last_index_at_most(&self.encoded, timestamp)
    }

    pub(super) fn add_range_summary(
        &self,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    ) {
        let indices = C::range_indices(&self.encoded, start, end);
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

    pub(super) fn entry(&self, index: usize) -> Entry<V> {
        C::entry(&self.encoded, index)
    }
}

#[derive(Clone, Copy)]
pub(super) enum ChunkRef<'a, V: Copy, S: Summary<V>> {
    Closed(&'a ClosedRawChunk<V, S>),
    Open(&'a OpenChunk<V, S>),
}

impl<'a, V: Copy, S: Summary<V>> ChunkRef<'a, V, S> {
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

    pub(super) fn first_index_at_least(&self, timestamp: u64) -> Option<usize> {
        match self {
            ChunkRef::Closed(chunk) => chunk.first_index_at_least(timestamp),
            ChunkRef::Open(chunk) => chunk.first_index_at_least(timestamp),
        }
    }

    pub(super) fn last_index_at_most(&self, timestamp: u64) -> Option<usize> {
        match self {
            ChunkRef::Closed(chunk) => chunk.last_index_at_most(timestamp),
            ChunkRef::Open(chunk) => chunk.last_index_at_most(timestamp),
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

    pub(super) fn entry(&self, index: usize) -> Entry<V> {
        match self {
            ChunkRef::Closed(chunk) => chunk.entry(index),
            ChunkRef::Open(chunk) => chunk.entry(index),
        }
    }

    pub(super) fn summary_value(&self) -> &S {
        match self {
            ChunkRef::Closed(chunk) => &chunk.summary,
            ChunkRef::Open(chunk) => &chunk.summary,
        }
    }
}
