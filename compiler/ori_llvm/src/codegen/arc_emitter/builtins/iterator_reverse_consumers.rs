//! Reverse iterator consumers, string joining, and join provenance.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{FunctionId, ValueId};

use super::super::ArcIrEmitter;
use super::trampolines::TrampolineKind;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `last()` — iterate forward keeping the last element.
    ///
    /// Returns `Option<T>`: `{ i64 tag, T payload }` via sret.
    pub(in crate::codegen) fn emit_iter_last(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Option layout: {i64 tag, T payload}
        let tag_llvm = self.builder.scx().type_i64().into();
        let payload_llvm = self.type_resolver.resolve(elem_ty);
        let opt_struct = self
            .builder
            .scx()
            .type_struct(&[tag_llvm, payload_llvm], false);
        let opt_struct_ty = self.builder.register_type(opt_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "last.out", opt_struct_ty);

        let func_id = self.builder.runtime_fn("ori_iter_last");
        self.emit_rt_call(func_id, &[iter_ptr, elem_size_val, out_ptr], "");

        Some(self.builder.load(opt_struct_ty, out_ptr, "last.result"))
    }

    /// Emit `rfind(predicate)` — find last matching element (collect + search backward).
    pub(in crate::codegen) fn emit_iter_rfind(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        _args: &[ArcVarId],
        _arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Predicate, None);

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Option layout: {i64 tag, T payload}
        let tag_llvm = self.builder.scx().type_i64().into();
        let payload_llvm = self.type_resolver.resolve(elem_ty);
        let opt_struct = self
            .builder
            .scx()
            .type_struct(&[tag_llvm, payload_llvm], false);
        let opt_struct_ty = self.builder.register_type(opt_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "rfind.out", opt_struct_ty);

        let func_id = self.builder.runtime_fn("ori_iter_rfind");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val, out_ptr],
            "",
        );

        Some(self.builder.load(opt_struct_ty, out_ptr, "rfind.result"))
    }

    /// Emit `rfold(initial, op)` — fold right-to-left (collect + fold backward).
    ///
    /// Follows the same pattern as `emit_iter_fold`, delegating to `ori_iter_rfold`.
    pub(in crate::codegen) fn emit_iter_rfold(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
        arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 3 {
            return None;
        }
        let init_val = arg_vals[1];
        let closure = arg_vals[2];

        let acc_ty = arc_func.var_type(args[1]);
        let acc_llvm_ty = self.resolve_type(acc_ty);

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Fold, Some(acc_ty));

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let acc_size = self.element_store_size(acc_ty);
        let acc_size_val = self.builder.const_i64(acc_size as i64);

        let init_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "rfold.init", acc_llvm_ty);
        self.builder.store(init_val, init_alloca);

        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "rfold.result", acc_llvm_ty);

        let func_id = self.builder.runtime_fn("ori_iter_rfold");
        self.emit_rt_call(
            func_id,
            &[
                iter_ptr,
                init_alloca,
                tramp_fn,
                closure_env,
                elem_size_val,
                acc_size_val,
                out_alloca,
            ],
            "",
        );

        Some(self.builder.load(acc_llvm_ty, out_alloca, "rfold.result"))
    }

    /// Emit `join(separator)` — join iterator elements into a string.
    ///
    /// For string-element iterators, passes `null` for `to_str_fn` (elements
    /// are already strings). For primitive types (int, float, bool, char),
    /// generates a `to_str` trampoline that calls the appropriate
    /// `ori_str_from_*` runtime function. Unsupported types (byte, Duration,
    /// Size, Ordering, structs, closures, etc.) produce a codegen error.
    ///
    /// The consumed-element release argument (`elem_dec_fn`) is computed by
    /// a compile-time provenance walk over the iterator's def chain: non-null
    /// only when every element reaching join is provably adapter-produced
    /// (a `map`-terminal chain, possibly through element-identity adapters);
    /// source-borrowed, mixed-provenance (`chain`), inner-dependent
    /// (`flat_map`), and untraceable chains all pass null — the leak-safe
    /// verdict that can never double-free.
    pub(in crate::codegen) fn emit_iter_join(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
        arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }

        let resolved_elem = self.pool.resolve_fully(elem_ty);
        let tag = self.pool.tag(resolved_elem);

        // Determine to_str_fn: null for strings, trampoline for primitives
        let (to_str_fn, to_str_env) = if tag == Tag::Str {
            // Elements are already strings — no conversion needed.
            (self.builder.const_null_ptr(), self.builder.const_null_ptr())
        } else if let Some(tramp_fn_id) = self.generate_join_to_str_trampoline(elem_ty) {
            let tramp_fn_ptr = self.builder.get_function_ptr(tramp_fn_id);
            // No closure environment needed — conversion logic is baked in.
            (tramp_fn_ptr, self.builder.const_null_ptr())
        } else {
            self.builder.record_codegen_error_with_msg(format!(
                "iter_join on {tag:?} elements not yet supported in LLVM backend"
            ));
            return Some(self.builder.poison_value);
        };

        let separator = arg_vals[1];

        // Separator is an OriStr (24-byte union: heap or SSO).
        // Pass all 3 struct fields to the runtime, which reconstructs
        // the OriStr and handles SSO vs heap internally. Direct field
        // extraction is safe here because we pass the RAW bits — the
        // runtime reinterprets them correctly regardless of SSO state.
        let sep_field0 = self
            .builder
            .extract_value(separator, FIELD_LEN, "join.sep.len")?;
        let sep_field1 = self
            .builder
            .extract_value(separator, FIELD_CAP, "join.sep.cap")?;
        let sep_field2 = self
            .builder
            .extract_value(separator, FIELD_DATA, "join.sep.data")?;

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Consumed-element release: non-null iff the provenance walk proves
        // every element adapter-produced. `get_or_generate_elem_dec_fn`
        // returns null for scalar element types, so the trampoline path
        // stays behavior-unchanged even on a map-terminal scalar chain.
        let elem_dec_fn = if args
            .first()
            .is_some_and(|&iter_var| self.join_chain_yields_fresh(iter_var, arc_func))
        {
            self.get_or_generate_elem_dec_fn(elem_ty)
        } else {
            self.builder.const_null_ptr()
        };

        // OriStr result type (always str, regardless of element type)
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "join.out", str_ty);

        let func_id = self.builder.runtime_fn("ori_iter_join");
        self.emit_rt_call(
            func_id,
            &[
                iter_ptr,
                sep_field0,
                sep_field1,
                sep_field2,
                to_str_fn,
                to_str_env,
                elem_size_val,
                elem_dec_fn,
                out_alloca,
            ],
            "",
        );

        Some(self.builder.load(str_ty, out_alloca, "join.str"))
    }

    /// Generate a `to_str` trampoline for `join` on non-string element types.
    ///
    /// The trampoline has C ABI signature `(env: ptr, elem_ptr: ptr, out_ptr: ptr) -> void`.
    /// It reads the element from `elem_ptr`, calls the appropriate `ori_str_from_*`
    /// runtime function, and writes the resulting `OriStr` to `out_ptr` (sret pattern).
    ///
    /// Returns `None` for unsupported types (structs, closures, etc.).
    fn generate_join_to_str_trampoline(&mut self, elem_ty: Idx) -> Option<FunctionId> {
        let resolved = self.pool.resolve_fully(elem_ty);
        let tag = self.pool.tag(resolved);

        // Determine the runtime conversion function name and the element
        // load type. All conversion functions write an OriStr to the sret
        // pointer, so the trampoline uses out_ptr directly as the sret arg.
        //
        // Only types whose ori_str_from_* produces the same output as
        // the interpreter's Printable::to_str() are supported. Duration/Size
        // route to their unit-aware formatters (matching the interpreter's
        // "1s"/"1kb" output). Still excluded:
        // - Ordering: formatted as "Less"/"Equal"/"Greater" not ints
        // - Byte: formatted as hex ("0xff") not decimal
        // These need proper Printable method dispatch — future work.
        let rt_func_name = match tag {
            Tag::Int => "ori_str_from_int",
            Tag::Duration => "ori_str_from_duration",
            Tag::Size => "ori_str_from_size",
            Tag::Float => "ori_str_from_float",
            Tag::Bool => "ori_str_from_bool",
            Tag::Char => "ori_str_from_char",
            _ => return None,
        };

        let tramp_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let tramp_name = format!("_ori_join_to_str_{tramp_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        let ptr_ty = self.builder.ptr_type();

        // Declare: (env: ptr, elem_ptr: ptr, out_ptr: ptr) -> void
        let func_id = self
            .builder
            .declare_void_function(&tramp_name, &[ptr_ty, ptr_ty, ptr_ty]);
        self.builder.set_module_local(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        for i in 0..3 {
            self.builder.add_noundef_param_attribute(func_id, i);
        }

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        // Parameters: env (ignored), elem_ptr, out_ptr
        let _env_ptr = self.builder.get_param(func_id, 0);
        let elem_ptr = self.builder.get_param(func_id, 1);
        let out_ptr = self.builder.get_param(func_id, 2);

        // Load element from elem_ptr using canonical type.
        // Narrowing is confined to the list storage boundary.
        let buf_elem_llvm_ty = self.resolve_type(elem_ty);
        let raw = self.builder.load(buf_elem_llvm_ty, elem_ptr, "elem");

        // With canonical types, buf_elem_llvm_ty is already the
        // canonical type (i64 for int). No sext needed — the load produces
        // the correct canonical value directly.
        let elem_val = raw;

        // Call the runtime conversion function with out_ptr as sret.
        // Runtime functions returning OriStr (24 bytes) use sret pattern:
        // void @ori_str_from_*(ptr sret(%OriStr) out_ptr, <param_ty> value)
        let rt_func = self.builder.runtime_fn(rt_func_name);
        self.builder.call(rt_func, &[out_ptr, elem_val], "");

        self.builder.ret_void();

        // Function-level LLVM IR verification.
        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!(
                    name = tramp_name,
                    "LLVM IR verification failed (generate_join_to_str_trampoline)"
                );
                self.builder.record_codegen_error();
            }
        }

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        Some(func_id)
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Provenance walk for the join consumer's element-release verdict.
    ///
    /// Walks the iterator variable's def chain through element-identity adapters
    /// (`filter`/`take`/`skip`/`rev` — single upstream iterator, each surviving
    /// element passed through unchanged) to the nearest defining node:
    ///
    /// - `map` — every element reaching join is a fresh trampoline-produced
    ///   value (consumer-owned, RC 1): the join call releases each consumed
    ///   element, so return `true`.
    /// - anything else — a source (`iter` — elements borrowed from the backing
    ///   buffer), `chain` (two operands with potentially mixed provenance),
    ///   `flat_map`/`flatten` (element ownership depends on the inner
    ///   iterators), `cycle` (re-yields the same element), an unknown adapter,
    ///   a function parameter, or a block-param merge: return `false`, the
    ///   leak-safe verdict (never a double-free; byte-identical to the
    ///   pre-release behavior for those chains).
    ///
    /// Every adapter-name match is gated on TWO conditions: the adapted
    /// operand is iterator-typed, AND `resolve_callee` — the same 4-step
    /// resolution order (`lookup_method_by_receiver` ->
    /// `lookup_method_by_return_type` -> `ctx.functions` unqualified ->
    /// `lookup_mono_dispatch`) codegen uses to dispatch the call itself —
    /// finds no matching user-defined symbol under that bare name. Only a
    /// call `resolve_callee` cannot resolve necessarily falls through to the
    /// compiler's builtin dispatcher (`try_emit_builtin_method`), so only
    /// then does the surface name genuinely identify the builtin adapter
    /// rather than a same-named user free function
    /// (`@map(xs: Iterator<str>) -> Iterator<str>`) or a `map` method on a
    /// non-iterator receiver.
    fn join_chain_yields_fresh(&self, iter_var: ArcVarId, arc_func: &ArcFunction) -> bool {
        let is_iter = |var: ArcVarId| -> bool {
            self.pool
                .tag(self.pool.resolve_fully(arc_func.var_type(var)))
                .is_iterator()
        };
        // Adapter chains are expression-local and short; the bound only guards
        // against a pathological alias cycle in malformed IR.
        let mut current = iter_var;
        for _ in 0..64 {
            match find_var_definition(arc_func, current) {
                VarDef::Alias(src) => current = src,
                VarDef::Call {
                    func,
                    args,
                    dst,
                    mono_instance_id,
                } => {
                    // A genuine iterator adapter consumes an iterator; if the first
                    // argument is not iterator-typed the name is not the builtin
                    // adapter — leak-safe decline.
                    let Some(src) = args.first().copied().filter(|&fa| is_iter(fa)) else {
                        return false;
                    };
                    // A resolved user symbol (free function, type-qualified
                    // method, or monomorphized instance) sharing this bare
                    // name is not the builtin adapter — leak-safe decline.
                    if self
                        .resolve_callee(func, &args, dst, arc_func, mono_instance_id)
                        .is_some()
                    {
                        return false;
                    }
                    match self.interner.lookup(func) {
                        "map" => return true,
                        "filter" | "take" | "skip" | "rev" => current = src,
                        _ => return false,
                    }
                }
                VarDef::Other => return false,
            }
        }
        false
    }
}

/// A variable's defining node, reduced to what the join provenance walk
/// distinguishes: a transparent alias, a named direct call (instruction
/// `Apply` or terminator `Invoke`) with its full argument list, destination,
/// and mono-dispatch index (the `resolve_callee` call-site shape), or
/// anything else.
enum VarDef {
    Alias(ArcVarId),
    Call {
        func: ori_ir::Name,
        args: Vec<ArcVarId>,
        dst: ArcVarId,
        mono_instance_id: Option<ori_ir::canon::MonoInstanceId>,
    },
    Other,
}

fn find_var_definition(arc_func: &ArcFunction, var: ArcVarId) -> VarDef {
    use ori_arc::ir::{ArcInstr, ArcTerminator, ArcValue};

    for block in &arc_func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } if *dst == var => return VarDef::Alias(*src),
                ArcInstr::Apply {
                    dst,
                    func,
                    args,
                    mono_instance_id,
                    ..
                } if *dst == var => {
                    return VarDef::Call {
                        func: *func,
                        args: args.clone(),
                        dst: *dst,
                        mono_instance_id: *mono_instance_id,
                    };
                }
                _ => {
                    if instr.defined_var() == Some(var) {
                        return VarDef::Other;
                    }
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func,
            args,
            mono_instance_id,
            ..
        } = &block.terminator
        {
            if *dst == var {
                return VarDef::Call {
                    func: *func,
                    args: args.clone(),
                    dst: *dst,
                    mono_instance_id: *mono_instance_id,
                };
            }
        }
    }
    VarDef::Other
}
