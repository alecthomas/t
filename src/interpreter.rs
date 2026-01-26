//! Interpreter for the t language.
//!
//! The interpreter executes a programme by applying operators to a value.
//! Operators are either transforms (Value -> Value) or navigations (mutate depth).

use regex::Regex;

use crate::ast;
use crate::error::{Error, Result};
use crate::operators::{
    Ascend, Columnate, Count, DedupeSelectionWithCounts, DedupeWithCounts, DeleteEmpty, Descend,
    Filter, GroupBy, Join, JoinDelim, Lowercase, LowercaseSelected, Partition, Replace, Select,
    SortAscending, SortDescending, Split, SplitDelim, Sum, ToNumber, ToNumberSelected, Trim,
    TrimSelected, Uppercase, UppercaseSelected,
};
use crate::value::Value;

/// A transform operator converts a value to a new value.
pub trait Transform {
    /// Apply the transformation to a value.
    fn apply(&self, value: Value) -> Result<Value>;

    /// Returns true if this operator requires seeing all input to produce correct output.
    /// Operators like sort, dedupe, count, sum need full input and cannot use truncation.
    fn requires_full_input(&self) -> bool {
        false
    }
}

/// A navigation operator modifies the interpreter's depth.
pub trait Navigate {
    /// Apply the navigation to the context.
    fn apply(&self, ctx: &mut Context);
}

/// An operator is either a transform or a navigation.
pub enum Operator {
    Transform(Box<dyn Transform>),
    Navigate(Box<dyn Navigate>),
}

impl Operator {
    /// Returns true if this operator requires seeing all input to produce correct output.
    pub fn requires_full_input(&self) -> bool {
        match self {
            Operator::Transform(t) => t.requires_full_input(),
            Operator::Navigate(_) => false,
        }
    }
}

/// Execution context for the interpreter.
///
/// Owns the root value and tracks the current depth for navigation.
/// Depth 0 means operations apply to the root.
/// Depth N means operations are mapped over N levels of nesting.
pub struct Context {
    /// The root value. Wrapped in Option to allow taking ownership without cloning.
    root: Option<Value>,
    depth: usize,
}

impl Context {
    /// Create a new context with the given root value.
    pub fn new(root: Value) -> Self {
        Self {
            root: Some(root),
            depth: 0,
        }
    }

    /// Consume the context and return the final value.
    pub fn into_value(self) -> Value {
        self.root.expect("context should have root value")
    }

    /// Get the current depth.
    #[allow(dead_code)] // Reserved for future use
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Increment the depth (descend into nested structures).
    pub fn descend(&mut self) {
        self.depth += 1;
    }

    /// Decrement the depth (ascend back up).
    pub fn ascend(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Execute a transform operator at the current depth.
    pub fn execute(&mut self, op: &dyn Transform) -> Result<()> {
        let root = self.root.take().expect("context should have root value");
        self.root = Some(replace_at_depth(root, self.depth, op)?);
        Ok(())
    }
}

/// Recursively replace values at the given depth.
///
/// At depth 0, applies the transform directly to the value.
/// At depth N > 0, maps the transform over array elements at depth N-1.
fn replace_at_depth(value: Value, depth: usize, op: &dyn Transform) -> Result<Value> {
    if depth == 0 {
        op.apply(value)
    } else {
        match value {
            Value::Array(mut arr) => {
                arr.elements = arr
                    .elements
                    .into_iter()
                    .map(|v| replace_at_depth(v, depth - 1, op))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(arr))
            }
            // At depth > 0 but not an array: just return unchanged
            // (can't descend into non-arrays)
            other => Ok(other),
        }
    }
}

/// Run a programme (sequence of operators) on a context.
pub fn run(ops: &[Operator], ctx: &mut Context) -> Result<()> {
    for op in ops {
        match op {
            Operator::Transform(t) => ctx.execute(t.as_ref())?,
            Operator::Navigate(n) => n.apply(ctx),
        }
    }
    Ok(())
}

/// Compile an AST programme into a sequence of operators.
///
/// Returns an error if any operator fails to compile (e.g., invalid regex).
pub fn compile(programme: &ast::Programme) -> Result<Vec<Operator>> {
    programme.operators.iter().map(compile_op).collect()
}

/// Compile a single AST operator into an Operator.
///
/// Returns an error if a regex pattern is invalid.
fn compile_op(op: &ast::Operator) -> Result<Operator> {
    Ok(match op {
        ast::Operator::Split => Operator::Transform(Box::new(Split)),
        ast::Operator::SplitDelim(delim) => {
            Operator::Transform(Box::new(SplitDelim::new(delim.clone())))
        }
        ast::Operator::Join => Operator::Transform(Box::new(Join)),
        ast::Operator::JoinDelim(delim) => {
            Operator::Transform(Box::new(JoinDelim::new(delim.clone())))
        }
        ast::Operator::Descend => Operator::Navigate(Box::new(Descend)),
        ast::Operator::Ascend => Operator::Navigate(Box::new(Ascend)),
        ast::Operator::Uppercase => Operator::Transform(Box::new(Uppercase)),
        ast::Operator::UppercaseSelected(sel) => {
            Operator::Transform(Box::new(UppercaseSelected::new(sel.clone())))
        }
        ast::Operator::Lowercase => Operator::Transform(Box::new(Lowercase)),
        ast::Operator::LowercaseSelected(sel) => {
            Operator::Transform(Box::new(LowercaseSelected::new(sel.clone())))
        }
        ast::Operator::ToNumber => Operator::Transform(Box::new(ToNumber)),
        ast::Operator::ToNumberSelected(sel) => {
            Operator::Transform(Box::new(ToNumberSelected::new(sel.clone())))
        }
        ast::Operator::Replace {
            selection,
            pattern,
            replacement,
        } => {
            let regex = Regex::new(pattern)
                .map_err(|e| Error::runtime(format!("invalid regex '{}': {}", pattern, e)))?;
            Operator::Transform(Box::new(Replace::new(
                regex,
                replacement.clone(),
                selection.clone(),
            )))
        }
        ast::Operator::Trim => Operator::Transform(Box::new(Trim)),
        ast::Operator::TrimSelected(sel) => {
            Operator::Transform(Box::new(TrimSelected::new(sel.clone())))
        }
        ast::Operator::DeleteEmpty => Operator::Transform(Box::new(DeleteEmpty)),
        ast::Operator::DedupeWithCounts => Operator::Transform(Box::new(DedupeWithCounts)),
        ast::Operator::DedupeSelectionWithCounts(sel) => {
            Operator::Transform(Box::new(DedupeSelectionWithCounts::new(sel.clone())))
        }
        ast::Operator::Sum => Operator::Transform(Box::new(Sum)),
        ast::Operator::Count => Operator::Transform(Box::new(Count)),
        ast::Operator::Columnate => Operator::Transform(Box::new(Columnate)),
        ast::Operator::Partition(sel) => Operator::Transform(Box::new(Partition::new(sel.clone()))),
        ast::Operator::SortDescending => Operator::Transform(Box::new(SortDescending)),
        ast::Operator::SortAscending => Operator::Transform(Box::new(SortAscending)),
        ast::Operator::Selection(sel) => Operator::Transform(Box::new(Select::new(sel.clone()))),
        ast::Operator::Filter { pattern, negate } => {
            let regex = Regex::new(pattern)
                .map_err(|e| Error::runtime(format!("invalid regex '{}': {}", pattern, e)))?;
            Operator::Transform(Box::new(Filter::new(regex, *negate)))
        }
        ast::Operator::GroupBy(sel) => Operator::Transform(Box::new(GroupBy::new(sel.clone()))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Array, Level};

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn line_array(lines: &[&str]) -> Value {
        Value::Array(Array::from((
            lines.iter().map(|s| text(s)).collect(),
            Level::Line,
        )))
    }

    #[test]
    fn context_descend_ascend() {
        let mut ctx = Context::new(text("hello"));
        assert_eq!(ctx.depth(), 0);

        ctx.descend();
        assert_eq!(ctx.depth(), 1);

        ctx.descend();
        assert_eq!(ctx.depth(), 2);

        ctx.ascend();
        assert_eq!(ctx.depth(), 1);

        ctx.ascend();
        assert_eq!(ctx.depth(), 0);

        // Can't go below 0
        ctx.ascend();
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn run_empty_programme() {
        let mut ctx = Context::new(line_array(&["hello", "world"]));
        run(&[], &mut ctx).unwrap();

        match ctx.into_value() {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn compile_simple_programme() {
        let programme = ast::Programme {
            operators: vec![
                ast::Operator::Split,
                ast::Operator::Descend,
                ast::Operator::Lowercase,
                ast::Operator::Ascend,
                ast::Operator::Join,
            ],
        };

        let ops = compile(&programme).unwrap();
        assert_eq!(ops.len(), 5);
    }

    #[test]
    fn compile_filter() {
        let programme = ast::Programme {
            operators: vec![ast::Operator::Filter {
                pattern: "^a".to_string(),
                negate: false,
            }],
        };
        let ops = compile(&programme).unwrap();
        assert_eq!(ops.len(), 1);
    }

    #[test]
    fn compile_invalid_regex() {
        let programme = ast::Programme {
            operators: vec![ast::Operator::Filter {
                pattern: "[invalid".to_string(),
                negate: false,
            }],
        };
        assert!(compile(&programme).is_err());
    }
}
