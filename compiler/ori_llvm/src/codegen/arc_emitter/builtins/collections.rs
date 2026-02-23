//! Collection type builtin methods.
//!
//! Handles `length`/`len`, `is_empty`, `concat`, `iter` for List, Str, Map, Set, Range.

declare_builtins! { emitter, ctx;
    // str
    ("str", "clone", borrow: true) => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("str", "length", borrow: true) => emitter.emit_str_length(ctx.arg_vals[0]),
    ("str", "len", borrow: true) => emitter.emit_str_length(ctx.arg_vals[0]),
    ("str", "is_empty", borrow: true) => emitter.emit_str_is_empty(ctx.arg_vals[0]),
    ("str", "concat", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            Some(emitter.emit_str_runtime_call("ori_str_concat", ctx.arg_vals[0], ctx.arg_vals[1], true))
        } else {
            None
        }
    },
    ("str", "to_str", borrow: true) => Some(ctx.arg_vals[0]),
    ("str", "iter", borrow: true) => emitter.emit_str_iter(ctx.arg_vals[0]),
    // list
    ("list", "clone", borrow: true) => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("list", "length", borrow: true) => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "len", borrow: true) => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "is_empty", borrow: true) => emitter.emit_list_is_empty(ctx.arg_vals[0]),
    ("list", "iter", borrow: true) => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_iter(ctx.arg_vals[0], ctx.receiver_ty, *element)
        } else {
            None
        }
    },
    // map
    ("map", "clone", borrow: true) => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("map", "length", borrow: true) => emitter.emit_map_length(ctx.arg_vals[0]),
    ("map", "len", borrow: true) => emitter.emit_map_length(ctx.arg_vals[0]),
    ("map", "iter", borrow: true) => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_iter(ctx.arg_vals[0], *key, *value)
        } else {
            None
        }
    },
    // Set
    ("Set", "clone", borrow: true) => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("Set", "length", borrow: true) => emitter.emit_set_length(ctx.arg_vals[0]),
    ("Set", "len", borrow: true) => emitter.emit_set_length(ctx.arg_vals[0]),
    ("Set", "iter", borrow: true) => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_list_iter(ctx.arg_vals[0], ctx.receiver_ty, *element)
        } else {
            None
        }
    },
    // range
    ("range", "iter", borrow: true) => emitter.emit_range_iter(ctx.arg_vals[0]),
}

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `str.length` — extract field 0 (len) from `{i64 len, ptr data}`.
    pub(crate) fn emit_str_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "str.len")
    }

    /// Emit `str.is_empty()` — `len == 0`.
    pub(crate) fn emit_str_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "str.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "str.is_empty"))
    }

    /// Emit `list.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_list_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "list.len")
    }

    /// Emit `list.is_empty()` — `len == 0`.
    pub(crate) fn emit_list_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "list.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
    }

    /// Emit `map.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr keys, ptr vals}`.
    pub(crate) fn emit_map_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "map.len")
    }

    /// Emit `set.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_set_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "set.len")
    }

    /// Emit `list.iter()` — call `ori_iter_from_list(data_ptr, len, elem_size)`.
    ///
    /// List layout: `{i64 len, i64 cap, ptr data}`. The runtime expects the
    /// raw element data pointer (field 2), not a pointer to the list struct.
    pub(crate) fn emit_list_iter(
        &mut self,
        receiver: ValueId,
        _receiver_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let llvm_func = self
            .builder
            .scx()
            .llmod
            .get_function("ori_iter_from_list")?;
        let func_id = self.builder.intern_function(llvm_func);

        // Extract the raw data pointer (field 2) from {i64 len, i64 cap, ptr data}
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "list.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // List length (field 0)
        let len = self
            .builder
            .extract_value(receiver, 0, "list.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Element size
        let elem_size = self.type_info.get(elem_ty).size().unwrap_or(8);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        self.builder
            .call(func_id, &[data_ptr, len, elem_size_val], "list.iter")
    }

    /// Emit `range.iter()` — call `ori_iter_from_range(start, end, step, inclusive)`.
    ///
    /// Range is lowered as a 4-element Tuple `{i64 start, i64 end, i64 step,
    /// i64 inclusive}` by `lower_range`. The inclusive flag (field 3) is
    /// stored as i64 (0 or 1) and truncated to i1 for the runtime call.
    pub(crate) fn emit_range_iter(&mut self, receiver: ValueId) -> Option<ValueId> {
        let llvm_func = self
            .builder
            .scx()
            .llmod
            .get_function("ori_iter_from_range")?;
        let func_id = self.builder.intern_function(llvm_func);

        let start = self
            .builder
            .extract_value(receiver, 0, "range.start")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let end = self
            .builder
            .extract_value(receiver, 1, "range.end")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let step = self
            .builder
            .extract_value(receiver, 2, "range.step")
            .unwrap_or_else(|| self.builder.const_i64(1));
        let incl_i64 = self
            .builder
            .extract_value(receiver, 3, "range.incl.raw")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Truncate inclusive flag from i64 to i1 for the runtime
        let bool_ty = self.builder.bool_type();
        let inclusive = self.builder.trunc(incl_i64, bool_ty, "range.inclusive");

        self.builder
            .call(func_id, &[start, end, step, inclusive], "range.iter")
    }

    /// Emit `str.iter()` — call `ori_iter_from_str(data, len)`.
    ///
    /// Str layout: `{i64 len, ptr data}`. Yields `char` (i32) values.
    pub(crate) fn emit_str_iter(&mut self, receiver: ValueId) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_iter_from_str")?;
        let func_id = self.builder.intern_function(llvm_func);

        let data_ptr = self
            .builder
            .extract_value(receiver, 1, "str.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "str.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        self.builder.call(func_id, &[data_ptr, len], "str.iter")
    }

    /// Emit `map.iter()` — call `ori_iter_from_map(keys, vals, len, key_size, val_size)`.
    ///
    /// Map layout: `{i64 len, i64 cap, ptr keys, ptr vals}`.
    /// Yields `(K, V)` tuples as concatenated key+value bytes.
    pub(crate) fn emit_map_iter(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_iter_from_map")?;
        let func_id = self.builder.intern_function(llvm_func);

        let keys = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let vals = self
            .builder
            .extract_value(receiver, 3, "map.vals")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let key_size = self.type_info.get(key_ty).size().unwrap_or(8);
        let val_size = self.type_info.get(val_ty).size().unwrap_or(8);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);

        self.builder.call(
            func_id,
            &[keys, vals, len, key_size_val, val_size_val],
            "map.iter",
        )
    }
}
