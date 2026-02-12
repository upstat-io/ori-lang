//! Pattern lowering — binding destructuring for `let` expressions.
//!
//! - [`bind_pattern`] — destructure a `CanBindingPattern` into scope bindings.
//!
//! Match pattern compilation is handled by the decision tree pipeline
//! (`decision_tree::flatten` → `decision_tree::compile` → `decision_tree::emit`).

use ori_ir::canon::{CanBindingPattern, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::ArcVarId;

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // bind_pattern (for let)

    /// Bind a `CanBindingPattern` to an ARC IR value.
    ///
    /// Recursively destructures tuples, structs, and lists, adding
    /// `Project` instructions for each field and binding names in the scope.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field/variant/element indices never exceed u32"
    )]
    pub(crate) fn bind_pattern(
        &mut self,
        pattern: &CanBindingPattern,
        value: ArcVarId,
        mutable: bool,
        init_id: CanId,
    ) {
        match pattern {
            CanBindingPattern::Name(name) => {
                if mutable {
                    self.scope.bind_mutable(*name, value);
                } else {
                    self.scope.bind(*name, value);
                }
            }

            CanBindingPattern::Wildcard => {
                // Discard — no binding.
            }

            CanBindingPattern::Tuple(elements) => {
                let init_ty = self.expr_type(init_id);
                let elem_ids: Vec<_> = self.arena.get_binding_pattern_list(*elements).to_vec();
                for (i, &sub_pat_id) in elem_ids.iter().enumerate() {
                    let sub_pattern = self.arena.get_binding_pattern(sub_pat_id);
                    let elem_ty = self.tuple_elem_type(init_ty, i);
                    let proj = self.builder.emit_project(elem_ty, value, i as u32, None);
                    self.bind_pattern(sub_pattern, proj, mutable, init_id);
                }
            }

            CanBindingPattern::Struct { fields } => {
                let init_ty = self.expr_type(init_id);
                let field_bindings: Vec<_> = self.arena.get_field_bindings(*fields).to_vec();
                for (i, fb) in field_bindings.iter().enumerate() {
                    let field_ty = self.struct_field_type(init_ty, fb.name, i);
                    let proj = self.builder.emit_project(field_ty, value, i as u32, None);
                    let sub_pattern = self.arena.get_binding_pattern(fb.pattern);
                    // If the sub-pattern is just a Name matching the field name,
                    // bind it directly. Otherwise recurse.
                    self.bind_pattern(sub_pattern, proj, mutable, init_id);
                }
            }

            CanBindingPattern::List { elements, rest } => {
                let init_ty = self.expr_type(init_id);
                let elem_ty = self.list_elem_type(init_ty);
                let elem_ids: Vec<_> = self.arena.get_binding_pattern_list(*elements).to_vec();
                for (i, &sub_pat_id) in elem_ids.iter().enumerate() {
                    let sub_pattern = self.arena.get_binding_pattern(sub_pat_id);
                    let proj = self.builder.emit_project(elem_ty, value, i as u32, None);
                    self.bind_pattern(sub_pattern, proj, mutable, init_id);
                }
                if let Some(rest_name) = rest {
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

    // Type helpers

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

// Tests

#[cfg(test)]
mod tests {
    use ori_ir::canon::{CanArena, CanBindingPattern, CanExpr, CanNode, CanonResult};
    use ori_ir::{Name, Span, StringInterner, TypeId};
    use ori_types::Idx;
    use ori_types::Pool;

    #[test]
    fn bind_name_pattern() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = CanArena::with_capacity(200);

        let x_name = Name::from_raw(100);
        let pat = arena.push_binding_pattern(CanBindingPattern::Name(x_name));
        let init = arena.push(CanNode::new(
            CanExpr::Int(42),
            Span::new(10, 12),
            TypeId::from_raw(Idx::INT.raw()),
        ));

        let let_expr = arena.push(CanNode::new(
            CanExpr::Let {
                pattern: pat,
                init,
                mutable: false,
            },
            Span::new(0, 12),
            TypeId::from_raw(Idx::UNIT.raw()),
        ));

        let x_ref = arena.push(CanNode::new(
            CanExpr::Ident(x_name),
            Span::new(14, 15),
            TypeId::from_raw(Idx::INT.raw()),
        ));
        let stmts = arena.push_expr_list(&[let_expr]);
        let block = arena.push(CanNode::new(
            CanExpr::Block {
                stmts,
                result: x_ref,
            },
            Span::new(0, 16),
            TypeId::from_raw(Idx::INT.raw()),
        ));

        let canon = CanonResult {
            arena,
            constants: ori_ir::canon::ConstantPool::new(),
            decision_trees: ori_ir::canon::DecisionTreePool::default(),
            root: block,
            roots: vec![],
            method_roots: vec![],
            problems: vec![],
        };

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function_can(
            Name::from_raw(1),
            &[],
            Idx::INT,
            block,
            &canon,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty(), "problems: {problems:?}");
        assert!(func.blocks[0].body.len() >= 2);
    }
}
