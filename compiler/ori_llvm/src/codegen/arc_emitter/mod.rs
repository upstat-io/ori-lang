//! ARC IR → LLVM IR emitter (Tier 2 codegen).
//!
//! Translates `ArcFunction` basic blocks and instructions directly to LLVM IR,
//! including RC operations (`ori_rc_inc`, `ori_rc_dec`) and structured cleanup
//! via `invoke`/`landingpad`.
//!
//! This runs **alongside** Tier 1 (`ExprLowerer`), not replacing it.
//! Tier 1 compiles `CanExpr` → LLVM IR without RC. Tier 2 compiles
//! `CanExpr` → ARC IR → LLVM IR with RC lifecycle.
//!
//! # Architecture
//!
//! ```text
//! Tier 1:  CanExpr  →  ExprLowerer  →  LLVM IR  (no RC)
//! Tier 2:  CanExpr  →  ARC IR  →  ArcIrEmitter  →  LLVM IR  (with RC)
//! ```

mod drop_gen;

use ori_arc::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue, PrimOp,
};
use ori_arc::ArcClassification;
use ori_ir::{BinaryOp, Name, StringInterner, UnaryOp};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::abi::{FunctionAbi, ReturnPassing};
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
    /// `None` when ARC codegen is disabled (Tier 1 path).
    classifier: Option<&'a dyn ArcClassification>,
    /// Cache: type `Idx` → already-generated drop function `FunctionId`.
    /// Avoids regenerating drop functions for the same type and handles
    /// recursive types (entry inserted before body generation).
    drop_fn_cache: FxHashMap<Idx, FunctionId>,
    /// The LLVM function being compiled.
    current_function: FunctionId,
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    functions: &'a FxHashMap<Name, (FunctionId, FunctionAbi)>,
    /// Type-qualified method lookup: `(type_name, method_name)` → (`FunctionId`, ABI).
    method_functions: &'a FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
    /// Maps receiver type `Idx` → type `Name` for operator trait dispatch.
    type_idx_to_name: &'a FxHashMap<Idx, Name>,
    /// ARC variable → LLVM value mapping.
    var_map: Vec<Option<ValueId>>,
    /// ARC block → LLVM block mapping.
    block_map: Vec<BlockId>,
    /// Deferred phi incoming values: `block_index` → `[(param_index, value, source_block)]`.
    /// Collected during terminator emission, applied after all blocks are emitted.
    phi_incoming: Vec<(usize, usize, ValueId, BlockId)>,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new ARC IR emitter.
    #[allow(
        clippy::too_many_arguments,
        reason = "ARC emitter needs all codegen contexts; grouping would add indirection"
    )]
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'a Pool,
        classifier: Option<&'a dyn ArcClassification>,
        current_function: FunctionId,
        functions: &'a FxHashMap<Name, (FunctionId, FunctionAbi)>,
        method_functions: &'a FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
        type_idx_to_name: &'a FxHashMap<Idx, Name>,
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
            functions,
            method_functions,
            type_idx_to_name,
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

    /// Look up the LLVM value for an ARC variable.
    ///
    /// Returns `ValueId::NONE` and logs a warning if the variable is not yet
    /// defined — this is an internal invariant violation but should not crash
    /// the compiler. The malformed IR will be caught by `codegen_error_count`.
    fn var(&self, v: ArcVarId) -> ValueId {
        if let Some(Some(val)) = self.var_map.get(v.index()) {
            *val
        } else {
            tracing::error!(var = v.raw(), "ArcIrEmitter: variable not yet defined");
            ValueId::NONE
        }
    }

    /// Bind an ARC variable to an LLVM value.
    fn def_var(&mut self, v: ArcVarId, val: ValueId) {
        let idx = v.index();
        if idx >= self.var_map.len() {
            self.var_map.resize(idx + 1, None);
        }
        self.var_map[idx] = Some(val);
    }

    /// Look up the LLVM block for an ARC block.
    fn block(&self, b: ori_arc::ir::ArcBlockId) -> BlockId {
        self.block_map[b.index()]
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

        // No classifier → no drop analysis possible
        let Some(classifier) = self.classifier else {
            return self.builder.const_null_ptr();
        };

        // Compute what drop operations this type needs
        let Some(drop_info) = ori_arc::compute_drop_info(ty, classifier, self.pool) else {
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
        // Pre-create all LLVM blocks
        self.block_map = func
            .blocks
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let name = format!("bb{i}");
                self.builder.append_block(self.current_function, &name)
            })
            .collect();

        // Resize var_map to hold all variables
        self.var_map.resize(func.var_types.len(), None);

        // Bind function parameters
        let sret_offset = u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));
        for (i, param) in func.params.iter().enumerate() {
            let llvm_param = self
                .builder
                .get_param(self.current_function, i as u32 + sret_offset);
            self.def_var(param.var, llvm_param);
        }

        // Pre-scan: find blocks that are unwind destinations of Invoke terminators.
        // These blocks must start with a `landingpad` instruction per LLVM requirements.
        let mut unwind_blocks = rustc_hash::FxHashSet::default();
        for block in &func.blocks {
            if let ArcTerminator::Invoke { unwind, .. } = &block.terminator {
                unwind_blocks.insert(unwind.index());
            }
        }

        // Set personality function on the LLVM function if any invokes exist.
        // Required for any function containing `invoke`/`landingpad`.
        let personality_id = if unwind_blocks.is_empty() {
            None
        } else {
            self.builder
                .scx()
                .llmod
                .get_function("rust_eh_personality")
                .map(|f| {
                    let pid = self.builder.intern_function(f);
                    self.builder.set_personality(self.current_function, pid);
                    pid
                })
        };

        // Position at entry block
        let entry = self.block(func.entry);
        self.builder.position_at_end(entry);

        // Create phi nodes for blocks with parameters
        let mut phi_nodes: Vec<Vec<(ArcVarId, ValueId)>> = Vec::new();
        for block in &func.blocks {
            let mut block_phis = Vec::new();
            if !block.params.is_empty() {
                self.builder.position_at_end(self.block(block.id));
                for &(var, ty) in &block.params {
                    let llvm_ty = self.resolve_type(ty);
                    let phi_val = self.builder.phi(llvm_ty, &format!("v{}", var.raw()));
                    self.def_var(var, phi_val);
                    block_phis.push((var, phi_val));
                }
            }
            phi_nodes.push(block_phis);
        }

        // Emit each block's body and terminator.
        // For unwind blocks: emit `landingpad cleanup` as the first instruction,
        // then any cleanup instructions, then `resume` at the terminator.
        let mut landingpad_values: FxHashMap<usize, ValueId> = FxHashMap::default();
        for block in &func.blocks {
            self.builder.position_at_end(self.block(block.id));

            // Unwind blocks must start with a landingpad instruction
            if unwind_blocks.contains(&block.id.index()) {
                if let Some(pid) = personality_id {
                    let lp = self.builder.landingpad(pid, true, "lp");
                    landingpad_values.insert(block.id.index(), lp);
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
            );
        }

        // Patch phi incoming values
        for &(block_idx, param_idx, value, source_block) in &self.phi_incoming {
            let (_, phi_val) = phi_nodes[block_idx][param_idx];
            self.builder
                .add_phi_incoming(phi_val, &[(value, source_block)]);
        }
    }

    // -----------------------------------------------------------------------
    // Terminator emission
    // -----------------------------------------------------------------------

    /// Emit an `ArcTerminator` as LLVM control flow.
    fn emit_terminator(
        &mut self,
        term: &ArcTerminator,
        current_block: ori_arc::ir::ArcBlockId,
        _phi_nodes: &[Vec<(ArcVarId, ValueId)>],
        abi: &FunctionAbi,
        landingpad_values: &FxHashMap<usize, ValueId>,
    ) {
        match term {
            ArcTerminator::Return { value } => {
                let val = self.var(*value);
                match &abi.return_abi.passing {
                    ReturnPassing::Sret { .. } => {
                        let sret_ptr = self.builder.get_param(self.current_function, 0);
                        self.builder.store(val, sret_ptr);
                        self.builder.ret_void();
                    }
                    ReturnPassing::Direct => {
                        self.builder.ret(val);
                    }
                    ReturnPassing::Void => {
                        self.builder.ret_void();
                    }
                }
            }

            ArcTerminator::Jump { target, args } => {
                // Record phi incoming values for the target block's parameters
                let target_idx = target.index();
                if !args.is_empty() {
                    let Some(source_block) = self.builder.current_block() else {
                        tracing::error!("ARC jump: no current block — skipping phi incoming");
                        self.builder.record_codegen_error();
                        self.builder.br(self.block(*target));
                        return;
                    };
                    for (i, &arg) in args.iter().enumerate() {
                        let val = self.var(arg);
                        self.phi_incoming.push((target_idx, i, val, source_block));
                    }
                }
                self.builder.br(self.block(*target));
            }

            ArcTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = self.var(*cond);
                self.builder
                    .cond_br(cond_val, self.block(*then_block), self.block(*else_block));
            }

            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => {
                let scrut_val = self.var(*scrutinee);
                let llvm_cases: Vec<(ValueId, BlockId)> = cases
                    .iter()
                    .map(|&(tag, block_id)| {
                        let tag_val = self.builder.const_i64(tag as i64);
                        (tag_val, self.block(block_id))
                    })
                    .collect();
                self.builder
                    .switch(scrut_val, self.block(*default), &llvm_cases);
            }

            ArcTerminator::Invoke {
                dst,
                ty: _,
                func,
                args,
                normal,
                unwind,
            } => self.emit_invoke(*dst, *func, args, *normal, *unwind),

            ArcTerminator::Resume => {
                // Re-raise the caught exception using the landingpad token
                // captured at the start of this unwind block.
                if let Some(&lp_val) = landingpad_values.get(&current_block.index()) {
                    self.builder.resume(lp_val);
                } else {
                    // No landingpad for this block — should not happen if ARC IR
                    // is well-formed, but emit unreachable as a safety fallback.
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without landingpad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }

            ArcTerminator::Unreachable => {
                self.builder.unreachable();
            }
        }
    }

    /// Emit an `Invoke` terminator (ABI-aware function call with unwind).
    fn emit_invoke(
        &mut self,
        dst: ArcVarId,
        func: Name,
        args: &[ArcVarId],
        normal: ori_arc::ir::ArcBlockId,
        unwind: ori_arc::ir::ArcBlockId,
    ) {
        let func_name_str = self.interner.lookup(func);
        let normal_block = self.block(normal);
        let unwind_block = self.block(unwind);
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        if let Some(&(func_id, ref func_abi)) = self.functions.get(&func) {
            let result = match &func_abi.return_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(func_abi.return_abi.ty);
                    let sret_alloca = self.builder.alloca(ret_ty, "sret.tmp");
                    let mut full_args = vec![sret_alloca];
                    full_args.extend_from_slice(&arg_vals);
                    self.builder
                        .invoke(func_id, &full_args, normal_block, unwind_block, "invoke");
                    self.builder.position_at_end(normal_block);
                    Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.builder
                        .invoke(func_id, &arg_vals, normal_block, unwind_block, "invoke")
                }
            };
            if let Some(val) = result {
                self.def_var(dst, val);
            }
        } else if let Some(llvm_func) = self.builder.scx().llmod.get_function(func_name_str) {
            let func_id = self.builder.intern_function(llvm_func);
            if let Some(val) =
                self.builder
                    .invoke(func_id, &arg_vals, normal_block, unwind_block, "invoke")
            {
                self.def_var(dst, val);
            }
        } else {
            tracing::warn!(
                name = func_name_str,
                "ArcIrEmitter: unresolved function in invoke"
            );
        }
    }

    /// Emit an `Apply` instruction (ABI-aware direct call).
    fn emit_apply(&mut self, dst: ArcVarId, callee: Name, args: &[ArcVarId]) {
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        let callee_name_str = self.interner.lookup(callee);

        let result = if let Some(&(func_id, ref abi)) = self.functions.get(&callee) {
            let passed_args = self.apply_param_passing(&arg_vals, &abi.params);
            match &abi.return_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(abi.return_abi.ty);
                    self.call_with_sret(func_id, &passed_args, ret_ty, "call")
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.builder.call(func_id, &passed_args, "call")
                }
            }
        } else if let Some(llvm_func) = self.builder.scx().llmod.get_function(callee_name_str) {
            let func_id = self.builder.intern_function(llvm_func);
            self.builder.call(func_id, &arg_vals, "call")
        } else {
            tracing::warn!(
                name = callee_name_str,
                "ArcIrEmitter: unresolved function in apply"
            );
            self.builder.record_codegen_error();
            None
        };

        if let Some(val) = result {
            self.def_var(dst, val);
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
        let fn_ptr = self.builder.extract_value(closure_val, 0, "closure.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "closure.env_ptr");

        if let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) {
            let mut arg_vals = Vec::with_capacity(1 + args.len());
            arg_vals.push(env_ptr);
            for &a in args {
                arg_vals.push(self.var(a));
            }

            let ptr_ty = self.builder.ptr_type();
            let mut param_types = Vec::with_capacity(1 + args.len());
            param_types.push(ptr_ty);
            for &a in args {
                let arg_ty = func.var_type(a);
                param_types.push(self.resolve_type(arg_ty));
            }

            let ret_ty = self.resolve_type(ty);
            if let Some(val) =
                self.builder
                    .call_indirect(ret_ty, &param_types, fn_ptr, &arg_vals, "icall")
            {
                self.def_var(dst, val);
            }
        }
    }

    /// Emit a `PartialApply` instruction (closure creation stub).
    fn emit_partial_apply(&mut self, dst: ArcVarId, callee: Name, args: &[ArcVarId]) {
        // Full closure compilation requires generating a wrapper function
        // and packing captures into an env struct.
        let callee_name_str = self.interner.lookup(callee);
        tracing::debug!(
            name = callee_name_str,
            args = args.len(),
            "ArcIrEmitter: PartialApply — closure creation (stub)"
        );

        let closure_ty = self.builder.closure_type();
        let null_ptr = self.builder.const_null_ptr();
        let closure = self
            .builder
            .build_struct(closure_ty, &[null_ptr, null_ptr], "partial_apply");
        self.def_var(dst, closure);
    }

    /// Emit a `Project` instruction (field extraction).
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
        if let Some(extracted) = self
            .builder
            .extract_value(val, field, &format!("proj.{field}"))
        {
            self.def_var(dst, extracted);
        } else {
            // Fallback: GEP-based field access for heap-allocated types
            let val_ty = func.var_type(value);
            let llvm_val_ty = self.resolve_type(val_ty);
            let gep =
                self.builder
                    .struct_gep(llvm_val_ty, val, field, &format!("proj.{field}.gep"));
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var(dst, loaded);
        }
    }

    // -----------------------------------------------------------------------
    // Instruction emission
    // -----------------------------------------------------------------------

    /// Emit a single `ArcInstr` as LLVM IR.
    fn emit_instr(&mut self, instr: &ArcInstr, func: &ArcFunction) {
        match instr {
            ArcInstr::Let { dst, ty, value } => {
                let val = self.emit_value(value, *ty, func);
                self.def_var(*dst, val);
            }

            ArcInstr::Apply {
                dst,
                ty: _,
                func: callee,
                args,
            } => self.emit_apply(*dst, *callee, args),

            ArcInstr::ApplyIndirect {
                dst,
                ty,
                closure,
                args,
            } => self.emit_apply_indirect(*dst, *ty, *closure, args, func),

            ArcInstr::PartialApply {
                dst,
                ty: _,
                func: callee,
                args,
            } => self.emit_partial_apply(*dst, *callee, args),

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
                self.def_var(*dst, val);
            }

            // RC operations
            ArcInstr::RcInc { var, count } => {
                let val = self.var(*var);
                let rc_inc_name = "ori_rc_inc";
                if let Some(llvm_func) = self.builder.scx().llmod.get_function(rc_inc_name) {
                    let func_id = self.builder.intern_function(llvm_func);
                    for _ in 0..*count {
                        self.builder.call(func_id, &[val], "");
                    }
                }
            }

            ArcInstr::RcDec { var } => {
                let val = self.var(*var);
                let ty = func.var_type(*var);
                let drop_fn_ptr = self.get_or_generate_drop_fn(ty);
                if let Some(llvm_func) = self.builder.scx().llmod.get_function("ori_rc_dec") {
                    let func_id = self.builder.intern_function(llvm_func);
                    self.builder.call(func_id, &[val, drop_fn_ptr], "");
                }
            }

            ArcInstr::IsShared { dst, var } => {
                // Inline refcount check: data_ptr - 8 = strong_count (i64).
                // Shared when strong_count > 1 (more than one owner).
                let data_ptr = self.var(*var);
                let i8_ty = self.builder.i8_type();
                let neg8 = self.builder.const_i64(-8);
                let rc_ptr = self.builder.gep(i8_ty, data_ptr, &[neg8], "rc_ptr");
                let i64_ty = self.builder.i64_type();
                let rc_val = self.builder.load(i64_ty, rc_ptr, "rc_val");
                let one = self.builder.const_i64(1);
                let is_shared = self.builder.icmp_sgt(rc_val, one, "is_shared");
                self.def_var(*dst, is_shared);
            }

            ArcInstr::Reset { var, token } => {
                // Reset marks a value for potential reuse. After expansion by
                // Section 09, this becomes IsShared + conditional.
                // The token IS the variable (reuse its memory if unique).
                let val = self.var(*var);
                self.def_var(*token, val);
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
                self.def_var(*dst, val);
                let _ = token;
            }

            ArcInstr::Set { base, field, value } => {
                // In-place field update (only valid when uniquely owned).
                // After expand_reuse, this only appears in the fast path for
                // heap-allocated RC'd objects (pointer-typed base).
                let base_val = self.var(*base);
                let new_val = self.var(*value);
                let base_ty = func.var_type(*base);
                let llvm_ty = self.resolve_type(base_ty);

                // GEP + store for heap-allocated RC'd objects.
                // The base is a pointer to the struct data on the heap.
                let field_ptr =
                    self.builder
                        .struct_gep(llvm_ty, base_val, *field, &format!("set.{field}.ptr"));
                self.builder.store(new_val, field_ptr);
                // base pointer unchanged — mutation is in-place
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
                let global = self.builder.build_global_string_ptr(s, "str");
                let len = self.builder.const_i64(s.len() as i64);
                // Ori string: { i64 len, ptr data }
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

    /// Emit a binary operation.
    ///
    /// For primitive types, emits direct LLVM instructions. For non-primitive
    /// types, dispatches to the corresponding operator trait method
    /// (e.g., `+` → `Add.add()`).
    // SYNC: also update ExprLowerer::lower_binary_op in lower_operators.rs
    fn emit_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId, lhs_ty: Idx) -> ValueId {
        // Trait dispatch for non-primitive types (user-defined operator impls)
        if !lhs_ty.is_primitive() {
            if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
        }

        let is_float = matches!(
            self.type_info.get(lhs_ty),
            super::type_info::TypeInfo::Float
        );
        let is_str = matches!(self.type_info.get(lhs_ty), super::type_info::TypeInfo::Str);

        match op {
            BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
            BinaryOp::Add if is_str => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
            BinaryOp::Add => self.builder.add(lhs, rhs, "add"),
            BinaryOp::Sub if is_float => self.builder.fsub(lhs, rhs, "sub"),
            BinaryOp::Sub => self.builder.sub(lhs, rhs, "sub"),
            BinaryOp::Mul if is_float => self.builder.fmul(lhs, rhs, "mul"),
            BinaryOp::Mul => self.builder.mul(lhs, rhs, "mul"),
            BinaryOp::Div if is_float => self.builder.fdiv(lhs, rhs, "div"),
            BinaryOp::Div => self.builder.sdiv(lhs, rhs, "div"),
            BinaryOp::Mod if is_float => self.builder.frem(lhs, rhs, "rem"),
            BinaryOp::Mod => self.builder.srem(lhs, rhs, "rem"),
            BinaryOp::Eq if is_float => self.builder.fcmp_oeq(lhs, rhs, "eq"),
            BinaryOp::Eq if is_str => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq if is_float => self.builder.fcmp_one(lhs, rhs, "ne"),
            BinaryOp::NotEq if is_str => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::Lt if is_float => self.builder.fcmp_olt(lhs, rhs, "lt"),
            BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
            BinaryOp::Gt if is_float => self.builder.fcmp_ogt(lhs, rhs, "gt"),
            BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
            BinaryOp::LtEq if is_float => self.builder.fcmp_ole(lhs, rhs, "le"),
            BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
            BinaryOp::GtEq if is_float => self.builder.fcmp_oge(lhs, rhs, "ge"),
            BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
            BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
            BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
            BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
            BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
            BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Coalesce | BinaryOp::MatMul => {
                // Range/coalesce/matmul ops are desugared or trait-dispatched before reaching ARC IR
                tracing::warn!(?op, "ArcIrEmitter: desugared op in binary expression");
                self.builder.const_i64(0)
            }
        }
    }

    /// Emit a unary operation.
    ///
    /// For primitive types, emits direct LLVM instructions. For non-primitive
    /// types, dispatches to the corresponding operator trait method
    /// (e.g., `-` → `Negate.negate()`).
    fn emit_unary_op(&mut self, op: UnaryOp, operand: ValueId, operand_ty: Idx) -> ValueId {
        // Trait dispatch for non-primitive types (user-defined operator impls)
        if !operand_ty.is_primitive() {
            if let Some(result) = self.emit_unary_op_via_trait(op, operand, operand_ty) {
                return result;
            }
        }

        let is_float = matches!(
            self.type_info.get(operand_ty),
            super::type_info::TypeInfo::Float
        );

        match op {
            UnaryOp::Neg if is_float => self.builder.fneg(operand, "neg"),
            UnaryOp::Neg => self.builder.neg(operand, "neg"),
            UnaryOp::Not => self.builder.not(operand, "not"),
            UnaryOp::BitNot => {
                let all_ones = self.builder.const_i64(-1);
                self.builder.xor(operand, all_ones, "bitnot")
            }
            UnaryOp::Try => {
                // Try is desugared before reaching ARC IR
                tracing::warn!("ArcIrEmitter: try op in unary expression");
                self.builder.const_i64(0)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Operator trait dispatch
    // -----------------------------------------------------------------------

    /// Dispatch a binary operator to a trait method for non-primitive types.
    ///
    /// Maps the operator to its trait method name (e.g., `+` → `"add"`),
    /// looks up the compiled method function, and emits a method call.
    // SYNC: also update ExprLowerer::lower_binary_op_via_trait in lower_operators.rs
    fn emit_binary_op_via_trait(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = op.trait_method_name()?;
        let type_name = *self.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern(method_name);
        // Scope the immutable borrow of method_functions: extract only what
        // we need so we can call &mut self methods below.
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self.method_functions.get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "op_trait")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &passed_args, "op_trait")
            }
        }
    }

    /// Dispatch a unary operator to a trait method for non-primitive types.
    ///
    /// Maps the operator to its trait method name (e.g., `-` → `"negate"`),
    /// looks up the compiled method function, and emits a method call.
    // SYNC: also update ExprLowerer::lower_unary_op_via_trait in lower_operators.rs
    fn emit_unary_op_via_trait(
        &mut self,
        op: UnaryOp,
        operand: ValueId,
        operand_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = op.trait_method_name()?;
        let type_name = *self.type_idx_to_name.get(&operand_ty)?;
        let interned_method = self.interner.intern(method_name);
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self.method_functions.get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [operand];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "op_trait")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &passed_args, "op_trait")
            }
        }
    }

    // -----------------------------------------------------------------------
    // Constructor emission
    // -----------------------------------------------------------------------

    /// Emit a `Construct` instruction.
    fn emit_construct(&mut self, ty: Idx, ctor: &CtorKind, args: &[ArcVarId]) -> ValueId {
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        let llvm_ty = self.resolve_type(ty);

        match ctor {
            CtorKind::Struct(_) | CtorKind::Tuple => {
                // Build a struct value from fields
                self.builder.build_struct(llvm_ty, &arg_vals, "ctor")
            }

            CtorKind::EnumVariant { variant, .. } => {
                // Enum: { tag, fields... }
                // Build tag + fields as a struct
                let tag_val = self.builder.const_i64(i64::from(*variant));
                let mut fields = Vec::with_capacity(1 + arg_vals.len());
                fields.push(tag_val);
                fields.extend_from_slice(&arg_vals);
                self.builder.build_struct(llvm_ty, &fields, "variant")
            }

            CtorKind::ListLiteral => {
                // List construction: allocate data, store elements, build struct
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx = match &type_info {
                    super::type_info::TypeInfo::List { element } => *element,
                    _ => ori_types::Idx::INT,
                };
                let elem_llvm_ty = self.resolve_type(elem_idx);
                let elem_size = self.type_info.get(elem_idx).size().unwrap_or(8);

                let cap_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                let data_ptr = if let Some(alloc_fn) =
                    self.builder.scx().llmod.get_function("ori_list_alloc_data")
                {
                    let func_id = self.builder.intern_function(alloc_fn);
                    self.builder
                        .call(func_id, &[cap_val, esize_val], "list.data")
                        .unwrap_or_else(|| self.builder.const_null_ptr())
                } else {
                    self.builder.const_null_ptr()
                };

                // Store each element into the data buffer
                for (i, &val) in arg_vals.iter().enumerate() {
                    let idx = self.builder.const_i64(i as i64);
                    let elem_ptr =
                        self.builder
                            .gep(elem_llvm_ty, data_ptr, &[idx], "list.elem_ptr");
                    self.builder.store(val, elem_ptr);
                }

                // Build list struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[cap_val, cap_val, data_ptr], "list")
            }

            CtorKind::MapLiteral | CtorKind::SetLiteral => {
                // Map/set construction — stub for now
                tracing::debug!("ArcIrEmitter: map/set literal construction (stub)");
                self.builder.const_null_ptr()
            }

            CtorKind::Closure { func } => {
                // Closure: { fn_ptr, env_ptr }
                let callee_name_str = self.interner.lookup(*func);
                let fn_ptr = if let Some(llvm_func) =
                    self.builder.scx().llmod.get_function(callee_name_str)
                {
                    let fid = self.builder.intern_function(llvm_func);
                    self.builder.get_function_ptr(fid)
                } else if let Some(&(func_id, _)) = self.functions.get(func) {
                    self.builder.get_function_ptr(func_id)
                } else {
                    self.builder.const_null_ptr()
                };

                // Environment pointer: pack captured args into an alloca
                // TODO: proper env packing with RC-tracked allocation
                let env_ptr = if arg_vals.is_empty() {
                    self.builder.const_null_ptr()
                } else {
                    let ptr_ty = self.builder.ptr_type();
                    self.builder.alloca(ptr_ty, "env")
                };

                let closure_ty = self.builder.closure_type();
                self.builder
                    .build_struct(closure_ty, &[fn_ptr, env_ptr], "closure")
            }
        }
    }

    // -----------------------------------------------------------------------
    // ABI helpers
    // -----------------------------------------------------------------------

    /// Apply parameter passing modes to argument values.
    ///
    /// Mirrors `ExprLowerer::apply_param_passing` — handles all `ParamPassing`
    /// variants: `Indirect`/`Reference` (alloca+store+pass ptr), `Direct`
    /// (pass through), `Void` (skip).
    ///
    // SYNC: also update ExprLowerer::apply_param_passing in lower_calls.rs
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
    // String runtime call helpers
    // -----------------------------------------------------------------------

    /// Call a string runtime function: `ori_str_concat`, `ori_str_eq`, `ori_str_ne`.
    ///
    /// String values are `{ i64, ptr }` structs passed by pointer to the runtime.
    /// `returns_str` controls the return type: `true` → `{ i64, ptr }`, `false` → `i1`.
    // SYNC: also update ExprLowerer::lower_str_concat / lower_str_eq / lower_str_ne in lower_operators.rs
    fn emit_str_runtime_call(
        &mut self,
        func_name: &str,
        lhs: ValueId,
        rhs: ValueId,
        returns_str: bool,
    ) -> ValueId {
        let Some(llvm_func) = self.builder.scx().llmod.get_function(func_name) else {
            tracing::warn!(func_name, "ArcIrEmitter: string runtime function not found");
            return self.builder.const_i64(0);
        };
        let func_id = self.builder.intern_function(llvm_func);

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
