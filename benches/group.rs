use criterion::{Criterion, black_box, criterion_group, criterion_main};
use t::ast::{SelectItem, Selection};
use t::interpreter::Transform;
use t::operators::GroupBy;
use t::value::{Array, Level, Value};

fn make_rows(count: usize, cardinality: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| {
            let key = format!("key{}", i % cardinality);
            Value::Array(Array::from((
                vec![Value::Text(key), Value::Number(i as f64)],
                Level::Word,
            )))
        })
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_group(c: &mut Criterion) {
    let sel = Selection {
        items: vec![SelectItem::Index(0)],
    };

    let small = make_rows(100, 10);
    let medium = make_rows(10_000, 100);
    let large = make_rows(100_000, 1000);

    c.bench_function("group_100", |b| {
        b.iter(|| {
            let input = small.deep_copy();
            black_box(GroupBy::new(sel.clone()).apply(input).unwrap())
        })
    });

    c.bench_function("group_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(GroupBy::new(sel.clone()).apply(input).unwrap())
        })
    });

    c.bench_function("group_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(GroupBy::new(sel.clone()).apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_group);
criterion_main!(benches);
