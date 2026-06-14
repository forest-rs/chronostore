// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Retention policy for sealed chronology chunks.
///
/// Retention is enforced at sealed-chunk granularity. The currently open chunk
/// is not counted against the sealed-chunk limit and is never partially evicted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetentionPolicy {
    max_sealed_chunks: Option<usize>,
}

impl RetentionPolicy {
    /// Keep all sealed chunks.
    pub const fn unbounded() -> Self {
        RetentionPolicy {
            max_sealed_chunks: None,
        }
    }

    /// Keep at most `max_sealed_chunks` sealed chunks.
    pub const fn max_sealed_chunks(max_sealed_chunks: usize) -> Self {
        RetentionPolicy {
            max_sealed_chunks: Some(max_sealed_chunks),
        }
    }

    /// Return the configured sealed-chunk limit.
    pub const fn sealed_chunk_limit(self) -> Option<usize> {
        self.max_sealed_chunks
    }
}
