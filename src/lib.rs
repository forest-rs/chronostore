// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # Chronostore
//!
//! Chronostore is a system for storing time series in memory.
//! Chronostore is intended for use wihh datasets where a
//! single time series has 100 million or fewer points.
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
//! The initial implementation is quite naive and is just
//! here to get something working. Over time, the implementation
//! will evolve and become significantly more sophisticated.

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
mod null_summary;
mod simple_summary;
mod summary;

pub use self::chronology::{Chronology, InsertError, DEFAULT_CHUNK_CAPACITY};
pub use self::entry::Entry;
pub use self::null_summary::NullSummary;
pub use self::simple_summary::SimpleSummary;
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

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn basics() {
        let mut v = Chronology::<f32, NullSummary<f32>>::new();
        v.insert_values(&[
            Entry::new(5, 2.0),
            Entry::new(10, 3.0),
            Entry::new(15, 4.0),
            Entry::new(20, 5.0),
        ])
        .expect("timestamps are monotonic");
        assert_eq!(
            v.find_nearest_value(2, Direction::Forward),
            Some(Entry::new(5, 2.0))
        );
        assert_eq!(
            v.find_nearest_value(12, Direction::Forward),
            Some(Entry::new(15, 4.0))
        );
        assert_eq!(v.find_nearest_value(22, Direction::Forward), None);
    }

    #[test]
    fn searches_backward() {
        let mut chronology = Chronology::<f32, NullSummary<f32>>::new();
        chronology
            .insert_values(&[
                Entry::new(5, 2.0),
                Entry::new(10, 3.0),
                Entry::new(15, 4.0),
                Entry::new(20, 5.0),
            ])
            .expect("timestamps are monotonic");

        assert_eq!(chronology.find_nearest_value(2, Direction::Backward), None);
        assert_eq!(
            chronology.find_nearest_value(12, Direction::Backward),
            Some(Entry::new(10, 3.0))
        );
        assert_eq!(
            chronology.find_nearest_value(20, Direction::Backward),
            Some(Entry::new(20, 5.0))
        );
        assert_eq!(
            chronology.find_nearest_value(22, Direction::Backward),
            Some(Entry::new(20, 5.0))
        );
    }

    #[test]
    fn searches_across_chunks() {
        let mut chronology = Chronology::<f32, NullSummary<f32>>::with_chunk_capacity(2);
        chronology
            .insert_values(&[
                Entry::new(5, 2.0),
                Entry::new(10, 3.0),
                Entry::new(15, 4.0),
                Entry::new(20, 5.0),
                Entry::new(25, 6.0),
            ])
            .expect("timestamps are monotonic");

        assert_eq!(chronology.len(), 5);
        assert_eq!(chronology.sealed_chunk_count(), 2);
        assert_eq!(
            chronology.find_nearest_value(11, Direction::Forward),
            Some(Entry::new(15, 4.0))
        );
        assert_eq!(
            chronology.find_nearest_value(21, Direction::Backward),
            Some(Entry::new(20, 5.0))
        );
    }

    #[test]
    fn rejects_non_monotonic_batches_without_mutating() {
        let mut chronology = Chronology::<f32, NullSummary<f32>>::new();
        chronology
            .insert_values(&[Entry::new(5, 2.0), Entry::new(10, 3.0)])
            .expect("timestamps are monotonic");

        assert_eq!(
            chronology.insert_values(&[Entry::new(12, 4.0), Entry::new(11, 5.0)]),
            Err(InsertError::NonMonotonicTimestamp {
                previous: 12,
                next: 11,
            })
        );
        assert_eq!(chronology.len(), 2);
        assert_eq!(
            chronology.find_nearest_value(12, Direction::Backward),
            Some(Entry::new(10, 3.0))
        );
    }

    #[test]
    fn simple_summary_tracks_empty_positive_and_negative_ranges() {
        let mut chronology = Chronology::<f32, SimpleSummary<f32>>::new();
        assert_eq!(chronology.summary().min, None);
        assert_eq!(chronology.summary().max, None);

        chronology
            .insert_values(&[Entry::new(1, 3.0), Entry::new(2, 8.0)])
            .expect("timestamps are monotonic");
        assert_eq!(chronology.summary().min, Some(3.0));
        assert_eq!(chronology.summary().max, Some(8.0));

        chronology
            .insert_values(&[Entry::new(3, -5.0), Entry::new(4, -2.0)])
            .expect("timestamps are monotonic");
        assert_eq!(chronology.summary().min, Some(-5.0));
        assert_eq!(chronology.summary().max, Some(8.0));
    }
}
