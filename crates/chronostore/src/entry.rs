// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A timestamped sample stored in or returned from a chronology.
///
/// `Entry` is the named pair used by [`Chronology`](crate::Chronology) at the
/// API boundary: callers pass entries to
/// [`Chronology::insert_value`](crate::Chronology::insert_value) and
/// [`Chronology::insert_values`](crate::Chronology::insert_values), and receive
/// entries from exact queries such as
/// [`Chronology::find_nearest_value`](crate::Chronology::find_nearest_value),
/// and [`Chronology::entries_in_range`](crate::Chronology::entries_in_range).
///
/// ```
/// use chronostore::Entry;
///
/// let sample = Entry::new(42, 3.5);
/// assert_eq!(sample.timestamp, 42);
/// assert_eq!(sample.value, 3.5);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry<V> {
    /// Timestamp associated with the sample.
    ///
    /// Chronostore treats timestamps as opaque monotonic `u64` values. The
    /// unit is chosen by the producer.
    pub timestamp: u64,

    /// The recorded value.
    pub value: V,
}

impl<V> Entry<V> {
    /// Create a new timestamped sample.
    pub fn new(timestamp: u64, value: V) -> Self {
        Entry { timestamp, value }
    }
}
