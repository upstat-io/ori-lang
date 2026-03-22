//! String (Str) builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains`, `starts_with`, `ends_with`,
//! `trim`, `to_uppercase`, `to_lowercase`, `replace`, `repeat`, `chars`, `split`,
//! `iter`, and `to_str` for the `str` type.
//!
//! **SSO invariant**: `OriStr` is a 24-byte union — heap `{len, cap, data}` or SSO
//! `{[u8;23], flags}`. The two layouts share the same byte footprint but field
//! values differ. All property access (length, data pointer) MUST go through
//! runtime helpers (`ori_str_len`, `ori_str_data`) rather than extracting struct
//! fields directly, because field 0 means "len" for heap but "first 8 inline
//! bytes" for SSO.

use ori_arc::ir::ArcVarId;
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `str.length()` — call `ori_str_len(*const OriStr) -> i64`.
    ///
    /// SSO-safe: the runtime helper dispatches on the SSO flag byte.
    pub(crate) fn emit_str_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_len");
        let ptr = self.str_to_ptr(receiver, "str_len.self");
        self.emit_rt_call(func_id, &[ptr], "str.len")
    }

    /// Emit `str.length()` with borrowed parameter forwarding.
    ///
    /// When the receiver is a borrowed parameter, forwards its pointer directly
    /// to `ori_str_len` instead of creating an alloca+store round-trip.
    pub(crate) fn emit_str_length_forwarded(
        &mut self,
        receiver: ValueId,
        var: ArcVarId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_len");
        let ptr = self.str_to_ptr_forwarded(receiver, var, "str_len.self");
        self.emit_rt_call(func_id, &[ptr], "str.len")
    }

    /// Emit `str.is_empty()` — `ori_str_len(s) == 0`.
    pub(crate) fn emit_str_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.emit_str_length(receiver)?;
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
        self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], func_name)
    }

    /// Emit a `(str) -> str` runtime call (`trim`, `to_uppercase`, `to_lowercase`).
    pub(crate) fn emit_str_unary_call(
        &mut self,
        func_name: &'static str,
        receiver: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);
        let ptr = self.str_to_ptr(receiver, "str_op.self");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[ptr], str_ty, func_name)
    }

    /// Emit `str.substring(start, end)` — `(str, i64, i64) -> str` runtime call.
    ///
    /// Creates a zero-copy seamless slice for heap strings (RC inc on original).
    /// SSO strings are copied (no shareable heap buffer).
    pub(crate) fn emit_str_substring(
        &mut self,
        receiver: ValueId,
        start: ValueId,
        end: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_substring");
        let ptr = self.str_to_ptr(receiver, "substring.self");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[ptr, start, end], str_ty, "ori_str_substring")
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
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder.call_with_sret(
            func_id,
            &[s_ptr, from_ptr, to_ptr],
            str_ty,
            "ori_str_replace",
        )
    }

    /// Emit `str.repeat(count)` — `(str, i64) -> str` runtime call.
    pub(crate) fn emit_str_repeat(&mut self, receiver: ValueId, count: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_repeat");
        let s_ptr = self.str_to_ptr(receiver, "str_op.self");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[s_ptr, count], str_ty, "ori_str_repeat")
    }

    /// Emit `str.chars()` — returns `[char]` (list of i32 code points).
    ///
    /// Calls `ori_str_chars(data_ptr, len, out_ptr)`. SSO-safe: extracts the
    /// correct data pointer and length via `ori_str_data`/`ori_str_len`.
    pub(crate) fn emit_str_chars(&mut self, receiver: ValueId) -> Option<ValueId> {
        let chars_fn = self.builder.runtime_fn("ori_str_chars");
        let str_ptr = self.str_to_ptr(receiver, "chars.self");

        let data_fn = self.builder.runtime_fn("ori_str_data");
        let data_ptr = self
            .builder
            .call(data_fn, &[str_ptr], "chars.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        let len_fn = self.builder.runtime_fn("ori_str_len");
        let len = self
            .builder
            .call(len_fn, &[str_ptr], "chars.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "chars.out", list_ty);

        self.builder
            .call(chars_fn, &[data_ptr, len, out_alloca], "chars");

        Some(self.builder.load(list_ty, out_alloca, "chars.val"))
    }

    /// Emit `str.split(sep)` — returns `[str]` (list of strings).
    ///
    /// Calls `ori_str_split(data_ptr, len, cap, sep_data, sep_len, elem_dec_fn, out_ptr)`.
    /// SSO-safe: extracts data/len via runtime helpers. Passes cap for slice awareness
    /// so that splitting a substring/trim result doesn't crash on misaligned RC access.
    pub(crate) fn emit_str_split(
        &mut self,
        receiver: ValueId,
        separator: ValueId,
        str_ty: Idx,
    ) -> Option<ValueId> {
        let split_fn = self.builder.runtime_fn("ori_str_split");
        let data_fn = self.builder.runtime_fn("ori_str_data");
        let len_fn = self.builder.runtime_fn("ori_str_len");

        // Self string — extract data, len, and cap
        let self_ptr = self.str_to_ptr(receiver, "split.self");
        let data_ptr = self
            .emit_rt_call(data_fn, &[self_ptr], "split.self.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let str_len = self
            .emit_rt_call(len_fn, &[self_ptr], "split.self.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        // Cap field (field 1) — needed for slice detection in the runtime.
        // For SSO strings cap is meaningless (str_len <= 23 → no slicing).
        // For heap strings cap >= 0. For slices cap has SLICE_FLAG.
        let str_cap = self
            .builder
            .extract_value(receiver, 1, "split.self.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Separator string
        let sep_ptr = self.str_to_ptr(separator, "split.sep");
        let sep_data = self
            .emit_rt_call(data_fn, &[sep_ptr], "split.sep.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let sep_len = self
            .emit_rt_call(len_fn, &[sep_ptr], "split.sep.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // elem_dec_fn for [str] element cleanup
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(str_ty);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "split.out", list_ty);

        self.emit_rt_call(
            split_fn,
            &[
                data_ptr,
                str_len,
                str_cap,
                sep_data,
                sep_len,
                elem_dec_fn,
                out_alloca,
            ],
            "split",
        );

        Some(self.builder.load(list_ty, out_alloca, "split.val"))
    }

    /// Emit `str.iter()` — call `ori_iter_from_str(*const OriStr) -> ptr`.
    ///
    /// SSO-safe: passes the full `OriStr` by pointer. The runtime handles SSO
    /// (copies inline bytes to heap) vs heap (takes RC reference).
    pub(crate) fn emit_str_iter(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_str");
        let str_ptr = self.str_to_ptr(receiver, "str_iter.self");
        self.emit_rt_call(func_id, &[str_ptr], "str.iter")
    }
}
