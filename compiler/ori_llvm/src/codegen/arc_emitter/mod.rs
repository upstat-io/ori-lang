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

mod builtins;
mod closures;
mod construction;
mod context;
mod drop_gen;
mod operators;
mod rc_helpers;
mod rc_ops;
mod terminators;

pub use context::CodegenContext;
use context::{is_boxed_enum_field, EmittedValue, InvokeMode};

use ori_arc::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue, PrimOp, ValueRepr,
};
use ori_arc::ArcClassification;
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use super::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

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
            current_function,
            ctx,
            partial_apply_counter: 0,
            var_map: Vec::new(),
            block_map: Vec::new(),
            phi_incoming: Vec::new(),
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

    /// Get or generate the drop function for a type.
    ///
    /// Returns a function pointer `ValueId` suitable for passing to
    /// `ori_rc_dec`. Returns null for scalar types or when no classifier
    /// is available (no drop needed).
    ///
    /// Drop functions are cached per type. For recursive types, the
    /// `FunctionId` is cached **before** body generation to break cycles.
    fn get_or_generate_drop_fn(&mut self, ty: Idx) -> ValueId {
        // Fast path: already generated
        if let Some(&func_id) = self.drop_fn_cache.get(&ty) {
            return self.builder.get_function_ptr(func_id);
        }

        // Compute what drop operations this type needs
        let Some(drop_info) = ori_arc::compute_drop_info(ty, self.classifier, self.pool) else {
            return self.builder.const_null_ptr();
        };

        // Save current builder position (we're about to create a new function)
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Generate the drop function (handles declaration, caching, and body)
        let func_id = drop_gen::generate_drop_fn(self, ty, &drop_info);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(func_id)
    }

    // -----------------------------------------------------------------------
    // Top-level emission
    // -----------------------------------------------------------------------

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

        // Emit each block's body and terminator.
        // Dead unwind blocks are skipped entirely — no LLVM block was created.
        // Live unwind blocks start with a landing pad. Two flavors:
        // - **Cleanup** (terminator = Resume): `landingpad cleanup`, RC cleanup, resume
        // - **Catch** (terminator = Jump): `landingpad catch null`, free exception via
        //   `ori_catch_cleanup`, then jump to catch handler. Used by `catch(expr:)`.
        let mut landingpad_values: FxHashMap<usize, ValueId> = FxHashMap::default();
        for block in &func.blocks {
            // Dead unwind block: no LLVM block exists, skip entirely
            if dead_unwind.contains(&block.id.index()) {
                continue;
            }

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

            for instr in &block.body {
                self.emit_instr(instr, func);
            }
            self.emit_terminator(
                &block.terminator,
                block.id,
                &phi_nodes,
                abi,
                &landingpad_values,
                func,
            );
        }

        // Patch phi incoming values
        for &(block_idx, param_idx, value, source_block) in &self.phi_incoming {
            let (_, phi_val) = phi_nodes[block_idx][param_idx];
            self.builder
                .add_phi_incoming(phi_val, &[(value, source_block)]);
        }
    }

    /// Emit either LLVM `invoke` or `call` + `br` based on [`InvokeMode`].
    ///
    /// - `InvokeMode::Invoke`: emits `invoke` with normal + unwind continuations
    /// - `InvokeMode::Call`: emits `call` + unconditional `br` to normal block
    fn call_or_invoke_llvm(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        mode: InvokeMode,
        name: &str,
    ) -> Option<ValueId> {
        match mode {
            InvokeMode::Call { normal } => {
                let result = self.builder.call(func_id, args, name);
                self.builder.br(normal);
                result
            }
            InvokeMode::Invoke { normal, unwind } => {
                self.builder.invoke(func_id, args, normal, unwind, name)
            }
        }
    }

    /// Look up a method function using the first arg's type as a receiver.
    ///
    /// Derived methods (e.g., `compare`, `eq`, `clone`) in ARC IR use unqualified
    /// names. When two types derive the same trait, the unqualified lookup is
    /// ambiguous. This method uses the first arg's type index to resolve the
    /// correct type-qualified entry in `method_functions`.
    fn lookup_method_by_receiver(
        &self,
        name: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let &first_arg = args.first()?;
        let receiver_ty = func.var_type(first_arg);
        let type_name = self.ctx.type_idx_to_name.get(&receiver_ty)?;
        self.ctx.method_functions.get(&(*type_name, name))
    }

    /// Look up a static/associated method by its return type.
    ///
    /// Type-qualified calls with no receiver (e.g., `Point.default()`) have an
    /// empty `args` list in ARC IR, so `lookup_method_by_receiver` fails.
    /// For factory methods like `default()`, the return type IS the owning type,
    /// so we can use `func.var_type(dst)` to find the correct type-qualified
    /// entry in `method_functions`.
    fn lookup_method_by_return_type(
        &self,
        name: Name,
        dst: ArcVarId,
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let return_ty = func.var_type(dst);
        let type_name = self.ctx.type_idx_to_name.get(&return_ty)?;
        self.ctx.method_functions.get(&(*type_name, name))
    }

    /// Diagnostic check for method lookup when all typed dispatches miss.
    ///
    /// Always returns `None` — this function only logs diagnostics.
    /// If a method exists in `method_functions` but wasn't found through
    /// normal dispatch, it means the receiver's type wasn't registered in
    /// `type_idx_to_name` (e.g., enum types whose derives aren't compiled yet).
    /// Returning `None` ensures the caller falls through to the "unresolved
    /// function" error path instead of silently calling the wrong method.
    fn lookup_method_fallback(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        let exists = self
            .ctx
            .method_functions
            .iter()
            .any(|((_, method_name), _)| *method_name == name);
        if exists {
            tracing::warn!(
                method = %self.interner.lookup(name),
                "method exists for another type but receiver type not registered — \
                 likely missing enum derive codegen"
            );
        }
        None
    }

    /// Resolve a generic function call to its monomorphized variant.
    ///
    /// The ARC IR uses the **original** generic name (e.g., `"identity"`),
    /// but the LLVM function was declared under the **mangled** name
    /// (e.g., `"identity$m$int"`). This method matches the concrete argument
    /// types at the call site to find the correct monomorphization.
    fn lookup_mono_dispatch(
        &self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let entries = self.ctx.mono_dispatch.get(&callee)?;
        let arg_types: Vec<Idx> = args
            .iter()
            .map(|a| self.pool.resolve_fully(func.var_type(*a)))
            .collect();
        entries
            .iter()
            .find(|(params, _)| {
                params.len() == arg_types.len()
                    && params
                        .iter()
                        .zip(&arg_types)
                        .all(|(p, a)| self.pool.resolve_fully(*p) == *a)
            })
            .and_then(|(_, mangled)| self.ctx.functions.get(mangled))
    }

    /// Emit an `Apply` instruction (ABI-aware direct call).
    fn emit_apply(&mut self, dst: ArcVarId, callee: Name, args: &[ArcVarId], func: &ArcFunction) {
        let callee_name_str = self.interner.lookup(callee);

        // Internal protocol: __iter_next(iter, elem_ty_marker).
        // args[0] = iterator pointer, args[1] = zero marker carrying elem_ty.
        // Result type is INT (no RC management); actual element type comes
        // from the marker argument.
        if callee_name_str == "__iter_next" && args.len() >= 2 {
            let iter_ptr = self.var(args[0]);
            let elem_ty = func.var_type(args[1]);
            if let Some(val) = self.emit_iter_next(iter_ptr, elem_ty) {
                self.def_var_repr(dst, val, func);
            }
            return;
        }

        // ori_list_take uses explicit sret pattern: void(list_ptr, out_ptr).
        // The ARC IR emits Apply "ori_list_take"(list_ptr) expecting a struct return.
        // We handle the sret plumbing here: alloca result struct, call, load.
        if callee_name_str == "ori_list_take" && !args.is_empty() {
            if let Some(val) = self.emit_list_take(args[0], func) {
                self.def_var_repr(dst, val, func);
            }
            return;
        }

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if let Some(val) = self.try_emit_format_call(callee_name_str, args, func) {
            self.def_var_repr(dst, val, func);
            return;
        }

        // Prelude builtin functions (str, int, float, byte, hash_combine, etc.)
        if let Some(val) =
            builtins::prelude::try_emit_prelude_function(self, callee_name_str, args, func)
        {
            self.def_var_repr(dst, val, func);
            return;
        }

        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        // Method dispatch chain (same as emit_invoke):
        // 1. Receiver-based: use first arg's type (instance methods)
        // 2. Return-type-based: use dst's type (static methods like default)
        // 3. Unqualified: bare function name (free functions)
        // 4. Monomorphized generic: match arg types → mangled specialization
        // 5. Diagnostic fallback: logs warning, returns None
        let resolved = self
            .lookup_method_by_receiver(callee, args, func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, args, func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi.clone()));

        let result = if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing(&arg_vals, &params);
            match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    self.call_with_sret(func_id, &passed_args, ret_ty, "call")
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.builder.call(func_id, &passed_args, "call")
                }
            }
        } else if let Some(val) = self.try_emit_builtin_method(callee, args, func) {
            Some(val)
        } else if let Some(func_id) = self.builder.try_runtime_fn(callee_name_str) {
            // Runtime function fallback: coerce aggregate args to pointers.
            // Runtime functions (ori_print, ori_str_*, etc.) take ptr params,
            // but ARC IR passes aggregate structs (Str, List, etc.) by value.
            let is_list_push = callee_name_str == "ori_list_push";
            let coerced_args: Vec<ValueId> = args
                .iter()
                .zip(arg_vals.iter())
                .enumerate()
                .map(|(i, (arc_var, &val))| {
                    let arg_ty = func.var_type(*arc_var);
                    if is_list_push && i == 1 {
                        // ori_list_push(list_ptr, elem_ptr, elem_size):
                        // arg[1] is the element value that must be coerced
                        // to a pointer regardless of its type (even scalars).
                        self.coerce_any_to_ptr(val, arg_ty)
                    } else {
                        self.coerce_aggregate_to_ptr(val, arg_ty)
                    }
                })
                .collect();
            self.builder.call(func_id, &coerced_args, "call")
        } else {
            let msg = format!(
                "unresolved function `{callee_name_str}` in apply — missing mono instance?"
            );
            tracing::warn!("{msg}");
            self.builder.record_codegen_error_with_msg(msg);
            None
        };

        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        }
    }

    /// Emit an `ApplyIndirect` instruction (indirect call through closure).
    fn emit_apply_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let closure_val = self.var(closure);
        tracing::trace!(
            ?ty,
            tag = ?self.pool.tag(ty),
            closure_var = closure.raw(),
            args = args.len(),
            "emit_apply_indirect"
        );
        let fn_ptr = self.builder.extract_value(closure_val, 0, "closure.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "closure.env_ptr");

        if let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) {
            let ptr_ty = self.builder.ptr_type();
            let mut arg_vals = Vec::with_capacity(1 + args.len());
            let mut param_types = Vec::with_capacity(1 + args.len());
            arg_vals.push(env_ptr);
            param_types.push(ptr_ty);

            for &a in args {
                let arg_ty = func.var_type(a);
                let passing = super::abi::compute_param_passing(arg_ty, self.type_info);
                match passing {
                    super::abi::ParamPassing::Indirect { .. }
                    | super::abi::ParamPassing::Reference => {
                        // Large struct: alloca, store, pass pointer
                        let llvm_ty = self.resolve_type(arg_ty);
                        let alloca = self.builder.alloca(llvm_ty, "icall.arg.tmp");
                        self.builder.store(self.var(a), alloca);
                        arg_vals.push(alloca);
                        param_types.push(ptr_ty);
                    }
                    super::abi::ParamPassing::Void => {}
                    super::abi::ParamPassing::Direct => {
                        arg_vals.push(self.var(a));
                        param_types.push(self.resolve_type(arg_ty));
                    }
                }
            }

            let ret_ty = self.resolve_type(ty);
            tracing::trace!(
                ?ty,
                resolved_llvm_ty = ?self.builder.arena.get_type(ret_ty),
                "emit_apply_indirect: resolved return type"
            );
            if let Some(val) =
                self.builder
                    .call_indirect(ret_ty, &param_types, fn_ptr, &arg_vals, "icall")
            {
                self.def_var_repr(dst, val, func);
            }
        } else {
            tracing::error!(
                closure_var = closure.raw(),
                "emit_apply_indirect: extract_value failed — fn_ptr or env_ptr is None"
            );
        }
    }

    /// Emit a `Project` instruction (field extraction).
    ///
    /// For tagged union payload fields (Result, Enum), the LLVM storage type
    /// may differ from the expected type (e.g., `int` payload stored in a
    /// `{i64, ptr}` slot of `Result<int, str>`). These use alloca + GEP + load
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
                // Construct. If Reuse appears (e.g., expansion was skipped), fall
                // back to fresh construction.
                tracing::debug!("ArcIrEmitter: Reuse instruction not expanded — using Construct");
                let val = self.emit_construct(*ty, ctor, args);
                self.def_var_repr(*dst, val, func);
                let _ = token;
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

    // -----------------------------------------------------------------------
    // Value emission (for ArcValue in Let instructions)
    // -----------------------------------------------------------------------

    /// Emit an `ArcValue` as an LLVM value.
    fn emit_value(&mut self, value: &ArcValue, ty: Idx, func: &ArcFunction) -> ValueId {
        match value {
            ArcValue::Var(v) => self.var(*v),

            ArcValue::Literal(lit) => self.emit_literal(lit),

            ArcValue::PrimOp { op, args } => {
                let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
                self.emit_primop(*op, &arg_vals, ty, func, args)
            }
        }
    }

    /// Emit a literal value.
    fn emit_literal(&mut self, lit: &LitValue) -> ValueId {
        match lit {
            LitValue::Int(n) => self.builder.const_i64(*n),
            LitValue::Float(bits) => self.builder.const_f64(f64::from_bits(*bits)),
            LitValue::Bool(b) => self.builder.const_bool(*b),
            LitValue::Char(c) => self.builder.const_i32(*c as i32),
            LitValue::Unit => self.builder.const_i64(0),
            LitValue::String(name) => {
                let s = self.interner.lookup(*name);
                // Use ori_str_from_raw to create an RC-managed heap copy of
                // the string literal. This ensures the data pointer has a
                // valid RC header for ARC RcInc/RcDec operations.
                let global = self.builder.build_global_string_ptr(s, "str");
                let len = self.builder.const_i64(s.len() as i64);
                let func_id = self.builder.runtime_fn("ori_str_from_raw");
                self.builder
                    .call(func_id, &[global, len], "str.val")
                    .unwrap_or_else(|| {
                        // Fallback: build inline struct (no RC safety)
                        let str_ty = self.builder.register_type(
                            self.builder
                                .scx()
                                .type_struct(
                                    &[
                                        self.builder.scx().type_i64().into(),
                                        self.builder.scx().type_ptr().into(),
                                    ],
                                    false,
                                )
                                .into(),
                        );
                        self.builder.build_struct(str_ty, &[len, global], "str.val")
                    })
            }
            LitValue::Duration { value, unit } => {
                let nanos = unit.to_nanos(*value);
                self.builder.const_i64(nanos)
            }
            LitValue::Size { value, unit } => {
                let bytes = unit.to_bytes(*value);
                self.builder.const_i64(bytes as i64)
            }
        }
    }

    /// Emit a primitive operation.
    fn emit_primop(
        &mut self,
        op: PrimOp,
        arg_vals: &[ValueId],
        _ty: Idx,
        func: &ArcFunction,
        arc_args: &[ArcVarId],
    ) -> ValueId {
        match op {
            PrimOp::Binary(bin_op) => {
                let lhs = arg_vals[0];
                let rhs = arg_vals[1];
                let lhs_ty = func.var_type(arc_args[0]);
                self.emit_binary_op(bin_op, lhs, rhs, lhs_ty)
            }
            PrimOp::Unary(un_op) => {
                let operand = arg_vals[0];
                let operand_ty = func.var_type(arc_args[0]);
                self.emit_unary_op(un_op, operand, operand_ty)
            }
        }
    }

    // -----------------------------------------------------------------------
    // ABI helpers
    // -----------------------------------------------------------------------

    /// Apply parameter passing modes to argument values.
    ///
    /// Apply param passing: `Indirect`/`Reference` (alloca+store+pass ptr),
    /// `Direct` (pass through), `Void` (skip).
    fn apply_param_passing(
        &mut self,
        args: &[ValueId],
        params: &[super::abi::ParamAbi],
    ) -> Vec<ValueId> {
        let mut result = Vec::with_capacity(args.len());
        let mut arg_idx = 0;

        for param_abi in params {
            if arg_idx >= args.len() {
                break;
            }

            match &param_abi.passing {
                super::abi::ParamPassing::Indirect { .. } | super::abi::ParamPassing::Reference => {
                    let param_ty = self.resolve_type(param_abi.ty);
                    let alloca = self.builder.create_entry_alloca(
                        self.current_function,
                        "ref_arg",
                        param_ty,
                    );
                    self.builder.store(args[arg_idx], alloca);
                    result.push(alloca);
                    arg_idx += 1;
                }
                super::abi::ParamPassing::Direct => {
                    result.push(args[arg_idx]);
                    arg_idx += 1;
                }
                super::abi::ParamPassing::Void => {
                    // Void params are not physically passed — skip
                }
            }
        }

        // Pass remaining args directly (shouldn't happen in well-typed code)
        while arg_idx < args.len() {
            result.push(args[arg_idx]);
            arg_idx += 1;
        }

        result
    }

    /// Call a function with sret (struct return via hidden pointer).
    fn call_with_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        ret_ty: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let sret_alloca = self.builder.alloca(ret_ty, "sret.tmp");
        let mut full_args = Vec::with_capacity(1 + args.len());
        full_args.push(sret_alloca);
        full_args.extend_from_slice(args);
        self.builder.call(func_id, &full_args, name);
        Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
    }

    // -----------------------------------------------------------------------
    // List take (sret helper for for-yield finalization)
    // -----------------------------------------------------------------------

    /// Emit `ori_list_take(list_ptr, out_ptr)` with manual sret handling.
    ///
    /// `ori_list_take` uses an explicit sret pattern: `void(ptr list, ptr out)`.
    /// We alloca a `{i64, i64, ptr}` result, call the function, then load.
    fn emit_list_take(&mut self, list_var: ArcVarId, _func: &ArcFunction) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_take");
        let list_ptr = self.var(list_var);

        // Alloca {i64, i64, ptr} for the result
        let list_struct_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            "list_take.out",
            list_struct_ty,
        );

        // Call ori_list_take(list_ptr, out_alloca) — void return
        self.builder
            .call(func_id, &[list_ptr, out_alloca], "list_take");

        // Load the result struct from the alloca
        Some(
            self.builder
                .load(list_struct_ty, out_alloca, "list_take.val"),
        )
    }

    // -----------------------------------------------------------------------
    // Aggregate-to-pointer coercion
    // -----------------------------------------------------------------------

    // Catch(expr:) EH helpers

    /// Emit `ori_catch_cleanup(exc_ptr)` to free a caught Rust exception.
    ///
    /// Calls `_Unwind_DeleteException` via the runtime wrapper, which invokes
    /// the cleanup callback in the Itanium ABI `_Unwind_Exception` header.
    /// This properly frees the Rust-allocated panic payload without requiring
    /// C++ EH ABI functions (`__cxa_begin_catch`/`__cxa_end_catch`), which
    /// are incompatible with Rust's panic infrastructure.
    ///
    /// Called in catch-style unwind blocks right after the landing pad,
    /// before any RC cleanup or catch handler logic.
    fn emit_catch_cleanup(&mut self, exc_ptr: ValueId) {
        let func_id = self.builder.runtime_fn("ori_catch_cleanup");
        self.builder.call(func_id, &[exc_ptr], "");
    }

    /// Coerce an aggregate value to a pointer for runtime function calls.
    ///
    /// Runtime functions like `ori_print` expect `ptr` arguments (pointers to
    /// structs), but ARC IR passes aggregate values directly. When we detect
    /// that a call arg is an aggregate but the callee expects `ptr`, we
    /// alloca+store+pass the pointer.
    fn coerce_aggregate_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let tag = self.pool.tag(ty);
        match tag {
            Tag::Str | Tag::List | Tag::Set | Tag::Map => {
                let llvm_ty = self.resolve_type(ty);
                let alloca =
                    self.builder
                        .create_entry_alloca(self.current_function, "rt_arg", llvm_ty);
                self.builder.store(val, alloca);
                alloca
            }
            _ => val,
        }
    }

    /// Coerce any value (including scalars) to a pointer via alloca+store.
    ///
    /// Unlike `coerce_aggregate_to_ptr` which only handles struct types,
    /// this works for ALL types. Used by `ori_list_push` which needs a
    /// `*const u8` pointer to any element's bytes.
    fn coerce_any_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let llvm_ty = self.resolve_type(ty);
        let alloca = self
            .builder
            .create_entry_alloca(self.current_function, "elem_arg", llvm_ty);
        self.builder.store(val, alloca);
        alloca
    }

    // -----------------------------------------------------------------------
    // String runtime call helpers
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // hash_combine (free function)
    // -----------------------------------------------------------------------

    /// Emit `hash_combine(a, b)` inline using the boost `hash_combine` pattern.
    ///
    /// `a ^ (b + 0x9e3779b9 + (a << 6) + (a >> 2))`
    pub(crate) fn emit_hash_combine(&mut self, a: ValueId, b: ValueId) -> ValueId {
        let magic = self.builder.const_i64(0x9e37_79b9_i64);
        let six = self.builder.const_i64(6);
        let two = self.builder.const_i64(2);

        let a_shl6 = self.builder.shl(a, six, "hc.shl");
        let a_shr2 = self.builder.ashr(a, two, "hc.shr");
        let sum1 = self.builder.add(b, magic, "hc.sum1");
        let sum2 = self.builder.add(sum1, a_shl6, "hc.sum2");
        let sum3 = self.builder.add(sum2, a_shr2, "hc.sum3");
        self.builder.xor(a, sum3, "hc.result")
    }

    // -----------------------------------------------------------------------
    // Format call decomposition
    // -----------------------------------------------------------------------

    /// Intercept `ori_format_*` calls and decompose the string spec argument.
    ///
    /// ARC IR emits `Apply("ori_format_int", [val, spec_str])` with 2 args.
    /// Runtime expects `ori_format_int(val, spec_ptr, spec_len)` — 3 args.
    /// The `spec_str` is `{i64 len, ptr data}` that needs decomposition.
    fn try_emit_format_call(
        &mut self,
        callee_name: &str,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.len() < 2 {
            return None;
        }

        let func_id = match callee_name {
            "ori_format_int" => self.builder.runtime_fn("ori_format_int"),
            "ori_format_float" => self.builder.runtime_fn("ori_format_float"),
            "ori_format_str" => self.builder.runtime_fn("ori_format_str"),
            "ori_format_bool" => self.builder.runtime_fn("ori_format_bool"),
            "ori_format_char" => self.builder.runtime_fn("ori_format_char"),
            _ => return None,
        };

        // args[0] = the value to format
        let value = self.var(args[0]);
        // args[1] = spec string {i64 len, ptr data}
        let spec_str = self.var(args[1]);

        // For ori_format_str, the value arg is also a string struct — coerce to ptr.
        let value_arg = if callee_name == "ori_format_str" {
            let val_ty = func.var_type(args[0]);
            self.coerce_aggregate_to_ptr(value, val_ty)
        } else {
            value
        };

        // Decompose spec string: extract len (field 0) and data (field 1)
        let spec_len = self.builder.extract_value(spec_str, 0, "fmt.spec_len")?;
        let spec_ptr = self.builder.extract_value(spec_str, 1, "fmt.spec_ptr")?;

        // Call runtime: ori_format_*(value, spec_ptr, spec_len)
        self.builder
            .call(func_id, &[value_arg, spec_ptr, spec_len], "fmt")
    }

    // -----------------------------------------------------------------------
    // String runtime call helpers
    // -----------------------------------------------------------------------

    /// Call a string runtime function: `ori_str_concat`, `ori_str_eq`, `ori_str_ne`.
    ///
    /// String values are `{ i64, ptr }` structs passed by pointer to the runtime.
    /// `returns_str` controls the return type: `true` → `{ i64, ptr }`, `false` → `i1`.
    fn emit_str_runtime_call(
        &mut self,
        func_name: &'static str,
        lhs: ValueId,
        rhs: ValueId,
        returns_str: bool,
    ) -> ValueId {
        let func_id = self.builder.runtime_fn(func_name);

        // Alloca + store both operands (runtime takes pointers to string structs)
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        let result = self.builder.call(func_id, &[lhs_ptr, rhs_ptr], func_name);

        if returns_str {
            // ori_str_concat returns { i64, ptr } — load it from the alloca
            result.unwrap_or_else(|| {
                tracing::warn!("ArcIrEmitter: string runtime call returned no value");
                self.builder.const_i64(0)
            })
        } else {
            // ori_str_eq / ori_str_ne return i1 (bool)
            result.unwrap_or_else(|| self.builder.const_bool(false))
        }
    }
}

#[cfg(test)]
mod tests;
