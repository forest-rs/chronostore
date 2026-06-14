// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Entry;

/// Downsample entries with Largest Triangle Three Buckets.
///
/// LTTB is a display-oriented downsampling algorithm for drawing a long series
/// as a smaller line while preserving visual shape. It uses entry timestamps as
/// the x-axis, converts them to `f64` for area comparisons, and uses
/// `project_value` to map each stored value into a y-axis coordinate.
///
/// The algorithm was defined by Sveinn Steinarsson in [*Downsampling Time
/// Series for Visual Representation*](https://skemman.is/bitstream/1946/15343/3/SS_MSthesis.pdf),
/// 2013.
///
/// This function borrows the input slice and returns an iterator. It does not
/// allocate and does not copy every input entry. Callers that want to reuse an
/// output allocation can clear a buffer and push the selected entries into it.
///
/// When `target_len` is greater than or equal to `entries.len()`, all entries
/// are returned. For small targets, `target_len == 0` returns no entries,
/// `target_len == 1` returns the first entry, and `target_len == 2` returns the
/// first and last entries.
///
/// ```
/// use chronostore::{lttb, Entry};
///
/// let entries = [
///     Entry::new(0, 1.0),
///     Entry::new(1, 12.0),
///     Entry::new(2, 2.0),
///     Entry::new(3, 3.0),
/// ];
///
/// let sampled = lttb(&entries, 3, |value| value).collect::<Vec<_>>();
/// assert_eq!(sampled.first(), Some(&Entry::new(0, 1.0)));
/// assert_eq!(sampled.last(), Some(&Entry::new(3, 3.0)));
/// assert_eq!(sampled.len(), 3);
/// ```
pub fn lttb<'a, V, F>(
    entries: &'a [Entry<V>],
    target_len: usize,
    project_value: F,
) -> impl Iterator<Item = Entry<V>> + 'a
where
    V: Copy,
    F: Fn(V) -> f64 + 'a,
{
    Lttb::new(entries, target_len, project_value)
}

struct Lttb<'a, V, F>
where
    V: Copy,
    F: Fn(V) -> f64,
{
    entries: &'a [Entry<V>],
    target_len: usize,
    project_value: F,
    output_index: usize,
    previous_index: usize,
}

impl<'a, V, F> Lttb<'a, V, F>
where
    V: Copy,
    F: Fn(V) -> f64,
{
    #[inline]
    fn new(entries: &'a [Entry<V>], target_len: usize, project_value: F) -> Self {
        Lttb {
            entries,
            target_len,
            project_value,
            output_index: 0,
            previous_index: 0,
        }
    }

    #[inline]
    fn output_len(&self) -> usize {
        self.target_len.min(self.entries.len())
    }

    #[inline]
    fn selected_entry(&mut self) -> Entry<V> {
        let bucket = self.output_index - 1;
        let interior_len = self.entries.len() - 2;
        let bucket_count = self.target_len - 2;
        let current_start = scaled_bucket_boundary(bucket, interior_len, bucket_count) + 1;
        let current_end = (scaled_bucket_boundary(bucket + 1, interior_len, bucket_count) + 1)
            .min(self.entries.len() - 1);
        let next_start = (scaled_bucket_boundary(bucket + 1, interior_len, bucket_count) + 1)
            .min(self.entries.len() - 1);
        let next_end = (scaled_bucket_boundary(bucket + 2, interior_len, bucket_count) + 1)
            .min(self.entries.len());
        let (average_x, average_y) =
            average_point(&self.entries[next_start..next_end], &self.project_value).unwrap_or_else(
                || {
                    let last = self.entries[self.entries.len() - 1];
                    (last.timestamp as f64, (self.project_value)(last.value))
                },
            );

        let previous = self.entries[self.previous_index];
        let previous_x = previous.timestamp as f64;
        let previous_y = (self.project_value)(previous.value);
        let mut selected_index = current_start;
        let mut selected_area = f64::NEG_INFINITY;

        for (index, candidate) in self
            .entries
            .iter()
            .copied()
            .enumerate()
            .take(current_end)
            .skip(current_start)
        {
            let candidate_x = candidate.timestamp as f64;
            let candidate_y = (self.project_value)(candidate.value);
            let area = triangle_area(
                previous_x,
                previous_y,
                candidate_x,
                candidate_y,
                average_x,
                average_y,
            );

            if area > selected_area {
                selected_area = area;
                selected_index = index;
            }
        }

        self.previous_index = selected_index;
        self.entries[selected_index]
    }
}

impl<V, F> Iterator for Lttb<'_, V, F>
where
    V: Copy,
    F: Fn(V) -> f64,
{
    type Item = Entry<V>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let output_len = self.output_len();
        if self.output_index >= output_len {
            return None;
        }

        let entry = if output_len == self.entries.len() {
            self.entries[self.output_index]
        } else if output_len == 1 {
            self.entries[0]
        } else if output_len == 2 {
            match self.output_index {
                0 => self.entries[0],
                _ => self.entries[self.entries.len() - 1],
            }
        } else {
            match self.output_index {
                0 => self.entries[0],
                index if index == output_len - 1 => self.entries[self.entries.len() - 1],
                _ => self.selected_entry(),
            }
        };

        self.output_index += 1;
        Some(entry)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.output_len() - self.output_index;
        (remaining, Some(remaining))
    }
}

impl<V, F> ExactSizeIterator for Lttb<'_, V, F>
where
    V: Copy,
    F: Fn(V) -> f64,
{
}

#[inline]
fn scaled_bucket_boundary(bucket: usize, interior_len: usize, bucket_count: usize) -> usize {
    ((bucket as u128 * interior_len as u128) / bucket_count as u128) as usize
}

#[inline]
fn average_point<V, F>(entries: &[Entry<V>], project_value: &F) -> Option<(f64, f64)>
where
    V: Copy,
    F: Fn(V) -> f64,
{
    if entries.is_empty() {
        return None;
    }

    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for entry in entries {
        sum_x += entry.timestamp as f64;
        sum_y += project_value(entry.value);
    }

    let len = entries.len() as f64;
    Some((sum_x / len, sum_y / len))
}

#[inline]
fn triangle_area(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    ((ax - cx) * (by - ay) - (ax - bx) * (cy - ay)).abs() * 0.5
}
