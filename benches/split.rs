use criterion::{Criterion, black_box, criterion_group, criterion_main};
use t::interpreter::Transform;
use t::operators::{Split, SplitDelim};
use t::value::{Array, Level, Value};

fn make_lines(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("word1 word2 word3 word4 word5 line{}", i)))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn make_csv_lines(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("field1,field2,field3,field4,field5,{}", i)))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_split(c: &mut Criterion) {
    let small = make_lines(100);
    let medium = make_lines(10_000);
    let large = make_lines(100_000);
    let split: Split = Default::default();

    c.bench_function("split_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(split.apply(input).unwrap())
        })
    });

    c.bench_function("split_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(split.apply(input).unwrap())
        })
    });

    c.bench_function("split_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(split.apply(input).unwrap())
        })
    });
}

fn bench_split_delim(c: &mut Criterion) {
    let small = make_csv_lines(100);
    let medium = make_csv_lines(10_000);
    let large = make_csv_lines(100_000);
    let splitter = SplitDelim::new(",".to_string());

    c.bench_function("split_delim_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(splitter.apply(input).unwrap())
        })
    });

    c.bench_function("split_delim_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(splitter.apply(input).unwrap())
        })
    });

    c.bench_function("split_delim_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(splitter.apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_split, bench_split_delim);
criterion_main!(benches);
