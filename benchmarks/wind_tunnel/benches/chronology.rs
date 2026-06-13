// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Baseline chronology storage benchmarks.

use chronostore::{Chronology, Direction, Entry, NullSummary, SimpleSummary};
use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::hint::black_box;

const CHUNK_CAPACITY: usize = 4_096;
const BATCH_LEN: usize = 1_000_000;
const SERIES_LEN: usize = 1_000_000;
const QUERY_LEN: usize = 16_384;

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
    let mut chronology = Chronology::<f64, NullSummary<f64>>::new();
    let entries = build_entries(SERIES_LEN);
    let queries = build_queries(QUERY_LEN, SERIES_LEN as u64);
    chronology
        .insert_values(&entries)
        .expect("timestamps are monotonic");

    let mut group = c.benchmark_group("find_nearest_value");
    group.throughput(Throughput::Elements(queries.len() as u64));

    group.bench_function("forward_chunked_binary_search", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(
                    chronology.find_nearest_value(black_box(*query), black_box(Direction::Forward)),
                );
            }
        });
    });

    group.bench_function("backward_chunked_binary_search", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(
                    chronology
                        .find_nearest_value(black_box(*query), black_box(Direction::Backward)),
                );
            }
        });
    });

    group.finish();
}

fn build_entries(len: usize) -> Vec<Entry<f64>> {
    (0..len)
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
    targets = insert_values, find_nearest_value
}
criterion_main!(benches);
