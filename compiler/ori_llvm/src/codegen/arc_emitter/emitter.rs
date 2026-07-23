//! ARC IR emitter state.
//!
//! [`ori_arc::ArcFunction`] blocks become LLVM control flow, RC operations,
//! builtins, calls, and structured unwind cleanup. This is the shared JIT and
//! AOT function-emission path.

use super::{
    ArcClassification, ArcEmitterFunctionContext, ArcVarId, BlockId, CodegenContext, DebugContext,
    EmittedValue, FormatRtNames, FunctionId, FxHashMap, FxHashSet, Idx, IrBuilder, LLVMTypeId,
    ListRtNames, MemoryContract, Pool, StringInterner, TokenId, TypeInfoStore, TypeLayoutResolver,
    ValueId,
};

/// Emits LLVM IR from ARC IR basic blocks.
///
/// Maps `ArcVarId` → `ValueId` and `ArcBlockId` → `BlockId`, walking
/// each block's instructions and terminator to produce LLVM IR.
pub struct ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// ID-based LLVM instruction builder.
    pub(super) builder: &'a mut IrBuilder<'scx, 'ctx>,
    /// Type info cache (`Idx` → `TypeInfo`).
    pub(super) type_info: &'a TypeInfoStore<'tcx>,
    /// Recursive type layout resolver.
    pub(super) type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
    /// String interner for `Name` → `&str`.
    pub(super) interner: &'a StringInterner,
    /// Pre-interned list-runtime-symbol names for emission-site identity checks.
    pub(super) list_rt_names: ListRtNames,
    /// Pre-interned formatting-runtime-symbol names for identity checks.
    pub(super) format_rt_names: FormatRtNames,
    /// Type pool for structural queries (used by drop function generation).
    pub(super) pool: &'a Pool,
    /// ARC type classifier for drop function generation.
    pub(super) classifier: &'a dyn ArcClassification,
    /// Cache: type `Idx` → already-generated drop function `FunctionId`.
    /// Avoids regenerating drop functions for the same type and handles
    /// recursive types (entry inserted before body generation).
    pub(super) drop_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated element-dec function.
    /// Element-dec functions receive a pointer to an element within a buffer
    /// and decrement that element's RC children (without freeing the element).
    pub(super) elem_dec_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated element-inc function.
    /// Element-inc functions receive a pointer to an element within a buffer
    /// and increment that element's RC children for COW copying.
    pub(super) elem_inc_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated comparison thunk `FunctionId`.
    /// Compare thunks have signature `fn(*const u8, *const u8) -> i32` and
    /// return a three-way ordering code.
    pub(super) compare_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated equality thunk `FunctionId`.
    /// Equality thunks have signature `fn(*const u8, *const u8) -> bool` and
    /// compare two element addresses.
    pub(super) eq_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated hash thunk `FunctionId`.
    /// Hash thunks have signature `fn(*const u8) -> i64` and hash one element
    /// address.
    pub(super) hash_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// The LLVM function being compiled.
    pub(super) current_function: FunctionId,
    /// Shared function-resolution lookup tables.
    pub(super) ctx: &'a CodegenContext,
    /// Debug info context (None for JIT or when debug info is disabled).
    pub(super) debug_context: Option<&'a DebugContext<'ctx>>,
    /// Counter for unique `PartialApply` wrapper/drop function names.
    pub(super) partial_apply_counter: u32,
    /// Counter for unique catch thunk function names (SEH `catch(expr:)`).
    pub(super) catch_thunk_counter: u32,
    /// ARC variable → typed LLVM value mapping.
    pub(super) var_map: Vec<Option<EmittedValue>>,
    /// ARC block → LLVM block mapping (`None` for dead unwind blocks).
    pub(super) block_map: Vec<Option<BlockId>>,
    /// Deferred phi incoming values: `block_index` → `[(param_index, value, source_block)]`.
    /// Collected during terminator emission, applied after all blocks are emitted.
    pub(super) phi_incoming: Vec<(usize, usize, ValueId, BlockId)>,
    /// Current block index in the ARC IR function (set during emission).
    /// Together with `current_instr_idx`, this keys the current instruction's
    /// `CowAnnotations` entry.
    pub(super) current_block_idx: usize,
    /// Current instruction index within the current block (set during emission).
    pub(super) current_instr_idx: usize,
    /// Active SEH cleanup-pad token.
    ///
    /// Runtime calls carry this token in their `"funclet"` operand bundle.
    pub(super) current_cleanup_pad: Option<TokenId>,
    /// Live ARC unwind block for the in-flight INTERCEPTED may-unwind builtin
    /// emission (e.g. list `updated` — `ori_list_updated_cow` panics on OOB).
    /// When `Some`, `emit_rt_call` routes calls to non-nounwind runtime
    /// functions through `invoke` with this block as the unwind edge so the
    /// ARC cleanup decs run on the panic path. Set/cleared by `emit_invoke`
    /// around `try_emit_builtin_method`; Itanium-only (`None` inside SEH
    /// funclet pads).
    pub(super) intercepted_unwind: Option<crate::codegen::value_id::BlockId>,
    /// Maps each same-frame catch-scoped inline checked-op result `ArcVarId`
    /// to the LLVM landing-pad `BlockId` for its cleanup-only unwind block.
    /// Empty when the function has no same-frame checked-op catch.
    pub(super) same_frame_catch_landing_pads:
        FxHashMap<ArcVarId, crate::codegen::value_id::BlockId>,
    /// Variables rooted at borrowed parameters (or Let-aliases thereof).
    /// When storing an inline enum value to a boxed field, sub-pointers
    /// must be incremented if the source is borrowed-rooted (the caller
    /// retains a reference, so the boxed store creates an additional one).
    pub(super) borrowed_rooted_vars: FxHashSet<ArcVarId>,
    /// The current function's interprocedural `MemoryContract`, when available.
    pub(super) func_contract: Option<&'a MemoryContract>,
    /// Returned yield result whose private clone materializes only its length.
    pub(super) length_only_yield_result: Option<ArcVarId>,
    /// Variables rooted at a parameter whose final contract demands a whole-
    /// value ownership credit. For such a `.iter()` receiver the iterator owns
    /// the backing buffer even when borrow inference selected a reference ABI.
    /// This is the callee-side counterpart of the credit emitted by a closure
    /// adapter (or ordinary call-site realization).
    pub(super) iter_owns_rooted_vars: FxHashSet<ArcVarId>,
    /// Borrowed parameter pointer forwarding: maps `ArcVarId` → original LLVM
    /// parameter pointer for variables received as `Reference`/`Indirect` params.
    /// Pointer-accepting callees receive the original parameter pointer without
    /// an alloca/store round trip.
    pub(super) borrowed_param_ptrs: FxHashMap<ArcVarId, ValueId>,
    /// Borrowed `Reference`/`Indirect` parameters whose entry-block aggregate
    /// load was elided (bound to a zero placeholder) because every use forwards
    /// the source pointer. A `Direct` (by-value) call argument cannot forward a
    /// pointer — it must materialize the aggregate by loading from
    /// `borrowed_param_ptrs`, so the Direct passing path consults this set to
    /// reload only the elided params.
    pub(super) pointer_only_params: FxHashSet<ArcVarId>,
    /// Decomposed iterator-next results: maps the `Apply(__iter_next)` dst
    /// `ArcVarId` → `(tag: ValueId, scratch_ptr: ValueId, elem_ty: LLVMTypeId)`.
    ///
    /// When `emit_project` encounters a `Project(value, field)` where `value`
    /// is in this map, it returns the tag (field 0) or loads from scratch
    /// (field 1) directly without materializing a `{i64, T}` wrapper struct.
    pub(super) iter_next_decomposed: FxHashMap<ArcVarId, (ValueId, ValueId, LLVMTypeId)>,
    /// The function's sret destination and exact LLVM pointee type.
    /// Nested `call_with_sret` forwarding consumes the destination only when
    /// the callee writes the same physical type. A mismatched aggregate
    /// must use its own alloca or it can overwrite the caller's result slot.
    /// A compatible destination is consumed on first use to prevent multiple
    /// calls from writing to the same sret pointer.
    pub(super) current_sret: Option<(ValueId, LLVMTypeId)>,

    /// The `ValueId` of the result loaded from a forwarded sret pointer.
    /// When set, the `Return + Sret` terminator can skip the identity store
    /// (the value is already at the sret destination).
    pub(super) sret_forwarded_result: Option<ValueId>,

    /// Narrowed widths for local integer variables. Definitions truncate to the
    /// stored width, uses extend to `i64`, and phi nodes retain the narrow type.
    /// Parameters remain canonical because ARC IR exposes no ABI visibility.
    pub(super) narrowed_vars: FxHashMap<ArcVarId, ori_repr::IntWidth>,

    /// Repr plan for collection element narrowing.
    ///
    /// When present, collection construction and element access paths consult
    /// this plan for narrowed element types (e.g., `[int]` with elements in
    /// `[-128, 127]` uses `i8` element storage instead of `i64`).
    pub(super) repr_plan: Option<&'a ori_repr::ReprPlan>,

    /// Yield collection and element types indexed by shared element-size operand.
    pub(super) yield_types_by_elem_size_var: FxHashMap<ArcVarId, (Idx, Idx)>,

    /// Closed yield-allocation lineage index for the function being emitted.
    pub(super) yield_lineages: ori_arc::YieldLineageIndex,

    /// Niche-encoded enum tag tracking.
    ///
    /// When `Project { field: 0 }` extracts a tag from a niche-encoded enum,
    /// the destination variable holds the raw niche field value (not a logical
    /// variant index). This map records `dst_var → source_enum_type_idx` so
    /// that `Switch` can emit niche-aware comparisons instead of a standard
    /// LLVM switch instruction.
    pub(super) niche_scrutinees: FxHashMap<ArcVarId, Idx>,

    /// Whether ARC/LLVM IR verification is enabled (`ORI_VERIFY_ARC=1`).
    /// Set via [`set_verify_arc`] after construction; defaults to `false`.
    /// SSOT: plumbed from `FunctionCompiler::verify_arc` — do NOT re-read env var.
    pub(super) verify_arc: bool,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new ARC IR emitter.
    pub(crate) fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
        interner: &'a StringInterner,
        pool: &'a Pool,
        classifier: &'a dyn ArcClassification,
        function: ArcEmitterFunctionContext<'a, 'ctx>,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            list_rt_names: ListRtNames::from_interner(interner),
            format_rt_names: FormatRtNames::from_interner(interner),
            interner,
            pool,
            classifier,
            drop_fn_cache: FxHashMap::default(),
            elem_dec_fn_cache: FxHashMap::default(),
            elem_inc_fn_cache: FxHashMap::default(),
            compare_thunk_cache: FxHashMap::default(),
            eq_thunk_cache: FxHashMap::default(),
            hash_thunk_cache: FxHashMap::default(),
            current_function: function.current_function,
            ctx: function.codegen,
            debug_context: function.debug,
            partial_apply_counter: 0,
            catch_thunk_counter: 0,
            var_map: Vec::new(),
            block_map: Vec::new(),
            phi_incoming: Vec::new(),
            current_block_idx: 0,
            current_instr_idx: 0,
            current_cleanup_pad: None,
            intercepted_unwind: None,
            same_frame_catch_landing_pads: FxHashMap::default(),
            borrowed_rooted_vars: FxHashSet::default(),
            func_contract: None,
            length_only_yield_result: None,
            iter_owns_rooted_vars: FxHashSet::default(),
            borrowed_param_ptrs: FxHashMap::default(),
            pointer_only_params: FxHashSet::default(),
            iter_next_decomposed: FxHashMap::default(),
            current_sret: None,
            sret_forwarded_result: None,
            narrowed_vars: FxHashMap::default(),
            repr_plan: type_resolver.repr_plan(),
            yield_types_by_elem_size_var: FxHashMap::default(),
            yield_lineages: ori_arc::YieldLineageIndex::default(),
            niche_scrutinees: FxHashMap::default(),
            verify_arc: false,
        }
    }

    /// Set whether ARC/LLVM IR verification is enabled.
    ///
    /// Stores the verification choice for all subsequent emission.
    pub fn set_verify_arc(&mut self, enable: bool) {
        self.verify_arc = enable;
    }

    /// Check if a variable is rooted at a borrowed parameter.
    ///
    /// Returns `true` if the variable is a borrowed function parameter or
    /// a `Let`-alias chain leading to one. The classification determines
    /// whether boxed inline-enum storage needs a sub-pointer increment
    /// (borrowed → yes, consumed → no).
    pub(super) fn is_var_borrowed_rooted(&self, var: ArcVarId) -> bool {
        self.borrowed_rooted_vars.contains(&var)
    }

    /// Supply the current function's interprocedural `MemoryContract` before
    /// `emit_function`, so `compute_borrowed_rooted_vars` can seed iterator
    /// ownership from its `ParamContract`s. Defaults to `None` (no flips) for
    /// low-level emitter tests that do not bind a closed executable artifact.
    pub(in crate::codegen) fn set_func_contract(&mut self, contract: Option<&'a MemoryContract>) {
        self.func_contract = contract;
    }

    /// Select the single returned-yield payload virtualized in a private clone.
    pub(in crate::codegen) fn set_length_only_yield_result(&mut self, result: Option<ArcVarId>) {
        self.length_only_yield_result = result;
    }

    /// Whether the `.iter()` receiver `var` roots to a parameter whose final
    /// contract requires a whole-value ownership credit. The iterator consumes
    /// that credit (`owns_data=true`) even when the physical ABI is borrowed.
    pub(super) fn iter_receiver_owns_via_contract(&self, var: ArcVarId) -> bool {
        self.iter_owns_rooted_vars.contains(&var)
    }
}

// Why: Debug output omits shared compiler contexts to avoid recursive context dumps.
impl std::fmt::Debug for ArcIrEmitter<'_, '_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcIrEmitter")
            .field("current_function", &self.current_function)
            .field("drop_fn_cache_size", &self.drop_fn_cache.len())
            .field("elem_dec_fn_cache_size", &self.elem_dec_fn_cache.len())
            .field("elem_inc_fn_cache_size", &self.elem_inc_fn_cache.len())
            .field("current_block_idx", &self.current_block_idx)
            .field("current_instr_idx", &self.current_instr_idx)
            .field("var_count", &self.var_map.len())
            .field("block_count", &self.block_map.len())
            .field("verify_arc", &self.verify_arc)
            .finish_non_exhaustive()
    }
}
