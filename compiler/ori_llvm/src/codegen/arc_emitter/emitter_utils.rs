//! Variable, block, and type utility methods for [`ArcIrEmitter`].
//!
//! Provides the core infrastructure that all emission submodules depend on:
//!
//! - **Variable mapping**: `var()`, `var_emitted()`, `def_var()`, `def_var_repr()`
//! - **Block mapping**: `block()`
//! - **Type resolution**: `resolve_type()`, `element_store_size()`, `element_store_align()`
//! - **COW annotations**: `cow_mode_const()`, `mark_cow_data_noalias_if_unique()`
//! - **RC allocation**: `rc_alloc()`

use ori_arc::ir::{ArcFunction, ArcTerminator, ArcValue, ArcVarId, PrimOp, ValueRepr};
use ori_arc::CowMode;
use ori_ir::UnaryOp;
use ori_types::Idx;

use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

use super::emitted_mappings::let_alias_root;
use super::ArcIrEmitter;

/// Read representation metadata at the LLVM phase boundary.
///
/// A missing entry means AIMS handed codegen an unrealized function. LLVM
/// cannot safely substitute a physical representation for that missing fact.
pub(super) fn required_var_repr(var: ArcVarId, func: &ArcFunction) -> ValueRepr {
    let Some(repr) = func.var_repr(var) else {
        // Why: AIMS realization assigns every variable representation before LLVM emission.
        unreachable!("LLVM emission requires realized variable representations")
    };
    repr
}

/// Canonical home for the field/variant RC-walk ORDER decision.
///
/// Teardown (dec/drop) walks reverse-declaration (LIFO) per
/// drop-trait-proposal §Drop and panic, so a user `@drop` side effect observes
/// a consistent field-teardown order across every emission path: the heap
/// drop-fn walk (`drop_gen::emit_drop_fields`), the inline-aggregate dec walk
/// (`rc_value_traversal::dec_aggregate_fields`), and the struct/Result/Option
/// payload walk (`rc_enum_values`). Inc keeps forward declaration order (order is
/// unobservable for inc). The tagless-enum payload walk
/// (`tagless_enum`) consumes the same LIFO contract over borrowed fields.
///
/// Consolidating the order decision here means a change to the teardown order
/// is made in ONE place; the walks cannot drift on the LIFO invariant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldRcWalkOrder {
    Forward,
    Teardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RcOperation {
    Retain { count: u32 },
    Release,
}

impl RcOperation {
    pub(crate) const fn prefix(self) -> &'static str {
        match self {
            Self::Retain { .. } => "rc_inc",
            Self::Release => "rc_dec",
        }
    }

    pub(crate) const fn retain_count(self) -> Option<u32> {
        match self {
            Self::Retain { count } => Some(count),
            Self::Release => None,
        }
    }

    pub(crate) const fn field_walk_order(self) -> FieldRcWalkOrder {
        match self {
            Self::Retain { .. } => FieldRcWalkOrder::Forward,
            Self::Release => FieldRcWalkOrder::Teardown,
        }
    }
}

pub(crate) fn field_rc_walk_order<T: Copy>(decl_order: &[T], order: FieldRcWalkOrder) -> Vec<T> {
    if order == FieldRcWalkOrder::Teardown {
        decl_order.iter().rev().copied().collect()
    } else {
        decl_order.to_vec()
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Read the current instruction's frozen logical COW decision.
    pub(crate) fn cow_mode(&self, arc_func: &ArcFunction) -> CowMode {
        arc_func
            .cow_annotations
            .get(self.current_block_idx, self.current_instr_idx)
    }

    fn yield_allocation_for_receiver(
        &self,
        arc_func: &ArcFunction,
        receiver: ArcVarId,
    ) -> Option<ori_repr::CompiledAllocationDecision> {
        let result = self.yield_lineages.result_for_receiver(receiver)?;
        self.repr_plan?
            .yield_allocation_for_result(arc_func.name, result)
    }

    /// Whether `receiver` belongs to a yield lineage whose frozen compiled
    /// allocation mechanism is a stack slot.
    pub(crate) fn is_stack_slot_yield_receiver(
        &self,
        arc_func: &ArcFunction,
        receiver: ArcVarId,
    ) -> bool {
        self.yield_allocation_for_receiver(arc_func, receiver)
            .is_some_and(|decision| decision.mechanism.is_stack())
    }

    /// Whether `receiver` uses compact stack backing with no preceding RC header.
    pub(crate) fn is_compact_stack_slot_yield_receiver(
        &self,
        arc_func: &ArcFunction,
        receiver: ArcVarId,
    ) -> bool {
        self.yield_allocation_for_receiver(arc_func, receiver)
            .is_some_and(|decision| {
                matches!(
                    decision.mechanism,
                    ori_repr::CompiledAllocationMechanism::CompactStack { .. }
                )
            })
    }

    /// Whether a stack-backed yield receiver contains only trivial scalar elements.
    ///
    /// Such a receiver has neither a managed allocation to release nor
    /// element destructors to run, so its physical release is empty.
    pub(crate) fn is_scalar_stack_slot_yield_receiver(
        &self,
        arc_func: &ArcFunction,
        receiver: ArcVarId,
    ) -> bool {
        self.yield_allocation_for_receiver(arc_func, receiver)
            .is_some_and(|decision| {
                decision.mechanism.is_stack()
                    && self.classifier.is_scalar(decision.elem_ty)
                    && self.user_drop_method(decision.elem_ty).is_none()
            })
    }

    /// Whether `receiver` is the result of a call redirected to a private
    /// length-only physical clone.
    pub(crate) fn is_length_projection_result(
        &self,
        arc_func: &ArcFunction,
        receiver: ArcVarId,
    ) -> bool {
        let root = let_alias_root(arc_func, receiver);
        self.ctx
            .length_projection_call_targets
            .contains_key(&(arc_func.name, root))
    }

    /// Whether the current `updated` call is the normal-edge half of
    /// `xs[i] = !xs[i]` for the same receiver and index.
    ///
    /// The preceding checked index invoke owns the panic edge. On its normal
    /// continuation, replacing the second checked COW update with one byte xor
    /// preserves both bounds semantics and the frozen unique stack identity.
    pub(crate) fn is_negated_same_index_update(
        &self,
        arc_func: &ArcFunction,
        receiver: ArcVarId,
        index: ArcVarId,
        value: ArcVarId,
    ) -> bool {
        let Some(index_result) = arc_func
            .blocks
            .iter()
            .flat_map(|block| &block.body)
            .find_map(|instruction| match instruction {
                ori_arc::ArcInstr::Let {
                    dst,
                    value:
                        ArcValue::PrimOp {
                            op: PrimOp::Unary(UnaryOp::Not),
                            args,
                        },
                    ..
                } if *dst == value && args.len() == 1 => Some(args[0]),
                _ => None,
            })
        else {
            return false;
        };

        arc_func.blocks.iter().any(|block| {
            let ArcTerminator::Invoke {
                dst,
                func,
                args,
                normal,
                ..
            } = &block.terminator
            else {
                return false;
            };
            *dst == index_result
                && normal.index() == self.current_block_idx
                && self.interner.lookup(*func) == "__index"
                && args.len() == 2
                && let_alias_root(arc_func, args[0]) == let_alias_root(arc_func, receiver)
                && let_alias_root(arc_func, args[1]) == let_alias_root(arc_func, index)
        })
    }

    /// Get the current instruction's COW mode as an LLVM `i32` constant.
    ///
    /// Queries the `ArcFunction`'s `cow_annotations` for the current
    /// `(block_idx, instr_idx)` coordinate. Returns `Dynamic` (0) when
    /// no annotation exists — this is the safe default (runtime RC check).
    pub(crate) fn cow_mode_const(&mut self, arc_func: &ArcFunction) -> ValueId {
        let mode = self.cow_mode(arc_func);
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

    /// Emit a runtime call with the active SEH cleanup-pad bundle.
    ///
    /// Itanium targets have no cleanup token and emit a plain call.
    pub(super) fn emit_rt_call(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        if let Some(pad) = self.current_cleanup_pad {
            return self.builder.call_with_funclet(callee, args, pad, name);
        }
        // Intercepted may-unwind builtin emission: route calls to
        // non-nounwind runtime functions (e.g. `ori_list_updated_cow`, which
        // panics on OOB) through `invoke` with the ARC unwind block as the
        // unwind edge, so the cleanup decs run on the panic path. Armed by
        // `emit_invoke` around `try_emit_builtin_method` (per
        // `context::intercepted_emission_invokes_unwind`).
        if let Some(unwind_bb) = self.intercepted_unwind {
            if !self.builder.function_has_nounwind_attr(callee) {
                let cont = self.builder.append_block(self.current_function, "rt.cont");
                let result = self.builder.invoke(callee, args, cont, unwind_bb, name);
                self.builder.position_at_end(cont);
                return result;
            }
        }
        self.builder.call(callee, args, name)
    }

    /// Build an invoke-aware call to an sret-returning runtime function.
    ///
    /// The sret twin of [`Self::emit_rt_call`] — mirrors
    /// [`crate::codegen::ir_builder::IrBuilder::call_with_sret`]'s alloca +
    /// prepend + call + load bookkeeping, but routes the underlying call
    /// through `emit_rt_call` so a may-unwind sret callee (e.g.
    /// `ori_str_index`) correctly emits `invoke` when `intercepted_unwind`
    /// is armed, instead of always emitting a plain `call`.
    pub(super) fn emit_rt_call_with_sret(
        &mut self,
        callee: crate::codegen::value_id::FunctionId,
        args: &[crate::codegen::value_id::ValueId],
        sret_type: crate::codegen::value_id::LLVMTypeId,
        name: &str,
    ) -> Option<crate::codegen::value_id::ValueId> {
        let sret_ptr = self.builder.create_entry_alloca(
            self.current_function,
            &format!("{name}.sret"),
            sret_type,
        );
        let mut full_args = Vec::with_capacity(args.len().saturating_add(1));
        full_args.push(sret_ptr);
        full_args.extend_from_slice(args);
        self.emit_rt_call(callee, &full_args, name);
        Some(self.builder.load(sret_type, sret_ptr, name))
    }

    /// Branch to `target` only when no SEH cleanup pad is active.
    pub(super) fn br_outside_cleanup_pad(&mut self, target: BlockId) {
        if self.current_cleanup_pad.take().is_some() {
            self.builder.record_codegen_error_with_msg(
                "normal branch inside cleanuppad; cleanup pads must exit with Resume",
            );
            self.builder.unreachable();
            return;
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
        let Some(allocation) = self.emit_rt_call(rc_alloc_func, &[size_val, align_val], "rc.alloc")
        else {
            // Why: The registered ori_rc_alloc ABI returns a non-void data pointer.
            unreachable!("ori_rc_alloc must produce its registered return value")
        };
        allocation
    }

    /// Compute the store size in bytes for a type index.
    ///
    /// Uses `TypeInfo::size()` for well-known types (primitives, str=16, list=24, etc.).
    /// For structs/tuples with `ReprPlan` entries, uses the `ReprPlan` size
    /// which includes trailing alignment padding (matching `pool_type_store_size`).
    pub(crate) fn element_store_size(&self, ty: Idx) -> u64 {
        if let Some(sz) = self.type_info.get(ty).size() {
            return sz;
        }
        // Use ReprPlan size for structs/tuples — includes trailing alignment
        // padding, matching pool_type_store_size in the ARC lowerer.
        if let Some(plan) = self.repr_plan {
            let resolved = self.pool.resolve_fully(ty);
            if let Some(repr) = plan.get_repr(resolved) {
                let repr_size = match repr {
                    ori_repr::MachineRepr::Struct(s) => Some(u64::from(s.size)),
                    ori_repr::MachineRepr::Tuple(t) => Some(u64::from(t.size)),
                    _ => None,
                };
                if let Some(sz) = repr_size {
                    return sz;
                }
            }
        }
        let llvm_ty = self.type_resolver.resolve(ty);
        TypeLayoutResolver::type_store_size(llvm_ty)
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
}
