use super::*;
use crate::ir::ArcVarId;

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
