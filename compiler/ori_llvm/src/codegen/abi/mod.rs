//! ABI types and calling convention computation for V2 codegen.
//!
//! Determines how function parameters and return values are passed at the
//! LLVM level. This replaces the scattered sret checks in the legacy
//! `CodegenCx::needs_sret` and `declare.rs` with a centralized, testable
//! ABI computation pipeline.
//!
//! # Key Distinction
//!
//! - **`ori_types::FunctionSig`** = *semantic*: type params, bounds, capabilities
//! - **`FunctionAbi`** = *physical*: passing modes, calling convention, alignment
//!
//! Codegen only sees `FunctionAbi`. The semantic signature is consumed once
//! by `compute_function_abi` and never referenced again during IR emission.
//!
//! # References
//!
//! - Rust `rustc_target::abi::call::FnAbi`
//! - Swift `lib/IRGen/GenCall.cpp`
//! - Zig `src/codegen/llvm.zig` (calling convention selection)

mod size;

use ori_arc::{AnnotatedSig, ArcClass, ArcClassification, ArcClassifier, Ownership};
use ori_ir::Name;
use ori_repr::ReprPlan;
use ori_types::{FunctionSig, Idx};

#[cfg(test)]
pub(crate) use size::abi_alignment;
pub use size::abi_size;
use size::indirect_alignment;

use super::type_info::TypeInfoStore;

// Passing mode enums

/// How a parameter is passed to the callee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamPassing {
    /// Passed directly in registers (scalars, small structs ≤16 bytes).
    Direct,
    /// Passed by pointer (large structs >16 bytes). Callee reads from pointer.
    Indirect { alignment: u32 },
    /// Borrowed parameter — callee receives a pointer to the caller's value.
    /// No RC operations at the call site. The callee must not store or return
    /// the value. Produced when borrow inference determines `Ownership::Borrowed`
    /// and the type is non-Scalar (needs RC).
    Reference,
    /// Parameter has void/unit type — not physically passed.
    Void,
}

/// How a return value is passed back to the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnPassing {
    /// Returned directly in registers.
    Direct,
    /// Returned via hidden first parameter (`ptr sret(T) noalias`).
    Sret { alignment: u32 },
    /// Function returns void (unit type).
    Void,
}

/// Calling convention selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallConv {
    /// LLVM `fastcc` — internal Ori functions. Enables tail call optimization
    /// and allows LLVM to use non-standard register conventions.
    Fast,
    /// LLVM `ccc` (C calling convention) — extern functions, `@main`, FFI.
    C,
}

// ABI descriptors

/// Physical ABI for a single parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamAbi {
    /// Parameter name (for debug info / naming LLVM values).
    pub name: Name,
    /// Ori type index (for LLVM type resolution).
    pub ty: Idx,
    /// How this parameter is physically passed.
    pub passing: ParamPassing,
    /// Whether the callee only reads this parameter (never mutates).
    /// Set for `Ownership::Borrowed` params. Enables LLVM `readonly`
    /// attribute on Indirect/Reference pointer params.
    pub readonly: bool,
}

/// Physical ABI for the return value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReturnAbi {
    /// Ori type index.
    pub ty: Idx,
    /// How the return value is physically passed.
    pub passing: ReturnPassing,
}

/// Complete physical ABI for a function.
///
/// Computed once from `ori_types::FunctionSig` via `compute_function_abi`.
/// All downstream codegen (declaration, body emission, call sites) uses this
/// instead of querying types ad-hoc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionAbi {
    /// Physical ABI for each parameter.
    pub params: Vec<ParamAbi>,
    /// Physical ABI for the return value.
    pub return_abi: ReturnAbi,
    /// Calling convention.
    pub call_conv: CallConv,
}

// ABI computation
//
// Size + alignment walkers live in [`size`] (re-exported above); this module
// owns the passing-mode classification and calling-convention policy.

/// Compute the passing mode for a function parameter.
pub fn compute_param_passing(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
) -> ParamPassing {
    if ty == Idx::UNIT || ty == Idx::NEVER {
        return ParamPassing::Void;
    }

    let size = abi_size(ty, store, repr_plan);
    if size <= 16 {
        ParamPassing::Direct
    } else {
        ParamPassing::Indirect {
            alignment: indirect_alignment(ty, store, repr_plan),
        }
    }
}

/// Compute the passing mode for a function return value.
pub fn compute_return_passing(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
) -> ReturnPassing {
    if ty == Idx::UNIT || ty == Idx::NEVER {
        return ReturnPassing::Void;
    }

    let size = abi_size(ty, store, repr_plan);
    if size <= 16 {
        ReturnPassing::Direct
    } else {
        ReturnPassing::Sret {
            alignment: indirect_alignment(ty, store, repr_plan),
        }
    }
}

/// The kind of function a calling convention is selected for.
///
/// Single policy home (AB-6) — every site assigning a `CallConv` routes
/// through [`select_call_conv`] with the site's context; direct
/// `CallConv::` assignment outside this module is a policy leak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallConvSite {
    /// Ordinary Ori function (incl. capturing lambdas) — `fastcc`.
    OriFunction,
    /// `@main` entry point — C ABI (invoked by the runtime entry wrapper).
    Main,
    /// Non-capturing lambda — `ccc` so the declaration matches the closure
    /// calling convention `(ptr %env, user_args...)` directly.
    NonCapturingLambda,
    /// Test wrapper invoked by the test harness — C ABI.
    TestWrapper,
}

/// Select the calling convention for a function from its site context.
pub fn select_call_conv(site: CallConvSite) -> CallConv {
    match site {
        CallConvSite::OriFunction => CallConv::Fast,
        CallConvSite::Main | CallConvSite::NonCapturingLambda | CallConvSite::TestWrapper => {
            CallConv::C
        }
    }
}

/// `CallConvSite` for a type-checker signature (`@main` vs ordinary).
fn sig_call_conv_site(sig: &FunctionSig) -> CallConvSite {
    if sig.is_main {
        CallConvSite::Main
    } else {
        CallConvSite::OriFunction
    }
}

/// Compute the complete physical ABI for a function from its type-checker signature.
///
/// This is the single entry point that bridges `ori_types::FunctionSig` → `FunctionAbi`.
pub fn compute_function_abi(
    sig: &FunctionSig,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
) -> FunctionAbi {
    debug_assert_eq!(
        sig.param_names.len(),
        sig.param_types.len(),
        "param_names and param_types must be parallel (function {:?})",
        sig.name
    );
    let params: Vec<ParamAbi> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&name, &ty)| ParamAbi {
            name,
            ty,
            passing: compute_param_passing(ty, store, repr_plan),
            readonly: false,
        })
        .collect();

    let return_abi = ReturnAbi {
        ty: sig.return_type,
        passing: compute_return_passing(sig.return_type, store, repr_plan),
    };

    let call_conv = select_call_conv(sig_call_conv_site(sig));

    FunctionAbi {
        params,
        return_abi,
        call_conv,
    }
}

// ARC borrow-aware ABI computation

/// Compute parameter passing with ownership annotation from borrow inference.
///
/// When a parameter is `Borrowed` AND non-Scalar, it becomes `Reference`
/// (pointer, no RC). Otherwise, falls through to size-based logic
/// (`Direct`/`Indirect`).
pub fn compute_param_passing_with_ownership(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    ownership: Ownership,
    arc_class: ArcClass,
) -> ParamPassing {
    if ty == Idx::UNIT || ty == Idx::NEVER {
        return ParamPassing::Void;
    }
    // Borrowed non-scalar → pass by reference, no RC
    if ownership == Ownership::Borrowed && arc_class != ArcClass::Scalar {
        return ParamPassing::Reference;
    }
    // Owned or scalar → existing size-based logic
    compute_param_passing(ty, store, repr_plan)
}

/// Compute the complete physical ABI for a function with borrow annotations.
///
/// When `annotated_sig` is provided (from borrow inference), parameters
/// annotated as `Borrowed` with non-Scalar types are passed by `Reference`
/// (pointer, no RC at call site). All other parameters use the standard
/// size-based passing mode.
///
/// When `annotated_sig` is `None`, falls through to `compute_function_abi`.
pub fn compute_function_abi_with_ownership(
    sig: &FunctionSig,
    store: &TypeInfoStore<'_>,
    annotated_sig: Option<&AnnotatedSig>,
    classifier: &ArcClassifier<'_>,
    repr_plan: Option<&ReprPlan>,
) -> FunctionAbi {
    let Some(annotated_sig) = annotated_sig else {
        return compute_function_abi(sig, store, repr_plan);
    };

    debug_assert_eq!(
        sig.param_names.len(),
        sig.param_types.len(),
        "param_names and param_types must be parallel (function {:?})",
        sig.name
    );
    // annotated_sig is produced by borrow inference over THIS function — a
    // length mismatch is an upstream invariant break; the Owned fallback
    // below exists only for the genuinely-permitted None-annotated_sig path
    // (handled by the early return above), never for a truncated sig.
    debug_assert_eq!(
        annotated_sig.params.len(),
        sig.param_types.len(),
        "annotated_sig params must be parallel to sig param_types (function {:?})",
        sig.name
    );
    let params: Vec<ParamAbi> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .enumerate()
        .map(|(i, (&name, &ty))| {
            let (ownership, arc_class) = if i < annotated_sig.params.len() {
                (annotated_sig.params[i].ownership, classifier.arc_class(ty))
            } else {
                // No annotation → default to owned (standard passing)
                (Ownership::Owned, ArcClass::Scalar)
            };

            ParamAbi {
                name,
                ty,
                passing: compute_param_passing_with_ownership(
                    ty, store, repr_plan, ownership, arc_class,
                ),
                readonly: ownership == Ownership::Borrowed,
            }
        })
        .collect();

    let return_abi = ReturnAbi {
        ty: sig.return_type,
        passing: compute_return_passing(sig.return_type, store, repr_plan),
    };

    let call_conv = select_call_conv(sig_call_conv_site(sig));

    FunctionAbi {
        params,
        return_abi,
        call_conv,
    }
}

// Tests

#[cfg(test)]
mod tests;
