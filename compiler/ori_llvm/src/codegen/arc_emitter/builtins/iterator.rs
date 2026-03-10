//! Iterator adapter and consumer builtin methods.
//!
//! All iterator methods receive `ptr` (opaque iterator handle) as receiver.
//! Simple adapters (take, skip, chain, enumerate, zip) are direct runtime
//! calls. Closure adapters (map, filter) need trampolines to bridge Ori
//! closures to C-ABI function pointers.

declare_builtins! { emitter, ctx;
    // Internal iteration protocol
    ("Iterator", "__iter_next") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iter_next(ctx.arg_vals[0], *element)
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
        let func_id = self.builder.runtime_fn("ori_iter_next");

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Allocate scratch buffer for the element.
        let elem_llvm_ty = self.resolve_type(elem_ty);
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
        let elem_size = self.element_store_size(elem_ty);
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

        let elem_size = self.element_store_size(elem_ty);
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

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_filter");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.filter",
        )
    }
}
