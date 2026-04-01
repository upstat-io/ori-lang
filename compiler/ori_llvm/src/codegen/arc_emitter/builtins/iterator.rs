//! Iterator adapter and consumer builtin methods.
//!
//! All iterator methods receive `ptr` (opaque iterator handle) as receiver.
//! Simple adapters (take, skip, chain, enumerate, zip) are direct runtime
//! calls. Closure adapters (map, filter) need trampolines to bridge Ori
//! closures to C-ABI function pointers.

declare_builtins! { emitter, ctx;
    // Internal iteration protocol (dead code path — __iter_next is intercepted
    // by try_emit_protocol before reaching builtin method dispatch)
    ("Iterator", "__iter_next") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iter_next(ctx.arg_vals[0], *element).map(|(tag, _, _)| tag)
        } else {
            None
        }
    },
    // Simple adapters
    ("Iterator", "take") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "skip") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "chain") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "enumerate") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "zip") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Closure adapters
    ("Iterator", "map") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "filter") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Closure/simple adapters: new
    ("Iterator", "flatten") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "flat_map") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "cycle") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // DEI adapters/consumers
    ("DoubleEndedIterator", "rev") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "last") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "rfind") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "rfold") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Consumer: join (Iterator, not DEI)
    ("Iterator", "join") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Consumers
    ("Iterator", "collect") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "count") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "any") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "all") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "find") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "for_each") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "fold") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
}

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::{LLVMTypeId, ValueId};

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
            // Internal: for-loop iteration protocol (dead code path — __iter_next
            // is intercepted by try_emit_protocol before reaching emit_iterator_method)
            "__iter_next" => self
                .emit_iter_next(iter_ptr, elem_ty)
                .map(|(tag, _, _)| tag),

            // Simple adapters (no closure)
            "take" => self.emit_iter_take(iter_ptr, arg_vals),
            "skip" => self.emit_iter_skip(iter_ptr, arg_vals),
            "chain" => self.emit_iter_chain(iter_ptr, arg_vals),
            "enumerate" => self.emit_iter_enumerate(iter_ptr),
            "zip" => self.emit_iter_zip(iter_ptr, arg_vals, elem_ty),

            // Closure adapters (need trampolines)
            "map" => self.emit_iter_map(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "filter" => self.emit_iter_filter(iter_ptr, arg_vals, args, arc_func, elem_ty),

            // Adapters: new (runtime-backed)
            "flatten" => self.emit_iter_flatten(iter_ptr, elem_ty),
            "flat_map" => self.emit_iter_flat_map(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "cycle" => self.emit_iter_cycle(iter_ptr, elem_ty),
            "rev" => self.emit_iter_rev(iter_ptr, elem_ty),

            // Consumers
            "collect" => self.emit_iter_collect(iter_ptr, elem_ty),
            "count" => self.emit_iter_count(iter_ptr, elem_ty),
            "any" => self.emit_iter_any(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "all" => self.emit_iter_all(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "find" => self.emit_iter_find(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "for_each" => self.emit_iter_for_each(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "fold" => self.emit_iter_fold(iter_ptr, arg_vals, args, arc_func, elem_ty),

            // Consumers: new (runtime-backed)
            "last" => self.emit_iter_last(iter_ptr, elem_ty),
            "rfind" => self.emit_iter_rfind(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "rfold" => self.emit_iter_rfold(iter_ptr, arg_vals, args, arc_func, elem_ty),
            "join" => self.emit_iter_join(iter_ptr, arg_vals, elem_ty),

            _ => None,
        }
    }

    // Internal: for-loop __iter_next

    /// Emit `__iter_next(iter)` — the for-loop iteration protocol.
    ///
    /// Calls `ori_iter_next(iter, scratch, elem_size)` and returns a decomposed
    /// `(tag, scratch_ptr, elem_llvm_ty)` triple. The tag is an i64 (0=done,
    /// 1=has element) and the scratch pointer holds the element data.
    ///
    /// The caller is responsible for registering the decomposed result in
    /// `iter_next_decomposed` so that `emit_project` can extract the tag
    /// (field 0) and element (field 1) without building an intermediate
    /// `{i64, T}` wrapper struct.
    pub(in crate::codegen) fn emit_iter_next(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<(ValueId, ValueId, LLVMTypeId)> {
        let func_id = self.builder.runtime_fn("ori_iter_next");

        // Use narrowed element size/type for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Allocate scratch buffer for the element (narrowed type if applicable).
        let elem_llvm_ty = self.int_element_llvm_type(elem_ty);
        let scratch = self.builder.create_entry_alloca(
            self.current_function,
            "iter_next.scratch",
            elem_llvm_ty,
        );

        // Call ori_iter_next(iter, scratch, elem_size) -> i8 (0=done, 1=has element)
        let has_next_i8 = self.emit_rt_call(
            func_id,
            &[iter_ptr, scratch, elem_size_val],
            "iter_next.has",
        )?;

        // Zero-extend i8 → i64 for ARC IR tag (projected as Idx::INT).
        let i64_ty = self.builder.i64_type();
        let tag = self.builder.zext(has_next_i8, i64_ty, "iter_next.tag");

        Some((tag, scratch, elem_llvm_ty))
    }

    // Simple adapters

    fn emit_iter_take(&mut self, iter_ptr: ValueId, arg_vals: &[ValueId]) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let n = arg_vals[1];
        let func_id = self.builder.runtime_fn("ori_iter_take");
        self.emit_rt_call(func_id, &[iter_ptr, n], "iter.take")
    }

    fn emit_iter_skip(&mut self, iter_ptr: ValueId, arg_vals: &[ValueId]) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let n = arg_vals[1];
        let func_id = self.builder.runtime_fn("ori_iter_skip");
        self.emit_rt_call(func_id, &[iter_ptr, n], "iter.skip")
    }

    fn emit_iter_chain(&mut self, iter_ptr: ValueId, arg_vals: &[ValueId]) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let other = arg_vals[1];
        let func_id = self.builder.runtime_fn("ori_iter_chain");
        self.emit_rt_call(func_id, &[iter_ptr, other], "iter.chain")
    }

    fn emit_iter_enumerate(&mut self, iter_ptr: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_enumerate");
        self.emit_rt_call(func_id, &[iter_ptr], "iter.enumerate")
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
        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let func_id = self.builder.runtime_fn("ori_iter_zip");
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_map");
        self.emit_rt_call(
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_filter");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.filter",
        )
    }

    // New adapters (runtime-backed)

    fn emit_iter_flatten(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let func_id = self.builder.runtime_fn("ori_iter_flatten");
        self.emit_rt_call(func_id, &[iter_ptr, elem_size_val], "iter.flatten")
    }

    fn emit_iter_flat_map(
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
        // flat_map(f) = map(f).flatten()
        // First apply map, then flatten the result
        let mapped = self.emit_iter_map(iter_ptr, arg_vals, args, arc_func, elem_ty)?;
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let func_id = self.builder.runtime_fn("ori_iter_flatten");
        self.emit_rt_call(func_id, &[mapped, elem_size_val], "iter.flat_map")
    }

    fn emit_iter_cycle(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let func_id = self.builder.runtime_fn("ori_iter_cycle");
        self.emit_rt_call(func_id, &[iter_ptr, elem_size_val], "iter.cycle")
    }

    fn emit_iter_rev(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let func_id = self.builder.runtime_fn("ori_iter_rev");
        self.emit_rt_call(func_id, &[iter_ptr, elem_size_val], "iter.rev")
    }
}
