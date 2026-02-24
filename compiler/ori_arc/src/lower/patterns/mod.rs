//! Pattern lowering — binding destructuring for `let` and `for` expressions.
//!
//! - [`bind_pattern`] — destructure a `CanBindingPattern` into scope bindings (for `let`).
//! - [`bind_for_pattern`] — bind a for-loop pattern (always immutable).
//!
//! Match pattern compilation is handled by the decision tree pipeline
//! (`decision_tree::flatten` → `decision_tree::compile` → `decision_tree::emit`).

use ori_ir::canon::{CanBindingPattern, CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::ArcVarId;

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // Pattern binding (shared infrastructure)

    /// Bind a `CanBindingPattern` to an ARC IR value (for `let` bindings).
    ///
    /// Respects per-binding mutability flags from the pattern, supporting
    /// mixed mutability like `let ($x, y) = ...`.
    pub(crate) fn bind_pattern(
        &mut self,
        pattern: &CanBindingPattern,
        value: ArcVarId,
        init_id: CanId,
    ) {
        let ty = self.expr_type(init_id);
        self.bind_pattern_inner(pattern, value, ty, false);
    }

    /// Bind a for-loop iteration pattern — always immutable.
    ///
    /// For-loop iteration variables are rebound each iteration by the loop
    /// infrastructure, not by user assignment. Using `scope.bind()` (not
    /// `bind_mutable()`) prevents them from appearing in `mutable_bindings()`
    /// and generating incorrect SSA phi nodes at subsequent loop headers.
    pub(crate) fn bind_for_pattern(
        &mut self,
        pattern: CanBindingPatternId,
        value: ArcVarId,
        elem_ty: Idx,
    ) {
        let pat = *self.arena.get_binding_pattern(pattern);
        self.bind_pattern_inner(&pat, value, elem_ty, true);
    }

    /// Shared recursive pattern binding.
    ///
    /// When `force_immutable` is true, all name bindings use `scope.bind()`
    /// regardless of the pattern's mutability flag. This is used for for-loop
    /// variables which are semantically rebound each iteration.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field/variant/element indices never exceed u32"
    )]
    fn bind_pattern_inner(
        &mut self,
        pattern: &CanBindingPattern,
        value: ArcVarId,
        ty: Idx,
        force_immutable: bool,
    ) {
        match pattern {
            CanBindingPattern::Name {
                name,
                mutable: pat_mutable,
            } => {
                let is_mut = !force_immutable && pat_mutable.is_mutable();
                tracing::trace!(
                    name = self.name_str(*name),
                    var = value.raw(),
                    mutable = is_mut,
                    "pattern: bind name"
                );
                if is_mut {
                    // Record this name as let-bound in the current block so
                    // lower_block's propagation can distinguish shadows from
                    // reassignments. Reassignments go through lower_assign
                    // (which does NOT add to block_let_names).
                    self.block_let_names.insert(*name);
                    self.scope.bind_mutable(*name, value);
                } else {
                    self.scope.bind(*name, value);
                }
            }

            CanBindingPattern::Wildcard => {}

            CanBindingPattern::Tuple(elements) => {
                let elem_ids: Vec<_> = self.arena.get_binding_pattern_list(*elements).to_vec();
                for (i, &sub_pat_id) in elem_ids.iter().enumerate() {
                    let sub_pattern = self.arena.get_binding_pattern(sub_pat_id);
                    let elem_ty = self.tuple_elem_type(ty, i);
                    let proj = self.builder.emit_project(elem_ty, value, i as u32, None);
                    self.bind_pattern_inner(sub_pattern, proj, elem_ty, force_immutable);
                }
            }

            CanBindingPattern::Struct { fields } => {
                let field_bindings: Vec<_> = self.arena.get_field_bindings(*fields).to_vec();
                for (i, fb) in field_bindings.iter().enumerate() {
                    let field_ty = self.struct_field_type(ty, fb.name, i);
                    let proj = self.builder.emit_project(field_ty, value, i as u32, None);
                    let sub_pattern = self.arena.get_binding_pattern(fb.pattern);
                    self.bind_pattern_inner(sub_pattern, proj, field_ty, force_immutable);
                }
            }

            CanBindingPattern::List { elements, rest } => {
                let elem_ty_inner = self.list_elem_type(ty);
                let elem_ids: Vec<_> = self.arena.get_binding_pattern_list(*elements).to_vec();
                for (i, &sub_pat_id) in elem_ids.iter().enumerate() {
                    let sub_pattern = self.arena.get_binding_pattern(sub_pat_id);
                    let proj = self
                        .builder
                        .emit_project(elem_ty_inner, value, i as u32, None);
                    self.bind_pattern_inner(sub_pattern, proj, elem_ty_inner, force_immutable);
                }
                if let Some((rest_name, rest_mut)) = rest {
                    let rest_is_mut = !force_immutable && rest_mut.is_mutable();
                    if rest_is_mut {
                        self.block_let_names.insert(*rest_name);
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
        let tuple_ty = self.pool.resolve_fully(tuple_ty);
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
        let resolved = self.pool.resolve_fully(struct_ty);
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
        let list_ty = self.pool.resolve_fully(list_ty);
        if self.pool.tag(list_ty) == Tag::List {
            return self.pool.list_elem(list_ty);
        }
        Idx::UNIT
    }
}

// Tests

#[cfg(test)]
mod tests;
