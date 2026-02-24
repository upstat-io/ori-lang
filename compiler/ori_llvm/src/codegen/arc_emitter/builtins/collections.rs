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
    ("str", "contains", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_contains", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "starts_with", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_starts_with", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "ends_with", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_ends_with", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "trim", borrow: true) => emitter.emit_str_unary_call("ori_str_trim", ctx.arg_vals[0]),
    ("str", "to_uppercase", borrow: true) => emitter.emit_str_unary_call("ori_str_to_uppercase", ctx.arg_vals[0]),
    ("str", "to_lowercase", borrow: true) => emitter.emit_str_unary_call("ori_str_to_lowercase", ctx.arg_vals[0]),
    ("str", "replace", borrow: true) => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_replace(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "repeat", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_repeat(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "iter", borrow: true) => emitter.emit_str_iter(ctx.arg_vals[0]),
    // list
    ("list", "clone", borrow: true) => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("list", "length", borrow: true) => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "len", borrow: true) => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "is_empty", borrow: true) => emitter.emit_list_is_empty(ctx.arg_vals[0]),
    ("list", "concat", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_concat(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "add", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_concat(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "push", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_push_new(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "first", borrow: true) => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_first(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("list", "last", borrow: true) => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_last(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("list", "contains", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_contains(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "reverse", borrow: true) => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_reverse(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
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
    ("map", "is_empty", borrow: true) => emitter.emit_map_is_empty(ctx.arg_vals[0]),
    ("map", "contains_key", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_map_contains_key(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("map", "keys", borrow: true) => {
        if let TypeInfo::Map { key, .. } = ctx.type_info {
            emitter.emit_map_keys(ctx.arg_vals[0], *key)
        } else {
            None
        }
    },
    ("map", "values", borrow: true) => {
        if let TypeInfo::Map { value, .. } = ctx.type_info {
            emitter.emit_map_values(ctx.arg_vals[0], *value)
        } else {
            None
        }
    },
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
use crate::codegen::value_id::{LLVMTypeId, ValueId};

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

    /// Emit `map.is_empty()` — `len == 0`.
    pub(crate) fn emit_map_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "map.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "map.is_empty"))
    }

    /// Emit `map.contains_key(key)` — linear scan through string keys.
    ///
    /// Calls `ori_map_contains_key(keys_ptr, len, needle_ptr)`.
    /// Map layout: `{i64 len, i64 cap, ptr keys, ptr vals}`.
    pub(crate) fn emit_map_contains_key(
        &mut self,
        receiver: ValueId,
        key: ValueId,
    ) -> Option<ValueId> {
        let llvm_func = self
            .builder
            .scx()
            .llmod
            .get_function("ori_map_contains_key")?;
        let func_id = self.builder.intern_function(llvm_func);

        let keys_ptr = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let needle_ptr = self.str_to_ptr(key, "contains_key.needle");
        let result = self
            .builder
            .call(func_id, &[keys_ptr, len, needle_ptr], "contains_key")?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains_key.bool"))
    }

    /// Emit `map.keys()` — extract keys as a new list.
    ///
    /// Calls `ori_map_keys_to_list(keys_ptr, len, key_size, out_ptr)`.
    /// Returns `{i64 len, i64 cap, ptr data}` (list struct).
    pub(crate) fn emit_map_keys(&mut self, receiver: ValueId, key_ty: Idx) -> Option<ValueId> {
        let llvm_func = self
            .builder
            .scx()
            .llmod
            .get_function("ori_map_keys_to_list")?;
        let func_id = self.builder.intern_function(llvm_func);

        let keys_ptr = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let key_size = self.element_store_size(key_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "keys.out", list_ty);

        self.builder
            .call(func_id, &[keys_ptr, len, key_size_val, out_alloca], "keys");

        Some(self.builder.load(list_ty, out_alloca, "keys.val"))
    }

    /// Emit `map.values()` — extract values as a new list.
    ///
    /// Calls `ori_map_values_to_list(vals_ptr, len, val_size, out_ptr)`.
    /// Returns `{i64 len, i64 cap, ptr data}` (list struct).
    pub(crate) fn emit_map_values(&mut self, receiver: ValueId, val_ty: Idx) -> Option<ValueId> {
        let llvm_func = self
            .builder
            .scx()
            .llmod
            .get_function("ori_map_values_to_list")?;
        let func_id = self.builder.intern_function(llvm_func);

        let vals_ptr = self
            .builder
            .extract_value(receiver, 3, "map.vals")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let val_size = self.element_store_size(val_ty);
        let val_size_val = self.builder.const_i64(val_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "values.out", list_ty);

        self.builder.call(
            func_id,
            &[vals_ptr, len, val_size_val, out_alloca],
            "values",
        );

        Some(self.builder.load(list_ty, out_alloca, "values.val"))
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
        let elem_size = self.element_store_size(elem_ty);
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

        let key_size = self.element_store_size(key_ty);
        let val_size = self.element_store_size(val_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);

        self.builder.call(
            func_id,
            &[keys, vals, len, key_size_val, val_size_val],
            "map.iter",
        )
    }

    /// Alloca+store a string value and return the pointer.
    ///
    /// Runtime string methods take `*const OriStr`, but LLVM values are
    /// `{ i64, ptr }` aggregates. This helper allocates stack space, stores
    /// the aggregate, and returns the pointer for the runtime call.
    fn str_to_ptr(&mut self, val: ValueId, name: &str) -> ValueId {
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, str_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Emit a `(str, str) -> bool` runtime call (`contains`, `starts_with`, `ends_with`).
    pub(crate) fn emit_str_bool_call(
        &mut self,
        func_name: &str,
        receiver: ValueId,
        arg: ValueId,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function(func_name)?;
        let func_id = self.builder.intern_function(llvm_func);
        let lhs_ptr = self.str_to_ptr(receiver, "str_op.lhs");
        let rhs_ptr = self.str_to_ptr(arg, "str_op.rhs");
        self.builder.call(func_id, &[lhs_ptr, rhs_ptr], func_name)
    }

    /// Emit a `(str) -> str` runtime call (`trim`, `to_uppercase`, `to_lowercase`).
    pub(crate) fn emit_str_unary_call(
        &mut self,
        func_name: &str,
        receiver: ValueId,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function(func_name)?;
        let func_id = self.builder.intern_function(llvm_func);
        let ptr = self.str_to_ptr(receiver, "str_op.self");
        self.builder.call(func_id, &[ptr], func_name)
    }

    /// Emit `str.replace(from, to)` — `(str, str, str) -> str` runtime call.
    pub(crate) fn emit_str_replace(
        &mut self,
        receiver: ValueId,
        from: ValueId,
        to: ValueId,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_str_replace")?;
        let func_id = self.builder.intern_function(llvm_func);
        let s_ptr = self.str_to_ptr(receiver, "str_op.self");
        let from_ptr = self.str_to_ptr(from, "str_op.from");
        let to_ptr = self.str_to_ptr(to, "str_op.to");
        self.builder
            .call(func_id, &[s_ptr, from_ptr, to_ptr], "ori_str_replace")
    }

    /// Emit `str.repeat(count)` — `(str, i64) -> str` runtime call.
    pub(crate) fn emit_str_repeat(&mut self, receiver: ValueId, count: ValueId) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_str_repeat")?;
        let func_id = self.builder.intern_function(llvm_func);
        let s_ptr = self.str_to_ptr(receiver, "str_op.self");
        self.builder
            .call(func_id, &[s_ptr, count], "ori_str_repeat")
    }

    // -----------------------------------------------------------------------
    // List method helpers
    // -----------------------------------------------------------------------

    /// Build the LLVM struct type `{i64, i64, ptr}` for list sret returns.
    fn list_struct_type(&mut self) -> LLVMTypeId {
        self.builder.register_type(
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
        )
    }

    /// Extract list data pointer (field 2) and len (field 0) from receiver.
    fn extract_list_data_and_len(&mut self, receiver: ValueId) -> (ValueId, ValueId) {
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "list.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "list.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        (data_ptr, len)
    }

    /// Alloca+store an element value and return the pointer.
    ///
    /// List runtime methods take `*const u8` for elements. This helper
    /// allocates stack space for the element, stores the value, and
    /// returns the pointer.
    fn elem_to_ptr(&mut self, val: ValueId, elem_ty: Idx, name: &str) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, llvm_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Emit `list.concat(other)` / `list.add(other)` — concatenate two lists.
    ///
    /// Calls `ori_list_concat(data1, len1, data2, len2, elem_size, out_ptr)`.
    /// Returns a new `{i64, i64, ptr}` list struct.
    pub(crate) fn emit_list_concat(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_list_concat")?;
        let func_id = self.builder.intern_function(llvm_func);

        let (data1, len1) = self.extract_list_data_and_len(receiver);
        let (data2, len2) = self.extract_list_data_and_len(other);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "concat.out", list_ty);

        self.builder.call(
            func_id,
            &[data1, len1, data2, len2, elem_size_val, out_alloca],
            "concat",
        );

        Some(self.builder.load(list_ty, out_alloca, "concat.val"))
    }

    /// Emit `list.push(x)` — functional push returning a new list.
    ///
    /// Calls `ori_list_push_new(data, len, elem_ptr, elem_size, out_ptr)`.
    /// The result is a new `{i64, i64, ptr}` list struct.
    pub(crate) fn emit_list_push_new(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_list_push_new")?;
        let func_id = self.builder.intern_function(llvm_func);

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "push.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_ptr, elem_size_val, out_alloca],
            "push",
        );

        Some(self.builder.load(list_ty, out_alloca, "push.val"))
    }

    /// Emit `list.first()` — returns `Option<T>` as `{i64 tag, T value}`.
    ///
    /// Calls `ori_list_first(data, len, elem_size, out_ptr)`.
    pub(crate) fn emit_list_first(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, "ori_list_first", "first")
    }

    /// Emit `list.last()` — returns `Option<T>` as `{i64 tag, T value}`.
    ///
    /// Calls `ori_list_last(data, len, elem_size, out_ptr)`.
    pub(crate) fn emit_list_last(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, "ori_list_last", "last")
    }

    /// Shared implementation for `first()` and `last()`.
    fn emit_list_first_or_last(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        func_name: &str,
        label: &str,
    ) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function(func_name)?;
        let func_id = self.builder.intern_function(llvm_func);

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Option<T> layout: {i64 tag, T value}
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let raw_elem_ty = self.builder.raw_type(elem_llvm_ty);
        let option_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(&[self.builder.scx().type_i64().into(), raw_elem_ty], false)
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            &format!("{label}.out"),
            option_ty,
        );

        self.builder
            .call(func_id, &[data_ptr, len, elem_size_val, out_alloca], label);

        Some(
            self.builder
                .load(option_ty, out_alloca, &format!("{label}.val")),
        )
    }

    /// Emit `list.contains(x)` — returns `bool`.
    ///
    /// Dispatches to type-specific runtime functions:
    /// - `[int]` → `ori_list_contains_int(data, len, needle)`
    /// - `[str]` → `ori_list_contains_str(data, len, needle_ptr)`
    pub(crate) fn emit_list_contains(
        &mut self,
        receiver: ValueId,
        needle: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let (data_ptr, len) = self.extract_list_data_and_len(receiver);

        let elem_info = self.type_info.get(elem_ty);
        let (func_name, args): (&str, Vec<ValueId>) = match &elem_info {
            TypeInfo::Int => ("ori_list_contains_int", vec![data_ptr, len, needle]),
            TypeInfo::Str => {
                let needle_ptr = self.str_to_ptr(needle, "contains.needle");
                ("ori_list_contains_str", vec![data_ptr, len, needle_ptr])
            }
            _ => return None, // Other element types not yet supported
        };

        let llvm_func = self.builder.scx().llmod.get_function(func_name)?;
        let func_id = self.builder.intern_function(llvm_func);
        let result = self.builder.call(func_id, &args, "contains")?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains.bool"))
    }

    /// Emit `list.reverse()` — returns a new reversed list.
    ///
    /// Calls `ori_list_reverse(data, len, elem_size, out_ptr)`.
    pub(crate) fn emit_list_reverse(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let llvm_func = self.builder.scx().llmod.get_function("ori_list_reverse")?;
        let func_id = self.builder.intern_function(llvm_func);

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "reverse.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_size_val, out_alloca],
            "reverse",
        );

        Some(self.builder.load(list_ty, out_alloca, "reverse.val"))
    }
}
