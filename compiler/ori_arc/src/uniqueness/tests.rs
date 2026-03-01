use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use super::*;
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};
use crate::liveness::compute_liveness;
use crate::test_helpers::{make_func, owned_param, v};
use crate::{ArcClass, ArcClassification};

// -- Uniqueness lattice properties --

#[test]
fn join_same_state_is_idempotent() {
    assert_eq!(
        Uniqueness::Unique.join(Uniqueness::Unique),
        Uniqueness::Unique
    );
    assert_eq!(
        Uniqueness::MaybeShared.join(Uniqueness::MaybeShared),
        Uniqueness::MaybeShared
    );
    assert_eq!(
        Uniqueness::Shared.join(Uniqueness::Shared),
        Uniqueness::Shared
    );
}

#[test]
fn join_is_commutative() {
    let pairs = [
        (Uniqueness::Unique, Uniqueness::Shared),
        (Uniqueness::Unique, Uniqueness::MaybeShared),
        (Uniqueness::Shared, Uniqueness::MaybeShared),
    ];
    for (a, b) in pairs {
        assert_eq!(
            a.join(b),
            b.join(a),
            "join not commutative for {a:?} and {b:?}"
        );
    }
}

#[test]
fn join_is_associative() {
    let states = [
        Uniqueness::Unique,
        Uniqueness::MaybeShared,
        Uniqueness::Shared,
    ];
    for &a in &states {
        for &b in &states {
            for &c in &states {
                assert_eq!(
                    a.join(b).join(c),
                    a.join(b.join(c)),
                    "join not associative for {a:?}, {b:?}, {c:?}"
                );
            }
        }
    }
}

#[test]
fn join_different_states_gives_maybe_shared() {
    assert_eq!(
        Uniqueness::Unique.join(Uniqueness::Shared),
        Uniqueness::MaybeShared
    );
    assert_eq!(
        Uniqueness::Unique.join(Uniqueness::MaybeShared),
        Uniqueness::MaybeShared
    );
    assert_eq!(
        Uniqueness::Shared.join(Uniqueness::MaybeShared),
        Uniqueness::MaybeShared
    );
}

// -- MaybeShared is top (⊤) of the join-semilattice --

#[test]
fn maybe_shared_is_top() {
    // join(x, MaybeShared) = MaybeShared for all x
    let states = [
        Uniqueness::Unique,
        Uniqueness::MaybeShared,
        Uniqueness::Shared,
    ];
    for &s in &states {
        assert_eq!(
            s.join(Uniqueness::MaybeShared),
            Uniqueness::MaybeShared,
            "MaybeShared should be ⊤: {s:?} ⊔ MaybeShared = MaybeShared"
        );
    }
}

// -- Predicates --

#[test]
fn predicate_methods() {
    assert!(Uniqueness::Unique.is_unique());
    assert!(!Uniqueness::Unique.is_maybe_shared());
    assert!(!Uniqueness::Unique.is_shared());

    assert!(!Uniqueness::MaybeShared.is_unique());
    assert!(Uniqueness::MaybeShared.is_maybe_shared());
    assert!(!Uniqueness::MaybeShared.is_shared());

    assert!(!Uniqueness::Shared.is_unique());
    assert!(!Uniqueness::Shared.is_maybe_shared());
    assert!(Uniqueness::Shared.is_shared());
}

// -- Display --

#[test]
fn display_formatting() {
    assert_eq!(format!("{}", Uniqueness::Unique), "unique");
    assert_eq!(format!("{}", Uniqueness::MaybeShared), "maybe_shared");
    assert_eq!(format!("{}", Uniqueness::Shared), "shared");
}

// -- CowMode --

#[test]
fn cow_mode_from_uniqueness() {
    assert_eq!(
        CowMode::from_uniqueness(Uniqueness::Unique),
        CowMode::StaticUnique
    );
    assert_eq!(
        CowMode::from_uniqueness(Uniqueness::MaybeShared),
        CowMode::Dynamic
    );
    assert_eq!(
        CowMode::from_uniqueness(Uniqueness::Shared),
        CowMode::StaticShared
    );
}

#[test]
fn cow_mode_display() {
    assert_eq!(format!("{}", CowMode::Dynamic), "dynamic");
    assert_eq!(format!("{}", CowMode::StaticUnique), "static_unique");
    assert_eq!(format!("{}", CowMode::StaticShared), "static_shared");
}

// -- UniquenessMap --

#[test]
fn new_map_defaults_to_maybe_shared() {
    let map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);
    assert_eq!(map.get(v0), Uniqueness::MaybeShared);
}

#[test]
fn set_and_get() {
    let mut map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    map.set(v0, Uniqueness::Unique);
    map.set(v1, Uniqueness::Shared);

    assert_eq!(map.get(v0), Uniqueness::Unique);
    assert_eq!(map.get(v1), Uniqueness::Shared);
}

#[test]
fn mark_unique_and_mark_shared() {
    let mut map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    map.mark_unique(v0);
    map.mark_shared(v1);

    assert_eq!(map.get(v0), Uniqueness::Unique);
    assert_eq!(map.get(v1), Uniqueness::Shared);
}

#[test]
fn join_elevates_to_maybe_shared() {
    let mut map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);

    map.set(v0, Uniqueness::Unique);
    map.join(v0, Uniqueness::Shared);

    assert_eq!(map.get(v0), Uniqueness::MaybeShared);
}

#[test]
fn join_same_state_preserves() {
    let mut map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);

    map.set(v0, Uniqueness::Unique);
    map.join(v0, Uniqueness::Unique);

    assert_eq!(map.get(v0), Uniqueness::Unique);
}

#[test]
fn join_untracked_var_uses_maybe_shared_as_base() {
    let mut map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);

    // v0 is not tracked → implicit MaybeShared
    map.join(v0, Uniqueness::Unique);
    // MaybeShared ⊔ Unique = MaybeShared
    assert_eq!(map.get(v0), Uniqueness::MaybeShared);
}

#[test]
fn join_from_merges_maps() {
    let mut map_a = UniquenessMap::new();
    let mut map_b = UniquenessMap::new();
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);

    // map_a: v0=Unique, v1=Shared
    map_a.set(v0, Uniqueness::Unique);
    map_a.set(v1, Uniqueness::Shared);

    // map_b: v0=Shared, v2=Unique
    map_b.set(v0, Uniqueness::Shared);
    map_b.set(v2, Uniqueness::Unique);

    map_a.join_from(&map_b);

    // v0: Unique ⊔ Shared = MaybeShared
    assert_eq!(map_a.get(v0), Uniqueness::MaybeShared);
    // v1: Shared (unchanged, v1 not in map_b → joined with implicit MaybeShared)
    assert_eq!(map_a.get(v1), Uniqueness::Shared);
    // v2: MaybeShared ⊔ Unique = MaybeShared (implicit in map_a)
    assert_eq!(map_a.get(v2), Uniqueness::MaybeShared);
}

#[test]
fn cow_mode_from_map() {
    let mut map = UniquenessMap::new();
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);

    map.set(v0, Uniqueness::Unique);
    map.set(v1, Uniqueness::Shared);
    // v2 not tracked → MaybeShared

    assert_eq!(map.cow_mode(v0), CowMode::StaticUnique);
    assert_eq!(map.cow_mode(v1), CowMode::StaticShared);
    assert_eq!(map.cow_mode(v2), CowMode::Dynamic);
}

#[test]
fn len_and_is_empty() {
    let mut map = UniquenessMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);

    map.set(ArcVarId::new(0), Uniqueness::Unique);
    assert!(!map.is_empty());
    assert_eq!(map.len(), 1);

    map.set(ArcVarId::new(1), Uniqueness::Shared);
    assert_eq!(map.len(), 2);
}

#[test]
fn with_capacity_works() {
    let map = UniquenessMap::with_capacity(16);
    assert!(map.is_empty());
}

#[test]
fn iter_yields_all_tracked_vars() {
    let mut map = UniquenessMap::new();
    map.set(ArcVarId::new(0), Uniqueness::Unique);
    map.set(ArcVarId::new(1), Uniqueness::Shared);
    map.set(ArcVarId::new(2), Uniqueness::MaybeShared);

    let mut items: Vec<_> = map.iter().collect();
    items.sort_by_key(|(var, _)| var.raw());

    assert_eq!(items.len(), 3);
    assert_eq!(items[0], (ArcVarId::new(0), Uniqueness::Unique));
    assert_eq!(items[1], (ArcVarId::new(1), Uniqueness::Shared));
    assert_eq!(items[2], (ArcVarId::new(2), Uniqueness::MaybeShared));
}

// -- compute_cow_annotations tests --

/// Test classifier: `Idx::STR` (index 3) is `DefiniteRef`, everything else Scalar.
struct TestClassifier;

impl ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if idx == Idx::STR {
            ArcClass::DefiniteRef
        } else {
            ArcClass::Scalar
        }
    }
}

/// Type shorthand: RC'd type (str).
const RC: Idx = Idx::STR;

/// COW method name (interned as `Name`).
const PUSH_NAME: Name = Name::from_raw(42);

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn construct(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: v(dst),
        ty: RC,
        ctor: CtorKind::ListLiteral,
        args: Vec::new(),
    }
}

fn cow_apply(dst: u32, receiver: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: v(dst),
        ty: RC,
        func: PUSH_NAME,
        args: vec![v(receiver)],
        arg_ownership: Vec::new(),
    }
}

fn ret(var: u32) -> ArcTerminator {
    ArcTerminator::Return { value: v(var) }
}

/// Create an Invoke terminator calling a COW method (like push).
/// `dst` receives the result, `receiver` is the COW target.
fn cow_invoke(dst: u32, receiver: u32, normal: u32, unwind: u32) -> ArcTerminator {
    ArcTerminator::Invoke {
        dst: v(dst),
        ty: RC,
        func: PUSH_NAME,
        args: vec![v(receiver)],
        arg_ownership: Vec::new(),
        normal: ArcBlockId::new(normal),
        unwind: ArcBlockId::new(unwind),
    }
}

fn cow_names() -> FxHashSet<Name> {
    let mut s = FxHashSet::default();
    s.insert(PUSH_NAME);
    s
}

fn cow_summaries() -> FxHashMap<Name, UniquenessSummary> {
    let mut m = FxHashMap::default();
    m.insert(
        PUSH_NAME,
        UniquenessSummary {
            params: Vec::new(),
            return_val: Uniqueness::Unique,
            preserves_freshness: true,
        },
    );
    m
}

#[test]
fn fresh_list_push_is_static_unique() {
    // v0 = Construct([])   → Unique
    // v1 = Apply(push, v0) → push consumes unique list, returns unique
    // Return(v1)
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![construct(0), cow_apply(1, 0)], ret(1))],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    // The push at (block 0, instr 1) should be StaticUnique because v0 is
    // a freshly constructed list (Unique).
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations.get(0, 1), CowMode::StaticUnique);
}

#[test]
fn param_list_push_is_dynamic() {
    // v0 = param (list)     → MaybeShared (can't prove unique for params)
    // v1 = Apply(push, v0)  → dynamic check needed
    // Return(v1)
    let func = make_func(
        vec![owned_param(0, RC)],
        RC,
        vec![block(0, vec![cow_apply(1, 0)], ret(1))],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    // The push at (block 0, instr 0) should be Dynamic because v0 is a
    // parameter with unknown uniqueness.
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations.get(0, 0), CowMode::Dynamic);
}

#[test]
fn chained_pushes_on_fresh_list_all_static_unique() {
    // v0 = Construct([])     → Unique
    // v1 = Apply(push, v0)   → StaticUnique (v0 is unique)
    // v2 = Apply(push, v1)   → StaticUnique (push returns unique via summary)
    // Return(v2)
    let func = make_func(
        vec![],
        RC,
        vec![block(
            0,
            vec![construct(0), cow_apply(1, 0), cow_apply(2, 1)],
            ret(2),
        )],
        vec![RC, RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    // Both pushes should be StaticUnique: first because v0 is fresh,
    // second because push's summary says return is Unique.
    assert_eq!(annotations.len(), 2);
    assert_eq!(annotations.get(0, 1), CowMode::StaticUnique);
    assert_eq!(annotations.get(0, 2), CowMode::StaticUnique);
    assert_eq!(annotations.static_unique_count(), 2);
}

#[test]
fn shared_list_push_is_dynamic() {
    // v0 = Construct([])    → Unique
    // v1 = Let(v0)          → v0 becomes Shared (aliased)
    // v2 = Apply(push, v0)  → Dynamic (v0 was aliased)
    // Return(v2)
    let func = make_func(
        vec![],
        RC,
        vec![block(
            0,
            vec![
                construct(0),
                ArcInstr::Let {
                    dst: v(1),
                    ty: RC,
                    value: ArcValue::Var(v(0)),
                },
                cow_apply(2, 0),
            ],
            ret(2),
        )],
        vec![RC, RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    // The push at (block 0, instr 2) should NOT be StaticUnique because v0
    // was aliased by v1 = Let(v0).
    assert_eq!(annotations.len(), 1);
    assert_ne!(annotations.get(0, 2), CowMode::StaticUnique);
}

#[test]
fn unannotated_instruction_defaults_to_dynamic() {
    let annotations = CowAnnotations::new();
    // Any instruction not in the map defaults to Dynamic.
    assert_eq!(annotations.get(0, 0), CowMode::Dynamic);
    assert_eq!(annotations.get(99, 99), CowMode::Dynamic);
}

#[test]
fn cow_annotations_metrics() {
    // 3 pushes: 2 on fresh values (StaticUnique), 1 on param (Dynamic)
    // v0 = param
    // v1 = Construct([])
    // v2 = Apply(push, v0)  → Dynamic
    // v3 = Apply(push, v1)  → StaticUnique
    // v4 = Apply(push, v3)  → StaticUnique (push returns unique)
    // Return(v4)
    let func = make_func(
        vec![owned_param(0, RC)],
        RC,
        vec![block(
            0,
            vec![
                construct(1),
                cow_apply(2, 0),
                cow_apply(3, 1),
                cow_apply(4, 3),
            ],
            ret(4),
        )],
        vec![RC, RC, RC, RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations.static_unique_count(), 2);
    assert_eq!(annotations.static_shared_count(), 0);
}

#[test]
fn invoke_terminator_cow_is_annotated() {
    // COW calls emitted as Invoke terminators (unwindable) must also be
    // annotated — not just body Apply instructions.
    //
    // bb0:
    //   v0 = Construct([])       → Unique
    //   Invoke push(v0) → bb1 / bb2   (terminator, not body instruction)
    // bb1:
    //   Return(v1)
    // bb2:
    //   Resume
    let func = make_func(
        vec![],
        RC,
        vec![
            block(0, vec![construct(0)], cow_invoke(1, 0, 1, 2)),
            block(1, vec![], ret(1)),
            block(2, vec![], ArcTerminator::Resume),
        ],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    // The Invoke at (block 0, body.len() = 1) should be StaticUnique because
    // v0 is a freshly constructed list.
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations.get(0, 1), CowMode::StaticUnique);
}

#[test]
fn invoke_terminator_param_cow_is_dynamic() {
    // Same as above but receiver is a parameter — should be Dynamic.
    //
    // bb0:
    //   v0 = param
    //   Invoke push(v0) → bb1 / bb2
    // bb1:
    //   Return(v1)
    // bb2:
    //   Resume
    let func = make_func(
        vec![owned_param(0, RC)],
        RC,
        vec![
            block(0, vec![], cow_invoke(1, 0, 1, 2)),
            block(1, vec![], ret(1)),
            block(2, vec![], ArcTerminator::Resume),
        ],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let summaries = cow_summaries();
    let cow_method_names = cow_names();

    let annotations =
        compute_cow_annotations(&func, &classifier, &liveness, &summaries, &cow_method_names);

    // The Invoke at (block 0, body.len() = 0) should be Dynamic because
    // v0 is a parameter with unknown uniqueness.
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations.get(0, 0), CowMode::Dynamic);
}
