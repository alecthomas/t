use criterion::{Criterion, black_box, criterion_group, criterion_main};
use t::ast::{SelectItem, Selection, Slice};
use t::interpreter::Transform;
use t::operators::Select;
use t::value::{Array, Level, Value};

fn make_lines(count: usize) -> Value {
    let elements: Vec<Value> = (0..count)
        .map(|i| Value::Text(format!("line{}", i)))
        .collect();
    Value::Array(Array::from((elements, Level::Line)))
}

fn bench_select(c: &mut Criterion) {
    let medium = make_lines(10_000);
    let large = make_lines(100_000);

    // Single index
    let sel_single = Selection {
        items: vec![SelectItem::Index(0)],
    };
    c.bench_function("select_single_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(Select::new(sel_single.clone()).apply(input).unwrap())
        })
    });

    // First 100 elements
    let sel_slice = Selection {
        items: vec![SelectItem::Slice(Slice {
            start: None,
            end: Some(100),
            step: None,
        })],
    };
    c.bench_function("select_slice_100_from_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(Select::new(sel_slice.clone()).apply(input).unwrap())
        })
    });

    c.bench_function("select_slice_100_from_100k", |b| {
        b.iter(|| {
            let input = large.deep_copy();
            black_box(Select::new(sel_slice.clone()).apply(input).unwrap())
        })
    });

    // Every other element
    let sel_stride = Selection {
        items: vec![SelectItem::Slice(Slice {
            start: None,
            end: None,
            step: Some(2),
        })],
    };
    c.bench_function("select_stride_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(Select::new(sel_stride.clone()).apply(input).unwrap())
        })
    });

    // Reverse
    let sel_rev = Selection {
        items: vec![SelectItem::Slice(Slice {
            start: None,
            end: None,
            step: Some(-1),
        })],
    };
    c.bench_function("select_reverse_10k", |b| {
        b.iter(|| {
            let input = medium.deep_copy();
            black_box(Select::new(sel_rev.clone()).apply(input).unwrap())
        })
    });
}

criterion_group!(benches, bench_select);
criterion_main!(benches);
