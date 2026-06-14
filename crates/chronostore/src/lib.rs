// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Chronostore
//!
//! Chronostore is a `no_std` plus `alloc` storage kernel for monotonic,
//! timestamped time series in memory. Chronostore is intended for datasets
//! where a single time series has 100 million or fewer points.
//!
//! Chronostore intends to be fast at inserts, fast at queries,
//! and memory efficient.
//!
//! Once data has been collected from a primary source
//! such as profiling samplers or counters, program tracing,
//! hardware counters, or other sources of high frequency,
//! high precision data, it is often useful to have it in
//! a form that tools can work with for analyzing and
//! visualizing that data.
//!
//! What is Chronostore NOT?
//!
//! * It does not try to be a distributed system.
//! * It does not have failover.
//! * It doesn't run as a separate process out of the box.
//! * It doesn't even persist data to disk automatically.
//!
//! ## Implementation Status
//!
//! Chronostore stores monotonic samples in chunks, maintains mergeable summary
//! state, supports exact range access, supports bucketed range summaries for
//! zoomed views, and provides display helpers such as min/max envelopes and
//! LTTB over decoded entries. Raw sealed chunks are the default storage codec;
//! [`GorillaF64Codec`] is available for compression experiments.
//!
//! ## Basic Use
//!
//! Create a [`Chronology`], insert monotonic [`Entry`] values, and query either
//! exact samples or mergeable summaries.
//!
//! ```
//! use chronostore::{Chronology, Direction, Entry, StatsSummary};
//!
//! let mut series = Chronology::<f64, StatsSummary<f64>>::new();
//! series
//!     .insert_values(&[
//!         Entry::new(0, 1.0),
//!         Entry::new(5, 2.5),
//!         Entry::new(10, 2.0),
//!     ])
//!     .expect("timestamps are monotonic");
//!
//! assert_eq!(
//!     series.find_nearest_value(7, Direction::Backward),
//!     Some(Entry::new(5, 2.5))
//! );
//!
//! let summary = series.range_summary(0, 11);
//! assert_eq!(summary.len, 3);
//! assert_eq!(summary.summary.max, Some(2.5));
//! ```
//!
//! ## Display Queries
//!
//! Bucketed summaries are useful for zoomed timelines and charts. With a
//! summary that exposes min/max values, [`Chronology::range_envelope`] returns
//! buckets that can be drawn as a spike-preserving graph envelope. When a
//! line-shaped sample is preferable, use [`lttb`] over exact entries.
//!
//! ```
//! use chronostore::{lttb, Chronology, Entry, StatsSummary};
//!
//! let mut series = Chronology::<f64, StatsSummary<f64>>::new();
//! series
//!     .insert_values(&[
//!         Entry::new(0, 1.0),
//!         Entry::new(1, 8.0),
//!         Entry::new(2, 2.0),
//!         Entry::new(3, 4.0),
//!     ])
//!     .expect("timestamps are monotonic");
//!
//! let envelope = series.range_envelope(0, 4, 2).collect::<Vec<_>>();
//! assert_eq!(envelope.len(), 2);
//! assert_eq!(envelope[0].max, Some(8.0));
//!
//! let mut visible_entries = Vec::new();
//! visible_entries.extend(series.entries_in_range(0, 4));
//!
//! let mut line = Vec::new();
//! for entry in lttb(&visible_entries, 3, |value| value) {
//!     line.push(entry);
//! }
//! assert_eq!(line.len(), 3);
//! ```

#![no_std]

extern crate alloc;

mod chronology;
mod entry;
mod envelope;
mod lttb;
mod null_summary;
mod simple_summary;
mod stats_summary;
mod summary;
#[cfg(test)]
mod tests;

pub use self::chronology::{
    Chronology, ChunkCodec, ChunkSummary, DEFAULT_CHUNK_CAPACITY, GorillaF64Chronology,
    GorillaF64Codec, GorillaF64EncodedChunk, GorillaF64Entries, InsertError, RangeSummary,
    RawCodec, RawEncodedChunk, RawEntries, RetentionPolicy, SUMMARY_FANOUT,
};
pub use self::entry::Entry;
pub use self::envelope::{EnvelopeBucket, EnvelopeSummary};
pub use self::lttb::lttb;
pub use self::null_summary::NullSummary;
pub use self::simple_summary::SimpleSummary;
pub use self::stats_summary::StatsSummary;
pub use self::summary::Summary;

/// Direction to search for a value from a timestamp.
///
/// This is typically used by passing it to [`Chronology::find_nearest_value()`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    /// Search backward from the timestamp.
    Backward,

    /// Search forward from the timestamp.
    Forward,
}
