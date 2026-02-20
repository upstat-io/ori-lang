//! Type-agnostic inner operations: equality, comparison, and hashing.
//!
//! These functions dispatch on `TypeInfo` to emit the correct operation for
//! any inner type, enabling recursive structural operations on Option, Result,
//! Tuple, List, Map, and Set.

use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Emit equality comparison for an inner value, dispatching on `TypeInfo`.
    ///
    /// Every `TypeInfo` variant has an explicit arm — no catch-all — so that
    /// adding a new variant produces a compile error here, forcing the
    /// implementer to decide the correct equality semantics.
    pub(crate) fn emit_inner_eq(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_type: Idx,
        name: &str,
    ) -> ValueId {
        match self.type_info.get(inner_type) {
            // Integer-representable primitives and pointer-identity types
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Ordering
            | TypeInfo::Iterator { .. }
            | TypeInfo::Channel { .. } => self.builder.icmp_eq(lhs, rhs, name),

            TypeInfo::Float => self.builder.fcmp_oeq(lhs, rhs, name),
            TypeInfo::Str => self.emit_str_eq_call(lhs, rhs, name),

            TypeInfo::Option { inner } => self
                .emit_option_equals(lhs, rhs, inner)
                .unwrap_or_else(|| self.builder.const_bool(false)),
            TypeInfo::Result { ok, err } => self
                .emit_result_equals(lhs, rhs, ok, err)
                .unwrap_or_else(|| self.builder.const_bool(false)),
            TypeInfo::Tuple { elements } => self
                .emit_tuple_equals(lhs, rhs, &elements)
                .unwrap_or_else(|| self.builder.const_bool(false)),
            TypeInfo::List { element } => self
                .emit_list_equals(lhs, rhs, element)
                .unwrap_or_else(|| self.builder.const_bool(false)),
            TypeInfo::Map { key, value } => self
                .emit_map_equals(lhs, rhs, key, value)
                .unwrap_or_else(|| self.builder.const_bool(false)),
            TypeInfo::Set { element } => self
                .emit_set_equals(lhs, rhs, element)
                .unwrap_or_else(|| self.builder.const_bool(false)),

            // User-defined struct: delegate to derived/user eq, bitwise fallback
            TypeInfo::Struct { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let eq_name = self.prop_names.eq;
                    if let Some((func_id, _abi)) = self.method_functions.get(&(type_name, eq_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[lhs, rhs], name)
                            .unwrap_or_else(|| self.builder.const_bool(false));
                    }
                }
                self.builder.icmp_eq(lhs, rhs, name)
            }
            // User-defined enum: delegate to derived/user eq, no bitwise fallback
            TypeInfo::Enum { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let eq_name = self.prop_names.eq;
                    if let Some((func_id, _abi)) = self.method_functions.get(&(type_name, eq_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[lhs, rhs], name)
                            .unwrap_or_else(|| self.builder.const_bool(false));
                    }
                }
                self.builder.const_bool(false)
            }

            // Range: compare all 3 fields (start, end, inclusive)
            TypeInfo::Range => {
                let s_a = self.builder.extract_value(lhs, 0, &format!("{name}.s.a"));
                let s_b = self.builder.extract_value(rhs, 0, &format!("{name}.s.b"));
                let e_a = self.builder.extract_value(lhs, 1, &format!("{name}.e.a"));
                let e_b = self.builder.extract_value(rhs, 1, &format!("{name}.e.b"));
                let i_a = self.builder.extract_value(lhs, 2, &format!("{name}.i.a"));
                let i_b = self.builder.extract_value(rhs, 2, &format!("{name}.i.b"));
                match (s_a, s_b, e_a, e_b, i_a, i_b) {
                    (Some(sa), Some(sb), Some(ea), Some(eb), Some(ia), Some(ib)) => {
                        let s_eq = self.builder.icmp_eq(sa, sb, &format!("{name}.s"));
                        let e_eq = self.builder.icmp_eq(ea, eb, &format!("{name}.e"));
                        let i_eq = self.builder.icmp_eq(ia, ib, &format!("{name}.i"));
                        let se = self.builder.and(s_eq, e_eq, &format!("{name}.se"));
                        self.builder.and(se, i_eq, name)
                    }
                    _ => self.builder.const_bool(false),
                }
            }

            // Function: compare fn_ptr and env_ptr fields
            TypeInfo::Function { .. } => {
                let fn_a = self.builder.extract_value(lhs, 0, &format!("{name}.fn.a"));
                let fn_b = self.builder.extract_value(rhs, 0, &format!("{name}.fn.b"));
                let env_a = self.builder.extract_value(lhs, 1, &format!("{name}.env.a"));
                let env_b = self.builder.extract_value(rhs, 1, &format!("{name}.env.b"));
                match (fn_a, fn_b, env_a, env_b) {
                    (Some(fa), Some(fb), Some(ea), Some(eb)) => {
                        let fn_eq = self.builder.icmp_eq(fa, fb, &format!("{name}.fn"));
                        let env_eq = self.builder.icmp_eq(ea, eb, &format!("{name}.env"));
                        self.builder.and(fn_eq, env_eq, name)
                    }
                    _ => self.builder.const_bool(false),
                }
            }

            // Unit: always equal (only one value)
            TypeInfo::Unit => self.builder.const_bool(true),

            // Never/Error: unreachable in well-typed code
            TypeInfo::Never | TypeInfo::Error => self.builder.const_bool(false),
        }
    }

    /// Emit three-way comparison for an inner value, returning Ordering (i8).
    ///
    /// Every `TypeInfo` variant has an explicit arm. Types that don't support
    /// comparison (Map, Set, Channel, Function) return `Equal` — these should
    /// not reach this path if the type checker is correct.
    pub(crate) fn emit_inner_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_type: Idx,
        name: &str,
    ) -> ValueId {
        match self.type_info.get(inner_type) {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                self.emit_icmp_ordering(lhs, rhs, name, true)
            }
            TypeInfo::Char | TypeInfo::Byte | TypeInfo::Ordering => {
                self.emit_icmp_ordering(lhs, rhs, name, false)
            }
            TypeInfo::Bool => {
                // false(0) < true(1): zext to i8 then unsigned
                let i8_ty = self.builder.i8_type();
                let l = self.builder.zext(lhs, i8_ty, &format!("{name}.l.ext"));
                let r = self.builder.zext(rhs, i8_ty, &format!("{name}.r.ext"));
                self.emit_icmp_ordering(l, r, name, false)
            }
            TypeInfo::Float => self.emit_fcmp_ordering(lhs, rhs, name),
            TypeInfo::Str => self.emit_str_runtime_call(lhs, rhs, "ori_str_compare", name),
            TypeInfo::Option { inner } => self
                .emit_option_compare(lhs, rhs, inner)
                .unwrap_or_else(|| self.builder.const_i8(1)),
            TypeInfo::Result { ok, err } => self
                .emit_result_compare(lhs, rhs, ok, err)
                .unwrap_or_else(|| self.builder.const_i8(1)),
            TypeInfo::Tuple { elements } => self
                .emit_tuple_compare(lhs, rhs, &elements)
                .unwrap_or_else(|| self.builder.const_i8(1)),
            TypeInfo::List { element } => self
                .emit_list_compare(lhs, rhs, element)
                .unwrap_or_else(|| self.builder.const_i8(1)),

            // User-defined types: delegate to derived/user compare method
            TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let compare_name = self.prop_names.compare;
                    if let Some((func_id, _abi)) =
                        self.method_functions.get(&(type_name, compare_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[lhs, rhs], name)
                            .unwrap_or_else(|| self.builder.const_i8(1));
                    }
                }
                self.builder.const_i8(1) // Equal fallback
            }

            // Types without a total ordering — should not reach here if the
            // type checker is correct. Return Equal as a safe fallback.
            TypeInfo::Map { .. }
            | TypeInfo::Set { .. }
            | TypeInfo::Range
            | TypeInfo::Iterator { .. }
            | TypeInfo::Channel { .. }
            | TypeInfo::Function { .. }
            | TypeInfo::Unit
            | TypeInfo::Never
            | TypeInfo::Error => self.builder.const_i8(1),
        }
    }

    /// Emit hash computation for an inner value, producing i64.
    ///
    /// Every `TypeInfo` variant has an explicit arm. Types that don't support
    /// hashing (Channel, Function) return 0 — these should not reach this
    /// path if the type checker is correct.
    pub(crate) fn emit_inner_hash(&mut self, val: ValueId, inner_type: Idx, name: &str) -> ValueId {
        match self.type_info.get(inner_type) {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => val,
            // Unsigned small types: zero-extend to i64
            TypeInfo::Byte | TypeInfo::Bool => {
                let i64_ty = self.builder.i64_type();
                self.builder.zext(val, i64_ty, name)
            }
            // Signed small types: sign-extend to i64
            TypeInfo::Char | TypeInfo::Ordering => {
                let i64_ty = self.builder.i64_type();
                self.builder.sext(val, i64_ty, name)
            }
            TypeInfo::Float => {
                let normalized = self.normalize_float_for_hash(val);
                let i64_ty = self.builder.i64_type();
                self.builder.bitcast(normalized, i64_ty, name)
            }
            TypeInfo::Str => self.emit_str_hash_call(val, name),
            TypeInfo::Option { inner } => self
                .emit_option_hash(val, inner)
                .unwrap_or_else(|| self.builder.const_i64(0)),
            TypeInfo::Result { ok, err } => self
                .emit_result_hash(val, ok, err)
                .unwrap_or_else(|| self.builder.const_i64(0)),
            TypeInfo::Tuple { elements } => self
                .emit_tuple_hash(val, &elements)
                .unwrap_or_else(|| self.builder.const_i64(0)),
            TypeInfo::List { element } => self
                .emit_list_hash(val, element)
                .unwrap_or_else(|| self.builder.const_i64(0)),
            TypeInfo::Map { key, value } => self
                .emit_map_hash(val, key, value)
                .unwrap_or_else(|| self.builder.const_i64(0)),
            TypeInfo::Set { element } => self
                .emit_set_hash(val, element)
                .unwrap_or_else(|| self.builder.const_i64(0)),

            // User-defined types: delegate to derived/user hash method
            TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let hash_name = self.prop_names.hash;
                    if let Some((func_id, _abi)) =
                        self.method_functions.get(&(type_name, hash_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[val], name)
                            .unwrap_or_else(|| self.builder.const_i64(0));
                    }
                }
                self.builder.const_i64(0)
            }

            // Range: hash_combine over start, end, inclusive fields
            TypeInfo::Range => {
                let start = self.builder.extract_value(val, 0, &format!("{name}.s"));
                let end = self.builder.extract_value(val, 1, &format!("{name}.e"));
                let incl = self.builder.extract_value(val, 2, &format!("{name}.i"));
                match (start, end, incl) {
                    (Some(s), Some(e), Some(i)) => {
                        let i64_ty = self.builder.i64_type();
                        let i_ext = self.builder.zext(i, i64_ty, &format!("{name}.i.ext"));
                        let h = self.builder.const_i64(0);
                        let h = self.emit_hash_combine(h, s, &format!("{name}.s.hc"));
                        let h = self.emit_hash_combine(h, e, &format!("{name}.e.hc"));
                        self.emit_hash_combine(h, i_ext, &format!("{name}.i.hc"))
                    }
                    _ => self.builder.const_i64(0),
                }
            }

            // Types without meaningful hash — should not reach here if the
            // type checker is correct. Return 0 as a safe fallback.
            TypeInfo::Iterator { .. }
            | TypeInfo::Channel { .. }
            | TypeInfo::Function { .. }
            | TypeInfo::Unit
            | TypeInfo::Never
            | TypeInfo::Error => self.builder.const_i64(0),
        }
    }
}
