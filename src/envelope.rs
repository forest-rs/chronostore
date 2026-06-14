// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{RangeSummary, SimpleSummary, StatsSummary};

/// A summary that can expose minimum and maximum values for graph envelopes.
pub trait EnvelopeSummary<V: Copy> {
    /// Return the minimum value represented by this summary.
    fn min_value(&self) -> Option<V>;

    /// Return the maximum value represented by this summary.
    fn max_value(&self) -> Option<V>;
}

/// A min/max bucket for rendering a range envelope.
///
/// Each bucket covers `start..end` and represents `len` samples. Empty buckets
/// have `len == 0` and no min/max values.
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
        EnvelopeBucket {
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
