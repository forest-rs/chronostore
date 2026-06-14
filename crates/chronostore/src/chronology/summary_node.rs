// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
    pub(super) fn merge_nodes<V>(nodes: &[Self]) -> Self
    where
        S: Summary<V>,
    {
        debug_assert!(
            !nodes.is_empty(),
            "summary merge requires at least one node"
        );

        let mut summary = nodes[0].summary.clone();
        let mut len = nodes[0].len;

        for node in &nodes[1..] {
            len += node.len;
            summary.merge(&node.summary);
        }

        Self {
            start: nodes[0].start,
            end: nodes[nodes.len() - 1].end,
            len,
            summary,
        }
    }
}
