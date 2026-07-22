//! Leaf `debug`/`to_str` LLVM emission: per-element formatting, the element
//! dispatchers, derived-method calls, and string literal/concat utilities.

use super::{super::ArcIrEmitter, RenderStyle};
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;
use ori_types::Idx;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Materialize a runtime-owned `OriStr` from a compiler literal.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_literal_ori_str(&mut self, text: &str) -> Option<ValueId> {
        let ptr = self.builder.build_global_string_ptr(text, "lit.str");
        let len = self.builder.const_i64(text.len() as i64);
        let func_id = self.builder.runtime_fn("ori_str_from_raw");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[ptr, len], str_ty, "lit")
    }

    /// Render an `Ordering` tag through the shared runtime name mapping.
    ///
    /// Printable and Debug use the same names; the tag is sign-extended to
    /// match the C ABI.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_ordering_name(&mut self, val: ValueId) -> Option<ValueId> {
        let i64_ty = self.builder.i64_type();
        let tag_i64 = self.builder.sext(val, i64_ty, "ord.sext");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let func_id = self.builder.runtime_fn("ori_str_from_ordering");
        self.builder
            .call_with_sret(func_id, &[tag_i64], str_ty, "ord.name")
    }

    /// Concatenate two `OriStr` values via `ori_str_concat`.
    #[must_use = "the absence of a value must be handled"]
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
    /// Extracts `{len, cap, data}` fields and calls `ori_str_rc_dec` with the
    /// plain-buffer drop function. SSO strings are runtime no-ops; a null drop
    /// function would decrement a heap intermediate to zero without freeing it.
    pub(super) fn dec_intermediate_str(&mut self, str_val: ValueId) {
        let data = self.builder.extract_value(str_val, 2, "dbg.dec.data");
        let cap = self.builder.extract_value(str_val, 1, "dbg.dec.cap");
        if let (Some(dp), Some(cp)) = (data, cap) {
            let drop_fn = self.builder.runtime_fn("ori_str_drop_buffer");
            let drop_fn_ptr = self.builder.get_function_ptr(drop_fn);
            self.call_str_rc_dec(dp, cp, drop_fn_ptr);
        }
    }

    /// Escape control characters for map-key formatting without adding quotes.
    #[must_use = "the absence of a value must be handled"]
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
    /// Strings and chars render without quoting; compound types recurse with
    /// [`RenderStyle::Printable`]. Unsupported types render as `<?>`.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_element_to_str(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);
        match &type_info {
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Float
            | TypeInfo::Bool
            | TypeInfo::Char => self.emit_to_str(val, &type_info),

            TypeInfo::Ordering => self.emit_ordering_name(val),

            TypeInfo::Str => Some(val),

            TypeInfo::Byte => {
                let i64_ty = self
                    .builder
                    .register_type(self.builder.scx().type_i64().into());
                let as_i64 = self.builder.zext(val, i64_ty, "tstr.byte.zext");
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_byte_printable_format");
                self.builder
                    .call_with_sret(func_id, &[as_i64], str_ty, "tstr.byte")
            }

            TypeInfo::Option { inner } => {
                let inner = *inner;
                let tag = self.builder.extract_value(val, 0, "tstr.opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "tstr.opt.is_some");
                let payload = self.builder.extract_value(val, 1, "tstr.opt.payload")?;
                self.emit_option_debug_branch(is_some, payload, inner, RenderStyle::Printable)
            }

            TypeInfo::Result {
                ok: ok_ty,
                err: err_ty,
            } => {
                let ok_ty = *ok_ty;
                let err_ty = *err_ty;
                self.emit_nested_result_render(val, ty, ok_ty, err_ty, RenderStyle::Printable)
            }

            TypeInfo::List { element } => {
                let element = *element;
                self.emit_list_debug(val, ty, element, RenderStyle::Printable)
            }

            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_debug(val, &elements, ty, RenderStyle::Printable)
            }

            TypeInfo::Map { key, value } => {
                let key = *key;
                let value = *value;
                self.emit_map_debug(val, ty, key, value, RenderStyle::Printable)
            }

            TypeInfo::Set { element } => {
                let element = *element;
                self.emit_set_debug(val, element, RenderStyle::Printable)
            }

            _ => self
                .emit_derived_to_str_call(val, ty)
                .or_else(|| self.emit_literal_ori_str("<?>")),
        }
    }

    /// Emit Debug formatting for an element of any type.
    ///
    /// Strings and chars render quoted and escaped; compound types recurse with
    /// [`RenderStyle::Debug`]. Unsupported types render as `<?>`.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_element_debug(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);
        match &type_info {
            TypeInfo::Unit => self.emit_literal_ori_str("()"),
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Float
            | TypeInfo::Bool => self.emit_to_str(val, &type_info),

            TypeInfo::Ordering => self.emit_ordering_name(val),

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

            TypeInfo::Char => {
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_char_debug_format");
                self.builder
                    .call_with_sret(func_id, &[val], str_ty, "dbg.char.fmt")
            }

            TypeInfo::Byte => {
                let i64_ty = self
                    .builder
                    .register_type(self.builder.scx().type_i64().into());
                let as_i64 = self.builder.zext(val, i64_ty, "dbg.byte.zext");
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_str_from_int");
                self.builder
                    .call_with_sret(func_id, &[as_i64], str_ty, "dbg.byte")
            }

            TypeInfo::Option { inner } => {
                let inner = *inner;
                let tag = self.builder.extract_value(val, 0, "dbg.opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "dbg.opt.is_some");
                let payload = self.builder.extract_value(val, 1, "dbg.opt.payload")?;
                self.emit_option_debug_branch(is_some, payload, inner, RenderStyle::Debug)
            }

            TypeInfo::Result {
                ok: ok_ty,
                err: err_ty,
            } => {
                let ok_ty = *ok_ty;
                let err_ty = *err_ty;
                self.emit_nested_result_render(val, ty, ok_ty, err_ty, RenderStyle::Debug)
            }

            TypeInfo::List { element } => {
                let element = *element;
                self.emit_list_debug(val, ty, element, RenderStyle::Debug)
            }

            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_debug(val, &elements, ty, RenderStyle::Debug)
            }

            TypeInfo::Map { key, value } => {
                let key = *key;
                let value = *value;
                self.emit_map_debug(val, ty, key, value, RenderStyle::Debug)
            }

            TypeInfo::Set { element } => {
                let element = *element;
                self.emit_set_debug(val, element, RenderStyle::Debug)
            }

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
        // Why: The concrete monomorphized type selects the layout-correct fields.
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
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_derived_debug_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        self.emit_derived_str_method_call(val, ty, "debug", "dbg.derived")
    }

    /// Call a compiled `.to_str()` (Printable) method for a type via method
    /// dispatch. Printable analog of [`emit_derived_debug_call`].
    ///
    /// Returns `None` if no compiled `to_str` method exists for the type.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_derived_to_str_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        self.emit_derived_str_method_call(val, ty, "to_str", "tstr.derived")
    }
}
