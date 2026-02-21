//! Constant registration (Pass 0e).
//!
//! Registers constant definitions by inferring their types from value
//! expressions. Uses full expression inference so that computed constant
//! expressions (arithmetic, comparison, references to other constants)
//! are handled correctly.

use ori_ir::ExprId;

use crate::{Idx, ModuleChecker};

/// Register constant types.
pub fn register_consts(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    for const_def in &module.consts {
        register_const(checker, const_def);
    }
}

/// Register a single constant.
fn register_const(checker: &mut ModuleChecker<'_>, const_def: &ori_ir::ConstDef) {
    // Infer type from the value expression
    let ty = infer_const_type(checker, const_def.value);
    checker.register_const_type(const_def.name, ty);
}

/// Infer the type of a constant value expression.
///
/// Uses full expression inference so that computed constant expressions
/// (arithmetic, comparison, logical, references to other constants) are
/// handled correctly — not just literals.
fn infer_const_type(checker: &mut ModuleChecker<'_>, value_id: ExprId) -> Idx {
    let arena = checker.arena();
    let mut engine = checker.create_engine();
    let ty = crate::infer_expr(&mut engine, arena, value_id);
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    for err in errors {
        checker.push_error(err);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }
    ty
}
