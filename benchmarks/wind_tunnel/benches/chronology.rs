// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Baseline chronology storage benchmarks.

use chronostore::{
    Chronology, ChunkCodec, Direction, Entry, GorillaF64Codec, NullSummary, RawCodec,
    RetentionPolicy, SimpleSummary, Summary, lttb,
};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

const CHUNK_CAPACITY: usize = 4_096;
const BATCH_LEN: usize = 1_000_000;
const QUERY_SERIES_LENS: &[usize] = &[1_000_000, 10_000_000];
const QUERY_LEN: usize = 16_384;
const VIEWPORT_BUCKETS: usize = 1_024;
const EXACT_RANGE_LEN: usize = 65_536;
const LTTB_TARGET_LEN: usize = 1_024;
const SEED_BATCH_LEN: usize = 65_536;
const RETENTION_SEALED_CHUNKS: usize = 256;
const RETENTION_MAX_AGE: u64 = (RETENTION_SEALED_CHUNKS as u64) * (CHUNK_CAPACITY as u64) * 16;

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
            BenchmarkId::new("bucketed_summaries_reuse", series_len),
            &series_len,
            |b, _| {
                let mut summaries = Vec::with_capacity(VIEWPORT_BUCKETS);
                b.iter(|| {
                    summaries.clear();
                    summaries.extend(chronology.bucketed_summaries(
                        black_box(0),
                        black_box(end),
                        black_box(VIEWPORT_BUCKETS),
                    ));
                    black_box(summaries.len());
                    black_box(&summaries);
                });
            },
        );
    }

    group.finish();
}

fn codec_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec_storage");
    let raw = seed_chronology_with_codec::<NullSummary<f64>, RawCodec>(BATCH_LEN);
    let gorilla = seed_chronology_with_codec::<NullSummary<f64>, GorillaF64Codec>(BATCH_LEN);
    let raw_size = raw.sealed_encoded_size();
    let gorilla_size = gorilla.sealed_encoded_size();

    group.throughput(Throughput::Elements(BATCH_LEN as u64));
    group.bench_with_input(
        BenchmarkId::new("raw_sealed_encoded_size_bytes", raw_size),
        &raw,
        |b, chronology| {
            b.iter(|| black_box(chronology.sealed_encoded_size()));
        },
    );
    group.bench_with_input(
        BenchmarkId::new("gorilla_f64_sealed_encoded_size_bytes", gorilla_size),
        &gorilla,
        |b, chronology| {
            b.iter(|| black_box(chronology.sealed_encoded_size()));
        },
    );

    group.finish();
}

fn codec_find_nearest_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec_find_nearest_value");
    let raw = seed_chronology_with_codec::<NullSummary<f64>, RawCodec>(BATCH_LEN);
    let gorilla = seed_chronology_with_codec::<NullSummary<f64>, GorillaF64Codec>(BATCH_LEN);
    let queries = build_queries(QUERY_LEN, BATCH_LEN as u64);

    group.throughput(Throughput::Elements(queries.len() as u64));
    group.bench_function("raw_forward_1m", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(raw.find_nearest_value(black_box(*query), black_box(Direction::Forward)));
            }
        });
    });
    group.bench_function("gorilla_f64_forward_1m", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(
                    gorilla.find_nearest_value(black_box(*query), black_box(Direction::Forward)),
                );
            }
        });
    });

    group.finish();
}

fn codec_range_summaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec_range_summaries");
    let raw = seed_chronology_with_codec::<SimpleSummary<f64>, RawCodec>(BATCH_LEN);
    let gorilla = seed_chronology_with_codec::<SimpleSummary<f64>, GorillaF64Codec>(BATCH_LEN);
    let end = (BATCH_LEN as u64).saturating_mul(16);

    group.throughput(Throughput::Elements(VIEWPORT_BUCKETS as u64));
    group.bench_function("raw_bucketed_summaries_reuse_1m", |b| {
        let mut summaries = Vec::with_capacity(VIEWPORT_BUCKETS);
        b.iter(|| {
            summaries.clear();
            summaries.extend(raw.bucketed_summaries(
                black_box(0),
                black_box(end),
                black_box(VIEWPORT_BUCKETS),
            ));
            black_box(&summaries);
        });
    });
    group.bench_function("gorilla_f64_bucketed_summaries_reuse_1m", |b| {
        let mut summaries = Vec::with_capacity(VIEWPORT_BUCKETS);
        b.iter(|| {
            summaries.clear();
            summaries.extend(gorilla.bucketed_summaries(
                black_box(0),
                black_box(end),
                black_box(VIEWPORT_BUCKETS),
            ));
            black_box(&summaries);
        });
    });
    group.bench_function("raw_range_envelope_reuse_1m", |b| {
        let mut envelope = Vec::with_capacity(VIEWPORT_BUCKETS);
        b.iter(|| {
            envelope.clear();
            envelope.extend(raw.range_envelope(
                black_box(0),
                black_box(end),
                black_box(VIEWPORT_BUCKETS),
            ));
            black_box(&envelope);
        });
    });
    group.bench_function("gorilla_f64_range_envelope_reuse_1m", |b| {
        let mut envelope = Vec::with_capacity(VIEWPORT_BUCKETS);
        b.iter(|| {
            envelope.clear();
            envelope.extend(gorilla.range_envelope(
                black_box(0),
                black_box(end),
                black_box(VIEWPORT_BUCKETS),
            ));
            black_box(&envelope);
        });
    });

    group.finish();
}

fn codec_exact_range_entries(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec_exact_range_entries");
    let raw = seed_chronology_with_codec::<NullSummary<f64>, RawCodec>(BATCH_LEN);
    let gorilla = seed_chronology_with_codec::<NullSummary<f64>, GorillaF64Codec>(BATCH_LEN);
    let start = ((BATCH_LEN - EXACT_RANGE_LEN) as u64 / 2).saturating_mul(16);
    let end = start + (EXACT_RANGE_LEN as u64).saturating_mul(16);

    group.throughput(Throughput::Elements(EXACT_RANGE_LEN as u64));
    group.bench_function("raw_iter_65536", |b| {
        b.iter(|| {
            let mut count = 0;
            let mut sum = 0.0;
            for entry in raw.entries_in_range(black_box(start), black_box(end)) {
                count += 1;
                sum += entry.value;
            }
            black_box((count, sum));
        });
    });
    group.bench_function("gorilla_f64_iter_65536", |b| {
        b.iter(|| {
            let mut count = 0;
            let mut sum = 0.0;
            for entry in gorilla.entries_in_range(black_box(start), black_box(end)) {
                count += 1;
                sum += entry.value;
            }
            black_box((count, sum));
        });
    });

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

    group.bench_function("append_chunk_with_time_window_256", |b| {
        let mut chronology =
            Chronology::<f64, SimpleSummary<f64>>::with_chunk_capacity_and_retention(
                CHUNK_CAPACITY,
                RetentionPolicy::max_age(RETENTION_MAX_AGE),
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

fn lttb_downsampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("lttb_downsampling");
    let entries = build_entries(BATCH_LEN);

    group.throughput(Throughput::Elements(BATCH_LEN as u64));
    group.bench_function("entries_1m_to_1024", |b| {
        let mut sampled = Vec::with_capacity(LTTB_TARGET_LEN);
        b.iter(|| {
            sampled.clear();
            for entry in lttb(black_box(&entries), black_box(LTTB_TARGET_LEN), |value| {
                value
            }) {
                sampled.push(entry);
            }
            black_box(&sampled);
        });
    });
    group.bench_function("entries_1m_to_1024_count", |b| {
        b.iter(|| {
            let mut count = 0;
            for entry in lttb(black_box(&entries), black_box(LTTB_TARGET_LEN), |value| {
                value
            }) {
                count += 1;
                black_box(entry);
            }
            black_box(count);
        });
    });

    group.finish();
}

fn seed_chronology<S>(len: usize) -> Chronology<f64, S>
where
    S: Summary<f64>,
{
    seed_chronology_with_codec::<S, RawCodec>(len)
}

fn seed_chronology_with_codec<S, C>(len: usize) -> Chronology<f64, S, C>
where
    S: Summary<f64>,
    C: ChunkCodec<f64>,
{
    let mut chronology = Chronology::<f64, S, C>::with_chunk_capacity(CHUNK_CAPACITY);
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
            let value =
                f64::from(u32::try_from(index % 257).expect("modulo 257 fits in u32")) * 0.25;
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
    targets = insert_values, find_nearest_value, range_summaries, codec_storage,
        codec_find_nearest_value, codec_range_summaries, codec_exact_range_entries,
        retention, lttb_downsampling
}
criterion_main!(benches);
