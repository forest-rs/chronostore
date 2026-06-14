// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Summary;

/// Number of child summary nodes merged into each higher-level summary node.
pub const SUMMARY_FANOUT: usize = 8;

pub(super) struct SummaryNode<S> {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) len: usize,
    pub(super) summary: S,
}

impl<S> SummaryNode<S> {
    pub(super) fn merge_nodes<V>(nodes: &[SummaryNode<S>]) -> Self
    where
        S: Summary<V>,
    {
        debug_assert!(!nodes.is_empty());

        let mut summary = nodes[0].summary.clone();
        let mut len = nodes[0].len;

        for node in &nodes[1..] {
            len += node.len;
            summary.merge(&node.summary);
        }

        SummaryNode {
            start: nodes[0].start,
            end: nodes[nodes.len() - 1].end,
            len,
            summary,
        }
    }
}
