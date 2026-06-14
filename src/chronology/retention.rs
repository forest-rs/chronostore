// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Retention policy for chronology chunks.
///
/// Pass a `RetentionPolicy` to
/// [`Chronology::with_retention_policy`](crate::Chronology::with_retention_policy),
/// [`Chronology::with_chunk_capacity_and_retention`](crate::Chronology::with_chunk_capacity_and_retention),
/// or [`Chronology::set_retention_policy`](crate::Chronology::set_retention_policy).
///
/// Retention is enforced at sealed-chunk granularity. The currently open chunk
/// is not counted against the sealed-chunk limit, is not partially evicted by
/// time-window retention, and may contain samples older than the configured
/// window until it is sealed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetentionPolicy {
    max_sealed_chunks: Option<usize>,
    max_age: Option<u64>,
}

impl RetentionPolicy {
    /// Keep all sealed chunks.
    pub const fn unbounded() -> Self {
        RetentionPolicy {
            max_sealed_chunks: None,
            max_age: None,
        }
    }

    /// Keep at most `max_sealed_chunks` sealed chunks.
    pub const fn max_sealed_chunks(max_sealed_chunks: usize) -> Self {
        RetentionPolicy {
            max_sealed_chunks: Some(max_sealed_chunks),
            max_age: None,
        }
    }

    /// Keep sealed chunks whose end timestamp is within `max_age` of the latest
    /// timestamp.
    ///
    /// `max_age` is measured in the same timestamp units as inserted entries.
    /// Chunks are retained when their end timestamp is greater than or equal to
    /// `latest_timestamp - max_age`.
    pub const fn max_age(max_age: u64) -> Self {
        RetentionPolicy {
            max_sealed_chunks: None,
            max_age: Some(max_age),
        }
    }

    /// Return this policy with a sealed-chunk limit.
    pub const fn with_max_sealed_chunks(self, max_sealed_chunks: usize) -> Self {
        RetentionPolicy {
            max_sealed_chunks: Some(max_sealed_chunks),
            max_age: self.max_age,
        }
    }

    /// Return this policy with a time-window limit.
    ///
    /// `max_age` is measured in the same timestamp units as inserted entries.
    pub const fn with_max_age(self, max_age: u64) -> Self {
        RetentionPolicy {
            max_sealed_chunks: self.max_sealed_chunks,
            max_age: Some(max_age),
        }
    }

    /// Return the configured sealed-chunk limit.
    pub const fn sealed_chunk_limit(self) -> Option<usize> {
        self.max_sealed_chunks
    }

    /// Return the configured time-window limit.
    pub const fn age_limit(self) -> Option<u64> {
        self.max_age
    }
}
