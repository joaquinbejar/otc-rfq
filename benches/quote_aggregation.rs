//! Benchmarks for quote aggregation performance.

#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};

fn quote_aggregation_benchmark(c: &mut Criterion) {
    c.bench_function("placeholder", |b| {
        b.iter(|| {
            // TODO: Implement benchmarks in M5
            std::hint::black_box(1 + 1)
        })
    });
}

criterion_group!(benches, quote_aggregation_benchmark);
criterion_main!(benches);
