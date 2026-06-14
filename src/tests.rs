// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::*;
use alloc::vec::Vec;

#[test]
fn basics() {
    let mut v = Chronology::<f32, NullSummary<f32>>::new();
    v.insert_values(&[
        Entry::new(5, 2.0),
        Entry::new(10, 3.0),
        Entry::new(15, 4.0),
        Entry::new(20, 5.0),
    ])
    .expect("timestamps are monotonic");
    assert_eq!(
        v.find_nearest_value(2, Direction::Forward),
        Some(Entry::new(5, 2.0))
    );
    assert_eq!(
        v.find_nearest_value(12, Direction::Forward),
        Some(Entry::new(15, 4.0))
    );
    assert_eq!(v.find_nearest_value(22, Direction::Forward), None);
}

#[test]
fn searches_backward() {
    let mut chronology = Chronology::<f32, NullSummary<f32>>::new();
    chronology
        .insert_values(&[
            Entry::new(5, 2.0),
            Entry::new(10, 3.0),
            Entry::new(15, 4.0),
            Entry::new(20, 5.0),
        ])
        .expect("timestamps are monotonic");

    assert_eq!(chronology.find_nearest_value(2, Direction::Backward), None);
    assert_eq!(
        chronology.find_nearest_value(12, Direction::Backward),
        Some(Entry::new(10, 3.0))
    );
    assert_eq!(
        chronology.find_nearest_value(20, Direction::Backward),
        Some(Entry::new(20, 5.0))
    );
    assert_eq!(
        chronology.find_nearest_value(22, Direction::Backward),
        Some(Entry::new(20, 5.0))
    );
}

#[test]
fn searches_across_chunks() {
    let mut chronology = Chronology::<f32, NullSummary<f32>>::with_chunk_capacity(2);
    chronology
        .insert_values(&[
            Entry::new(5, 2.0),
            Entry::new(10, 3.0),
            Entry::new(15, 4.0),
            Entry::new(20, 5.0),
            Entry::new(25, 6.0),
        ])
        .expect("timestamps are monotonic");

    assert_eq!(chronology.len(), 5);
    assert_eq!(chronology.sealed_chunk_count(), 2);
    assert_eq!(
        chronology.find_nearest_value(11, Direction::Forward),
        Some(Entry::new(15, 4.0))
    );
    assert_eq!(
        chronology.find_nearest_value(21, Direction::Backward),
        Some(Entry::new(20, 5.0))
    );
}

#[test]
fn rejects_non_monotonic_batches_without_mutating() {
    let mut chronology = Chronology::<f32, NullSummary<f32>>::new();
    chronology
        .insert_values(&[Entry::new(5, 2.0), Entry::new(10, 3.0)])
        .expect("timestamps are monotonic");

    assert_eq!(
        chronology.insert_values(&[Entry::new(12, 4.0), Entry::new(11, 5.0)]),
        Err(InsertError::NonMonotonicTimestamp {
            previous: 12,
            next: 11,
        })
    );
    assert_eq!(chronology.len(), 2);
    assert_eq!(
        chronology.find_nearest_value(12, Direction::Backward),
        Some(Entry::new(10, 3.0))
    );
}

#[test]
fn simple_summary_tracks_empty_positive_and_negative_ranges() {
    let mut chronology = Chronology::<f32, SimpleSummary<f32>>::new();
    assert_eq!(chronology.summary().min, None);
    assert_eq!(chronology.summary().max, None);

    chronology
        .insert_values(&[Entry::new(1, 3.0), Entry::new(2, 8.0)])
        .expect("timestamps are monotonic");
    assert_eq!(chronology.summary().min, Some(3.0));
    assert_eq!(chronology.summary().max, Some(8.0));

    chronology
        .insert_values(&[Entry::new(3, -5.0), Entry::new(4, -2.0)])
        .expect("timestamps are monotonic");
    assert_eq!(chronology.summary().min, Some(-5.0));
    assert_eq!(chronology.summary().max, Some(8.0));
}

#[test]
fn exposes_chunk_summaries_and_summary_pyramid_shape() {
    let mut chronology = Chronology::<u64, SimpleSummary<u64>>::with_chunk_capacity(2);
    let entries = (0..18)
        .map(|value| Entry::new(value, value))
        .collect::<Vec<_>>();
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    assert_eq!(chronology.chunk_count(), 9);
    assert_eq!(chronology.sealed_chunk_count(), 9);
    assert_eq!(chronology.summary_level_count(), 2);
    assert_eq!(chronology.summary_node_count(0), Some(9));
    assert_eq!(chronology.summary_node_count(1), Some(1));

    let first = chronology.chunk_summary(0).expect("first chunk exists");
    assert_eq!(first.start, 0);
    assert_eq!(first.end, 1);
    assert_eq!(first.len, 2);
    assert_eq!(first.summary.min, Some(0));
    assert_eq!(first.summary.max, Some(1));
}

#[test]
fn summarizes_half_open_timestamp_ranges() {
    let mut chronology = Chronology::<u64, SimpleSummary<u64>>::with_chunk_capacity(2);
    let entries = (0..20)
        .map(|value| Entry::new(value, value))
        .collect::<Vec<_>>();
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    let summary = chronology.range_summary(3, 15);
    assert_eq!(summary.start, 3);
    assert_eq!(summary.end, 15);
    assert_eq!(summary.len, 12);
    assert_eq!(summary.summary.min, Some(3));
    assert_eq!(summary.summary.max, Some(14));

    let empty = chronology.range_summary(15, 15);
    assert_eq!(empty.len, 0);
    assert_eq!(empty.summary.min, None);
    assert_eq!(empty.summary.max, None);
}

#[test]
fn summarizes_viewport_buckets() {
    let mut chronology = Chronology::<u64, SimpleSummary<u64>>::with_chunk_capacity(2);
    let entries = (0..16)
        .map(|value| Entry::new(value, value))
        .collect::<Vec<_>>();
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    let summaries = chronology.summarize_range(0, 16, 4);
    assert_eq!(summaries.len(), 4);
    assert_eq!(summaries[0].len, 4);
    assert_eq!(summaries[0].summary.min, Some(0));
    assert_eq!(summaries[0].summary.max, Some(3));
    assert_eq!(summaries[3].len, 4);
    assert_eq!(summaries[3].summary.min, Some(12));
    assert_eq!(summaries[3].summary.max, Some(15));
}

#[test]
fn retention_keeps_latest_sealed_chunks() {
    let mut chronology = Chronology::<u64, SimpleSummary<u64>>::with_chunk_capacity_and_retention(
        2,
        RetentionPolicy::max_sealed_chunks(2),
    );
    let entries = (0..10)
        .map(|value| Entry::new(value, value))
        .collect::<Vec<_>>();
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    assert_eq!(chronology.len(), 4);
    assert_eq!(chronology.sealed_chunk_count(), 2);
    assert_eq!(chronology.chunk_count(), 2);
    assert_eq!(
        chronology.find_nearest_value(5, Direction::Forward),
        Some(Entry::new(6, 6))
    );
    assert_eq!(chronology.find_nearest_value(5, Direction::Backward), None);
    assert_eq!(chronology.summary().min, Some(6));
    assert_eq!(chronology.summary().max, Some(9));
}

#[test]
fn setting_retention_rebuilds_summaries_and_keeps_open_chunk() {
    let mut chronology = Chronology::<u64, SimpleSummary<u64>>::with_chunk_capacity(2);
    let entries = (0..9)
        .map(|value| Entry::new(value, value))
        .collect::<Vec<_>>();
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    chronology.set_retention_policy(RetentionPolicy::max_sealed_chunks(2));

    assert_eq!(chronology.len(), 5);
    assert_eq!(chronology.sealed_chunk_count(), 2);
    assert_eq!(chronology.chunk_count(), 3);
    assert_eq!(
        chronology.find_nearest_value(0, Direction::Forward),
        Some(Entry::new(4, 4))
    );
    assert_eq!(chronology.summary().min, Some(4));
    assert_eq!(chronology.summary().max, Some(8));

    let summary = chronology.range_summary(0, 9);
    assert_eq!(summary.len, 5);
    assert_eq!(summary.summary.min, Some(4));
    assert_eq!(summary.summary.max, Some(8));
}
