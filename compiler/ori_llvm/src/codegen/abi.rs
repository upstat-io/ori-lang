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
use ori_types::{FunctionSig, Idx};
use rustc_hash::FxHashSet;

use super::type_info::TypeInfoStore;

// ---------------------------------------------------------------------------
// Passing mode enums
// ---------------------------------------------------------------------------

/// How a parameter is passed to the callee.
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

// ---------------------------------------------------------------------------
// ABI descriptors
// ---------------------------------------------------------------------------

/// Physical ABI for a single parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamAbi {
    /// Parameter name (for debug info / naming LLVM values).
    pub name: Name,
    /// Ori type index (for LLVM type resolution).
    pub ty: Idx,
    /// How this parameter is physically passed.
    pub passing: ParamPassing,
}

/// Physical ABI for the return value.
#[derive(Clone, Debug, PartialEq, Eq)]
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

// ---------------------------------------------------------------------------
// ABI computation
// ---------------------------------------------------------------------------

/// Compute the ABI size of a type in bytes.
///
/// For types where `TypeInfo::size()` returns `None` (Tuple, Struct, Enum),
/// walks child types recursively via the store to compute the total size.
/// Recursive types (e.g., `type Expr = Leaf(int) | Binop(Expr, Expr)`) are
/// detected via a visiting set and treated as pointer-sized (8 bytes).
pub fn abi_size(ty: Idx, store: &TypeInfoStore<'_>) -> u64 {
    let mut visiting = FxHashSet::default();
    abi_size_inner(ty, store, &mut visiting)
}

fn abi_size_inner(ty: Idx, store: &TypeInfoStore<'_>, visiting: &mut FxHashSet<Idx>) -> u64 {
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
    let result = match &info {
        TypeInfo::Tuple { elements } => {
            // Sum of element sizes (simplified — ignores padding between fields,
            // but sufficient for ABI classification where we only care about
            // the ≤16 byte threshold).
            elements
                .iter()
                .map(|&e| abi_size_inner(e, store, visiting))
                .sum()
        }
        TypeInfo::Struct { fields } => fields
            .iter()
            .map(|&(_, ty)| abi_size_inner(ty, store, visiting))
            .sum(),
        TypeInfo::Enum { variants } => {
            // 1 byte tag + max variant payload
            let max_payload: u64 = variants
                .iter()
                .map(|v| {
                    v.fields
                        .iter()
                        .map(|&f| abi_size_inner(f, store, visiting))
                        .sum::<u64>()
                })
                .max()
                .unwrap_or(0);
            // Tag (1 byte) + padding to 8-byte alignment + payload
            if max_payload == 0 {
                1 // All-unit enum: just a tag
            } else {
                8 + max_payload // 1-byte tag padded to 8 + payload
            }
        }
        _ => 8, // Fallback: pointer-sized
    };

    visiting.remove(&ty);
    result
}

/// Compute the passing mode for a function parameter.
pub fn compute_param_passing(ty: Idx, store: &TypeInfoStore<'_>) -> ParamPassing {
    if ty == Idx::UNIT || ty == Idx::NEVER {
        return ParamPassing::Void;
    }

    let size = abi_size(ty, store);
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
pub fn compute_return_passing(ty: Idx, store: &TypeInfoStore<'_>) -> ReturnPassing {
    if ty == Idx::UNIT || ty == Idx::NEVER {
        return ReturnPassing::Void;
    }

    let size = abi_size(ty, store);
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
pub fn compute_function_abi(sig: &FunctionSig, store: &TypeInfoStore<'_>) -> FunctionAbi {
    let params: Vec<ParamAbi> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&name, &ty)| ParamAbi {
            name,
            ty,
            passing: compute_param_passing(ty, store),
        })
        .collect();

    let return_abi = ReturnAbi {
        ty: sig.return_type,
        passing: compute_return_passing(sig.return_type, store),
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

// ---------------------------------------------------------------------------
// ARC borrow-aware ABI computation
// ---------------------------------------------------------------------------

/// Compute parameter passing with ownership annotation from borrow inference.
///
/// When a parameter is `Borrowed` AND non-Scalar, it becomes `Reference`
/// (pointer, no RC). Otherwise, falls through to size-based logic
/// (`Direct`/`Indirect`).
pub fn compute_param_passing_with_ownership(
    ty: Idx,
    store: &TypeInfoStore<'_>,
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
    compute_param_passing(ty, store)
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
) -> FunctionAbi {
    let Some(annotated_sig) = annotated_sig else {
        return compute_function_abi(sig, store);
    };

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
                passing: compute_param_passing_with_ownership(ty, store, ownership, arc_class),
            }
        })
        .collect();

    let return_abi = ReturnAbi {
        ty: sig.return_type,
        passing: compute_return_passing(sig.return_type, store),
    };

    let call_conv = select_call_conv("", sig.is_main, false);

    FunctionAbi {
        params,
        return_abi,
        call_conv,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ori_types::Pool;

    fn test_store() -> (Pool, TypeInfoStore<'static>) {
        let pool = Pool::new();
        // SAFETY: We're creating the store in the same scope as the pool.
        // The store borrows the pool, and we return both together.
        // This is safe because both live for the duration of the test.
        let store = unsafe {
            let pool_ref: &Pool = &*(&pool as *const Pool);
            TypeInfoStore::new(pool_ref)
        };
        (pool, store)
    }

    // -- abi_size tests --

    #[test]
    fn primitive_abi_sizes() {
        let (_pool, store) = test_store();

        assert_eq!(abi_size(Idx::INT, &store), 8);
        assert_eq!(abi_size(Idx::FLOAT, &store), 8);
        assert_eq!(abi_size(Idx::BOOL, &store), 1);
        assert_eq!(abi_size(Idx::CHAR, &store), 4);
        assert_eq!(abi_size(Idx::BYTE, &store), 1);
        assert_eq!(abi_size(Idx::UNIT, &store), 8);
        assert_eq!(abi_size(Idx::ORDERING, &store), 1);
    }

    #[test]
    fn composite_abi_sizes() {
        let (_pool, store) = test_store();

        // str: {i64, ptr} = 16
        assert_eq!(abi_size(Idx::STR, &store), 16);
    }

    #[test]
    fn list_abi_size_is_indirect() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let store = TypeInfoStore::new(&pool);

        // [int]: {i64, i64, ptr} = 24
        assert_eq!(abi_size(list_int, &store), 24);
    }

    #[test]
    fn map_abi_size_is_indirect() {
        let mut pool = Pool::new();
        let map_ty = pool.map(Idx::STR, Idx::INT);
        let store = TypeInfoStore::new(&pool);

        // {str: int}: {i64, i64, ptr, ptr} = 32
        assert_eq!(abi_size(map_ty, &store), 32);
    }

    #[test]
    fn option_int_is_direct() {
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);
        let store = TypeInfoStore::new(&pool);

        // option[int]: {i8, i64} = 16
        assert_eq!(abi_size(opt_int, &store), 16);
    }

    #[test]
    fn tuple_abi_size_computed_recursively() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
        let store = TypeInfoStore::new(&pool);

        // (int, float) = 8 + 8 = 16
        assert_eq!(abi_size(tup, &store), 16);
    }

    #[test]
    fn large_tuple_exceeds_threshold() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::STR]);
        let store = TypeInfoStore::new(&pool);

        // (int, float, str) = 8 + 8 + 16 = 32
        assert_eq!(abi_size(tup, &store), 32);
    }

    // -- Param passing tests --

    #[test]
    fn param_passing_direct_for_small_types() {
        let (_pool, store) = test_store();

        assert_eq!(
            compute_param_passing(Idx::INT, &store),
            ParamPassing::Direct
        );
        assert_eq!(
            compute_param_passing(Idx::STR, &store),
            ParamPassing::Direct
        );
    }

    #[test]
    fn param_passing_void_for_unit() {
        let (_pool, store) = test_store();

        assert_eq!(compute_param_passing(Idx::UNIT, &store), ParamPassing::Void);
        assert_eq!(
            compute_param_passing(Idx::NEVER, &store),
            ParamPassing::Void
        );
    }

    #[test]
    fn param_passing_indirect_for_large_types() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let store = TypeInfoStore::new(&pool);

        assert_eq!(
            compute_param_passing(list_int, &store),
            ParamPassing::Indirect { alignment: 8 }
        );
    }

    // -- Return passing tests --

    #[test]
    fn return_passing_direct_for_small_types() {
        let (_pool, store) = test_store();

        assert_eq!(
            compute_return_passing(Idx::INT, &store),
            ReturnPassing::Direct
        );
        assert_eq!(
            compute_return_passing(Idx::STR, &store),
            ReturnPassing::Direct
        );
    }

    #[test]
    fn return_passing_void_for_unit() {
        let (_pool, store) = test_store();

        assert_eq!(
            compute_return_passing(Idx::UNIT, &store),
            ReturnPassing::Void
        );
    }

    #[test]
    fn return_passing_sret_for_large_types() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let store = TypeInfoStore::new(&pool);

        assert_eq!(
            compute_return_passing(list_int, &store),
            ReturnPassing::Sret { alignment: 8 }
        );
    }

    #[test]
    fn return_passing_sret_for_map() {
        let mut pool = Pool::new();
        let map_ty = pool.map(Idx::STR, Idx::INT);
        let store = TypeInfoStore::new(&pool);

        assert_eq!(
            compute_return_passing(map_ty, &store),
            ReturnPassing::Sret { alignment: 8 }
        );
    }

    // -- Calling convention tests --

    #[test]
    fn call_conv_fast_for_normal_functions() {
        assert_eq!(select_call_conv("my_func", false, false), CallConv::Fast);
        assert_eq!(select_call_conv("add", false, false), CallConv::Fast);
    }

    #[test]
    fn call_conv_c_for_main() {
        assert_eq!(select_call_conv("main", true, false), CallConv::C);
    }

    #[test]
    fn call_conv_c_for_extern() {
        assert_eq!(select_call_conv("ffi_func", false, true), CallConv::C);
    }

    #[test]
    fn call_conv_c_for_runtime() {
        assert_eq!(select_call_conv("ori_print", false, false), CallConv::C);
    }

    // -- compute_function_abi e2e --

    #[test]
    fn compute_abi_simple_function() {
        let pool = Pool::new();
        let store = TypeInfoStore::new(&pool);

        let sig = FunctionSig {
            name: Name::from_raw(1),
            type_params: vec![],
            param_names: vec![Name::from_raw(2), Name::from_raw(3)],
            param_types: vec![Idx::INT, Idx::INT],
            return_type: Idx::INT,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 2,
        };

        let abi = compute_function_abi(&sig, &store);

        assert_eq!(abi.params.len(), 2);
        assert_eq!(abi.params[0].passing, ParamPassing::Direct);
        assert_eq!(abi.params[1].passing, ParamPassing::Direct);
        assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
        assert_eq!(abi.call_conv, CallConv::Fast);
    }

    #[test]
    fn compute_abi_void_return() {
        let pool = Pool::new();
        let store = TypeInfoStore::new(&pool);

        let sig = FunctionSig {
            name: Name::from_raw(1),
            type_params: vec![],
            param_names: vec![],
            param_types: vec![],
            return_type: Idx::UNIT,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
        };

        let abi = compute_function_abi(&sig, &store);

        assert!(abi.params.is_empty());
        assert_eq!(abi.return_abi.passing, ReturnPassing::Void);
    }

    #[test]
    fn compute_abi_main_uses_c_convention() {
        let pool = Pool::new();
        let store = TypeInfoStore::new(&pool);

        let sig = FunctionSig {
            name: Name::from_raw(1),
            type_params: vec![],
            param_names: vec![],
            param_types: vec![],
            return_type: Idx::UNIT,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: true,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
        };

        let abi = compute_function_abi(&sig, &store);
        assert_eq!(abi.call_conv, CallConv::C);
    }

    // -- Borrow-aware param passing tests --

    #[test]
    fn borrowed_definiteref_becomes_reference() {
        let (_pool, store) = test_store();
        // str is DefiniteRef (heap-allocated), Borrowed → Reference
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::STR,
                &store,
                Ownership::Borrowed,
                ArcClass::DefiniteRef,
            ),
            ParamPassing::Reference
        );
    }

    #[test]
    fn borrowed_possibleref_becomes_reference() {
        let (_pool, store) = test_store();
        // PossibleRef + Borrowed → Reference (conservative: might need RC)
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::STR,
                &store,
                Ownership::Borrowed,
                ArcClass::PossibleRef,
            ),
            ParamPassing::Reference
        );
    }

    #[test]
    fn borrowed_scalar_stays_direct() {
        let (_pool, store) = test_store();
        // int is Scalar — Borrowed doesn't change passing (no RC regardless)
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::INT,
                &store,
                Ownership::Borrowed,
                ArcClass::Scalar,
            ),
            ParamPassing::Direct
        );
    }

    #[test]
    fn owned_definiteref_uses_size_based() {
        let (_pool, store) = test_store();
        // str (16 bytes, ≤ threshold) + Owned → Direct (size-based)
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::STR,
                &store,
                Ownership::Owned,
                ArcClass::DefiniteRef,
            ),
            ParamPassing::Direct
        );
    }

    #[test]
    fn owned_scalar_stays_direct() {
        let (_pool, store) = test_store();
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::INT,
                &store,
                Ownership::Owned,
                ArcClass::Scalar,
            ),
            ParamPassing::Direct
        );
    }

    #[test]
    fn unit_always_void_regardless_of_ownership() {
        let (_pool, store) = test_store();
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::UNIT,
                &store,
                Ownership::Borrowed,
                ArcClass::Scalar,
            ),
            ParamPassing::Void
        );
        assert_eq!(
            compute_param_passing_with_ownership(
                Idx::NEVER,
                &store,
                Ownership::Owned,
                ArcClass::Scalar,
            ),
            ParamPassing::Void
        );
    }

    #[test]
    fn owned_large_type_stays_indirect() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let store = TypeInfoStore::new(&pool);
        // [int] (24 bytes, > threshold) + Owned → Indirect
        assert_eq!(
            compute_param_passing_with_ownership(
                list_int,
                &store,
                Ownership::Owned,
                ArcClass::DefiniteRef,
            ),
            ParamPassing::Indirect { alignment: 8 }
        );
    }

    #[test]
    fn borrowed_large_type_becomes_reference() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let store = TypeInfoStore::new(&pool);
        // [int] + Borrowed + DefiniteRef → Reference (not Indirect)
        assert_eq!(
            compute_param_passing_with_ownership(
                list_int,
                &store,
                Ownership::Borrowed,
                ArcClass::DefiniteRef,
            ),
            ParamPassing::Reference
        );
    }

    // -- compute_function_abi_with_ownership e2e --

    #[test]
    fn abi_with_ownership_uses_reference_for_borrowed_params() {
        let pool = Pool::new();
        let store = TypeInfoStore::new(&pool);
        let classifier = ArcClassifier::new(&pool);

        let sig = FunctionSig {
            name: Name::from_raw(1),
            type_params: vec![],
            param_names: vec![Name::from_raw(2), Name::from_raw(3)],
            param_types: vec![Idx::STR, Idx::INT],
            return_type: Idx::INT,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 2,
        };

        let annotated = AnnotatedSig {
            params: vec![
                ori_arc::AnnotatedParam {
                    name: Name::from_raw(2),
                    ty: Idx::STR,
                    ownership: Ownership::Borrowed,
                },
                ori_arc::AnnotatedParam {
                    name: Name::from_raw(3),
                    ty: Idx::INT,
                    ownership: Ownership::Owned,
                },
            ],
            return_type: Idx::INT,
        };

        let abi = compute_function_abi_with_ownership(&sig, &store, Some(&annotated), &classifier);

        // str param is Borrowed + DefiniteRef → Reference
        assert_eq!(abi.params[0].passing, ParamPassing::Reference);
        // int param is Owned + Scalar → Direct
        assert_eq!(abi.params[1].passing, ParamPassing::Direct);
        assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
    }

    #[test]
    fn abi_with_ownership_none_falls_through() {
        let pool = Pool::new();
        let store = TypeInfoStore::new(&pool);
        let classifier = ArcClassifier::new(&pool);

        let sig = FunctionSig {
            name: Name::from_raw(1),
            type_params: vec![],
            param_names: vec![Name::from_raw(2)],
            param_types: vec![Idx::STR],
            return_type: Idx::STR,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 1,
        };

        // No borrow info → falls through to standard compute_function_abi
        let abi = compute_function_abi_with_ownership(&sig, &store, None, &classifier);
        assert_eq!(abi.params[0].passing, ParamPassing::Direct);
    }
}
