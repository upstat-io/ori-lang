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

use ori_arc::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue, PrimOp,
};
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
    /// Type pool for structural queries (used by future type-dependent emission).
    #[expect(dead_code, reason = "Reserved for future type-dependent IR emission")]
    pool: &'a Pool,
    /// The LLVM function being compiled.
    current_function: FunctionId,
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    functions: &'a FxHashMap<Name, (FunctionId, FunctionAbi)>,
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
        current_function: FunctionId,
        functions: &'a FxHashMap<Name, (FunctionId, FunctionAbi)>,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            current_function,
            functions,
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

        // Emit each block's body and terminator
        for block in &func.blocks {
            self.builder.position_at_end(self.block(block.id));
            for instr in &block.body {
                self.emit_instr(instr, func);
            }
            self.emit_terminator(&block.terminator, block.id, &phi_nodes, abi);
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
        _current_block: ori_arc::ir::ArcBlockId,
        _phi_nodes: &[Vec<(ArcVarId, ValueId)>],
        abi: &FunctionAbi,
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
                    let source_block = self.builder.current_block().expect("no current block");
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
                // Re-raise the caught exception.
                // The landing pad value should be in scope from the unwind block.
                // For now, emit unreachable — full resume requires tracking the
                // landingpad value through the unwind block's instructions.
                self.builder.unreachable();
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
                let rc_dec_name = "ori_rc_dec";
                if let Some(llvm_func) = self.builder.scx().llmod.get_function(rc_dec_name) {
                    let func_id = self.builder.intern_function(llvm_func);
                    // Drop function: for now, pass null (trivial drop).
                    // Full drop function lookup from DropInfo is wired in C.4.
                    let null_drop = self.builder.const_null_ptr();
                    self.builder.call(func_id, &[val, null_drop], "");
                }
            }

            ArcInstr::IsShared { dst, var: _ } => {
                // Test if refcount > 1: load rc at ptr-8, compare > 1
                // For now, emit `false` (assume unique) — proper inline RC check
                // requires knowing the header layout.
                let val = self.builder.const_bool(false);
                self.def_var(*dst, val);
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
                // Reuse: construct using token's memory if available.
                // After expansion by Section 09, this is the "fast path" —
                // the token's memory is already allocated.
                // For the initial scaffold, just construct normally.
                let val = self.emit_construct(*ty, ctor, args);
                self.def_var(*dst, val);
                // Token is consumed but not needed for the basic path.
                let _ = token;
            }

            ArcInstr::Set { base, field, value } => {
                // In-place field update (only valid when uniquely owned)
                let base_val = self.var(*base);
                let new_val = self.var(*value);

                // insert_value for value-typed structs
                let updated =
                    self.builder
                        .insert_value(base_val, new_val, *field, &format!("set.{field}"));
                // Re-bind the base variable to the updated value
                self.def_var(*base, updated);
            }

            ArcInstr::SetTag { base, tag } => {
                // In-place tag update for enum variants
                // The tag is typically field 0 of the enum representation
                let base_val = self.var(*base);
                let tag_val = self.builder.const_i64(*tag as i64);
                let updated = self.builder.insert_value(base_val, tag_val, 0, "set.tag");
                self.def_var(*base, updated);
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
    fn emit_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId, lhs_ty: Idx) -> ValueId {
        let is_float = matches!(
            self.type_info.get(lhs_ty),
            super::type_info::TypeInfo::Float
        );

        match op {
            BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
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
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq if is_float => self.builder.fcmp_one(lhs, rhs, "ne"),
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
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Coalesce => {
                // Range/coalesce ops are desugared before reaching ARC IR
                tracing::warn!(?op, "ArcIrEmitter: desugared op in binary expression");
                self.builder.const_i64(0)
            }
        }
    }

    /// Emit a unary operation.
    fn emit_unary_op(&mut self, op: UnaryOp, operand: ValueId, operand_ty: Idx) -> ValueId {
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
                // List construction: allocate and populate
                // For now, use the runtime list_new helper
                let elem_count = self.builder.const_i64(arg_vals.len() as i64);
                let elem_size = self.builder.const_i64(8); // sizeof(i64)
                if let Some(list_new) = self.builder.scx().llmod.get_function("ori_list_new") {
                    let func_id = self.builder.intern_function(list_new);
                    if let Some(list) = self.builder.call(func_id, &[elem_count, elem_size], "list")
                    {
                        return list;
                    }
                }
                self.builder.const_null_ptr()
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
    fn apply_param_passing(
        &mut self,
        args: &[ValueId],
        params: &[super::abi::ParamAbi],
    ) -> Vec<ValueId> {
        args.iter()
            .zip(params.iter())
            .map(|(&val, param)| match &param.passing {
                super::abi::ParamPassing::Reference => {
                    let ty = self.resolve_type(param.ty);
                    let alloca = self.builder.alloca(ty, "ref.tmp");
                    self.builder.store(val, alloca);
                    alloca
                }
                _ => val,
            })
            .collect()
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
}
