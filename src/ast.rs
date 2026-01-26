/// A programme is a sequence of operators.
#[derive(Debug, Clone, PartialEq)]
pub struct Programme {
    pub operators: Vec<Operator>,
}

/// An operator in the t language.
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    /// `s` - split natural (split each element by its semantic level)
    Split,
    /// `S<delim>` - split on a custom delimiter
    SplitDelim(String),
    /// `j` - join/flatten natural
    Join,
    /// `J<delim>` - join with a custom delimiter
    JoinDelim(String),
    /// `@` - descend into nested structures
    Descend,
    /// `^` - ascend back up
    Ascend,
    /// `u` - uppercase
    Uppercase,
    /// `U<selection>` - uppercase selected elements
    UppercaseSelected(Selection),
    /// `l` - lowercase
    Lowercase,
    /// `L<selection>` - lowercase selected elements
    LowercaseSelected(Selection),
    /// `r[<selection>]/<old>/<new>/` - regex replace, optionally in selected elements
    Replace {
        selection: Option<Selection>,
        pattern: String,
        replacement: String,
    },
    /// `n` - convert to number
    ToNumber,
    /// `N<selection>` - convert to number selected elements
    ToNumberSelected(Selection),
    /// `t` - trim whitespace
    Trim,
    /// `T<selection>` - trim selected elements
    TrimSelected(Selection),
    /// `x` - delete empty elements
    DeleteEmpty,
    /// `d` - dedupe with counts
    DedupeWithCounts,
    /// `D` - dedupe (without counts)
    Dedupe,
    /// `+` - sum numeric values
    Sum,
    /// `#` - count elements
    Count,
    /// `c` - columnate
    Columnate,
    /// `p<selection>` - partition array at indices
    Partition(Selection),
    /// `o` - sort descending
    SortDescending,
    /// `O` - sort ascending
    SortAscending,
    /// Selection - select elements by index, slice, or multi-select
    Selection(Selection),
    /// `/<regex>/` - filter keep matching elements
    /// `!/<regex>/` - filter remove matching elements (keep non-matching)
    Filter { pattern: String, negate: bool },
    /// `g<selection>` - group by the value(s) at the selection
    GroupBy(Selection),
}

/// A selection is a comma-separated list of select items.
/// It's a reduce operator that collapses an array to a subset.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub items: Vec<SelectItem>,
}

/// A single item in a selection: either an index or a slice.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    /// A single index (0-based, negative counts from end)
    Index(i64),
    /// A slice with optional start, end, and step
    Slice(Slice),
}

/// A slice selects a range of elements.
/// All fields are optional:
/// - `start`: starting index (default: 0 or end if step is negative)
/// - `end`: ending index, exclusive (default: end or 0 if step is negative)
/// - `step`: stride (default: 1)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Slice {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub step: Option<i64>,
}
