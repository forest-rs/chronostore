// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{RangeSummary, SimpleSummary, StatsSummary};

/// A summary that can expose minimum and maximum values for graph envelopes.
///
/// Implement this for summary types that should work with
/// [`Chronology::range_envelope`](crate::Chronology::range_envelope).
/// Chronostore implements it for [`SimpleSummary`] and [`StatsSummary`].
pub trait EnvelopeSummary<V: Copy> {
    /// Return the minimum value represented by this summary.
    fn min_value(&self) -> Option<V>;

    /// Return the maximum value represented by this summary.
    fn max_value(&self) -> Option<V>;
}

/// A min/max bucket for rendering a range envelope.
///
/// `EnvelopeBucket` is returned by
/// the iterator from [`Chronology::range_envelope`](crate::Chronology::range_envelope).
/// Each bucket covers `start..end` and represents `len` samples. Empty buckets
/// have `len == 0` and no min/max values.
///
/// ```
/// use chronostore::{Chronology, Entry, StatsSummary};
///
/// let mut series = Chronology::<u64, StatsSummary<u64>>::new();
/// series
///     .insert_values(&[Entry::new(0, 4), Entry::new(1, 9), Entry::new(2, 5)])
///     .expect("timestamps are monotonic");
///
/// let buckets = series.range_envelope(0, 3, 1).collect::<Vec<_>>();
/// assert_eq!(buckets[0].min, Some(4));
/// assert_eq!(buckets[0].max, Some(9));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeBucket<V> {
    /// Inclusive start of the bucket timestamp range.
    pub start: u64,
    /// Exclusive end of the bucket timestamp range.
    pub end: u64,
    /// Number of entries represented by this bucket.
    pub len: usize,
    /// Minimum value in this bucket.
    pub min: Option<V>,
    /// Maximum value in this bucket.
    pub max: Option<V>,
}

impl<V: Copy> EnvelopeBucket<V> {
    /// Build an envelope bucket from a range summary with min/max metadata.
    pub fn from_range_summary<S>(range_summary: RangeSummary<S>) -> Self
    where
        S: EnvelopeSummary<V>,
    {
        Self {
            start: range_summary.start,
            end: range_summary.end,
            len: range_summary.len,
            min: range_summary.summary.min_value(),
            max: range_summary.summary.max_value(),
        }
    }
}

impl<V: Copy + PartialOrd> EnvelopeSummary<V> for SimpleSummary<V> {
    fn min_value(&self) -> Option<V> {
        self.min
    }

    fn max_value(&self) -> Option<V> {
        self.max
    }
}

impl<V: Copy> EnvelopeSummary<V> for StatsSummary<V> {
    fn min_value(&self) -> Option<V> {
        self.min
    }

    fn max_value(&self) -> Option<V> {
        self.max
    }
}
