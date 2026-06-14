// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Entry;
use alloc::vec::Vec;
use core::ops::Range;

pub(super) trait ChunkCodec<V: Copy> {
    type Encoded;

    fn encode(timestamps: Vec<u64>, values: Vec<V>) -> Self::Encoded;

    fn first_index_at_least(encoded: &Self::Encoded, timestamp: u64) -> Option<usize>;

    fn last_index_at_most(encoded: &Self::Encoded, timestamp: u64) -> Option<usize>;

    fn range_indices(encoded: &Self::Encoded, start: u64, end: u64) -> Range<usize>;

    fn entry(encoded: &Self::Encoded, index: usize) -> Entry<V>;
}

pub(super) struct RawCodec;

pub(super) struct RawEncodedChunk<V> {
    timestamps: Vec<u64>,
    values: Vec<V>,
}

impl<V: Copy> ChunkCodec<V> for RawCodec {
    type Encoded = RawEncodedChunk<V>;

    fn encode(timestamps: Vec<u64>, values: Vec<V>) -> Self::Encoded {
        RawEncodedChunk { timestamps, values }
    }

    fn first_index_at_least(encoded: &Self::Encoded, timestamp: u64) -> Option<usize> {
        match encoded.timestamps.binary_search(&timestamp) {
            Ok(index) => Some(index),
            Err(index) => (index < encoded.timestamps.len()).then_some(index),
        }
    }

    fn last_index_at_most(encoded: &Self::Encoded, timestamp: u64) -> Option<usize> {
        match encoded.timestamps.binary_search(&timestamp) {
            Ok(index) => Some(index),
            Err(index) => index.checked_sub(1),
        }
    }

    fn range_indices(encoded: &Self::Encoded, start: u64, end: u64) -> Range<usize> {
        let start_index = match encoded.timestamps.binary_search(&start) {
            Ok(index) | Err(index) => index,
        };
        let end_index = match encoded.timestamps.binary_search(&end) {
            Ok(index) | Err(index) => index,
        };

        start_index..end_index
    }

    fn entry(encoded: &Self::Encoded, index: usize) -> Entry<V> {
        Entry::new(encoded.timestamps[index], encoded.values[index])
    }
}
