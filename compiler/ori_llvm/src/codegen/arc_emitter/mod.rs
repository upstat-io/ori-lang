//! ARC IR → LLVM IR emitter.
//!
//! Translates [`ArcFunction`] basic blocks and instructions directly to LLVM IR,
//! including RC operations (`ori_rc_inc`, `ori_rc_dec`) and structured cleanup
//! via `invoke`/`landingpad`.
//!
//! This is the **sole codegen path** for all Ori functions (JIT and AOT).
//! Every function goes through: `CanExpr → ARC IR → ArcIrEmitter → LLVM IR`.
//!
//! # Architecture
//!
//! ```text
//! CanExpr  →  ori_arc::lower  →  ArcFunction
//!          →  ori_arc pipeline (borrow, RC, reuse, eliminate)
//!          →  ArcIrEmitter    →  LLVM IR  (with RC lifecycle)
//! ```
//!
//! # Submodules
//!
//! - [`builtins`] — builtin method emission (string, list, map, iterator ops)
//! - [`closures`] — closure (partial application) emission and environment management
//! - [`construction`] — value construction: structs, enums, lists, maps, sets
//! - [`context`] — shared types: `CodegenContext`, `EmittedValue`, `InvokeMode`, `is_boxed_enum_field`
//! - [`drop_gen`] — per-type LLVM drop function generation (cached by mangled name)
//! - [`operators`] — binary and unary operator emission (primitive + trait dispatch)
//! - [`rc_helpers`] — RC data pointer extraction and inline enum cleanup
//! - [`rc_ops`] — `ori_rc_inc`/`ori_rc_dec` emission with closure-aware `env_ptr` handling
//! - [`terminators`] — `ArcTerminator` → LLVM control flow emission

mod apply;
mod builtins;
mod closures;
mod construction;
mod context;
mod drop_gen;
mod element_fn_gen;
mod operators;
mod rc_helpers;
mod rc_ops;
mod rc_value_traversal;
mod terminators;
mod value_emission;

pub use context::CodegenContext;
use context::{is_boxed_enum_field, EmittedValue};

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy, ValueRepr};
use ori_arc::{ArcClassification, CowMode};
use ori_ir::StringInterner;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

/// DFS helper for RPO computation. Visits successors then appends self
/// to post-order list. Defined at module level to satisfy `items_after_statements`.
fn rpo_dfs(
    func: &ArcFunction,
    idx: usize,
    visited: &mut [bool],
    post_order: &mut Vec<usize>,
    dead: &rustc_hash::FxHashSet<usize>,
) {
    if idx >= func.blocks.len() || visited[idx] || dead.contains(&idx) {
        return;
    }
    visited[idx] = true;

    match &func.blocks[idx].terminator {
        ArcTerminator::Jump { target, .. } => {
            rpo_dfs(func, target.index(), visited, post_order, dead);
        }
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            rpo_dfs(func, then_block.index(), visited, post_order, dead);
            rpo_dfs(func, else_block.index(), visited, post_order, dead);
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for &(_, target) in cases {
                rpo_dfs(func, target.index(), visited, post_order, dead);
            }
            rpo_dfs(func, default.index(), visited, post_order, dead);
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            rpo_dfs(func, normal.index(), visited, post_order, dead);
            rpo_dfs(func, unwind.index(), visited, post_order, dead);
        }
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }

    post_order.push(idx);
}

/// Compute Reverse Post-Order (RPO) traversal of ARC function blocks.
///
/// RPO guarantees that a block's dominators (and thus variable definitions
/// from preceding blocks) are visited before the block itself. This is
/// critical after `expand_reuse`, which appends fast/slow/merge blocks at
/// the end of the block array — their Invoke terminators target existing
/// blocks with lower indices, creating forward references if iterated in
/// array order.
fn compute_block_rpo(func: &ArcFunction, dead: &rustc_hash::FxHashSet<usize>) -> Vec<usize> {
    let n = func.blocks.len();
    let mut visited = vec![false; n];
    let mut post_order = Vec::with_capacity(n);
    rpo_dfs(
        func,
        func.entry.index(),
        &mut visited,
        &mut post_order,
        dead,
    );
    post_order.reverse();
    post_order
}

// ---------------------------------------------------------------------------
// ArcIrEmitter
// ---------------------------------------------------------------------------

/// Emits LLVM IR from ARC IR basic blocks.
///
/// Maps `ArcVarId` → `ValueId` and `ArcBlockId` → `BlockId`, walking
/// each block's instructions and terminator to produce LLVM IR.
pub struct ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// ID-based LLVM instruction builder.
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    /// Type info cache (`Idx` → `TypeInfo`).
    type_info: &'a TypeInfoStore<'tcx>,
    /// Recursive type layout resolver.
    type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    /// String interner for `Name` → `&str`.
    interner: &'a StringInterner,
    /// Type pool for structural queries (used by drop function generation).
    pool: &'a Pool,
    /// ARC type classifier for drop function generation.
    classifier: &'a dyn ArcClassification,
    /// Cache: type `Idx` → already-generated drop function `FunctionId`.
    /// Avoids regenerating drop functions for the same type and handles
    /// recursive types (entry inserted before body generation).
    drop_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated element-dec function.
    /// Element-dec functions receive a pointer to an element within a buffer
    /// and decrement that element's RC children (without freeing the element).
    elem_dec_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated element-inc function.
    /// Element-inc functions receive a pointer to an element within a buffer
    /// and increment that element's RC children. Used by COW slow paths.
    elem_inc_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated comparison thunk `FunctionId`.
    /// Compare thunks have signature `fn(*const u8, *const u8) -> i32` (-1/0/1)
    /// and are used by `ori_list_sort_cow`.
    compare_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated equality thunk `FunctionId`.
    /// Equality thunks have signature `fn(*const u8, *const u8) -> bool` (i1)
    /// and are used by map/set COW operations for key/element comparison.
    eq_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// The LLVM function being compiled.
    current_function: FunctionId,
    /// Shared function-resolution lookup tables.
    ctx: &'a CodegenContext,
    /// Counter for unique `PartialApply` wrapper/drop function names.
    partial_apply_counter: u32,
    /// ARC variable → typed LLVM value mapping.
    var_map: Vec<Option<EmittedValue>>,
    /// ARC block → LLVM block mapping (`None` for dead unwind blocks).
    block_map: Vec<Option<BlockId>>,
    /// Deferred phi incoming values: `block_index` → `[(param_index, value, source_block)]`.
    /// Collected during terminator emission, applied after all blocks are emitted.
    phi_incoming: Vec<(usize, usize, ValueId, BlockId)>,
    /// Current block index in the ARC IR function (set during emission).
    /// Used by COW emitters to query `CowAnnotations` for the current instruction.
    pub(crate) current_block_idx: usize,
    /// Current instruction index within the current block (set during emission).
    pub(crate) current_instr_idx: usize,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new ARC IR emitter.
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'a Pool,
        classifier: &'a dyn ArcClassification,
        current_function: FunctionId,
        ctx: &'a CodegenContext,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            classifier,
            drop_fn_cache: FxHashMap::default(),
            elem_dec_fn_cache: FxHashMap::default(),
            elem_inc_fn_cache: FxHashMap::default(),
            compare_thunk_cache: FxHashMap::default(),
            eq_thunk_cache: FxHashMap::default(),
            current_function,
            ctx,
            partial_apply_counter: 0,
            var_map: Vec::new(),
            block_map: Vec::new(),
            phi_incoming: Vec::new(),
            current_block_idx: 0,
            current_instr_idx: 0,
        }
    }

    /// Get the current instruction's COW mode as an LLVM `i32` constant.
    ///
    /// Queries the `ArcFunction`'s `cow_annotations` for the current
    /// `(block_idx, instr_idx)` coordinate. Returns `Dynamic` (0) when
    /// no annotation exists — this is the safe default (runtime RC check).
    pub(crate) fn cow_mode_const(&mut self, arc_func: &ArcFunction) -> ValueId {
        let mode = arc_func
            .cow_annotations
            .get(self.current_block_idx, self.current_instr_idx);
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
    fn resolve_type(&mut self, idx: Idx) -> LLVMTypeId {
        let llvm_ty = self.type_resolver.resolve(idx);
        self.builder.register_type(llvm_ty)
    }

    /// Allocate a heap cell via `ori_rc_alloc(size, align)`.
    ///
    /// Returns a `ptr` to the RC-managed allocation. Used for boxing
    /// recursive enum fields that must be stored as pointers in the payload.
    fn rc_alloc(&mut self, size: u64, align: u64) -> ValueId {
        let size_val = self.builder.const_i64(size as i64);
        let align_val = self.builder.const_i64(align as i64);
        let rc_alloc_func = self.builder.runtime_fn("ori_rc_alloc");
        self.builder
            .call(rc_alloc_func, &[size_val, align_val], "rc.alloc")
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
    fn var(&self, v: ArcVarId) -> ValueId {
        self.var_emitted(v).into_raw()
    }

    /// Look up the typed emitted value for an ARC variable.
    ///
    /// Returns the full [`EmittedValue`] including representation info.
    /// Prefer this over [`var`](Self::var) when the consumer needs to
    /// distinguish between value kinds (e.g., RC operations).
    fn var_emitted(&self, v: ArcVarId) -> EmittedValue {
        if let Some(Some(val)) = self.var_map.get(v.index()) {
            *val
        } else {
            tracing::error!(var = v.raw(), "ArcIrEmitter: variable not yet defined");
            EmittedValue::Immediate(ValueId::NONE)
        }
    }

    /// Bind an ARC variable to a typed LLVM value.
    fn def_var(&mut self, v: ArcVarId, val: EmittedValue) {
        let idx = v.index();
        if idx >= self.var_map.len() {
            self.var_map.resize(idx + 1, None);
        }
        self.var_map[idx] = Some(val);
    }

    /// Bind an ARC variable to a raw LLVM value, inferring its [`EmittedValue`]
    /// variant from the variable's [`ValueRepr`] in the ARC function.
    fn def_var_repr(&mut self, v: ArcVarId, val: ValueId, func: &ArcFunction) {
        let repr = func.var_repr(v).unwrap_or(ValueRepr::Scalar);
        self.def_var(v, EmittedValue::from_repr(repr, val));
    }

    /// Look up the LLVM block for an ARC block.
    ///
    /// Panics if the block is a dead unwind block (no LLVM block was created).
    fn block(&self, b: ori_arc::ir::ArcBlockId) -> BlockId {
        self.block_map[b.index()]
            .expect("block() called for dead unwind block — invariant violated")
    }

    /// Emit an entire `ArcFunction` as LLVM IR.
    ///
    /// Pre-creates all LLVM blocks, binds function parameters, emits each
    /// block's instructions and terminator, then patches phi nodes.
    pub fn emit_function(&mut self, func: &ArcFunction, abi: &FunctionAbi) {
        // Pre-scan: find dead unwind blocks. With nounwind analysis,
        // Invoke terminators calling known-nounwind functions are downgraded
        // to `call` + `br`, so their unwind blocks become dead code.
        // This must happen before block pre-creation so we can skip creating
        // LLVM basic blocks for dead blocks entirely.
        let mut all_invoke_unwind = rustc_hash::FxHashSet::default();
        let mut unwind_blocks = rustc_hash::FxHashSet::default();
        for block in &func.blocks {
            if let ArcTerminator::Invoke {
                unwind,
                func: callee,
                ..
            } = &block.terminator
            {
                all_invoke_unwind.insert(unwind.index());
                if !self.ctx.nounwind_functions.contains(callee) {
                    unwind_blocks.insert(unwind.index());
                }
            }
        }

        // Dead unwind blocks: targets only of nounwind Invokes (downgraded to call).
        // These blocks have no predecessors and must not be emitted.
        let dead_unwind: rustc_hash::FxHashSet<usize> = all_invoke_unwind
            .difference(&unwind_blocks)
            .copied()
            .collect();

        // Invariant: dead unwind blocks must not be reachable via non-Invoke edges.
        // If a Jump/Branch/Switch targets a dead block, the detection is broken.
        debug_assert!(
            {
                let mut ok = true;
                for block in &func.blocks {
                    let non_invoke_targets: Vec<usize> = match &block.terminator {
                        ArcTerminator::Jump { target, .. } => vec![target.index()],
                        ArcTerminator::Branch {
                            then_block,
                            else_block,
                            ..
                        } => {
                            vec![then_block.index(), else_block.index()]
                        }
                        ArcTerminator::Switch { cases, default, .. } => {
                            let mut t: Vec<usize> = cases.iter().map(|(_, b)| b.index()).collect();
                            t.push(default.index());
                            t
                        }
                        ArcTerminator::Invoke { normal, .. } => vec![normal.index()],
                        _ => vec![],
                    };
                    for target in non_invoke_targets {
                        if dead_unwind.contains(&target) {
                            ok = false;
                        }
                    }
                }
                ok
            },
            "dead unwind block is reachable via non-Invoke terminator — \
             dead_unwind detection invariant violated"
        );

        // Pre-create LLVM blocks, skipping dead unwind blocks
        self.block_map = func
            .blocks
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if dead_unwind.contains(&i) {
                    None
                } else {
                    let name = format!("bb{i}");
                    Some(self.builder.append_block(self.current_function, &name))
                }
            })
            .collect();

        // Resize var_map to hold all variables
        self.var_map.resize(func.var_types.len(), None);

        // Bind function parameters (respecting ABI passing modes).
        // Reference and Indirect params arrive as pointers — load the actual
        // value so ARC IR sees the struct, not the pointer.
        //
        // Non-capturing lambdas have a phantom `ptr %_env` prepended to their
        // LLVM param list (so they're directly callable as closures). Skip it
        // by adding 1 to the starting index.
        let sret_offset = u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));
        let phantom_env_offset = u32::from(self.ctx.non_capturing_lambdas.contains(&func.name));
        let needs_loads = abi.params.iter().any(|p| {
            matches!(
                p.passing,
                ParamPassing::Indirect { .. } | ParamPassing::Reference
            )
        });
        if needs_loads {
            // Position at entry block for load instructions
            self.builder.position_at_end(self.block(func.entry));
        }
        let mut llvm_param_idx = sret_offset + phantom_env_offset;
        for (i, param) in func.params.iter().enumerate() {
            let passing = &abi.params[i].passing;
            match passing {
                ParamPassing::Direct => {
                    let llvm_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    self.def_var_repr(param.var, llvm_param, func);
                    llvm_param_idx += 1;
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    let ptr_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    let ty = self.resolve_type(param.ty);
                    let loaded = self.builder.load(ty, ptr_param, "param.load");
                    self.def_var_repr(param.var, loaded, func);
                    llvm_param_idx += 1;
                }
                ParamPassing::Void => {
                    // No physical LLVM param — bind to a zero/unit constant
                    let zero = self.builder.const_i64(0);
                    self.def_var(param.var, EmittedValue::Immediate(zero));
                }
            }
        }

        // Set personality function on the LLVM function if any real invokes exist.
        // Required for any function containing `invoke`/`landingpad`.
        let personality_id = if unwind_blocks.is_empty() {
            None
        } else {
            let pid = self.builder.runtime_fn("rust_eh_personality");
            self.builder.set_personality(self.current_function, pid);
            Some(pid)
        };

        // Position at entry block
        let entry = self.block(func.entry);
        self.builder.position_at_end(entry);

        // Create phi nodes for blocks with parameters (skip dead unwind blocks)
        let mut phi_nodes: Vec<Vec<(ArcVarId, ValueId)>> = Vec::new();
        for block in &func.blocks {
            let mut block_phis = Vec::new();
            if !block.params.is_empty() && !dead_unwind.contains(&block.id.index()) {
                self.builder.position_at_end(self.block(block.id));
                for &(var, ty) in &block.params {
                    let llvm_ty = self.resolve_type(ty);
                    let phi_val = self.builder.phi(llvm_ty, &format!("v{}", var.raw()));
                    self.def_var_repr(var, phi_val, func);
                    block_phis.push((var, phi_val));
                }
            }
            phi_nodes.push(block_phis);
        }

        // Emit each block's body and terminator in Reverse Post-Order (RPO).
        //
        // RPO ensures that a block's dominator (and thus the variable definitions
        // from preceding blocks) is visited first. This is critical after
        // `expand_reuse`, which appends fast/slow/merge blocks at the end of the
        // block array — their Invoke terminators may target existing blocks with
        // lower array indices, creating forward references if emitted in array order.
        //
        // Dead unwind blocks are skipped entirely — no LLVM block was created.
        // Live unwind blocks start with a landing pad. Two flavors:
        // - **Cleanup** (terminator = Resume): `landingpad cleanup`, RC cleanup, resume
        // - **Catch** (terminator = Jump): `landingpad catch null`, free exception via
        //   `ori_catch_cleanup`, then jump to catch handler. Used by `catch(expr:)`.
        let rpo = compute_block_rpo(func, &dead_unwind);
        let mut landingpad_values: FxHashMap<usize, ValueId> = FxHashMap::default();
        for &block_idx in &rpo {
            let block = &func.blocks[block_idx];

            self.builder.position_at_end(self.block(block.id));

            // Live unwind blocks must start with a landingpad instruction
            if unwind_blocks.contains(&block.id.index()) {
                if let Some(pid) = personality_id {
                    let is_catch = !matches!(block.terminator, ArcTerminator::Resume);
                    if is_catch {
                        // Catch-all landing pad: catches the exception
                        let lp = self.builder.landingpad_catch_all(pid, "lp.catch");
                        landingpad_values.insert(block.id.index(), lp);

                        // Extract exception pointer and free via _Unwind_DeleteException
                        let exc_ptr = self.builder.extract_value(lp, 0, "exc.ptr");
                        if let Some(exc_ptr) = exc_ptr {
                            self.emit_catch_cleanup(exc_ptr);
                        }
                    } else {
                        // Cleanup landing pad: do RC cleanup, then re-raise
                        let lp = self.builder.landingpad(pid, true, "lp");
                        landingpad_values.insert(block.id.index(), lp);
                    }
                }
            }

            for (instr_idx, instr) in block.body.iter().enumerate() {
                self.current_block_idx = block_idx;
                self.current_instr_idx = instr_idx;
                self.emit_instr(instr, func);
            }
            // Set instruction index for terminator: one past the last body
            // instruction, matching the convention in compute_cow_annotations.
            self.current_instr_idx = block.body.len();
            self.emit_terminator(
                &block.terminator,
                block.id,
                &phi_nodes,
                abi,
                &landingpad_values,
                func,
            );
        }

        // Terminate blocks that RPO didn't visit (unreachable from entry).
        // These blocks were pre-created as LLVM blocks but never filled with
        // instructions. LLVM requires every block to have a terminator.
        {
            let visited: rustc_hash::FxHashSet<usize> = rpo.iter().copied().collect();
            for (i, llvm_block) in self.block_map.iter().enumerate() {
                if let Some(block_id) = llvm_block {
                    if !visited.contains(&i) {
                        self.builder.position_at_end(*block_id);
                        self.builder.unreachable();
                    }
                }
            }
        }

        // Patch phi incoming values
        for &(block_idx, param_idx, value, source_block) in &self.phi_incoming {
            let (_, phi_val) = phi_nodes[block_idx][param_idx];
            self.builder
                .add_phi_incoming(phi_val, &[(value, source_block)]);
        }
    }

    /// Emit a `Project` instruction (field extraction).
    ///
    /// For tagged union payload fields (Result, Enum), the LLVM storage type
    /// may differ from the expected type (e.g., `int` payload stored in a
    /// `{i64, i64, ptr}` slot of `Result<int, str>`). These use alloca + GEP + load
    /// for type-safe extraction through pointer reinterpretation.
    fn emit_project(
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
                super::type_info::TypeInfo::Result { .. } | super::type_info::TypeInfo::Enum { .. }
            ) {
                let is_general_enum =
                    matches!(val_type_info, super::type_info::TypeInfo::Enum { .. });
                let llvm_val_ty = self.resolve_type(val_ty);
                let alloca = self.builder.alloca(llvm_val_ty, "proj.alloca");
                self.builder.store(val, alloca);
                if is_general_enum {
                    // General enum: payload is [M x i64] at struct field 1.
                    // Index into the payload array with i64-stride GEP.
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

    // -----------------------------------------------------------------------
    // Instruction emission
    // -----------------------------------------------------------------------

    /// Emit a single `ArcInstr` as LLVM IR.
    fn emit_instr(&mut self, instr: &ArcInstr, func: &ArcFunction) {
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

#[cfg(test)]
mod tests;
