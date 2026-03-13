//! Tests for the unified realization module.

use super::decide::{
    decide, decide_annotations, decide_cow, decide_drop_hint, AnnotationSiteContext,
    DecisionContext, DecisionSite, InstructionDecisions, RcDecision, ReuseContext, ReuseDecision,
};
use crate::aims::lattice::{Cardinality, ReuseCtorKind, ShapeClass, Uniqueness};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;
use rustc_hash::FxHashSet;

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

// decide_cow tests

#[test]
fn cow_unique_no_rc_inc_is_static_unique() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_unique_with_rc_inc_is_dynamic() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: true,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_maybe_shared_is_dynamic() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::MaybeShared,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

// decide_drop_hint tests

#[test]
fn drop_hint_unique_clean_is_eligible() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    assert!(decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_unique_with_rc_inc_not_eligible() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: true,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_unique_borrowed_arg_not_eligible() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: true,
        rc_incremented_set: &FxHashSet::default(),
    };
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_maybe_shared_not_eligible() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::MaybeShared,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
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
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    let result = decide_annotations(&ctx, true, false);
    assert_eq!(result.cow, Some(CowMode::StaticUnique));
    assert!(!result.drop_hint);
}

#[test]
fn annotations_drop_site_unique_clean() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    let result = decide_annotations(&ctx, false, true);
    assert_eq!(result.cow, None);
    assert!(result.drop_hint);
}

#[test]
fn annotations_both_cow_and_drop() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    let result = decide_annotations(&ctx, true, true);
    assert_eq!(result.cow, Some(CowMode::StaticUnique));
    assert!(result.drop_hint);
}

#[test]
fn annotations_neither_cow_nor_drop() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Unique,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    let result = decide_annotations(&ctx, false, false);
    assert_eq!(result.cow, None);
    assert!(!result.drop_hint);
}

#[test]
fn annotations_shared_cow_site() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::Shared,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
    let result = decide_annotations(&ctx, true, false);
    assert_eq!(result.cow, Some(CowMode::StaticShared));
    assert!(!result.drop_hint);
}

#[test]
fn annotations_maybe_shared_drop_site_not_eligible() {
    let ctx = AnnotationSiteContext {
        var: var(0),
        uniqueness: Uniqueness::MaybeShared,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: &FxHashSet::default(),
    };
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
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
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
