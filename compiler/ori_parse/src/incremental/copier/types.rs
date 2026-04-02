//! Type and function construct copying for the AST copier.

use super::AstCopier;
use ori_ir::{
    ast::{FunctionExp, FunctionSeq},
    ExprArena, NamedExpr, ParsedType, ParsedTypeId, ParsedTypeRange,
};

impl AstCopier<'_> {
    /// Copy a parsed type, allocating nested types in the new arena.
    pub(super) fn copy_parsed_type(
        &self,
        ty: &ParsedType,
        new_arena: &mut ExprArena,
    ) -> ParsedType {
        match ty {
            ParsedType::Primitive(id) => ParsedType::Primitive(*id),
            ParsedType::Named { name, type_args } => {
                let new_type_args = self.copy_parsed_type_range(*type_args, new_arena);
                ParsedType::Named {
                    name: *name,
                    type_args: new_type_args,
                }
            }
            ParsedType::List(elem_id) => {
                let new_elem_id = self.copy_parsed_type_id(*elem_id, new_arena);
                ParsedType::List(new_elem_id)
            }
            ParsedType::FixedList { elem, capacity } => {
                let new_elem = self.copy_parsed_type_id(*elem, new_arena);
                let new_capacity = self.copy_expr(*capacity, new_arena);
                ParsedType::FixedList {
                    elem: new_elem,
                    capacity: new_capacity,
                }
            }
            ParsedType::Tuple(elems) => {
                let new_elems = self.copy_parsed_type_range(*elems, new_arena);
                ParsedType::Tuple(new_elems)
            }
            ParsedType::Function { params, ret } => {
                let new_params = self.copy_parsed_type_range(*params, new_arena);
                let new_ret = self.copy_parsed_type_id(*ret, new_arena);
                ParsedType::Function {
                    params: new_params,
                    ret: new_ret,
                }
            }
            ParsedType::Map { key, value } => {
                let new_key = self.copy_parsed_type_id(*key, new_arena);
                let new_value = self.copy_parsed_type_id(*value, new_arena);
                ParsedType::Map {
                    key: new_key,
                    value: new_value,
                }
            }
            ParsedType::Infer => ParsedType::Infer,
            ParsedType::SelfType => ParsedType::SelfType,
            ParsedType::AssociatedType { base, assoc_name } => {
                let new_base = self.copy_parsed_type_id(*base, new_arena);
                ParsedType::AssociatedType {
                    base: new_base,
                    assoc_name: *assoc_name,
                }
            }
            ParsedType::ConstExpr(expr_id) => {
                let new_expr = self.copy_expr(*expr_id, new_arena);
                ParsedType::ConstExpr(new_expr)
            }
            ParsedType::TraitBounds(bounds) => {
                let new_bounds = self.copy_parsed_type_range(*bounds, new_arena);
                ParsedType::TraitBounds(new_bounds)
            }
        }
    }

    /// Copy a parsed type by ID, allocating in the new arena.
    pub(super) fn copy_parsed_type_id(
        &self,
        old_id: ParsedTypeId,
        new_arena: &mut ExprArena,
    ) -> ParsedTypeId {
        let old_ty = self.old_arena.get_parsed_type(old_id);
        let new_ty = self.copy_parsed_type(old_ty, new_arena);
        new_arena.alloc_parsed_type(new_ty)
    }

    /// Copy an optional parsed type ID (INVALID sentinel = no type annotation).
    pub(super) fn copy_optional_parsed_type_id(
        &self,
        id: ParsedTypeId,
        new_arena: &mut ExprArena,
    ) -> ParsedTypeId {
        if id.is_valid() {
            self.copy_parsed_type_id(id, new_arena)
        } else {
            ParsedTypeId::INVALID
        }
    }

    /// Copy a parsed type range, allocating in the new arena.
    pub(super) fn copy_parsed_type_range(
        &self,
        range: ParsedTypeRange,
        new_arena: &mut ExprArena,
    ) -> ParsedTypeRange {
        let old_ids = self.old_arena.get_parsed_type_list(range);
        let new_ids: Vec<_> = old_ids
            .iter()
            .map(|id| self.copy_parsed_type_id(*id, new_arena))
            .collect();
        new_arena.alloc_parsed_type_list(new_ids)
    }

    /// Copy a `FunctionSeq`.
    pub(super) fn copy_function_seq(
        &self,
        seq: &FunctionSeq,
        new_arena: &mut ExprArena,
    ) -> FunctionSeq {
        match seq {
            FunctionSeq::Try {
                stmts,
                result,
                span,
            } => {
                let old_stmts = self.old_arena.get_stmt_range(*stmts);
                let new_stmts: Vec<_> = old_stmts
                    .iter()
                    .map(|s| self.copy_stmt(s, new_arena))
                    .collect();
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "statement indices won't exceed u32::MAX in practice"
                )]
                let start_id = if new_stmts.is_empty() {
                    0
                } else {
                    let first_id = new_arena.alloc_stmt(new_stmts[0].clone());
                    for stmt in new_stmts.iter().skip(1) {
                        new_arena.alloc_stmt(stmt.clone());
                    }
                    first_id.index() as u32
                };
                FunctionSeq::Try {
                    stmts: new_arena.alloc_stmt_range(start_id, new_stmts.len()),
                    result: self.copy_expr(*result, new_arena),
                    span: self.adjust_span(*span),
                }
            }
            FunctionSeq::Match {
                scrutinee,
                arms,
                span,
            } => {
                let old_arms = self.old_arena.get_arms(*arms);
                let new_arms: Vec<_> = old_arms
                    .iter()
                    .map(|arm| self.copy_match_arm(arm, new_arena))
                    .collect();
                FunctionSeq::Match {
                    scrutinee: self.copy_expr(*scrutinee, new_arena),
                    arms: new_arena.alloc_arms(new_arms),
                    span: self.adjust_span(*span),
                }
            }
            FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default,
                span,
            } => FunctionSeq::ForPattern {
                over: self.copy_expr(*over, new_arena),
                map: map.map(|m| self.copy_expr(m, new_arena)),
                arm: self.copy_match_arm(arm, new_arena),
                default: self.copy_expr(*default, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a `FunctionExp`.
    pub(super) fn copy_function_exp(
        &self,
        exp: &FunctionExp,
        new_arena: &mut ExprArena,
    ) -> FunctionExp {
        let old_props = self.old_arena.get_named_exprs(exp.props);
        let new_props: Vec<_> = old_props
            .iter()
            .map(|p| self.copy_named_expr(p, new_arena))
            .collect();
        FunctionExp {
            kind: exp.kind,
            props: new_arena.alloc_named_exprs(new_props),
            type_args: self.copy_parsed_type_range(exp.type_args, new_arena),
            span: self.adjust_span(exp.span),
        }
    }

    /// Copy a named expression.
    fn copy_named_expr(&self, expr: &NamedExpr, new_arena: &mut ExprArena) -> NamedExpr {
        NamedExpr {
            name: expr.name,
            value: self.copy_expr(expr.value, new_arena),
            span: self.adjust_span(expr.span),
        }
    }
}
