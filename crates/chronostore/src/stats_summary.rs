// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Entry, Summary};
use core::ops::AddAssign;

/// A [`Summary`] that tracks common numeric statistics.
///
/// Use `StatsSummary<V>` as the summary parameter for a
/// [`Chronology`](crate::Chronology) when range queries need min/max, sum,
/// count, and latest value metadata. Because it exposes min/max values, it also
/// works with [`Chronology::range_envelope`](crate::Chronology::range_envelope).
///
/// `StatsSummary` tracks minimum, maximum, sum, count, and the latest value in
/// insertion order. Chronostore merges summaries in chronological order, so the
/// `latest` field remains the last value covered by the merged summary.
///
/// ```
/// use chronostore::{Chronology, Entry, StatsSummary};
///
/// let mut series = Chronology::<f64, StatsSummary<f64>>::new();
/// series
///     .insert_values(&[Entry::new(0, 1.0), Entry::new(1, 3.0)])
///     .expect("timestamps are monotonic");
///
/// assert_eq!(series.summary().sum, 4.0);
/// assert_eq!(series.summary().latest, Some(3.0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatsSummary<V> {
    /// The minimum value seen by this summary.
    pub min: Option<V>,
    /// The maximum value seen by this summary.
    pub max: Option<V>,
    /// Sum of all values seen by this summary.
    pub sum: V,
    /// Number of values seen by this summary.
    pub count: usize,
    /// The latest value seen by this summary in insertion order.
    pub latest: Option<V>,
}

impl<V: Default> Default for StatsSummary<V> {
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            sum: V::default(),
            count: 0,
            latest: None,
        }
    }
}

impl<V> Summary<V> for StatsSummary<V>
where
    V: Copy + Default + AddAssign + PartialOrd,
{
    fn update(&mut self, entry: &Entry<V>) {
        self.max = match self.max {
            Some(max) if max >= entry.value => Some(max),
            _ => Some(entry.value),
        };
        self.min = match self.min {
            Some(min) if min <= entry.value => Some(min),
            _ => Some(entry.value),
        };
        self.sum += entry.value;
        self.count += 1;
        self.latest = Some(entry.value);
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
        self.sum += other.sum;
        self.count += other.count;
        if other.count > 0 {
            self.latest = other.latest;
        }
    }
}
