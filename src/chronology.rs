// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Direction, Entry, Summary};
use alloc::vec::Vec;

/// A stream of values over time for a single variable.
///
/// A chronology stores timestamped values where the timestamp
/// is when the new value was set.
///
/// ## Values
///
/// Values stored within a [`Chronology`] must implement the [`Copy`]
/// trait. Values are copied when they are stored within the
/// [`Chronology`]. For this reason, it is typically advisable to
/// keep them simple and easy to copy if you're dealing with
/// large numbers of values and need the highest levels of
/// performance.
///
/// ## Nature of Timestamps
///
/// Timestamps are represented as unsigned 64 bit integer values.
/// The exact interpretation of this value is up to the producer
/// and consumer of the data.
///
/// Typical interpretations might be:
///
/// * 1 unit is 1 second.
/// * 1 unit is 1 millisecond.
/// * 1 unit is 1 nanosecond.
/// * 1 unit is 10 nanoseconds.
/// * 1 unit is 100 picoseconds.
///
/// Timestamps may also be interpreted as being an absolute point
/// in time or a relative point in time, again up to the application
/// producing and consuming the data.
///
/// Some applications may be happy tracking number of seconds since
/// 1900. Others are using timestamps that correspond to the number
/// of nanoseconds since the application started or the CPU was
/// powered on.
///
/// ## Inserting Values
///
/// Values are inserted in bulk via [`Chronology::insert_values`].
/// [`Entry`] wraps values along with their timestamp.
///
/// ```
/// use chronostore::{Chronology, Entry, NullSummary};
///
/// let mut chrono = Chronology::<f32, NullSummary<f32>>::new();
/// chrono.insert_values(&[Entry::new(0, 0.3),
///                        Entry::new(5, 0.5)]);
/// ```
///
/// ## Querying Values
///
/// A [`Chronology`] can be [queried](Chronology::find_nearest_value) for
/// the current value at any point in time. It will find either the last
/// value set prior to the point in time by searching with
/// [`Direction::Backward`] or the next value that has been set by
/// searching with [`Direction::Forward`].
///
/// ```
/// use chronostore::{Chronology, Direction, Entry, NullSummary};
///
/// let mut chrono = Chronology::<f32, NullSummary<f32>>::new();
/// chrono.insert_values(&[Entry::new(0, 0.3),
///                        Entry::new(5, 0.5)]);
///
/// assert_eq!(chrono.find_nearest_value(4, Direction::Forward),
///            Some(Entry::new(5, 0.5)));
/// ```
#[derive(Default)]
pub struct Chronology<V: Copy + Default, S: Default + Summary<V>> {
    timestamps: Vec<u64>,
    values: Vec<V>,
    summary: S,
}

impl<V: Copy + Default, S: Default + Summary<V>> Chronology<V, S> {
    /// Create a new `Chronology`.
    pub fn new() -> Self {
        Chronology::default()
    }

    /// Find the nearest value in time.
    ///
    /// The `direction` is not currently used.
    pub fn find_nearest_value(&self, timestamp: u64, _direction: Direction) -> Option<Entry<V>> {
        self.timestamps
            .iter()
            .position(|&t| t > timestamp)
            .map(|idx| Entry::new(self.timestamps[idx], self.values[idx]))
    }

    /// Record a set of values and their timestamps.
    pub fn insert_values(&mut self, values: &[Entry<V>]) {
        for v in values {
            self.timestamps.push(v.timestamp);
            self.values.push(v.value);
        }
        self.summary.batch_update(values);
    }
}
