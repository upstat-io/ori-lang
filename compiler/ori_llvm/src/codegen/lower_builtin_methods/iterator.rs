//! Iterator method lowering for LLVM codegen.
//!
//! Dispatches iterator adapter and consumer methods to the corresponding
//! `ori_iter_*` runtime functions. Closure-based methods require trampoline
//! generation to bridge typed closures to the runtime's generic pointer ABI.
//!
//! # Adapters
//! map, filter, take, skip, enumerate, zip, chain
//!
//! # Consumers
//! collect, count, any, all, find, `for_each`, fold

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Dispatch iterator methods on `Iterator<T>` values.
    pub(super) fn lower_iterator_method(
        &mut self,
        recv: ValueId,
        element: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            // Adapters
            "map" => self.lower_iter_map(recv, element, args),
            "filter" => self.lower_iter_filter(recv, element, args),
            "take" => self.lower_iter_take(recv, args),
            "skip" => self.lower_iter_skip(recv, args),
            "enumerate" => self.lower_iter_enumerate(recv),
            "zip" => self.lower_iter_zip(recv, element, args),
            "chain" => self.lower_iter_chain(recv, args),
            // Consumers
            "collect" => self.lower_iter_collect(recv, element),
            "count" => self.lower_iter_count(recv, element),
            "any" => self.lower_iter_any(recv, element, args),
            "all" => self.lower_iter_all(recv, element, args),
            "find" => self.lower_iter_find(recv, element, args),
            "for_each" => self.lower_iter_for_each(recv, element, args),
            "fold" => self.lower_iter_fold(recv, element, args),
            _ => None,
        }
    }

    /// `.iter()` on List — extract data/len, call `ori_iter_from_list`.
    pub(super) fn lower_list_iter(&mut self, recv: ValueId, element: Idx) -> Option<ValueId> {
        let len = self.builder.extract_value(recv, 0, "list.len")?;
        let data = self.builder.extract_value(recv, 2, "list.data")?;
        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let from_list = self.builder.get_or_declare_function(
            "ori_iter_from_list",
            &[ptr_ty, i64_ty, i64_ty],
            ptr_ty,
        );
        self.builder
            .call(from_list, &[data, len, elem_size], "list.iter")
    }

    /// `.iter()` on Range — extract start/end/inclusive, call `ori_iter_from_range`.
    pub(super) fn lower_range_iter(&mut self, recv: ValueId) -> Option<ValueId> {
        let start = self.builder.extract_value(recv, 0, "range.start")?;
        let end = self.builder.extract_value(recv, 1, "range.end")?;
        let inclusive = self.builder.extract_value(recv, 2, "range.incl")?;
        let step = self.builder.const_i64(1);

        let i64_ty = self.builder.i64_type();
        let bool_ty = self.builder.bool_type();
        let ptr_ty = self.builder.ptr_type();
        let from_range = self.builder.get_or_declare_function(
            "ori_iter_from_range",
            &[i64_ty, i64_ty, i64_ty, bool_ty],
            ptr_ty,
        );
        self.builder
            .call(from_range, &[start, end, step, inclusive], "range.iter")
    }

    // -- Adapters --

    /// `.map(f)` — generate trampoline, call `ori_iter_map`.
    fn lower_iter_map(&mut self, iter: ValueId, element: Idx, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let closure_id = *arg_ids.first()?;
        let closure_val = self.lower(closure_id)?;

        // Determine U from closure type (T) -> U
        let closure_type = self.expr_type(closure_id);
        let result_elem = self.pool.function_return(closure_type);

        let (wrapper, tramp_fn) =
            self.generate_map_trampoline(closure_val, element, result_elem)?;

        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let map_fn = self.builder.get_or_declare_function(
            "ori_iter_map",
            &[ptr_ty, ptr_ty, ptr_ty, i64_ty],
            ptr_ty,
        );
        self.builder
            .call(map_fn, &[iter, tramp_fn, wrapper, elem_size], "iter.map")
    }

    /// `.filter(f)` — generate trampoline, call `ori_iter_filter`.
    fn lower_iter_filter(
        &mut self,
        iter: ValueId,
        element: Idx,
        args: CanRange,
    ) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let closure_id = *arg_ids.first()?;
        let closure_val = self.lower(closure_id)?;

        let (wrapper, tramp_fn) = self.generate_filter_trampoline(closure_val, element)?;

        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let filter_fn = self.builder.get_or_declare_function(
            "ori_iter_filter",
            &[ptr_ty, ptr_ty, ptr_ty, i64_ty],
            ptr_ty,
        );
        self.builder.call(
            filter_fn,
            &[iter, tramp_fn, wrapper, elem_size],
            "iter.filter",
        )
    }

    /// `.take(n)` — call `ori_iter_take`.
    fn lower_iter_take(&mut self, iter: ValueId, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let n = self.lower(*arg_ids.first()?)?;

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let take_fn =
            self.builder
                .get_or_declare_function("ori_iter_take", &[ptr_ty, i64_ty], ptr_ty);
        self.builder.call(take_fn, &[iter, n], "iter.take")
    }

    /// `.skip(n)` — call `ori_iter_skip`.
    fn lower_iter_skip(&mut self, iter: ValueId, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let n = self.lower(*arg_ids.first()?)?;

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let skip_fn =
            self.builder
                .get_or_declare_function("ori_iter_skip", &[ptr_ty, i64_ty], ptr_ty);
        self.builder.call(skip_fn, &[iter, n], "iter.skip")
    }

    /// `.enumerate()` — call `ori_iter_enumerate`.
    fn lower_iter_enumerate(&mut self, iter: ValueId) -> Option<ValueId> {
        let ptr_ty = self.builder.ptr_type();
        let enumerate_fn =
            self.builder
                .get_or_declare_function("ori_iter_enumerate", &[ptr_ty], ptr_ty);
        self.builder.call(enumerate_fn, &[iter], "iter.enumerate")
    }

    /// `.zip(other)` — lower other iterator, call `ori_iter_zip`.
    fn lower_iter_zip(&mut self, iter: ValueId, element: Idx, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let other_id = *arg_ids.first()?;
        let other_val = self.lower(other_id)?;

        let left_elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let zip_fn =
            self.builder
                .get_or_declare_function("ori_iter_zip", &[ptr_ty, ptr_ty, i64_ty], ptr_ty);
        self.builder
            .call(zip_fn, &[iter, other_val, left_elem_size], "iter.zip")
    }

    /// `.chain(other)` — lower other iterator, call `ori_iter_chain`.
    fn lower_iter_chain(&mut self, iter: ValueId, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let other_id = *arg_ids.first()?;
        let other_val = self.lower(other_id)?;

        let ptr_ty = self.builder.ptr_type();
        let chain_fn =
            self.builder
                .get_or_declare_function("ori_iter_chain", &[ptr_ty, ptr_ty], ptr_ty);
        self.builder
            .call(chain_fn, &[iter, other_val], "iter.chain")
    }

    // -- Consumers --

    /// `.collect()` — allocate sret buffer, call `ori_iter_collect`, load result.
    ///
    /// Uses sret pattern to avoid returning >16-byte struct (JIT `FastISel` bug).
    /// Runtime writes `OriList { i64 len, i64 cap, ptr data }` to output pointer.
    fn lower_iter_collect(&mut self, iter: ValueId, element: Idx) -> Option<ValueId> {
        let elem_size = self.compute_elem_byte_size(element);

        // Allocate stack space for OriList { i64, i64, ptr } = 24 bytes
        let list_llvm_ty = self.builder.register_type(
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
        let out_ptr = self.builder.alloca(list_llvm_ty, "iter.collect.buf");

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let collect_fn = self
            .builder
            .get_or_declare_void_function("ori_iter_collect", &[ptr_ty, i64_ty, ptr_ty]);
        self.builder
            .call(collect_fn, &[iter, elem_size, out_ptr], "iter.collect.call");

        // Load the completed OriList struct
        Some(self.builder.load(list_llvm_ty, out_ptr, "iter.collected"))
    }

    /// `.count()` — call `ori_iter_count`.
    fn lower_iter_count(&mut self, iter: ValueId, element: Idx) -> Option<ValueId> {
        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let count_fn =
            self.builder
                .get_or_declare_function("ori_iter_count", &[ptr_ty, i64_ty], i64_ty);
        self.builder
            .call(count_fn, &[iter, elem_size], "iter.count")
    }

    /// `.any(f)` — reuse filter trampoline (predicate), call `ori_iter_any`, convert i8 -> bool.
    fn lower_iter_any(&mut self, iter: ValueId, element: Idx, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let closure_id = *arg_ids.first()?;
        let closure_val = self.lower(closure_id)?;

        let (wrapper, tramp_fn) = self.generate_filter_trampoline(closure_val, element)?;
        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let i8_ty = self.builder.i8_type();
        let any_fn = self.builder.get_or_declare_function(
            "ori_iter_any",
            &[ptr_ty, ptr_ty, ptr_ty, i64_ty],
            i8_ty,
        );
        let result =
            self.builder
                .call(any_fn, &[iter, tramp_fn, wrapper, elem_size], "iter.any")?;

        // Convert i8 -> i1 (bool): result != 0
        let zero = self.builder.const_i8(0);
        Some(self.builder.icmp_ne(result, zero, "iter.any.bool"))
    }

    /// `.all(f)` — reuse filter trampoline (predicate), call `ori_iter_all`, convert i8 -> bool.
    fn lower_iter_all(&mut self, iter: ValueId, element: Idx, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let closure_id = *arg_ids.first()?;
        let closure_val = self.lower(closure_id)?;

        let (wrapper, tramp_fn) = self.generate_filter_trampoline(closure_val, element)?;
        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let i8_ty = self.builder.i8_type();
        let all_fn = self.builder.get_or_declare_function(
            "ori_iter_all",
            &[ptr_ty, ptr_ty, ptr_ty, i64_ty],
            i8_ty,
        );
        let result =
            self.builder
                .call(all_fn, &[iter, tramp_fn, wrapper, elem_size], "iter.all")?;

        let zero = self.builder.const_i8(0);
        Some(self.builder.icmp_ne(result, zero, "iter.all.bool"))
    }

    /// `.find(f)` — reuse filter trampoline, alloca sret for Option<T>, call `ori_iter_find`.
    fn lower_iter_find(&mut self, iter: ValueId, element: Idx, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let closure_id = *arg_ids.first()?;
        let closure_val = self.lower(closure_id)?;

        let (wrapper, tramp_fn) = self.generate_filter_trampoline(closure_val, element)?;
        let elem_size = self.compute_elem_byte_size(element);

        // Allocate sret buffer for Option<T> = { i8 tag, T payload }
        // LLVM layout: { i8, padding, T } — use {i8, i64} for i64 payload
        let option_llvm_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i8().into(),
                        self.type_resolver.resolve(element),
                    ],
                    false,
                )
                .into(),
        );
        let out_ptr = self.builder.alloca(option_llvm_ty, "iter.find.buf");

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let find_fn = self.builder.get_or_declare_void_function(
            "ori_iter_find",
            &[ptr_ty, ptr_ty, ptr_ty, i64_ty, ptr_ty],
        );
        self.builder.call(
            find_fn,
            &[iter, tramp_fn, wrapper, elem_size, out_ptr],
            "iter.find.call",
        );

        Some(self.builder.load(option_llvm_ty, out_ptr, "iter.found"))
    }

    /// `.for_each(f)` — generate `for_each` trampoline, call `ori_iter_for_each`.
    fn lower_iter_for_each(
        &mut self,
        iter: ValueId,
        element: Idx,
        args: CanRange,
    ) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let closure_id = *arg_ids.first()?;
        let closure_val = self.lower(closure_id)?;

        let (wrapper, tramp_fn) = self.generate_for_each_trampoline(closure_val, element)?;
        let elem_size = self.compute_elem_byte_size(element);

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let foreach_fn = self
            .builder
            .get_or_declare_void_function("ori_iter_for_each", &[ptr_ty, ptr_ty, ptr_ty, i64_ty]);
        self.builder.call(
            foreach_fn,
            &[iter, tramp_fn, wrapper, elem_size],
            "iter.foreach.call",
        );

        // for_each returns void — produce unit value
        Some(self.builder.const_i64(0))
    }

    /// `.fold(init, op)` — generate fold trampoline, alloca sret, call `ori_iter_fold`.
    fn lower_iter_fold(&mut self, iter: ValueId, element: Idx, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        if arg_ids.len() < 2 {
            return None;
        }
        let init_id = arg_ids[0];
        let closure_id = arg_ids[1];

        let init_val = self.lower(init_id)?;
        let closure_val = self.lower(closure_id)?;

        // Determine acc_type from init's type (same as fold return type)
        let acc_type = self.expr_type(init_id);

        let (wrapper, tramp_fn) = self.generate_fold_trampoline(closure_val, acc_type, element)?;

        let elem_size = self.compute_elem_byte_size(element);
        let acc_size_val = {
            let size = self.type_info.get(acc_type).size().unwrap_or(8);
            self.builder.const_i64(size as i64)
        };

        // Alloca for init value (store to stack so we can pass ptr)
        let acc_llvm_ty = self
            .builder
            .register_type(self.type_resolver.resolve(acc_type));
        let init_ptr = self.builder.alloca(acc_llvm_ty, "iter.fold.init");
        self.builder.store(init_val, init_ptr);

        // Alloca for result
        let result_ptr = self.builder.alloca(acc_llvm_ty, "iter.fold.result");

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let fold_fn = self.builder.get_or_declare_void_function(
            "ori_iter_fold",
            &[ptr_ty, ptr_ty, ptr_ty, ptr_ty, i64_ty, i64_ty, ptr_ty],
        );
        self.builder.call(
            fold_fn,
            &[
                iter,
                init_ptr,
                tramp_fn,
                wrapper,
                elem_size,
                acc_size_val,
                result_ptr,
            ],
            "iter.fold.call",
        );

        Some(self.builder.load(acc_llvm_ty, result_ptr, "iter.folded"))
    }

    // -- Helpers --

    /// Compute element byte size as an i64 constant.
    ///
    /// Uses `TypeInfo::size()` for known types. Falls back to 8 for types
    /// where size is not statically known (Tuple, Struct, Enum).
    fn compute_elem_byte_size(&mut self, elem_type: Idx) -> ValueId {
        let size = self.type_info.get(elem_type).size().unwrap_or(8);
        self.builder.const_i64(size as i64)
    }
}
