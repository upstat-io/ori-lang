//! Constant registration (Pass 0e).
//!
//! Registers constant definitions by inferring their types from value
//! expressions. Uses full expression inference so that computed constant
//! expressions (arithmetic, comparison, references to other constants)
//! are handled correctly.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, UnaryOp};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::const_eval::{checked_int_const_binary, is_int_const_binary_op};
use crate::{ConstValue, Expected, Idx, InvalidFixedListCapacityReason, ModuleChecker};

/// Register constant types.
pub fn register_consts(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    let values = {
        let mut evaluator = ModuleConstEvaluator::new(checker.arena(), module);
        evaluator.evaluate_all()
    };
    predeclare_const_types(checker, module, &values);
    for const_def in &module.consts {
        register_const(checker, const_def);
    }

    for (name, value) in values {
        checker.register_const_value(name, value);
    }
}

/// Predeclare every module constant before inferring any initializer. Concrete
/// values provide their exact type; the remaining declarations share fresh
/// unification variables across forward references and cycles.
fn predeclare_const_types(
    checker: &mut ModuleChecker<'_>,
    module: &ori_ir::Module,
    values: &FxHashMap<Name, ConstValue>,
) {
    for definition in &module.consts {
        let ty = match values.get(&definition.name) {
            Some(ConstValue::Int(_)) => Idx::INT,
            Some(ConstValue::Bool(_)) => Idx::BOOL,
            None => checker.pool_mut().fresh_named_var(definition.name),
        };
        checker.register_const_type(definition.name, ty);
    }
}

/// Register a single constant.
#[expect(
    clippy::expect_used,
    reason = "predeclare_const_types iterates the same module.consts list and registers every \
              const's type before register_const runs, so const_type() is always populated here"
)]
fn register_const(checker: &mut ModuleChecker<'_>, const_def: &ori_ir::ConstDef) {
    let expected = checker
        .const_type(const_def.name)
        .expect("module const type must be predeclared");
    infer_const_type(checker, const_def.value, expected);
}

/// Infer the type of a constant value expression.
///
/// Uses full expression inference so that computed constant expressions
/// (arithmetic, comparison, logical, references to other constants) are
/// handled correctly — not just literals.
fn infer_const_type(checker: &mut ModuleChecker<'_>, value_id: ExprId, expected: Idx) {
    let arena = checker.arena();
    let (expr_types, errors, warnings) = {
        let mut engine = checker.create_engine();
        let ty = crate::infer_expr(&mut engine, arena, value_id);
        let span = arena.get_expr(value_id).span;
        let _ = engine.check_type(ty, &Expected::no_expectation(expected), span);

        let mut expr_types = engine.take_expr_types();
        for expr_ty in expr_types.values_mut() {
            *expr_ty = engine.pool().resolve_fully(*expr_ty);
        }
        (expr_types, engine.take_errors(), engine.take_warnings())
    };
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }
    for err in errors {
        checker.push_error(err);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }
}

/// Evaluates the integer/bool subset needed by const-generic capacity types.
/// Unsupported module constants remain valid declarations but publish no value
/// evidence, so using one as a capacity fails closed with E2059.
struct ModuleConstEvaluator<'a> {
    arena: &'a ExprArena,
    definitions: FxHashMap<Name, ExprId>,
    values: FxHashMap<Name, ConstValue>,
    visiting: FxHashSet<Name>,
    failed: FxHashSet<Name>,
}

impl<'a> ModuleConstEvaluator<'a> {
    fn new(arena: &'a ExprArena, module: &ori_ir::Module) -> Self {
        Self {
            arena,
            definitions: module
                .consts
                .iter()
                .map(|definition| (definition.name, definition.value))
                .collect(),
            values: FxHashMap::default(),
            visiting: FxHashSet::default(),
            failed: FxHashSet::default(),
        }
    }

    fn evaluate_all(&mut self) -> FxHashMap<Name, ConstValue> {
        let names: Vec<_> = self.definitions.keys().copied().collect();
        for name in names {
            let _ = self.evaluate_name(name);
        }
        std::mem::take(&mut self.values)
    }

    fn evaluate_name(&mut self, name: Name) -> Result<ConstValue, InvalidFixedListCapacityReason> {
        if let Some(value) = self.values.get(&name).cloned() {
            return Ok(value);
        }
        if self.failed.contains(&name) || !self.visiting.insert(name) {
            return Err(InvalidFixedListCapacityReason::UnsupportedExpression);
        }
        let result = self
            .definitions
            .get(&name)
            .copied()
            .ok_or(InvalidFixedListCapacityReason::UnsupportedExpression)
            .and_then(|expr| self.evaluate_expr(expr));
        self.visiting.remove(&name);
        match result {
            Ok(value) => {
                self.values.insert(name, value.clone());
                Ok(value)
            }
            Err(reason) => {
                self.failed.insert(name);
                Err(reason)
            }
        }
    }

    fn evaluate_expr(
        &mut self,
        expr: ExprId,
    ) -> Result<ConstValue, InvalidFixedListCapacityReason> {
        match self.arena.get_expr(expr).kind {
            ExprKind::Int(value) => Ok(ConstValue::Int(value)),
            ExprKind::Bool(value) => Ok(ConstValue::Bool(value)),
            ExprKind::Ident(name) | ExprKind::Const(name) => self.evaluate_name(name),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match self.evaluate_expr(operand)? {
                ConstValue::Int(value) => value
                    .checked_neg()
                    .map(ConstValue::Int)
                    .ok_or(InvalidFixedListCapacityReason::ArithmeticOverflow),
                ConstValue::Bool(_) => Err(InvalidFixedListCapacityReason::NonInteger),
            },
            ExprKind::Binary { op, left, right } if is_int_const_binary_op(op) => {
                let ConstValue::Int(left) = self.evaluate_expr(left)? else {
                    return Err(InvalidFixedListCapacityReason::NonInteger);
                };
                let ConstValue::Int(right) = self.evaluate_expr(right)? else {
                    return Err(InvalidFixedListCapacityReason::NonInteger);
                };
                checked_int_const_binary(op, left, right).map(ConstValue::Int)
            }
            _ => Err(InvalidFixedListCapacityReason::UnsupportedExpression),
        }
    }
}
