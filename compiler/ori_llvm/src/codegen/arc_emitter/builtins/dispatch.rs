//! Builtin-method dispatch entry point.
//!
//! Split out of `builtins/mod.rs` (WASTE-12: keep source files under the
//! 500-line cap). Holds the `ArcIrEmitter` dispatch surface that routes a
//! method call through the declarative submodule chain declared in
//! `builtins/mod.rs`.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::Idx;

use super::iterators_guard::{auto_iter_element_type, is_iterator_method};
use super::{
    collections, compound_traits, iterator, option_result, primitives, traits, BuiltinCtx,
};
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

        // `_ori_inject_trace_entry` takes `function`/`file` by pointer (AB-1:
        // 24-byte OriStr is Indirect); spill each value to an alloca.
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
    /// `Error` is 32 bytes (sret-returned by value), so the receiver arrives as
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
        // receiver arrives by value (32-byte sret Error), so spill it.
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

    /// Try to emit inline IR for a builtin method call.
    ///
    /// Returns `Some(result_value)` if the method was handled, `None` if
    /// the caller should fall through to the runtime function lookup.
    ///
    /// # Dispatch order
    ///
    /// 1. Build [`BuiltinCtx`] (type name, method, resolved arg values)
    /// 2. Try submodule dispatch chain (declarative via `declare_builtins!`)
    /// 3. Fall through to generic clone for types without builtin names
    pub(in crate::codegen::arc_emitter) fn try_emit_builtin_method(
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
        let receiver_ty = arc_func.var_type(args[0]);
        let type_info = self.type_info.get(receiver_ty);
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        // Traceless Traceable accessors (Error-struct + Result/Option delegation).
        // SSOT helper, also invoked as an early intercept in `emit_apply` /
        // `emit_invoke` before callee resolution.
        if let Some(val) = self.try_emit_traceless_traceable(callee, args, arc_func, dst_ty) {
            return Some(val);
        }

        // Types with builtin names dispatch through the declarative submodule chain
        if let Some(type_name) = type_info.builtin_type_name() {
            let ctx = BuiltinCtx {
                type_name,
                method: method_name,
                arg_vals: &arg_vals,
                receiver_ty,
                dst_ty,
                type_info: &type_info,
                arc_args: args,
                arc_func,
            };

            let result = primitives::dispatch(self, &ctx)
                .or_else(|| collections::dispatch(self, &ctx))
                .or_else(|| traits::dispatch(self, &ctx))
                .or_else(|| option_result::dispatch(self, &ctx))
                .or_else(|| iterator::dispatch(self, &ctx))
                .or_else(|| compound_traits::dispatch(self, &ctx))
                .or_else(|| self.try_emit_dei_rekeyed(&ctx));

            if result.is_some() {
                return result;
            }

            // Auto-iter promotion: collection.iter_method(...) → collection.iter.iter_method(...)
            // When a collection type (list, map, Set, str, range) has an unresolved method
            // that IS a known iterator method, emit.iter implicitly and forward.
            if is_iterator_method(method_name) {
                if let Some(iter_val) = self.emit_auto_iter(&type_info, arg_vals[0], receiver_ty) {
                    let iter_info = TypeInfo::Iterator {
                        element: auto_iter_element_type(&type_info),
                    };
                    let mut iter_args = vec![iter_val];
                    iter_args.extend_from_slice(&arg_vals[1..]);
                    let iter_ctx = BuiltinCtx {
                        type_name: "Iterator",
                        method: method_name,
                        arg_vals: &iter_args,
                        receiver_ty,
                        dst_ty,
                        type_info: &iter_info,
                        arc_args: args,
                        arc_func,
                    };
                    if let Some(iter_val) = iterator::dispatch(self, &iter_ctx) {
                        return Some(self.collect_auto_iter_result(iter_val, dst_ty));
                    }
                }
            }

            // Eval-parity unknown-method trap. The receiver is a builtin type
            // and every dispatch surface missed (submodule chain, auto-iter
            // promotion). The interpreter resolves builtin methods at dispatch
            // time and raises `no method '<name>' on type <type>` as a RUNTIME
            // panic — mirror that here instead of failing codegen, preserving
            // dual-execution parity for typechecked-but-undispatchable calls
            // (e.g. `str.updated(...)` — str implements Index, not IndexSet).
            // Gates:
            // - `is_callee_intercepted` (SSOT) — the call must be one no
            //   other resolution surface owns (not a declared function, not
            //   a runtime fn, not a monomorphized generic like a stdlib
            //   `assert_eq` whose first ARG happens to be a builtin type,
            //   not prelude/protocol). Mis-firing on those turns a
            //   missing-mono codegen error into a wrong runtime panic.
            // - Methods the registry DOES define on this receiver type are
            //   excluded — a missing dispatch arm for a real method is a
            //   codegen gap (keep the unresolved-function error), not an
            //   unknown method.
            // - Names the registry defines as an ASSOCIATED function on any
            //   type are excluded — associated calls (`Size.from_bytes`)
            //   carry no receiver, so `args[0]` is an ordinary argument and
            //   the receiver-type heuristic misattributes it.
            // Runtime/protocol names (`ori_*`, `__*`) are additionally
            // excluded: `is_callee_intercepted` classifies protocol builtins
            // (`ori_iter_drop`, `__iter_next`) as intercepted, but they
            // resolve via the protocol/runtime-fn fallbacks after this
            // returns None.
            if !method_name.starts_with("ori_")
                && !method_name.starts_with("__")
                && crate::codegen::arc_emitter::context::is_callee_intercepted(
                    method_name,
                    callee,
                    args,
                    arc_func,
                    self.ctx,
                    self.type_info,
                )
                && !registry_defines_method(type_name, method_name)
                && !registry_has_associated_fn(method_name)
            {
                return Some(self.emit_unknown_method_panic(type_name, method_name, dst_ty));
            }
        }

        // Types without builtin names: Unit, Struct, Enum, Function
        // Only clone is handled (identity for Unit, RC inc for heap types).
        if method_name == "clone" {
            return match &type_info {
                TypeInfo::Unit => Some(arg_vals[0]),
                TypeInfo::Struct { .. } | TypeInfo::Enum { .. } | TypeInfo::Function { .. } => {
                    self.emit_rc_inc_clone(arg_vals[0], receiver_ty)
                }
                _ => None,
            };
        }

        None
    }

    /// Re-key a `DoubleEndedIterator` method dispatch onto an `"Iterator"`
    /// receiver.
    ///
    /// `rev` / `last` / `rfind` / `rfold` / `next_back` register under the
    /// `"DoubleEndedIterator"` key (registry `dei_only`), but every iterator
    /// handle realizes as `TypeInfo::Iterator` (`type_name` `"Iterator"`) at
    /// codegen — there is no DEI `TypeInfo` variant. The type checker proved the
    /// receiver is double-ended, so dispatch the same `BuiltinCtx` under
    /// `"DoubleEndedIterator"` to reach the existing `emit_iter_*` emitters.
    fn try_emit_dei_rekeyed(&mut self, base: &BuiltinCtx<'_>) -> Option<ValueId> {
        if base.type_name != "Iterator" || !ori_registry::is_dei_only(base.method) {
            return None;
        }
        let dei_ctx = BuiltinCtx {
            type_name: "DoubleEndedIterator",
            ..*base
        };
        iterator::dispatch(self, &dei_ctx)
    }

    /// Emit the eval-parity `no method '<name>' on type <type>` runtime panic.
    ///
    /// Emits an unconditional branch to a panic block (`ori_panic_cstr` +
    /// `unreachable`), then positions the builder at an unreachable
    /// continuation block and returns a zero value of the destination type so
    /// the caller's normal-block wiring stays well-formed. Mirrors the
    /// interpreter's unknown-method dispatch error (dual-execution parity).
    fn emit_unknown_method_panic(
        &mut self,
        type_name: &str,
        method_name: &str,
        dst_ty: Idx,
    ) -> ValueId {
        let panic_bb = self
            .builder
            .append_block(self.current_function, "no_method.panic");
        let cont_bb = self
            .builder
            .append_block(self.current_function, "no_method.cont");
        self.builder.br(panic_bb);

        self.builder.position_at_end(panic_bb);
        let msg = format!("no method '{method_name}' on type {type_name}");
        let msg_ptr = self.builder.build_global_string_ptr(&msg, "no_method.msg");
        let panic_fn = self.builder.runtime_fn("ori_panic_cstr");
        self.emit_rt_call(panic_fn, &[msg_ptr], "");
        self.builder.unreachable();

        // Unreachable continuation — keeps the caller's `br` to the normal
        // block well-formed; the zero value is never observed.
        self.builder.position_at_end(cont_bb);
        let dst_llvm_ty = self.resolve_type(dst_ty);
        self.builder.const_zero_ty(dst_llvm_ty)
    }

    /// Materialize an auto-iter-promoted iterator handle back into its
    /// declared result collection.
    ///
    /// Auto-iter promotion (`try_emit_builtin_method`) handles an eager
    /// collection method (`[T].filter`, `[T].map`, etc.) by implicitly
    /// `.iter()`-ing the receiver and dispatching the iterator adapter, which
    /// yields an opaque runtime iterator handle (`ptr`). But the method's
    /// result type per Spec: Annex C is the eager collection (`[T]` for list
    /// adapters), not an iterator. The handle's representation (opaque `ptr`)
    /// must agree with its type (a `{len,cap,data}` fat pointer) before any
    /// downstream `.len()` / indexing / RC op runs — otherwise codegen does
    /// `extract_value` on the opaque handle. So when `dst_ty` resolves to a
    /// collection, collect the iterator back into that collection.
    ///
    /// Consumer methods (`count → int`, `any → bool`, `fold → T`) have a
    /// non-collection `dst_ty`; the dispatch already produced the final value
    /// and this is the identity. Explicit `.iter()...collect()` chains keep an
    /// `Iterator`-typed `dst_ty` at the adapter step and likewise pass through.
    fn collect_auto_iter_result(&mut self, iter_val: ValueId, dst_ty: Idx) -> ValueId {
        let resolved = self.pool.resolve_fully(dst_ty);
        match self.pool.tag(resolved) {
            ori_types::Tag::List => {
                let elem_ty = self.pool.list_elem(resolved);
                self.emit_iter_collect(iter_val, elem_ty)
                    .unwrap_or(iter_val)
            }
            ori_types::Tag::Set => {
                let elem_ty = self.pool.set_elem(resolved);
                self.emit_iter_collect_set(iter_val, elem_ty)
                    .unwrap_or(iter_val)
            }
            // Non-collection result (`count`/`any`/`all`/`fold`/`find`
            // consumers → int/bool/T) or a genuinely iterator-typed result
            // (explicit `.iter()` chains) — the dispatch already produced the
            // correct value; no collect-back.
            _ => iter_val,
        }
    }

    /// Emit slice-aware RC increment for a value.
    ///
    /// For List/Set: uses `ori_list_rc_inc(data, cap)` which handles
    /// seamless slices (where `data` is interior to another buffer).
    /// For Str: uses `ori_str_rc_inc(data, cap)` which handles SSO,
    /// heap, and seamless slices from `str.split`.
    /// For other types: falls back to `ori_rc_inc(data)`.
    pub(super) fn emit_slice_aware_rc_inc(&mut self, val: ValueId, ty: ori_types::Idx) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            ori_types::Tag::List | ori_types::Tag::Set => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, 1, "rc_inc.cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    self.call_list_rc_inc(dp, cap, 1);
                } else {
                    self.call_rc_inc_all(&[val], 1);
                }
            }
            // Str: slice-aware RC inc via ori_str_rc_inc(data, cap).
            // Handles SSO, heap, and seamless slices from str.split.
            ori_types::Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, 1, "rc_inc.str_cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    self.call_str_rc_inc(dp, cap, 1);
                } else {
                    self.call_rc_inc_all(&[val], 1);
                }
            }
            _ => {
                let rc_inc = self.builder.runtime_fn("ori_rc_inc");
                let data_ptrs = self.extract_rc_data_ptrs(val, ty);
                for data_ptr in data_ptrs {
                    self.emit_rt_call(rc_inc, &[data_ptr], "");
                }
            }
        }
    }

    /// Emit RC increment + return receiver (clone for heap-backed types).
    ///
    /// Uses slice-aware RC inc for List/Set and Str types.
    pub(crate) fn emit_rc_inc_clone(
        &mut self,
        val: ValueId,
        ty: ori_types::Idx,
    ) -> Option<ValueId> {
        self.emit_slice_aware_rc_inc(val, ty);
        Some(val)
    }

    /// Emit `str.into() : Error` — construct the user-facing
    /// `Error` struct `{ message: str }` from the receiver string.
    ///
    /// The receiver is borrowed (the caller retains its reference), so the
    /// constructed `Error` takes its own ref to the message (slice-aware
    /// RC inc) before the message str becomes the struct's field 0. Mirrors
    /// the evaluator's `Value::error(s)` (`ori_eval/methods/collections.rs`)
    /// so interp↔LLVM observable behavior agrees (message preserved).
    pub(crate) fn emit_str_into_error(
        &mut self,
        receiver_str: ValueId,
        str_ty: ori_types::Idx,
        error_ty: ori_types::Idx,
    ) -> Option<ValueId> {
        self.emit_slice_aware_rc_inc(receiver_str, str_ty);
        let llvm_ty = self.resolve_type(error_ty);
        // Build the FULL 2-field Error `{ message, trace }` — `trace` zero-inited
        // to an empty list `{ 0, 0, null }` (the `[TraceEntry]` fat pointer shares
        // the str/list layout). A 1-field build left `trace` undef, so a later
        // `trace_entries()` read a garbage fat pointer. Mirrors the empty-trace
        // init in `declare_error_constructor`.
        let list_llvm = self.resolve_type(ori_types::Idx::STR);
        let zero = self.builder.const_i64(0);
        let null_data = self.builder.const_null_ptr();
        let empty_trace =
            self.builder
                .build_struct(list_llvm, &[zero, zero, null_data], "empty.trace");
        Some(
            self.builder
                .build_struct(llvm_ty, &[receiver_str, empty_trace], "error.into"),
        )
    }

    /// Emit an implicit `.iter` call for a collection type.
    ///
    /// The iterator takes ownership of one RC reference. Since the caller
    /// didn't account for this (no explicit `.iter` in the ARC IR), we
    /// must `RcInc` the collection before creating the iterator.
    ///
    /// Returns the iterator pointer if the receiver is a collection that
    /// supports iteration, `None` otherwise.
    fn emit_auto_iter(
        &mut self,
        type_info: &TypeInfo,
        receiver: ValueId,
        receiver_ty: ori_types::Idx,
    ) -> Option<ValueId> {
        // RcInc the collection — the iterator will consume one reference.
        // For List/Set: slice-aware ori_list_rc_inc(data, cap).
        // For Str: slice-aware ori_str_rc_inc(data, cap).
        self.emit_slice_aware_rc_inc(receiver, receiver_ty);
        match type_info {
            // The slice-aware inc above gave the iterator its own ref → owns_data = true.
            TypeInfo::List { element } => {
                self.emit_list_iter(receiver, receiver_ty, *element, true)
            }
            // The slice-aware inc above gave the iterator its own ref → receiver_owned = true
            // (same as the List arm's hardcoded `true`); emit_set_iter decs the set buffer.
            TypeInfo::Set { element } => self.emit_set_iter(receiver, *element, true),
            TypeInfo::Map { key, value } => {
                self.emit_map_iter(receiver, *key, *value, receiver_ty, true)
            }
            TypeInfo::Str => self.emit_str_iter(receiver),
            TypeInfo::Range => self.emit_range_iter(receiver),
            _ => None,
        }
    }
}

/// Whether the registry defines `method` on the builtin type whose lowercase
/// dispatch name is `type_name` (any method kind).
fn registry_defines_method(type_name: &str, method: &str) -> bool {
    ori_registry::BUILTIN_TYPES.iter().any(|td| {
        ori_registry::lowercase_type_name(td.name) == type_name
            && td.methods.iter().any(|m| m.name == method)
    })
}

/// Whether ANY builtin type defines `method` as an ASSOCIATED function
/// (factory — no receiver; `args[0]` is an ordinary argument).
fn registry_has_associated_fn(method: &str) -> bool {
    ori_registry::BUILTIN_TYPES.iter().any(|td| {
        td.methods
            .iter()
            .any(|m| m.name == method && m.kind == ori_registry::MethodKind::Associated)
    })
}
