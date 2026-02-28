//! String (Str) builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains`, `starts_with`, `ends_with`,
//! `trim`, `to_uppercase`, `to_lowercase`, `replace`, `repeat`, `chars`, `split`,
//! `iter`, and `to_str` for the `str` type.

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

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

    /// Emit a `(str, str) -> bool` runtime call (`contains`, `starts_with`, `ends_with`).
    pub(crate) fn emit_str_bool_call(
        &mut self,
        func_name: &'static str,
        receiver: ValueId,
        arg: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);
        let lhs_ptr = self.str_to_ptr(receiver, "str_op.lhs");
        let rhs_ptr = self.str_to_ptr(arg, "str_op.rhs");
        self.builder.call(func_id, &[lhs_ptr, rhs_ptr], func_name)
    }

    /// Emit a `(str) -> str` runtime call (`trim`, `to_uppercase`, `to_lowercase`).
    pub(crate) fn emit_str_unary_call(
        &mut self,
        func_name: &'static str,
        receiver: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);
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
        let func_id = self.builder.runtime_fn("ori_str_replace");
        let s_ptr = self.str_to_ptr(receiver, "str_op.self");
        let from_ptr = self.str_to_ptr(from, "str_op.from");
        let to_ptr = self.str_to_ptr(to, "str_op.to");
        self.builder
            .call(func_id, &[s_ptr, from_ptr, to_ptr], "ori_str_replace")
    }

    /// Emit `str.repeat(count)` — `(str, i64) -> str` runtime call.
    pub(crate) fn emit_str_repeat(&mut self, receiver: ValueId, count: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_repeat");
        let s_ptr = self.str_to_ptr(receiver, "str_op.self");
        self.builder
            .call(func_id, &[s_ptr, count], "ori_str_repeat")
    }

    /// Emit `str.chars()` — returns `[char]` (list of i32 code points).
    ///
    /// Calls `ori_str_chars(data_ptr, len, out_ptr)`.
    pub(crate) fn emit_str_chars(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_chars");

        let data_ptr = self
            .builder
            .extract_value(receiver, 1, "chars.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "chars.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "chars.out", list_ty);

        self.builder
            .call(func_id, &[data_ptr, len, out_alloca], "chars");

        Some(self.builder.load(list_ty, out_alloca, "chars.val"))
    }

    /// Emit `str.split(sep)` — returns `[str]` (list of strings).
    ///
    /// Calls `ori_str_split(data_ptr, len, sep_data, sep_len, out_ptr)`.
    pub(crate) fn emit_str_split(
        &mut self,
        receiver: ValueId,
        separator: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_split");

        let data_ptr = self
            .builder
            .extract_value(receiver, 1, "split.self.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let str_len = self
            .builder
            .extract_value(receiver, 0, "split.self.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let sep_data = self
            .builder
            .extract_value(separator, 1, "split.sep.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let sep_len = self
            .builder
            .extract_value(separator, 0, "split.sep.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "split.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, str_len, sep_data, sep_len, out_alloca],
            "split",
        );

        Some(self.builder.load(list_ty, out_alloca, "split.val"))
    }

    /// Emit `str.iter()` — call `ori_iter_from_str(data, len, owns_data)`.
    ///
    /// Str layout: `{i64 len, ptr data}`. Yields `char` (i32) values.
    /// The iterator takes ownership of one RC reference to the string data
    /// and releases it via `ori_rc_dec` when dropped.
    pub(crate) fn emit_str_iter(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_str");

        let data_ptr = self
            .builder
            .extract_value(receiver, 1, "str.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "str.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let owns_data = self.builder.const_bool(true);

        self.builder
            .call(func_id, &[data_ptr, len, owns_data], "str.iter")
    }
}
