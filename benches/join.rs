use criterion::{Criterion, black_box, criterion_group, criterion_main};
use t::interpreter::Transform;
use t::operators::{Join, JoinDelim};
use t::value::{Array, Level, Value};

fn make_words(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("word{}", i)))
        .collect();
    Value::Array(Array::from((elements, Level::Word)))
}

fn make_nested(rows: usize, cols: usize) -> Value {
    let elements: Vec<Value> = (0..rows)
        .map(|_| {
            let words: Vec<Value> = (0..cols)
                .map(|j| Value::Text(format!("word{}", j)))
                .collect();
            Value::Array(Array::from((words, Level::Word)))
        })
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_join(c: &mut Criterion) {
    let small = make_words(100);
    let medium = make_words(10_000);
    let large = make_words(100_000);
    let join: Join = Default::default();

    c.bench_function("join_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(join.apply(input).unwrap())
        })
    });

    c.bench_function("join_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(join.apply(input).unwrap())
        })
    });

    c.bench_function("join_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(join.apply(input).unwrap())
        })
    });

    let nested_small = make_nested(100, 10);
    let nested_medium = make_nested(10_000, 10);

    c.bench_function("join_flatten_100x10", |b| {
        b.iter(|| {
            let input = nested_small.deep_copy();
            black_box(join.apply(input).unwrap())
        })
    });

    c.bench_function("join_flatten_10kx10", |b| {
        b.iter(|| {
            let input = nested_medium.deep_copy();
            black_box(join.apply(input).unwrap())
        })
    });
}

fn bench_join_delim(c: &mut Criterion) {
    let small = make_words(100);
    let medium = make_words(10_000);
    let large = make_words(100_000);
    let joiner = JoinDelim::new(",".to_string());

    c.bench_function("join_delim_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(joiner.apply(input).unwrap())
        })
    });

    c.bench_function("join_delim_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(joiner.apply(input).unwrap())
        })
    });

    c.bench_function("join_delim_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(joiner.apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_join, bench_join_delim);
criterion_main!(benches);
