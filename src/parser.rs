use winnow::ModalResult;
use winnow::ascii::digit1;
use winnow::combinator::{alt, cut_err, opt, repeat, separated};
use winnow::error::{StrContext, StrContextValue};
use winnow::prelude::*;
use winnow::token::{one_of, take_till};

use crate::ast::{Operator, Programme, SelectItem, Selection, Slice};

/// Parse a complete programme (sequence of operators).
pub fn parse_programme(input: &str) -> std::result::Result<Programme, String> {
    programme.parse(input).map_err(|e| {
        let offset = e.offset();
        let message = if let Some(ctx) = e.inner().context().next() {
            match ctx {
                StrContext::Label(label) => format!("parse error: expected {}", label),
                StrContext::Expected(StrContextValue::Description(desc)) => {
                    format!("parse error: expected {}", desc)
                }
                _ => "parse error: unexpected character".to_string(),
            }
        } else {
            "parse error: unexpected character".to_string()
        };
        format!("{}\n  {}\n  {}^", message, input, " ".repeat(offset))
    })
}

/// Parser for the full programme.
fn programme(input: &mut &str) -> ModalResult<Programme> {
    let operators = repeat(0.., operator).parse_next(input)?;
    Ok(Programme { operators })
}

/// Parser for a single operator.
fn operator(input: &mut &str) -> ModalResult<Operator> {
    alt((
        simple_op,
        split_delim_op,
        join_delim_op,
        lowercase_selected_op,
        uppercase_selected_op,
        replace_op,
        filter_op,
        group_by_op,
        selection_op,
    ))
    .parse_next(input)
}

/// Parser for simple single-character operators.
fn simple_op(input: &mut &str) -> ModalResult<Operator> {
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

/// Parser for split delimiter operator: `S<char>` or `S"<delim>"`
fn split_delim_op(input: &mut &str) -> ModalResult<Operator> {
    'S'.parse_next(input)?;
    let delim = cut_err(alt((non_empty_quoted_string, single_char_delim)))
        .context(StrContext::Expected(StrContextValue::Description(
            "<delimiter>",
        )))
        .parse_next(input)?;
    Ok(Operator::SplitDelim(delim))
}

/// Parser for join delimiter operator: `J<char>` or `J"<delim>"`
fn join_delim_op(input: &mut &str) -> ModalResult<Operator> {
    'J'.parse_next(input)?;
    let delim = cut_err(alt((quoted_string, single_char_delim)))
        .context(StrContext::Expected(StrContextValue::Description(
            "<delimiter>",
        )))
        .parse_next(input)?;
    Ok(Operator::JoinDelim(delim))
}

/// Parser for lowercase selected operator: `L<selection>`
fn lowercase_selected_op(input: &mut &str) -> ModalResult<Operator> {
    'L'.parse_next(input)?;
    let sel = cut_err(selection)
        .context(StrContext::Expected(StrContextValue::Description(
            "<selection>",
        )))
        .parse_next(input)?;
    Ok(Operator::LowercaseSelected(sel))
}

/// Parser for uppercase selected operator: `U<selection>`
fn uppercase_selected_op(input: &mut &str) -> ModalResult<Operator> {
    'U'.parse_next(input)?;
    let sel = cut_err(selection)
        .context(StrContext::Expected(StrContextValue::Description(
            "<selection>",
        )))
        .parse_next(input)?;
    Ok(Operator::UppercaseSelected(sel))
}

/// Parser for replace operator: `r[<selection>]/<old>/<new>/`
fn replace_op(input: &mut &str) -> ModalResult<Operator> {
    'r'.parse_next(input)?;
    // Optional selection before the first /
    let sel = opt(selection).parse_next(input)?;
    // Now expect /<pattern>/<replacement>/
    cut_err('/')
        .context(StrContext::Expected(StrContextValue::Description("'/'")))
        .parse_next(input)?;
    let pattern: &str = cut_err(take_till(1.., '/'))
        .context(StrContext::Expected(StrContextValue::Description(
            "<pattern>",
        )))
        .parse_next(input)?;
    cut_err('/')
        .context(StrContext::Expected(StrContextValue::Description("'/'")))
        .parse_next(input)?;
    let replacement: &str = take_till(0.., '/').parse_next(input)?;
    cut_err('/')
        .context(StrContext::Expected(StrContextValue::Description(
            "closing '/'",
        )))
        .parse_next(input)?;
    Ok(Operator::Replace {
        selection: sel,
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
    })
}

/// Parse a non-empty quoted string (for delimiters that can't be empty).
fn non_empty_quoted_string(input: &mut &str) -> ModalResult<String> {
    // Check for empty string "" before consuming
    if input.starts_with("\"\"") {
        // Consume just the first quote so error points at the right place
        '"'.parse_next(input)?;
        return cut_err(winnow::combinator::fail)
            .context(StrContext::Expected(StrContextValue::Description(
                "non-empty delimiter",
            )))
            .parse_next(input);
    }
    quoted_string(input)
}

/// Parse a quoted string with escape sequences.
/// Supports: \\ \n \r \t \" \' \0 \xNN \uNNNN
fn quoted_string(input: &mut &str) -> ModalResult<String> {
    '"'.parse_next(input)?;
    let mut result = String::new();
    loop {
        // Take characters until we hit a backslash or closing quote
        let chunk: &str = take_till(0.., ('\\', '"')).parse_next(input)?;
        result.push_str(chunk);

        // Check what we hit
        if input.starts_with('"') {
            // End of string
            '"'.parse_next(input)?;
            return Ok(result);
        } else if input.starts_with('\\') {
            // Escape sequence
            '\\'.parse_next(input)?;
            let escaped = cut_err(parse_escape_char)
                .context(StrContext::Expected(StrContextValue::Description(
                    "escape sequence",
                )))
                .parse_next(input)?;
            result.push(escaped);
        } else {
            // End of input without closing quote
            return cut_err('"')
                .context(StrContext::Expected(StrContextValue::Description(
                    "closing '\"'",
                )))
                .parse_next(input)
                .map(|_| result);
        }
    }
}

/// Parse the character after a backslash in an escape sequence.
fn parse_escape_char(input: &mut &str) -> ModalResult<char> {
    let c = winnow::token::any.parse_next(input)?;
    match c {
        '\\' => Ok('\\'),
        '"' => Ok('"'),
        '\'' => Ok('\''),
        'n' => Ok('\n'),
        'r' => Ok('\r'),
        't' => Ok('\t'),
        '0' => Ok('\0'),
        'x' => parse_hex_escape(input),
        'u' => parse_unicode_escape(input),
        _ => cut_err(winnow::combinator::fail)
            .context(StrContext::Expected(StrContextValue::Description(
                "valid escape sequence (\\n, \\r, \\t, \\0, \\\\, \\\", \\', \\xNN, \\uNNNN)",
            )))
            .parse_next(input),
    }
}

/// Parse a hex escape: \xNN (exactly 2 hex digits).
fn parse_hex_escape(input: &mut &str) -> ModalResult<char> {
    let digits: &str = cut_err(winnow::token::take(2usize))
        .context(StrContext::Expected(StrContextValue::Description(
            "2 hex digits",
        )))
        .parse_next(input)?;
    let code = u32::from_str_radix(digits, 16).ok();
    match code.and_then(char::from_u32) {
        Some(c) => Ok(c),
        None => cut_err(winnow::combinator::fail)
            .context(StrContext::Expected(StrContextValue::Description(
                "valid hex digits",
            )))
            .parse_next(input),
    }
}

/// Parse a unicode escape: \uNNNN (exactly 4 hex digits).
fn parse_unicode_escape(input: &mut &str) -> ModalResult<char> {
    let digits: &str = cut_err(winnow::token::take(4usize))
        .context(StrContext::Expected(StrContextValue::Description(
            "4 hex digits",
        )))
        .parse_next(input)?;
    let code = u32::from_str_radix(digits, 16).ok();
    match code.and_then(char::from_u32) {
        Some(c) => Ok(c),
        None => cut_err(winnow::combinator::fail)
            .context(StrContext::Expected(StrContextValue::Description(
                "valid unicode code point",
            )))
            .parse_next(input),
    }
}

/// Parse a single character as a delimiter.
fn single_char_delim(input: &mut &str) -> ModalResult<String> {
    winnow::token::any
        .map(|c: char| c.to_string())
        .parse_next(input)
}

/// Parser for filter operator: `/<regex>/` or `!/<regex>/`
fn filter_op(input: &mut &str) -> ModalResult<Operator> {
    let negate = opt('!').parse_next(input)?.is_some();
    '/'.parse_next(input)?;
    let pattern: &str = cut_err(take_till(1.., '/'))
        .context(StrContext::Expected(StrContextValue::Description(
            "<pattern>",
        )))
        .parse_next(input)?;
    cut_err('/')
        .context(StrContext::Expected(StrContextValue::Description(
            "closing '/'",
        )))
        .parse_next(input)?;
    Ok(Operator::Filter {
        pattern: pattern.to_string(),
        negate,
    })
}

/// Parser for group by operator: `g<selection>`
fn group_by_op(input: &mut &str) -> ModalResult<Operator> {
    'g'.parse_next(input)?;
    let sel = cut_err(selection)
        .context(StrContext::Expected(StrContextValue::Description(
            "<selection>",
        )))
        .parse_next(input)?;
    Ok(Operator::GroupBy(sel))
}

/// Parser for selection operator (indices, slices, multi-select).
fn selection_op(input: &mut &str) -> ModalResult<Operator> {
    selection.map(Operator::Selection).parse_next(input)
}

/// Parser for a selection (comma-separated list of select items).
fn selection(input: &mut &str) -> ModalResult<Selection> {
    separated(1.., select_item, ',')
        .map(|items| Selection { items })
        .parse_next(input)
}

/// Parser for a single select item (either a slice or an index).
fn select_item(input: &mut &str) -> ModalResult<SelectItem> {
    alt((slice_item, index_item)).parse_next(input)
}

/// Parser for a slice (must contain at least one ':').
fn slice_item(input: &mut &str) -> ModalResult<SelectItem> {
    let start = opt(index).parse_next(input)?;
    ':'.parse_next(input)?;
    let end = opt(index).parse_next(input)?;
    let step = opt((':'.value(()), index).map(|(_, i)| i)).parse_next(input)?;

    Ok(SelectItem::Slice(Slice { start, end, step }))
}

/// Parser for a single index (returns as SelectItem).
fn index_item(input: &mut &str) -> ModalResult<SelectItem> {
    index.map(SelectItem::Index).parse_next(input)
}

/// Parser for an integer index (possibly negative).
fn index(input: &mut &str) -> ModalResult<i64> {
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

    #[test]
    fn group_by_missing_selection_error() {
        let result = parse_programme("sg");
        assert_eq!(
            result,
            Err("parse error: expected <selection>\n  sg\n    ^".to_string())
        );
    }

    #[test]
    fn filter_empty_pattern_error() {
        let result = parse_programme("//");
        assert_eq!(
            result,
            Err("parse error: expected <pattern>\n  //\n   ^".to_string())
        );
    }

    #[test]
    fn filter_missing_closing_slash_error() {
        let result = parse_programme("/foo");
        assert_eq!(
            result,
            Err("parse error: expected closing '/'\n  /foo\n      ^".to_string())
        );
    }

    #[test]
    fn split_delim_single_char() {
        let result = parse_programme("S,").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim(",".to_string())]
        );
    }

    #[test]
    fn split_delim_colon() {
        let result = parse_programme("S:").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim(":".to_string())]
        );
    }

    #[test]
    fn split_delim_quoted_multi_char() {
        let result = parse_programme(r#"S"::""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("::".to_string())]
        );
    }

    #[test]
    fn split_delim_quoted_single_char() {
        let result = parse_programme(r#"S",""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim(",".to_string())]
        );
    }

    #[test]
    fn split_delim_empty_string_error() {
        let result = parse_programme(r#"S"""#);
        assert!(result.is_err());
    }

    #[test]
    fn split_delim_escape_newline() {
        let result = parse_programme(r#"S"\n""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("\n".to_string())]
        );
    }

    #[test]
    fn split_delim_escape_tab() {
        let result = parse_programme(r#"S"\t""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("\t".to_string())]
        );
    }

    #[test]
    fn split_delim_escape_backslash() {
        let result = parse_programme(r#"S"\\""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("\\".to_string())]
        );
    }

    #[test]
    fn split_delim_escape_quote() {
        let result = parse_programme(r#"S"\"""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("\"".to_string())]
        );
    }

    #[test]
    fn split_delim_escape_hex() {
        let result = parse_programme(r#"S"\x41""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("A".to_string())]
        );
    }

    #[test]
    fn split_delim_escape_unicode() {
        let result = parse_programme(r#"S"\u0041""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("A".to_string())]
        );
    }

    #[test]
    fn split_delim_combined_escapes() {
        let result = parse_programme(r#"S"\t\n\r""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim("\t\n\r".to_string())]
        );
    }

    #[test]
    fn split_delim_followed_by_ops() {
        let result = parse_programme("S,l").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::SplitDelim(",".to_string()), Operator::Lowercase,]
        );
    }

    #[test]
    fn split_delim_missing_delimiter_error() {
        let result = parse_programme("S");
        assert!(result.is_err());
    }

    #[test]
    fn split_delim_unclosed_quote_error() {
        let result = parse_programme(r#"S"foo"#);
        assert!(result.is_err());
    }

    #[test]
    fn split_delim_invalid_escape_error() {
        let result = parse_programme(r#"S"\q""#);
        assert!(result.is_err());
    }

    #[test]
    fn split_delim_invalid_hex_error() {
        let result = parse_programme(r#"S"\xGG""#);
        assert!(result.is_err());
    }

    #[test]
    fn split_delim_short_unicode_error() {
        let result = parse_programme(r#"S"\u41""#);
        assert!(result.is_err());
    }

    #[test]
    fn join_delim_single_char() {
        let result = parse_programme("J,").unwrap();
        assert_eq!(result.operators, vec![Operator::JoinDelim(",".to_string())]);
    }

    #[test]
    fn join_delim_quoted_multi_char() {
        let result = parse_programme(r#"J", ""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::JoinDelim(", ".to_string())]
        );
    }

    #[test]
    fn join_delim_empty_string() {
        let result = parse_programme(r#"J"""#).unwrap();
        assert_eq!(result.operators, vec![Operator::JoinDelim("".to_string())]);
    }

    #[test]
    fn join_delim_escape_newline() {
        let result = parse_programme(r#"J"\n""#).unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::JoinDelim("\n".to_string())]
        );
    }

    #[test]
    fn join_delim_followed_by_ops() {
        let result = parse_programme("sJ,").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Split, Operator::JoinDelim(",".to_string()),]
        );
    }

    #[test]
    fn join_delim_missing_delimiter_error() {
        let result = parse_programme("J");
        assert!(result.is_err());
    }

    #[test]
    fn lowercase_selected_single_index() {
        let result = parse_programme("L0").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::LowercaseSelected(Selection {
                items: vec![SelectItem::Index(0)]
            })]
        );
    }

    #[test]
    fn lowercase_selected_slice() {
        let result = parse_programme("L:2").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::LowercaseSelected(Selection {
                items: vec![SelectItem::Slice(Slice {
                    start: None,
                    end: Some(2),
                    step: None,
                })]
            })]
        );
    }

    #[test]
    fn lowercase_selected_multi() {
        let result = parse_programme("L0,2").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::LowercaseSelected(Selection {
                items: vec![SelectItem::Index(0), SelectItem::Index(2)]
            })]
        );
    }

    #[test]
    fn lowercase_selected_missing_selection_error() {
        let result = parse_programme("L");
        assert!(result.is_err());
    }

    #[test]
    fn uppercase_selected_single_index() {
        let result = parse_programme("U0").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::UppercaseSelected(Selection {
                items: vec![SelectItem::Index(0)]
            })]
        );
    }

    #[test]
    fn uppercase_selected_missing_selection_error() {
        let result = parse_programme("U");
        assert!(result.is_err());
    }

    #[test]
    fn replace_basic() {
        let result = parse_programme("r/foo/bar/").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Replace {
                selection: None,
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
            }]
        );
    }

    #[test]
    fn replace_empty_replacement() {
        let result = parse_programme("r/foo//").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Replace {
                selection: None,
                pattern: "foo".to_string(),
                replacement: "".to_string(),
            }]
        );
    }

    #[test]
    fn replace_with_selection() {
        let result = parse_programme("r0/foo/bar/").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Replace {
                selection: Some(Selection {
                    items: vec![SelectItem::Index(0)]
                }),
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
            }]
        );
    }

    #[test]
    fn replace_with_slice_selection() {
        let result = parse_programme("r:2/foo/bar/").unwrap();
        assert_eq!(
            result.operators,
            vec![Operator::Replace {
                selection: Some(Selection {
                    items: vec![SelectItem::Slice(Slice {
                        start: None,
                        end: Some(2),
                        step: None,
                    })]
                }),
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
            }]
        );
    }

    #[test]
    fn replace_followed_by_ops() {
        let result = parse_programme("r/a/b/l").unwrap();
        assert_eq!(
            result.operators,
            vec![
                Operator::Replace {
                    selection: None,
                    pattern: "a".to_string(),
                    replacement: "b".to_string(),
                },
                Operator::Lowercase,
            ]
        );
    }

    #[test]
    fn replace_missing_pattern_error() {
        let result = parse_programme("r//b/");
        assert!(result.is_err());
    }

    #[test]
    fn replace_missing_closing_slash_error() {
        let result = parse_programme("r/foo/bar");
        assert!(result.is_err());
    }
}
