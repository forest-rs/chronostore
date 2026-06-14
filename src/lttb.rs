// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Entry;
use alloc::vec::Vec;

/// Downsample entries with Largest Triangle Three Buckets.
///
/// LTTB is a display-oriented downsampling algorithm for drawing a long series
/// as a smaller line while preserving visual shape. It uses entry timestamps as
/// the x-axis, converts them to `f64` for area comparisons, and uses
/// `project_value` to map each stored value into a y-axis coordinate.
///
/// When `target_len` is greater than or equal to `entries.len()`, all entries
/// are returned. For small targets, `target_len == 0` returns no entries,
/// `target_len == 1` returns the first entry, and `target_len == 2` returns the
/// first and last entries.
pub fn lttb<V, F>(entries: &[Entry<V>], target_len: usize, project_value: F) -> Vec<Entry<V>>
where
    V: Copy,
    F: Fn(V) -> f64,
{
    if target_len == 0 || entries.is_empty() {
        return Vec::new();
    }
    if target_len >= entries.len() {
        return entries.to_vec();
    }
    if target_len == 1 {
        return alloc::vec![entries[0]];
    }
    if target_len == 2 {
        return alloc::vec![entries[0], entries[entries.len() - 1]];
    }

    let mut sampled = Vec::with_capacity(target_len);
    let interior_len = entries.len() - 2;
    let bucket_count = target_len - 2;
    let mut previous_index = 0;

    sampled.push(entries[0]);

    for bucket in 0..(target_len - 2) {
        let current_start = scaled_bucket_boundary(bucket, interior_len, bucket_count) + 1;
        let current_end = (scaled_bucket_boundary(bucket + 1, interior_len, bucket_count) + 1)
            .min(entries.len() - 1);
        let next_start = (scaled_bucket_boundary(bucket + 1, interior_len, bucket_count) + 1)
            .min(entries.len() - 1);
        let next_end =
            (scaled_bucket_boundary(bucket + 2, interior_len, bucket_count) + 1).min(entries.len());
        let (average_x, average_y) = average_point(&entries[next_start..next_end], &project_value)
            .unwrap_or_else(|| {
                let last = entries[entries.len() - 1];
                (last.timestamp as f64, project_value(last.value))
            });

        let previous = entries[previous_index];
        let previous_x = previous.timestamp as f64;
        let previous_y = project_value(previous.value);
        let mut selected_index = current_start;
        let mut selected_area = f64::NEG_INFINITY;

        for (index, candidate) in entries
            .iter()
            .copied()
            .enumerate()
            .take(current_end)
            .skip(current_start)
        {
            let candidate_x = candidate.timestamp as f64;
            let candidate_y = project_value(candidate.value);
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

        sampled.push(entries[selected_index]);
        previous_index = selected_index;
    }

    sampled.push(entries[entries.len() - 1]);
    sampled
}

fn scaled_bucket_boundary(bucket: usize, interior_len: usize, bucket_count: usize) -> usize {
    ((bucket as u128 * interior_len as u128) / bucket_count as u128) as usize
}

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

fn triangle_area(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    ((ax - cx) * (by - ay) - (ax - bx) * (cy - ay)).abs() * 0.5
}
