//! Variable, block, and type utility methods for [`ArcIrEmitter`].
//!
//! Provides the core infrastructure that all emission submodules depend on:
//!
//! - **Variable mapping**: `var()`, `var_emitted()`, `def_var()`, `def_var_repr()`
//! - **Block mapping**: `block()`
//! - **Type resolution**: `resolve_type()`, `element_store_size()`, `element_store_align()`
//! - **COW annotations**: `cow_mode_const()`, `mark_cow_data_noalias_if_unique()`
//! - **RC allocation**: `rc_alloc()`

use ori_arc::ir::{ArcFunction, ArcVarId, ValueRepr};
use ori_arc::CowMode;
use ori_types::Idx;

use super::context::EmittedValue;
use super::ArcIrEmitter;
use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Get the current instruction's COW mode as an LLVM `i32` constant.
    ///
    /// Queries the `ArcFunction`'s `cow_annotations` for the current
    /// `(block_idx, instr_idx)` coordinate. Returns `Dynamic` (0) when
    /// no annotation exists — this is the safe default (runtime RC check).
    pub(crate) fn cow_mode_const(&mut self, arc_func: &ArcFunction) -> ValueId {
        let mode = arc_func
            .cow_annotations
            .get(self.current_block_idx, self.current_instr_idx);
        tracing::debug!(
            block = self.current_block_idx,
            instr = self.current_instr_idx,
            ?mode,
            "cow_mode_const queried"
        );
        self.builder.const_i32(mode as i32)
    }

    /// Mark `data_ptr` (param 0) as `noalias` on the last emitted call if
    /// the current instruction's COW mode is [`CowMode::StaticUnique`].
    ///
    /// When static uniqueness analysis proves a collection buffer has
    /// refcount == 1, no other live pointer can reference it. This lets
    /// LLVM optimize loads/stores in the COW runtime function without
    /// alias concerns (same principle as Rust's `noalias` on `&mut T`).
    ///
    /// Must be called immediately after the `self.builder.call()` that
    /// invokes the COW runtime function.
    pub(crate) fn mark_cow_data_noalias_if_unique(&mut self, arc_func: &ArcFunction) {
        let mode = arc_func
            .cow_annotations
            .get(self.current_block_idx, self.current_instr_idx);
        if mode == CowMode::StaticUnique {
            self.builder.mark_last_call_param_noalias(0);
        }
    }

    /// Resolve an `Idx` to an `LLVMTypeId`.
    pub(super) fn resolve_type(&mut self, idx: Idx) -> LLVMTypeId {
        let llvm_ty = self.type_resolver.resolve(idx);
        self.builder.register_type(llvm_ty)
    }

    /// Emit a runtime call, automatically adding a `"funclet"` operand bundle
    /// when inside a SEH pad (`current_funclet_pad` is `Some`).
    ///
    /// On Itanium (non-MSVC) targets, `current_funclet_pad` is always `None`
    /// so this is a plain `self.builder.call()`. On SEH targets, cleanup and
    /// catch pads set `current_funclet_pad` before emitting body instructions,
    /// and this method transparently attaches the required bundle.
    pub(super) fn emit_rt_call(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        if let Some((pad, _kind)) = self.current_funclet_pad {
            self.builder.call_with_funclet(callee, args, pad, name)
        } else {
            self.builder.call(callee, args, name)
        }
    }

    /// Branch to `target`, exiting the current catchpad via `catchret` trampoline.
    ///
    /// Emits `catchret pad → trampoline → br target`. Only valid for catchpads;
    /// cleanup pads exit via `cleanupret` (handled by the Resume terminator).
    ///
    /// No-op + plain `br` when `current_funclet_pad` is `None`.
    pub(super) fn br_exiting_catchpad(&mut self, target: BlockId) {
        if let Some((pad, kind)) = self.current_funclet_pad.take() {
            match kind {
                super::FuncletPadKind::Catch => {
                    let trampoline = self
                        .builder
                        .append_block(self.current_function, "seh.continue");
                    self.builder.catchret(pad, trampoline);
                    self.builder.position_at_end(trampoline);
                }
                super::FuncletPadKind::Cleanup => {
                    self.builder.record_codegen_error_with_msg(
                        "br_exiting_catchpad called from cleanuppad — \
                         cleanup pads must exit via cleanupret (Resume terminator)",
                    );
                    self.builder.unreachable();
                    return; // block is terminated; skip the br below
                }
            }
        }
        self.builder.br(target);
    }

    /// Allocate a heap cell via `ori_rc_alloc(size, align)`.
    ///
    /// Returns a `ptr` to the RC-managed allocation. Used for boxing
    /// recursive enum fields that must be stored as pointers in the payload.
    pub(super) fn rc_alloc(&mut self, size: u64, align: u64) -> ValueId {
        let size_val = self.builder.const_i64(size as i64);
        let align_val = self.builder.const_i64(align as i64);
        let rc_alloc_func = self.builder.runtime_fn("ori_rc_alloc");
        self.emit_rt_call(rc_alloc_func, &[size_val, align_val], "rc.alloc")
            .unwrap_or_else(|| self.builder.const_null_ptr())
    }

    /// Compute the store size in bytes for a type index.
    ///
    /// Uses `TypeInfo::size()` for well-known types (primitives, str=16, list=24, etc.).
    /// Falls back to `TypeLayoutResolver::type_store_size()` for compound types
    /// (struct, tuple, enum) where the size depends on field layout.
    pub(crate) fn element_store_size(&self, ty: Idx) -> u64 {
        self.type_info.get(ty).size().unwrap_or_else(|| {
            let llvm_ty = self.type_resolver.resolve(ty);
            TypeLayoutResolver::type_store_size(llvm_ty)
        })
    }

    /// Compute the ABI alignment in bytes for a type index.
    ///
    /// Uses the type's own alignment (from `TypeInfo::alignment()`) rather
    /// than deriving it from size. Falls back to `element_store_size` for
    /// compound types whose alignment depends on field layout.
    pub(crate) fn element_store_align(&self, ty: Idx) -> u64 {
        let info = self.type_info.get(ty);
        u64::from(info.alignment())
    }

    /// Look up the raw LLVM value for an ARC variable.
    ///
    /// Returns the underlying `ValueId`, suitable for consumers that don't
    /// need representation info. For typed access, use [`var_emitted`](Self::var_emitted).
    ///
    /// # Panics
    /// Panics if the stored value is `Pair` or `ZeroSized`. Use `var_emitted()`
    /// for variables that may hold those variants.
    ///
    /// Returns `ValueId::NONE` and logs an error if the variable is not yet defined.
    pub(super) fn var(&self, v: ArcVarId) -> ValueId {
        self.var_emitted(v).into_raw()
    }

    /// Look up the typed emitted value for an ARC variable.
    ///
    /// Returns the full [`EmittedValue`] including representation info.
    /// Prefer this over [`var`](Self::var) when the consumer needs to
    /// distinguish between value kinds (e.g., RC operations).
    pub(super) fn var_emitted(&self, v: ArcVarId) -> EmittedValue {
        if let Some(Some(val)) = self.var_map.get(v.index()) {
            *val
        } else {
            tracing::error!(var = v.raw(), "ArcIrEmitter: variable not yet defined");
            EmittedValue::Immediate(ValueId::NONE)
        }
    }

    /// Bind an ARC variable to a typed LLVM value.
    pub(super) fn def_var(&mut self, v: ArcVarId, val: EmittedValue) {
        let idx = v.index();
        if idx >= self.var_map.len() {
            self.var_map.resize(idx + 1, None);
        }
        self.var_map[idx] = Some(val);
    }

    /// Bind an ARC variable to a raw LLVM value, inferring its [`EmittedValue`]
    /// variant from the variable's [`ValueRepr`] in the ARC function.
    ///
    /// §04.4 Phase B: If the variable is in `narrowed_vars`, the incoming i64
    /// value is truncated to the narrow width and immediately sign-extended back
    /// to i64. This trunc+sext pair (a) validates the value fits in the narrow
    /// range and (b) informs LLVM of the restricted range for optimization.
    /// Consistent with the phi path which also stores sext'd i64 values.
    pub(super) fn def_var_repr(&mut self, v: ArcVarId, val: ValueId, func: &ArcFunction) {
        let repr = func.var_repr(v).unwrap_or(ValueRepr::Scalar);
        let final_val = self.narrow_local_if_needed(v, val);
        self.def_var(v, EmittedValue::from_repr(repr, final_val));
    }

    /// §04.4 Phase B: Insert trunc+sext for a narrowed local variable.
    ///
    /// If `v` is in `narrowed_vars`, truncates `val` from i64 to the narrow
    /// width, then sign-extends back to i64. Returns the sext'd value.
    /// If `v` is not narrowed, returns `val` unchanged.
    fn narrow_local_if_needed(&mut self, v: ArcVarId, val: ValueId) -> ValueId {
        let Some(&width) = self.narrowed_vars.get(&v) else {
            return val;
        };

        // Only narrow actual i64 int values — skip non-int, zero-sized, pairs.
        let llvm_val = self.builder.arena.get_value(val);
        if !llvm_val.is_int_value() {
            return val;
        }
        let bits = llvm_val.into_int_value().get_type().get_bit_width();
        if bits <= width.size_bytes() * 8 {
            // Already narrow or narrower — no truncation needed.
            return val;
        }

        let narrow_ty = self.llvm_type_for_int_width(width);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx.type_i64().into());

        let truncated = self
            .builder
            .trunc(val, narrow_ty, &format!("local.trunc.{}", v.raw()));
        self.builder
            .sext(truncated, i64_ty, &format!("local.sext.{}", v.raw()))
    }

    /// §04.4 Phase B: Compute which local variables should use narrow int types.
    ///
    /// Scans all variables in the function and checks if their value range
    /// (from the `ReprPlan`) fits in a narrower integer type. Function
    /// parameters are excluded (no visibility info in ARC IR — conservative).
    pub(super) fn compute_narrowed_vars(&mut self, func: &ArcFunction) {
        use ori_types::Tag;

        self.narrowed_vars.clear();

        let Some(plan) = self.type_resolver.repr_plan() else {
            return;
        };

        // Collect parameter variable IDs for exclusion.
        let param_vars: rustc_hash::FxHashSet<ArcVarId> =
            func.params.iter().map(|p| p.var).collect();

        // Check each variable in the function.
        for (raw_idx, &ty_idx) in func.var_types.iter().enumerate() {
            let var = ArcVarId::new(raw_idx as u32);

            // Skip function parameters (can't distinguish pub from private).
            if param_vars.contains(&var) {
                continue;
            }

            // Only narrow int-typed variables.
            let resolved = self.pool.resolve_fully(ty_idx);
            if self.pool.tag(resolved) != Tag::Int {
                continue;
            }

            // Query the range analysis for this variable.
            let range = plan.var_range(func.name, var);
            let width = range.min_width();

            // Only record if narrower than canonical i64.
            if width != ori_repr::IntWidth::I64 {
                self.narrowed_vars.insert(var, width);
            }
        }

        if !self.narrowed_vars.is_empty() {
            tracing::debug!(
                func = func.name.raw(),
                count = self.narrowed_vars.len(),
                "§04.4 Phase B: narrowed local variables"
            );
        }
    }

    /// §04.4 Phase B: Get the LLVM integer type for a given `IntWidth`.
    pub(super) fn llvm_type_for_int_width(&mut self, width: ori_repr::IntWidth) -> LLVMTypeId {
        let scx = self.builder.scx();
        let ty = match width {
            ori_repr::IntWidth::I8 => scx.type_i8().into(),
            ori_repr::IntWidth::I16 => scx.type_i16().into(),
            ori_repr::IntWidth::I32 => scx.type_i32().into(),
            ori_repr::IntWidth::I64 => scx.type_i64().into(),
        };
        self.builder.register_type(ty)
    }

    /// Look up the LLVM block for an ARC block.
    ///
    /// Panics if the block is a dead unwind block (no LLVM block was created).
    pub(super) fn block(&self, b: ori_arc::ir::ArcBlockId) -> super::BlockId {
        self.block_map[b.index()]
            .expect("block() called for dead unwind block — invariant violated")
    }

    /// Truncate struct field values from canonical width (i64) to narrowed
    /// field width (i8/i16/i32) when constructing a narrowed struct.
    ///
    /// Returns a new `Vec` with truncated values where needed. Values whose
    /// LLVM type already matches the struct field type pass through unchanged.
    ///
    /// Only truncates fields whose pool type is `Tag::Int` (canonical i64) —
    /// naturally narrow types (Byte → i8, Char → i32) are not affected.
    ///
    /// This is the construction-side half of §04.4 integer narrowing codegen.
    /// The extraction-side half is [`sext_narrowed_field`](Self::sext_narrowed_field).
    pub(super) fn trunc_for_narrowed_struct(
        &mut self,
        struct_ty_id: LLVMTypeId,
        args: &[ValueId],
        ctor_type: Idx,
    ) -> Vec<ValueId> {
        use inkwell::types::BasicTypeEnum;
        use ori_types::Tag;

        let raw_ty = self.builder.arena.get_type(struct_ty_id);
        let BasicTypeEnum::StructType(st) = raw_ty else {
            return args.to_vec();
        };

        // Get the struct/tuple field types from the pool.
        let resolved = self.pool.resolve_fully(ctor_type);
        let pool_tag = self.pool.tag(resolved);
        let field_pool_types: Vec<Idx> = if pool_tag == Tag::Struct {
            self.pool
                .struct_fields(resolved)
                .into_iter()
                .map(|(_, idx)| idx)
                .collect()
        } else if pool_tag == Tag::Tuple {
            self.pool.tuple_elems(resolved)
        } else {
            return args.to_vec();
        };

        args.iter()
            .enumerate()
            .map(|(i, &val)| {
                // Only truncate when the pool field type is Int (canonical i64)
                // but the struct field is narrower.
                let is_int_field = field_pool_types
                    .get(i)
                    .is_some_and(|&idx| self.pool.tag(self.pool.resolve_fully(idx)) == Tag::Int);
                if !is_int_field {
                    return val;
                }
                let Some(BasicTypeEnum::IntType(field_int)) = st.get_field_type_at_index(i as u32)
                else {
                    return val;
                };
                let field_bits = field_int.get_bit_width();
                if field_bits >= 64 {
                    return val;
                }
                let v = self.builder.arena.get_value(val);
                if !v.is_int_value() {
                    return val;
                }
                let val_bits = v.into_int_value().get_type().get_bit_width();
                if val_bits > field_bits {
                    let field_ty_id = self.builder.register_type(field_int.into());
                    self.builder
                        .trunc(val, field_ty_id, &format!("narrow.trunc.{i}"))
                } else {
                    val
                }
            })
            .collect()
    }

    /// Sign-extend a narrowed struct field value (i8/i16/i32) back to
    /// canonical width (i64) after extraction.
    ///
    /// Only extends when the ARC IR destination type is `Tag::Int` (expects
    /// canonical i64) but the extracted value is narrower — this ensures
    /// naturally narrow types (Byte → i8, Char → i32) are not affected.
    ///
    /// This is the extraction-side half of §04.4 integer narrowing codegen.
    pub(super) fn sext_narrowed_field(
        &mut self,
        extracted: ValueId,
        field_index: u32,
        dst_type: Idx,
    ) -> ValueId {
        use ori_types::Tag;

        // Only sext when the destination expects i64 (Tag::Int).
        let resolved = self.pool.resolve_fully(dst_type);
        if self.pool.tag(resolved) != Tag::Int {
            return extracted;
        }
        let v = self.builder.arena.get_value(extracted);
        if !v.is_int_value() {
            return extracted;
        }
        let bits = v.into_int_value().get_type().get_bit_width();
        if bits >= 64 {
            return extracted;
        }
        let i64_ty = self
            .builder
            .register_type(self.builder.scx.type_i64().into());
        self.builder
            .sext(extracted, i64_ty, &format!("narrow.sext.{field_index}"))
    }
}
