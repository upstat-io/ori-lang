//! Iterator adapter and consumer builtin methods.
//!
//! All iterator methods receive `ptr` (opaque iterator handle) as receiver.
//! Simple adapters (take, skip, chain, enumerate, zip) are direct runtime
//! calls. Closure adapters (map, filter) need trampolines to bridge Ori
//! closures to C-ABI function pointers.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;
use super::trampolines::TrampolineKind;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an iterator method.
    pub(crate) fn emit_iterator_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
        arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let iter_ptr = arg_vals[0];

        match method {
            // Internal: for-loop iteration protocol
            "__iter_next" => self.emit_iter_next(iter_ptr, elem_ty),

            // Simple adapters (no closure)
            "take" => self.emit_iter_take(iter_ptr, arg_vals),
            "skip" => self.emit_iter_skip(iter_ptr, arg_vals),
            "chain" => self.emit_iter_chain(iter_ptr, arg_vals),
            "enumerate" => self.emit_iter_enumerate(iter_ptr),
            "zip" => self.emit_iter_zip(iter_ptr, arg_vals, elem_ty),

            // Closure adapters (need trampolines)
            "map" => self.emit_iter_map(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "filter" => self.emit_iter_filter(iter_ptr, arg_vals, args, arc_func, elem_ty),

            // Consumers
            "collect" => self.emit_iter_collect(iter_ptr, elem_ty),
            "count" => self.emit_iter_count(iter_ptr, elem_ty),
            "any" => self.emit_iter_any(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "all" => self.emit_iter_all(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "find" => self.emit_iter_find(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "for_each" => self.emit_iter_for_each(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "fold" => self.emit_iter_fold(iter_ptr, arg_vals, args, arc_func, elem_ty),

            _ => None,
        }
    }

    // Internal: for-loop __iter_next

    /// Emit `__iter_next(iter)` — the for-loop iteration protocol.
    ///
    /// Calls `ori_iter_next(iter, scratch, elem_size)` and returns a struct
    /// `{i64 tag, T element}` where tag=0 means done, tag=1 means has element.
    /// The ARC IR for-loop projects field 0 (tag) and field 1 (element).
    pub(in crate::codegen) fn emit_iter_next(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func = self.builder.scx().llmod.get_function("ori_iter_next")?;
        let func_id = self.builder.intern_function(func);

        // Compute element size — use TypeInfo for primitives/known types,
        // fall back to LLVM type layout for compound types (tuples, structs).
        let elem_size = self.type_info.get(elem_ty).size().unwrap_or_else(|| {
            let llvm_ty = self.type_resolver.resolve(elem_ty);
            TypeLayoutResolver::type_store_size(llvm_ty)
        });
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Allocate scratch buffer for the element.
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let scratch = self.builder.create_entry_alloca(
            self.current_function,
            "iter_next.scratch",
            elem_llvm_ty,
        );

        // Call ori_iter_next(iter, scratch, elem_size) -> i8 (0=done, 1=has element)
        let has_next_i8 = self.builder.call(
            func_id,
            &[iter_ptr, scratch, elem_size_val],
            "iter_next.has",
        )?;

        // Zero-extend i8 → i64 for ARC IR tag (projected as Idx::INT).
        let i64_ty = self.builder.i64_type();
        let tag = self.builder.zext(has_next_i8, i64_ty, "iter_next.tag");

        // Load element from scratch buffer.
        let elem = self.builder.load(elem_llvm_ty, scratch, "iter_next.elem");

        // Build result struct {i64, elem_type}.
        let elem_raw_ty = self.type_resolver.resolve(elem_ty);
        let i64_raw = self.builder.scx().type_i64().into();
        let result_struct = self
            .builder
            .scx()
            .type_struct(&[i64_raw, elem_raw_ty], false);
        let result_ty_id = self.builder.register_type(result_struct.into());

        Some(
            self.builder
                .build_struct(result_ty_id, &[tag, elem], "iter_next"),
        )
    }

    // Simple adapters

    fn emit_iter_take(&mut self, iter_ptr: ValueId, arg_vals: &[ValueId]) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let n = arg_vals[1];
        let func = self.builder.scx().llmod.get_function("ori_iter_take")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(func_id, &[iter_ptr, n], "iter.take")
    }

    fn emit_iter_skip(&mut self, iter_ptr: ValueId, arg_vals: &[ValueId]) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let n = arg_vals[1];
        let func = self.builder.scx().llmod.get_function("ori_iter_skip")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(func_id, &[iter_ptr, n], "iter.skip")
    }

    fn emit_iter_chain(&mut self, iter_ptr: ValueId, arg_vals: &[ValueId]) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let other = arg_vals[1];
        let func = self.builder.scx().llmod.get_function("ori_iter_chain")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(func_id, &[iter_ptr, other], "iter.chain")
    }

    fn emit_iter_enumerate(&mut self, iter_ptr: ValueId) -> Option<ValueId> {
        let func = self
            .builder
            .scx()
            .llmod
            .get_function("ori_iter_enumerate")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(func_id, &[iter_ptr], "iter.enumerate")
    }

    fn emit_iter_zip(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let other = arg_vals[1];
        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let func = self.builder.scx().llmod.get_function("ori_iter_zip")?;
        let func_id = self.builder.intern_function(func);
        self.builder
            .call(func_id, &[iter_ptr, other, elem_size_val], "iter.zip")
    }

    // Closure adapters

    fn emit_iter_map(
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
        let closure = arg_vals[1];

        // Determine result type from the closure's function type
        let closure_ty = arc_func.var_type(args[1]);
        let result_ty = if self.pool.tag(closure_ty) == ori_types::Tag::Function {
            Some(self.pool.function_return(closure_ty))
        } else {
            None
        };

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Map, result_ty);

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func = self.builder.scx().llmod.get_function("ori_iter_map")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.map",
        )
    }

    fn emit_iter_filter(
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

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func = self.builder.scx().llmod.get_function("ori_iter_filter")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.filter",
        )
    }

    // Consumers

    fn emit_iter_collect(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func = self.builder.scx().llmod.get_function("ori_iter_collect")?;
        let func_id = self.builder.intern_function(func);

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // sret pattern: allocate output list struct {i64 len, i64 cap, ptr data}
        let i64_llvm = self.builder.scx().type_i64().into();
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let list_struct = self
            .builder
            .scx()
            .type_struct(&[i64_llvm, i64_llvm, ptr_llvm], false);
        let list_struct_ty = self.builder.register_type(list_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "collect.out", list_struct_ty);

        self.builder
            .call(func_id, &[iter_ptr, elem_size_val, out_ptr], "");

        // Load the result list from the sret alloca
        Some(self.builder.load(list_struct_ty, out_ptr, "collect.list"))
    }

    fn emit_iter_count(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func = self.builder.scx().llmod.get_function("ori_iter_count")?;
        let func_id = self.builder.intern_function(func);

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        self.builder
            .call(func_id, &[iter_ptr, elem_size_val], "iter.count")
    }

    fn emit_iter_any(
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

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func = self.builder.scx().llmod.get_function("ori_iter_any")?;
        let func_id = self.builder.intern_function(func);
        let result = self.builder.call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.any",
        )?;

        // Convert i8 -> i1
        let zero = self.builder.const_i64(0);
        let i8_ty = self.builder.i8_type();
        let zero_i8 = self.builder.trunc(zero, i8_ty, "zero");
        Some(self.builder.icmp_ne(result, zero_i8, "iter.any.bool"))
    }

    fn emit_iter_all(
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

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func = self.builder.scx().llmod.get_function("ori_iter_all")?;
        let func_id = self.builder.intern_function(func);
        let result = self.builder.call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.all",
        )?;

        // Convert i8 -> i1
        let zero = self.builder.const_i64(0);
        let i8_ty = self.builder.i8_type();
        let zero_i8 = self.builder.trunc(zero, i8_ty, "zero");
        Some(self.builder.icmp_ne(result, zero_i8, "iter.all.bool"))
    }

    fn emit_iter_find(
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

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func = self.builder.scx().llmod.get_function("ori_iter_find")?;
        let func_id = self.builder.intern_function(func);

        // sret pattern for Option<T> result
        // Option layout: {i64 tag, T payload} — matches TypeLayoutResolver
        let i64_llvm = self.builder.scx().type_i64().into();
        let opt_struct = self.builder.scx().type_struct(&[i64_llvm, i64_llvm], false);
        let opt_struct_ty = self.builder.register_type(opt_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "find.out", opt_struct_ty);

        self.builder.call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val, out_ptr],
            "",
        );

        Some(self.builder.load(opt_struct_ty, out_ptr, "find.result"))
    }

    fn emit_iter_for_each(
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
            self.build_trampoline(closure, elem_ty, TrampolineKind::ForEach, None);

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func = self.builder.scx().llmod.get_function("ori_iter_for_each")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "",
        );

        // for_each returns unit
        Some(self.builder.const_i64(0))
    }

    fn emit_iter_fold(
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

        // Determine accumulator type from init value
        let acc_ty = arc_func.var_type(args[1]);
        let acc_llvm_ty = self.resolve_type(acc_ty);

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Fold, Some(acc_ty));

        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let acc_size = self.type_info.get(acc_ty).size().unwrap_or(8);
        let acc_size_val = self.builder.const_i64(acc_size as i64);

        // Store init value to alloca for passing as ptr
        let init_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "fold.init", acc_llvm_ty);
        self.builder.store(init_val, init_alloca);

        // Output alloca for sret
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "fold.out", acc_llvm_ty);

        let func = self.builder.scx().llmod.get_function("ori_iter_fold")?;
        let func_id = self.builder.intern_function(func);
        self.builder.call(
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

        Some(self.builder.load(acc_llvm_ty, out_alloca, "fold.result"))
    }
}
