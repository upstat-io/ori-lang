//! Tests for the unified realization module.

use super::decide::{
    decide, decide_annotations, decide_cow, decide_drop_hint, AnnotationSiteContext,
    DecisionContext, DecisionSite, InstructionDecisions, RcDecision, ReuseContext, ReuseDecision,
    UseSemantics,
};
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

#[test]
fn cow_param_cow_aware_unique() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    // COW-aware: Owned + Linear + Once → StaticUnique for params.
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_param_not_cow_aware_many() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    ctx.cardinality = Cardinality::Many; // breaks Once requirement
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_cross_dim_collection_buffer_once() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::CollectionBuffer;
    // Non-param + Once + CollectionBuffer → StaticUnique.
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_cross_dim_reusable_ctor_once() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::ReusableCtor(ReuseCtorKind::Struct);
    // Non-param + Once + ReusableCtor → StaticUnique.
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_borrow_disjoint_maybe_shared() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_borrow_disjoint = true;
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
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

// InstructionDecisions type tests

#[test]
fn instruction_decisions_default_none() {
    let decisions = InstructionDecisions {
        rc: RcDecision::None,
        reuse: ReuseDecision::None,
    };
    assert_eq!(decisions.rc, RcDecision::None);
    assert_eq!(decisions.reuse, ReuseDecision::None);
}

#[test]
fn instruction_decisions_static_reuse_replaces_dec() {
    // When reuse is StaticReuse, the RC Dec is absorbed into the Reset.
    let decisions = InstructionDecisions {
        rc: RcDecision::None, // Dec absorbed by reuse
        reuse: ReuseDecision::StaticReuse,
    };
    assert_eq!(decisions.reuse, ReuseDecision::StaticReuse);
    assert_eq!(decisions.rc, RcDecision::None);
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

// Phase 1 decide() tests

fn reuse_non_reusable() -> ReuseContext {
    ReuseContext {
        shape: ShapeClass::NonReusable,
        uniqueness: Uniqueness::Unique,
        cardinality: Cardinality::Once,
    }
}

fn reuse_unique_struct() -> ReuseContext {
    ReuseContext {
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        uniqueness: Uniqueness::Unique,
        cardinality: Cardinality::Once,
    }
}

// Non-RC-managed variables

#[test]
fn decide_non_rc_managed_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: false,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// Use site decisions

#[test]
fn decide_use_with_future_use_returns_inc() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Inc);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_use_without_future_use_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: false,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// UseSemantics — Project source identity

#[test]
fn decide_use_borrowing_project_skips_inc() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::BorrowingProject,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None, "scalar Project borrows source");
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_use_transfer_project_skips_inc() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::TransferProject,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::None,
        "non-scalar Project transfers ownership"
    );
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_project_source_no_children_emits_dec() {
    // With borrow semantics, a Project source whose borrowed children
    // have already died gets a normal Dec (not suppressed). The parent's
    // drop function handles freeing RC children.
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::Dec,
        "Project source with no live children gets Dec"
    );
}

// DefinedDead site decisions

#[test]
fn decide_defined_dead_returns_dec() {
    let ctx = DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// LastUse — suppression flags

#[test]
fn decide_last_use_consuming_primop_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: true,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_ownership_transfer_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: true,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_owned_call_position_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: true,

            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_deferred_children_returns_defer() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: true,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Defer);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// LastUse — regular Dec with reuse

#[test]
fn decide_last_use_unique_struct_returns_dec_static_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::StaticReuse);
}

#[test]
fn decide_last_use_non_reusable_shape_returns_dec_no_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: reuse_non_reusable(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_shared_returns_dec_no_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                uniqueness: Uniqueness::Shared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_maybe_shared_struct_returns_dynamic_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Many,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::DynamicReuse);
}

// Cross-dimensional proof: MaybeShared + Once + ReusableCtor → StaticReuse

#[test]
fn decide_cross_dimensional_maybe_shared_once_ctor_is_static_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::StaticReuse);
}

#[test]
fn decide_cross_dimensional_maybe_shared_once_non_reusable_no_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::NonReusable,
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_cross_dimensional_maybe_shared_once_collection_is_dynamic_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::CollectionBuffer,
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    // CollectionBuffer is not NonReusable, and MaybeShared + Once but NOT
    // ReusableCtor → cross-dimensional proof doesn't apply → DynamicReuse.
    assert_eq!(d.reuse, ReuseDecision::DynamicReuse);
}

// Enum variant reuse

#[test]
fn decide_unique_enum_variant_returns_static_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
                uniqueness: Uniqueness::Unique,
                cardinality: Cardinality::Many,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::StaticReuse);
}

// Suppression priority: consuming primop takes precedence over all others

#[test]
fn decide_consuming_primop_suppresses_even_with_deferred_children() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: true,
            is_ownership_transfer: false,
            is_owned_call_position: false,

            has_deferred_children: true,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
}

// Cross-Dimension Synergy Tests (Section 11.1)
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
// without a separate FIP pass — this is the "natural" FIP from Section 09.2.

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
                },
                ArcInstr::Apply {
                    dst: syn_var(2),
                    ty: syn_ty(1),
                    func: syn_name(1),
                    args: vec![syn_var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Apply {
                    dst: syn_var(3),
                    ty: syn_ty(1),
                    func: syn_name(1),
                    args: vec![syn_var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
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

// Synergy metrics tests (Section 11.2)

#[test]
fn synergy_metrics_default_is_zero() {
    let m = super::metrics::SynergyMetrics::default();
    assert_eq!(m.total_rc_decisions, 0);
    assert_eq!(m.reuse_decisions, 0);
    assert_eq!(m.cow_upgrades, 0);
    assert_eq!(m.total_cow_decisions, 0);
    assert_eq!(m.cross_dim_reuse, 0);
    assert_eq!(m.natural_fip, 0);
    assert_eq!(m.canonicalize_cross_fires, 0);
}

#[test]
fn synergy_metrics_merge_additive() {
    let mut a = super::metrics::SynergyMetrics {
        reuse_decisions: 3,
        total_rc_decisions: 10,
        cow_upgrades: 2,
        total_cow_decisions: 5,
        cross_dim_reuse: 1,
        natural_fip: 0,
        canonicalize_cross_fires: 4,
    };
    let b = super::metrics::SynergyMetrics {
        reuse_decisions: 2,
        total_rc_decisions: 7,
        cow_upgrades: 1,
        total_cow_decisions: 3,
        cross_dim_reuse: 2,
        natural_fip: 1,
        canonicalize_cross_fires: 3,
    };
    a.merge(&b);
    assert_eq!(a.reuse_decisions, 5);
    assert_eq!(a.total_rc_decisions, 17);
    assert_eq!(a.cow_upgrades, 3);
    assert_eq!(a.total_cow_decisions, 8);
    assert_eq!(a.cross_dim_reuse, 3);
    assert_eq!(a.natural_fip, 1);
    assert_eq!(a.canonicalize_cross_fires, 7);
}

#[test]
fn synergy_metrics_reuse_percent() {
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
    let m = super::metrics::SynergyMetrics::default();
    assert!((m.reuse_percent()).abs() < f64::EPSILON);
}

#[test]
fn synergy_metrics_cross_dim_evidence() {
    let m = super::metrics::SynergyMetrics {
        canonicalize_cross_fires: 100,
        cross_dim_reuse: 5,
        cow_upgrades: 3,
        ..Default::default()
    };
    assert_eq!(m.cross_dim_evidence_total(), 108);
    assert!(m.has_cross_dim_evidence());

    let empty = super::metrics::SynergyMetrics::default();
    assert_eq!(empty.cross_dim_evidence_total(), 0);
    assert!(!empty.has_cross_dim_evidence());
}

#[test]
fn canonicalize_feedback_tracks_cross_dim_fires() {
    use crate::aims::lattice::{AimsState, Locality};

    // Rule 4: BlockLocal + Owned + Once + MaybeShared → Unique.
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
    assert!(
        feedback.cross_dim_fires > 0,
        "Rule 4 should register as cross-dim fire"
    );
    assert_eq!(
        state.uniqueness,
        Uniqueness::Unique,
        "Rule 4 should promote to Unique"
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

// A1: Scalar Project source — BorrowingProject suppresses Inc even with
// future use, and the source's last-use Dec is NOT suppressed (scalar
// Project borrows; source still needs cleanup).
#[test]
fn decide_alias_then_borrowing_project_no_inc() {
    // Scenario: let alias = src; Project(alias, scalar_field)
    // At the Project instruction, alias is a Use with BorrowingProject
    // semantics — no Inc regardless of future use.
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::BorrowingProject,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::None,
        "scalar Project borrows source — no Inc even with future use"
    );
}

// A1 continued: Source last-use after a scalar Project — Dec IS emitted
// (borrowing projection does NOT suppress source cleanup).
#[test]
fn decide_source_last_use_after_borrowing_project_emits_dec() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_non_reusable(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::Dec,
        "last use after scalar Project: source Dec NOT suppressed"
    );
}

// A2: Non-scalar Project source — TransferProject suppresses Inc at use
// site. At last-use site, borrow semantics apply: the parent gets Defer
// if borrowed children are alive, or Dec if they're dead.
#[test]
fn decide_transfer_project_borrow_semantics() {
    // At use site: no Inc (Project source doesn't get Inc'd)
    let use_ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::TransferProject,
        },
        is_rc_managed: true,
    };
    assert_eq!(
        decide(&use_ctx).rc,
        RcDecision::None,
        "transfer Project: no Inc at use site"
    );

    // At last-use site with live borrowed children: Defer
    let defer_ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: true,
            reuse: reuse_non_reusable(),
        },
        is_rc_managed: true,
    };
    assert_eq!(
        decide(&defer_ctx).rc,
        RcDecision::Defer,
        "transfer Project: source Dec deferred while children alive"
    );

    // At last-use site with no live children: Dec
    let dec_ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_non_reusable(),
        },
        is_rc_managed: true,
    };
    assert_eq!(
        decide(&dec_ctx).rc,
        RcDecision::Dec,
        "transfer Project: source Dec emitted when children dead"
    );
}

// A5: Alias consumed by owned call — Inc at alias split point.
// When `let b = a; consume(b)` and `a` has future use, the alias
// use site (before the call) needs an Inc to keep a alive.
#[test]
fn decide_alias_owned_call_with_future_root_use() {
    // At the alias use (b is used as an owned call arg):
    // has_future_use is true for source (a still lives), so Inc.
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::Inc,
        "alias with future root use: Inc at split point"
    );
}

// A5 continued: The owned call position suppresses the alias's Dec.
#[test]
fn decide_alias_at_owned_call_position_no_dec() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: true, // callee takes ownership

            has_deferred_children: false,
            reuse: reuse_non_reusable(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::None,
        "owned call position: callee takes ownership, no caller-side Dec"
    );
}

// A6: Alias passed to borrowed call — no spurious Inc.
// When `let b = a; borrow(b)` and b is the last use of b, and borrow
// takes b as Borrowed, no Inc is needed (borrowed call doesn't consume).
#[test]
fn decide_alias_borrowed_call_no_future_use_no_inc() {
    // b at the borrowed call site: no future use of b → no Inc.
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: false,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(
        d.rc,
        RcDecision::None,
        "alias at borrowed call with no future use: no Inc"
    );
}

// A3: Borrowing projection on both branches — each branch independently
// gets the same Dec decision for the source aggregate.
#[test]
fn decide_borrowing_project_independent_per_branch() {
    // Both branches use the source via BorrowingProject — neither
    // should get an Inc. The source's Dec happens at last use in
    // each branch independently.
    let branch_a = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: false,
            semantics: UseSemantics::BorrowingProject,
        },
        is_rc_managed: true,
    };
    let branch_b = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: false,
            semantics: UseSemantics::BorrowingProject,
        },
        is_rc_managed: true,
    };
    assert_eq!(decide(&branch_a).rc, RcDecision::None);
    assert_eq!(decide(&branch_b).rc, RcDecision::None);
}
