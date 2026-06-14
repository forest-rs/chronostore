// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Entry, Summary};

/// A [`Summary`] that tracks the minimum and maximum values.
///
/// Use `SimpleSummary<V>` as the summary parameter for a
/// [`Chronology`](crate::Chronology) when range queries and envelopes only need
/// min/max values.
#[derive(Clone, Copy)]
pub struct SimpleSummary<V: Copy + PartialOrd> {
    /// The minimum value seen by this summary.
    pub min: Option<V>,
    /// The maximum value seen by this summary.
    pub max: Option<V>,
}

impl<V: Copy + PartialOrd> Default for SimpleSummary<V> {
    fn default() -> Self {
        SimpleSummary {
            min: None,
            max: None,
        }
    }
}

impl<V: Copy + PartialOrd> Summary<V> for SimpleSummary<V> {
    fn update(&mut self, entry: &Entry<V>) {
        self.max = match self.max {
            Some(max) if max >= entry.value => Some(max),
            _ => Some(entry.value),
        };
        self.min = match self.min {
            Some(min) if min <= entry.value => Some(min),
            _ => Some(entry.value),
        };
    }

    fn merge(&mut self, other: &Self) {
        if let Some(max) = other.max {
            self.max = match self.max {
                Some(current) if current >= max => Some(current),
                _ => Some(max),
            };
        }
        if let Some(min) = other.min {
            self.min = match self.min {
                Some(current) if current <= min => Some(current),
                _ => Some(min),
            };
        }
    }
}
