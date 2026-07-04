//! `Traceable` trait emission: `?`-hop trace injection and the read
//! accessors (`trace` / `has_trace` / `trace_entries` / `with_trace`) on an
//! `Error` struct or a Result/Option delegation receiver.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::Idx;

use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit the Traceable read accessors (`trace` / `has_trace` / `trace_entries`,
    /// `with_trace`) on an `Error` struct or a Result/Option delegation receiver,
    /// and intercept `__ori_inject_trace` (the `?`-hop trace-injection protocol
    /// call). Returns `None` for any other method/receiver.
    ///
    /// The `Error` struct carries a `trace: [TraceEntry]` field: the accessors GEP
    /// into it and `__ori_inject_trace` calls the runtime `_ori_inject_trace_entry`
    /// to COW-push a `?`-hop entry. Invoked as an early intercept in `emit_apply` /
    /// `emit_invoke` ahead of `resolve_callee` — a `backend_required: false`
    /// Traceable method otherwise resolves to an unbacked `_ori_trace` mono decl
    /// with a mismatched ABI.
    pub(in crate::codegen::arc_emitter) fn try_emit_traceless_traceable(
        &mut self,
        callee: Name,
        args: &[ArcVarId],
        arc_func: &ArcFunction,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        if args.is_empty() {
            return None;
        }
        let method_name = self.interner.lookup(callee);
        if !matches!(
            method_name,
            "trace_entries" | "trace" | "has_trace" | "with_trace" | "__ori_inject_trace"
        ) {
            return None;
        }

        let receiver_ty = arc_func.var_type(args[0]);
        let is_error_struct = self.pool.error_struct_idx().is_some_and(|e| {
            receiver_ty == e || self.pool.resolve_fully(receiver_ty) == self.pool.resolve_fully(e)
        });

        let is_option_or_result = self
            .type_info
            .get(receiver_ty)
            .builtin_type_name()
            .is_some_and(|n| n == "Option" || n == "Result");

        if method_name == "__ori_inject_trace" {
            if !is_error_struct {
                return None;
            }
        } else if !is_error_struct && !is_option_or_result {
            return None;
        }

        let receiver_val = self.var(args[0]);

        if method_name == "__ori_inject_trace" {
            return self.emit_trace_injection(receiver_val, arc_func);
        }

        let error_ptr = self.traceable_error_ptr(receiver_val, receiver_ty, is_error_struct)?;

        self.emit_traceable_accessor(
            method_name,
            error_ptr,
            receiver_val,
            args,
            is_error_struct,
            dst_ty,
        )
    }

    fn current_trace_location(&self, arc_func: &ArcFunction) -> (String, i64, i64) {
        let Some(dc) = self.debug_context else {
            return ("<unknown>".to_string(), 0, 0);
        };
        let span = arc_func
            .spans
            .get(self.current_block_idx)
            .and_then(|block_spans| block_spans.get(self.current_instr_idx))
            .copied()
            .flatten()
            .unwrap_or(ori_ir::Span::DUMMY);
        let (line, col) = dc.offset_to_line_col(span.start);
        (dc.source_path.clone(), i64::from(line), i64::from(col))
    }

    fn emit_trace_injection(
        &mut self,
        receiver_val: ValueId,
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        let (file_path, line, col) = self.current_trace_location(arc_func);
        let func_name_str = self.interner.lookup(arc_func.name);
        let func_const = self.emit_literal_ori_str(func_name_str)?;
        let file_const = self.emit_literal_ori_str(&file_path)?;
        let line_const = self.builder.const_i64(line);
        let col_const = self.builder.const_i64(col);

        // `_ori_inject_trace_entry` takes `function`/`file` by pointer (a 24-byte
        // OriStr is > 16 bytes, so the SysV ABI passes it indirectly); spill each
        // value to an alloca and hand over the slot pointer.
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let func_ptr = self.builder.alloca(str_ty, "trace.fn.ptr");
        self.builder.store(func_const, func_ptr);
        let file_ptr = self.builder.alloca(str_ty, "trace.file.ptr");
        self.builder.store(file_const, file_ptr);

        let error_struct_idx = self.pool.error_struct_idx().unwrap();
        let error_struct_ty = self.resolve_type(error_struct_idx);
        let alloca = self.error_receiver_ptr(receiver_val, error_struct_idx);

        let func_id = self.builder.runtime_fn("_ori_inject_trace_entry");
        self.emit_rt_call(
            func_id,
            &[alloca, func_ptr, file_ptr, line_const, col_const],
            "_ori_inject_trace_entry",
        );
        let updated_val = self.builder.load(error_struct_ty, alloca, "error.updated");
        Some(updated_val)
    }

    /// Materialize an `Error` struct VALUE to a pointer the accessors + runtime
    /// calls (`_ori_format_error_trace` / `_ori_error_with_trace`) can GEP into.
    /// `Error` is 48 bytes (sret-returned by value), so the receiver arrives as
    /// an SSA struct value, not a pointer; spill it to a fresh alloca. SSOT for
    /// the by-value->pointer materialization shared by trace injection, the
    /// error-struct accessor path, and `with_trace`.
    fn error_receiver_ptr(&mut self, receiver_val: ValueId, error_ty: Idx) -> ValueId {
        let error_struct_ty = self.resolve_type(error_ty);
        let alloca = self.builder.alloca(error_struct_ty, "error.recv.ptr");
        self.builder.store(receiver_val, alloca);
        alloca
    }

    fn traceable_error_ptr(
        &mut self,
        receiver_val: ValueId,
        receiver_ty: Idx,
        is_error_struct: bool,
    ) -> Option<ValueId> {
        if is_error_struct {
            return Some(self.error_receiver_ptr(receiver_val, receiver_ty));
        }

        let tag = self.builder.extract_value(receiver_val, 0, "tag")?;
        let receiver_name = self.type_info.get(receiver_ty).builtin_type_name()?;
        let is_match = if receiver_name == "Option" {
            let zero = self.builder.const_i64(0);
            self.builder.icmp_eq(tag, zero, "is_some")
        } else {
            let one = self.builder.const_i64(1);
            self.builder.icmp_eq(tag, one, "is_err")
        };

        let payload_ty = match self.type_info.get(receiver_ty) {
            TypeInfo::Option { inner } => inner,
            TypeInfo::Result { err, .. } => err,
            _ => return None,
        };

        let llvm_recv_ty = self.resolve_type(receiver_ty);
        let alloca = self.builder.alloca(llvm_recv_ty, "enum.alloca");
        self.builder.store(receiver_val, alloca);
        let payload_ptr = self
            .builder
            .struct_gep(llvm_recv_ty, alloca, 1, "payload.ptr");

        let error_struct_ptr = if self.is_boxed_enum_field(receiver_ty, payload_ty) {
            let ptr_ty = self.builder.ptr_type();
            self.builder.load(ptr_ty, payload_ptr, "payload.box")
        } else {
            payload_ptr
        };

        let null_ptr = self.builder.const_null_ptr();
        Some(
            self.builder
                .select(is_match, error_struct_ptr, null_ptr, "error_ptr"),
        )
    }

    fn emit_traceable_accessor(
        &mut self,
        method_name: &str,
        error_ptr: ValueId,
        receiver_val: ValueId,
        args: &[ArcVarId],
        is_error_struct: bool,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        let error_struct_idx = self.pool.error_struct_idx()?;
        let mem_trace_field = self.remap_struct_field(error_struct_idx, 1);

        match method_name {
            "has_trace" => {
                self.emit_has_trace_accessor(error_ptr, error_struct_idx, mem_trace_field)
            }
            "trace_entries" => self.emit_trace_entries_accessor(
                error_ptr,
                error_struct_idx,
                mem_trace_field,
                dst_ty,
            ),
            "trace" => self.emit_trace_string_accessor(error_ptr, dst_ty),
            "with_trace" if is_error_struct => {
                self.emit_with_trace_accessor(receiver_val, args, dst_ty)
            }
            _ => None,
        }
    }

    fn emit_has_trace_accessor(
        &mut self,
        error_ptr: ValueId,
        error_struct_idx: Idx,
        mem_trace_field: u32,
    ) -> Option<ValueId> {
        let error_struct_ty = self.resolve_type(error_struct_idx);
        let is_null = self.builder.is_null_ptr(error_ptr, "is_null");
        let is_not_null = self.builder.not(is_null, "is_not_null");

        let entry_bb = self.builder.current_block().unwrap();
        let then_bb = self
            .builder
            .append_block(self.current_function, "has_trace.then");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "has_trace.merge");

        self.builder.cond_br(is_not_null, then_bb, merge_bb);

        self.builder.position_at_end(then_bb);
        let trace_ptr =
            self.builder
                .struct_gep(error_struct_ty, error_ptr, mem_trace_field, "trace_ptr");

        let str_llvm = self.type_resolver.resolve(Idx::STR);
        let list_struct_ty = self.builder.register_type(str_llvm);
        let len_ptr = self
            .builder
            .struct_gep(list_struct_ty, trace_ptr, 0, "len_ptr");
        let i64_ty = self.resolve_type(Idx::INT);
        let len_val = self.builder.load(i64_ty, len_ptr, "len_val");

        let zero = self.builder.const_i64(0);
        let has_trace_val = self.builder.icmp_sgt(len_val, zero, "has_trace");
        self.builder.br(merge_bb);
        let then_end_bb = self.builder.current_block().unwrap();

        self.builder.position_at_end(merge_bb);
        let false_val = self.builder.const_bool(false);
        let bool_ty = self.builder.bool_type();
        let has_trace_res = self.builder.phi_from_incoming(
            bool_ty,
            &[(has_trace_val, then_end_bb), (false_val, entry_bb)],
            "has_trace_res",
        );
        Some(has_trace_res.unwrap())
    }

    fn emit_trace_entries_accessor(
        &mut self,
        error_ptr: ValueId,
        error_struct_idx: Idx,
        mem_trace_field: u32,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        let is_null = self.builder.is_null_ptr(error_ptr, "is_null");
        let is_not_null = self.builder.not(is_null, "is_not_null");

        let error_struct_ty = self.resolve_type(error_struct_idx);
        let list_llvm = self.resolve_type(dst_ty);
        let zero_list = self.builder.const_zero_ty(list_llvm);

        let entry_bb = self.builder.current_block().unwrap();
        let then_bb = self
            .builder
            .append_block(self.current_function, "trace_entries.then");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "trace_entries.merge");

        self.builder.cond_br(is_not_null, then_bb, merge_bb);

        self.builder.position_at_end(then_bb);
        let trace_ptr =
            self.builder
                .struct_gep(error_struct_ty, error_ptr, mem_trace_field, "trace_ptr");
        let loaded = self.builder.load(list_llvm, trace_ptr, "trace_entries");
        self.builder.br(merge_bb);
        let then_end_bb = self.builder.current_block().unwrap();

        self.builder.position_at_end(merge_bb);
        let trace_entries_res = self.builder.phi_from_incoming(
            list_llvm,
            &[(loaded, then_end_bb), (zero_list, entry_bb)],
            "trace_entries_res",
        );
        Some(trace_entries_res.unwrap())
    }

    fn emit_trace_string_accessor(&mut self, error_ptr: ValueId, dst_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("_ori_format_error_trace");
        let llvm_dst_ty = self.resolve_type(dst_ty);
        Some(
            self.call_with_sret(
                func_id,
                &[error_ptr],
                llvm_dst_ty,
                "_ori_format_error_trace",
            )
            .expect("sret call returns value"),
        )
    }

    fn emit_with_trace_accessor(
        &mut self,
        receiver_val: ValueId,
        args: &[ArcVarId],
        dst_ty: Idx,
    ) -> Option<ValueId> {
        let entry_ptr = self.var(*args.get(1)?);
        // `_ori_error_with_trace`'s 2nd param is a `ptr` to the Error; the
        // receiver arrives by value (48-byte sret Error), so spill it.
        let error_struct_idx = self.pool.error_struct_idx()?;
        let error_ptr = self.error_receiver_ptr(receiver_val, error_struct_idx);
        let func_id = self.builder.runtime_fn("_ori_error_with_trace");
        let llvm_dst_ty = self.resolve_type(dst_ty);

        let out_alloca = self.builder.alloca(llvm_dst_ty, "with_trace_out");
        self.emit_rt_call(
            func_id,
            &[out_alloca, error_ptr, entry_ptr],
            "_ori_error_with_trace",
        );
        Some(
            self.builder
                .load(llvm_dst_ty, out_alloca, "with_trace_result"),
        )
    }
}
