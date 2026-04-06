//! Instruction dispatch for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_instr`] which dispatches each `ArcInstr`
//! variant to the appropriate emission handler, and [`ArcIrEmitter::emit_project`]
//! for field extraction from structs and enums.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcVarId, RcStrategy, ValueRepr};
use ori_types::{Idx, Tag};

use super::context::{is_boxed_enum_field, EmittedValue};
use super::drop_enum::compute_variant_field_offsets;
use super::ArcIrEmitter;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

/// Compute the byte offset for a given payload field in an enum variant.
///
/// Searches all variants of the enum to find one where the field at
/// `payload_field_idx` (0-based) has type matching `field_type`. Returns
/// the byte offset within the `[M x i64]` payload area.
///
/// Falls back to `payload_field_idx * 8` if no matching variant is found
/// (single-slot fields at sequential positions — the legacy behavior).
fn enum_payload_byte_offset(
    emitter: &ArcIrEmitter<'_, '_, '_, '_>,
    enum_ty: Idx,
    payload_field_idx: u32,
    field_type: Idx,
) -> u64 {
    let resolved = emitter.pool.resolve_fully(enum_ty);
    if emitter.pool.tag(resolved) != Tag::Enum {
        return u64::from(payload_field_idx) * 8;
    }
    let variants = emitter.pool.enum_variants(resolved);
    let fi = payload_field_idx as usize;
    let resolved_ft = emitter.pool.resolve_fully(field_type);

    for (_, fields) in &variants {
        if fi < fields.len() && emitter.pool.resolve_fully(fields[fi]) == resolved_ft {
            let offsets = compute_variant_field_offsets(fields, resolved, emitter);
            return offsets.get(fi).copied().unwrap_or(0);
        }
    }
    // Fallback: assume single-slot fields
    u64::from(payload_field_idx) * 8
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Handle `Project` on a decomposed `__iter_next` result.
    ///
    /// Field 0 returns the tag (already an `i64` in the `var_map`).
    /// Field 1 loads the element from the scratch buffer and registers the
    /// scratch pointer in `borrowed_param_ptrs` for downstream forwarding.
    fn emit_project_iter_next(
        &mut self,
        dst: ArcVarId,
        field: u32,
        tag: ValueId,
        scratch_ptr: ValueId,
        elem_llvm_ty: LLVMTypeId,
        func: &ArcFunction,
    ) {
        if field == 0 {
            self.def_var_repr(dst, tag, func);
        } else {
            // Load element from scratch buffer (deferred from emit_iter_next).
            // Note: tuple reordering is currently disabled (only structs are
            // reordered). If tuple reordering is enabled in the future,
            // map iterator elements need special handling because the runtime
            // writes (key, value) in declaration order.
            let elem = self
                .builder
                .load(elem_llvm_ty, scratch_ptr, &format!("proj.{field}"));
            // Sign-extend narrowed int element back to canonical i64.
            let dst_ty = func.var_type(dst);
            let elem = self.sext_narrowed_int_element(elem, dst_ty, "iter_next.sext");
            self.def_var_repr(dst, elem, func);
            // Register scratch pointer for borrowed-parameter forwarding:
            // downstream calls (e.g., ori_str_len) can forward the scratch
            // pointer directly instead of alloca+store round-trip.
            self.borrowed_param_ptrs.insert(dst, scratch_ptr);
        }
    }

    /// Try to emit a `Project` for an enum/Result payload field.
    ///
    /// Returns `true` if the field was an enum/Result payload and was handled,
    /// `false` if it should go through the normal `extractvalue` path.
    #[expect(clippy::too_many_arguments, reason = "extracted from emit_project")]
    fn try_emit_project_enum_payload(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        val: ValueId,
        result_ty: LLVMTypeId,
        func: &ArcFunction,
    ) -> bool {
        let val_ty = func.var_type(value);
        let val_type_info = self.type_info.get(val_ty);
        if !matches!(
            val_type_info,
            super::super::type_info::TypeInfo::Result { .. }
                | super::super::type_info::TypeInfo::Enum { .. }
        ) {
            return false;
        }

        // BUG-04-008: If the projected field is Unit/Never, it's zero-sized
        // and doesn't exist in the LLVM payload. Return a zero constant.
        let resolved_field_ty = self.pool.resolve_fully(ty);
        if matches!(self.pool.tag(resolved_field_ty), Tag::Unit | Tag::Never) {
            let zero = self.builder.const_zero_ty(result_ty);
            self.def_var_repr(dst, zero, func);
            return true;
        }

        let is_general_enum = matches!(
            val_type_info,
            super::super::type_info::TypeInfo::Enum { .. }
        );

        // Compute byte offset for this field within the payload.
        let byte_offset = if is_general_enum {
            enum_payload_byte_offset(self, val_ty, field - 1, ty)
        } else {
            0
        };

        // Fast path: extractvalue chain for general enum scalar fields.
        let slot_index = byte_offset / 8;
        if is_general_enum
            && !is_boxed_enum_field(self.pool, val_ty, ty)
            && self.builder.is_struct_value(val)
            && self.builder.is_single_slot_type(result_ty)
            && byte_offset % 8 == 0
        {
            let payload = self
                .builder
                .extract_value(val, 1, "proj.payload")
                .expect("enum value should be a struct");
            #[expect(clippy::cast_possible_truncation, reason = "slot index fits u32")]
            let raw = self.builder.extract_value_any(
                payload,
                slot_index as u32,
                &format!("proj.{field}.raw"),
            );
            let converted =
                self.builder
                    .reinterpret_from_i64(raw, result_ty, &format!("proj.{field}"));
            self.def_var_repr(dst, converted, func);
            return true;
        }

        // Slow path: alloca+store+GEP+load for Result types, boxed fields,
        // multi-word types, and pointer-sourced values.
        let llvm_val_ty = self.resolve_type(val_ty);
        let alloca = self.builder.alloca(llvm_val_ty, "proj.alloca");
        self.builder.store(val, alloca);
        if is_general_enum {
            let payload_ptr = self
                .builder
                .struct_gep(llvm_val_ty, alloca, 1, "proj.payload");
            let i8_ty = self.builder.i8_type();
            let offset_val = self.builder.const_i64(byte_offset as i64);
            let slot_ptr = self.builder.gep(
                i8_ty,
                payload_ptr,
                &[offset_val],
                &format!("proj.{field}.gep"),
            );

            if is_boxed_enum_field(self.pool, val_ty, ty) {
                let ptr_ty = self.builder.ptr_type();
                let rc_ptr = self
                    .builder
                    .load(ptr_ty, slot_ptr, &format!("proj.{field}.ptr"));
                let loaded = self
                    .builder
                    .load(result_ty, rc_ptr, &format!("proj.{field}"));
                self.def_var_repr(dst, loaded, func);
            } else {
                let loaded = self
                    .builder
                    .load(result_ty, slot_ptr, &format!("proj.{field}"));
                self.def_var_repr(dst, loaded, func);
            }
        } else {
            // Result/Option: payload is a typed field.
            // §07.2: niche layout has no tag field → payload at index 0.
            let struct_idx = if self.get_niche_encoding(val_ty).is_some() {
                field - 1 // niche: no tag field
            } else {
                field // explicit: tag at 0, payload at 1+
            };
            let gep = self.builder.struct_gep(
                llvm_val_ty,
                alloca,
                struct_idx,
                &format!("proj.{field}.gep"),
            );
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
        true
    }

    /// Emit a `Project` instruction (field extraction).
    ///
    /// For tagged union payload fields (Result, Enum), delegates to
    /// [`try_emit_project_enum_payload`](Self::try_emit_project_enum_payload).
    /// For decomposed `__iter_next` results, delegates to
    /// [`emit_project_iter_next`](Self::emit_project_iter_next).
    pub(super) fn emit_project(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        func: &ArcFunction,
    ) {
        // Fast path: decomposed iter_next result — extract tag or element
        // directly without going through the {i64, T} wrapper struct.
        if let Some(&(tag, scratch_ptr, elem_llvm_ty)) = self.iter_next_decomposed.get(&value) {
            self.emit_project_iter_next(dst, field, tag, scratch_ptr, elem_llvm_ty, func);
            return;
        }

        // §07.3.A: Tagged-pointer enum projection.
        // The entire enum is a single 64-bit slot encoded as `(ptr | tag)`.
        // Field 0 decodes the tag (low 3 bits) — this becomes the switch
        // scrutinee directly, no `tagged_ptr_scrutinees` map needed because
        // the decoded i64 tag works with the standard `Switch` path.
        // Field > 0 decodes the payload pointer (high 61 bits) — every
        // variant carries at most one pointer field, so the field index
        // beyond 0 always means "the payload pointer".
        let val_ty = func.var_type(value);
        if self.get_tagged_ptr_encoding(val_ty).is_some() {
            let v = self.var(value);
            if field == 0 {
                let tag = self.tagged_ptr_decode_tag(v, "tagged.tag");
                self.def_var(dst, super::EmittedValue::Immediate(tag));
            } else {
                let ptr = self.tagged_ptr_decode_ptr(v, "tagged.ptr");
                self.def_var_repr(dst, ptr, func);
            }
            return;
        }

        // §07.2: Niche-encoded enum tag extraction.
        // When Project { field: 0 } targets a niche-encoded enum, extract the
        // niche field value (not a logical variant index). The raw niche field
        // value is recorded in `niche_scrutinees` so Switch can emit the
        // correct comparison.
        if field == 0 {
            if let Some(encoding) = self.get_niche_encoding(val_ty) {
                if encoding.is_tagless() {
                    // Single-variant: tag is always 0.
                    let zero = self.builder.const_i64(0);
                    self.def_var_repr(dst, zero, func);
                    return;
                }
                // Niche: extract the niche field from the struct.
                let niche_idx = encoding.niche_field_index().unwrap();
                let v = self.var(value);
                let llvm_ty = self.resolve_type(val_ty);
                let niche_val = if let Some(extracted) =
                    self.builder.extract_value(v, niche_idx, "niche.field")
                {
                    extracted
                } else {
                    // Pointer-based access: GEP + load.
                    let field_ty = self
                        .builder
                        .struct_field_type(llvm_ty, niche_idx)
                        .unwrap_or_else(|| self.builder.i64_type());
                    let gep = self
                        .builder
                        .struct_gep(llvm_ty, v, niche_idx, "niche.field.gep");
                    self.builder.load(field_ty, gep, "niche.field")
                };
                self.niche_scrutinees.insert(dst, val_ty);
                self.def_var(dst, super::EmittedValue::Immediate(niche_val));
                return;
            }
        }

        let val = self.var(value);
        let result_ty = self.resolve_type(ty);

        // For enum/Result payload fields (index > 0), the storage type may
        // differ from the variant's actual type. Use alloca + GEP + load to
        // reinterpret the bytes correctly through pointer casting.
        if field > 0
            && self.try_emit_project_enum_payload(dst, ty, value, field, val, result_ty, func)
        {
            return;
        }

        // Remap declaration-order field index to memory-order for LLVM.
        let val_ty = func.var_type(value);
        let mem_field = self.remap_struct_field(val_ty, field);

        if let Some(extracted) =
            self.builder
                .extract_value(val, mem_field, &format!("proj.{field}"))
        {
            // Sign-extend narrowed int fields (i8/i16/i32) back to
            // canonical width (i64) for computation. Only applies when the
            // ARC IR destination expects i64 (Tag::Int) but the struct field
            // is narrower due to integer narrowing.
            let dst_ty = func.var_type(dst);
            let widened = self.sext_narrowed_field(extracted, field, dst_ty);
            self.def_var_repr(dst, widened, func);
        } else {
            // Fallback: GEP-based field access for heap-allocated types
            let llvm_val_ty = self.resolve_type(val_ty);
            let gep =
                self.builder
                    .struct_gep(llvm_val_ty, val, mem_field, &format!("proj.{field}.gep"));
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

    /// Emit a single `ArcInstr` as LLVM IR.
    #[expect(
        clippy::too_many_lines,
        reason = "ARC instruction dispatch over all ArcInstr variants"
    )]
    pub(super) fn emit_instr(&mut self, instr: &ArcInstr, func: &ArcFunction) {
        tracing::trace!(?instr, "emit_instr");
        match instr {
            ArcInstr::Let { dst, ty, value } => {
                let val = if let Some(&elem_ty) = self.for_yield_elem_size_types.get(dst) {
                    // Override ARC-emitted pool_type_store_size with the LLVM
                    // struct store size. Required for reordered structs/tuples
                    // where the ARC size (original layout) differs from the
                    // LLVM size (reordered layout).
                    let llvm_size = self.element_store_size(elem_ty);
                    self.builder.const_i64(llvm_size as i64)
                } else {
                    self.emit_value(value, *ty, func)
                };
                // Only narrow computation results (PrimOps),
                // not copies (Var) or literals (Literal). Narrowing copies
                // or literals creates new SSA values that break CSE cache
                // coherence — two `x + 1` expressions using different
                // sext'd copies of `x` or `1` won't match in the cache.
                let should_narrow = matches!(value, ori_arc::ir::ArcValue::PrimOp { .. });
                if should_narrow {
                    self.def_var_repr(*dst, val, func);
                } else {
                    let repr = func
                        .var_repr(*dst)
                        .unwrap_or(ori_arc::ir::ValueRepr::Scalar);
                    self.def_var(*dst, super::EmittedValue::from_repr(repr, val));
                }
                // Propagate borrowed parameter source pointers through aliases.
                // When `Let { dst, Var(src) }`, if src has a known source pointer
                // (from a borrowed param), dst inherits it for pointer forwarding.
                if let ori_arc::ir::ArcValue::Var(src) = value {
                    if let Some(&ptr) = self.borrowed_param_ptrs.get(src) {
                        self.borrowed_param_ptrs.insert(*dst, ptr);
                    }
                }
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
                arg_ownership: _,
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

                    // §07.3.A: Tagged-pointer enums have no struct layout —
                    // there are no individual fields to GEP into. The entire
                    // enum is a single i64 slot. AIMS reuse should never
                    // generate a `Set` for a tagged-pointer enum because the
                    // encoding is monolithic. If this ever fires, the AIMS
                    // pipeline needs to be taught about tagged-pointer
                    // encoding before in-place mutation can be supported.
                    debug_assert!(
                        self.get_tagged_ptr_encoding(base_ty).is_none(),
                        "Set on tagged-pointer enum — AIMS reuse must produce \
                         a full Construct, not a per-field Set, for tagged-ptr \
                         encoded enums (no individual field layout exists)"
                    );

                    // Remap declaration-order field to memory-order.
                    let mem_field = self.remap_struct_field(base_ty, *field);

                    // GEP + store for heap-allocated RC'd objects.
                    // The base is a pointer to the struct data on the heap.
                    let field_ptr = self.builder.struct_gep(
                        llvm_ty,
                        base_val,
                        mem_field,
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
                let base_val = self.var(*base);
                let base_ty = func.var_type(*base);
                let llvm_ty = self.resolve_type(base_ty);

                // §07.3.A: Tagged-pointer enum — re-encode the tag bits.
                // The encoded value lives in a single i64 slot. To set the
                // discriminant we mask off the existing low 3 bits and OR
                // in the new variant index. The pointer payload (high 61
                // bits) is preserved — useful for variant→variant
                // transitions where the same payload pointer applies (e.g.,
                // marking a node).
                if self.get_tagged_ptr_encoding(base_ty).is_some() {
                    let cleared_ptr = self.tagged_ptr_decode_ptr(base_val, "set_tag.ptr");
                    let updated = self.tagged_ptr_encode(cleared_ptr, *tag as u32, "set_tag");
                    self.def_var(*base, super::EmittedValue::Immediate(updated));
                    return;
                }

                // §07.2: Niche/tagless encoding — conditional tag store.
                if let Some(encoding) = self.get_niche_encoding(base_ty) {
                    if encoding.is_tagless() {
                        // Single-variant enum: no tag to store.
                    } else if encoding.needs_tag_store(*tag as u32) {
                        // Niche variant: write niche_value into the niche field.
                        let niche_idx = encoding.niche_field_index().unwrap();
                        let niche_value = encoding.variant_to_tag_value(*tag as u32);
                        if self.builder.is_struct_value(base_val) {
                            // Register value: use insert_value + re-bind variable.
                            let niche_const = self.builder.const_int_for_struct_field(
                                llvm_ty,
                                niche_idx,
                                niche_value,
                            );
                            let updated = self.builder.insert_value(
                                base_val,
                                niche_const,
                                niche_idx,
                                "set.niche",
                            );
                            self.def_var(*base, super::EmittedValue::Aggregate(updated));
                        } else {
                            // Pointer: GEP + store (in-place mutation).
                            let field_ptr = self.builder.struct_gep(
                                llvm_ty,
                                base_val,
                                niche_idx,
                                "set.niche.ptr",
                            );
                            let field_val = self.builder.const_int_for_struct_field(
                                llvm_ty,
                                niche_idx,
                                niche_value,
                            );
                            self.builder.store(field_val, field_ptr);
                        }
                    }
                    // Non-niche variant: no-op — payload implicitly identifies variant.
                } else {
                    // Explicit tag: field 0 of { narrowed_tag, ... }
                    let tag_ptr = self.builder.struct_gep(llvm_ty, base_val, 0, "set.tag.ptr");
                    let tag_val = self.builder.const_int_for_struct_field(llvm_ty, 0, *tag);
                    self.builder.store(tag_val, tag_ptr);
                }
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
                // Select is a computation (branchless conditional),
                // route through def_var_repr() for local narrowing.
                self.def_var_repr(*dst, result, func);
            }
        }
    }
}
