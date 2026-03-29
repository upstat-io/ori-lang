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

    /// Look up the LLVM block for an ARC block.
    ///
    /// Panics if the block is a dead unwind block (no LLVM block was created).
    pub(super) fn block(&self, b: ori_arc::ir::ArcBlockId) -> super::BlockId {
        self.block_map[b.index()]
            .expect("block() called for dead unwind block — invariant violated")
    }
}
