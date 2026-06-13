// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Entry, Summary};
use core::marker::PhantomData;

/// A [`Summary`] that does nothing.
#[derive(Default)]
pub struct NullSummary<V> {
    phantom: PhantomData<V>,
}

impl<V> Summary<V> for NullSummary<V> {
    fn batch_update(&mut self, _entries: &[Entry<V>]) {}

    fn update(&mut self, _entry: &Entry<V>) {}
}
