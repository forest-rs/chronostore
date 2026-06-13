// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Entry, Summary};

/// A [`Summary`] that tracks the minimum and maximum values.
#[derive(Default)]
pub struct SimpleSummary<V: Copy + PartialOrd> {
    /// The minimum value seen by this summary.
    pub min: Option<V>,
    /// The maximum value seen by this summary.
    pub max: Option<V>,
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
}
