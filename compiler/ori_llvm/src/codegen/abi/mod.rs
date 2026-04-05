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
//! by `compute_function_abi()` and never referenced again during IR emission.
//!
//! # References
//!
//! - Rust `rustc_target::abi::call::FnAbi`
//! - Swift `lib/IRGen/GenCall.cpp`
//! - Zig `src/codegen/llvm.zig` (calling convention selection)

use ori_arc::{AnnotatedSig, ArcClass, ArcClassification, ArcClassifier, Ownership};
use ori_ir::Name;
use ori_repr::ReprPlan;
use ori_types::{FunctionSig, Idx};
use rustc_hash::FxHashSet;

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
/// Computed once from `ori_types::FunctionSig` via `compute_function_abi()`.
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

/// Compute the ABI size of a type in bytes.
///
/// For types where `TypeInfo::size()` returns `None` (Tuple, Struct, Enum),
/// walks child types recursively via the store to compute the total size.
/// Recursive types (e.g., `type Expr = Leaf(int) | Binop(Expr, Expr)`) are
/// detected via a visiting set and treated as pointer-sized (8 bytes).
///
/// When `repr_plan` is provided, consults it for niche-encoded Option/Result
/// types (§07.2) — niche layouts omit the tag field, reducing ABI size.
pub fn abi_size(ty: Idx, store: &TypeInfoStore<'_>, repr_plan: Option<&ReprPlan>) -> u64 {
    let mut visiting = FxHashSet::default();
    abi_size_inner(ty, store, repr_plan, &mut visiting)
}

/// §07.2: Check if a type has niche encoding in the `ReprPlan`.
fn is_niche_encoded(ty: Idx, store: &TypeInfoStore<'_>, repr_plan: Option<&ReprPlan>) -> bool {
    repr_plan
        .and_then(|plan| plan.get_enum_repr(store.pool().resolve_fully(ty)))
        .is_some_and(|e| e.tag.is_niche())
}

/// §07.2: Check if an enum type (Option/Result/user-defined) has niche or tagless
/// encoding in the `ReprPlan`. Returns `Some(size)` if an optimized layout applies,
/// `None` to fall through to the explicit tag computation.
fn niche_enum_size(
    ty: Idx,
    variants: &[super::type_info::EnumVariantInfo],
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    visiting: &mut FxHashSet<Idx>,
) -> Option<u64> {
    let plan = repr_plan?;
    let resolved = store.pool().resolve_fully(ty);
    let enum_repr = plan.get_enum_repr(resolved)?;

    if enum_repr.tag.is_tagless() {
        let variant = variants.first()?;
        let payload: u64 = variant
            .fields
            .iter()
            .map(|&f| abi_size_inner(f, store, repr_plan, visiting))
            .sum();
        return Some(payload.max(1));
    }

    if let ori_repr::EnumTag::Niche {
        niche_variant_idx, ..
    } = &enum_repr.tag
    {
        let data_idx = usize::from(*niche_variant_idx == 0);
        let variant = variants.get(data_idx)?;
        let payload: u64 = variant
            .fields
            .iter()
            .map(|&f| {
                let size = abi_size_inner(f, store, repr_plan, visiting);
                size.div_ceil(8) * 8
            })
            .sum();
        return Some(payload.max(1));
    }

    None
}

fn abi_size_inner(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    visiting: &mut FxHashSet<Idx>,
) -> u64 {
    use super::type_info::TypeInfo;

    let info = store.get(ty);
    if let Some(size) = info.size() {
        return size;
    }

    // Cycle detection: recursive types must use heap indirection,
    // so treat them as pointer-sized when encountered again.
    if !visiting.insert(ty) {
        return 8;
    }

    // Dynamic-size types: compute recursively
    //
    // FIXME(roadmap:section-05): abi_size_inner sums field sizes WITHOUT
    // alignment padding. A struct { byte, int, byte } computes as 10 bytes
    // here, but LLVM lays it out as 24 bytes (1+7 padding + 8 + 1+7 padding).
    // This can misclassify as Direct (≤16) when Indirect (>16) is needed.
    // Currently safe because built-in composite types (Range, Option, etc.)
    // use pre-computed TypeInfo::size() that accounts for LLVM layout.
    // Needs LLVM TargetData query when user-defined structs land in Pool.
    // Also safe for dereferenceable(N) — underestimating is legal (minimum).
    let result = match &info {
        TypeInfo::Option { inner } => {
            if is_niche_encoded(ty, store, repr_plan) {
                abi_size_inner(*inner, store, repr_plan, visiting)
            } else {
                // Explicit tag: {i64 tag, payload}
                8 + abi_size_inner(*inner, store, repr_plan, visiting)
            }
        }
        TypeInfo::Result { ok, err } => {
            let ok_size = abi_size_inner(*ok, store, repr_plan, visiting);
            let err_size = abi_size_inner(*err, store, repr_plan, visiting);
            if is_niche_encoded(ty, store, repr_plan) {
                ok_size.max(err_size)
            } else {
                // Explicit tag: {i64 tag, max(ok, err) payload}
                8 + ok_size.max(err_size)
            }
        }
        TypeInfo::Tuple { elements } => elements
            .iter()
            .map(|&e| abi_size_inner(e, store, repr_plan, visiting))
            .sum(),
        TypeInfo::Struct { fields } => fields
            .iter()
            .map(|&(_, ty)| abi_size_inner(ty, store, repr_plan, visiting))
            .sum(),
        TypeInfo::Enum { variants } => {
            if let Some(size) = niche_enum_size(ty, variants, store, repr_plan, visiting) {
                visiting.remove(&ty);
                return size;
            }
            // Enum layout: {tag, [M x i64] payload} — tag width varies
            // per enum via min_tag_width (§07.1 discriminant narrowing).
            // Each variant field occupies at least one full i64 slot (8 bytes),
            // matching resolve_enum() in layout_resolver.rs.
            let tag_size = u64::from(ori_repr::min_tag_width(variants.len()).size_bytes());
            let max_payload: u64 = variants
                .iter()
                .map(|v| {
                    v.fields
                        .iter()
                        .map(|&f| {
                            let size = abi_size_inner(f, store, repr_plan, visiting);
                            // Round up to 8-byte i64 slot boundary
                            size.div_ceil(8) * 8
                        })
                        .sum::<u64>()
                })
                .max()
                .unwrap_or(0);
            if max_payload == 0 {
                tag_size // All-unit enum: { tag } = tag_size bytes
            } else {
                // Tag is padded to 8 due to [M x i64] payload alignment
                8 + max_payload
            }
        }
        _ => 8, // Fallback: pointer-sized
    };

    visiting.remove(&ty);
    result
}

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
        let info = store.get(ty);
        ParamPassing::Indirect {
            alignment: info.alignment(),
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
        let info = store.get(ty);
        ReturnPassing::Sret {
            alignment: info.alignment(),
        }
    }
}

/// Select the calling convention for a function.
///
/// - `@main` and `@extern` functions use C calling convention
/// - All other Ori functions use `fastcc` for better optimization
pub fn select_call_conv(name: &str, is_main: bool, is_extern: bool) -> CallConv {
    if is_main || is_extern || name.starts_with("ori_") {
        CallConv::C
    } else {
        CallConv::Fast
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

    let call_conv = select_call_conv(
        "", // Name not available in sig — caller overrides if needed
        sig.is_main,
        false, // Extern detection happens at caller level
    );

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
/// When `annotated_sig` is `None`, falls through to `compute_function_abi()`.
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

    let call_conv = select_call_conv("", sig.is_main, false);

    FunctionAbi {
        params,
        return_abi,
        call_conv,
    }
}

// Tests

#[cfg(test)]
mod tests;
