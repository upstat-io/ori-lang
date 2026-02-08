//! Pattern lowering — binding destructuring and match pattern tests.
//!
//! - [`bind_pattern`] — destructure a `BindingPattern` into scope bindings
//!   (for `let` expressions).
//! - [`compile_pattern_test`] — emit a boolean test for a `MatchPattern`
//!   (for `match` arms).
//! - [`bind_match_pattern`] — extract values from a matched scrutinee into
//!   the current scope.

use ori_ir::ast::MatchPattern;
use ori_ir::{BindingPattern, ExprId, Name, Span};
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // ── bind_pattern (for let) ─────────────────────────────────

    /// Bind a `BindingPattern` to an ARC IR value.
    ///
    /// Recursively destructures tuples, structs, and lists, adding
    /// `Project` instructions for each field and binding names in the scope.
    // Field/variant/element indices never exceed u32.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn bind_pattern(
        &mut self,
        pattern: &BindingPattern,
        value: ArcVarId,
        mutable: bool,
        init_id: ExprId,
    ) {
        match pattern {
            BindingPattern::Name(name) => {
                if mutable {
                    self.scope.bind_mutable(*name, value);
                } else {
                    self.scope.bind(*name, value);
                }
            }

            BindingPattern::Wildcard => {
                // Discard — no binding.
            }

            BindingPattern::Tuple(elements) => {
                let init_ty = self.expr_type(init_id);
                for (i, sub_pattern) in elements.iter().enumerate() {
                    let elem_ty = self.tuple_elem_type(init_ty, i);
                    let proj = self.builder.emit_project(elem_ty, value, i as u32, None);
                    self.bind_pattern(sub_pattern, proj, mutable, init_id);
                }
            }

            BindingPattern::Struct { fields } => {
                let init_ty = self.expr_type(init_id);
                for (i, (field_name, sub_pattern)) in fields.iter().enumerate() {
                    let field_ty = self.struct_field_type(init_ty, *field_name, i);
                    let proj = self.builder.emit_project(field_ty, value, i as u32, None);
                    if let Some(sub) = sub_pattern {
                        self.bind_pattern(sub, proj, mutable, init_id);
                    } else {
                        // Shorthand: `let { x } = val` binds field to name.
                        if mutable {
                            self.scope.bind_mutable(*field_name, proj);
                        } else {
                            self.scope.bind(*field_name, proj);
                        }
                    }
                }
            }

            BindingPattern::List { elements, rest } => {
                let init_ty = self.expr_type(init_id);
                let elem_ty = self.list_elem_type(init_ty);
                for (i, sub_pattern) in elements.iter().enumerate() {
                    let proj = self.builder.emit_project(elem_ty, value, i as u32, None);
                    self.bind_pattern(sub_pattern, proj, mutable, init_id);
                }
                if let Some(rest_name) = rest {
                    // List rest pattern: `let [head, ..tail] = list`
                    // For now, bind rest to the original value (full list subslice
                    // extraction requires runtime support).
                    if mutable {
                        self.scope.bind_mutable(*rest_name, value);
                    } else {
                        self.scope.bind(*rest_name, value);
                    }
                    tracing::debug!("list rest pattern bound to full value (subslice pending)");
                }
            }
        }
    }

    // ── compile_pattern_test (for match) ───────────────────────

    /// Emit a boolean test for whether a scrutinee matches a `MatchPattern`.
    ///
    /// Returns an `ArcVarId` holding a `bool` value.
    pub(crate) fn compile_pattern_test(
        &mut self,
        pattern: &MatchPattern,
        scrutinee: ArcVarId,
        scrut_ty: Idx,
        span: Span,
    ) -> ArcVarId {
        match pattern {
            MatchPattern::Wildcard | MatchPattern::Binding(_) => {
                // Always matches.
                self.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::Literal(LitValue::Bool(true)),
                    Some(span),
                )
            }

            MatchPattern::Literal(expr_id) => {
                let pat_val = self.lower_expr(*expr_id);
                self.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                        args: vec![scrutinee, pat_val],
                    },
                    Some(span),
                )
            }

            MatchPattern::Variant { name, inner: _ } => {
                // Tag check: project tag field and compare.
                let tag = self
                    .builder
                    .emit_project(Idx::INT, scrutinee, 0, Some(span));
                let variant_idx = self.resolve_variant_index(scrut_ty, *name);
                let expected_tag = self.builder.emit_let(
                    Idx::INT,
                    ArcValue::Literal(LitValue::Int(i64::from(variant_idx))),
                    None,
                );
                self.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                        args: vec![tag, expected_tag],
                    },
                    Some(span),
                )
            }

            MatchPattern::Tuple(_) | MatchPattern::Struct { .. } | MatchPattern::List { .. } => {
                // Structural patterns always match (after type checking).
                self.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::Literal(LitValue::Bool(true)),
                    Some(span),
                )
            }

            MatchPattern::Range {
                start,
                end,
                inclusive,
            } => self.compile_range_test(scrutinee, *start, *end, *inclusive, span),

            MatchPattern::Or(sub_patterns) => {
                let pat_ids: Vec<_> = self.arena.get_match_pattern_list(*sub_patterns).to_vec();

                if pat_ids.is_empty() {
                    return self.builder.emit_let(
                        Idx::BOOL,
                        ArcValue::Literal(LitValue::Bool(false)),
                        Some(span),
                    );
                }

                // Sequential OR: test each sub-pattern.
                let first_pat = self.arena.get_match_pattern(pat_ids[0]);
                let mut result = self.compile_pattern_test(first_pat, scrutinee, scrut_ty, span);

                for &pat_id in &pat_ids[1..] {
                    let sub_pat = self.arena.get_match_pattern(pat_id);
                    let sub_result = self.compile_pattern_test(sub_pat, scrutinee, scrut_ty, span);
                    result = self.builder.emit_let(
                        Idx::BOOL,
                        ArcValue::PrimOp {
                            op: PrimOp::Binary(ori_ir::BinaryOp::Or),
                            args: vec![result, sub_result],
                        },
                        Some(span),
                    );
                }

                result
            }

            MatchPattern::At { name, pattern } => {
                // Test the inner pattern, binding the scrutinee to the name.
                self.scope.bind(*name, scrutinee);
                let inner_pat = self.arena.get_match_pattern(*pattern);
                self.compile_pattern_test(inner_pat, scrutinee, scrut_ty, span)
            }
        }
    }

    /// Compile a range pattern test: `start <= scrutinee && scrutinee <= end`.
    fn compile_range_test(
        &mut self,
        scrutinee: ArcVarId,
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
        span: Span,
    ) -> ArcVarId {
        let start_ok = if let Some(start_id) = start {
            let start_val = self.lower_expr(start_id);
            self.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::GtEq),
                    args: vec![scrutinee, start_val],
                },
                Some(span),
            )
        } else {
            self.builder
                .emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(true)), None)
        };

        let end_ok = if let Some(end_id) = end {
            let end_val = self.lower_expr(end_id);
            let cmp_op = if inclusive {
                ori_ir::BinaryOp::LtEq
            } else {
                ori_ir::BinaryOp::Lt
            };
            self.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(cmp_op),
                    args: vec![scrutinee, end_val],
                },
                Some(span),
            )
        } else {
            self.builder
                .emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(true)), None)
        };

        self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::And),
                args: vec![start_ok, end_ok],
            },
            Some(span),
        )
    }

    // ── bind_match_pattern ─────────────────────────────────────

    /// Extract values from a matched scrutinee into the current scope.
    ///
    /// Called after a pattern test succeeds. Projects fields from the
    /// scrutinee and binds them to pattern names.
    // Field/variant/element indices never exceed u32.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn bind_match_pattern(&mut self, pattern: &MatchPattern, scrutinee: ArcVarId) {
        match pattern {
            // These patterns don't bind anything.
            MatchPattern::Wildcard | MatchPattern::Literal(_) | MatchPattern::Range { .. } => {}

            MatchPattern::Binding(name) => {
                self.scope.bind(*name, scrutinee);
            }

            MatchPattern::Variant { name: _, inner } => {
                let pat_ids: Vec<_> = self.arena.get_match_pattern_list(*inner).to_vec();
                // Project payload field (field 1, after tag).
                for (i, &pat_id) in pat_ids.iter().enumerate() {
                    let inner_pat = self.arena.get_match_pattern(pat_id);
                    let payload = self.builder.emit_project(
                        Idx::UNIT, // Type refined later.
                        scrutinee,
                        (i + 1) as u32,
                        None,
                    );
                    self.bind_match_pattern(inner_pat, payload);
                }
            }

            MatchPattern::Tuple(sub_patterns) => {
                let pat_ids: Vec<_> = self.arena.get_match_pattern_list(*sub_patterns).to_vec();
                for (i, &pat_id) in pat_ids.iter().enumerate() {
                    let inner_pat = self.arena.get_match_pattern(pat_id);
                    let proj = self
                        .builder
                        .emit_project(Idx::UNIT, scrutinee, i as u32, None);
                    self.bind_match_pattern(inner_pat, proj);
                }
            }

            MatchPattern::Struct { fields } => {
                for (i, (field_name, sub_pattern)) in fields.iter().enumerate() {
                    let proj = self
                        .builder
                        .emit_project(Idx::UNIT, scrutinee, i as u32, None);
                    if let Some(pat_id) = sub_pattern {
                        let inner_pat = self.arena.get_match_pattern(*pat_id);
                        self.bind_match_pattern(inner_pat, proj);
                    } else {
                        // Shorthand: field name becomes the binding.
                        self.scope.bind(*field_name, proj);
                    }
                }
            }

            MatchPattern::List { elements, rest } => {
                let pat_ids: Vec<_> = self.arena.get_match_pattern_list(*elements).to_vec();
                for (i, &pat_id) in pat_ids.iter().enumerate() {
                    let inner_pat = self.arena.get_match_pattern(pat_id);
                    let proj = self
                        .builder
                        .emit_project(Idx::UNIT, scrutinee, i as u32, None);
                    self.bind_match_pattern(inner_pat, proj);
                }
                if let Some(rest_name) = rest {
                    self.scope.bind(*rest_name, scrutinee);
                }
            }

            MatchPattern::Or(sub_patterns) => {
                // Or patterns: bind using the first alternative.
                let pat_ids: Vec<_> = self.arena.get_match_pattern_list(*sub_patterns).to_vec();
                if let Some(&first_id) = pat_ids.first() {
                    let first_pat = self.arena.get_match_pattern(first_id);
                    self.bind_match_pattern(first_pat, scrutinee);
                }
            }

            MatchPattern::At { name, pattern } => {
                self.scope.bind(*name, scrutinee);
                let inner_pat = self.arena.get_match_pattern(*pattern);
                self.bind_match_pattern(inner_pat, scrutinee);
            }
        }
    }

    // ── Type helpers ───────────────────────────────────────────

    /// Get the type of a tuple element.
    fn tuple_elem_type(&self, tuple_ty: Idx, index: usize) -> Idx {
        use ori_types::Tag;
        if self.pool.tag(tuple_ty) == Tag::Tuple {
            let count = self.pool.tuple_elem_count(tuple_ty);
            if index < count {
                return self.pool.tuple_elem(tuple_ty, index);
            }
        }
        Idx::UNIT
    }

    /// Get the type of a struct field by name.
    fn struct_field_type(&self, struct_ty: Idx, field: Name, _fallback_index: usize) -> Idx {
        use ori_types::Tag;
        let resolved = self.pool.resolve(struct_ty).unwrap_or(struct_ty);
        if self.pool.tag(resolved) == Tag::Struct {
            let count = self.pool.struct_field_count(resolved);
            for i in 0..count {
                let (fname, fty) = self.pool.struct_field(resolved, i);
                if fname == field {
                    return fty;
                }
            }
        }
        Idx::UNIT
    }

    /// Get the element type of a list.
    fn list_elem_type(&self, list_ty: Idx) -> Idx {
        use ori_types::Tag;
        if self.pool.tag(list_ty) == Tag::List {
            return self.pool.list_elem(list_ty);
        }
        Idx::UNIT
    }

    /// Resolve a variant name to its index in an enum type.
    // Variant indices never exceed u32.
    #[allow(clippy::cast_possible_truncation)]
    fn resolve_variant_index(&self, enum_ty: Idx, variant_name: Name) -> u32 {
        use ori_types::Tag;
        let resolved = self.pool.resolve(enum_ty).unwrap_or(enum_ty);
        if self.pool.tag(resolved) == Tag::Enum {
            let count = self.pool.enum_variant_count(resolved);
            for i in 0..count {
                let (vname, _) = self.pool.enum_variant(resolved, i);
                if vname == variant_name {
                    return i as u32;
                }
            }
        }
        tracing::debug!(?enum_ty, ?variant_name, "could not resolve variant index");
        0
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::ast::{Expr, ExprKind, MatchArm, MatchPattern};
    use ori_ir::{BindingPattern, ExprArena, Name, Span, StringInterner};
    use ori_types::Idx;
    use ori_types::Pool;

    #[test]
    fn bind_name_pattern() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let x_name = Name::from_raw(100);
        let pat = arena.alloc_binding_pattern(BindingPattern::Name(x_name));
        let init = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(10, 12)));

        let let_expr = arena.alloc_expr(Expr::new(
            ExprKind::Let {
                pattern: pat,
                ty: ori_ir::ParsedTypeId::INVALID,
                init,
                mutable: false,
            },
            Span::new(0, 12),
        ));

        // After the let, reference x.
        let x_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(x_name), Span::new(14, 15)));
        let stmts_start = arena.alloc_stmt(ori_ir::Stmt::new(
            ori_ir::StmtKind::Expr(let_expr),
            Span::new(0, 12),
        ));
        #[allow(clippy::cast_possible_truncation)] // Test code: index always fits u32.
        let stmts = arena.alloc_stmt_range(stmts_start.index() as u32, 1);

        let block = arena.alloc_expr(Expr::new(
            ExprKind::Block {
                stmts,
                result: x_ref,
            },
            Span::new(0, 16),
        ));

        let max_id = block.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[init.index()] = Idx::INT;
        expr_types[let_expr.index()] = Idx::UNIT;
        expr_types[x_ref.index()] = Idx::INT;
        expr_types[block.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            block,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty(), "problems: {problems:?}");
        assert!(func.blocks[0].body.len() >= 2);
    }

    #[test]
    fn match_literal_pattern_test() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let scrut = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(6, 7)));
        let lit_42 = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(12, 14)));
        let body1 = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(18, 22)));
        let body2 = arena.alloc_expr(Expr::new(ExprKind::Bool(false), Span::new(28, 33)));

        let arm1 = MatchArm {
            pattern: MatchPattern::Literal(lit_42),
            guard: None,
            body: body1,
            span: Span::new(10, 22),
        };
        let arm2 = MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body2,
            span: Span::new(24, 33),
        };
        let arms = arena.alloc_arms([arm1, arm2]);

        let match_expr = arena.alloc_expr(Expr::new(
            ExprKind::Match {
                scrutinee: scrut,
                arms,
            },
            Span::new(0, 34),
        ));

        let max_id = match_expr.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[scrut.index()] = Idx::INT;
        expr_types[lit_42.index()] = Idx::INT;
        expr_types[body1.index()] = Idx::BOOL;
        expr_types[body2.index()] = Idx::BOOL;
        expr_types[match_expr.index()] = Idx::BOOL;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::BOOL,
            match_expr,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty(), "problems: {problems:?}");
        // Should have multiple blocks for match arms.
        assert!(func.blocks.len() >= 3);
    }
}
