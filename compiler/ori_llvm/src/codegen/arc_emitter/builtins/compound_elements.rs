//! Recursive structural-trait dispatch for compound elements.

use ori_ir::Name;
use ori_types::Idx;

use crate::codegen::abi::{FunctionAbi, ParamAbi};
use crate::codegen::ir_builder::IntegerSignedness;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::{FunctionId, ValueId};

use super::super::{ArcIrEmitter, StringRuntimeReturnAbi};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `lhs.equals(rhs)` for any type, dispatching recursively.
    pub(crate) fn emit_element_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        match &type_info {
            TypeInfo::Unit | TypeInfo::Never => Some(self.builder.const_bool(true)),
            TypeInfo::Int
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Ordering => Some(self.builder.icmp_eq(lhs, rhs, "elem_eq")),
            TypeInfo::Float => Some(self.builder.fcmp_oeq(lhs, rhs, "elem_eq")),
            TypeInfo::Str => Some(self.emit_str_runtime_call(
                "ori_str_eq",
                lhs,
                rhs,
                StringRuntimeReturnAbi::BoolDirect,
            )),
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
            TypeInfo::Set { element } => {
                let element = *element;
                self.emit_set_equals(lhs, rhs, elem_ty, element)
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

    /// Resolve a derived method for a (possibly generic-composite) operand type,
    /// returning the full ABI. Prefers the per-instantiation map keyed by the
    /// materialized concrete Idx (`pool.resolve_fully`); falls back to the
    /// type-name-keyed map. Shared mono-first resolver for the eq/hash/compare
    /// element paths and the `debug`/`to_str` format paths (which need the return
    /// ABI for the sret decision).
    pub(super) fn derived_method_full(
        &self,
        ty: Idx,
        method: Name,
    ) -> Option<(FunctionId, FunctionAbi)> {
        if self.ctx.executable_facts_bound {
            return self.lookup_exact_method_target(ty, method).cloned();
        }
        let resolved = self.pool.resolve_fully(ty);
        if let Some((fid, abi)) = self.ctx.mono_derive_functions.get(&(resolved, method)) {
            return Some((*fid, abi.clone()));
        }
        let type_name = *self.ctx.type_idx_to_name.get(&ty)?;
        let (fid, abi) = self.ctx.method_functions.get(&(type_name, method))?;
        Some((*fid, abi.clone()))
    }

    /// Params-only projection of [`Self::derived_method_full`] for the element
    /// eq/hash/compare paths that only need `apply_param_passing`.
    fn derived_method_for(&self, ty: Idx, method: Name) -> Option<(FunctionId, Vec<ParamAbi>)> {
        self.derived_method_full(ty, method)
            .map(|(fid, abi)| (fid, abi.params))
    }

    /// Call a compiled derived `eq` method for a user-defined type.
    fn emit_derived_eq_call(&mut self, lhs: ValueId, rhs: ValueId, ty: Idx) -> Option<ValueId> {
        let interned_eq = self.interner.intern("eq");
        let (func_id, params) = self.derived_method_for(ty, interned_eq)?;
        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, None, &params);
        self.emit_rt_call(func_id, &passed_args, "derived_eq")
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
            TypeInfo::Unit | TypeInfo::Never => Some(self.builder.const_i8(1)),
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => Some(
                self.builder
                    .emit_icmp_ordering(lhs, rhs, "elem_cmp", IntegerSignedness::Signed),
            ),
            TypeInfo::Float => Some(self.builder.emit_fcmp_ordering(lhs, rhs, "elem_cmp")),
            TypeInfo::Bool | TypeInfo::Char | TypeInfo::Byte => Some(
                self.builder
                    .emit_icmp_ordering(lhs, rhs, "elem_cmp", IntegerSignedness::Unsigned),
            ),
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
    ///
    /// Mono-first (per-instantiation concrete Idx) so a multi-instantiation
    /// generic composite dispatches the layout-correct `compare` instead of the
    /// last-bound type-name layout.
    fn emit_derived_compare_call(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        ty: Idx,
    ) -> Option<ValueId> {
        let interned_compare = self.interner.intern("compare");
        let (func_id, params) = self.derived_method_for(ty, interned_compare)?;
        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, None, &params);
        self.emit_rt_call(func_id, &passed_args, "derived_cmp")
    }

    /// Emit `val.hash()` for any type, dispatching recursively.
    pub(crate) fn emit_element_hash(&mut self, val: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        let i64_ty = self.builder.i64_type();
        match &type_info {
            TypeInfo::Unit | TypeInfo::Never => Some(self.builder.const_i64(0)),
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
            TypeInfo::Map { key, value } => {
                let key = *key;
                let value = *value;
                self.emit_map_hash(val, key, value)
            }
            TypeInfo::Set { element } => {
                let element = *element;
                self.emit_set_hash(val, elem_ty, element)
            }
            TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
                self.emit_derived_hash_call(val, elem_ty)
            }
            _ => None,
        }
    }

    /// Call a compiled derived `hash` method for a user-defined type.
    fn emit_derived_hash_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let interned_hash = self.interner.intern("hash");
        let (func_id, params) = self.derived_method_for(ty, interned_hash)?;
        let raw_args = [val];
        let passed_args = self.apply_param_passing(&raw_args, None, &params);
        self.emit_rt_call(func_id, &passed_args, "derived_hash")
    }
}
