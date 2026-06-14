// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::*;
use alloc::vec;
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
fn stats_summary_tracks_common_numeric_fields() {
    let mut chronology = Chronology::<f64, StatsSummary<f64>>::with_chunk_capacity(2);
    chronology
        .insert_values(&[
            Entry::new(1, 3.0),
            Entry::new(2, 8.0),
            Entry::new(3, -5.0),
            Entry::new(4, 2.0),
        ])
        .expect("timestamps are monotonic");

    assert_eq!(chronology.summary().min, Some(-5.0));
    assert_eq!(chronology.summary().max, Some(8.0));
    assert_eq!(chronology.summary().sum, 8.0);
    assert_eq!(chronology.summary().count, 4);
    assert_eq!(chronology.summary().latest, Some(2.0));

    let range = chronology.range_summary(2, 4);
    assert_eq!(range.len, 2);
    assert_eq!(range.summary.min, Some(-5.0));
    assert_eq!(range.summary.max, Some(8.0));
    assert_eq!(range.summary.sum, 3.0);
    assert_eq!(range.summary.count, 2);
    assert_eq!(range.summary.latest, Some(-5.0));
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
fn returns_exact_entries_in_half_open_ranges() {
    let mut chronology = Chronology::<u64, NullSummary<u64>>::with_chunk_capacity(3);
    let entries = (0..8)
        .map(|value| Entry::new(value * 10, value))
        .collect::<Vec<_>>();
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    assert_eq!(
        chronology.entries_in_range(10, 61),
        vec![
            Entry::new(10, 1),
            Entry::new(20, 2),
            Entry::new(30, 3),
            Entry::new(40, 4),
            Entry::new(50, 5),
            Entry::new(60, 6),
        ]
    );
    assert_eq!(chronology.entries_in_range(60, 60), Vec::new());

    let mut visited = Vec::new();
    chronology.visit_range_entries(65, 100, |entry| visited.push(entry));
    assert_eq!(visited, vec![Entry::new(70, 7)]);
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

#[test]
fn gorilla_f64_codec_matches_raw_queries() {
    let entries = [
        Entry::new(0, 1.0),
        Entry::new(16, 1.0),
        Entry::new(32, 1.5),
        Entry::new(63, -2.25),
        Entry::new(95, 256.5),
        Entry::new(128, 256.5),
        Entry::new(160, 0.125),
    ];

    let mut raw = Chronology::<f64, SimpleSummary<f64>>::with_chunk_capacity(3);
    let mut gorilla = GorillaF64Chronology::<SimpleSummary<f64>>::with_chunk_capacity(3);

    raw.insert_values(&entries)
        .expect("timestamps are monotonic");
    gorilla
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    assert!(raw.sealed_encoded_size() > 0);
    assert!(gorilla.sealed_encoded_size() > 0);
    assert_eq!(raw.sealed_chunk_count(), gorilla.sealed_chunk_count());

    for (timestamp, direction) in [
        (0, Direction::Forward),
        (1, Direction::Forward),
        (64, Direction::Backward),
        (95, Direction::Backward),
        (161, Direction::Backward),
    ] {
        assert_eq!(
            raw.find_nearest_value(timestamp, direction),
            gorilla.find_nearest_value(timestamp, direction)
        );
    }

    let raw_summary = raw.range_summary(16, 129);
    let gorilla_summary = gorilla.range_summary(16, 129);
    assert_eq!(raw_summary.len, gorilla_summary.len);
    assert_eq!(raw_summary.summary.min, gorilla_summary.summary.min);
    assert_eq!(raw_summary.summary.max, gorilla_summary.summary.max);

    let raw_buckets = raw.summarize_range(0, 161, 5);
    let gorilla_buckets = gorilla.summarize_range(0, 161, 5);
    assert_eq!(raw_buckets.len(), gorilla_buckets.len());
    for (raw_bucket, gorilla_bucket) in raw_buckets.iter().zip(gorilla_buckets.iter()) {
        assert_eq!(raw_bucket.len, gorilla_bucket.len);
        assert_eq!(raw_bucket.summary.min, gorilla_bucket.summary.min);
        assert_eq!(raw_bucket.summary.max, gorilla_bucket.summary.max);
    }
}

#[test]
fn gorilla_f64_codec_matches_raw_queries_after_decoder_anchors() {
    let mut timestamp = 0;
    let entries = (0..512)
        .map(|index| {
            timestamp += 11 + (index % 7) as u64;
            let value = f64::from(((index * 17) % 257) as u32) * 0.125;
            Entry::new(timestamp, value)
        })
        .collect::<Vec<_>>();

    let mut raw = Chronology::<f64, SimpleSummary<f64>>::with_chunk_capacity(512);
    let mut gorilla = GorillaF64Chronology::<SimpleSummary<f64>>::with_chunk_capacity(512);

    raw.insert_values(&entries)
        .expect("timestamps are monotonic");
    gorilla
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    assert_eq!(raw.sealed_chunk_count(), 1);
    assert_eq!(gorilla.sealed_chunk_count(), 1);

    for (timestamp, direction) in [
        (entries[129].timestamp - 1, Direction::Forward),
        (entries[130].timestamp, Direction::Backward),
        (entries[300].timestamp + 3, Direction::Forward),
        (entries[384].timestamp, Direction::Backward),
        (entries[511].timestamp + 100, Direction::Backward),
    ] {
        assert_eq!(
            raw.find_nearest_value(timestamp, direction),
            gorilla.find_nearest_value(timestamp, direction)
        );
    }

    let start = entries[140].timestamp;
    let end = entries[430].timestamp + 1;
    let raw_summary = raw.range_summary(start, end);
    let gorilla_summary = gorilla.range_summary(start, end);
    assert_eq!(raw_summary.len, gorilla_summary.len);
    assert_eq!(raw_summary.summary.min, gorilla_summary.summary.min);
    assert_eq!(raw_summary.summary.max, gorilla_summary.summary.max);

    assert_eq!(
        raw.entries_in_range(start, end),
        gorilla.entries_in_range(start, end)
    );
}
