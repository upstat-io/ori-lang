//! Trait method codegen for compound types (Option, Result, Tuple, List).
//!
//! Implements `equals`, `compare`, and `hash` for compound types by
//! structural recursion into element types. Each method dispatches to
//! the element's trait implementation via `emit_element_*` helpers.
//!
//! ## ARC enum convention
//!
//! - **Option**: `{i64 tag, T payload}` — Some=0, None=1
//! - **Result**: `{i64 tag, payload}`   — Ok=0, Err=1
//! - **Tuple**:  `{A, B, ...}`         — flat struct of resolved element types
//! - **List**:   `{i64 len, i64 cap, ptr data}` — element-wise iteration

declare_builtins! { emitter, ctx;
    // list structural trait methods
    ("list", "equals") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "is_equal") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "compare") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_compare(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "hash") => emitter.emit_element_hash(ctx.arg_vals[0], ctx.receiver_ty),
    // Option structural trait methods
    ("Option", "equals") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_equals(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "is_equal") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_equals(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "compare") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_compare(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "hash") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            emitter.emit_option_hash(ctx.arg_vals[0], *inner)
        } else {
            None
        }
    },
    // Result structural trait methods
    ("Result", "equals") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "is_equal") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "compare") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_compare(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "hash") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            emitter.emit_result_hash(ctx.arg_vals[0], ctx.receiver_ty, *ok, *err)
        } else {
            None
        }
    },
    // tuple structural trait methods
    ("tuple", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("tuple", "equals") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_tuple_equals(ctx.arg_vals[0], ctx.arg_vals[1], elements, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("tuple", "compare") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_tuple_compare(ctx.arg_vals[0], ctx.arg_vals[1], elements, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("tuple", "hash") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            emitter.emit_tuple_hash(ctx.arg_vals[0], elements, ctx.receiver_ty)
        } else {
            None
        }
    },
}

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // Recursive element dispatch

    /// Emit `lhs.equals(rhs)` for any type, dispatching recursively.
    pub(crate) fn emit_element_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        match &type_info {
            TypeInfo::Int
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Ordering => Some(self.builder.icmp_eq(lhs, rhs, "elem_eq")),
            TypeInfo::Float => Some(self.builder.fcmp_oeq(lhs, rhs, "elem_eq")),
            TypeInfo::Str => Some(self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false)),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_equals(lhs, rhs, inner)
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_equals(lhs, rhs, elem_ty, ok, err)
            }
            TypeInfo::Tuple { elements } => self.emit_tuple_equals(lhs, rhs, elements, elem_ty),
            TypeInfo::List { element } => {
                let elem = *element;
                self.emit_list_equals(lhs, rhs, elem)
            }
            TypeInfo::Map { key, value } => {
                let key = *key;
                let value = *value;
                self.emit_map_equals(lhs, rhs, key, value)
            }
            TypeInfo::Struct { fields } => {
                let fields = fields.clone();
                self.emit_derived_eq_call(lhs, rhs, elem_ty)
                    .or_else(|| self.emit_structural_eq(lhs, rhs, &fields))
            }
            TypeInfo::Enum { variants } => {
                let variants = variants.clone();
                self.emit_derived_eq_call(lhs, rhs, elem_ty)
                    .or_else(|| self.emit_structural_eq_enum(lhs, rhs, &variants))
            }
            _ => None,
        }
    }

    /// Call a compiled derived `eq` method for a user-defined type.
    ///
    /// Looks up the type name and compiled `eq` function, applies ABI
    /// parameter passing (Indirect for large structs), and returns the bool.
    fn emit_derived_eq_call(&mut self, lhs: ValueId, rhs: ValueId, ty: Idx) -> Option<ValueId> {
        let type_name = *self.ctx.type_idx_to_name.get(&ty)?;
        let interned_eq = self.interner.intern("eq");
        let (func_id, params) = {
            let (fid, abi) = self.ctx.method_functions.get(&(type_name, interned_eq))?;
            (*fid, abi.params.clone())
        };
        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);
        self.emit_rt_call(func_id, &passed_args, "derived_eq")
    }

    /// Emit structural field-by-field equality for a struct without `#derive(Eq)`.
    ///
    /// Compares each field using `emit_element_equals` recursively, with
    /// short-circuit AND semantics (returns false on first mismatch).
    /// Falls back to integer comparison if field types are unknown.
    fn emit_structural_eq(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        fields: &[(ori_ir::Name, ori_types::Idx)],
    ) -> Option<ValueId> {
        if fields.is_empty() {
            return Some(self.builder.const_bool(true));
        }

        // Multi-field: accumulate AND of all field comparisons.
        // Branch-free AND chain — most structs have 2-5 fields.
        let mut result = None;
        for (i, &(_, field_ty)) in fields.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field index from type definition, always small"
            )]
            let idx = i as u32;
            let lhs_f = self.builder.extract_value(lhs, idx, &format!("seq.l.{i}"));
            let rhs_f = self.builder.extract_value(rhs, idx, &format!("seq.r.{i}"));
            let (Some(lhs_f), Some(rhs_f)) = (lhs_f, rhs_f) else {
                continue; // Skip fields that can't be extracted (void/unit)
            };
            let Some(field_eq) = self.emit_element_equals(lhs_f, rhs_f, field_ty) else {
                // Field type can't be compared (e.g., enum without #derive(Eq)).
                // Structural equality is not possible for this struct.
                return None;
            };
            result = Some(match result {
                None => field_eq,
                Some(acc) => self.builder.and(acc, field_eq, &format!("seq.and.{i}")),
            });
        }
        Some(result.unwrap_or_else(|| self.builder.const_bool(true)))
    }

    /// Emit structural equality for an enum without `#derive(Eq)`.
    ///
    /// For unit-only enums (all variants have zero fields), compares tags directly.
    /// For enums with any payload variants, returns None (payload comparison
    /// requires variant-specific field layout knowledge that needs `#derive(Eq)`).
    fn emit_structural_eq_enum(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        variants: &[crate::codegen::type_info::EnumVariantInfo],
    ) -> Option<ValueId> {
        // Unit-only enums: all variants have zero fields → tag comparison suffices
        let all_unit = variants.iter().all(|v| v.fields.is_empty());
        if !all_unit {
            return None;
        }
        let lhs_tag = self.builder.extract_value(lhs, 0, "eeq.ltag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "eeq.rtag")?;
        Some(self.builder.icmp_eq(lhs_tag, rhs_tag, "eeq.tags"))
    }

    /// Emit `lhs.compare(rhs)` for any type, dispatching recursively.
    ///
    /// Returns Ordering as i8 (Less=0, Equal=1, Greater=2).
    pub(crate) fn emit_element_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        match &type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                Some(self.builder.emit_icmp_ordering(lhs, rhs, "elem_cmp", true))
            }
            TypeInfo::Float => Some(self.builder.emit_fcmp_ordering(lhs, rhs, "elem_cmp")),
            TypeInfo::Bool | TypeInfo::Char | TypeInfo::Byte => {
                Some(self.builder.emit_icmp_ordering(lhs, rhs, "elem_cmp", false))
            }
            TypeInfo::Str => self.emit_str_compare_call(lhs, rhs),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_compare(lhs, rhs, inner)
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_compare(lhs, rhs, elem_ty, ok, err)
            }
            TypeInfo::Tuple { elements } => self.emit_tuple_compare(lhs, rhs, elements, elem_ty),
            TypeInfo::List { element } => {
                let elem = *element;
                self.emit_list_compare(lhs, rhs, elem)
            }
            TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
                self.emit_derived_compare_call(lhs, rhs, elem_ty)
            }
            _ => None,
        }
    }

    /// Call a compiled derived `compare` method for a user-defined type.
    fn emit_derived_compare_call(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        ty: Idx,
    ) -> Option<ValueId> {
        let type_name = *self.ctx.type_idx_to_name.get(&ty)?;
        let interned_compare = self.interner.intern("compare");
        let (func_id, params) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_compare))?;
            (*fid, abi.params.clone())
        };
        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);
        self.emit_rt_call(func_id, &passed_args, "derived_cmp")
    }

    /// Emit `val.hash()` for any type, dispatching recursively.
    pub(crate) fn emit_element_hash(&mut self, val: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        let i64_ty = self.builder.i64_type();
        match &type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => Some(val),
            TypeInfo::Float => {
                let arg_vals = [val];
                self.emit_trait_method("hash", &arg_vals, &type_info)
            }
            TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Ordering => {
                Some(self.builder.zext(val, i64_ty, "elem_hash"))
            }
            TypeInfo::Char => Some(self.builder.sext(val, i64_ty, "elem_hash")),
            TypeInfo::Str => self.emit_str_hash_call(val),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_hash(val, inner)
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_hash(val, elem_ty, ok, err)
            }
            TypeInfo::Tuple { elements } => self.emit_tuple_hash(val, elements, elem_ty),
            TypeInfo::List { element } => {
                let elem = *element;
                self.emit_list_hash(val, elem)
            }
            TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
                self.emit_derived_hash_call(val, elem_ty)
            }
            _ => None,
        }
    }

    /// Call a compiled derived `hash` method for a user-defined type.
    fn emit_derived_hash_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_name = *self.ctx.type_idx_to_name.get(&ty)?;
        let interned_hash = self.interner.intern("hash");
        let (func_id, params) = {
            let (fid, abi) = self.ctx.method_functions.get(&(type_name, interned_hash))?;
            (*fid, abi.params.clone())
        };
        let raw_args = [val];
        let passed_args = self.apply_param_passing(&raw_args, &params);
        self.emit_rt_call(func_id, &passed_args, "derived_hash")
    }
}
