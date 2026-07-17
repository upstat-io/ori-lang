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

use ori_arc::ir::ArcVarId;
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
        args: &[ArcVarId],
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

        // Parametrized `(val: int) -> Self` unit factories
        // (`Duration.from_seconds`, `Size.from_kilobytes`, ...) scale the int
        // operand by the unit's base-unit factor. Duration/Size are both stored
        // as a bare `i64` (nanoseconds / bytes), so the result is the scaled
        // operand. Mirrors `ori_eval`'s `duration_from_int` / `size_from_int`
        // (`ori_eval/src/methods/units.rs`): checked multiply (panic on
        // overflow); Size rejects a negative operand.
        if method.params.len() == 1 && method.returns == ReturnTag::SelfType {
            if let Some(factor) = unit_factory_factor(type_tag, method_name) {
                let val = self.var(*args.first()?);
                if type_tag == TypeTag::Size {
                    let zero = self.builder.const_i64(0);
                    let non_neg = self.builder.icmp_sge(val, zero, "size_factory.nonneg");
                    self.emit_unwrap_branch(non_neg, "Size cannot be negative", "size_factory")?;
                }
                if factor == 1 {
                    return Some(val);
                }
                let factor_val = self.builder.const_i64(factor);
                // Factory-specific overflow message matches `ori_eval`'s
                // `integer_overflow("<duration|size> factory conversion")` for
                // dual-execution parity.
                let overflow_msg = match type_tag {
                    TypeTag::Size => "integer overflow in size factory conversion",
                    _ => "integer overflow in duration factory conversion",
                };
                return Some(self.builder.checked_mul_msg(
                    val,
                    factor_val,
                    "unit_factory",
                    overflow_msg,
                ));
            }
        }

        None
    }
}

/// Base-unit scale factor for a Duration/Size factory associated function,
/// mirroring `ori_eval`'s `duration_from_int` / `size_from_int` multipliers
/// (`ori_eval/src/methods/units.rs`). Returns `None` when `method` is not a
/// scaled factory on `type_tag`.
fn unit_factory_factor(type_tag: TypeTag, method: &str) -> Option<i64> {
    use ori_ir::builtin_constants::{duration, size};
    #[expect(
        clippy::match_same_arms,
        reason = "base-unit arms (factor 1) kept per-type for Duration/Size grouping clarity"
    )]
    Some(match (type_tag, method) {
        (TypeTag::Duration, "from_nanoseconds" | "from_nanos") => 1,
        (TypeTag::Duration, "from_microseconds" | "from_micros") => duration::NS_PER_US,
        (TypeTag::Duration, "from_milliseconds" | "from_millis") => duration::NS_PER_MS,
        (TypeTag::Duration, "from_seconds") => duration::NS_PER_S,
        (TypeTag::Duration, "from_minutes") => duration::NS_PER_M,
        (TypeTag::Duration, "from_hours") => duration::NS_PER_H,
        (TypeTag::Size, "from_bytes") => 1,
        (TypeTag::Size, "from_kilobytes" | "from_kb") => size::BYTES_PER_KB as i64,
        (TypeTag::Size, "from_megabytes" | "from_mb") => size::BYTES_PER_MB as i64,
        (TypeTag::Size, "from_gigabytes" | "from_gb") => size::BYTES_PER_GB as i64,
        (TypeTag::Size, "from_terabytes" | "from_tb") => size::BYTES_PER_TB as i64,
        _ => return None,
    })
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
        Tag::Unit => TypeTag::Unit,
        Tag::Duration => TypeTag::Duration,
        Tag::Size => TypeTag::Size,
        Tag::Ordering => TypeTag::Ordering,
        _ => return None,
    })
}
