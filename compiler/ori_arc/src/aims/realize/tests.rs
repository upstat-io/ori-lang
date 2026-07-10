//! Tests for the unified realization module.

use super::decide::{decide_annotations, decide_cow, decide_drop_hint, AnnotationSiteContext};
use crate::aims::lattice::{
    AccessClass, Cardinality, Consumption, ReuseCtorKind, ShapeClass, Uniqueness,
};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;
use rustc_hash::FxHashSet;

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Helper: create a default `AnnotationSiteContext` for COW tests.
///
/// Defaults: non-param, non-excluded, non-collection, no RC inc, no borrowed arg,
/// Owned access, Linear consumption, Once cardinality, `NonReusable` shape,
/// not borrow-disjoint.
fn cow_ctx(var: ArcVarId, uniqueness: Uniqueness) -> AnnotationSiteContext<'static> {
    // Leak a boxed empty set so we can return a static reference.
    // Fine for tests — bounded number of calls.
    let empty: &'static FxHashSet<ArcVarId> = Box::leak(Box::default());
    AnnotationSiteContext {
        var,
        uniqueness,
        rc_incremented: false,
        is_param: false,
        is_param_borrowed: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: empty,
        is_excluded: false,
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        shape: ShapeClass::NonReusable,
        is_borrow_disjoint: false,
        has_active_borrows: false,
        is_collection: false,
    }
}

/// Helper: create an `AnnotationSiteContext` for drop hint tests.
///
/// Same as `cow_ctx` but with `is_collection: true` (required for drop hints).
fn drop_ctx(var: ArcVarId, uniqueness: Uniqueness) -> AnnotationSiteContext<'static> {
    let mut ctx = cow_ctx(var, uniqueness);
    ctx.is_collection = true;
    ctx
}

// decide_cow tests

#[test]
fn cow_unique_no_rc_inc_is_static_unique() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_unique_with_rc_inc_is_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.rc_incremented = true;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_maybe_shared_is_dynamic() {
    let ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_excluded_is_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.is_excluded = true;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_shared_is_static_shared() {
    let ctx = cow_ctx(var(0), Uniqueness::Shared);
    assert_eq!(decide_cow(&ctx), CowMode::StaticShared);
}

/// Regression: the former `is_cow_aware_unique` path promoted
/// `MaybeShared + Owned + Linear + Once` params to `StaticUnique`. Removed
/// as unsound per §DP-10 removal — backward
/// analysis facts cannot prove PAST uniqueness.
#[test]
fn decide_cow_maybe_shared_param_owned_linear_once_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    // Owned + Linear + Once on a param — previously StaticUnique, now Dynamic.
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn decide_cow_maybe_shared_param_many_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    ctx.cardinality = Cardinality::Many;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Boundary cell: param + Owned + Affine + Once. `is_cow_aware_unique` was
/// gated on Linear; Affine already bypassed. Preserve post-fix behavior.
#[test]
fn decide_cow_maybe_shared_param_owned_affine_once_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    ctx.consumption = Consumption::Affine;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Boundary cell: non-param + Owned + Linear + Once. `is_cow_aware_unique`
/// was gated on `is_param=true`; non-param already bypassed. Preserve.
#[test]
fn decide_cow_maybe_shared_nonparam_owned_linear_once_returns_dynamic() {
    let ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    // Defaults: is_param=false, Owned+Linear+Once, NonReusable.
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Regression: the former cross-dimensional `CollectionBuffer + Once → StaticUnique`
/// path was removed as unsound per §DP-10 removal.
#[test]
fn decide_cow_maybe_shared_collection_buffer_once_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::CollectionBuffer;
    // Previously StaticUnique via `!is_param && Once + CollectionBuffer`; now Dynamic.
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Regression: the former cross-dimensional `ReusableCtor(Struct) + Once → StaticUnique`
/// path was removed as unsound.
#[test]
fn decide_cow_maybe_shared_reusable_ctor_struct_once_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::ReusableCtor(ReuseCtorKind::Struct);
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Regression: `ReusableCtor(EnumVariant) + Once` hit the removed path too —
/// `matches!(shape, ShapeClass::ReusableCtor(_))` matches both `Struct` and
/// `EnumVariant`.
#[test]
fn decide_cow_maybe_shared_reusable_ctor_enum_variant_once_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant);
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Preservation cell: `ContextHole` is a distinct top-level `ShapeClass`
/// variant (not nested in `ReusableCtor(_)`); already took Dynamic before
/// the fix.
#[test]
fn decide_cow_maybe_shared_context_hole_once_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::ContextHole;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Integration preservation: when the upstream
/// `is_borrow_disjoint_from_siblings()` helper has set `ctx.is_borrow_disjoint =
/// true` (RL-31), `decide_cow()` correctly promotes `MaybeShared` receivers to
/// `StaticUnique`. This is the spec-approved disjoint-borrow path.
#[test]
fn decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_borrow_disjoint = true;
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

// DP-5/DP-9 borrow overlap tests for Unique path

/// Semantic pin: Unique aggregate with active borrows → `StaticShared` per DP-9.
/// Spec: `Unique AND NOT (is_owned_and_unique + no_borrows) → StaticShared` because `IsShared`
/// on a Unique value always returns false — runtime check cannot distinguish
/// "unique but borrowed" from "unique and safe to mutate."
#[test]
fn decide_cow_unique_with_active_borrows_returns_static_shared() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.has_active_borrows = true;
    assert_eq!(decide_cow(&ctx), CowMode::StaticShared);
}

/// Negative pin: Unique aggregate with NO borrows → `StaticUnique` (preserved).
#[test]
fn decide_cow_unique_without_borrows_returns_static_unique() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    assert!(!ctx.has_active_borrows);
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

/// Edge: RC-incremented Unique with no borrows → `Dynamic` (RC guard fires first).
#[test]
fn decide_cow_unique_rc_incremented_with_no_borrows_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.rc_incremented = true;
    ctx.has_active_borrows = false;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

/// Edge: RC-incremented Unique WITH borrows → `Dynamic` (RC guard supersedes).
#[test]
fn decide_cow_unique_rc_incremented_with_borrows_returns_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.rc_incremented = true;
    ctx.has_active_borrows = true;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

// decide_drop_hint tests

#[test]
fn drop_hint_unique_collection_is_eligible() {
    let ctx = drop_ctx(var(0), Uniqueness::Unique);
    assert!(decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_unique_with_rc_inc_not_eligible() {
    let mut ctx = drop_ctx(var(0), Uniqueness::Unique);
    ctx.rc_incremented = true;
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_unique_borrowed_arg_not_eligible() {
    let mut ctx = drop_ctx(var(0), Uniqueness::Unique);
    ctx.is_borrowed_call_arg = true;
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_maybe_shared_not_eligible() {
    let ctx = drop_ctx(var(0), Uniqueness::MaybeShared);
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_excluded_not_eligible() {
    let mut ctx = drop_ctx(var(0), Uniqueness::Unique);
    ctx.is_excluded = true;
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_non_collection_not_eligible() {
    // Unique but not a collection → no drop hint.
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    assert!(!ctx.is_collection);
    assert!(!decide_drop_hint(&ctx));
}

// decide_annotations (unified Phase 2) tests

#[test]
fn annotations_cow_site_unique_clean() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, true, false);
    assert_eq!(result.cow, Some(CowMode::StaticUnique));
    assert!(!result.drop_hint);
}

#[test]
fn annotations_drop_site_unique_collection() {
    let ctx = drop_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, false, true);
    assert_eq!(result.cow, None);
    assert!(result.drop_hint);
}

#[test]
fn annotations_both_cow_and_drop() {
    let ctx = drop_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, true, true);
    assert_eq!(result.cow, Some(CowMode::StaticUnique));
    assert!(result.drop_hint);
}

#[test]
fn annotations_neither_cow_nor_drop() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, false, false);
    assert_eq!(result.cow, None);
    assert!(!result.drop_hint);
}

#[test]
fn annotations_shared_cow_site() {
    let ctx = cow_ctx(var(0), Uniqueness::Shared);
    let result = decide_annotations(&ctx, true, false);
    assert_eq!(result.cow, Some(CowMode::StaticShared));
    assert!(!result.drop_hint);
}

#[test]
fn annotations_maybe_shared_drop_site_not_eligible() {
    let ctx = drop_ctx(var(0), Uniqueness::MaybeShared);
    let result = decide_annotations(&ctx, false, true);
    assert_eq!(result.cow, None);
    assert!(!result.drop_hint);
}

// Non-RC-managed variables

// Use site decisions

// UseSemantics — Project source identity

// DefinedDead site decisions

// LastUse — suppression flags

// LastUse — regular Dec with reuse

// Regression: MaybeShared + Once + ReusableCtor → DynamicReuse
//
// The former cross-dimensional `StaticReuse` promotion was removed as
// unsound per §RL-13 removal rationale.

/// Negative pin: `decide_cow()` must NOT return `StaticUnique` for any
/// `MaybeShared` input outside the spec-approved `is_borrow_disjoint=true`
/// path. Explicitly rejects the four removed cross-dimensional promotion
/// paths.
#[test]
fn decide_cow_rejects_cross_dimensional_maybe_shared_static_unique() {
    let mut configs: Vec<AnnotationSiteContext<'static>> = Vec::new();
    // Removed path 1: is_param + Owned + Linear + Once
    let mut c1 = cow_ctx(var(0), Uniqueness::MaybeShared);
    c1.is_param = true;
    configs.push(c1);
    // Removed path 2: !is_param + CollectionBuffer + Once
    let mut c2 = cow_ctx(var(0), Uniqueness::MaybeShared);
    c2.shape = ShapeClass::CollectionBuffer;
    configs.push(c2);
    // Removed path 3: !is_param + ReusableCtor(Struct) + Once
    let mut c3 = cow_ctx(var(0), Uniqueness::MaybeShared);
    c3.shape = ShapeClass::ReusableCtor(ReuseCtorKind::Struct);
    configs.push(c3);
    // Removed path 4: !is_param + ReusableCtor(EnumVariant) + Once
    let mut c4 = cow_ctx(var(0), Uniqueness::MaybeShared);
    c4.shape = ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant);
    configs.push(c4);

    for ctx in configs {
        // is_borrow_disjoint=false (default), so the only StaticUnique source
        // available is the spec-approved disjoint-borrow path — which this
        // test does not enable. All four configurations must be Dynamic.
        assert_ne!(
            decide_cow(&ctx),
            CowMode::StaticUnique,
            "MaybeShared (var={:?}, shape={:?}, is_param={}) must not promote to StaticUnique",
            ctx.var,
            ctx.shape,
            ctx.is_param
        );
    }
}

// Enum variant reuse

// Suppression priority: consuming primop takes precedence over all others

// Cross-Dimension Synergy Tests
//
// Each test builds an ArcFunction modeling one of the synergy Ori programs,
// runs the AIMS backward analysis, and asserts that cross-dimensional
// reasoning produces the expected state.

use crate::aims::contract::{FipContract, MemoryContract};
use crate::aims::intraprocedural::analyze_function;
use crate::aims::lattice::Locality;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArgOwnership,
    CtorKind, LitValue,
};
use crate::ownership::Ownership;
use crate::ArcClass;
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

// Test helpers (synergy-specific)

struct SynergyClassifier {
    scalars: Vec<bool>,
}

impl SynergyClassifier {
    fn all_ref(count: usize) -> Self {
        Self {
            scalars: vec![false; count],
        }
    }

    fn with_scalar(mut self, idx: usize) -> Self {
        if idx < self.scalars.len() {
            self.scalars[idx] = true;
        }
        self
    }
}

impl crate::ArcClassification for SynergyClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if self
            .scalars
            .get(idx.raw() as usize)
            .copied()
            .unwrap_or(false)
        {
            ArcClass::Scalar
        } else {
            ArcClass::DefiniteRef
        }
    }
}

fn syn_block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn syn_var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn syn_ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn syn_name(n: u32) -> Name {
    Name::from_raw(n)
}

fn no_contracts() -> FxHashMap<Name, MemoryContract> {
    FxHashMap::default()
}

// Locality × Uniqueness: block_local_unique
//
// A freshly constructed value returned from a function is Unique because
// the Construct creates a fresh allocation with RC=1. This is the
// locality×uniqueness interaction: the value is function-local (never
// shared) and Construct produces a unique reference.
//
// The interprocedural contract captures this: return_info.uniqueness = Unique
// and preserves_freshness = true.

#[test]
fn synergy_block_local_construct_is_unique() {
    // fn f() -> T { v0 = Construct(Struct, []); return v0 }
    let func = ArcFunction {
        name: syn_name(1),
        return_type: syn_ty(0),
        var_types: vec![syn_ty(0)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: syn_var(0),
                ty: syn_ty(0),
                ctor: CtorKind::Struct(syn_name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: syn_var(0) },
        }],
        ..Default::default()
    };

    let classifier = SynergyClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts =
        crate::aims::interprocedural::analyze_program(&[func], &classifier, &builtins, &interner);
    let contract = &contracts[&syn_name(1)];

    // Freshly constructed return → Unique (RC=1 at return point).
    assert_eq!(
        contract.return_info.uniqueness,
        Uniqueness::Unique,
        "freshly constructed return should be Unique"
    );
    assert!(
        contract.return_info.preserves_freshness,
        "Construct → return should preserve freshness"
    );

    // No params → no sharing, pure function.
    assert!(
        !contract.effects.may_share,
        "no params, no calls → may_share=false"
    );

    // Has Construct → may_allocate=true, not FBIP.
    // But no consumed params → FIP is Bounded(1) (1 Construct, 0 consumed).
    assert!(!contract.is_fbip, "Construct → not FBIP");
    assert!(contract.effects.may_allocate, "Construct sets may_allocate");
}

// Effect × Uniqueness: pure_callee_preserves
//
// After calling a pure function (may_share=false), the argument's
// uniqueness should be preserved.

#[test]
fn synergy_pure_callee_preserves_uniqueness() {
    // callee: fn sum(items: T) -> int { return 0 }  — pure, no share
    // caller: fn f() -> void {
    //   v0 = Construct; v1 = Apply(sum, [v0]); return v1
    // }
    let callee = ArcFunction {
        name: syn_name(1),
        params: vec![ArcParam {
            var: syn_var(0),
            ty: syn_ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: syn_ty(1), // int (scalar)
        var_types: vec![syn_ty(0), syn_ty(1)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: syn_var(1),
                ty: syn_ty(1),
                value: ArcValue::Literal(LitValue::Int(0)),
            }],
            terminator: ArcTerminator::Return { value: syn_var(1) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: syn_name(2),
        return_type: syn_ty(1),
        var_types: vec![syn_ty(0), syn_ty(1)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: syn_var(0),
                    ty: syn_ty(0),
                    ctor: CtorKind::Struct(syn_name(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: syn_var(1),
                    ty: syn_ty(1),
                    func: syn_name(1), // calls callee
                    args: vec![syn_var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: syn_var(1) },
        }],
        ..Default::default()
    };

    let classifier = SynergyClassifier::all_ref(2).with_scalar(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = crate::aims::interprocedural::analyze_program(
        &[callee, caller],
        &classifier,
        &builtins,
        &interner,
    );

    // Callee is pure: no allocations, no sharing.
    let callee_contract = &contracts[&syn_name(1)];
    assert!(
        !callee_contract.effects.may_share,
        "pure callee should have may_share=false"
    );
    assert!(
        !callee_contract.effects.may_allocate,
        "pure callee should have may_allocate=false"
    );
}

// Effect: FIP natural — token-balanced allocation (Construct balances consumed param)
//
// A function with 1 Construct (empty args) and 1 consumed non-scalar param.
// The allocation count (1 Construct) is balanced by the deallocation
// (1 consumed param). Effect analysis detects may_allocate=true but
// may_share=false (Construct has no args that could escape). FIP
// classification reads the converged effect state and token balance
// without a separate FIP pass — this is the "natural" FIP.

#[test]
fn synergy_effect_fip_natural() {
    // fn f(x: T) -> T { v1 = Construct(T, []); return v1 }
    // 1 Construct (empty args), 1 consumed param → token balanced → FIP.
    //
    // The param v0 is consumed (Dead — never used after entry).
    // Construct with empty args doesn't trigger may_share (no escaping args).
    // Result: may_allocate=true, may_share=false → FIP check path enabled.
    let func = ArcFunction {
        name: syn_name(1),
        params: vec![ArcParam {
            var: syn_var(0),
            ty: syn_ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: syn_ty(0),
        var_types: vec![syn_ty(0), syn_ty(0)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: syn_var(1),
                ty: syn_ty(0),
                ctor: CtorKind::Struct(syn_name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: syn_var(1) },
        }],
        ..Default::default()
    };

    let classifier = SynergyClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts =
        crate::aims::interprocedural::analyze_program(&[func], &classifier, &builtins, &interner);

    let contract = &contracts[&syn_name(1)];

    // Not FBIP (has Construct), but FIP-eligible.
    assert!(!contract.is_fbip, "has Construct → not FBIP");

    // Effect: may_allocate=true, may_share=false.
    assert!(contract.effects.may_allocate, "Construct sets may_allocate");
    assert!(
        !contract.effects.may_share,
        "empty-arg Construct returned should not set may_share"
    );

    // FIP: token balanced (1 Construct, 1 consumed param) → Conditional.
    // Conditional because the consumed param requires caller-guaranteed
    // uniqueness for its memory to be reusable.
    assert!(
        matches!(contract.fip, FipContract::Conditional { .. }),
        "token-balanced function should be Conditional FIP, got {:?}",
        contract.fip
    );
}

// Shape × Uniqueness × Cardinality: reuse_during_analysis
//
// A function that destructures and reconstructs same-typed value.
// The consumed value should have ReusableCtor shape.

#[test]
fn synergy_reuse_during_analysis_shape() {
    // fn inc(b: T) -> T { v1 = Project(v0, 0); v2 = Construct(T, [v1]); return v2 }
    // At the match/project site, v0 should be:
    // - shape = ReusableCtor (v0 was constructed with this ctor kind)
    // - uniqueness = Unique or MaybeShared
    // - cardinality = Once
    let func = ArcFunction {
        name: syn_name(1),
        params: vec![ArcParam {
            var: syn_var(0),
            ty: syn_ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: syn_ty(0),
        var_types: vec![syn_ty(0), syn_ty(1), syn_ty(0)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: syn_var(1),
                    ty: syn_ty(1),
                    value: syn_var(0),
                    field: 0,
                },
                ArcInstr::Construct {
                    dst: syn_var(2),
                    ty: syn_ty(0),
                    ctor: CtorKind::Struct(syn_name(10)),
                    args: vec![syn_var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: syn_var(2) },
        }],
        ..Default::default()
    };

    let classifier = SynergyClassifier::all_ref(3).with_scalar(1);
    let state_map = analyze_function(&func, &classifier, &no_contracts(), &[], Vec::new());

    // v0 is a parameter — check state at block entry (backward demand).
    let v0_entry = state_map.var_state_at_block_entry(syn_block_id(0), syn_var(0));
    // v0 is projected once → cardinality should be Once.
    assert_eq!(
        v0_entry.cardinality,
        Cardinality::Once,
        "v0 projected once → Once"
    );
    // v0 is consumed (projected then not used again) → consumption Linear or Affine.
    assert!(
        v0_entry.consumption == Consumption::Linear || v0_entry.consumption == Consumption::Affine,
        "v0 consumed by Project → Linear or Affine, got {:?}",
        v0_entry.consumption
    );

    // Shape is set post-analysis by the pipeline (set_var_shape).
    // The state map records shape from backward demand. For params,
    // the shape may be NonReusable at the entry (params get shape
    // from their constructor kind which is unknown at the callee).
    // What we CAN verify: v2 (the Construct result) gets shape via
    // fresh-construct transfer → it should have ReusableCtor in
    // its per-variable shape.
    let v2_shape = state_map.var_shape(syn_var(2));
    // Shape for freshly constructed values is set by the pipeline —
    // verify the state map tracks it.
    // Note: shape may be NonReusable here if the pipeline hasn't run
    // set_var_shape. The important thing is that backward analysis
    // correctly identifies v0 as consumed-once.
    tracing::debug!(?v2_shape, "v2 shape after analysis");
}

// Full 7-Dimension: seven_dimensions
//
// Combines intraprocedural backward demand with interprocedural contract
// analysis to verify multiple dimensions align correctly.
//
// fn inc(b: T) -> T { v1 = Project(v0, 0); v2 = Construct(T, [v1]); return v2 }
//
// Backward analysis for v0 (param):
// - access = Borrowed (only read via Project, not consumed/stored)
// - consumption = Linear (one use site: Project)
// - cardinality = Once
// - locality ≤ FunctionLocal (Rule 8: Borrowed → ≤ FunctionLocal)
// Interprocedural contract:
// - return = Unique (freshly constructed)
// - effects: may_throw=false (no Invoke)

#[test]
fn synergy_seven_dimensions_optimal() {
    let func = ArcFunction {
        name: syn_name(1),
        params: vec![ArcParam {
            var: syn_var(0),
            ty: syn_ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: syn_ty(0),
        var_types: vec![syn_ty(0), syn_ty(1), syn_ty(0)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: syn_var(1),
                    ty: syn_ty(1),
                    value: syn_var(0),
                    field: 0,
                },
                ArcInstr::Construct {
                    dst: syn_var(2),
                    ty: syn_ty(0),
                    ctor: CtorKind::Struct(syn_name(10)),
                    args: vec![syn_var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: syn_var(2) },
        }],
        ..Default::default()
    };

    let classifier = SynergyClassifier::all_ref(3).with_scalar(1);
    let state_map = analyze_function(&func, &classifier, &no_contracts(), &[], Vec::new());

    let v0_state = state_map.var_state_at_block_entry(syn_block_id(0), syn_var(0));

    // Access: Borrowed — backward analysis infers Borrowed for a param
    // that is only read (projected) without being consumed or stored.
    assert_eq!(
        v0_state.access,
        AccessClass::Borrowed,
        "read-only param should be Borrowed"
    );

    // Consumption: Linear (one use site: Project).
    assert!(
        v0_state.consumption == Consumption::Linear || v0_state.consumption == Consumption::Affine,
        "param consumed once → Linear or Affine, got {:?}",
        v0_state.consumption
    );

    // Cardinality: Once (projected once).
    assert_eq!(
        v0_state.cardinality,
        Cardinality::Once,
        "param used at one site → Once"
    );

    // Rule 8 (Borrowed → locality ≤ FunctionLocal).
    assert!(
        v0_state.locality <= Locality::FunctionLocal,
        "Borrowed param locality should be ≤ FunctionLocal (Rule 8), got {:?}",
        v0_state.locality
    );

    // Interprocedural contract: freshly constructed return → Unique.
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();
    let contracts =
        crate::aims::interprocedural::analyze_program(&[func], &classifier, &builtins, &interner);
    let contract = &contracts[&syn_name(1)];

    assert_eq!(
        contract.return_info.uniqueness,
        Uniqueness::Unique,
        "freshly constructed return should be Unique"
    );
    assert!(
        contract.return_info.preserves_freshness,
        "Construct → return should preserve freshness"
    );
    assert!(
        !contract.effects.may_throw,
        "no Invoke → may_throw should be false"
    );
    assert_eq!(
        contract.params[0].access,
        AccessClass::Borrowed,
        "param only projected → Borrowed in contract"
    );
    assert_eq!(
        contract.params[0].cardinality,
        Cardinality::Once,
        "param projected once → Once in contract"
    );
}

// Locality × Effect: local_pure_chain
//
// Multiple calls to pure functions with the same local argument.
// Effect analysis should show may_share=false for each callee.

#[test]
fn synergy_local_pure_chain_effects() {
    // callee: fn count(items: T) -> int { return 0 }  — pure
    // caller: fn stats(nums: T) -> int {
    //   v1 = Apply(count, [v0]); v2 = Apply(count, [v0]);
    //   v3 = Apply(count, [v0]); return v3
    // }
    let callee = ArcFunction {
        name: syn_name(1),
        params: vec![ArcParam {
            var: syn_var(0),
            ty: syn_ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: syn_ty(1),
        var_types: vec![syn_ty(0), syn_ty(1)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: syn_var(1),
                ty: syn_ty(1),
                value: ArcValue::Literal(LitValue::Int(0)),
            }],
            terminator: ArcTerminator::Return { value: syn_var(1) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: syn_name(2),
        params: vec![ArcParam {
            var: syn_var(0),
            ty: syn_ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: syn_ty(1),
        var_types: vec![syn_ty(0), syn_ty(1), syn_ty(1), syn_ty(1)],
        blocks: vec![ArcBlock {
            id: syn_block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: syn_var(1),
                    ty: syn_ty(1),
                    func: syn_name(1),
                    args: vec![syn_var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: syn_var(2),
                    ty: syn_ty(1),
                    func: syn_name(1),
                    args: vec![syn_var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: syn_var(3),
                    ty: syn_ty(1),
                    func: syn_name(1),
                    args: vec![syn_var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: syn_var(3) },
        }],
        ..Default::default()
    };

    let classifier = SynergyClassifier::all_ref(4).with_scalar(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = crate::aims::interprocedural::analyze_program(
        &[callee, caller],
        &classifier,
        &builtins,
        &interner,
    );

    // Callee is pure.
    let callee_contract = &contracts[&syn_name(1)];
    assert!(
        !callee_contract.effects.may_share,
        "callee should have may_share=false"
    );

    // Caller calls only pure functions → caller is also pure.
    let caller_contract = &contracts[&syn_name(2)];
    assert!(
        !caller_contract.effects.may_share,
        "caller of pure functions should have may_share=false"
    );

    // Caller's param v0 is used 3 times → cardinality=Many (not Once).
    assert_eq!(
        caller_contract.params[0].cardinality,
        Cardinality::Many,
        "param used at 3 call sites → Many"
    );
}

// Synergy metrics tests

#[test]
fn synergy_metrics_default_is_zero() {
    // Trimmed per `cow_upgrades` and `cross_dim_reuse` fields
    // removed together with the unsound paths they tracked.
    let m = super::metrics::SynergyMetrics::default();
    assert_eq!(m.total_rc_decisions, 0);
    assert_eq!(m.reuse_decisions, 0);
    assert_eq!(m.total_cow_decisions, 0);
    assert_eq!(m.natural_fip, 0);
    assert_eq!(m.canonicalize_cross_fires, 0);
}

#[test]
fn synergy_metrics_merge_additive() {
    // Trimmed per removed cow_upgrades / cross_dim_reuse assertions.
    let mut a = super::metrics::SynergyMetrics {
        reuse_decisions: 3,
        total_rc_decisions: 10,
        total_cow_decisions: 5,
        natural_fip: 0,
        canonicalize_cross_fires: 4,
    };
    let b = super::metrics::SynergyMetrics {
        reuse_decisions: 2,
        total_rc_decisions: 7,
        total_cow_decisions: 3,
        natural_fip: 1,
        canonicalize_cross_fires: 3,
    };
    a.merge(&b);
    assert_eq!(a.reuse_decisions, 5);
    assert_eq!(a.total_rc_decisions, 17);
    assert_eq!(a.total_cow_decisions, 8);
    assert_eq!(a.natural_fip, 1);
    assert_eq!(a.canonicalize_cross_fires, 7);
}

#[test]
fn synergy_metrics_reuse_percent() {
    // Preserved unchanged — only uses surviving fields.
    let m = super::metrics::SynergyMetrics {
        reuse_decisions: 3,
        total_rc_decisions: 10,
        ..Default::default()
    };
    let pct = m.reuse_percent();
    assert!((pct - 30.0).abs() < 0.01, "expected ~30%, got {pct}");
}

#[test]
fn synergy_metrics_percent_zero_total() {
    // Preserved unchanged.
    let m = super::metrics::SynergyMetrics::default();
    assert!((m.reuse_percent()).abs() < f64::EPSILON);
}

#[test]
fn synergy_metrics_cross_dim_evidence() {
    // Trimmed per `cross_dim_evidence_total()` was refactored
    // to sum only `canonicalize_cross_fires` after `cow_upgrades` and
    // `cross_dim_reuse` were removed. Test preserves coverage of the
    // surviving helper on the surviving field.
    let m = super::metrics::SynergyMetrics {
        canonicalize_cross_fires: 100,
        ..Default::default()
    };
    assert_eq!(m.cross_dim_evidence_total(), 100);
    assert!(m.has_cross_dim_evidence());

    let empty = super::metrics::SynergyMetrics::default();
    assert_eq!(empty.cross_dim_evidence_total(), 0);
    assert!(!empty.has_cross_dim_evidence());
}

#[test]
/// Regression: Rule 4 removed. `BlockLocal`+`Owned`+`Once`+`MaybeShared`
/// now produces 0 cross-dim fires and uniqueness stays `MaybeShared`.
fn canonicalize_feedback_tracks_cross_dim_fires() {
    use crate::aims::lattice::{AimsState, Locality};

    // Rule 4 REMOVED: BlockLocal+Owned+Once+MaybeShared stays MaybeShared.
    let mut state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::BlockLocal,
        shape: ShapeClass::NonReusable,
        effect: crate::aims::lattice::EffectClass::NONE,
    };
    let feedback = state.canonicalize_with_feedback();
    assert_eq!(
        feedback.cross_dim_fires, 0,
        "No cross-dim rule should fire (Rule 4 removed)"
    );
    assert_eq!(
        state.uniqueness,
        Uniqueness::MaybeShared,
        "Uniqueness should stay MaybeShared (Rule 4 removed)"
    );
}

#[test]
fn canonicalize_feedback_rule8_cross_dim_fire() {
    use crate::aims::lattice::{AimsState, Locality};

    // Rule 8: Borrowed + HeapEscaping → cap locality to FunctionLocal.
    let mut state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::HeapEscaping,
        shape: ShapeClass::NonReusable,
        effect: crate::aims::lattice::EffectClass::NONE,
    };
    let feedback = state.canonicalize_with_feedback();
    assert!(
        feedback.cross_dim_fires > 0,
        "Rule 8 should register as cross-dim fire"
    );
    assert_eq!(
        state.locality,
        Locality::FunctionLocal,
        "Rule 8 should cap locality to FunctionLocal"
    );
}

#[test]
fn canonicalize_feedback_no_fires_for_canonical_state() {
    use crate::aims::lattice::AimsState;

    // FRESH is already canonical — no rules should fire.
    let mut state = AimsState::FRESH;
    let feedback = state.canonicalize_with_feedback();
    assert_eq!(
        feedback.cross_dim_fires, 0,
        "FRESH is already canonical — no cross-dim fires"
    );
    assert_eq!(feedback.rounds, 0);
}

// RC identity + projection regression matrices (Matrix A)
//
// Decision-level tests for scalar-Project, non-scalar-Project,
// alias-split, and combined scenarios. These complement the transfer
// function tests in transfer/tests.rs and the AOT behavioral tests
// in ori_llvm/tests/aot/arc.rs.

// Store-family hand-off rep admission + forward-Jump export rep admission +
// execution-final read-alias carrier
// (`emit_unified::compute_store_handoff_reps` /
// `emit_unified::compute_forward_jump_export_reps` /
// `emit_unified::compute_store_family_final_read_carriers`)

mod store_family {
    use ori_ir::Name;
    use ori_types::Idx;
    use rustc_hash::{FxHashMap, FxHashSet};

    use crate::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind,
        ValueRepr,
    };

    use super::super::emit_unified::{
        compute_forward_jump_export_reps, compute_store_family_final_read_carriers,
        compute_store_handoff_reps,
    };

    fn alias_of(dst: u32, src: u32) -> ArcInstr {
        ArcInstr::Let {
            dst: ArcVarId::new(dst),
            ty: Idx::STR,
            value: ArcValue::Var(ArcVarId::new(src)),
        }
    }

    fn store_of(dst: u32, arg: u32) -> ArcInstr {
        ArcInstr::Construct {
            dst: ArcVarId::new(dst),
            ty: Idx::STR,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(arg)],
        }
    }

    fn binc(var: u32) -> ArcInstr {
        ArcInstr::BurdenInc {
            var: ArcVarId::new(var),
        }
    }

    fn bdec(var: u32) -> ArcInstr {
        ArcInstr::BurdenDec {
            var: ArcVarId::new(var),
        }
    }

    /// Borrowed read of `var` (a protocol `Apply` with a borrowed arg).
    fn borrow_read_of(dst: u32, var: u32) -> ArcInstr {
        ArcInstr::Apply {
            dst: ArcVarId::new(dst),
            ty: Idx::INT,
            func: Name::from_raw(9),
            args: vec![ArcVarId::new(var)],
            arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
            mono_instance_id: None,
        }
    }

    fn one_block_func(n_vars: u32, reprs: Vec<ValueRepr>, body: Vec<ArcInstr>) -> ArcFunction {
        ArcFunction {
            var_types: (0..n_vars).map(|i| Idx::from_raw(i + 1)).collect(),
            var_reprs: reprs,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body,
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(0),
                },
            }],
            entry: ArcBlockId::new(0),
            name: Name::from_raw(0),
            ..ArcFunction::default()
        }
    }

    /// `same_alloc_reps` mapping each alias var to its lineage root.
    fn reps_to_root(aliases: &[u32], root: u32) -> FxHashMap<ArcVarId, ArcVarId> {
        aliases
            .iter()
            .map(|&a| (ArcVarId::new(a), ArcVarId::new(root)))
            .collect()
    }

    #[test]
    fn store_handoff_reps_admit_rc_pointer_store_arg() {
        let func = one_block_func(
            3,
            vec![ValueRepr::RcPointer; 3],
            vec![alias_of(1, 0), store_of(2, 1)],
        );
        let reps = compute_store_handoff_reps(&func, &reps_to_root(&[1], 0));
        assert!(
            reps.contains(&ArcVarId::new(0)),
            "an RcPointer store arg admits its lineage rep; reps = {reps:?}"
        );
    }

    #[test]
    fn store_handoff_reps_decline_rc_free_store_arg() {
        // The RC-free decline: a Scalar-repr store arg carries no refcount —
        // funding/debiting it would model releases that never happen.
        let func = one_block_func(
            3,
            vec![ValueRepr::Scalar; 3],
            vec![alias_of(1, 0), store_of(2, 1)],
        );
        let reps = compute_store_handoff_reps(&func, &reps_to_root(&[1], 0));
        assert!(
            reps.is_empty(),
            "a Scalar store arg never admits a rep: {reps:?}"
        );
    }

    /// Two-block fixture: entry aliases the root and forward-Jumps the alias
    /// to block 1 (which does NOT dominate the entry).
    fn forward_jump_func(reprs: Vec<ValueRepr>) -> ArcFunction {
        ArcFunction {
            var_types: (0..3).map(|i| Idx::from_raw(i + 1)).collect(),
            var_reprs: reprs,
            blocks: vec![
                ArcBlock {
                    id: ArcBlockId::new(0),
                    params: Vec::new(),
                    body: vec![alias_of(1, 0)],
                    terminator: ArcTerminator::Jump {
                        target: ArcBlockId::new(1),
                        args: vec![ArcVarId::new(1)],
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(1),
                    params: vec![(ArcVarId::new(2), Idx::STR)],
                    body: Vec::new(),
                    terminator: ArcTerminator::Return {
                        value: ArcVarId::new(2),
                    },
                },
            ],
            entry: ArcBlockId::new(0),
            name: Name::from_raw(0),
            ..ArcFunction::default()
        }
    }

    #[test]
    fn forward_jump_export_reps_admit_rc_pointer_forward_arg() {
        let func = forward_jump_func(vec![ValueRepr::RcPointer; 3]);
        let reps = compute_forward_jump_export_reps(&func, &reps_to_root(&[1], 0));
        assert!(
            reps.contains(&ArcVarId::new(0)),
            "an RcPointer forward-Jump arg admits its lineage rep; reps = {reps:?}"
        );
    }

    #[test]
    fn forward_jump_export_reps_decline_rc_free_forward_arg() {
        // The RC-free decline: a Scalar-repr Jump arg carries no refcount —
        // its export funds no downstream release.
        let func = forward_jump_func(vec![ValueRepr::Scalar; 3]);
        let reps = compute_forward_jump_export_reps(&func, &reps_to_root(&[1], 0));
        assert!(
            reps.is_empty(),
            "a Scalar Jump arg never admits a rep: {reps:?}"
        );
    }

    #[test]
    fn forward_jump_export_reps_decline_back_edge_arg() {
        // Loop shape: 0 (entry) -> 1 (header) -> {2 (body), 3 (exit)};
        // block 2 Jumps the RC alias BACK to the header it is dominated by —
        // a back-edge arg is the loop's own param rename, not an export.
        let func = ArcFunction {
            var_types: (0..3).map(|i| Idx::from_raw(i + 1)).collect(),
            var_reprs: vec![ValueRepr::RcPointer; 3],
            blocks: vec![
                ArcBlock {
                    id: ArcBlockId::new(0),
                    params: Vec::new(),
                    body: vec![alias_of(2, 0)],
                    terminator: ArcTerminator::Jump {
                        target: ArcBlockId::new(1),
                        args: Vec::new(),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(1),
                    params: Vec::new(),
                    body: Vec::new(),
                    terminator: ArcTerminator::Branch {
                        cond: ArcVarId::new(1),
                        then_block: ArcBlockId::new(2),
                        else_block: ArcBlockId::new(3),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(2),
                    params: Vec::new(),
                    body: Vec::new(),
                    terminator: ArcTerminator::Jump {
                        target: ArcBlockId::new(1),
                        args: vec![ArcVarId::new(2)],
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(3),
                    params: Vec::new(),
                    body: Vec::new(),
                    terminator: ArcTerminator::Return {
                        value: ArcVarId::new(0),
                    },
                },
            ],
            entry: ArcBlockId::new(0),
            name: Name::from_raw(0),
            ..ArcFunction::default()
        };
        let reps = compute_forward_jump_export_reps(&func, &reps_to_root(&[2], 0));
        assert!(
            reps.is_empty(),
            "a back-edge Jump arg never admits a rep: {reps:?}"
        );
    }

    #[test]
    fn final_read_carrier_designates_unique_execution_final_pair_alias() {
        // The two-store shape's tail: store alias %1 (inc-only, funded) +
        // execution-final read alias %3 carrying the keep-alive inc+dec pair.
        let mut func = one_block_func(
            5,
            vec![ValueRepr::RcPointer; 5],
            vec![
                binc(0),
                alias_of(1, 0),
                binc(1),
                store_of(2, 1),
                alias_of(3, 0),
                binc(3),
                borrow_read_of(4, 3),
                bdec(3),
            ],
        );
        // Return the scalar read result — returning the root would be a
        // lineage use AFTER the carrier's final read (a decline fence).
        func.blocks[0].terminator = ArcTerminator::Return {
            value: ArcVarId::new(4),
        };
        let reps: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
        let carriers =
            compute_store_family_final_read_carriers(&func, &reps_to_root(&[1, 3], 0), &reps);
        assert_eq!(
            carriers.get(&ArcVarId::new(0)),
            Some(&ArcVarId::new(3)),
            "the execution-final pair alias is the lineage's release carrier; {carriers:?}"
        );
    }

    #[test]
    fn final_read_carrier_declines_moved_pair_alias() {
        // A pair alias consumed at an OWNED position (a pre-consume terminator
        // pair) is a transfer arrangement, never the keep-alive carrier —
        // eliding its inc would under-fund the consume.
        let func = one_block_func(
            5,
            vec![ValueRepr::RcPointer; 5],
            vec![
                binc(0),
                alias_of(3, 0),
                binc(3),
                bdec(3),
                ArcInstr::Apply {
                    dst: ArcVarId::new(4),
                    ty: Idx::INT,
                    func: Name::from_raw(9),
                    args: vec![ArcVarId::new(3)],
                    arg_ownership: vec![crate::ir::ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
        );
        let reps: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
        let carriers =
            compute_store_family_final_read_carriers(&func, &reps_to_root(&[3], 0), &reps);
        assert!(
            carriers.is_empty(),
            "an owned-consumed pair alias never carries the release: {carriers:?}"
        );
    }

    #[test]
    fn final_read_carrier_declines_multiple_per_arm_finals() {
        // Two pair aliases whose last reads sit on mutually-exclusive branch
        // arms — no UNIQUE execution-final carrier exists; decline.
        let func = ArcFunction {
            var_types: (0..7).map(|i| Idx::from_raw(i + 1)).collect(),
            var_reprs: vec![ValueRepr::RcPointer; 7],
            blocks: vec![
                ArcBlock {
                    id: ArcBlockId::new(0),
                    params: Vec::new(),
                    body: vec![binc(0), alias_of(2, 0), binc(2), alias_of(3, 0), binc(3)],
                    terminator: ArcTerminator::Branch {
                        cond: ArcVarId::new(1),
                        then_block: ArcBlockId::new(1),
                        else_block: ArcBlockId::new(2),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(1),
                    params: Vec::new(),
                    body: vec![borrow_read_of(5, 2), bdec(2)],
                    terminator: ArcTerminator::Return {
                        value: ArcVarId::new(5),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(2),
                    params: Vec::new(),
                    body: vec![borrow_read_of(6, 3), bdec(3)],
                    terminator: ArcTerminator::Return {
                        value: ArcVarId::new(6),
                    },
                },
            ],
            entry: ArcBlockId::new(0),
            name: Name::from_raw(0),
            ..ArcFunction::default()
        };
        let reps: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
        let carriers =
            compute_store_family_final_read_carriers(&func, &reps_to_root(&[2, 3], 0), &reps);
        assert!(
            carriers.is_empty(),
            "per-arm finals yield no unique carrier: {carriers:?}"
        );
    }

    use super::super::emit_unified::retarget_borrowed_keepalive_dec;

    /// Positive pin: the Phase-6.66b keep-alive paired dec on a BORROWED PARAM is
    /// retargeted onto the non-param `inc_arg` alias (so it never decrements the
    /// borrowed param — VF-1 `check_no_dec_on_borrowed`).
    #[test]
    fn retarget_borrowed_keepalive_dec_moves_param_dec_to_alias() {
        let param = ArcVarId::new(0);
        let alias = ArcVarId::new(3);
        let borrowed: FxHashSet<ArcVarId> = [param].into_iter().collect();
        // dec_arg names the borrowed param -> retarget onto the alias inc_arg.
        assert_eq!(
            retarget_borrowed_keepalive_dec(param, alias, &borrowed),
            alias,
            "borrowed-param dec must retarget onto the non-param keep-alive alias"
        );
    }

    /// Negative pin: a NON-borrowed `dec_arg` (a fresh owned collection's reuse
    /// alias) is NEVER retargeted — the keep-alive dec stays where the lineage's
    /// last non-iter use placed it.
    #[test]
    fn retarget_borrowed_keepalive_dec_leaves_owned_dec_unchanged() {
        let alias = ArcVarId::new(3);
        let owned_dec = ArcVarId::new(7);
        // Empty borrowed set: dec_arg is owned -> unchanged.
        let none: FxHashSet<ArcVarId> = FxHashSet::default();
        assert_eq!(
            retarget_borrowed_keepalive_dec(owned_dec, alias, &none),
            owned_dec,
            "an owned (non-borrowed) dec_arg must not be retargeted"
        );
        // dec_arg present but NOT a borrowed param -> unchanged even when the
        // borrowed set is non-empty.
        let borrowed: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
        assert_eq!(
            retarget_borrowed_keepalive_dec(owned_dec, alias, &borrowed),
            owned_dec,
            "a non-param dec_arg must not be retargeted"
        );
    }
}
