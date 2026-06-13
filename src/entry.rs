// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A paired timestamp and value. These are used when
/// storing data within a [`Chronology`](crate::Chronology)
/// and returned when querying.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry<V> {
    /// Timestamp for which the value is valid.
    pub timestamp: u64,

    /// The value to be recorded.
    pub value: V,
}

impl<V> Entry<V> {
    /// Create a new [`Entry`].
    pub fn new(timestamp: u64, value: V) -> Self {
        Entry { timestamp, value }
    }
}
