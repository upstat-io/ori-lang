//! Common scaffolding shared by every derived-method body emitter: the
//! [`DeriveSetup`] context factory, signature/ABI construction, and the
//! sret-aware derived-method call + return helpers.
//!
//! Consumed across the `derive_codegen` submodules (`bodies`, `enum_bodies`,
//! `string_helpers`); the per-strategy dispatch entry points live in the parent
//! `mod`.

use ori_ir::{DerivedMethodShape, DerivedTrait, Name};
use ori_types::Idx;
use tracing::warn;

use super::super::abi::{compute_function_abi, FunctionAbi, ParamPassing, ReturnPassing};
use super::super::function_compiler::FunctionCompiler;
use super::super::value_id::{FunctionId, LLVMTypeId, ValueId};

/// Context returned by [`setup_derive_function`] for derive body emitters.
#[derive(Debug)]
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
    /// The Ori type index for this type (used for LLVM type resolution in
    /// payload enum derives).
    pub(super) type_idx: Idx,
}

/// Common scaffolding for all derived trait codegen functions.
///
/// Handles: method name interning, signature construction (driven by
/// [`DerivedMethodShape`]), ABI computation, symbol mangling, and function
/// declaration. Returns a [`DeriveSetup`] with the function handle and
/// parameter values for the body to use.
pub(super) fn setup_derive_function<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    mono: bool,
) -> DeriveSetup {
    let method_name_str = trait_kind.method_name();
    let method_name = fc.intern(method_name_str);
    let shape = trait_kind.shape();

    let (param_names, param_types) = build_derive_params(fc, shape, type_idx);
    let return_type = derive_return_type(shape, type_idx);

    let sig = make_sig(method_name, param_names, param_types, return_type);
    let abi = compute_function_abi(&sig, fc.type_info(), fc.repr_plan());
    // A generic composite emits one method per concrete instantiation; the
    // type-name-only symbol would collide across instantiations, so a mono
    // method carries a per-instantiation discriminator from its concrete Idx
    // Derived methods dispatch by `FunctionId` via the lookup
    // maps, never by the mangled string, so the suffix is internal-only.
    let symbol = if mono {
        format!(
            "{}$M{}",
            fc.mangle_method(type_name_str, method_name_str),
            type_idx.raw()
        )
    } else {
        fc.mangle_method(type_name_str, method_name_str)
    };

    let (func_id, self_val, param_vals) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    if mono {
        // Key per-instantiation dispatch on the materialized concrete Idx so
        // nested + multi-instantiation call sites resolve the layout-correct
        // body. `type_idx` is already the concrete Struct/Enum Idx.
        fc.register_mono_derive_function(type_idx, method_name, func_id, abi.clone());
    }

    // Approach (b.1): mark pure derived methods as nounwind directly.
    // Eq, Comparable, Hashable, Clone, Default only do field operations and
    // call nounwind runtime functions (ori_rc_inc, etc.). Printable and Debug
    // allocate strings and may call non-nounwind runtime functions.
    if trait_kind.is_nounwind_derived() {
        fc.builder_mut().add_nounwind_attribute(func_id);
    }

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
        type_idx,
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

/// Build a `FunctionSig` for a derived method (no generics, no capabilities).
fn make_sig(
    name: Name,
    param_names: Vec<Name>,
    param_types: Vec<Idx>,
    return_type: Idx,
) -> ori_types::FunctionSig {
    ori_types::FunctionSig::synthetic(name, param_names, param_types, return_type)
}

/// Verify a derived function's LLVM IR after body emission.
///
/// Gated on `FunctionCompiler::verify_arc()` (i.e., `ORI_VERIFY_ARC=1`).
/// Called at the end of each top-level derive body function — this is the
/// single source of truth for derive function verification logic.
pub(super) fn verify_derive_function<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    context: &str,
) {
    if fc.verify_arc() {
        let fn_val = fc.builder_mut().get_function_value(func_id);
        if !fn_val.verify(true) {
            tracing::error!(context, "LLVM IR verification failed (derive codegen)");
            fc.builder_mut().record_codegen_error();
        }
    }
}

/// Emit return instruction respecting ABI (direct, sret, or void).
///
/// Delegates to [`FunctionCompiler::emit_return`] which includes proper
/// error recording for the Direct branch's `None` case.
pub(super) fn emit_derive_return<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    result: Option<ValueId>,
) {
    fc.emit_return(func_id, abi, result, "<derive>");
}

/// Emit a derived method call whose self argument is ALREADY a pointer to the
/// receiver value (a boxed recursive back-edge field: the slot holds an RC `ptr`
/// to a heap-allocated value per `repr_box_oracle`). The self param is passed
/// directly — never stored to an alloca — since it is already the pointer the
/// Reference/Indirect ABI expects. Handles the sret return like
/// [`emit_method_call_for_derive`].
pub(super) fn emit_boxed_self_method_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    self_ptr: ValueId,
    name: &str,
) -> ValueId {
    match &abi.return_abi.passing {
        ReturnPassing::Sret { .. } => {
            let ret_ty = fc.resolve_type(abi.return_abi.ty);
            let ret_ty_id = fc.builder_mut().register_type(ret_ty);
            fc.builder_mut()
                .call_with_sret(func_id, &[self_ptr], ret_ty_id, name)
                .unwrap_or_else(|| {
                    warn!(name, "boxed sret call in derive method produced no value");
                    fc.builder_mut().record_codegen_error();
                    fc.builder_mut().const_i64(0)
                })
        }
        _ => fc
            .builder_mut()
            .call(func_id, &[self_ptr], name)
            .unwrap_or_else(|| {
                warn!(name, "boxed call in derive method produced no value");
                fc.builder_mut().record_codegen_error();
                fc.builder_mut().const_i64(0)
            }),
    }
}

/// Emit a method call for a derived method (handles sret return).
pub(super) fn emit_method_call_for_derive<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    args: &[ValueId],
    name: &str,
) -> ValueId {
    // Fixup args: if the callee expects Indirect/Reference, store the struct
    // value to an alloca and pass the pointer instead.
    let mut fixed_args: Vec<ValueId> = Vec::with_capacity(args.len());
    for (i, &arg) in args.iter().enumerate() {
        if let Some(param) = abi.params.get(i) {
            match param.passing {
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    let param_ty = fc.resolve_type(param.ty);
                    let param_ty_id = fc.builder_mut().register_type(param_ty);
                    let alloca = fc.entry_alloca(param_ty_id, &format!("{name}.arg.{i}"));
                    fc.builder_mut().store(arg, alloca);
                    fixed_args.push(alloca);
                }
                _ => fixed_args.push(arg),
            }
        } else {
            fixed_args.push(arg);
        }
    }

    match &abi.return_abi.passing {
        ReturnPassing::Sret { .. } => {
            let ret_ty = fc.resolve_type(abi.return_abi.ty);
            let ret_ty_id = fc.builder_mut().register_type(ret_ty);
            fc.builder_mut()
                .call_with_sret(func_id, &fixed_args, ret_ty_id, name)
                .unwrap_or_else(|| {
                    warn!(name, "sret call in derive method produced no value");
                    fc.builder_mut().record_codegen_error();
                    fc.builder_mut().const_i64(0)
                })
        }
        _ => fc
            .builder_mut()
            .call(func_id, &fixed_args, name)
            .unwrap_or_else(|| {
                warn!(name, "call in derive method produced no value");
                fc.builder_mut().record_codegen_error();
                fc.builder_mut().const_i64(0)
            }),
    }
}
