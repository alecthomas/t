use criterion::{Criterion, black_box, criterion_group, criterion_main};
use regex::Regex;
use t::interpreter::Transform;
use t::operators::Replace;
use t::value::{Array, Level, Value};

fn make_lines(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("ERROR: something happened on line {}", i)))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_replace(c: &mut Criterion) {
    let small = make_lines(100);
    let medium = make_lines(10_000);
    let large = make_lines(100_000);
    let replacer = Replace::new(Regex::new("ERROR: ").unwrap(), "".to_string(), None);

    c.bench_function("replace_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(replacer.apply(input).unwrap())
        })
    });

    c.bench_function("replace_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(replacer.apply(input).unwrap())
        })
    });

    c.bench_function("replace_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(replacer.apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_replace);
criterion_main!(benches);
