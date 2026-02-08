//! Pattern lowering — binding destructuring for `let` expressions.
//!
//! - [`bind_pattern`] — destructure a `BindingPattern` into scope bindings.
//!
//! Match pattern compilation is handled by the decision tree pipeline
//! (`decision_tree::flatten` → `decision_tree::compile` → `decision_tree::emit`).

use ori_ir::{BindingPattern, ExprId, Name};
use ori_types::Idx;

use crate::ir::ArcVarId;

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
