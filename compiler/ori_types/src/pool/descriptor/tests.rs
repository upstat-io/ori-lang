//! Tests for `TypeDescriptor` and `VariantDescriptor`.

use ori_ir::Name;

use super::{TypeDescriptor, VariantDescriptor};
use crate::pool::EnumVariant;
use crate::Tag;

#[test]
fn primitive_descriptor_equality() {
    let d1 = TypeDescriptor::Primitive(Tag::Int);
    let d2 = TypeDescriptor::Primitive(Tag::Int);
    let d3 = TypeDescriptor::Primitive(Tag::Str);

    assert_eq!(d1, d2);
    assert_ne!(d1, d3);
}

#[test]
fn container_descriptor_equality() {
    let d1 = TypeDescriptor::Container {
        tag: Tag::List,
        child_hash: 42,
    };
    let d2 = TypeDescriptor::Container {
        tag: Tag::List,
        child_hash: 42,
    };
    let d3 = TypeDescriptor::Container {
        tag: Tag::List,
        child_hash: 99,
    };

    assert_eq!(d1, d2);
    assert_ne!(d1, d3);
}

#[test]
fn function_descriptor_equality() {
    let d1 = TypeDescriptor::Function {
        param_hashes: vec![1, 2, 3],
        return_hash: 99,
    };
    let d2 = TypeDescriptor::Function {
        param_hashes: vec![1, 2, 3],
        return_hash: 99,
    };
    let d3 = TypeDescriptor::Function {
        param_hashes: vec![1, 2],
        return_hash: 99,
    };

    assert_eq!(d1, d2);
    assert_ne!(d1, d3);
}

#[test]
fn variant_descriptor_equality() {
    let v1 = VariantDescriptor {
        name: Name::new(0, 1),
        field_type_hashes: vec![10, 20],
    };
    let v2 = VariantDescriptor {
        name: Name::new(0, 1),
        field_type_hashes: vec![10, 20],
    };
    let v3 = VariantDescriptor {
        name: Name::new(0, 2),
        field_type_hashes: vec![10, 20],
    };

    assert_eq!(v1, v2);
    assert_ne!(v1, v3);
}

#[test]
fn enum_descriptor_with_variants() {
    let d = TypeDescriptor::Enum {
        name: Name::new(0, 5),
        variants: vec![
            VariantDescriptor {
                name: Name::new(0, 6),
                field_type_hashes: vec![],
            },
            VariantDescriptor {
                name: Name::new(0, 7),
                field_type_hashes: vec![42],
            },
        ],
    };

    // Verify we can clone and compare
    let d2 = d.clone();
    assert_eq!(d, d2);
}

#[test]
fn struct_descriptor_field_count() {
    let d = TypeDescriptor::Struct {
        name: Name::new(0, 10),
        field_names: vec![Name::new(0, 11), Name::new(0, 12), Name::new(0, 13)],
        field_type_hashes: vec![100, 200, 300],
    };

    if let TypeDescriptor::Struct {
        field_names,
        field_type_hashes,
        ..
    } = &d
    {
        assert_eq!(field_names.len(), 3);
        assert_eq!(field_type_hashes.len(), 3);
    } else {
        panic!("expected Struct descriptor");
    }
}

#[test]
fn scheme_descriptor() {
    let d = TypeDescriptor::Scheme {
        var_count: 2,
        var_ids: vec![0, 1],
        body_hash: 999,
    };

    if let TypeDescriptor::Scheme {
        var_count,
        var_ids,
        body_hash,
    } = &d
    {
        assert_eq!(*var_count, 2);
        assert_eq!(var_ids.len(), 2);
        assert_eq!(*body_hash, 999);
    } else {
        panic!("expected Scheme descriptor");
    }
}

#[test]
fn variable_descriptor_preserves_tag() {
    let var = TypeDescriptor::Variable {
        tag: Tag::Var,
        var_id: 7,
    };
    let bound = TypeDescriptor::Variable {
        tag: Tag::BoundVar,
        var_id: 7,
    };
    let rigid = TypeDescriptor::Variable {
        tag: Tag::RigidVar,
        var_id: 7,
    };

    // Same var_id but different tags → different descriptors
    assert_ne!(var, bound);
    assert_ne!(bound, rigid);
    assert_ne!(var, rigid);
}

#[test]
fn all_child_refs_are_hashes_not_idx() {
    // This test documents the design invariant: TypeDescriptor never contains Idx.
    // All child type references are u64 Merkle hashes. We verify this structurally
    // by constructing every variant — if any variant accepted Idx, this wouldn't compile.
    let all = vec![
        TypeDescriptor::Primitive(Tag::Int),
        TypeDescriptor::Container {
            tag: Tag::List,
            child_hash: 0u64,
        },
        TypeDescriptor::TwoChild {
            tag: Tag::Map,
            child1_hash: 0u64,
            child2_hash: 0u64,
        },
        TypeDescriptor::Borrowed {
            inner_hash: 0u64,
            lifetime_id: 0u32,
        },
        TypeDescriptor::Function {
            param_hashes: vec![0u64],
            return_hash: 0u64,
        },
        TypeDescriptor::Tuple {
            element_hashes: vec![0u64],
        },
        TypeDescriptor::Struct {
            name: Name::new(0, 0),
            field_names: vec![],
            field_type_hashes: vec![],
        },
        TypeDescriptor::Enum {
            name: Name::new(0, 0),
            variants: vec![],
        },
        TypeDescriptor::Applied {
            name: Name::new(0, 0),
            arg_hashes: vec![0u64],
        },
        TypeDescriptor::Named {
            name: Name::new(0, 0),
        },
        TypeDescriptor::Scheme {
            var_count: 0,
            var_ids: vec![],
            body_hash: 0u64,
        },
        TypeDescriptor::Variable {
            tag: Tag::Var,
            var_id: 0u32,
        },
    ];
    assert_eq!(all.len(), 12, "all TypeDescriptor variants covered");
}

/// Verify descriptors implement all required Salsa-compatible traits.
#[test]
fn salsa_trait_bounds() {
    fn assert_salsa_compatible<T: Clone + std::fmt::Debug + PartialEq + Eq + std::hash::Hash>() {}
    assert_salsa_compatible::<TypeDescriptor>();
    assert_salsa_compatible::<VariantDescriptor>();
}

// === Descriptor Generation Tests ===
//
// These tests verify that Pool::describe() and Pool::describe_transitive()
// correctly generate TypeDescriptors from live Pool types.

use crate::{Idx, Pool};

#[test]
fn describe_primitive() {
    let pool = Pool::new();
    let desc = pool.describe(Idx::INT);
    assert_eq!(desc, TypeDescriptor::Primitive(Tag::Int));

    let desc = pool.describe(Idx::STR);
    assert_eq!(desc, TypeDescriptor::Primitive(Tag::Str));

    let desc = pool.describe(Idx::BOOL);
    assert_eq!(desc, TypeDescriptor::Primitive(Tag::Bool));
}

#[test]
fn describe_simple_container() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let desc = pool.describe(list_int);

    assert!(matches!(
        desc,
        TypeDescriptor::Container { tag: Tag::List, .. }
    ));
    if let TypeDescriptor::Container { child_hash, .. } = desc {
        assert_eq!(child_hash, pool.hash(Idx::INT));
    }
}

#[test]
fn describe_option() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let desc = pool.describe(opt_str);

    if let TypeDescriptor::Container { tag, child_hash } = desc {
        assert_eq!(tag, Tag::Option);
        assert_eq!(child_hash, pool.hash(Idx::STR));
    } else {
        panic!("expected Container descriptor, got {desc:?}");
    }
}

#[test]
fn describe_map() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let desc = pool.describe(map_str_int);

    if let TypeDescriptor::TwoChild {
        tag,
        child1_hash,
        child2_hash,
    } = desc
    {
        assert_eq!(tag, Tag::Map);
        assert_eq!(child1_hash, pool.hash(Idx::STR));
        assert_eq!(child2_hash, pool.hash(Idx::INT));
    } else {
        panic!("expected TwoChild descriptor, got {desc:?}");
    }
}

#[test]
fn describe_function() {
    let mut pool = Pool::new();
    let fn_type = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let desc = pool.describe(fn_type);

    if let TypeDescriptor::Function {
        param_hashes,
        return_hash,
    } = desc
    {
        assert_eq!(param_hashes.len(), 2);
        assert_eq!(param_hashes[0], pool.hash(Idx::INT));
        assert_eq!(param_hashes[1], pool.hash(Idx::STR));
        assert_eq!(return_hash, pool.hash(Idx::BOOL));
    } else {
        panic!("expected Function descriptor, got {desc:?}");
    }
}

#[test]
fn describe_tuple() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::BOOL, Idx::STR]);
    let desc = pool.describe(tup);

    if let TypeDescriptor::Tuple { element_hashes } = desc {
        assert_eq!(element_hashes.len(), 3);
        assert_eq!(element_hashes[0], pool.hash(Idx::INT));
        assert_eq!(element_hashes[1], pool.hash(Idx::BOOL));
        assert_eq!(element_hashes[2], pool.hash(Idx::STR));
    } else {
        panic!("expected Tuple descriptor, got {desc:?}");
    }
}

#[test]
fn describe_struct() {
    let mut pool = Pool::new();
    let name = Name::new(0, 100);
    let f1 = Name::new(0, 101);
    let f2 = Name::new(0, 102);
    let struct_type = pool.struct_type(name, &[(f1, Idx::INT), (f2, Idx::STR)]);
    let desc = pool.describe(struct_type);

    if let TypeDescriptor::Struct {
        name: n,
        field_names,
        field_type_hashes,
    } = desc
    {
        assert_eq!(n, name);
        assert_eq!(field_names, vec![f1, f2]);
        assert_eq!(field_type_hashes.len(), 2);
        assert_eq!(field_type_hashes[0], pool.hash(Idx::INT));
        assert_eq!(field_type_hashes[1], pool.hash(Idx::STR));
    } else {
        panic!("expected Struct descriptor, got {desc:?}");
    }
}

#[test]
fn describe_enum() {
    let mut pool = Pool::new();
    let name = Name::new(0, 200);
    let v1_name = Name::new(0, 201);
    let v2_name = Name::new(0, 202);

    let enum_type = pool.enum_type(
        name,
        &[
            EnumVariant {
                name: v1_name,
                field_types: vec![],
            },
            EnumVariant {
                name: v2_name,
                field_types: vec![Idx::INT],
            },
        ],
    );
    let desc = pool.describe(enum_type);

    if let TypeDescriptor::Enum { name: n, variants } = desc {
        assert_eq!(n, name);
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].name, v1_name);
        assert!(variants[0].field_type_hashes.is_empty());
        assert_eq!(variants[1].name, v2_name);
        assert_eq!(variants[1].field_type_hashes.len(), 1);
        assert_eq!(variants[1].field_type_hashes[0], pool.hash(Idx::INT));
    } else {
        panic!("expected Enum descriptor, got {desc:?}");
    }
}

#[test]
fn describe_scheme() {
    let mut pool = Pool::new();
    let body = pool.function(&[Idx::INT], Idx::BOOL);
    let scheme = pool.scheme(&[0, 1], body);
    let desc = pool.describe(scheme);

    if let TypeDescriptor::Scheme {
        var_count,
        var_ids,
        body_hash,
    } = desc
    {
        assert_eq!(var_count, 2);
        assert_eq!(var_ids, vec![0, 1]);
        assert_eq!(body_hash, pool.hash(body));
    } else {
        panic!("expected Scheme descriptor, got {desc:?}");
    }
}

#[test]
fn describe_named() {
    let mut pool = Pool::new();
    let name = Name::new(0, 300);
    let named = pool.named(name);
    let desc = pool.describe(named);

    assert_eq!(desc, TypeDescriptor::Named { name });
}

#[test]
fn describe_applied() {
    let mut pool = Pool::new();
    let name = Name::new(0, 400);
    let applied = pool.applied(name, &[Idx::INT, Idx::STR]);
    let desc = pool.describe(applied);

    if let TypeDescriptor::Applied {
        name: n,
        arg_hashes,
    } = desc
    {
        assert_eq!(n, name);
        assert_eq!(arg_hashes.len(), 2);
        assert_eq!(arg_hashes[0], pool.hash(Idx::INT));
        assert_eq!(arg_hashes[1], pool.hash(Idx::STR));
    } else {
        panic!("expected Applied descriptor, got {desc:?}");
    }
}

/// The key integration test from the plan exit criteria:
/// `describe_transitive(List<Map<str, int>>)` should produce 4 descriptors
/// in topological order: int, str, Map<str,int>, List<Map<str,int>>.
#[test]
fn describe_transitive_list_of_map() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_map = pool.list(map_str_int);

    let descriptors = pool.describe_transitive(list_map);

    // Should have 4 unique types: int, str, Map<str,int>, List<Map<str,int>>
    assert_eq!(
        descriptors.len(),
        4,
        "expected 4 descriptors, got {}",
        descriptors.len()
    );

    // Topological order: leaves first
    // The first two should be primitives (int and str, order may vary)
    let primitive_count = descriptors
        .iter()
        .take(2)
        .filter(|(_, d)| matches!(d, TypeDescriptor::Primitive(_)))
        .count();
    assert_eq!(
        primitive_count, 2,
        "first 2 descriptors should be primitives"
    );

    // Third should be Map<str, int>
    assert!(
        matches!(
            &descriptors[2].1,
            TypeDescriptor::TwoChild { tag: Tag::Map, .. }
        ),
        "third descriptor should be Map, got {:?}",
        descriptors[2].1
    );

    // Fourth should be List<Map<str, int>>
    assert!(
        matches!(
            &descriptors[3].1,
            TypeDescriptor::Container { tag: Tag::List, .. }
        ),
        "fourth descriptor should be List, got {:?}",
        descriptors[3].1
    );

    // Verify hashes match pool's hashes
    assert_eq!(descriptors[2].0, pool.hash(map_str_int));
    assert_eq!(descriptors[3].0, pool.hash(list_map));
}

/// Verify deduplication: shared subtypes appear only once.
#[test]
fn describe_transitive_deduplicates() {
    let mut pool = Pool::new();
    // (int, int) -> int — int appears 3 times in the type, but should appear once
    let fn_type = pool.function(&[Idx::INT, Idx::INT], Idx::INT);

    let descriptors = pool.describe_transitive(fn_type);

    // Should have exactly 2: int, then (int, int) -> int
    assert_eq!(
        descriptors.len(),
        2,
        "expected 2 descriptors, got {}",
        descriptors.len()
    );
    assert!(matches!(
        &descriptors[0].1,
        TypeDescriptor::Primitive(Tag::Int)
    ));
    assert!(matches!(&descriptors[1].1, TypeDescriptor::Function { .. }));
}

/// Verify topological ordering: child hashes in a descriptor reference
/// types that appear earlier in the sequence.
#[test]
fn describe_transitive_topological_order() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let opt_list = pool.option(list_int);

    let descriptors = pool.describe_transitive(opt_list);

    // Should be: int, List<int>, Option<List<int>>
    assert_eq!(descriptors.len(), 3);

    // Build hash→position map
    let hash_positions: std::collections::HashMap<u64, usize> = descriptors
        .iter()
        .enumerate()
        .map(|(i, (h, _))| (*h, i))
        .collect();

    // For each descriptor, all referenced child hashes should be at earlier positions
    for (pos, (_, desc)) in descriptors.iter().enumerate() {
        let child_hashes: Vec<u64> = match desc {
            TypeDescriptor::Container { child_hash, .. } => vec![*child_hash],
            TypeDescriptor::TwoChild {
                child1_hash,
                child2_hash,
                ..
            } => vec![*child1_hash, *child2_hash],
            TypeDescriptor::Function {
                param_hashes,
                return_hash,
            } => {
                let mut h = param_hashes.clone();
                h.push(*return_hash);
                h
            }
            _ => vec![],
        };

        for child_hash in child_hashes {
            let child_pos = hash_positions[&child_hash];
            assert!(
                child_pos < pos,
                "child at position {child_pos} should appear before parent at position {pos}"
            );
        }
    }
}

/// Verify that `describe()` round-trips with hash verification:
/// the descriptor's child hashes match the Pool's stored hashes.
#[test]
fn describe_child_hashes_match_pool_hashes() {
    let mut pool = Pool::new();

    // Build a moderately complex type: (str, Map<int, bool>) -> Option<float>
    let map_int_bool = pool.map(Idx::INT, Idx::BOOL);
    let tup = pool.tuple(&[Idx::STR, map_int_bool]);
    let opt_float = pool.option(Idx::FLOAT);
    let fn_type = pool.function(&[tup], opt_float);

    let descriptors = pool.describe_transitive(fn_type);

    // Every descriptor's hash should match a Pool-stored hash
    for (hash, _) in &descriptors {
        assert!(
            pool.lookup_by_hash(*hash).is_some(),
            "descriptor hash 0x{hash:016x} not found in pool"
        );
    }
}

// === Reconstruction Tests ===

/// Key exit criteria test: reconstruct (Map<str, List<int>>) -> Option<bool>
/// in a fresh pool and verify all hashes match the original.
#[test]
fn reconstruct_complex_function_in_new_pool() {
    let mut source = Pool::new();

    // Build: (Map<str, List<int>>) -> Option<bool>
    let list_int = source.list(Idx::INT);
    let map_str_listint = source.map(Idx::STR, list_int);
    let opt_bool = source.option(Idx::BOOL);
    let fn_type = source.function(&[map_str_listint], opt_bool);

    // Generate descriptors
    let descriptors = source.describe_transitive(fn_type);

    // Reconstruct in a fresh pool
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    // The top-level function type should be reconstructable
    let fn_hash = source.hash(fn_type);
    let reconstructed = hash_to_idx[&fn_hash];

    // The hash in the new pool must match the original
    assert_eq!(
        target.hash(reconstructed),
        fn_hash,
        "reconstructed function type hash mismatch"
    );

    // All intermediate types should also match
    for (hash, _) in &descriptors {
        let idx = hash_to_idx[hash];
        assert_eq!(
            target.hash(idx),
            *hash,
            "hash mismatch for type 0x{hash:016x}"
        );
    }
}

/// Reconstruction skips types already in the pool.
#[test]
fn reconstruct_skips_existing() {
    let mut source = Pool::new();
    let list_int = source.list(Idx::INT);
    let descriptors = source.describe_transitive(list_int);

    // Target already has the same types (fresh pool has primitives pre-interned)
    let mut target = Pool::new();
    let pre_existing_list = target.list(Idx::INT);
    let pre_existing_hash = target.hash(pre_existing_list);

    let hash_to_idx = target.reconstruct(&descriptors);

    // The reconstructed Idx should be the same as the pre-existing one
    let list_hash = source.hash(list_int);
    assert_eq!(list_hash, pre_existing_hash);
    assert_eq!(hash_to_idx[&list_hash], pre_existing_list);
}

/// Round-trip: describe → reconstruct → describe again → descriptors match.
#[test]
fn roundtrip_describe_reconstruct_describe() {
    let mut source = Pool::new();

    // Build a struct type
    let name = Name::new(0, 100);
    let f1 = Name::new(0, 101);
    let f2 = Name::new(0, 102);
    let struct_type = source.struct_type(name, &[(f1, Idx::INT), (f2, Idx::STR)]);
    let list_of_struct = source.list(struct_type);

    // Describe
    let descriptors = source.describe_transitive(list_of_struct);

    // Reconstruct
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    // Describe again from the reconstructed pool
    let reconstructed_root = hash_to_idx[&source.hash(list_of_struct)];
    let descriptors2 = target.describe_transitive(reconstructed_root);

    // Descriptor sequences should be identical
    assert_eq!(descriptors.len(), descriptors2.len());
    for ((h1, d1), (h2, d2)) in descriptors.iter().zip(descriptors2.iter()) {
        assert_eq!(h1, h2, "hash mismatch in round-trip");
        assert_eq!(d1, d2, "descriptor mismatch in round-trip");
    }
}

/// Reconstruct an enum type with variants.
#[test]
fn reconstruct_enum() {
    use crate::pool::EnumVariant;

    let mut source = Pool::new();
    let name = Name::new(0, 200);
    let v1 = Name::new(0, 201);
    let v2 = Name::new(0, 202);
    let enum_type = source.enum_type(
        name,
        &[
            EnumVariant {
                name: v1,
                field_types: vec![],
            },
            EnumVariant {
                name: v2,
                field_types: vec![Idx::INT, Idx::STR],
            },
        ],
    );

    let descriptors = source.describe_transitive(enum_type);
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    let enum_hash = source.hash(enum_type);
    let reconstructed = hash_to_idx[&enum_hash];
    assert_eq!(target.hash(reconstructed), enum_hash);

    // Verify structure: name and variant count
    assert_eq!(target.enum_name(reconstructed), name);
    let variants = target.enum_variants(reconstructed);
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].0, v1);
    assert!(variants[0].1.is_empty());
    assert_eq!(variants[1].0, v2);
    assert_eq!(variants[1].1.len(), 2);
}

/// Reconstruct a tuple type.
#[test]
fn reconstruct_tuple() {
    let mut source = Pool::new();
    let tup = source.tuple(&[Idx::INT, Idx::BOOL, Idx::STR]);

    let descriptors = source.describe_transitive(tup);
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    let tup_hash = source.hash(tup);
    let reconstructed = hash_to_idx[&tup_hash];
    assert_eq!(target.hash(reconstructed), tup_hash);
    assert_eq!(target.tuple_elem_count(reconstructed), 3);
}

/// Reconstruct a scheme (quantified type).
#[test]
fn reconstruct_scheme() {
    let mut source = Pool::new();
    let body = source.function(&[Idx::INT], Idx::BOOL);
    let scheme = source.scheme(&[1, 2], body);

    let descriptors = source.describe_transitive(scheme);
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    let scheme_hash = source.hash(scheme);
    let reconstructed = hash_to_idx[&scheme_hash];
    assert_eq!(target.hash(reconstructed), scheme_hash);
}

/// Reconstruct a named type.
#[test]
fn reconstruct_named() {
    let mut source = Pool::new();
    let name = Name::new(0, 300);
    let named = source.named(name);

    let descriptors = source.describe_transitive(named);
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    let named_hash = source.hash(named);
    let reconstructed = hash_to_idx[&named_hash];
    assert_eq!(target.hash(reconstructed), named_hash);
    assert_eq!(target.named_name(reconstructed), name);
}

/// Reconstruct an applied generic type.
#[test]
fn reconstruct_applied() {
    let mut source = Pool::new();
    let name = Name::new(0, 400);
    let applied = source.applied(name, &[Idx::INT, Idx::STR]);

    let descriptors = source.describe_transitive(applied);
    let mut target = Pool::new();
    let hash_to_idx = target.reconstruct(&descriptors);

    let applied_hash = source.hash(applied);
    let reconstructed = hash_to_idx[&applied_hash];
    assert_eq!(target.hash(reconstructed), applied_hash);
    assert_eq!(target.applied_name(reconstructed), name);
    assert_eq!(target.applied_arg_count(reconstructed), 2);
}
