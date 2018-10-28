use std::collections::HashMap;
use std::rc::Rc;

use syntax::span::Span;

use crate::hir;
use crate::mir::builder::Builder;
use crate::mir::eval_hir::EvalHirCtx;
use crate::mir::ops;
use crate::mir::value::Value;

type ValueVec = Vec<(hir::VarId, Value)>;

#[derive(Clone, Debug)]
pub struct Closure {
    pub const_values: ValueVec,
    pub free_values: ValueVec,
}

impl Closure {
    pub fn empty() -> Closure {
        Closure {
            const_values: vec![],
            free_values: vec![],
        }
    }

    pub fn needs_closure_param(&self) -> bool {
        !self.free_values.is_empty()
    }
}

fn is_free_value(value: &Value) -> bool {
    match value {
        Value::Const(_)
        | Value::EqPred
        | Value::TyPred(_)
        | Value::RustFun(_)
        | Value::ArretFun(_) => false,
        _ => true,
    }
}

pub fn calculate_closure(
    local_values: &HashMap<hir::VarId, Value>,
    capturing_expr: &hir::Expr<hir::Inferred>,
) -> Closure {
    let mut captured_values = HashMap::new();

    // Only process captures if there are local values. This is to avoid visiting the expression
    // when capturing isn't possible
    if !local_values.is_empty() {
        // Look for references to variables inside the function
        hir::visitor::visit_exprs(&capturing_expr, &mut |expr| {
            if let hir::Expr::Ref(_, var_id) = expr {
                // Avoiding cloning the value if we've already captured
                if !captured_values.contains_key(var_id) {
                    if let Some(value) = local_values.get(var_id) {
                        // Local value is referenced; capture
                        captured_values.insert(*var_id, value.clone());
                    }
                }
            }
        });
    }

    // Determine which captures are constants
    type ValueVec = Vec<(hir::VarId, Value)>;
    let (free_values, const_values): (ValueVec, ValueVec) = captured_values
        .into_iter()
        .partition(|(_, value)| is_free_value(value));

    Closure {
        const_values,
        free_values,
    }
}

pub fn save_to_closure_reg(
    ehx: &mut EvalHirCtx,
    b: &mut Builder,
    span: Span,
    closure: &Closure,
) -> Option<ops::RegId> {
    match closure.free_values.first() {
        Some((_, value)) => {
            use crate::mir::value::build_reg::value_to_reg;
            use runtime::abitype;

            Some(value_to_reg(ehx, b, span, value, &abitype::BoxedABIType::Any.into()).into())
        }
        None => None,
    }
}

/// Loads a closure assuming all captured variables are still inside the local function
pub fn load_from_current_fun(local_values: &mut HashMap<hir::VarId, Value>, closure: &Closure) {
    local_values.extend(
        closure
            .const_values
            .iter()
            .chain(closure.free_values.iter())
            .map(|(var_id, value)| (*var_id, value.clone())),
    );
}

/// Loads a closure from a closure parameter
pub fn load_from_closure_param(
    local_values: &mut HashMap<hir::VarId, Value>,
    closure: &Closure,
    closure_reg: Option<ops::RegId>,
) {
    use crate::mir::value;
    use runtime::abitype;

    if closure.free_values.len() > 1 {
        // This needs record support
        unimplemented!("capturing multiple free values");
    }

    // Include the const values directly
    local_values.extend(
        closure
            .const_values
            .iter()
            .map(|(var_id, value)| (*var_id, value.clone())),
    );

    if let Some((var_id, _)) = closure.free_values.first() {
        let closure_reg = closure_reg.unwrap();

        local_values.insert(
            *var_id,
            Value::Reg(Rc::new(value::RegValue {
                reg: closure_reg,
                abi_type: abitype::BoxedABIType::Any.into(),
            })),
        );
    }
}
