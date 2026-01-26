use winnow::Result;
use winnow::ascii::digit1;
use winnow::combinator::{alt, opt, repeat, separated};
use winnow::prelude::*;
use winnow::token::{one_of, take_till};

use crate::ast::{Operator, Programme, SelectItem, Selection, Slice};

/// Parse a complete programme (sequence of operators).
pub fn parse_programme(input: &str) -> std::result::Result<Programme, String> {
    programme.parse(input).map_err(|e| {
        let offset = e.offset();
        format!(
            "parse error: unexpected character\n  {}\n  {}^",
            input,
            " ".repeat(offset)
        )
    })
}

/// Parser for the full programme.
fn programme(input: &mut &str) -> Result<Programme> {
    let operators = repeat(0.., operator).parse_next(input)?;
    Ok(Programme { operators })
}

/// Parser for a single operator.
fn operator(input: &mut &str) -> Result<Operator> {
    alt((simple_op, filter_op, group_by_op, selection_op)).parse_next(input)
}

/// Parser for simple single-character operators.
fn simple_op(input: &mut &str) -> Result<Operator> {
    one_of((
        's', 'j', '@', '^', 'u', 'l', 't', 'x', 'd', '+', '#', 'o', 'O',
    ))
    .map(|c| match c {
        's' => Operator::Split,
        'j' => Operator::Join,
        '@' => Operator::Descend,
        '^' => Operator::Ascend,
        'u' => Operator::Uppercase,
        'l' => Operator::Lowercase,
        't' => Operator::Trim,
        'x' => Operator::DeleteEmpty,
        'd' => Operator::DedupeWithCounts,
        '+' => Operator::Sum,
        '#' => Operator::Count,
        'o' => Operator::SortDescending,
        'O' => Operator::SortAscending,
        _ => unreachable!(),
    })
    .parse_next(input)
}

/// Parser for filter operator: `/<regex>/` or `!/<regex>/`
fn filter_op(input: &mut &str) -> Result<Operator> {
    let negate = opt('!').parse_next(input)?.is_some();
    '/'.parse_next(input)?;
    let pattern: &str = take_till(1.., '/').parse_next(input)?;
    '/'.parse_next(input)?;
    Ok(Operator::Filter {
        pattern: pattern.to_string(),
        negate,
    })
}

/// Parser for group by operator: `g<selection>`
fn group_by_op(input: &mut &str) -> Result<Operator> {
    'g'.parse_next(input)?;
    let sel = selection.parse_next(input)?;
    Ok(Operator::GroupBy(sel))
}

/// Parser for selection operator (indices, slices, multi-select).
fn selection_op(input: &mut &str) -> Result<Operator> {
    selection.map(Operator::Selection).parse_next(input)
}

/// Parser for a selection (comma-separated list of select items).
fn selection(input: &mut &str) -> Result<Selection> {
    separated(1.., select_item, ',')
        .map(|items| Selection { items })
        .parse_next(input)
}

/// Parser for a single select item (either a slice or an index).
fn select_item(input: &mut &str) -> Result<SelectItem> {
    alt((slice_item, index_item)).parse_next(input)
}

/// Parser for a slice (must contain at least one ':').
fn slice_item(input: &mut &str) -> Result<SelectItem> {
    let start = opt(index).parse_next(input)?;
    ':'.parse_next(input)?;
    let end = opt(index).parse_next(input)?;
    let step = opt((':'.value(()), index).map(|(_, i)| i)).parse_next(input)?;

    Ok(SelectItem::Slice(Slice { start, end, step }))
}

/// Parser for a single index (returns as SelectItem).
fn index_item(input: &mut &str) -> Result<SelectItem> {
    index.map(SelectItem::Index).parse_next(input)
}

/// Parser for an integer index (possibly negative).
fn index(input: &mut &str) -> Result<i64> {
    (opt('-'), digit1)
        .map(|(neg, digits): (Option<char>, &str)| {
            let value: i64 = digits.parse().expect("digit1 guarantees valid digits");
            if neg.is_some() { -value } else { value }
        })
        .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Operator, SelectItem, Selection, Slice};

    #[test]
    fn empty_programme() {
        let result = parse_programme("").unwrap();
        assert_eq!(result.operators, vec![]);
    }

    #[test]
    fn simple_operators() {
        let result = parse_programme("sjul").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Split,
                Operator::Join,
                Operator::Uppercase,
                Operator::Lowercase,
            ]
        );
    }

    #[test]
    fn descend_ascend() {
        let result = parse_programme("@^").unwrap();
        assert_eq!(result.operators, vec![Operator::Descend, Operator::Ascend,]);
    }

    #[test]
    fn single_index() {
        let result = parse_programme("0").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Index(0)]
            })]
        );
    }

    #[test]
    fn negative_index() {
        let result = parse_programme("-1").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Index(-1)]
            })]
        );
    }

    #[test]
    fn slice_from_start() {
        let result = parse_programme(":3").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: None,
                    end: Some(3),
                    step: None,
                })]
            })]
        );
    }

    #[test]
    fn slice_to_end() {
        let result = parse_programme("0:").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: Some(0),
                    end: None,
                    step: None,
                })]
            })]
        );
    }

    #[test]
    fn full_slice() {
        let result = parse_programme("1:5").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: Some(1),
                    end: Some(5),
                    step: None,
                })]
            })]
        );
    }

    #[test]
    fn slice_with_stride() {
        let result = parse_programme("1::3").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: Some(1),
                    end: None,
                    step: Some(3),
                })]
            })]
        );
    }

    #[test]
    fn full_slice_with_stride() {
        let result = parse_programme("1:5:2").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: Some(1),
                    end: Some(5),
                    step: Some(2),
                })]
            })]
        );
    }

    #[test]
    fn reverse_slice() {
        let result = parse_programme("::-1").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: None,
                    end: None,
                    step: Some(-1),
                })]
            })]
        );
    }

    #[test]
    fn multi_select_indices() {
        let result = parse_programme("0,2,3").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![
                    SelectItem::Index(0),
                    SelectItem::Index(2),
                    SelectItem::Index(3),
                ]
            })]
        );
    }

    #[test]
    fn mixed_selection() {
        let result = parse_programme("-1,0:-1").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Selection(Selection {
                items: vec![
                    SelectItem::Index(-1),
                    SelectItem::Slice(Slice {
                        start: Some(0),
                        end: Some(-1),
                        step: None,
                    }),
                ]
            })]
        );
    }

    #[test]
    fn combined_programme() {
        let result = parse_programme("s@0j").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Split,
                Operator::Descend,
                Operator::Selection(Selection {
                    items: vec![SelectItem::Index(0)]
                }),
                Operator::Join,
            ]
        );
    }

    #[test]
    fn complex_programme() {
        let result = parse_programme("s@0,2,3^j").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Split,
                Operator::Descend,
                Operator::Selection(Selection {
                    items: vec![
                        SelectItem::Index(0),
                        SelectItem::Index(2),
                        SelectItem::Index(3),
                    ]
                }),
                Operator::Ascend,
                Operator::Join,
            ]
        );
    }

    #[test]
    fn slice_then_simple_op() {
        let result = parse_programme(":3l").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Selection(Selection {
                    items: vec![SelectItem::Slice(Slice {
                        start: None,
                        end: Some(3),
                        step: None,
                    })]
                }),
                Operator::Lowercase,
            ]
        );
    }

    #[test]
    fn filter_keep() {
        let result = parse_programme("/^a/").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Filter {
                pattern: "^a".to_string(),
                negate: false,
            }]
        );
    }

    #[test]
    fn filter_remove() {
        let result = parse_programme("!/^a/").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Filter {
                pattern: "^a".to_string(),
                negate: true,
            }]
        );
    }

    #[test]
    fn filter_complex_pattern() {
        let result = parse_programme("/foo.*bar/").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Filter {
                pattern: "foo.*bar".to_string(),
                negate: false,
            }]
        );
    }

    #[test]
    fn filter_combined_with_other_ops() {
        let result = parse_programme("s/^a/l").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Split,
                Operator::Filter {
                    pattern: "^a".to_string(),
                    negate: false,
                },
                Operator::Lowercase,
            ]
        );
    }

    #[test]
    fn filter_chained() {
        let result = parse_programme("/foo/!/bar/").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Filter {
                    pattern: "foo".to_string(),
                    negate: false,
                },
                Operator::Filter {
                    pattern: "bar".to_string(),
                    negate: true,
                },
            ]
        );
    }

    #[test]
    fn group_by_single_index() {
        let result = parse_programme("g0").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::GroupBy(Selection {
                items: vec![SelectItem::Index(0)]
            })]
        );
    }

    #[test]
    fn group_by_negative_index() {
        let result = parse_programme("g-1").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::GroupBy(Selection {
                items: vec![SelectItem::Index(-1)]
            })]
        );
    }

    #[test]
    fn group_by_composite_key() {
        let result = parse_programme("g0,2").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::GroupBy(Selection {
                items: vec![SelectItem::Index(0), SelectItem::Index(2)]
            })]
        );
    }

    #[test]
    fn group_by_slice() {
        let result = parse_programme("g0:3").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::GroupBy(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: Some(0),
                    end: Some(3),
                    step: None,
                })]
            })]
        );
    }

    #[test]
    fn group_by_followed_by_ops() {
        let result = parse_programme("sg0o").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Split,
                Operator::GroupBy(Selection {
                    items: vec![SelectItem::Index(0)]
                }),
                Operator::SortDescending,
            ]
        );
    }
}
