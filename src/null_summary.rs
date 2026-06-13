// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Entry, Summary};
use core::marker::PhantomData;

/// A [`Summary`] that does nothing.
pub struct NullSummary<V> {
    phantom: PhantomData<V>,
}

impl<V> Clone for NullSummary<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for NullSummary<V> {}

impl<V> Default for NullSummary<V> {
    fn default() -> Self {
        NullSummary {
            phantom: PhantomData,
        }
    }
}

impl<V> Summary<V> for NullSummary<V> {
    fn update(&mut self, _entry: &Entry<V>) {}

    fn merge(&mut self, _other: &Self) {}
}
