// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Summary;

/// Owned summary metadata for a timestamp range.
///
/// `RangeSummary` is returned by [`Chronology::range_summary`](crate::Chronology::range_summary)
/// and [`Chronology::summarize_range`](crate::Chronology::summarize_range), and
/// is visited by [`Chronology::visit_range_summaries`](crate::Chronology::visit_range_summaries).
/// The `summary` field is the caller-selected [`Summary`] implementation for
/// the entries whose timestamps fall inside `start..end`.
#[derive(Clone)]
pub struct RangeSummary<S> {
    /// Inclusive start of the summarized timestamp range.
    pub start: u64,
    /// Exclusive end of the summarized timestamp range.
    pub end: u64,
    /// Number of entries represented by this summary.
    pub len: usize,
    /// Summary for entries whose timestamps fall inside `start..end`.
    pub summary: S,
}

impl<S> RangeSummary<S> {
    pub(super) fn empty(start: u64, end: u64) -> Self
    where
        S: Default,
    {
        RangeSummary {
            start,
            end,
            len: 0,
            summary: S::default(),
        }
    }

    pub(super) fn add_summary<V>(&mut self, len: usize, summary: &S)
    where
        S: Summary<V>,
    {
        if len == 0 {
            return;
        }
        self.len += len;
        self.summary.merge(summary);
    }
}
