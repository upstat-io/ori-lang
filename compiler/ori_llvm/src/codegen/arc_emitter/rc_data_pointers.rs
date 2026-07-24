//! RC-managed data-pointer extraction from aggregate values.

use ori_ir::{CLOSURE_FIELD_ENV, FIELD_DATA};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::ValueId;

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Extracts raw RC allocation pointers from a typed value.
    ///
    /// Collection and closure headers expose their owned pointer fields.
    /// Structs and tuples recurse through managed fields, while tagged unions
    /// require their tag-aware traversal.
    pub(super) fn extract_rc_data_ptrs(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            Tag::List | Tag::Set => {
                // INVARIANT: Slice retains use the capacity-aware aggregate path.
                if let Some(ptr) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Str => {
                // Why: SSO data fields hold inline bytes, so raw RC calls receive null.
                if let Some(ptr) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") {
                    let is_sso = self.emit_sso_check(ptr, "rc_str");
                    let null = self.builder.const_null_ptr();
                    let safe_ptr = self.builder.select(is_sso, null, ptr, "rc.str_safe_ptr");
                    vec![safe_ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Map => {
                if let Some(ptr) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Struct => self.extract_rc_from_struct_fields(val, resolved),
            Tag::Tuple => self.extract_rc_from_tuple_elems(val, resolved),
            Tag::Option => {
                // INVARIANT: Live `None` values use tag-aware traversal because
                // their payload field is uninitialized.
                let inner = self.pool.option_inner(resolved);
                if self.classifier.has_managed_ownership_obligation(inner) {
                    if let Some(field) = self.builder.extract_value(val, 1, "rc.opt_inner") {
                        if is_boxed_enum_field(self.pool, resolved, inner) {
                            return vec![field];
                        }
                        return self.extract_rc_data_ptrs(field, inner);
                    }
                }
                vec![]
            }
            Tag::Result => {
                // Why: The runtime tag selects the active payload's typed traversal.
                vec![]
            }
            Tag::Enum => {
                // Why: The runtime tag selects the active variant's typed traversal.
                vec![]
            }
            Tag::Function => {
                // Why: The closure code pointer carries no heap ownership.
                if let Some(env_ptr) =
                    self.builder
                        .extract_value(val, CLOSURE_FIELD_ENV, "rc.closure_env")
                {
                    vec![env_ptr]
                } else {
                    vec![]
                }
            }
            // INVARIANT: Range stores only scalar start, end, step, and inclusivity fields.
            Tag::Range => vec![],
            _ => vec![val],
        }
    }

    /// Extract RC data pointers from a struct's fields (remap to memory order).
    fn extract_rc_from_struct_fields(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let fields = self.pool.struct_fields(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "field count bounded by struct definition, well within u32 range"
        )]
        for (i, (_, field_ty)) in fields.into_iter().enumerate() {
            if self.classifier.has_managed_ownership_obligation(field_ty) {
                let mem_i = self.remap_struct_field(ty, i as u32);
                if let Some(field_val) =
                    self.builder
                        .extract_value(val, mem_i, &format!("rc.field.{i}"))
                {
                    if is_boxed_enum_field(self.pool, ty, field_ty) {
                        ptrs.push(field_val);
                    } else {
                        ptrs.extend(self.extract_rc_data_ptrs(field_val, field_ty));
                    }
                }
            }
        }
        ptrs
    }

    /// Extract RC data pointers from a tuple's elements (remap to memory order).
    fn extract_rc_from_tuple_elems(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let elems = self.pool.tuple_elems(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "element count bounded by tuple arity, well within u32 range"
        )]
        for (i, elem_ty) in elems.into_iter().enumerate() {
            if self.classifier.has_managed_ownership_obligation(elem_ty) {
                let mem_i = self.remap_struct_field(ty, i as u32);
                if let Some(elem_val) =
                    self.builder
                        .extract_value(val, mem_i, &format!("rc.elem.{i}"))
                {
                    if is_boxed_enum_field(self.pool, ty, elem_ty) {
                        ptrs.push(elem_val);
                    } else {
                        ptrs.extend(self.extract_rc_data_ptrs(elem_val, elem_ty));
                    }
                }
            }
        }
        ptrs
    }
}
