//! Iterator adapter emission.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use crate::codegen::ValueId;

use super::super::ArcIrEmitter;
use super::trampolines::TrampolineKind;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Dispatch a named iterator method (`take` / `map` / `flatten` / `cycle`
    /// / `collect` / …) to its per-method emitter. Returns the result
    /// `ValueId`, or `None` for void-returning consumers (`for_each`).
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
            "join" => self.emit_iter_join(iter_ptr, arg_vals, args, arc_func, elem_ty),

            _ => None,
        }
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
        // Use canonical element size — narrowing confined to list boundary.
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

        // Use canonical element size — narrowing confined to list boundary.
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let output_dec_fn = match result_ty {
            Some(ty) => self.get_or_generate_elem_dec_fn(ty),
            None => self.builder.const_null_ptr(),
        };

        let func_id = self.builder.runtime_fn("ori_iter_map");
        self.emit_rt_call(
            func_id,
            &[
                iter_ptr,
                tramp_fn,
                closure_env,
                elem_size_val,
                output_dec_fn,
            ],
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

        // Use canonical element size — narrowing confined to list boundary.
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_filter");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.filter",
        )
    }

    // New adapters (runtime-backed)

    /// Compute the inner element size to pass as `inner_elem_size` to
    /// `ori_iter_flatten`.
    ///
    /// The runtime contract: outer source yields 8-byte iterator handles; the
    /// `inner_elem_size` argument is `sizeof(inner element)` — the byte size
    /// of elements yielded by each inner iterator. See
    /// `ori_rt/src/iterator/adapters.rs` `ori_iter_flatten` and
    /// `ori_rt/src/iterator/next.rs::next_flattened`.
    ///
    /// `outer_elem_ty` MUST be an iterator type — `Iterator<T>` or
    /// `DoubleEndedIterator<T>`. The returned size is the canonical byte size
    /// of `T` (NR-3 — iterator pipeline uses canonical types).
    ///
    /// Production `assert!()` (not `debug_assert!`): the value is fed to an
    /// `extern "C"` runtime stride; silent stripping in release would propagate
    /// a wrong stride into memory operations. The `pool.iterator_elem` callee
    /// has its own `debug_assert!` that strips in release; this assert is the
    /// load-bearing guard that catches a violation BEFORE `iterator_elem`
    /// reads garbage from the data field.
    ///
    /// `pool.resolve_fully` is called first (every type index is fully
    /// resolved before LLVM type construction) to chase any binding-chain
    /// aliases before the iterator-tag check.
    fn flatten_inner_elem_size(&self, outer_elem_ty: Idx) -> i64 {
        let resolved = self.pool.resolve_fully(outer_elem_ty);
        let outer_tag = self.pool.tag(resolved);
        assert!(
            outer_tag.is_iterator(),
            "ori_iter_flatten requires outer iterator to yield iterator handles, \
             got tag {outer_tag:?} for elem_ty {outer_elem_ty:?} (resolved {resolved:?})",
        );
        let inner_ty = self.pool.iterator_elem(resolved);
        self.element_store_size(inner_ty) as i64
    }

    fn emit_iter_flatten(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        // `elem_ty` here is the OUTER iterator's element type — itself an
        // iterator handle (`Iterator<U>` or `DoubleEndedIterator<U>`). Peel
        // to compute the inner element's canonical byte size, which is what
        // the runtime expects as `inner_elem_size`.
        let inner_elem_size = self.flatten_inner_elem_size(elem_ty);
        let inner_elem_size_val = self.builder.const_i64(inner_elem_size);
        let func_id = self.builder.runtime_fn("ori_iter_flatten");
        self.emit_rt_call(func_id, &[iter_ptr, inner_elem_size_val], "iter.flatten")
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
        // First apply map, then flatten the result.
        let mapped = self.emit_iter_map(iter_ptr, arg_vals, args, arc_func, elem_ty)?;

        // The mapped iterator's element type is the closure's RETURN type —
        // an iterator handle (`Iterator<U>` or `DoubleEndedIterator<U>`).
        // resolve_fully chases binding-chain aliases;
        // Tag::Function guard mirrors `emit_iter_map:360` defensive pattern
        // for unresolved-type cases that shouldn't reach codegen but are
        // returned as None rather than asserted (consistency with sibling
        // emitter precedent).
        let closure_ty = self.pool.resolve_fully(arc_func.var_type(args[1]));
        let closure_return = if self.pool.tag(closure_ty) == ori_types::Tag::Function {
            self.pool.function_return(closure_ty)
        } else {
            return None;
        };

        // A divergent transform is a valid flat_map input: `Never` absorbs
        // the iterator-return constraint during type checking. No inner
        // iterator can exist on that path. The mapped source either exhausts
        // before invoking the transform or the transform diverges, so the
        // flatten adapter never observes an inner handle. Zero is the runtime
        // stride for this uninhabited output and is accepted by
        // `assert_elem_size`; keep `flatten_inner_elem_size` strict for every
        // inhabited return type so a non-iterator cannot become a stride.
        let resolved_return = self.pool.resolve_fully(closure_return);
        let inner_elem_size = if self.pool.tag(resolved_return) == ori_types::Tag::Never {
            0
        } else {
            self.flatten_inner_elem_size(resolved_return)
        };
        let inner_elem_size_val = self.builder.const_i64(inner_elem_size);
        let func_id = self.builder.runtime_fn("ori_iter_flatten");
        self.emit_rt_call(func_id, &[mapped, inner_elem_size_val], "iter.flat_map")
    }

    fn emit_iter_cycle(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        // Use canonical element size — narrowing confined to list boundary.
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        // The replay buffer OWNS its element copies: pass the element inc/dec fns
        // (null for scalar elements) so next_cycled incs on store and Drop decs
        // each stored master. Mirrors how Map threads key/val_dec_fn.
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let func_id = self.builder.runtime_fn("ori_iter_cycle");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, elem_size_val, elem_inc_fn, elem_dec_fn],
            "iter.cycle",
        )
    }

    fn emit_iter_rev(&mut self, iter_ptr: ValueId, elem_ty: Idx) -> Option<ValueId> {
        // Use canonical element size — narrowing confined to list boundary.
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        // The collected buffer OWNS its element copies: pass the element inc/dec
        // fns (null for scalar) so ori_iter_rev incs on collect and Drop decs each.
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let func_id = self.builder.runtime_fn("ori_iter_rev");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, elem_size_val, elem_inc_fn, elem_dec_fn],
            "iter.rev",
        )
    }
}
