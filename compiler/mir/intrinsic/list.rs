use syntax::span::Span;

use runtime::boxed;
use runtime::boxed::prelude::*;

use crate::mir::builder::Builder;
use crate::mir::error::Result;
use crate::mir::eval_hir::EvalHirCtx;
use crate::mir::intrinsic::Intrinsic;
use crate::mir::value::list::list_value_length;
use crate::mir::value::ListIterator;
use crate::mir::Value;

pub struct Length {}

impl Length {
    fn value_len(value: &Value) -> Option<usize> {
        list_value_length(value)
    }
}

impl Intrinsic for Length {
    fn eval_arg_list(
        ehx: &mut EvalHirCtx,
        _b: &mut Option<Builder>,
        _span: Span,
        mut iter: ListIterator<'_>,
    ) -> Result<Option<Value>> {
        let single_arg = iter.next_unchecked();

        Ok(Self::value_len(single_arg).map(|known_length| {
            Value::Const(boxed::Int::new(ehx, known_length as i64).as_any_ref())
        }))
    }
}
