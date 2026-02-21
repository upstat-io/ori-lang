//! LLVM IR generation for derived trait methods.
//!
//! Generates synthetic LLVM functions for `#[derive(...)]` attributes on structs.
//! Each derived method becomes a real LLVM function registered in `method_functions`,
//! so the existing `lower_method_call` dispatch finds them with no special path.
//!
//! Dispatch is strategy-driven: `DerivedTrait::strategy()` returns a `DeriveStrategy`
//! describing the composition logic (field iteration, result combination), and this
//! module interprets the strategy in LLVM IR terms. Adding a new trait only requires
//! adding a strategy entry in `ori_ir` — no per-trait function needed here.

mod bodies;
mod field_ops;
mod string_helpers;

use ori_ir::{DerivedMethodShape, DerivedTrait, Module, Name, StructBody, TypeDeclKind};
use ori_types::{Idx, TypeEntry, TypeKind};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use super::abi::{compute_function_abi, FunctionAbi, ReturnPassing};
use super::function_compiler::FunctionCompiler;
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

use bodies::{
    compile_clone_fields, compile_default_construct, compile_for_each_field, compile_format_fields,
};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Compile derived trait methods for all types in the module.
///
/// Iterates `module.types`, finds types with `#[derive(...)]`, resolves their
/// fields from `user_types`, and generates synthetic LLVM functions for each
/// derived method. Results are registered in `method_functions` so that
/// `lower_method_call` finds them through normal dispatch.
pub fn compile_derives<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    module: &Module,
    user_types: &[TypeEntry],
) {
    let type_map: FxHashMap<Name, &TypeEntry> = user_types.iter().map(|te| (te.name, te)).collect();

    for type_decl in &module.types {
        if type_decl.derives.is_empty() {
            continue;
        }

        let TypeDeclKind::Struct(_) = &type_decl.kind else {
            trace!(
                name = %fc.lookup_name(type_decl.name),
                "skipping derives for non-struct type"
            );
            continue;
        };

        let Some(type_entry) = type_map.get(&type_decl.name) else {
            warn!(
                name = %fc.lookup_name(type_decl.name),
                "no TypeEntry for type with derives — skipping"
            );
            continue;
        };

        let TypeKind::Struct(struct_def) = &type_entry.kind else {
            warn!(
                name = %fc.lookup_name(type_decl.name),
                "TypeEntry is not a struct — skipping derives"
            );
            continue;
        };

        let type_name = type_decl.name;
        let type_idx = type_entry.idx;
        let fields = &struct_def.fields;
        let type_name_str = fc.lookup_name(type_name).to_owned();

        debug!(
            name = %type_name_str,
            derives = type_decl.derives.len(),
            fields = fields.len(),
            "compiling derived methods"
        );

        for derive_name in &type_decl.derives {
            let trait_name_str = fc.lookup_name(*derive_name);
            let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) else {
                warn!(derive = %trait_name_str, "unknown derive trait — skipping");
                continue;
            };

            let strategy = trait_kind.strategy();
            match strategy.struct_body {
                StructBody::ForEachField { field_op, combine } => {
                    compile_for_each_field(
                        fc,
                        trait_kind,
                        type_name,
                        type_idx,
                        &type_name_str,
                        fields,
                        field_op,
                        combine,
                    );
                }
                StructBody::FormatFields {
                    open,
                    separator,
                    suffix,
                    include_names,
                } => {
                    compile_format_fields(
                        fc,
                        trait_kind,
                        type_name,
                        type_idx,
                        &type_name_str,
                        fields,
                        open,
                        separator,
                        suffix,
                        include_names,
                    );
                }
                StructBody::CloneFields => {
                    compile_clone_fields(
                        fc,
                        trait_kind,
                        type_name,
                        type_idx,
                        &type_name_str,
                        fields,
                    );
                }
                StructBody::DefaultConstruct => {
                    compile_default_construct(
                        fc,
                        trait_kind,
                        type_name,
                        type_idx,
                        &type_name_str,
                        fields,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory: common derive scaffolding
// ---------------------------------------------------------------------------

/// Context returned by [`setup_derive_function`] for derive body emitters.
pub(super) struct DeriveSetup {
    pub(super) func_id: FunctionId,
    pub(super) abi: FunctionAbi,
    /// Value for `self` parameter. `None` for nullary methods (Default).
    pub(super) self_val: Option<ValueId>,
    /// Value for `other` parameter. `None` for unary/nullary methods.
    pub(super) other_val: Option<ValueId>,
    /// Resolved `str` type for string operations. `None` for shapes that
    /// don't need string handling (`Nullary`, `UnaryIdentity`).
    pub(super) str_ty_id: Option<LLVMTypeId>,
}

/// Common scaffolding for all derived trait codegen functions.
///
/// Handles: method name interning, signature construction (driven by
/// [`DerivedMethodShape`]), ABI computation, symbol mangling, and function
/// declaration. Returns a [`DeriveSetup`] with the function handle and
/// parameter values for the body to use.
fn setup_derive_function<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
) -> DeriveSetup {
    let method_name_str = trait_kind.method_name();
    let method_name = fc.intern(method_name_str);
    let shape = trait_kind.shape();

    let (param_names, param_types) = build_derive_params(fc, shape, type_idx);
    let return_type = derive_return_type(shape, type_idx);

    let sig = make_sig(method_name, param_names, param_types, return_type);
    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, self_val, param_vals) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    let self_opt = if shape.has_self() {
        Some(self_val)
    } else {
        None
    };
    let other_opt = if shape.has_other() {
        Some(param_vals[0])
    } else {
        None
    };

    let str_ty_id = match shape {
        DerivedMethodShape::BinaryPredicate
        | DerivedMethodShape::BinaryToOrdering
        | DerivedMethodShape::UnaryToInt
        | DerivedMethodShape::UnaryToStr => {
            let str_ty = fc.resolve_type(Idx::STR);
            Some(fc.builder_mut().register_type(str_ty))
        }
        DerivedMethodShape::Nullary | DerivedMethodShape::UnaryIdentity => None,
    };

    DeriveSetup {
        func_id,
        abi,
        self_val: self_opt,
        other_val: other_opt,
        str_ty_id,
    }
}

/// Build parameter names and types for a derived method from its shape.
fn build_derive_params<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    shape: DerivedMethodShape,
    type_idx: Idx,
) -> (Vec<Name>, Vec<Idx>) {
    match shape {
        DerivedMethodShape::BinaryPredicate | DerivedMethodShape::BinaryToOrdering => {
            let self_name = fc.intern("self");
            let other_name = fc.intern("other");
            (vec![self_name, other_name], vec![type_idx, type_idx])
        }
        DerivedMethodShape::UnaryIdentity
        | DerivedMethodShape::UnaryToInt
        | DerivedMethodShape::UnaryToStr => {
            let self_name = fc.intern("self");
            (vec![self_name], vec![type_idx])
        }
        DerivedMethodShape::Nullary => (vec![], vec![]),
    }
}

/// Determine the return type for a derived method from its shape.
fn derive_return_type(shape: DerivedMethodShape, type_idx: Idx) -> Idx {
    match shape {
        DerivedMethodShape::BinaryPredicate => Idx::BOOL,
        DerivedMethodShape::UnaryIdentity | DerivedMethodShape::Nullary => type_idx,
        DerivedMethodShape::UnaryToInt => Idx::INT,
        DerivedMethodShape::UnaryToStr => Idx::STR,
        DerivedMethodShape::BinaryToOrdering => Idx::ORDERING,
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `FunctionSig` for a derived method (no generics, no capabilities).
fn make_sig(
    name: Name,
    param_names: Vec<Name>,
    param_types: Vec<Idx>,
    return_type: Idx,
) -> ori_types::FunctionSig {
    ori_types::FunctionSig::synthetic(name, param_names, param_types, return_type)
}

/// Emit return instruction respecting ABI (direct, sret, or void).
///
/// Delegates to [`FunctionCompiler::emit_return`] which includes proper
/// error recording for the Direct branch's `None` case.
fn emit_derive_return<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    result: Option<ValueId>,
) {
    fc.emit_return(func_id, abi, result, "<derive>");
}

/// Emit a method call for a derived method (handles sret return).
fn emit_method_call_for_derive<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    args: &[ValueId],
    name: &str,
) -> ValueId {
    match &abi.return_abi.passing {
        ReturnPassing::Sret { .. } => {
            let ret_ty = fc.resolve_type(abi.return_abi.ty);
            let ret_ty_id = fc.builder_mut().register_type(ret_ty);
            fc.builder_mut()
                .call_with_sret(func_id, args, ret_ty_id, name)
                .unwrap_or_else(|| {
                    warn!(name, "sret call in derive method produced no value");
                    fc.builder_mut().record_codegen_error();
                    fc.builder_mut().const_i64(0)
                })
        }
        _ => fc
            .builder_mut()
            .call(func_id, args, name)
            .unwrap_or_else(|| {
                warn!(name, "call in derive method produced no value");
                fc.builder_mut().record_codegen_error();
                fc.builder_mut().const_i64(0)
            }),
    }
}
