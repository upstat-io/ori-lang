//! Owned const expressions and checked operations shared by const consumers.

use ori_ir::visitor::{walk_expr, walk_stmt, Visitor};
use ori_ir::{
    BinaryOp, Expr, ExprArena, ExprId, ExprKind, Name, Param, ParsedType, ParsedTypeId, Stmt,
    UnaryOp,
};

use crate::{ConstValue, InvalidFixedListCapacityReason};

/// Arena-independent const expression retained by method metadata.
///
/// Method signatures and bodies outlive their parse arena in the registry. This
/// representation preserves the admitted generic-const expression shape so a
/// concrete call-site binding can be checked against every fixed-list capacity
/// it feeds without retaining `ExprId`s.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GenericConstExpr {
    /// Integer literal.
    Int(i64),
    /// Boolean literal.
    Bool(bool),
    /// Const parameter, local const, or module const reference.
    Name(Name),
    /// Checked integer negation.
    Neg(Box<Self>),
    /// Checked integer binary expression.
    Binary {
        /// Operator.
        op: BinaryOp,
        /// Left operand.
        left: Box<Self>,
        /// Right operand.
        right: Box<Self>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvaluatedConstExpr {
    Concrete(ConstValue),
    Symbolic,
}

impl GenericConstExpr {
    pub(crate) fn from_arena(
        arena: &ExprArena,
        expr: ExprId,
    ) -> Result<Self, InvalidFixedListCapacityReason> {
        match arena.get_expr(expr).kind {
            ExprKind::Int(value) => Ok(Self::Int(value)),
            ExprKind::Bool(value) => Ok(Self::Bool(value)),
            ExprKind::Ident(name) | ExprKind::Const(name) => Ok(Self::Name(name)),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => Ok(Self::Neg(Box::new(Self::from_arena(arena, operand)?))),
            ExprKind::Binary { op, left, right } if is_int_const_binary_op(op) => {
                Ok(Self::Binary {
                    op,
                    left: Box::new(Self::from_arena(arena, left)?),
                    right: Box::new(Self::from_arena(arena, right)?),
                })
            }
            _ => Err(InvalidFixedListCapacityReason::UnsupportedExpression),
        }
    }

    pub(crate) fn evaluate(
        &self,
        resolve_name: &mut impl FnMut(Name) -> Option<ConstValue>,
    ) -> Result<EvaluatedConstExpr, InvalidFixedListCapacityReason> {
        match self {
            Self::Int(value) => Ok(EvaluatedConstExpr::Concrete(ConstValue::Int(*value))),
            Self::Bool(value) => Ok(EvaluatedConstExpr::Concrete(ConstValue::Bool(*value))),
            Self::Name(name) => Ok(resolve_name(*name)
                .map_or(EvaluatedConstExpr::Symbolic, EvaluatedConstExpr::Concrete)),
            Self::Neg(operand) => match operand.evaluate(resolve_name)? {
                EvaluatedConstExpr::Concrete(ConstValue::Int(value)) => value
                    .checked_neg()
                    .map(ConstValue::Int)
                    .map(EvaluatedConstExpr::Concrete)
                    .ok_or(InvalidFixedListCapacityReason::ArithmeticOverflow),
                EvaluatedConstExpr::Concrete(ConstValue::Bool(_)) => {
                    Err(InvalidFixedListCapacityReason::NonInteger)
                }
                EvaluatedConstExpr::Symbolic => Ok(EvaluatedConstExpr::Symbolic),
            },
            Self::Binary { op, left, right } => {
                let left = left.evaluate(resolve_name)?;
                let right = right.evaluate(resolve_name)?;
                evaluate_const_binary(*op, left, right)
            }
        }
    }

    fn references_any(&self, names: &[Name]) -> bool {
        match self {
            Self::Name(name) => names.contains(name),
            Self::Neg(operand) => operand.references_any(names),
            Self::Binary { left, right, .. } => {
                left.references_any(names) || right.references_any(names)
            }
            Self::Int(_) | Self::Bool(_) => false,
        }
    }
}

fn evaluate_const_binary(
    op: BinaryOp,
    left: EvaluatedConstExpr,
    right: EvaluatedConstExpr,
) -> Result<EvaluatedConstExpr, InvalidFixedListCapacityReason> {
    match (left, right) {
        (
            EvaluatedConstExpr::Concrete(ConstValue::Int(left)),
            EvaluatedConstExpr::Concrete(ConstValue::Int(right)),
        ) => checked_int_const_binary(op, left, right)
            .map(ConstValue::Int)
            .map(EvaluatedConstExpr::Concrete),
        (EvaluatedConstExpr::Concrete(ConstValue::Bool(_)), _)
        | (_, EvaluatedConstExpr::Concrete(ConstValue::Bool(_))) => {
            Err(InvalidFixedListCapacityReason::NonInteger)
        }
        (_, EvaluatedConstExpr::Concrete(ConstValue::Int(right))) => {
            validate_known_int_binary_rhs(op, right)?;
            Ok(EvaluatedConstExpr::Symbolic)
        }
        _ => Ok(EvaluatedConstExpr::Symbolic),
    }
}

/// Collect owned fixed-list capacities that depend on method const binders.
pub(crate) fn collect_method_capacity_constraints(
    arena: &ExprArena,
    const_params: &[Name],
    params: &[Param],
    return_ty: &ParsedType,
    body: Option<ExprId>,
) -> Vec<GenericConstExpr> {
    if const_params.is_empty() {
        return Vec::new();
    }

    let mut collector = CapacityConstraintCollector {
        const_params,
        constraints: Vec::new(),
    };
    for param in params {
        if let Some(ty) = &param.ty {
            collector.collect_type(arena, ty);
        }
    }
    collector.collect_type(arena, return_ty);
    if let Some(body) = body {
        collector.visit_expr_id(body, arena);
    }
    collector.constraints
}

struct CapacityConstraintCollector<'a> {
    const_params: &'a [Name],
    constraints: Vec<GenericConstExpr>,
}

impl CapacityConstraintCollector<'_> {
    fn collect_type(&mut self, arena: &ExprArena, parsed: &ParsedType) {
        match parsed {
            ParsedType::FixedList { elem, capacity } => {
                if let Ok(expr) = GenericConstExpr::from_arena(arena, *capacity) {
                    if expr.references_any(self.const_params) && !self.constraints.contains(&expr) {
                        self.constraints.push(expr);
                    }
                }
                self.collect_type(arena, arena.get_parsed_type(*elem));
            }
            ParsedType::List(elem) => self.collect_type(arena, arena.get_parsed_type(*elem)),
            ParsedType::Map { key, value } => {
                self.collect_type(arena, arena.get_parsed_type(*key));
                self.collect_type(arena, arena.get_parsed_type(*value));
            }
            ParsedType::Tuple(elements) | ParsedType::TraitBounds(elements) => {
                for &element in arena.get_parsed_type_list(*elements) {
                    self.collect_type(arena, arena.get_parsed_type(element));
                }
            }
            ParsedType::Function { params, ret } => {
                for &param in arena.get_parsed_type_list(*params) {
                    self.collect_type(arena, arena.get_parsed_type(param));
                }
                self.collect_type(arena, arena.get_parsed_type(*ret));
            }
            ParsedType::Named { type_args, .. } => {
                for &arg in arena.get_parsed_type_list(*type_args) {
                    self.collect_type(arena, arena.get_parsed_type(arg));
                }
            }
            ParsedType::AssociatedType { base, .. } => {
                self.collect_type(arena, arena.get_parsed_type(*base));
            }
            ParsedType::Primitive(_)
            | ParsedType::Infer
            | ParsedType::SelfType
            | ParsedType::ConstExpr(_) => {}
        }
    }

    fn collect_type_id(&mut self, arena: &ExprArena, id: ParsedTypeId) {
        if id.is_valid() {
            self.collect_type(arena, arena.get_parsed_type(id));
        }
    }
}

impl<'ast> Visitor<'ast> for CapacityConstraintCollector<'_> {
    fn visit_expr(&mut self, expr: &Expr, arena: &'ast ExprArena) {
        match expr.kind {
            ExprKind::Let { ty, .. } | ExprKind::Cast { ty, .. } => {
                self.collect_type_id(arena, ty);
            }
            ExprKind::Lambda { ret_ty, .. } => self.collect_type_id(arena, ret_ty),
            _ => {}
        }
        walk_expr(self, expr, arena);
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt, arena: &'ast ExprArena) {
        if let ori_ir::StmtKind::Let { ty, .. } = stmt.kind {
            self.collect_type_id(arena, ty);
        }
        walk_stmt(self, stmt, arena);
    }

    fn visit_param(&mut self, param: &'ast Param, arena: &'ast ExprArena) {
        if let Some(ty) = &param.ty {
            self.collect_type(arena, ty);
        }
    }
}

pub(crate) const fn is_int_const_binary_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
    )
}

pub(crate) fn validate_known_int_binary_rhs(
    op: BinaryOp,
    right: i64,
) -> Result<(), InvalidFixedListCapacityReason> {
    match op {
        BinaryOp::Div | BinaryOp::Mod if right == 0 => {
            Err(InvalidFixedListCapacityReason::DivisionByZero)
        }
        BinaryOp::Shl | BinaryOp::Shr if !(0..i64::from(i64::BITS)).contains(&right) => {
            Err(InvalidFixedListCapacityReason::InvalidShift)
        }
        _ => Ok(()),
    }
}

pub(crate) fn checked_int_const_binary(
    op: BinaryOp,
    left: i64,
    right: i64,
) -> Result<i64, InvalidFixedListCapacityReason> {
    use InvalidFixedListCapacityReason::{
        ArithmeticOverflow, DivisionByZero, InvalidShift, UnsupportedExpression,
    };

    match op {
        BinaryOp::Add => left.checked_add(right).ok_or(ArithmeticOverflow),
        BinaryOp::Sub => left.checked_sub(right).ok_or(ArithmeticOverflow),
        BinaryOp::Mul => left.checked_mul(right).ok_or(ArithmeticOverflow),
        BinaryOp::Div => {
            if right == 0 {
                Err(DivisionByZero)
            } else {
                left.checked_div(right).ok_or(ArithmeticOverflow)
            }
        }
        BinaryOp::Mod => {
            if right == 0 {
                Err(DivisionByZero)
            } else {
                left.checked_rem(right).ok_or(ArithmeticOverflow)
            }
        }
        BinaryOp::BitAnd => Ok(left & right),
        BinaryOp::BitOr => Ok(left | right),
        BinaryOp::BitXor => Ok(left ^ right),
        BinaryOp::Shl => {
            validate_known_int_binary_rhs(op, right)?;
            let shift = u32::try_from(right).map_err(|_| InvalidShift)?;
            let shifted = left.checked_shl(shift).ok_or(InvalidShift)?;
            if shifted.checked_shr(shift) == Some(left) {
                Ok(shifted)
            } else {
                Err(ArithmeticOverflow)
            }
        }
        BinaryOp::Shr => {
            validate_known_int_binary_rhs(op, right)?;
            let shift = u32::try_from(right).map_err(|_| InvalidShift)?;
            left.checked_shr(shift).ok_or(InvalidShift)
        }
        _ => Err(UnsupportedExpression),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_integer_const_arithmetic_rejects_invalid_results() {
        assert_eq!(
            checked_int_const_binary(BinaryOp::Div, 4, 0),
            Err(InvalidFixedListCapacityReason::DivisionByZero)
        );
        assert_eq!(
            checked_int_const_binary(BinaryOp::Add, i64::MAX, 1),
            Err(InvalidFixedListCapacityReason::ArithmeticOverflow)
        );
        assert_eq!(
            checked_int_const_binary(BinaryOp::Shl, 1, 64),
            Err(InvalidFixedListCapacityReason::InvalidShift)
        );
        assert_eq!(
            checked_int_const_binary(BinaryOp::Shl, i64::MAX, 1),
            Err(InvalidFixedListCapacityReason::ArithmeticOverflow)
        );
    }

    #[test]
    fn checked_integer_const_arithmetic_evaluates_approved_operations() {
        let cases = [
            (BinaryOp::Add, 4, 2, 6),
            (BinaryOp::Sub, 4, 2, 2),
            (BinaryOp::Mul, 4, 2, 8),
            (BinaryOp::Div, 5, 2, 2),
            (BinaryOp::Mod, 5, 2, 1),
            (BinaryOp::BitAnd, 6, 3, 2),
            (BinaryOp::BitOr, 4, 2, 6),
            (BinaryOp::BitXor, 7, 3, 4),
            (BinaryOp::Shl, 3, 1, 6),
            (BinaryOp::Shr, 6, 1, 3),
        ];
        for (op, left, right, expected) in cases {
            assert_eq!(checked_int_const_binary(op, left, right), Ok(expected));
        }
    }
}
