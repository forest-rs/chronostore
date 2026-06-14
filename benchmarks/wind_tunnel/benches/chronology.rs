// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Baseline chronology storage benchmarks.

use chronostore::{
    Chronology, Direction, Entry, NullSummary, RetentionPolicy, SimpleSummary, Summary,
};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

const CHUNK_CAPACITY: usize = 4_096;
const BATCH_LEN: usize = 1_000_000;
const QUERY_SERIES_LENS: &[usize] = &[1_000_000, 10_000_000];
const QUERY_LEN: usize = 16_384;
const VIEWPORT_BUCKETS: usize = 1_024;
const SEED_BATCH_LEN: usize = 65_536;
const RETENTION_SEALED_CHUNKS: usize = 256;

fn insert_values(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_values");
    group.throughput(Throughput::Elements(BATCH_LEN as u64));

    group.bench_function("null_summary", |b| {
        let entries = build_entries(BATCH_LEN);
        b.iter_batched(
            || Chronology::<f64, NullSummary<f64>>::with_chunk_capacity(CHUNK_CAPACITY),
            |mut chronology| {
                chronology
                    .insert_values(black_box(&entries))
                    .expect("timestamps are monotonic");
                black_box(chronology.len());
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("simple_summary", |b| {
        let entries = build_entries(BATCH_LEN);
        b.iter_batched(
            || Chronology::<f64, SimpleSummary<f64>>::with_chunk_capacity(CHUNK_CAPACITY),
            |mut chronology| {
                chronology
                    .insert_values(black_box(&entries))
                    .expect("timestamps are monotonic");
                black_box(chronology.summary());
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn find_nearest_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_nearest_value");

    for &series_len in QUERY_SERIES_LENS {
        let chronology = seed_chronology::<NullSummary<f64>>(series_len);
        let queries = build_queries(QUERY_LEN, series_len as u64);
        group.throughput(Throughput::Elements(queries.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("forward_chunked_binary_search", series_len),
            &series_len,
            |b, _| {
                b.iter(|| {
                    for query in &queries {
                        black_box(
                            chronology.find_nearest_value(
                                black_box(*query),
                                black_box(Direction::Forward),
                            ),
                        );
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("backward_chunked_binary_search", series_len),
            &series_len,
            |b, _| {
                b.iter(|| {
                    for query in &queries {
                        black_box(
                            chronology.find_nearest_value(
                                black_box(*query),
                                black_box(Direction::Backward),
                            ),
                        );
                    }
                });
            },
        );
    }

    group.finish();
}

fn range_summaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_summaries");

    for &series_len in QUERY_SERIES_LENS {
        let chronology = seed_chronology::<SimpleSummary<f64>>(series_len);
        let end = (series_len as u64).saturating_mul(16);

        group.throughput(Throughput::Elements(series_len as u64));
        group.bench_with_input(
            BenchmarkId::new("full_series_summary", series_len),
            &series_len,
            |b, _| {
                b.iter(|| {
                    let summary = chronology.range_summary(black_box(0), black_box(end));
                    black_box(summary.len);
                    black_box(summary.summary);
                });
            },
        );

        group.throughput(Throughput::Elements(VIEWPORT_BUCKETS as u64));
        group.bench_with_input(
            BenchmarkId::new("viewport_buckets", series_len),
            &series_len,
            |b, _| {
                b.iter(|| {
                    let summaries = chronology.summarize_range(
                        black_box(0),
                        black_box(end),
                        black_box(VIEWPORT_BUCKETS),
                    );
                    black_box(summaries.len());
                    black_box(summaries);
                });
            },
        );
    }

    group.finish();
}

fn retention(c: &mut Criterion) {
    let mut group = c.benchmark_group("retention");
    group.throughput(Throughput::Elements(CHUNK_CAPACITY as u64));

    group.bench_function("append_chunk_with_eviction_256", |b| {
        let mut chronology =
            Chronology::<f64, SimpleSummary<f64>>::with_chunk_capacity_and_retention(
                CHUNK_CAPACITY,
                RetentionPolicy::max_sealed_chunks(RETENTION_SEALED_CHUNKS),
            );
        let mut next = 0;

        while chronology.sealed_chunk_count() < RETENTION_SEALED_CHUNKS {
            let entries = build_entries_range(next, next + CHUNK_CAPACITY);
            chronology
                .insert_values(&entries)
                .expect("timestamps are monotonic");
            next += CHUNK_CAPACITY;
        }

        b.iter(|| {
            let entries = build_entries_range(next, next + CHUNK_CAPACITY);
            chronology
                .insert_values(black_box(&entries))
                .expect("timestamps are monotonic");
            next += CHUNK_CAPACITY;

            black_box(chronology.sealed_chunk_count());
            black_box(chronology.summary());
        });
    });

    group.finish();
}

fn seed_chronology<S>(len: usize) -> Chronology<f64, S>
where
    S: Summary<f64>,
{
    let mut chronology = Chronology::<f64, S>::with_chunk_capacity(CHUNK_CAPACITY);
    let mut start = 0;

    while start < len {
        let end = start.saturating_add(SEED_BATCH_LEN).min(len);
        let entries = build_entries_range(start, end);
        chronology
            .insert_values(&entries)
            .expect("timestamps are monotonic");
        start = end;
    }

    chronology
}

fn build_entries(len: usize) -> Vec<Entry<f64>> {
    build_entries_range(0, len)
}

fn build_entries_range(start: usize, end: usize) -> Vec<Entry<f64>> {
    (start..end)
        .map(|index| {
            let timestamp = (index as u64).saturating_mul(16);
            let value = f64::from((index % 257) as u32) * 0.25;
            Entry::new(timestamp, value)
        })
        .collect()
}

fn build_queries(len: usize, series_len: u64) -> Vec<u64> {
    let span = series_len.saturating_mul(16);
    (0..len)
        .map(|index| {
            let mixed = (index as u64)
                .saturating_mul(4_099)
                .wrapping_add(17)
                .rotate_left(7);
            mixed % span
        })
        .collect()
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = insert_values, find_nearest_value, range_summaries, retention
}
criterion_main!(benches);
