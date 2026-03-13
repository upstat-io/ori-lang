//! Tests for the unified realization module.

use super::decide::{
    decide_annotations, decide_cow, decide_drop_hint, AnnotationSiteContext, InstructionDecisions,
    RcDecision, ReuseDecision,
};
use crate::aims::lattice::Uniqueness;
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
