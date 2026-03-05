//! Instruction dispatch for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_instr`] which dispatches each `ArcInstr`
//! variant to the appropriate emission handler, and [`ArcIrEmitter::emit_project`]
//! for field extraction from structs and enums.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcVarId, RcStrategy, ValueRepr};
use ori_types::Idx;

use super::context::{is_boxed_enum_field, EmittedValue};
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a `Project` instruction (field extraction).
    ///
    /// For tagged union payload fields (Result, Enum), the LLVM storage type
    /// may differ from the expected type (e.g., `int` payload stored in a
    /// `{i64, i64, ptr}` slot of `Result<int, str>`). These use alloca + GEP + load
    /// for type-safe extraction through pointer reinterpretation.
    pub(super) fn emit_project(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        func: &ArcFunction,
    ) {
        let val = self.var(value);
        let result_ty = self.resolve_type(ty);

        // For enum/Result payload fields (index > 0), the storage type may
        // differ from the variant's actual type. Use alloca + GEP + load to
        // reinterpret the bytes correctly through pointer casting.
        if field > 0 {
            let val_ty = func.var_type(value);
            let val_type_info = self.type_info.get(val_ty);
            if matches!(
                val_type_info,
                super::super::type_info::TypeInfo::Result { .. }
                    | super::super::type_info::TypeInfo::Enum { .. }
            ) {
                let is_general_enum = matches!(
                    val_type_info,
                    super::super::type_info::TypeInfo::Enum { .. }
                );

                // Fast path: extractvalue chain for general enum scalar fields.
                // Avoids alloca+store+GEP+load (5 instr) with extractvalue (2-3 instr).
                if is_general_enum
                    && !is_boxed_enum_field(self.pool, val_ty, ty)
                    && self.builder.is_struct_value(val)
                    && self.builder.is_single_slot_type(result_ty)
                {
                    let payload = self
                        .builder
                        .extract_value(val, 1, "proj.payload")
                        .expect("enum value should be a struct");
                    let raw = self.builder.extract_value_any(
                        payload,
                        field - 1,
                        &format!("proj.{field}.raw"),
                    );
                    let converted =
                        self.builder
                            .reinterpret_from_i64(raw, result_ty, &format!("proj.{field}"));
                    self.def_var_repr(dst, converted, func);
                    return;
                }

                // Slow path: alloca+store+GEP+load for Result types, boxed fields,
                // multi-word types, and pointer-sourced values.
                let llvm_val_ty = self.resolve_type(val_ty);
                let alloca = self.builder.alloca(llvm_val_ty, "proj.alloca");
                self.builder.store(val, alloca);
                if is_general_enum {
                    let payload_ptr =
                        self.builder
                            .struct_gep(llvm_val_ty, alloca, 1, "proj.payload");
                    let i64_ty = self.builder.i64_type();
                    let slot_idx = self.builder.const_i64(i64::from(field - 1));
                    let slot_ptr = self.builder.gep(
                        i64_ty,
                        payload_ptr,
                        &[slot_idx],
                        &format!("proj.{field}.gep"),
                    );

                    if is_boxed_enum_field(self.pool, val_ty, ty) {
                        // Recursive field: stored as RC pointer in the payload.
                        // Load the pointer, then load the struct from the heap.
                        let ptr_ty = self.builder.ptr_type();
                        let rc_ptr =
                            self.builder
                                .load(ptr_ty, slot_ptr, &format!("proj.{field}.ptr"));
                        let loaded = self
                            .builder
                            .load(result_ty, rc_ptr, &format!("proj.{field}"));
                        self.def_var_repr(dst, loaded, func);
                    } else {
                        let loaded =
                            self.builder
                                .load(result_ty, slot_ptr, &format!("proj.{field}"));
                        self.def_var_repr(dst, loaded, func);
                    }
                } else {
                    // Result: payload is a typed field at struct index 1.
                    let gep = self.builder.struct_gep(
                        llvm_val_ty,
                        alloca,
                        field,
                        &format!("proj.{field}.gep"),
                    );
                    let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
                    self.def_var_repr(dst, loaded, func);
                }
                return;
            }
        }

        if let Some(extracted) = self
            .builder
            .extract_value(val, field, &format!("proj.{field}"))
        {
            self.def_var_repr(dst, extracted, func);
        } else {
            // Fallback: GEP-based field access for heap-allocated types
            let val_ty = func.var_type(value);
            let llvm_val_ty = self.resolve_type(val_ty);
            let gep =
                self.builder
                    .struct_gep(llvm_val_ty, val, field, &format!("proj.{field}.gep"));
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

    /// Emit a single `ArcInstr` as LLVM IR.
    pub(super) fn emit_instr(&mut self, instr: &ArcInstr, func: &ArcFunction) {
        tracing::trace!(?instr, "emit_instr");
        match instr {
            ArcInstr::Let { dst, ty, value } => {
                let val = self.emit_value(value, *ty, func);
                self.def_var_repr(*dst, val, func);
            }

            ArcInstr::Apply {
                dst,
                ty: _,
                func: callee,
                args,
                arg_ownership: _,
            } => self.emit_apply(*dst, *callee, args, func),

            ArcInstr::ApplyIndirect {
                dst,
                ty,
                closure,
                args,
            } => self.emit_apply_indirect(*dst, *ty, *closure, args, func),

            ArcInstr::PartialApply {
                dst,
                ty,
                func: callee,
                args,
            } => self.emit_partial_apply(*dst, *ty, *callee, args, func),

            ArcInstr::Project {
                dst,
                ty,
                value,
                field,
            } => self.emit_project(*dst, *ty, *value, *field, func),

            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
            } => {
                let val = self.emit_construct(*ty, ctor, args);
                self.def_var_repr(*dst, val, func);
            }

            ArcInstr::CollectionReuse {
                old_var,
                dst,
                ty,
                ctor,
                args,
            } => {
                let val = self.emit_collection_reuse(*old_var, *ty, ctor, args);
                self.def_var_repr(*dst, val, func);
            }

            // RC operations — dispatched by strategy (no Pool queries)
            ArcInstr::RcInc {
                var,
                count,
                strategy,
            } => {
                self.emit_rc_inc(*var, *count, *strategy, func);
            }

            ArcInstr::RcDec { var, strategy } => {
                self.emit_rc_dec(*var, *strategy, func);
            }

            ArcInstr::IsShared { dst, var } => {
                // Inline refcount check: data_ptr - 8 = strong_count (i64).
                // Shared when strong_count > 1 (more than one owner).
                //
                // Only valid for RcPointer values (heap-allocated behind an RC
                // header). Aggregates (struct, tuple) and fat values (str) are
                // inline SSA values with no RC header — they are always
                // "shared" (force the slow Construct path).
                let repr = func.var_repr(*var).unwrap_or(ValueRepr::Scalar);
                if repr == ValueRepr::RcPointer {
                    let data_ptr = self.var(*var);
                    let i8_ty = self.builder.i8_type();
                    let neg8 = self.builder.const_i64(-8);
                    let rc_ptr = self.builder.gep(i8_ty, data_ptr, &[neg8], "rc_ptr");
                    let i64_ty = self.builder.i64_type();
                    let rc_val = self.builder.load(i64_ty, rc_ptr, "rc_val");
                    let one = self.builder.const_i64(1);
                    let is_shared = self.builder.icmp_sgt(rc_val, one, "is_shared");
                    self.def_var(*dst, EmittedValue::Immediate(is_shared));
                } else {
                    // Non-pointer value: no RC header to check.
                    // Emit `true` (always shared) to force the slow path
                    // which uses Construct instead of in-place Set.
                    tracing::trace!(
                        var = var.raw(),
                        ?repr,
                        "IsShared on non-pointer value — emitting true"
                    );
                    let always_shared = self.builder.const_bool(true);
                    self.def_var(*dst, EmittedValue::Immediate(always_shared));
                }
            }

            ArcInstr::Reset { var, token } => {
                // Reset marks a value for potential reuse. After expansion by
                // Section 09, this becomes IsShared + conditional.
                // The token IS the variable (reuse its memory if unique).
                let emitted = self.var_emitted(*var);
                self.def_var(*token, emitted);
            }

            ArcInstr::Reuse {
                token,
                dst,
                ty,
                ctor,
                args,
            } => {
                // Defensive fallback: after expand_reuse, Reuse instructions are
                // eliminated — the fast path uses Set/SetTag and the slow path uses
                // Construct. If Reuse appears (e.g., expansion was skipped because
                // Reset/Reuse span different blocks), fall back to: dec the original
                // buffer (held by token) + fresh construction.
                tracing::debug!(
                    "ArcIrEmitter: Reuse instruction not expanded — using Construct fallback"
                );

                // Dec the original buffer held by the token. Without this, the
                // Reset'd buffer leaks (Reset claimed ownership but Reuse didn't
                // reclaim it).
                if let Some(repr) = func.var_repr(*token) {
                    let strategy = RcStrategy::from_var(repr, self.pool, func.var_type(*token));
                    self.emit_rc_dec(*token, strategy, func);
                }

                let val = self.emit_construct(*ty, ctor, args);
                self.def_var_repr(*dst, val, func);
            }

            ArcInstr::Set { base, field, value } => {
                // In-place field update (only valid when uniquely owned).
                // After expand_reuse, this only appears in the fast path for
                // heap-allocated RC'd objects (pointer-typed base).
                let repr = func.var_repr(*base).unwrap_or(ValueRepr::Scalar);
                if repr == ValueRepr::RcPointer {
                    let base_val = self.var(*base);
                    let new_val = self.var(*value);
                    let base_ty = func.var_type(*base);
                    let llvm_ty = self.resolve_type(base_ty);

                    // GEP + store for heap-allocated RC'd objects.
                    // The base is a pointer to the struct data on the heap.
                    let field_ptr = self.builder.struct_gep(
                        llvm_ty,
                        base_val,
                        *field,
                        &format!("set.{field}.ptr"),
                    );
                    self.builder.store(new_val, field_ptr);
                    // base pointer unchanged — mutation is in-place
                } else {
                    // Non-pointer base: this block is unreachable (IsShared
                    // emitted `true` for non-pointer values, so the branch
                    // always takes the slow Construct path). Emit nothing.
                    tracing::trace!(
                        base = base.raw(),
                        field,
                        ?repr,
                        "Set on non-pointer value — skipping (unreachable)"
                    );
                }
            }

            ArcInstr::SetTag { base, tag } => {
                // In-place tag update for enum variants.
                // Tag is field 0 of the enum representation: { i8 tag, ... }
                let base_val = self.var(*base);
                let base_ty = func.var_type(*base);
                let llvm_ty = self.resolve_type(base_ty);

                let tag_ptr = self.builder.struct_gep(llvm_ty, base_val, 0, "set.tag.ptr");
                let tag_val = self.builder.const_i64(*tag as i64);
                self.builder.store(tag_val, tag_ptr);
                // base pointer unchanged — mutation is in-place
            }

            ArcInstr::Select {
                dst,
                cond,
                true_val,
                false_val,
                ..
            } => {
                let c = self.var(*cond);
                let t = self.var(*true_val);
                let f = self.var(*false_val);
                let result = self.builder.select(c, t, f, "sel");
                self.def_var(*dst, EmittedValue::Immediate(result));
            }
        }
    }
}
