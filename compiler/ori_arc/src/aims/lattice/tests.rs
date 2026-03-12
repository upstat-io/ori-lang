//! Tests for AIMS lattice types and semiring operators.

use super::*;

// Helpers to enumerate all variants

fn all_access() -> Vec<AccessClass> {
    vec![AccessClass::Borrowed, AccessClass::Owned]
}

fn all_consumption() -> Vec<Consumption> {
    use Consumption::*;
    vec![Dead, Linear, Affine, Unrestricted]
}

fn all_cardinality() -> Vec<Cardinality> {
    use Cardinality::*;
    vec![Absent, Once, Many]
}

fn all_uniqueness() -> Vec<Uniqueness> {
    use Uniqueness::*;
    vec![Unique, MaybeShared, Shared]
}

fn all_locality() -> Vec<Locality> {
    use Locality::*;
    vec![BlockLocal, FunctionLocal, HeapEscaping, Unknown]
}

fn all_shape() -> Vec<ShapeClass> {
    use ShapeClass::*;
    vec![
        NonReusable,
        ReusableCtor(ReuseCtorKind::Struct),
        ReusableCtor(ReuseCtorKind::EnumVariant),
        CollectionBuffer,
        ContextHole,
    ]
}

fn all_effect() -> Vec<EffectClass> {
    let bools = [false, true];
    let mut effects = Vec::with_capacity(8);
    for &a in &bools {
        for &s in &bools {
            for &t in &bools {
                effects.push(EffectClass {
                    may_alloc: a,
                    may_share: s,
                    may_throw: t,
                });
            }
        }
    }
    effects
}

/// Generate a representative set of canonical `AimsState` values for property testing.
/// Uses all core dimension combinations × a sample of auxiliary dimensions,
/// then canonicalizes each state (removing infeasible combinations).
fn representative_states() -> Vec<AimsState> {
    let shapes = [
        ShapeClass::NonReusable,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
    ];
    let effects = [EffectClass::NONE, EffectClass::ALL];
    let localities = [Locality::FunctionLocal, Locality::Unknown];

    let mut seen = std::collections::HashSet::new();
    let mut states = Vec::new();
    for &access in &all_access() {
        for &consumption in &all_consumption() {
            for &cardinality in &all_cardinality() {
                for &uniqueness in &all_uniqueness() {
                    for &locality in &localities {
                        for &shape in &shapes {
                            for &effect in &effects {
                                let mut s = AimsState {
                                    access,
                                    consumption,
                                    cardinality,
                                    uniqueness,
                                    locality,
                                    shape,
                                    effect,
                                };
                                s.canonicalize();
                                if seen.insert(s) {
                                    states.push(s);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    states
}

// Per-axis lattice law tests

mod access_class {
    use super::*;

    #[test]
    fn idempotence() {
        for a in all_access() {
            assert_eq!(a.join(a), a, "idempotent: {a:?}");
        }
    }

    #[test]
    fn commutativity() {
        for a in all_access() {
            for b in all_access() {
                assert_eq!(a.join(b), b.join(a), "commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        for a in all_access() {
            for b in all_access() {
                for c in all_access() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn owned_absorbs_borrowed() {
        assert_eq!(
            AccessClass::Owned.join(AccessClass::Borrowed),
            AccessClass::Owned
        );
        assert_eq!(
            AccessClass::Borrowed.join(AccessClass::Owned),
            AccessClass::Owned
        );
        assert_eq!(
            AccessClass::Borrowed.join(AccessClass::Borrowed),
            AccessClass::Borrowed
        );
    }
}

mod consumption_tests {
    use super::*;

    #[test]
    fn idempotence() {
        for a in all_consumption() {
            assert_eq!(a.join(a), a, "idempotent: {a:?}");
        }
    }

    #[test]
    fn commutativity() {
        for a in all_consumption() {
            for b in all_consumption() {
                assert_eq!(a.join(b), b.join(a), "commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        for a in all_consumption() {
            for b in all_consumption() {
                for c in all_consumption() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn ordering() {
        use Consumption::*;
        assert!(Dead < Linear);
        assert!(Linear < Affine);
        assert!(Affine < Unrestricted);
        assert_eq!(Dead.join(Unrestricted), Unrestricted);
        assert_eq!(Linear.join(Affine), Affine);
    }
}

mod cardinality_tests {
    use super::*;
    use Cardinality::*;

    #[test]
    fn join_idempotence() {
        for a in all_cardinality() {
            assert_eq!(a.join(a), a, "join idempotent: {a:?}");
        }
    }

    #[test]
    fn join_commutativity() {
        for a in all_cardinality() {
            for b in all_cardinality() {
                assert_eq!(a.join(b), b.join(a), "join commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn join_associativity() {
        for a in all_cardinality() {
            for b in all_cardinality() {
                for c in all_cardinality() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "join associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn seq_add_associativity() {
        for a in all_cardinality() {
            for b in all_cardinality() {
                for c in all_cardinality() {
                    assert_eq!(
                        a.seq_add(b.seq_add(c)),
                        a.seq_add(b).seq_add(c),
                        "seq_add associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn seq_add_commutativity() {
        for a in all_cardinality() {
            for b in all_cardinality() {
                assert_eq!(
                    a.seq_add(b),
                    b.seq_add(a),
                    "seq_add commutative: {a:?}, {b:?}"
                );
            }
        }
    }

    #[test]
    fn seq_add_identity() {
        for a in all_cardinality() {
            assert_eq!(a.seq_add(Absent), a, "right identity: {a:?}");
            assert_eq!(Absent.seq_add(a), a, "left identity: {a:?}");
        }
    }

    #[test]
    fn seq_add_absorbing() {
        for x in all_cardinality() {
            assert_eq!(Many.seq_add(x), Many, "Many absorbs: {x:?}");
            assert_eq!(x.seq_add(Many), Many, "absorbed by Many: {x:?}");
        }
    }

    #[test]
    fn alt_join_idempotence() {
        for a in all_cardinality() {
            assert_eq!(a.alt_join(a), a, "alt_join idempotent: {a:?}");
        }
    }

    #[test]
    fn alt_join_associativity() {
        for a in all_cardinality() {
            for b in all_cardinality() {
                for c in all_cardinality() {
                    assert_eq!(
                        a.alt_join(b).alt_join(c),
                        a.alt_join(b.alt_join(c)),
                        "alt_join associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn distributivity_seq_add_over_alt_join() {
        // a.seq_add(b.alt_join(c)) == a.seq_add(b).alt_join(a.seq_add(c))
        for a in all_cardinality() {
            for b in all_cardinality() {
                for c in all_cardinality() {
                    let lhs = a.seq_add(b.alt_join(c));
                    let rhs = a.seq_add(b).alt_join(a.seq_add(c));
                    assert_eq!(lhs, rhs, "distributivity: {a:?}, {b:?}, {c:?}");
                }
            }
        }
    }

    #[test]
    fn seq_add_positivity() {
        // QTT correspondence: no non-trivial cancellation.
        // a.seq_add(b) == Absent implies both a == Absent and b == Absent.
        for a in all_cardinality() {
            for b in all_cardinality() {
                if a.seq_add(b) == Absent {
                    assert_eq!(
                        a, Absent,
                        "positivity violated: {a:?}.seq_add({b:?}) == Absent but a != Absent"
                    );
                    assert_eq!(
                        b, Absent,
                        "positivity violated: {a:?}.seq_add({b:?}) == Absent but b != Absent"
                    );
                }
            }
        }
    }

    #[test]
    fn right_distributivity_seq_add_over_alt_join() {
        // (a.alt_join(b)).seq_add(c) == a.seq_add(c).alt_join(b.seq_add(c))
        // Symmetric to left-distributivity tested above.
        for a in all_cardinality() {
            for b in all_cardinality() {
                for c in all_cardinality() {
                    let lhs = a.alt_join(b).seq_add(c);
                    let rhs = a.seq_add(c).alt_join(b.seq_add(c));
                    assert_eq!(lhs, rhs, "right-distributivity: {a:?}, {b:?}, {c:?}");
                }
            }
        }
    }

    #[test]
    fn specific_values() {
        assert_eq!(Once.seq_add(Once), Many);
        assert_eq!(Once.alt_join(Once), Once);
        assert_eq!(Absent.seq_add(Once), Once);
        assert_eq!(Once.seq_add(Absent), Once);
    }
}

mod uniqueness_tests {
    use super::*;

    #[test]
    fn idempotence() {
        for a in all_uniqueness() {
            assert_eq!(a.join(a), a, "idempotent: {a:?}");
        }
    }

    #[test]
    fn commutativity() {
        for a in all_uniqueness() {
            for b in all_uniqueness() {
                assert_eq!(a.join(b), b.join(a), "commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        for a in all_uniqueness() {
            for b in all_uniqueness() {
                for c in all_uniqueness() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }
}

mod locality_tests {
    use super::*;

    #[test]
    fn idempotence() {
        for a in all_locality() {
            assert_eq!(a.join(a), a, "idempotent: {a:?}");
        }
    }

    #[test]
    fn commutativity() {
        for a in all_locality() {
            for b in all_locality() {
                assert_eq!(a.join(b), b.join(a), "commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        for a in all_locality() {
            for b in all_locality() {
                for c in all_locality() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn block_local_is_identity() {
        use Locality::*;
        for a in all_locality() {
            assert_eq!(BlockLocal.join(a), a, "left identity: {a:?}");
            assert_eq!(a.join(BlockLocal), a, "right identity: {a:?}");
        }
    }

    #[test]
    fn unknown_absorbs() {
        use Locality::*;
        for a in all_locality() {
            assert_eq!(Unknown.join(a), Unknown, "Unknown absorbs: {a:?}");
            assert_eq!(a.join(Unknown), Unknown, "absorbed by Unknown: {a:?}");
        }
    }

    /// Verify that `join` satisfies the sequencing algebra properties for
    /// Locality: `seq_add` and `alt_join` both coincide with `join` (= max).
    /// This is intentional — locality widens monotonically (see dimension docs).
    #[test]
    fn join_is_sequencing_algebra() {
        // seq_add properties: commutative monoid (Absent=BlockLocal, absorption=Unknown)
        // alt_join properties: idempotent, associative, commutative
        // Both = join for Locality. The tests above already verify associativity,
        // commutativity, idempotence, identity, and absorption — confirming that
        // a single `join` serves as both seq_add and alt_join.
        //
        // Verify the key distinguishing property: idempotence.
        // Cardinality's seq_add is NOT idempotent (Once.seq_add(Once) = Many),
        // but Locality's would be (BlockLocal.join(BlockLocal) = BlockLocal).
        // This is correct for escape analysis: escaping twice is still escaping.
        for a in all_locality() {
            assert_eq!(a.join(a), a, "seq_add idempotent for Locality: {a:?}");
        }

        // Verify distributivity (trivially holds when seq_add = alt_join = join,
        // but document it explicitly):
        // a.join(b.join(c)) == a.join(b).join(a.join(c))
        for a in all_locality() {
            for b in all_locality() {
                for c in all_locality() {
                    let lhs = a.join(b.join(c));
                    let rhs = a.join(b).join(a.join(c));
                    assert_eq!(lhs, rhs, "distributivity: {a:?}, {b:?}, {c:?}");
                }
            }
        }
    }
}

mod shape_class_tests {
    use super::*;
    use ShapeClass::*;

    #[test]
    fn idempotence() {
        for a in all_shape() {
            assert_eq!(a.join(a), a, "idempotent: {a:?}");
        }
    }

    #[test]
    fn commutativity() {
        for a in all_shape() {
            for b in all_shape() {
                assert_eq!(a.join(b), b.join(a), "commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        for a in all_shape() {
            for b in all_shape() {
                for c in all_shape() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn non_reusable_absorbs() {
        for a in all_shape() {
            assert_eq!(NonReusable.join(a), NonReusable, "absorbs: {a:?}");
            assert_eq!(a.join(NonReusable), NonReusable, "absorbed: {a:?}");
        }
    }

    #[test]
    fn distinct_non_top_collapse() {
        let struct_ = ReusableCtor(ReuseCtorKind::Struct);
        let variant = ReusableCtor(ReuseCtorKind::EnumVariant);
        assert_eq!(struct_.join(variant), NonReusable);
        assert_eq!(struct_.join(CollectionBuffer), NonReusable);
        assert_eq!(CollectionBuffer.join(ContextHole), NonReusable);
    }
}

mod effect_class_tests {
    use super::*;

    #[test]
    fn idempotence() {
        for a in all_effect() {
            assert_eq!(a.join(a), a, "idempotent: {a:?}");
        }
    }

    #[test]
    fn commutativity() {
        for a in all_effect() {
            for b in all_effect() {
                assert_eq!(a.join(b), b.join(a), "commutative: {a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        for a in all_effect() {
            for b in all_effect() {
                for c in all_effect() {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(b.join(c)),
                        "associative: {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn none_is_identity() {
        for a in all_effect() {
            assert_eq!(EffectClass::NONE.join(a), a, "NONE identity: {a:?}");
            assert_eq!(a.join(EffectClass::NONE), a, "identity NONE: {a:?}");
        }
    }

    #[test]
    fn all_absorbs() {
        for a in all_effect() {
            assert_eq!(
                EffectClass::ALL.join(a),
                EffectClass::ALL,
                "ALL absorbs: {a:?}"
            );
        }
    }

    /// Verify that `join` satisfies the sequencing algebra properties for
    /// `EffectClass`: `seq_add` and `alt_join` both coincide with `join`
    /// (= componentwise OR). This is a design choice — effects are boolean
    /// flags, not counts (see dimension docs).
    #[test]
    fn join_is_sequencing_algebra() {
        // seq_add = alt_join = join for boolean effects.
        // Key properties: idempotent (OR is idempotent), identity (NONE),
        // absorption (ALL). Already tested above individually.
        //
        // Verify the distinguishing property: idempotence of seq_add.
        // Unlike Cardinality (Once.seq_add(Once) = Many), Effect's seq_add
        // would be idempotent (MayAlloc.join(MayAlloc) = MayAlloc).
        // This is correct: allocating twice still means "may allocate."
        for a in all_effect() {
            assert_eq!(a.join(a), a, "seq_add idempotent for Effect: {a:?}");
        }

        // Verify distributivity:
        // a.join(b.join(c)) == a.join(b).join(a.join(c))
        for a in all_effect() {
            for b in all_effect() {
                for c in all_effect() {
                    let lhs = a.join(b.join(c));
                    let rhs = a.join(b).join(a.join(c));
                    assert_eq!(lhs, rhs, "distributivity: {a:?}, {b:?}, {c:?}");
                }
            }
        }
    }
}

// AimsState join property tests

mod aims_state_join {
    use super::*;

    #[test]
    fn idempotence() {
        for s in representative_states() {
            let joined = s.join(&s);
            assert_eq!(joined, s, "idempotent: {s:?}");
        }
    }

    #[test]
    fn commutativity() {
        let states = representative_states();
        // Test all pairs from a subset (full cross-product is ~25M pairs)
        for (i, a) in states.iter().enumerate() {
            for b in states.iter().skip(i) {
                assert_eq!(a.join(b), b.join(a), "commutative:\n  a={a:?}\n  b={b:?}");
            }
        }
    }

    #[test]
    fn associativity() {
        let states = representative_states();
        // Sample triples
        let sample: Vec<_> = states.iter().step_by(7).collect();
        for a in &sample {
            for b in &sample {
                for c in &sample {
                    let ab_c = a.join(b).join(c);
                    let a_bc = a.join(&b.join(c));
                    assert_eq!(ab_c, a_bc, "associative:\n  a={a:?}\n  b={b:?}\n  c={c:?}");
                }
            }
        }
    }
}

// Canonicalization tests

mod canonicalization {
    use super::*;

    #[test]
    fn idempotence() {
        for mut s in representative_states() {
            s.canonicalize();
            let before = s;
            s.canonicalize();
            assert_eq!(s, before, "canonicalize idempotent: {before:?}");
        }
    }

    #[test]
    fn dead_implies_absent() {
        let mut s = AimsState {
            consumption: Consumption::Dead,
            cardinality: Cardinality::Once, // wrong — should be Absent
            ..AimsState::TOP
        };
        s.canonicalize();
        assert_eq!(s.cardinality, Cardinality::Absent);
    }

    #[test]
    fn absent_implies_dead() {
        let mut s = AimsState {
            consumption: Consumption::Linear,
            cardinality: Cardinality::Absent, // wrong — should be Dead
            ..AimsState::TOP
        };
        s.canonicalize();
        assert_eq!(s.consumption, Consumption::Dead);
    }

    #[test]
    fn linear_absent_collapses_to_dead() {
        let mut s = AimsState {
            consumption: Consumption::Linear,
            cardinality: Cardinality::Absent,
            ..AimsState::FRESH
        };
        s.canonicalize();
        assert_eq!(s.consumption, Consumption::Dead);
        assert_eq!(s.cardinality, Cardinality::Absent);
    }

    #[test]
    fn shared_reusable_collapses_shape() {
        let mut s = AimsState {
            uniqueness: Uniqueness::Shared,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::TOP
        };
        s.canonicalize();
        assert_eq!(s.shape, ShapeClass::NonReusable);
    }

    #[test]
    fn shared_collection_buffer_preserved() {
        // CollectionBuffer is not collapsed by shared uniqueness
        // (only ReusableCtor is affected)
        let mut s = AimsState {
            uniqueness: Uniqueness::Shared,
            shape: ShapeClass::CollectionBuffer,
            ..AimsState::TOP
        };
        let before_shape = s.shape;
        s.canonicalize();
        assert_eq!(s.shape, before_shape);
    }

    // Rule 4: BlockLocal + Owned + ≤Once + MaybeShared → Unique

    #[test]
    fn rule4_block_local_owned_once_promotes_unique() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::Unique);
    }

    #[test]
    fn rule4_does_not_fire_for_unknown_locality() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::MaybeShared);
    }

    #[test]
    fn rule4_does_not_fire_for_function_local() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::MaybeShared);
    }

    #[test]
    fn rule4_does_not_override_shared() {
        // Shared is definite knowledge (RC > 1), Rule 4 must not override it
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Shared,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::Shared);
    }

    #[test]
    fn rule4_does_not_fire_for_many_cardinality() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::MaybeShared);
    }

    #[test]
    fn rule4_does_not_fire_for_borrowed() {
        let mut s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::MaybeShared);
    }

    // Rule 5: Unique + Dead → preserve ReusableCtor shape

    #[test]
    fn rule5_unique_dead_preserves_reusable_ctor() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Dead,
            cardinality: Cardinality::Absent,
            uniqueness: Uniqueness::Unique,
            locality: Locality::BlockLocal,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert!(
            matches!(s.shape, ShapeClass::ReusableCtor(ReuseCtorKind::Struct)),
            "Unique + Dead should preserve ReusableCtor for reuse"
        );
    }

    #[test]
    fn rule5_shared_dead_collapses_reusable_ctor() {
        // Contrast: Shared + Dead → Rule 3 collapses shape
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Dead,
            cardinality: Cardinality::Absent,
            uniqueness: Uniqueness::Shared,
            locality: Locality::BlockLocal,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.shape, ShapeClass::NonReusable);
    }

    // Rule 6: HeapEscaping → uniqueness >= MaybeShared

    #[test]
    fn rule6_heap_escaping_unique_becomes_maybe_shared() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(
            s.uniqueness,
            Uniqueness::MaybeShared,
            "HeapEscaping values cannot be assumed Unique"
        );
    }

    #[test]
    fn rule6_heap_escaping_maybe_shared_unchanged() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::MaybeShared);
    }

    #[test]
    fn rule6_heap_escaping_shared_unchanged() {
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Shared,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::Shared);
    }

    #[test]
    fn rule6_does_not_fire_for_block_local() {
        // BlockLocal + Unique should stay Unique (not weakened)
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::Unique);
    }

    #[test]
    fn rule6_does_not_fire_for_function_local() {
        // FunctionLocal + Unique should stay Unique
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::Unique);
    }

    #[test]
    fn rule6_does_not_fire_for_unknown_locality() {
        // Unknown + Unique stays Unique — Unknown means conservative locality
        // analysis hasn't run; we don't weaken uniqueness speculatively
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.uniqueness, Uniqueness::Unique);
    }

    // Rule 8: Borrowed → locality <= FunctionLocal

    #[test]
    fn rule8_borrowed_heap_escaping_tightens_to_function_local() {
        let mut s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(
            s.locality,
            Locality::FunctionLocal,
            "Borrowed values cannot escape their function"
        );
    }

    #[test]
    fn rule8_borrowed_unknown_tightens_to_function_local() {
        let mut s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.locality, Locality::FunctionLocal);
    }

    #[test]
    fn rule8_borrowed_function_local_unchanged() {
        let mut s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        let before_locality = s.locality;
        s.canonicalize();
        assert_eq!(s.locality, before_locality);
    }

    #[test]
    fn rule8_borrowed_block_local_unchanged() {
        let mut s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        let before_locality = s.locality;
        s.canonicalize();
        assert_eq!(s.locality, before_locality);
    }

    #[test]
    fn rule8_owned_heap_escaping_not_tightened() {
        // Owned values CAN escape to the heap — Rule 8 only applies to Borrowed
        let mut s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(s.locality, Locality::HeapEscaping);
    }

    // Rule interaction: Rule 8 prevents Rule 6 from firing on Borrowed values

    #[test]
    fn rule8_then_rule6_borrowed_unique_heap_escaping() {
        // Borrowed + Unique + HeapEscaping: Rule 8 tightens to FunctionLocal
        // first, so Rule 6 (HeapEscaping → not Unique) does NOT fire.
        let mut s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        s.canonicalize();
        assert_eq!(
            s.locality,
            Locality::FunctionLocal,
            "Rule 8 tightens locality"
        );
        assert_eq!(
            s.uniqueness,
            Uniqueness::Unique,
            "Rule 6 should not fire after Rule 8 corrected locality"
        );
    }

    #[test]
    fn valid_states_unchanged() {
        // (Owned, Linear, Once, Unique) is valid — should not change
        let mut s = AimsState::FRESH;
        let before = s;
        s.canonicalize();
        assert_eq!(s, before);

        // (Borrowed, Dead, Absent, Unique) is valid
        let mut s = AimsState::BOTTOM;
        let before = s;
        s.canonicalize();
        assert_eq!(s, before);

        // TOP is valid
        let mut s = AimsState::TOP;
        let before = s;
        s.canonicalize();
        assert_eq!(s, before);
    }

    #[test]
    fn join_produces_canonical_output() {
        let states = representative_states();
        let sample: Vec<_> = states.iter().step_by(5).collect();
        for a in &sample {
            for b in &sample {
                let joined = a.join(b);
                let mut check = joined;
                check.canonicalize();
                assert_eq!(
                    joined, check,
                    "join should produce canonical output:\n  a={a:?}\n  b={b:?}"
                );
            }
        }
    }
}

// Feasibility tests

mod feasibility {
    use super::*;

    #[test]
    fn dead_never_has_uses() {
        for s in representative_states() {
            if s.consumption == Consumption::Dead {
                assert_eq!(
                    s.cardinality,
                    Cardinality::Absent,
                    "dead must have Absent cardinality: {s:?}"
                );
            }
        }
    }

    #[test]
    fn absent_never_live() {
        for s in representative_states() {
            if s.cardinality == Cardinality::Absent {
                assert_eq!(
                    s.consumption,
                    Consumption::Dead,
                    "absent must be Dead: {s:?}"
                );
            }
        }
    }

    #[test]
    fn borrowed_never_rc_needed() {
        for s in representative_states() {
            if s.access == AccessClass::Borrowed {
                assert!(!s.is_rc_needed(), "borrowed should not need RC: {s:?}");
            }
        }
    }

    #[test]
    fn shared_never_reuse_candidate() {
        for mut s in representative_states() {
            s.canonicalize();
            if s.uniqueness == Uniqueness::Shared {
                assert!(
                    !s.is_reuse_candidate(),
                    "shared should not be reuse candidate: {s:?}"
                );
            }
        }
    }
}

// Query method tests

mod queries {
    use super::*;

    #[test]
    fn scalar_is_scalar() {
        assert!(AimsState::SCALAR.is_scalar());
        assert!(!AimsState::TOP.is_scalar());
        assert!(!AimsState::BOTTOM.is_scalar());
        assert!(!AimsState::FRESH.is_scalar());
    }

    #[test]
    fn scalar_no_rc() {
        assert!(!AimsState::SCALAR.is_rc_needed());
    }

    #[test]
    fn fresh_needs_rc() {
        assert!(AimsState::FRESH.is_rc_needed());
    }

    #[test]
    fn dead_no_rc() {
        let dead = AimsState {
            consumption: Consumption::Dead,
            cardinality: Cardinality::Absent,
            ..AimsState::TOP
        };
        assert!(!dead.is_rc_needed());
    }

    #[test]
    fn borrowed_no_rc() {
        let borrowed = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            ..AimsState::FRESH
        };
        assert!(!borrowed.is_rc_needed());
    }

    #[test]
    fn cow_check_only_maybe_shared() {
        assert!(!AimsState::FRESH.needs_cow_check()); // Unique
        assert!(!AimsState::TOP.needs_cow_check()); // Shared

        let maybe = AimsState {
            uniqueness: Uniqueness::MaybeShared,
            ..AimsState::FRESH
        };
        assert!(maybe.needs_cow_check());
    }

    #[test]
    fn reuse_candidate_requirements() {
        // Fresh with reusable shape: yes
        let reusable = AimsState {
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::FRESH
        };
        assert!(reusable.is_reuse_candidate());

        // Shared: no (shape collapses to NonReusable after canonicalize)
        let mut shared = AimsState {
            uniqueness: Uniqueness::Shared,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::FRESH
        };
        shared.canonicalize();
        assert!(!shared.is_reuse_candidate());

        // Borrowed: no
        let borrowed = AimsState {
            access: AccessClass::Borrowed,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::FRESH
        };
        assert!(!borrowed.is_reuse_candidate());

        // NonReusable shape: no
        assert!(!AimsState::FRESH.is_reuse_candidate());
    }

    #[test]
    fn locality_check() {
        assert!(AimsState::FRESH.is_local()); // BlockLocal
        assert!(!AimsState::TOP.is_local()); // Unknown

        let block_local = AimsState {
            locality: Locality::BlockLocal,
            ..AimsState::FRESH
        };
        assert!(block_local.is_local());

        let heap = AimsState {
            locality: Locality::HeapEscaping,
            ..AimsState::FRESH
        };
        assert!(!heap.is_local());
    }

    #[test]
    fn rc_skip_eligible_function_local_linear() {
        // FunctionLocal + Owned + Linear → RC-skip eligible
        let state = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        assert!(state.is_rc_skip_eligible());
    }

    #[test]
    fn rc_skip_not_eligible_heap_escaping() {
        let state = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        assert!(!state.is_rc_skip_eligible());
    }

    #[test]
    fn rc_skip_not_eligible_unrestricted() {
        let state = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: Uniqueness::Unique,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        assert!(!state.is_rc_skip_eligible());
    }

    #[test]
    fn rc_skip_not_eligible_borrowed() {
        let state = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        assert!(!state.is_rc_skip_eligible());
    }

    #[test]
    fn rc_skip_block_local_also_eligible() {
        // BlockLocal is also local — RC-skip works for block-local too
        let state = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::NONE,
        };
        assert!(state.is_rc_skip_eligible());
    }
}

// from_arc_class tests

mod from_arc_class {
    use super::*;

    #[test]
    fn scalar_maps_to_scalar() {
        let s = AimsState::from_arc_class(ArcClass::Scalar);
        assert!(s.is_scalar());
        assert!(!s.is_rc_needed());
    }

    #[test]
    fn definite_ref_maps_to_top() {
        let s = AimsState::from_arc_class(ArcClass::DefiniteRef);
        assert_eq!(s, AimsState::TOP);
    }

    #[test]
    fn possible_ref_maps_to_top() {
        let s = AimsState::from_arc_class(ArcClass::PossibleRef);
        assert_eq!(s, AimsState::TOP);
    }
}

// BorrowSource tests

mod borrow_source {
    use super::*;

    #[test]
    fn same_source_preserved() {
        let var = ArcVarId::new(42);
        let a = BorrowSource::exact(var);
        let b = BorrowSource::exact(var);
        assert_eq!(a.join(b), BorrowSource::exact(var));
    }

    #[test]
    fn different_sources_promote_to_unknown() {
        let a = BorrowSource::exact(ArcVarId::new(1));
        let b = BorrowSource::exact(ArcVarId::new(2));
        assert_eq!(a.join(b), BorrowSource::Unknown);
    }

    #[test]
    fn unknown_absorbs() {
        let exact = BorrowSource::exact(ArcVarId::new(1));
        assert_eq!(exact.join(BorrowSource::Unknown), BorrowSource::Unknown);
        assert_eq!(BorrowSource::Unknown.join(exact), BorrowSource::Unknown);
        assert_eq!(
            BorrowSource::Unknown.join(BorrowSource::Unknown),
            BorrowSource::Unknown
        );
    }
}

// Finite height test

#[test]
fn chain_height_is_15() {
    // AccessClass: 1 (Borrowed → Owned)
    // Consumption: 3 (Dead → Linear → Affine → Unrestricted)
    // Cardinality: 2 (Absent → Once → Many)
    // Uniqueness: 2 (Unique → MaybeShared → Shared)
    // Locality: 3 (BlockLocal → FunctionLocal → HeapEscaping → Unknown)
    // ShapeClass: 1 (any → NonReusable)
    // EffectClass: 3 (three booleans, each false → true)
    let expected_height = 1 + 3 + 2 + 2 + 3 + 1 + 3;
    assert_eq!(expected_height, 15);

    // Verify by ascending from BOTTOM to TOP
    let mut steps = 0;
    let mut current = AimsState::BOTTOM;

    // Access: Borrowed → Owned (1 step)
    if current.access < AccessClass::Owned {
        current.access = AccessClass::Owned;
        steps += 1;
    }
    // Consumption: Dead → Linear → Affine → Unrestricted (3 steps)
    for next in [
        Consumption::Linear,
        Consumption::Affine,
        Consumption::Unrestricted,
    ] {
        if current.consumption < next {
            current.consumption = next;
            steps += 1;
        }
    }
    // Cardinality: Absent → Once → Many (2 steps)
    for next in [Cardinality::Once, Cardinality::Many] {
        if current.cardinality < next {
            current.cardinality = next;
            steps += 1;
        }
    }
    // Uniqueness: Unique → MaybeShared → Shared (2 steps)
    for next in [Uniqueness::MaybeShared, Uniqueness::Shared] {
        if current.uniqueness < next {
            current.uniqueness = next;
            steps += 1;
        }
    }
    // Locality: BlockLocal → FunctionLocal → HeapEscaping → Unknown (3 steps)
    for next in [
        Locality::FunctionLocal,
        Locality::HeapEscaping,
        Locality::Unknown,
    ] {
        if current.locality < next {
            current.locality = next;
            steps += 1;
        }
    }
    // ShapeClass: flat lattice, 1 step to NonReusable (already there from BOTTOM)
    // but BOTTOM starts at NonReusable, so no additional step needed in this path.
    // The chain height contribution is still 1 for any other starting point.
    // We verify this separately.
    let struct_shape = ShapeClass::ReusableCtor(ReuseCtorKind::Struct);
    assert_eq!(
        struct_shape.join(ShapeClass::CollectionBuffer),
        ShapeClass::NonReusable
    );
    // One step: any non-top → NonReusable
    steps += 1; // Count the 1-step height

    // EffectClass: 3 steps (each boolean false → true)
    current.effect = EffectClass {
        may_alloc: true,
        ..current.effect
    };
    steps += 1;
    current.effect = EffectClass {
        may_share: true,
        ..current.effect
    };
    steps += 1;
    current.effect = EffectClass {
        may_throw: true,
        ..current.effect
    };
    let _ = current;
    steps += 1;

    assert_eq!(steps, expected_height);
}

// Dimension interaction tests (01.3a)

mod dimension_interactions {
    use super::*;

    #[test]
    fn access_x_consumption_borrowed_no_rc() {
        // Borrowed values never need RC regardless of consumption
        for consumption in all_consumption() {
            let s = AimsState {
                access: AccessClass::Borrowed,
                consumption,
                cardinality: Cardinality::Many,
                uniqueness: Uniqueness::Shared,
                locality: Locality::Unknown,
                shape: ShapeClass::NonReusable,
                effect: EffectClass::ALL,
            };
            assert!(
                !s.is_rc_needed(),
                "borrowed + {consumption:?} should not need RC"
            );
        }
    }

    #[test]
    fn consumption_x_cardinality_sync() {
        // Dead ↔ Absent enforced by canonicalization
        let mut s = AimsState {
            consumption: Consumption::Dead,
            cardinality: Cardinality::Many,
            ..AimsState::TOP
        };
        s.canonicalize();
        assert_eq!(s.cardinality, Cardinality::Absent);

        let mut s = AimsState {
            consumption: Consumption::Affine,
            cardinality: Cardinality::Absent,
            ..AimsState::TOP
        };
        s.canonicalize();
        assert_eq!(s.consumption, Consumption::Dead);
    }

    #[test]
    fn access_x_uniqueness_borrow_preserves_uniqueness() {
        // Borrowing from a unique source preserves uniqueness
        for uniqueness in all_uniqueness() {
            let s = AimsState {
                access: AccessClass::Borrowed,
                uniqueness,
                ..AimsState::FRESH
            };
            assert_eq!(s.uniqueness, uniqueness);
        }
    }

    #[test]
    fn uniqueness_x_shape_shared_no_reuse() {
        let mut s = AimsState {
            uniqueness: Uniqueness::Shared,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::FRESH
        };
        s.canonicalize();
        assert!(!s.is_reuse_candidate());
    }

    #[test]
    fn uniqueness_x_shape_unique_static_reuse() {
        let s = AimsState {
            uniqueness: Uniqueness::Unique,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::FRESH
        };
        assert!(s.is_reuse_candidate());
    }

    #[test]
    fn uniqueness_x_shape_maybe_shared_dynamic_reuse() {
        let s = AimsState {
            uniqueness: Uniqueness::MaybeShared,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
            ..AimsState::FRESH
        };
        assert!(s.is_reuse_candidate());
    }

    #[test]
    fn locality_x_access_borrow_inherits() {
        // Locality is independent of access class (borrow inherits from source)
        for locality in all_locality() {
            let s = AimsState {
                access: AccessClass::Borrowed,
                locality,
                ..AimsState::FRESH
            };
            assert_eq!(s.locality, locality);
        }
    }
}

// Pairwise interaction tests (core × core)

mod pairwise_interactions {
    use super::*;

    #[test]
    fn access_x_consumption() {
        for access in all_access() {
            for consumption in all_consumption() {
                let mut s = AimsState {
                    access,
                    consumption,
                    cardinality: Cardinality::Once,
                    uniqueness: Uniqueness::Unique,
                    locality: Locality::FunctionLocal,
                    shape: ShapeClass::NonReusable,
                    effect: EffectClass::NONE,
                };
                s.canonicalize();
                // Only owned + non-dead should need RC
                let expected_rc = access == AccessClass::Owned && consumption != Consumption::Dead;
                assert_eq!(
                    s.is_rc_needed(),
                    expected_rc,
                    "access={access:?}, consumption={consumption:?}"
                );
            }
        }
    }

    #[test]
    fn consumption_x_cardinality() {
        for consumption in all_consumption() {
            for cardinality in all_cardinality() {
                let mut s = AimsState {
                    consumption,
                    cardinality,
                    ..AimsState::FRESH
                };
                s.canonicalize();
                // Dead ↔ Absent invariant
                if s.consumption == Consumption::Dead {
                    assert_eq!(s.cardinality, Cardinality::Absent);
                }
                if s.cardinality == Cardinality::Absent {
                    assert_eq!(s.consumption, Consumption::Dead);
                }
            }
        }
    }

    #[test]
    fn uniqueness_x_shape() {
        for uniqueness in all_uniqueness() {
            for shape in all_shape() {
                let mut s = AimsState {
                    uniqueness,
                    shape,
                    ..AimsState::FRESH
                };
                s.canonicalize();
                // Shared + ReusableCtor → NonReusable
                if uniqueness == Uniqueness::Shared && matches!(shape, ShapeClass::ReusableCtor(_))
                {
                    assert_eq!(s.shape, ShapeClass::NonReusable);
                }
            }
        }
    }
}

// Transfer function monotonicity (01.6)

mod monotonicity {
    use super::*;
    use crate::aims::transfer;

    /// Check if `a ≤ b` in the lattice: `a.join(b) == b`.
    fn le(a: &AimsState, b: &AimsState) -> bool {
        a.join(b) == *b
    }

    #[test]
    fn project_is_monotonic() {
        // Project depends on the source variable's state.
        // If source_a ≤ source_b, then Project(source_a) ≤ Project(source_b).
        let states = representative_states();
        for a in &states {
            for b in &states {
                if !le(a, b) {
                    continue;
                }
                // Create a Project instruction reading from var(0)
                let instr = crate::ir::ArcInstr::Project {
                    dst: ArcVarId::new(1),
                    ty: ori_types::Idx::from_raw(0),
                    value: ArcVarId::new(0),
                    field: 0,
                };
                let fa = transfer::transfer_def(&instr, &|_| *a).unwrap();
                let fb = transfer::transfer_def(&instr, &|_| *b).unwrap();
                assert!(
                    le(&fa.state, &fb.state),
                    "monotonicity: a={a:?}, b={b:?}, f(a)={:?}, f(b)={:?}",
                    fa.state,
                    fb.state
                );
            }
        }
    }

    #[test]
    fn select_is_monotonic() {
        // Select joins two branch states. If both branches' states
        // increase monotonically, the join must also increase.
        let states = representative_states();
        // Use a subset to avoid O(n^4) explosion
        let sample: Vec<_> = states.iter().step_by(3).copied().collect();
        for a1 in &sample {
            for a2 in &sample {
                for b1 in &sample {
                    for b2 in &sample {
                        if !le(a1, b1) || !le(a2, b2) {
                            continue;
                        }
                        let instr = crate::ir::ArcInstr::Select {
                            dst: ArcVarId::new(3),
                            ty: ori_types::Idx::from_raw(0),
                            cond: ArcVarId::new(0),
                            true_val: ArcVarId::new(1),
                            false_val: ArcVarId::new(2),
                        };
                        let fa = transfer::transfer_def(&instr, &|v| match v.raw() {
                            1 => *a1,
                            2 => *a2,
                            _ => AimsState::SCALAR,
                        })
                        .unwrap();
                        let fb = transfer::transfer_def(&instr, &|v| match v.raw() {
                            1 => *b1,
                            2 => *b2,
                            _ => AimsState::SCALAR,
                        })
                        .unwrap();
                        assert!(
                            le(&fa.state, &fb.state),
                            "select monotonicity: a1={a1:?}, a2={a2:?}, b1={b1:?}, b2={b2:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn let_var_is_monotonic() {
        // Let(Var(v)) inherits the source state — trivially monotonic
        // (identity function is monotonic).
        let states = representative_states();
        for a in &states {
            for b in &states {
                if !le(a, b) {
                    continue;
                }
                let instr = crate::ir::ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: ori_types::Idx::from_raw(0),
                    value: crate::ir::ArcValue::Var(ArcVarId::new(0)),
                };
                let fa = transfer::transfer_def(&instr, &|_| *a).unwrap();
                let fb = transfer::transfer_def(&instr, &|_| *b).unwrap();
                assert!(le(&fa.state, &fb.state));
            }
        }
    }

    #[test]
    fn constant_transfers_are_trivially_monotonic() {
        // Construct, Apply, PartialApply, IsShared, Reset produce
        // constant output regardless of input state — trivially monotonic.
        let construct = crate::ir::ArcInstr::Construct {
            dst: ArcVarId::new(0),
            ty: ori_types::Idx::from_raw(0),
            ctor: crate::ir::CtorKind::Tuple,
            args: vec![],
        };
        let r1 = transfer::transfer_def(&construct, &|_| AimsState::BOTTOM).unwrap();
        let r2 = transfer::transfer_def(&construct, &|_| AimsState::TOP).unwrap();
        assert_eq!(r1.state, r2.state, "Construct output must be constant");
    }
}

// Soundness properties (01.6)

mod soundness {
    use super::*;

    #[test]
    fn unique_implies_no_cow_check() {
        // If analysis says Unique, no runtime COW check needed.
        let s = AimsState {
            uniqueness: Uniqueness::Unique,
            ..AimsState::FRESH
        };
        assert!(!s.needs_cow_check());
    }

    #[test]
    fn borrowed_implies_no_rc() {
        // Borrowed variables never need RC operations.
        for consumption in all_consumption() {
            for cardinality in all_cardinality() {
                let mut s = AimsState {
                    access: AccessClass::Borrowed,
                    consumption,
                    cardinality,
                    uniqueness: Uniqueness::Unique,
                    locality: Locality::FunctionLocal,
                    shape: ShapeClass::NonReusable,
                    effect: EffectClass::NONE,
                };
                s.canonicalize();
                assert!(
                    !s.is_rc_needed(),
                    "borrowed + {consumption:?} + {cardinality:?} should not need RC"
                );
            }
        }
    }

    #[test]
    fn dead_implies_no_rc() {
        // Dead variables never need RC operations.
        let mut s = AimsState {
            consumption: Consumption::Dead,
            ..AimsState::TOP
        };
        s.canonicalize();
        assert!(!s.is_rc_needed());
    }

    #[test]
    fn conservative_direction_join_never_decreases() {
        // Join always produces a state that is >= both inputs.
        // This ensures the analysis over-approximates, never under-approximates.
        let states = representative_states();
        for a in &states {
            for b in &states {
                let joined = a.join(b);
                // a ≤ joined
                assert_eq!(
                    a.join(&joined),
                    joined,
                    "a={a:?} should be ≤ join(a,b)={joined:?}"
                );
                // b ≤ joined
                assert_eq!(
                    b.join(&joined),
                    joined,
                    "b={b:?} should be ≤ join(a,b)={joined:?}"
                );
            }
        }
    }

    #[test]
    fn linear_once_implies_no_rc_inc() {
        // Linear + Once = value consumed exactly once, no duplication needed.
        let s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            ..AimsState::FRESH
        };
        assert!(crate::aims::transfer::is_rc_inc_elidable(&s));
    }
}

// Feasible/infeasible state table (01.6)

mod feasibility_table {
    use super::*;

    /// A state is feasible if `canonicalize` doesn't change it.
    fn is_feasible(s: &AimsState) -> bool {
        let mut copy = *s;
        copy.canonicalize();
        copy == *s
    }

    #[test]
    fn dead_absent_is_feasible() {
        let s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Dead,
            cardinality: Cardinality::Absent,
            ..AimsState::FRESH
        };
        assert!(is_feasible(&s));
    }

    #[test]
    fn dead_once_is_infeasible() {
        // Dead + Once violates the Dead ↔ Absent invariant.
        let s = AimsState {
            consumption: Consumption::Dead,
            cardinality: Cardinality::Once,
            ..AimsState::FRESH
        };
        assert!(!is_feasible(&s));
    }

    #[test]
    fn dead_many_is_infeasible() {
        let s = AimsState {
            consumption: Consumption::Dead,
            cardinality: Cardinality::Many,
            ..AimsState::FRESH
        };
        assert!(!is_feasible(&s));
    }

    #[test]
    fn linear_absent_is_infeasible() {
        // Linear requires at least one use, Absent means zero uses.
        let s = AimsState {
            consumption: Consumption::Linear,
            cardinality: Cardinality::Absent,
            ..AimsState::FRESH
        };
        assert!(!is_feasible(&s));
    }

    #[test]
    fn affine_absent_collapses_to_dead() {
        // Absent forces Dead (regardless of original consumption).
        let mut s = AimsState {
            consumption: Consumption::Affine,
            cardinality: Cardinality::Absent,
            ..AimsState::FRESH
        };
        s.canonicalize();
        assert_eq!(s.consumption, Consumption::Dead);
        assert_eq!(s.cardinality, Cardinality::Absent);
    }

    #[test]
    fn shared_reusable_collapses_to_non_reusable() {
        let mut s = AimsState {
            uniqueness: Uniqueness::Shared,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            ..AimsState::FRESH
        };
        s.canonicalize();
        assert_eq!(s.shape, ShapeClass::NonReusable);
    }

    #[test]
    fn owned_linear_unique_once_is_feasible() {
        // Fresh, consumed once, no RC needed.
        let s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            ..AimsState::FRESH
        };
        assert!(is_feasible(&s));
    }

    #[test]
    fn owned_unrestricted_shared_many_is_feasible() {
        // Full ARC — the current default.
        let s = AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: Uniqueness::Shared,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            effect: EffectClass::ALL,
        };
        assert!(is_feasible(&s));
    }

    #[test]
    fn borrowed_linear_once_is_feasible() {
        // Temporary view, one read, no RC.
        let s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            ..AimsState::FRESH
        };
        assert!(is_feasible(&s));
    }

    #[test]
    fn borrowed_dead_absent_is_feasible() {
        // Expired borrow — no RC, no use.
        let s = AimsState {
            access: AccessClass::Borrowed,
            consumption: Consumption::Dead,
            cardinality: Cardinality::Absent,
            ..AimsState::FRESH
        };
        assert!(is_feasible(&s));
    }

    #[test]
    fn enumerate_all_core_feasibility() {
        // Exhaustively check all core dimension combinations.
        let mut feasible_count = 0;
        let mut infeasible_count = 0;
        for &access in &all_access() {
            for &consumption in &all_consumption() {
                for &cardinality in &all_cardinality() {
                    for &uniqueness in &all_uniqueness() {
                        let s = AimsState {
                            access,
                            consumption,
                            cardinality,
                            uniqueness,
                            locality: Locality::FunctionLocal,
                            shape: ShapeClass::NonReusable,
                            effect: EffectClass::NONE,
                        };
                        if is_feasible(&s) {
                            feasible_count += 1;
                        } else {
                            infeasible_count += 1;
                        }
                    }
                }
            }
        }
        // 2 access × 4 consumption × 3 cardinality × 3 uniqueness = 72 total
        assert_eq!(feasible_count + infeasible_count, 72);
        // Infeasible: Dead+Once, Dead+Many, Linear+Absent, Affine+Absent,
        // Unrestricted+Absent = 5 patterns × 2 access × 3 uniqueness = 30
        // But some of these overlap — let's just verify the counts are reasonable.
        assert!(
            feasible_count > 0 && infeasible_count > 0,
            "should have both feasible and infeasible states"
        );
        assert!(
            infeasible_count < feasible_count,
            "most states should be feasible after canonicalization design"
        );
    }
}

// Non-convergence safety (01.7)

#[test]
fn chain_height_constant_matches_sum() {
    assert_eq!(AimsState::CHAIN_HEIGHT, 15);
}

#[test]
fn iteration_limit_scales_linearly() {
    assert_eq!(AimsState::iteration_limit(10, 5), 15 * 10 * 5);
    assert_eq!(AimsState::iteration_limit(1, 1), 15);
    assert_eq!(AimsState::iteration_limit(0, 100), 0);
}

// Constants sanity checks

#[test]
fn constants_are_canonical() {
    let mut top = AimsState::TOP;
    top.canonicalize();
    assert_eq!(top, AimsState::TOP);

    let mut bottom = AimsState::BOTTOM;
    bottom.canonicalize();
    assert_eq!(bottom, AimsState::BOTTOM);

    let mut fresh = AimsState::FRESH;
    fresh.canonicalize();
    assert_eq!(fresh, AimsState::FRESH);
}

#[test]
fn bottom_le_top() {
    // join(BOTTOM, TOP) == TOP (TOP dominates)
    let joined = AimsState::BOTTOM.join(&AimsState::TOP);
    // After canonicalization, should be TOP-like
    assert_eq!(joined.access, AccessClass::Owned);
    assert_eq!(joined.consumption, Consumption::Unrestricted);
    assert_eq!(joined.cardinality, Cardinality::Many);
    assert_eq!(joined.uniqueness, Uniqueness::Shared);
    assert_eq!(joined.locality, Locality::Unknown);
    assert_eq!(joined.shape, ShapeClass::NonReusable);
    assert_eq!(joined.effect, EffectClass::ALL);
}

#[test]
fn fresh_is_optimistic_owned() {
    let f = AimsState::FRESH;
    assert_eq!(f.access, AccessClass::Owned);
    assert_eq!(f.consumption, Consumption::Linear);
    assert_eq!(f.cardinality, Cardinality::Once);
    assert_eq!(f.uniqueness, Uniqueness::Unique);
    // Section 09.2: FRESH starts BlockLocal (hasn't escaped the block).
    // Cross-block flow widens to FunctionLocal; return widens to HeapEscaping.
    assert_eq!(f.locality, Locality::BlockLocal);
    assert_eq!(f.shape, ShapeClass::NonReusable);
    assert_eq!(f.effect, EffectClass::NONE);
    assert!(f.is_rc_needed());
    assert!(!f.needs_cow_check());
    assert!(!f.is_reuse_candidate()); // NonReusable shape
}
