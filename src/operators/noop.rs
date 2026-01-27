use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::Value;

pub struct NoOp;

impl Transform for NoOp {
    fn apply(&self, value: Value) -> Result<Value> {
        Ok(value)
    }
}
