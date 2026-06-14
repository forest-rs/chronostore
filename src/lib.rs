// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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
//! a Gorilla-inspired `f64` codec is available for compression experiments.

#![no_std]
#![warn(clippy::doc_markdown, missing_docs)]
#![deny(
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

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
    Chronology, ChunkCodec, ChunkSummary, GorillaF64Chronology, GorillaF64Codec, InsertError,
    RangeSummary, RawCodec, RetentionPolicy, DEFAULT_CHUNK_CAPACITY, SUMMARY_FANOUT,
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
