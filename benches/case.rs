use criterion::{Criterion, black_box, criterion_group, criterion_main};
use t::interpreter::Transform;
use t::operators::{Lowercase, Uppercase};
use t::value::{Array, Level, Value};

fn make_lines(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("HELLO World Test String Line{}", i)))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_case(c: &mut Criterion) {
    let small = make_lines(100);
    let medium = make_lines(10_000);
    let large = make_lines(100_000);

    c.bench_function("lowercase_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(Lowercase.apply(input).unwrap())
        })
    });

    c.bench_function("lowercase_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(Lowercase.apply(input).unwrap())
        })
    });

    c.bench_function("lowercase_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(Lowercase.apply(input).unwrap())
        })
    });

    c.bench_function("uppercase_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(Uppercase.apply(input).unwrap())
        })
    });

    c.bench_function("uppercase_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(Uppercase.apply(input).unwrap())
        })
    });

    c.bench_function("uppercase_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(Uppercase.apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_case);
criterion_main!(benches);
