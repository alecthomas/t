use criterion::{Criterion, black_box, criterion_group, criterion_main};
use t::interpreter::Transform;
use t::operators::DedupeWithCounts;
use t::value::{Array, Level, Value};

fn make_lines_high_cardinality(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("word{}", i % (count / 10))))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn make_lines_low_cardinality(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("word{}", i % 10)))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_dedupe(c: &mut Criterion) {
    let small = make_lines_low_cardinality(100);
    let medium = make_lines_low_cardinality(10_000);
    let large = make_lines_low_cardinality(100_000);

    c.bench_function("dedupe_100_low_card", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(DedupeWithCounts.apply(input).unwrap())
        })
    });

    c.bench_function("dedupe_10k_low_card", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(DedupeWithCounts.apply(input).unwrap())
        })
    });

    c.bench_function("dedupe_100k_low_card", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(DedupeWithCounts.apply(input).unwrap())
        })
    });

    let small_hi = make_lines_high_cardinality(100);
    let medium_hi = make_lines_high_cardinality(10_000);
    let large_hi = make_lines_high_cardinality(100_000);

    c.bench_function("dedupe_100_high_card", |b| {
        b.iter(|| {
            let input = small_hi.deep_copy();
            black_box(DedupeWithCounts.apply(input).unwrap())
        })
    });

    c.bench_function("dedupe_10k_high_card", |b| {
        b.iter(|| {
            let input = medium_hi.deep_copy();
            black_box(DedupeWithCounts.apply(input).unwrap())
        })
    });

    c.bench_function("dedupe_100k_high_card", |b| {
        b.iter(|| {
            let input = large_hi.deep_copy();
            black_box(DedupeWithCounts.apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_dedupe);
criterion_main!(benches);
