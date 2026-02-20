//! Iterator method lowering: map, filter, take, skip, enumerate, collect, count.
//!
//! Dispatches iterator adapter and consumer methods to the corresponding
//! `ori_iter_*` runtime functions. Map and filter require trampoline generation
//! to bridge typed closures to the runtime's generic pointer-based ABI.

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
            "map" => self.lower_iter_map(recv, element, args),
            "filter" => self.lower_iter_filter(recv, element, args),
            "take" => self.lower_iter_take(recv, args),
            "skip" => self.lower_iter_skip(recv, args),
            "enumerate" => self.lower_iter_enumerate(recv),
            "collect" => self.lower_iter_collect(recv, element),
            "count" => self.lower_iter_count(recv, element),
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
