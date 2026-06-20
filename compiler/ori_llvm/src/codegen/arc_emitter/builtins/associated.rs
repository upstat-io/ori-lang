//! Builtin associated-function codegen for the ARC emitter.
//!
//! Associated functions (factory / trait static methods called as
//! `Type.method()`) carry NO receiver, so the ARC `Apply`/`Invoke` lowers to a
//! bare callee `Name` (`default`, not `Duration.default`) and the receiver-keyed
//! [`super::try_emit_builtin_method`] cannot dispatch — it returns `None` on the
//! empty-args guard. The owning type is recovered from the call's RETURN type
//! (`dst_ty`), mirroring [`super::super::resolve_callee`]'s
//! `lookup_method_by_return_type` seam for synthesized associated methods.
//!
//! Eval parity: this is the LLVM analogue of `ori_eval`'s
//! `dispatch_associated_function`, which keys the same dispatch on the type name
//! at the `Type.method()` call site.

use ori_ir::Name;
use ori_registry::{MethodKind, ReturnTag, TypeTag};
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Try to emit inline IR for a no-receiver builtin associated function.
    ///
    /// Returns `Some(value)` when `callee` is an associated function the
    /// registry declares on the builtin type whose [`Idx`] is the call's return
    /// type `dst_ty`. Returns `None` otherwise so the caller falls through to
    /// the remaining resolution chain.
    pub(in crate::codegen::arc_emitter) fn try_emit_builtin_associated(
        &mut self,
        callee: Name,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = self.interner.lookup(callee);
        let resolved = self.pool.resolve_fully(dst_ty);
        let type_tag = ori_types_tag_to_registry(self.pool.tag(resolved))?;
        let method = ori_registry::find_method(type_tag, method_name)?;
        if method.kind != MethodKind::Associated {
            return None;
        }

        // `() -> Self` constructors with no parameters reduce to the type's
        // zero/default value. The `Default::default` trait method (registered as
        // an associated function on Duration/Size) is the canonical instance;
        // `zero` shares the shape. A `Self`-returning, no-param associated fn is
        // a value constructor whose result is the type's zero representation.
        if method.params.is_empty() && method.returns == ReturnTag::SelfType {
            let llvm_ty = self.resolve_type(resolved);
            return Some(self.builder.const_zero_ty(llvm_ty));
        }

        None
    }
}

/// Map an `ori_types::Tag` to the `ori_registry::TypeTag` used for registry
/// lookups. Returns `None` for non-builtin or unmapped tags.
fn ori_types_tag_to_registry(tag: ori_types::Tag) -> Option<TypeTag> {
    use ori_types::Tag;
    Some(match tag {
        Tag::Int => TypeTag::Int,
        Tag::Float => TypeTag::Float,
        Tag::Bool => TypeTag::Bool,
        Tag::Str => TypeTag::Str,
        Tag::Char => TypeTag::Char,
        Tag::Byte => TypeTag::Byte,
        Tag::Duration => TypeTag::Duration,
        Tag::Size => TypeTag::Size,
        Tag::Ordering => TypeTag::Ordering,
        _ => return None,
    })
}
