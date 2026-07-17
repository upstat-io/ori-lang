//! Leaf `debug`/`to_str` LLVM emission: per-element formatting, the element
//! dispatchers, derived-method calls, and string literal/concat utilities.
//!
//! Compound-aggregate renderers (Option/Result/List/Tuple branch and loop
//! emission) live in the sibling `debug_compound` module.

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Create an `OriStr` from a string literal via `ori_str_from_raw`.
    pub(super) fn emit_literal_ori_str(&mut self, text: &str) -> Option<ValueId> {
        let ptr = self.builder.build_global_string_ptr(text, "lit.str");
        let len = self.builder.const_i64(text.len() as i64);
        let func_id = self.builder.runtime_fn("ori_str_from_raw");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[ptr, len], str_ty, "lit")
    }

    /// Render an `Ordering` value (i8 tag) to its variant name string.
    ///
    /// Routes through the runtime `ori_str_from_ordering` SSOT (the single
    /// `Less=0`/`Equal=1`/`Greater=2` -> name home, shared with the derived
    /// `FormatFields` field path); Printable and Debug are identical for
    /// `Ordering`. The tag is sign-extended i8 -> i64 to match the C ABI.
    pub(super) fn emit_ordering_name(&mut self, val: ValueId) -> Option<ValueId> {
        let i64_ty = self.builder.i64_type();
        let tag_i64 = self.builder.sext(val, i64_ty, "ord.sext");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let func_id = self.builder.runtime_fn("ori_str_from_ordering");
        self.builder
            .call_with_sret(func_id, &[tag_i64], str_ty, "ord.name")
    }

    /// Concatenate two `OriStr` values via `ori_str_concat`.
    pub(super) fn emit_str_concat(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let a_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "cat.a", str_ty);
        self.builder.store(a, a_ptr);
        let b_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "cat.b", str_ty);
        self.builder.store(b, b_ptr);
        let func_id = self.builder.runtime_fn("ori_str_concat");
        self.builder
            .call_with_sret(func_id, &[a_ptr, b_ptr], str_ty, "cat")
    }

    /// Dec an intermediate `OriStr` value to prevent leaks in debug loops.
    ///
    /// Extracts `{len, cap, data}` fields and calls `ori_str_rc_dec`.
    /// SSO strings (cap encodes SSO flag) are no-ops in the runtime.
    pub(super) fn dec_intermediate_str(&mut self, str_val: ValueId) {
        let data = self.builder.extract_value(str_val, 2, "dbg.dec.data");
        let cap = self.builder.extract_value(str_val, 1, "dbg.dec.cap");
        if let (Some(dp), Some(cp)) = (data, cap) {
            let null_fn = self.builder.const_null_ptr();
            self.call_str_rc_dec(dp, cp, null_fn);
        }
    }

    /// Escape control characters in a string without adding quotes.
    ///
    /// Used for map key formatting in `Map.debug()` — matches the
    /// interpreter's `escape_debug_str` behavior (control-char escaping,
    /// no surrounding quotes).
    pub(super) fn emit_escape_control(&mut self, s: ValueId) -> Option<ValueId> {
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let s_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "esc.ptr", str_ty);
        self.builder.store(s, s_ptr);
        let func_id = self.builder.runtime_fn("ori_str_escape_control");
        self.builder
            .call_with_sret(func_id, &[s_ptr], str_ty, "esc.ctrl")
    }

    /// Emit `to_str` (Printable) for an element of any supported type.
    ///
    /// Mirrors [`emit_element_debug`](Self::emit_element_debug) exactly, but
    /// applies Printable (not Debug) leaf semantics: strings are returned raw
    /// (not quoted/escaped), chars render as the raw codepoint (not single-
    /// quoted), and compound types recurse with `is_debug = false`.
    ///
    /// Returns a placeholder for truly unsupported types (closures) after
    /// failing the derived-`to_str` lookup.
    pub(super) fn emit_element_to_str(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);
        match &type_info {
            // Primitives: Printable == raw to_str (char renders raw, not quoted)
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Float
            | TypeInfo::Bool
            | TypeInfo::Char => self.emit_to_str(val, &type_info),

            // Ordering: render the variant name (Less/Equal/Greater).
            TypeInfo::Ordering => self.emit_ordering_name(val),

            // Str: Printable returns the raw string (no quotes, no escaping)
            TypeInfo::Str => Some(val),

            // Byte Printable is the language's two-digit hexadecimal form.
            TypeInfo::Byte => {
                let i64_ty = self
                    .builder
                    .register_type(self.builder.scx().type_i64().into());
                let as_i64 = self.builder.sext(val, i64_ty, "tstr.byte.sext");
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_byte_debug_format");
                self.builder
                    .call_with_sret(func_id, &[as_i64], str_ty, "tstr.byte")
            }

            // Option: recursive Printable
            TypeInfo::Option { inner } => {
                let inner = *inner;
                let tag = self.builder.extract_value(val, 0, "tstr.opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "tstr.opt.is_some");
                let payload = self.builder.extract_value(val, 1, "tstr.opt.payload")?;
                self.emit_option_debug_branch(is_some, payload, inner, false)
            }

            // Result: recursive Printable
            TypeInfo::Result {
                ok: ok_ty,
                err: err_ty,
            } => {
                let ok_ty = *ok_ty;
                let err_ty = *err_ty;
                self.emit_nested_result_render(val, ty, ok_ty, err_ty, false)
            }

            // List: element-wise Printable loop
            TypeInfo::List { element } => {
                let element = *element;
                self.emit_list_debug(val, element, false)
            }

            // Tuple: field-wise Printable
            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_debug(val, &elements, false)
            }

            // Map: entry-wise Printable as `{key: value, ...}`
            TypeInfo::Map { key, value } => {
                let key = *key;
                let value = *value;
                self.emit_map_debug(val, ty, key, value, false)
            }

            // Set: element-wise Printable as `Set {elem, ...}`
            TypeInfo::Set { element } => {
                let element = *element;
                self.emit_set_debug(val, element, false)
            }

            // Generic dispatch: look up the type's compiled .to_str() method
            // (user structs/enums with #derive(Printable)). Falls back to a
            // placeholder for types with no compiled to_str.
            _ => self
                .emit_derived_to_str_call(val, ty)
                .or_else(|| self.emit_literal_ori_str("<?>")),
        }
    }

    /// Emit Debug formatting for an element of any type.
    ///
    /// Unlike `emit_element_to_str` (Printable semantics), this applies
    /// Debug semantics: strings are quoted+escaped, chars are single-quoted,
    /// and compound types (Option, Result, List) are recursively formatted.
    ///
    /// Falls back to `emit_to_str` for types where Debug == Printable
    /// (int, float, bool, Duration, Size), and returns a placeholder for
    /// truly unsupported types (structs, maps, sets, closures).
    pub(super) fn emit_element_debug(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);
        match &type_info {
            TypeInfo::Unit => self.emit_literal_ori_str("()"),
            // Primitives: Debug == Printable
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Float
            | TypeInfo::Bool => self.emit_to_str(val, &type_info),

            // Ordering: Debug == Printable — render the variant name.
            TypeInfo::Ordering => self.emit_ordering_name(val),

            // Str: Debug wraps with quotes and escapes special chars
            TypeInfo::Str => {
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let s_ptr =
                    self.builder
                        .create_entry_alloca(self.current_function, "dbg.str", str_ty);
                self.builder.store(val, s_ptr);
                let func_id = self.builder.runtime_fn("ori_str_debug_format");
                self.builder
                    .call_with_sret(func_id, &[s_ptr], str_ty, "dbg.str.fmt")
            }

            // Char: Debug wraps with single quotes and escapes
            TypeInfo::Char => {
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_char_debug_format");
                self.builder
                    .call_with_sret(func_id, &[val], str_ty, "dbg.char.fmt")
            }

            // Byte Debug is the same two-digit hexadecimal form as Printable.
            TypeInfo::Byte => {
                let i64_ty = self
                    .builder
                    .register_type(self.builder.scx().type_i64().into());
                let as_i64 = self.builder.sext(val, i64_ty, "dbg.byte.sext");
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_byte_debug_format");
                self.builder
                    .call_with_sret(func_id, &[as_i64], str_ty, "dbg.byte")
            }

            // Option: recursive Debug
            TypeInfo::Option { inner } => {
                let inner = *inner;
                let tag = self.builder.extract_value(val, 0, "dbg.opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "dbg.opt.is_some");
                let payload = self.builder.extract_value(val, 1, "dbg.opt.payload")?;
                self.emit_option_debug_branch(is_some, payload, inner, true)
            }

            // Result: recursive Debug
            TypeInfo::Result {
                ok: ok_ty,
                err: err_ty,
            } => {
                let ok_ty = *ok_ty;
                let err_ty = *err_ty;
                self.emit_nested_result_render(val, ty, ok_ty, err_ty, true)
            }

            // List: element-wise Debug loop
            TypeInfo::List { element } => {
                let element = *element;
                self.emit_list_debug(val, element, true)
            }

            // Tuple: field-wise Debug
            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_debug(val, &elements, true)
            }

            // Map: entry-wise Debug as `{key: value, ...}`
            TypeInfo::Map { key, value } => {
                let key = *key;
                let value = *value;
                self.emit_map_debug(val, ty, key, value, true)
            }

            // Set: element-wise Debug as `Set {elem, ...}`
            TypeInfo::Set { element } => {
                let element = *element;
                self.emit_set_debug(val, element, true)
            }

            // Generic dispatch: look up the type's compiled .debug() method.
            // Handles user structs/enums with #derive(Debug), and any other
            // type whose debug method was compiled and registered.
            _ => self
                .emit_derived_debug_call(val, ty)
                .or_else(|| self.emit_literal_ori_str("<?>")),
        }
    }

    /// Call a compiled `str`-returning derived method (`debug` / `to_str`)
    /// for a type via method dispatch.
    ///
    /// Looks up `method_name` in `method_functions`, applies ABI parameter
    /// passing (Indirect for large structs), and handles the sret return
    /// (both `debug` and `to_str` return `str`, which is 24 bytes -> sret).
    /// Returns `None` if no compiled method exists for the type.
    fn emit_derived_str_method_call(
        &mut self,
        val: ValueId,
        ty: Idx,
        method_name: &str,
        label: &str,
    ) -> Option<ValueId> {
        // Mono-first (per-instantiation concrete Idx) so a multi-instantiation
        // generic composite formats the layout-correct fields.
        let interned_method = self.interner.intern(method_name);
        let (func_id, abi) = self.derived_method_full(ty, interned_method)?;

        let raw_args = [val];
        let passed_args = self.apply_param_passing(&raw_args, None, &abi.params);

        match &abi.return_abi.passing {
            crate::codegen::abi::ReturnPassing::Sret { .. } => {
                let str_ty = self.resolve_type(Idx::STR);
                self.call_with_sret(func_id, &passed_args, str_ty, label)
            }
            _ => self.emit_rt_call(func_id, &passed_args, label),
        }
    }

    /// Call a compiled `.debug()` method for a type via method dispatch.
    ///
    /// Returns `None` if no compiled debug method exists for the type.
    pub(super) fn emit_derived_debug_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        self.emit_derived_str_method_call(val, ty, "debug", "dbg.derived")
    }

    /// Call a compiled `.to_str()` (Printable) method for a type via method
    /// dispatch. Printable analog of [`emit_derived_debug_call`].
    ///
    /// Returns `None` if no compiled `to_str` method exists for the type.
    pub(super) fn emit_derived_to_str_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        self.emit_derived_str_method_call(val, ty, "to_str", "tstr.derived")
    }
}
