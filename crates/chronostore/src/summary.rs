// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Entry;

/// Pluggable means of maintaining summary information about the
/// data stored in a [`Chronology`](crate::Chronology).
///
/// `Summary` is the second generic parameter of [`Chronology`](crate::Chronology).
/// Chronostore updates it during insertion and merges it when answering range,
/// bucketed-summary, and envelope queries.
///
/// Summary values are mergeable so callers can build range and viewport
/// summaries without decoding every sample. Implementations should make
/// [`Summary::merge`] associative: merging `a` with `b`, then with `c`, should
/// describe the same data as merging `a` with the result of `b` and `c`.
pub trait Summary<V>: Clone + Default {
    /// Return an empty summary value.
    fn empty() -> Self {
        Self::default()
    }

    /// Update the summary with a batch of entries.
    ///
    /// Some summary implementations may be able to operate
    /// more efficiently in batch form rather than updating
    /// over and over for each individual [`Entry`].
    fn batch_update(&mut self, entries: &[Entry<V>]) {
        for entry in entries {
            self.update(entry);
        }
    }

    /// Update the summary with a single new [`Entry`].
    fn update(&mut self, entry: &Entry<V>);

    /// Merge another summary into this summary.
    fn merge(&mut self, other: &Self);
}
