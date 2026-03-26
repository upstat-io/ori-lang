//! Tests for `ori_repr` types.

use crate::enum_repr::{EnumTag, VariantRepr};
use crate::escape::EscapeInfo;
use crate::plan::{DecisionReason, DecisionSource, NarrowingPolicy, ReprDecision};
use crate::range::ValueRange;
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
use crate::ReprAttribute;
use crate::ReprPlan;

use ori_ir::Name;

// ── IntWidth / FloatWidth ───────────────────────────────────────────

#[test]
fn int_width_sizes() {
    assert_eq!(IntWidth::I8.size_bytes(), 1);
    assert_eq!(IntWidth::I16.size_bytes(), 2);
    assert_eq!(IntWidth::I32.size_bytes(), 4);
    assert_eq!(IntWidth::I64.size_bytes(), 8);
}

#[test]
fn int_width_alignment_matches_size() {
    for width in [IntWidth::I8, IntWidth::I16, IntWidth::I32, IntWidth::I64] {
        assert_eq!(width.alignment(), width.size_bytes());
    }
}

#[test]
fn float_width_sizes() {
    assert_eq!(FloatWidth::F32.size_bytes(), 4);
    assert_eq!(FloatWidth::F64.size_bytes(), 8);
}

#[test]
fn float_width_alignment_matches_size() {
    for width in [FloatWidth::F32, FloatWidth::F64] {
        assert_eq!(width.alignment(), width.size_bytes());
    }
}

// ── MachineRepr ─────────────────────────────────────────────────────

#[test]
fn machine_repr_int_canonical() {
    let repr = MachineRepr::Int {
        width: IntWidth::I64,
        signed: true,
    };
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        }
    );
}

#[test]
fn machine_repr_clone_eq() {
    let repr = MachineRepr::Float {
        width: FloatWidth::F64,
    };
    let cloned = repr.clone();
    assert_eq!(repr, cloned);
}

#[test]
fn machine_repr_stack_promoted() {
    let inner = MachineRepr::Int {
        width: IntWidth::I32,
        signed: true,
    };
    let promoted = MachineRepr::StackPromoted {
        inner: Box::new(inner.clone()),
        had_rc: true,
    };
    if let MachineRepr::StackPromoted { inner: i, had_rc } = &promoted {
        assert_eq!(i.as_ref(), &inner);
        assert!(had_rc);
    } else {
        panic!("expected StackPromoted");
    }
}

// ── FatRepr ─────────────────────────────────────────────────────────

#[test]
fn fat_repr_str_vs_collection() {
    let str_repr = FatRepr::Str;
    let col_repr = FatRepr::Collection {
        element_repr: Box::new(MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }),
    };
    assert_ne!(str_repr, col_repr);
}

// ── ClosureRepr ─────────────────────────────────────────────────────

#[test]
fn closure_repr_basic() {
    let closure = ClosureRepr {
        params: vec![MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }],
        ret: Box::new(MachineRepr::Bool),
    };
    assert_eq!(closure.params.len(), 1);
    assert_eq!(*closure.ret, MachineRepr::Bool);
}

// ── StructRepr / FieldRepr ──────────────────────────────────────────

#[test]
fn struct_repr_empty() {
    let s = StructRepr {
        fields: vec![],
        size: 0,
        align: 1,
        trivial: true,
    };
    assert!(s.trivial);
    assert!(s.fields.is_empty());
}

#[test]
fn field_repr_preserves_original_index() {
    let field = FieldRepr {
        name: Name::new(0, 42),
        original_index: 3,
        offset: 16,
        repr: MachineRepr::Bool,
    };
    assert_eq!(field.original_index, 3);
    assert_eq!(field.offset, 16);
}

// ── TupleRepr ───────────────────────────────────────────────────────

#[test]
fn tuple_repr_two_elements() {
    let t = TupleRepr {
        elements: vec![
            FieldRepr {
                name: Name::new(0, 0),
                original_index: 0,
                offset: 0,
                repr: MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
            },
            FieldRepr {
                name: Name::new(0, 1),
                original_index: 1,
                offset: 8,
                repr: MachineRepr::Bool,
            },
        ],
        size: 16,
        align: 8,
        trivial: true,
    };
    assert_eq!(t.elements.len(), 2);
    assert!(t.trivial);
}

// ── RcRepr ──────────────────────────────────────────────────────────

#[test]
fn rc_repr_default_canonical() {
    let rc = RcRepr {
        rc_width: IntWidth::I64,
        atomic: true,
        inner: Box::new(MachineRepr::Struct(StructRepr {
            fields: vec![],
            size: 0,
            align: 1,
            trivial: true,
        })),
        stack_promotable: false,
    };
    assert!(rc.atomic);
    assert!(!rc.stack_promotable);
    assert_eq!(rc.rc_width, IntWidth::I64);
}

// ── EnumRepr / EnumTag / VariantRepr ────────────────────────────────

#[test]
fn enum_tag_explicit() {
    let tag = EnumTag::Explicit {
        width: IntWidth::I64,
    };
    assert_eq!(
        tag,
        EnumTag::Explicit {
            width: IntWidth::I64
        }
    );
}

#[test]
fn enum_tag_niche() {
    let tag = EnumTag::Niche {
        field_index: 0,
        niche_value: 0,
    };
    if let EnumTag::Niche {
        field_index,
        niche_value,
    } = tag
    {
        assert_eq!(field_index, 0);
        assert_eq!(niche_value, 0);
    } else {
        panic!("expected Niche");
    }
}

#[test]
fn variant_repr_unit_is_not_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 10),
        fields: vec![],
        size: 0,
        alignment: 1,
    };
    assert!(!v.is_pointer());
}

#[test]
fn variant_repr_single_fat_pointer_is_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 11),
        fields: vec![MachineRepr::FatPointer(FatRepr::Str)],
        size: 24,
        alignment: 8,
    };
    assert!(v.is_pointer());
}

#[test]
fn variant_repr_single_opaque_is_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 12),
        fields: vec![MachineRepr::OpaquePtr],
        size: 8,
        alignment: 8,
    };
    assert!(v.is_pointer());
}

#[test]
fn variant_repr_scalar_not_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 13),
        fields: vec![MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }],
        size: 8,
        alignment: 8,
    };
    assert!(!v.is_pointer());
}

#[test]
fn variant_repr_two_fields_not_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 14),
        fields: vec![MachineRepr::OpaquePtr, MachineRepr::Bool],
        size: 16,
        alignment: 8,
    };
    assert!(!v.is_pointer());
}

// ── Placeholder types ───────────────────────────────────────────────

#[test]
fn value_range_placeholder_exists() {
    // Verify the placeholder type compiles — replaced by §03.
    assert_eq!(std::mem::size_of::<ValueRange>(), 0);
}

#[test]
fn escape_info_placeholder_exists() {
    // Verify the placeholder type compiles — replaced by §08.
    assert_eq!(std::mem::size_of::<EscapeInfo>(), 0);
}

// ── Semantic Pin Tests ──────────────────────────────────────────────

/// Semantic pin: canonical int MUST be I64 signed.
/// This test fails if the default is changed.
#[test]
fn semantic_pin_canonical_int_is_i64_signed() {
    let repr = MachineRepr::Int {
        width: IntWidth::I64,
        signed: true,
    };
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "canonical int must be I64 signed — changing this breaks semantic equivalence"
    );
}

/// Semantic pin: canonical float MUST be F64.
#[test]
fn semantic_pin_canonical_float_is_f64() {
    let repr = MachineRepr::Float {
        width: FloatWidth::F64,
    };
    assert_eq!(
        repr,
        MachineRepr::Float {
            width: FloatWidth::F64
        },
        "canonical float must be F64 — changing this breaks semantic equivalence"
    );
}

// ── Canonical Mapping Tests ─────────────────────────────────────────

use crate::canonical::{canonical, canonical_cached};
use ori_types::{Idx, Pool};

/// Test canonical mapping for all 12 primitive types.
#[test]
fn canonical_primitives() {
    let pool = Pool::new();

    assert_eq!(
        canonical(&pool, Idx::INT),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        }
    );
    assert_eq!(
        canonical(&pool, Idx::FLOAT),
        MachineRepr::Float {
            width: FloatWidth::F64
        }
    );
    assert_eq!(canonical(&pool, Idx::BOOL), MachineRepr::Bool);
    assert_eq!(
        canonical(&pool, Idx::STR),
        MachineRepr::FatPointer(FatRepr::Str)
    );
    assert_eq!(canonical(&pool, Idx::CHAR), MachineRepr::Char);
    assert_eq!(canonical(&pool, Idx::BYTE), MachineRepr::Byte);
    assert_eq!(canonical(&pool, Idx::UNIT), MachineRepr::Unit);
    assert_eq!(canonical(&pool, Idx::NEVER), MachineRepr::Never);
    assert_eq!(canonical(&pool, Idx::DURATION), MachineRepr::Duration);
    assert_eq!(canonical(&pool, Idx::SIZE), MachineRepr::Size);
    assert_eq!(canonical(&pool, Idx::ORDERING), MachineRepr::Ordering);
}

/// Semantic pin: canonical(Int) MUST return I64 signed.
#[test]
fn semantic_pin_canonical_int_mapping() {
    let pool = Pool::new();
    assert_eq!(
        canonical(&pool, Idx::INT),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "canonical(Int) must be I64 signed — changing breaks semantic equivalence"
    );
}

/// Semantic pin: canonical(Float) MUST return F64.
#[test]
fn semantic_pin_canonical_float_mapping() {
    let pool = Pool::new();
    assert_eq!(
        canonical(&pool, Idx::FLOAT),
        MachineRepr::Float {
            width: FloatWidth::F64
        },
        "canonical(Float) must be F64 — changing breaks semantic equivalence"
    );
}

/// Test canonical mapping for List<int> — fat pointer with element repr.
#[test]
fn canonical_list_int() {
    let mut pool = Pool::new();
    let list_idx = pool.list(Idx::INT);
    let repr = canonical(&pool, list_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            })
        })
    );
}

/// Test canonical mapping for Set<str> — fat pointer.
#[test]
fn canonical_set_str() {
    let mut pool = Pool::new();
    let set_idx = pool.set(Idx::STR);
    let repr = canonical(&pool, set_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::FatPointer(FatRepr::Str))
        })
    );
}

/// Test canonical mapping for Map<str, int> — retains both key and value reprs.
#[test]
fn canonical_map() {
    let mut pool = Pool::new();
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let repr = canonical(&pool, map_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Map {
            key_repr: Box::new(MachineRepr::FatPointer(FatRepr::Str)),
            value_repr: Box::new(MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }),
        })
    );
}

/// Test canonical mapping for Range — always {i64, i64, i64, i64}.
#[test]
fn canonical_range() {
    let mut pool = Pool::new();
    let range_idx = pool.range(Idx::INT);
    assert_eq!(canonical(&pool, range_idx), MachineRepr::Range);
}

/// Test canonical mapping for Iterator — opaque pointer.
#[test]
fn canonical_iterator() {
    let mut pool = Pool::new();
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(canonical(&pool, iter_idx), MachineRepr::UnmanagedPtr);
}

/// Test canonical mapping for Channel — opaque pointer.
#[test]
fn canonical_channel() {
    let mut pool = Pool::new();
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(canonical(&pool, chan_idx), MachineRepr::OpaquePtr);
}

/// Test canonical mapping for Option<int> — 2-variant enum.
#[test]
fn canonical_option_int() {
    let mut pool = Pool::new();
    let opt_idx = pool.option(Idx::INT);
    let repr = canonical(&pool, opt_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2, "Option should have 2 variants");
        assert_eq!(
            e.tag,
            EnumTag::Explicit {
                width: IntWidth::I64
            }
        );
        // None variant has no fields
        assert!(e.variants[0].fields.is_empty());
        // Some variant has one field = Int
        assert_eq!(e.variants[1].fields.len(), 1);
        assert_eq!(
            e.variants[1].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
    } else {
        panic!("expected Enum for Option<int>, got {repr:?}");
    }
}

/// Test canonical mapping for Result<int, str> — 2-variant enum.
#[test]
fn canonical_result() {
    let mut pool = Pool::new();
    let result_idx = pool.result(Idx::INT, Idx::STR);
    let repr = canonical(&pool, result_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2, "Result should have 2 variants");
        // Ok variant has Int
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        // Err variant has Str
        assert_eq!(
            e.variants[1].fields[0],
            MachineRepr::FatPointer(FatRepr::Str)
        );
    } else {
        panic!("expected Enum for Result<int, str>, got {repr:?}");
    }
}

/// Test canonical mapping for Function (int) -> bool.
#[test]
fn canonical_function() {
    let mut pool = Pool::new();
    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, fn_idx);
    if let MachineRepr::Closure(ref c) = repr {
        assert_eq!(c.params.len(), 1);
        assert_eq!(
            c.params[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(*c.ret, MachineRepr::Bool);
    } else {
        panic!("expected Closure for function, got {repr:?}");
    }
}

/// Test canonical mapping for Tuple (int, bool).
#[test]
fn canonical_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.elements.len(), 2);
        assert_eq!(
            t.elements[0].repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(t.elements[1].repr, MachineRepr::Bool);
        assert!(t.trivial, "tuple of int and bool should be trivial");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Test canonical mapping for Tuple with non-trivial element.
#[test]
fn canonical_tuple_nontrivial() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::STR);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert!(!t.trivial, "(int, str) should NOT be trivial");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Test canonical mapping for Struct with named fields.
#[test]
fn canonical_struct() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, name_x);
        assert_eq!(s.fields[1].name, name_y);
        assert_eq!(s.fields[0].original_index, 0);
        assert_eq!(s.fields[1].original_index, 1);
        assert!(s.trivial, "struct of int and float should be trivial");
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Test canonical mapping for Enum with variants.
#[test]
fn canonical_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let a_name = Name::new(0, 301);
    let b_name = Name::new(0, 302);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: a_name,
                field_types: vec![],
            },
            EnumVariant {
                name: b_name,
                field_types: vec![Idx::INT],
            },
        ],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(
            e.tag,
            EnumTag::Explicit {
                width: IntWidth::I64
            }
        );
        // First variant (A) is unit — no fields
        assert!(e.variants[0].fields.is_empty());
        // Second variant (B) has one Int field
        assert_eq!(e.variants[1].fields.len(), 1);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// Test that unresolved Var returns None (not panic).
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_var() {
    let mut pool = Pool::new();
    let var_idx = pool.fresh_var();
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, var_idx, &mut cache).is_none(),
        "Var must return None, not panic"
    );
}

/// Test that `BoundVar` returns None — constructs a real `BoundVar` via `pool.intern`.
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_bound_var() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let bound_var_idx = pool.intern(Tag::BoundVar, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, bound_var_idx, &mut cache).is_none(),
        "BoundVar must return None, not panic"
    );
}

/// Test that `RigidVar` returns None.
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_rigid_var() {
    let mut pool = Pool::new();
    let rigid = pool.rigid_var(Name::new(0, 999));
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, rigid, &mut cache).is_none(),
        "RigidVar must return None, not panic"
    );
}

/// Test that Error type returns None (should not reach codegen).
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_error() {
    let pool = Pool::new();
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, Idx::ERROR, &mut cache).is_none(),
        "Error must return None, not panic"
    );
}

/// §01.5: Scheme type returns None (should never reach codegen).
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_scheme() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let scheme_idx = pool.scheme(&[0], Idx::INT);
    assert_eq!(pool.tag(scheme_idx), Tag::Scheme);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, scheme_idx, &mut cache).is_none(),
        "Scheme must return None, not panic"
    );
}

/// §01.5: Infer type returns None (should never reach codegen).
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_infer() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let infer_idx = pool.intern(Tag::Infer, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, infer_idx, &mut cache).is_none(),
        "Infer must return None, not panic"
    );
}

/// §01.5: Named→Int resolves to same canonical as Int directly.
#[test]
fn canonical_named_resolves_to_int() {
    let mut pool = Pool::new();
    let named_idx = pool.named(Name::new(0, 42));
    pool.set_resolution(named_idx, Idx::INT);

    let repr = canonical(&pool, named_idx);
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Named→Int must resolve to same repr as Int"
    );
}

/// §01.5: Alias chain A = B = int resolves to Int.
#[test]
fn canonical_alias_chain_resolves() {
    let mut pool = Pool::new();
    // A is a Named type
    let a_idx = pool.named(Name::new(0, 100));
    // B is another Named type
    let b_idx = pool.named(Name::new(0, 200));
    // A → B → Int
    pool.set_resolution(a_idx, b_idx);
    pool.set_resolution(b_idx, Idx::INT);

    let repr = canonical(&pool, a_idx);
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Named chain A→B→Int must resolve to Int"
    );
}

// ── ABI Layout Tests ──────────────────────────────────────────────

/// Semantic pin: (int, bool) must be 16 bytes with ABI padding, not 9.
/// int (8 bytes) at offset 0, bool (1 byte) at offset 8, 7 bytes trailing
/// padding to reach struct alignment of 8.
#[test]
fn canonical_tuple_abi_size() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 16, "(int, bool) must be 16 bytes with ABI padding");
        assert_eq!(t.align, 8, "(int, bool) alignment must be 8");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// (bool, int) must also be 16 bytes: bool at offset 0, 7 bytes padding,
/// int at offset 8, total 16.
#[test]
fn canonical_tuple_abi_size_reversed() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::BOOL, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 16, "(bool, int) must be 16 bytes with ABI padding");
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// (bool, bool) is 2 bytes with alignment 1 — no padding needed.
#[test]
fn canonical_tuple_no_padding_needed() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::BOOL, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 2, "(bool, bool) is 2 bytes — no padding");
        assert_eq!(t.align, 1);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Struct {x: int, y: float} — both 8-byte aligned, no internal padding,
/// total 16 bytes.
#[test]
fn canonical_struct_abi_size() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.size, 16, "struct(int, float) must be 16 bytes");
        assert_eq!(s.align, 8);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Struct {a: bool, b: int} — bool at 0, 7 bytes padding, int at 8, total 16.
#[test]
fn canonical_struct_abi_padding() {
    let mut pool = Pool::new();
    let name_a = Name::new(0, 100);
    let name_b = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_a, Idx::BOOL), (name_b, Idx::INT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(
            s.size, 16,
            "struct(bool, int) must be 16 bytes with ABI padding"
        );
        assert_eq!(s.align, 8);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Map<str, int> semantic pin: must retain both key and value representations.
#[test]
fn canonical_map_retains_value_repr() {
    let mut pool = Pool::new();
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let repr = canonical(&pool, map_idx);
    if let MachineRepr::FatPointer(FatRepr::Map {
        ref key_repr,
        ref value_repr,
    }) = repr
    {
        assert_eq!(**key_repr, MachineRepr::FatPointer(FatRepr::Str));
        assert_eq!(
            **value_repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
    } else {
        panic!("expected FatPointer(Map), got {repr:?}");
    }
}

// ── TPR-01-015: Cycle Detection for Recursive Types ─────────────

/// Recursive enum `type Tree = Leaf(int) | Node(Tree, Tree)` must not
/// stack overflow. Recursive positions yield `RcPointer`.
#[test]
fn canonical_recursive_enum_no_stack_overflow() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let tree_name = Name::new(0, 400);
    let leaf_name = Name::new(0, 401);
    let node_name = Name::new(0, 402);

    // Forward reference for Tree
    let tree_named = pool.named(tree_name);

    let tree_enum = pool.enum_type(
        tree_name,
        &[
            EnumVariant {
                name: leaf_name,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: node_name,
                field_types: vec![tree_named, tree_named],
            },
        ],
    );

    // Link Named → Enum
    pool.set_resolution(tree_named, tree_enum);

    // Must not infinite loop
    let repr = canonical(&pool, tree_enum);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2);
        // Leaf variant: one Int field
        assert_eq!(e.variants[0].fields.len(), 1);
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        // Node variant: two RcPointer fields (recursive positions)
        assert_eq!(e.variants[1].fields.len(), 2);
        assert!(
            matches!(e.variants[1].fields[0], MachineRepr::RcPointer(_)),
            "recursive field must be RcPointer, got {:?}",
            e.variants[1].fields[0]
        );
        assert!(
            matches!(e.variants[1].fields[1], MachineRepr::RcPointer(_)),
            "recursive field must be RcPointer, got {:?}",
            e.variants[1].fields[1]
        );
    } else {
        panic!("expected Enum for recursive Tree, got {repr:?}");
    }
}

/// Semantic pin: recursive type MUST return `RcPointer` at recursive position,
/// not `OpaquePtr` or infinite recursion.
#[test]
fn semantic_pin_recursive_field_is_rc_pointer() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let list_name = Name::new(0, 500);
    let nil_name = Name::new(0, 501);
    let cons_name = Name::new(0, 502);
    let list_named = pool.named(list_name);

    // type IntList = Nil | Cons(int, IntList)
    let list_enum = pool.enum_type(
        list_name,
        &[
            EnumVariant {
                name: nil_name,
                field_types: vec![],
            },
            EnumVariant {
                name: cons_name,
                field_types: vec![Idx::INT, list_named],
            },
        ],
    );
    pool.set_resolution(list_named, list_enum);

    let repr = canonical(&pool, list_enum);
    if let MachineRepr::Enum(ref e) = repr {
        // Cons variant's second field is the recursive ref
        let cons = &e.variants[1];
        assert_eq!(cons.fields.len(), 2);
        if let MachineRepr::RcPointer(ref rc) = cons.fields[1] {
            assert_eq!(rc.rc_width, IntWidth::I64);
            assert!(rc.atomic);
            assert!(!rc.stack_promotable);
        } else {
            panic!(
                "recursive field must be RcPointer, got {:?}",
                cons.fields[1]
            );
        }
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// TPR-01-021: Mutual recursion canonical-consistency test.
///
/// `type A = WrapA { b: B }`
/// `type B = WrapB { a: A }`
///
/// `canonical(A)` and `canonical(B)` must each produce consistent representations:
/// mutual recursive fields are `RcPointer` and cached representations are stable.
#[test]
fn canonical_mutual_recursion_consistent() {
    let mut pool = Pool::new();

    // Forward references
    let a_name = Name::new(0, 600);
    let b_name = Name::new(0, 601);
    let a_named = pool.named(a_name);
    let b_named = pool.named(b_name);

    let b_field_name = Name::new(0, 602);
    let a_field_name = Name::new(0, 603);

    // type A = struct { b: B }
    let a_struct = pool.struct_type(a_name, &[(b_field_name, b_named)]);
    // type B = struct { a: A }
    let b_struct = pool.struct_type(b_name, &[(a_field_name, a_named)]);

    pool.set_resolution(a_named, a_struct);
    pool.set_resolution(b_named, b_struct);

    // Compute both via shared cache (simulating populate_canonical)
    let mut cache = rustc_hash::FxHashMap::default();
    let Some(a_repr) = crate::canonical::canonical_cached(&pool, a_struct, &mut cache) else {
        panic!("A should canonicalize");
    };
    let Some(b_repr) = crate::canonical::canonical_cached(&pool, b_struct, &mut cache) else {
        panic!("B should canonicalize");
    };

    // Both should be Struct types
    let MachineRepr::Struct(ref a_s) = a_repr else {
        panic!("expected Struct for A, got {a_repr:?}");
    };
    let MachineRepr::Struct(ref b_s) = b_repr else {
        panic!("expected Struct for B, got {b_repr:?}");
    };

    // A has one field (b), B has one field (a)
    assert_eq!(a_s.fields.len(), 1, "A should have 1 field");
    assert_eq!(b_s.fields.len(), 1, "B should have 1 field");

    // A's B field = full B struct (B is first-visited from A, not a cycle).
    // B's A field = RcPointer (A was being visited when B encountered it).
    assert!(
        matches!(a_s.fields[0].repr, MachineRepr::Struct(_)),
        "A's B field should be full Struct (first visit), got {:?}",
        a_s.fields[0].repr
    );
    assert!(
        matches!(b_s.fields[0].repr, MachineRepr::RcPointer(_)),
        "B's A field should be RcPointer (back-edge), got {:?}",
        b_s.fields[0].repr
    );

    // Key consistency check (TPR-01-021): B nested inside A must equal standalone B.
    // With the shared cache, both resolve to the same representation.
    let b_inside_a = &a_s.fields[0].repr;
    assert_eq!(
        b_inside_a, &b_repr,
        "B nested inside A must equal standalone B (cache consistency)"
    );

    // Semantic pin: calling canonical_cached again returns the same result (cache hit)
    let Some(a_repr2) = crate::canonical::canonical_cached(&pool, a_struct, &mut cache) else {
        panic!("cached A should canonicalize");
    };
    assert_eq!(a_repr, a_repr2, "cached result must be stable");
}

/// TPR-01-047 semantic pin: a struct containing an Error-typed field returns
/// None (not panic) because the child type cannot be canonicalized.
/// This is the key regression test — if the fallible path is reverted to
/// panics, this test detects it.
#[test]
fn canonical_returns_none_for_struct_with_error_child() {
    let mut pool = Pool::new();
    let field_name = Name::new(0, 42);
    // Create a struct with one field of type Error
    let struct_idx = pool.struct_type(Name::new(0, 100), &[(field_name, Idx::ERROR)]);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, struct_idx, &mut cache).is_none(),
        "struct with Error child must return None, not panic"
    );
}

/// TPR-01-047 semantic pin: an Option wrapping an Error type returns None.
#[test]
fn canonical_returns_none_for_option_of_error() {
    let mut pool = Pool::new();
    let option_idx = pool.option(Idx::ERROR);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, option_idx, &mut cache).is_none(),
        "Option<Error> must return None, not panic"
    );
}

/// TPR-01-047 semantic pin: a list of Error-typed elements returns None.
#[test]
fn canonical_returns_none_for_list_of_error() {
    let mut pool = Pool::new();
    let list_idx = pool.list(Idx::ERROR);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, list_idx, &mut cache).is_none(),
        "[Error] must return None, not panic"
    );
}

/// TPR-01-047 semantic pin: `populate_canonical` does not panic on pools
/// that contain Error and type-variable types.
#[test]
fn populate_canonical_no_panics_with_error_types() {
    use crate::plan::NarrowingPolicy;

    let mut pool = Pool::new();
    // Add some valid types
    let _list_int = pool.list(Idx::INT);
    let _option_str = pool.option(Idx::STR);
    // Add some invalid types that should be silently skipped
    let _list_error = pool.list(Idx::ERROR);
    let _var = pool.fresh_var();

    // This must NOT panic — the fix eliminates catch_unwind
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    // Valid types should be populated
    assert!(
        plan.get_repr(Idx::INT).is_some(),
        "Int should have a canonical repr"
    );
}

/// Non-recursive type appearing multiple times is NOT treated as a cycle.
#[test]
fn canonical_non_recursive_repeated_type() {
    let mut pool = Pool::new();
    // (int, int) — int appears twice but is not recursive
    let tuple_idx = pool.pair(Idx::INT, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.elements.len(), 2);
        // Both should be Int, NOT RcPointer
        assert!(
            matches!(t.elements[0].repr, MachineRepr::Int { .. }),
            "repeated non-recursive type must not be RcPointer"
        );
        assert!(
            matches!(t.elements[1].repr, MachineRepr::Int { .. }),
            "repeated non-recursive type must not be RcPointer"
        );
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

// ── TPR-01-005: Unit/Never Zero-Size in Aggregates ──────────────

/// Semantic pin: ((), bool) size = 1 — Unit contributes 0 bytes in aggregates.
#[test]
fn canonical_tuple_unit_zero_sized() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::UNIT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 1,
            "(unit, bool) must be 1 byte — Unit is zero-sized"
        );
        assert_eq!(t.align, 1);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// (bool, (), int) size = 16 — Unit in the middle contributes 0 bytes.
#[test]
fn canonical_tuple_unit_middle() {
    let mut pool = Pool::new();
    let tuple_idx = pool.triple(Idx::BOOL, Idx::UNIT, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        // bool(1) + padding(7) + int(8) = 16. Unit adds 0.
        assert_eq!(
            t.size, 16,
            "(bool, unit, int) must be 16 — Unit contributes 0"
        );
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Struct with Unit field doesn't inflate size.
#[test]
fn canonical_struct_unit_field() {
    let mut pool = Pool::new();
    let name_a = Name::new(0, 100);
    let name_b = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_a, Idx::BOOL), (name_b, Idx::UNIT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.size, 1, "struct(bool, unit) must be 1 byte");
        assert_eq!(s.align, 1);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// `Option<()>` — tag + 0 payload. Size = 8 (just the i64 tag).
#[test]
fn canonical_option_unit_zero_payload() {
    let mut pool = Pool::new();
    let opt_idx = pool.option(Idx::UNIT);
    let repr = canonical(&pool, opt_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(
            e.size, 8,
            "Option<()> must be 8 bytes — tag only, zero payload"
        );
    } else {
        panic!("expected Enum for Option<()>, got {repr:?}");
    }
}

/// Never-typed field contributes 0 bytes in aggregates.
#[test]
fn canonical_tuple_never_zero_sized() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::NEVER);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 8,
            "(int, Never) must be 8 bytes — Never is zero-sized"
        );
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

// ── TPR-01-016: Recursive Triviality for Compound Types ─────────

/// Struct containing a trivial tuple `(int, bool)` must itself be trivial.
#[test]
fn trivial_struct_containing_trivial_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let name_t = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_t, tuple_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing (int, bool) must be trivial — all scalars"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Struct containing a non-trivial tuple `(int, str)` must be non-trivial.
#[test]
fn nontrivial_struct_containing_nontrivial_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::STR);
    let name_t = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_t, tuple_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            !s.trivial,
            "struct containing (int, str) must NOT be trivial — str has RC"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// All-unit enum (like `type Color = Red | Green | Blue`) is trivial.
#[test]
fn trivial_all_unit_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![],
            },
        ],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        let all_trivial = e.variants.iter().all(|v| {
            v.fields.iter().all(|f| {
                !matches!(
                    f,
                    MachineRepr::FatPointer(_)
                        | MachineRepr::RcPointer(_)
                        | MachineRepr::Closure(_)
                        | MachineRepr::OpaquePtr
                )
            })
        });
        assert!(all_trivial, "all-unit enum must be trivial");
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// Enum with scalar payloads `Shape = Circle(float) | Rect(float, float)` is trivial.
#[test]
fn trivial_scalar_payload_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![Idx::FLOAT],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    // Wrap in a struct to test nested triviality
    let name_s = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_s, enum_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing all-scalar enum must be trivial"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

// ── §01.2: ReprDecision Tracking ────────────────────────────────

#[test]
fn repr_plan_set_get_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let decision = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    plan.set_repr(Idx::INT, decision);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
}

#[test]
fn repr_plan_override_returns_second_decision() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let d1 = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    let d2 = ReprDecision {
        source: DecisionSource::IntegerNarrowing,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        },
        reason: DecisionReason::RangeFits {
            range: ValueRange,
            min_width: IntWidth::I32,
        },
    };
    plan.set_repr(Idx::INT, d1);
    plan.set_repr(Idx::INT, d2);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        })
    );
}

#[test]
fn repr_plan_audit_trail_preserves_both_decisions() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let d1 = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    let d2 = ReprDecision {
        source: DecisionSource::IntegerNarrowing,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        },
        reason: DecisionReason::RangeFits {
            range: ValueRange,
            min_width: IntWidth::I32,
        },
    };
    plan.set_repr(Idx::INT, d1);
    plan.set_repr(Idx::INT, d2);
    let audit = plan.dump_audit(&pool);
    assert!(!audit.is_empty());
    // Both entries should be present in order
    assert!(audit.contains("Canonical"), "audit must contain Canonical");
    assert!(
        audit.contains("IntegerNarrowing"),
        "audit must contain IntegerNarrowing"
    );
    // Canonical should appear before IntegerNarrowing (insertion order).
    // Both were asserted present above; Option<usize> ordering is safe here.
    assert!(
        audit.find("Canonical") < audit.find("IntegerNarrowing"),
        "Canonical must appear before IntegerNarrowing in audit trail"
    );
}

#[test]
fn repr_plan_get_unknown_idx_returns_none() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan.get_repr(Idx::INT).is_none());
}

#[test]
fn repr_plan_var_range_no_recorded_ranges_returns_default() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let func = Name::new(0, 1);
    let var = ori_arc::ArcVarId::new(0);
    let range = plan.var_range(func, var);
    assert_eq!(range, ValueRange);
}

#[test]
#[expect(
    clippy::zero_sized_map_values,
    reason = "ValueRange is a placeholder ZST — replaced by §03"
)]
fn repr_plan_set_var_ranges_round_trip_isolated() {
    use rustc_hash::FxHashMap;

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let func_a = Name::new(0, 1);
    let func_b = Name::new(0, 2);
    let var_0 = ori_arc::ArcVarId::new(0);
    let var_1 = ori_arc::ArcVarId::new(1);

    let mut ranges_a = FxHashMap::default();
    ranges_a.insert(var_0, ValueRange);
    plan.set_var_ranges(func_a, ranges_a);

    let mut ranges_b = FxHashMap::default();
    ranges_b.insert(var_1, ValueRange);
    plan.set_var_ranges(func_b, ranges_b);

    // func_a has var_0 but not var_1
    assert_eq!(plan.var_range(func_a, var_0), ValueRange);
    assert_eq!(plan.var_range(func_a, var_1), ValueRange); // default — no panic

    // func_b has var_1 but not var_0
    assert_eq!(plan.var_range(func_b, var_1), ValueRange);
    assert_eq!(plan.var_range(func_b, var_0), ValueRange); // default — no panic
}

#[test]
fn repr_plan_dump_audit_contains_tag_and_source() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_repr(
        Idx::INT,
        ReprDecision {
            source: DecisionSource::Triviality,
            type_idx: Idx::INT,
            repr: MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            reason: DecisionReason::TransitivelyTrivial,
        },
    );
    let audit = plan.dump_audit(&pool);
    assert!(!audit.is_empty());
    assert!(audit.contains("int"), "audit must contain type tag 'int'");
    assert!(
        audit.contains("Triviality"),
        "audit must contain source 'Triviality'"
    );
}

// ── §01.4 Query Interface Default Values ──────────────────────────────

#[test]
fn int_width_default_returns_i64() {
    // §01.4 test: int_width() defaults to I64 when no decision recorded.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(plan.int_width(Idx::INT), IntWidth::I64);
}

#[test]
fn float_width_default_returns_f64() {
    // §01.4 test: float_width() defaults to F64 when no decision recorded.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(plan.float_width(Idx::FLOAT), FloatWidth::F64);
}

#[test]
fn is_trivial_default_returns_false() {
    // §01.4 test: is_trivial() defaults to false when no decision recorded.
    // Safe default — never elides RC it shouldn't.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(
        !plan.is_trivial(Idx::INT),
        "safe default must be non-trivial"
    );
}

#[test]
fn escapes_default_returns_true() {
    // §01.4 test: escapes() defaults to true when no escape info recorded.
    // Safe default — never stack-promotes when unsure.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(
        plan.escapes(Name::new(0, 0), ori_arc::ArcVarId::new(0)),
        "safe default must assume escapes"
    );
}

// ── RC strategy (TPR-01-022, TPR-01-023) ──────────────────────────────

use crate::plan::RcStrategy;

#[test]
fn rc_strategy_default_is_atomic_i64() {
    // Semantic pin: rc_strategy() must return Atomic { I64 } when no decision
    // has been recorded. This is the documented §01 contract — §01 alone
    // causes zero behavioral change.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(
        plan.rc_strategy(Idx::INT),
        RcStrategy::Atomic {
            width: IntWidth::I64,
        },
    );
}

#[test]
fn rc_strategy_default_for_canonical_opaque_ptr() {
    // TPR-01-023: After populate_canonical(), Iterator/Channel are stored as
    // OpaquePtr. rc_strategy() must still return Atomic { I64 } (the safe
    // default) — NOT RcStrategy::None.
    let mut pool = Pool::new();
    let iter_idx = pool.iterator(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    // Simulate populate_canonical() storing an UnmanagedPtr for an iterator type.
    plan.set_repr(
        iter_idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: iter_idx,
            repr: MachineRepr::UnmanagedPtr,
            reason: DecisionReason::Canonical,
        },
    );
    // Must still report Atomic { I64 } — no §09/§10 decision has been made.
    assert_eq!(
        plan.rc_strategy(iter_idx),
        RcStrategy::Atomic {
            width: IntWidth::I64,
        },
    );
}

#[test]
fn set_rc_strategy_preserves_original_repr() {
    // TPR-01-022: set_rc_strategy() must NOT overwrite the type's
    // MachineRepr. The original layout must be preserved for codegen.
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let original_repr = MachineRepr::Struct(StructRepr {
        fields: vec![FieldRepr {
            repr: MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            original_index: 0,
            offset: 0,
            name: Name::new(0, 1),
        }],
        size: 8,
        align: 8,
        trivial: false,
    });
    plan.set_repr(
        Idx::INT,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: Idx::INT,
            repr: original_repr.clone(),
            reason: DecisionReason::Canonical,
        },
    );
    // Set RC strategy — must NOT destroy the struct layout.
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    // The repr must still be the original struct, not OpaquePtr.
    assert_eq!(plan.get_repr(Idx::INT), Some(&original_repr));
}

#[test]
fn set_rc_strategy_write_read_round_trip() {
    // §01.4 test: After set_rc_strategy(idx, RcStrategy::None, ...),
    // rc_strategy(idx) returns RcStrategy::None.
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    assert_eq!(plan.rc_strategy(Idx::INT), RcStrategy::None);
}

#[test]
fn set_rc_strategy_non_atomic_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let strategy = RcStrategy::NonAtomic {
        width: IntWidth::I16,
    };
    plan.set_rc_strategy(Idx::INT, strategy, DecisionSource::ThreadLocal);
    assert_eq!(plan.rc_strategy(Idx::INT), strategy);
}

#[test]
fn set_rc_strategy_atomic_narrow_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let strategy = RcStrategy::Atomic {
        width: IntWidth::I8,
    };
    plan.set_rc_strategy(Idx::INT, strategy, DecisionSource::ArcHeader);
    assert_eq!(plan.rc_strategy(Idx::INT), strategy);
}

#[test]
fn set_rc_strategy_records_audit_entry() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    let audit = plan.dump_audit(&pool);
    assert!(
        audit.contains("Triviality"),
        "audit must contain the RC strategy decision source"
    );
}

// ── §01.3 Pipeline Integration Tests ────────────────────────────────

#[test]
fn compute_repr_plan_populates_primitives() {
    // §01.3 test: compute_repr_plan() populates canonical representations
    // for all 11 non-error primitive types.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    // All 11 non-error primitives should have canonical entries.
    assert!(plan.get_repr(Idx::INT).is_some(), "Int must be populated");
    assert!(
        plan.get_repr(Idx::FLOAT).is_some(),
        "Float must be populated"
    );
    assert!(plan.get_repr(Idx::BOOL).is_some(), "Bool must be populated");
    assert!(plan.get_repr(Idx::STR).is_some(), "Str must be populated");
    assert!(plan.get_repr(Idx::CHAR).is_some(), "Char must be populated");
    assert!(plan.get_repr(Idx::BYTE).is_some(), "Byte must be populated");
    assert!(plan.get_repr(Idx::UNIT).is_some(), "Unit must be populated");
    assert!(
        plan.get_repr(Idx::NEVER).is_some(),
        "Never must be populated"
    );
    assert!(
        plan.get_repr(Idx::DURATION).is_some(),
        "Duration must be populated"
    );
    assert!(plan.get_repr(Idx::SIZE).is_some(), "Size must be populated");
    assert!(
        plan.get_repr(Idx::ORDERING).is_some(),
        "Ordering must be populated"
    );
    // Error type IS populated as Unit (TPR-02-005: trivial sentinel).
    assert!(
        plan.get_repr(Idx::ERROR).is_some(),
        "Error must be populated as Unit (TPR-02-005)"
    );
}

#[test]
fn compute_repr_plan_disabled_policy_skips_stubs() {
    // §01.3 test: NarrowingPolicy::Disabled returns after populate_canonical()
    // without calling any narrowing stubs.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled, &[]);
    // Same primitives should be populated (canonical-only).
    assert!(plan.get_repr(Idx::INT).is_some());
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Disabled);
}

#[test]
fn compute_repr_plan_aggressive_is_default_behavior() {
    // §01.3 test: NarrowingPolicy::Aggressive is the default — building
    // without --no-repr-opt results in Aggressive.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Aggressive);
    // Same primitives, same canonical results — no stubs are active yet.
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
}

#[test]
fn compute_repr_plan_canonical_int_semantic_pin() {
    // §01.3 semantic pin: canonical(Int) must be I64/signed.
    // This test fails if any future change alters the default int width.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }),
        "canonical int must be i64 signed — semantic pin"
    );
}

#[test]
fn compute_repr_plan_zero_behavioral_change_with_disabled() {
    // §01.3 test: identical canonical representations regardless of policy.
    let pool = ori_types::Pool::new();
    let plan_aggressive = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    let plan_disabled = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled, &[]);
    // Both should produce the same canonical repr for every primitive
    // (including ERROR, which is canonicalized as Unit — TPR-02-005).
    for raw in 0..Idx::PRIMITIVE_COUNT {
        let idx = Idx::from_raw(raw);
        assert_eq!(
            plan_aggressive.get_repr(idx),
            plan_disabled.get_repr(idx),
            "canonical repr for primitive {raw} must match regardless of policy"
        );
    }
}

// ── TPR-01-029/030: ORI_NO_REPR_OPT env var value parsing ─────────

#[test]
fn is_env_truthy_accepts_1() {
    assert!(crate::plan::query::is_env_truthy("1"));
}

#[test]
fn is_env_truthy_accepts_true_lowercase() {
    assert!(crate::plan::query::is_env_truthy("true"));
}

#[test]
fn is_env_truthy_accepts_true_uppercase() {
    assert!(crate::plan::query::is_env_truthy("TRUE"));
}

#[test]
fn is_env_truthy_accepts_true_mixed_case() {
    assert!(crate::plan::query::is_env_truthy("True"));
}

#[test]
fn is_env_truthy_accepts_yes_lowercase() {
    assert!(crate::plan::query::is_env_truthy("yes"));
}

#[test]
fn is_env_truthy_accepts_yes_uppercase() {
    assert!(crate::plan::query::is_env_truthy("YES"));
}

#[test]
fn is_env_truthy_rejects_0() {
    assert!(!crate::plan::query::is_env_truthy("0"));
}

#[test]
fn is_env_truthy_rejects_false() {
    assert!(!crate::plan::query::is_env_truthy("false"));
}

#[test]
fn is_env_truthy_rejects_no() {
    assert!(!crate::plan::query::is_env_truthy("no"));
}

#[test]
fn is_env_truthy_rejects_empty() {
    assert!(!crate::plan::query::is_env_truthy(""));
}

#[test]
fn is_env_truthy_rejects_arbitrary() {
    assert!(!crate::plan::query::is_env_truthy("banana"));
}

/// Semantic pin: `NarrowingPolicy::env_disabled()` must use strict
/// value parsing, not mere presence. This test would fail if the
/// implementation reverted to `std::env::var(...).is_ok()`.
#[test]
fn env_disabled_rejects_falsey_values() {
    // Testing the inner `is_env_truthy` function directly — this avoids
    // mutating the process-wide env (which would be racy in parallel tests).
    // The three call sites in oric/ori_llvm all use `NarrowingPolicy::env_disabled()`
    // which delegates to `is_env_truthy`, so verifying the inner function
    // is sufficient. (TPR-01-029)
    assert!(
        !crate::plan::query::is_env_truthy("0"),
        "0 must not enable --no-repr-opt"
    );
    assert!(
        !crate::plan::query::is_env_truthy("false"),
        "false must not enable --no-repr-opt"
    );
    assert!(
        !crate::plan::query::is_env_truthy(""),
        "empty must not enable --no-repr-opt"
    );
    assert!(
        crate::plan::query::is_env_truthy("1"),
        "1 must enable --no-repr-opt"
    );
    assert!(
        crate::plan::query::is_env_truthy("true"),
        "true must enable --no-repr-opt"
    );
}

// ── §01.7: #repr Attribute Integration tests ──────────────────────

#[test]
fn repr_c_stored_and_retrieved() {
    // §01.7: #repr("c") on a struct → ReprAttribute::C in repr_attrs.
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(100), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "#repr(\"c\") must be stored as ReprAttribute::C"
    );
}

#[test]
fn repr_packed_stored_and_retrieved() {
    // §01.7: #repr("packed") → ReprAttribute::Packed stored and retrieved.
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(101), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::Packed),);
}

#[test]
fn repr_transparent_stored_and_retrieved() {
    // §01.7: #repr("transparent") on a single-field struct → ReprAttribute::Transparent.
    let mut pool = ori_types::Pool::new();
    let field = (ori_ir::Name::from_raw(200), Idx::INT);
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(102), &[field]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Transparent)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Transparent),
    );
}

#[test]
fn repr_aligned_stored_and_retrieved() {
    // §01.7: #repr("aligned", 8) → ReprAttribute::Aligned(8).
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(103), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Aligned(8))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::Aligned(8)),);
}

#[test]
fn no_repr_returns_none() {
    // §01.7: Struct with no #repr → repr_attrs has no entry.
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(104), &[]);
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "no #repr → None from query"
    );
}

#[test]
fn repr_c_semantic_pin() {
    // §01.7 semantic pin: #repr("c") stored as ReprAttribute::C.
    // This test establishes the contract: populate_canonical() stores C
    // in repr_attrs, and a subsequent check returns Some(ReprAttribute::C).
    // Fails if the conversion or storage logic is reverted.
    let mut pool = ori_types::Pool::new();
    let name = ori_ir::Name::from_raw(105);
    let f1 = (ori_ir::Name::from_raw(201), Idx::INT);
    let f2 = (ori_ir::Name::from_raw(202), Idx::FLOAT);
    let struct_idx = pool.struct_type(name, &[f1, f2]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::C));
    // Other types should still have None.
    assert_eq!(plan.repr_attr(Idx::INT), None);
}

#[test]
fn repr_c_aligned_stored_and_retrieved() {
    // §01.7: CAligned(16) from merged c + aligned → ReprAttribute::CAligned(16).
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(106), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::CAligned(16))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::CAligned(16)),
        "#repr(\"c\") + #repr(\"aligned\", 16) must be stored as ReprAttribute::CAligned(16)"
    );
}

#[test]
fn repr_convert_c_aligned_roundtrip() {
    // §01.7 semantic pin: CAligned survives the ReprAttrKind → ReprAttribute conversion.
    let kind = ori_ir::ReprAttrKind::CAligned(32);
    let attr = crate::convert_repr_attr_kind(&kind);
    assert_eq!(attr, ReprAttribute::CAligned(32));
}

// ── TPR-01-046: Named-type Idx storage contract ────────────────────

#[test]
fn repr_attr_stored_via_named_idx() {
    // TPR-01-046: The live pipeline stores #repr attrs keyed by the Named Idx
    // from TypeEntry, not by a concrete struct_type Idx. This test pins that
    // the storage and retrieval contract works with Named Idx values, matching
    // the production codegen_pipeline path.
    let mut pool = ori_types::Pool::new();

    // Create a Named Idx (as the type registry and codegen pipeline would produce).
    let named_idx = pool.named(ori_ir::Name::from_raw(500));

    // Store #repr("c") under the Named Idx.
    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Verify retrieval works via the same Named Idx.
    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named Idx must store and retrieve #repr attrs — this is the production path"
    );

    // Also verify that a different Named Idx returns None.
    let other_named = pool.named(ori_ir::Name::from_raw(501));
    assert_eq!(
        plan.repr_attr(other_named),
        None,
        "repr_attr on unrelated Named Idx must return None"
    );
}

#[test]
fn repr_attr_named_vs_struct_idx_independent() {
    // TPR-01-046 semantic pin: Named Idx and struct_type Idx for the same name
    // are DIFFERENT pool entries. A #repr stored on one must NOT be visible on
    // the other. This verifies the storage contract uses exact Idx equality.
    let mut pool = ori_types::Pool::new();
    let name = ori_ir::Name::from_raw(600);

    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[]);

    // Store #repr("packed") on the Named Idx only.
    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::Packed),
        "Named Idx should have the attr"
    );
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "struct_type Idx should NOT have the attr — different pool entry"
    );
}

// ── §01.9: Canonical Representation Tests ──────────────────────────

/// §01.9 Item 4: Named→Struct resolution. Previous tests only covered
/// Named→Int; this verifies Named types pointing to structs resolve
/// through to the struct's representation including field layout.
#[test]
fn canonical_named_resolves_to_struct() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);

    let named_idx = pool.named(Name::new(0, 42));
    pool.set_resolution(named_idx, struct_idx);

    let repr = canonical(&pool, named_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.fields.len(), 2, "Named→Struct must resolve to 2 fields");
        assert_eq!(s.fields[0].name, name_x);
        assert_eq!(s.fields[1].name, name_y);
        assert!(s.trivial, "struct of (int, float) must be trivial");
        assert_eq!(s.size, 16);
    } else {
        panic!("expected Struct for Named→Struct, got {repr:?}");
    }
}

/// §01.9 Item 8 (TPR-01-017): Struct containing an all-unit enum must be
/// trivial. This exercises `is_trivial_repr()` on the `MachineRepr::Enum`
/// path through a wrapper aggregate. A regression in the enum triviality
/// branch would make the struct non-trivial, failing this test.
#[test]
fn trivial_struct_containing_all_unit_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 303),
                field_types: vec![],
            },
        ],
    );
    // Wrap in struct to exercise nested triviality
    let name_e = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_e, enum_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing all-unit enum must be trivial — \
             semantic pin for TPR-01-017"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// §01.9 Item 10: `SelfType` returns None (should never reach codegen).
/// TPR-01-047: canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_self_type() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let self_idx = pool.intern(Tag::SelfType, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, self_idx, &mut cache).is_none(),
        "SelfType must return None, not panic"
    );
}

/// §01.9 Item 11: `FatPointer` structural layout assertion.
/// Both `FatRepr::Str` and `FatRepr::Collection` are `FatPointer` variants
/// that produce `{i64, i64, ptr}` in LLVM. This test verifies the `ori_repr`
/// level structure — the LLVM equivalence is covered by Phase A tests in
/// `ori_llvm` (`try_repr_to_llvm_type` handles both identically).
#[test]
fn fat_pointer_str_and_collection_same_llvm_shape() {
    let mut pool = Pool::new();

    // Str → FatPointer(Str)
    let str_repr = canonical(&pool, Idx::STR);
    assert!(
        matches!(str_repr, MachineRepr::FatPointer(FatRepr::Str)),
        "str must be FatPointer(Str)"
    );

    // [int] → FatPointer(Collection { element_repr: Int })
    let list_idx = pool.list(Idx::INT);
    let list_repr = canonical(&pool, list_idx);
    assert!(
        matches!(
            list_repr,
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "list must be FatPointer(Collection)"
    );

    // {str: int} → FatPointer(Map { key, value })
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let map_repr = canonical(&pool, map_idx);
    assert!(
        matches!(map_repr, MachineRepr::FatPointer(FatRepr::Map { .. })),
        "map must be FatPointer(Map)"
    );

    // Semantic pin: all three are FatPointer variants, ensuring identical
    // LLVM lowering ({i64, i64, ptr}) via try_repr_to_llvm_type.
}

/// §01.9 Item 6: Storage type equivalence — containers and opaque types.
///
/// Covers 7 simple containers + 2 two-child containers + `DoubleEndedIterator`.
/// (Borrowed panics in `canonical()` — not a codegen type.)
#[test]
fn storage_equivalence_containers() {
    let mut pool = Pool::new();

    // Simple containers (7)
    let list_idx = pool.list(Idx::INT);
    assert!(
        matches!(
            canonical(&pool, list_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "List canonical"
    );
    let opt_idx = pool.option(Idx::INT);
    assert!(
        matches!(canonical(&pool, opt_idx), MachineRepr::Enum(_)),
        "Option canonical"
    );
    let set_idx = pool.set(Idx::STR);
    assert!(
        matches!(
            canonical(&pool, set_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "Set canonical"
    );
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(
        canonical(&pool, chan_idx),
        MachineRepr::OpaquePtr,
        "Channel canonical"
    );
    let range_idx = pool.range(Idx::INT);
    assert_eq!(
        canonical(&pool, range_idx),
        MachineRepr::Range,
        "Range canonical"
    );
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, iter_idx),
        MachineRepr::UnmanagedPtr,
        "Iterator canonical"
    );
    let deiter_idx = pool.double_ended_iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, deiter_idx),
        MachineRepr::UnmanagedPtr,
        "DoubleEndedIterator"
    );

    // Two-child containers (2 of 3 — Borrowed is non-codegen)
    let map_idx = pool.map(Idx::STR, Idx::INT);
    assert!(
        matches!(
            canonical(&pool, map_idx),
            MachineRepr::FatPointer(FatRepr::Map { .. })
        ),
        "Map canonical"
    );
    let result_idx = pool.result(Idx::INT, Idx::STR);
    assert!(
        matches!(canonical(&pool, result_idx), MachineRepr::Enum(_)),
        "Result canonical"
    );
}

/// §01.9 Item 6: Storage type equivalence — complex types and resolved names.
///
/// Covers Function, Tuple, Struct, Enum + Named/Applied/Alias resolution.
#[test]
fn storage_equivalence_complex_and_resolved() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();

    // Complex types (4)
    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, fn_idx), MachineRepr::Closure(_)),
        "Function canonical"
    );
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, tuple_idx), MachineRepr::Tuple(_)),
        "Tuple canonical"
    );
    let struct_name = Name::new(0, 500);
    let struct_idx = pool.struct_type(struct_name, &[(Name::new(0, 501), Idx::INT)]);
    assert!(
        matches!(canonical(&pool, struct_idx), MachineRepr::Struct(_)),
        "Struct canonical"
    );
    let enum_idx = pool.enum_type(
        Name::new(0, 600),
        &[
            EnumVariant {
                name: Name::new(0, 601),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 602),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert!(
        matches!(canonical(&pool, enum_idx), MachineRepr::Enum(_)),
        "Enum canonical"
    );

    // Named/resolved (3)
    let named_idx = pool.named(Name::new(0, 700));
    pool.set_resolution(named_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, named_idx), MachineRepr::Struct(_)),
        "Named→Struct"
    );
    let applied_idx = pool.applied(Name::new(0, 800), &[Idx::INT]);
    pool.set_resolution(applied_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, applied_idx), MachineRepr::Struct(_)),
        "Applied→Struct"
    );
    let alias_named = pool.named(Name::new(0, 900));
    pool.set_resolution(alias_named, Idx::INT);
    assert_eq!(
        canonical(&pool, alias_named),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Alias→Int canonical"
    );
}

/// §01.9 Item 6: Storage type equivalence — ZST-divergence cases.
///
/// `canonical()` correctly uses zero-sized fields for `Unit`/`Never` in
/// aggregates. `TypeInfoStore` uses `i64` for these — the divergence is
/// intentional (canonical is correct, `TypeInfoStore` is legacy).
#[test]
fn storage_equivalence_zst_divergence() {
    let mut pool = Pool::new();

    let opt_unit = pool.option(Idx::UNIT);
    if let MachineRepr::Enum(ref e) = canonical(&pool, opt_unit) {
        assert_eq!(e.size, 8, "Option<()> = 8 bytes (tag only, zero payload)");
    } else {
        panic!("Option<()> must be Enum");
    }

    let tup_unit_bool = pool.pair(Idx::UNIT, Idx::BOOL);
    if let MachineRepr::Tuple(ref t) = canonical(&pool, tup_unit_bool) {
        assert_eq!(
            t.size, 1,
            "((), bool) = 1 byte — Unit zero-sized in aggregates"
        );
    } else {
        panic!("((), bool) must be Tuple");
    }

    let result_unit_int = pool.result(Idx::UNIT, Idx::INT);
    if let MachineRepr::Enum(ref e) = canonical(&pool, result_unit_int) {
        assert_eq!(e.size, 16, "Result<(), int> = 16 bytes");
    } else {
        panic!("Result<(), int> must be Enum");
    }

    let struct_unit_idx = pool.struct_type(
        Name::new(0, 1000),
        &[
            (Name::new(0, 1001), Idx::UNIT),
            (Name::new(0, 1002), Idx::INT),
        ],
    );
    if let MachineRepr::Struct(ref s) = canonical(&pool, struct_unit_idx) {
        assert_eq!(s.size, 8, "Struct(unit, int) = 8 bytes — Unit zero-sized");
    } else {
        panic!("Struct with Unit field must be Struct");
    }
}

/// §01.9: `Borrowed` returns None — reserved type, not a codegen type.
#[test]
fn canonical_returns_none_for_borrowed() {
    use ori_types::{LifetimeId, Tag};

    let mut pool = Pool::new();
    let borrowed_idx = pool.borrowed(Idx::INT, LifetimeId::from_raw(1));
    assert_eq!(pool.tag(borrowed_idx), Tag::Borrowed);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, borrowed_idx, &mut cache).is_none(),
        "Borrowed must return None, not panic"
    );
}

/// §01.9: `Projection` returns None — type-checker artifact.
#[test]
fn canonical_returns_none_for_projection() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let proj_idx = pool.intern(Tag::Projection, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, proj_idx, &mut cache).is_none(),
        "Projection must return None, not panic"
    );
}

/// §01.9: `ModuleNs` returns None — module namespace, not a runtime type.
#[test]
fn canonical_returns_none_for_module_ns() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let ns_idx = pool.intern(Tag::ModuleNs, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, ns_idx, &mut cache).is_none(),
        "ModuleNs must return None, not panic"
    );
}

/// §01.10: Storage type equivalence — comprehensive 29-type matrix.
///
/// Verifies that `canonical()` produces the correct `MachineRepr` for every
/// type kind in the matrix from §01.9. Each assertion documents the expected
/// LLVM storage type that `TypeInfo::storage_type()` or `TypeLayoutResolver`
/// would produce for the same type. This is the parity contract between
/// `ori_repr` (representation-agnostic) and `ori_llvm` (LLVM-specific).
///
/// Matrix:
///   Primitives (12) | Containers (7) | Two-child (3) | Complex (4)
///   Named (3)       | Variables (3)  | Scheme/Special (5)
///
/// Divergences from `TypeInfoStore` are documented inline.
#[test]
#[expect(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "Exhaustive 29-type matrix test — splitting would obscure the complete coverage contract"
)]
fn storage_type_equivalence_full_29_type_matrix() {
    use ori_types::{EnumVariant, LifetimeId, Tag};

    let mut pool = Pool::new();

    // ── Primitives (12) ──
    // TypeInfo::Int → i64 | canonical: Int { I64, signed: true }
    assert_eq!(
        canonical(&pool, Idx::INT),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "[1/29] Int → i64"
    );
    // TypeInfo::Float → f64 | canonical: Float { F64 }
    assert_eq!(
        canonical(&pool, Idx::FLOAT),
        MachineRepr::Float {
            width: FloatWidth::F64
        },
        "[2/29] Float → f64"
    );
    // TypeInfo::Bool → i1
    assert_eq!(
        canonical(&pool, Idx::BOOL),
        MachineRepr::Bool,
        "[3/29] Bool → i1"
    );
    // TypeInfo::Str → {i64, i64, ptr}
    assert_eq!(
        canonical(&pool, Idx::STR),
        MachineRepr::FatPointer(FatRepr::Str),
        "[4/29] Str → {{i64, i64, ptr}}"
    );
    // TypeInfo::Char → i32
    assert_eq!(
        canonical(&pool, Idx::CHAR),
        MachineRepr::Char,
        "[5/29] Char → i32"
    );
    // TypeInfo::Byte → i8
    assert_eq!(
        canonical(&pool, Idx::BYTE),
        MachineRepr::Byte,
        "[6/29] Byte → i8"
    );
    // TypeInfo::Unit → i64 (DIVERGENCE: canonical uses zero-sized Unit)
    assert_eq!(
        canonical(&pool, Idx::UNIT),
        MachineRepr::Unit,
        "[7/29] Unit → ZST"
    );
    // TypeInfo::Never → i64 (DIVERGENCE: canonical uses zero-sized Never)
    assert_eq!(
        canonical(&pool, Idx::NEVER),
        MachineRepr::Never,
        "[8/29] Never → ZST"
    );
    // Error → i64 in TypeInfo, None in canonical (non-codegen)
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, Idx::ERROR, &mut cache).is_none(),
        "[9/29] Error → None"
    );
    // TypeInfo::Duration → i64
    assert_eq!(
        canonical(&pool, Idx::DURATION),
        MachineRepr::Duration,
        "[10/29] Duration → i64"
    );
    // TypeInfo::Size → i64
    assert_eq!(
        canonical(&pool, Idx::SIZE),
        MachineRepr::Size,
        "[11/29] Size → i64"
    );
    // TypeInfo::Ordering → i8
    assert_eq!(
        canonical(&pool, Idx::ORDERING),
        MachineRepr::Ordering,
        "[12/29] Ordering → i8"
    );

    // ── Simple containers (7) ──
    // TypeInfo::List → {i64, i64, ptr}
    let list_idx = pool.list(Idx::INT);
    assert!(
        matches!(
            canonical(&pool, list_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "[13/29] List → {{i64, i64, ptr}}"
    );
    // TypeInfo::Option → Enum (via TypeLayoutResolver)
    let opt_idx = pool.option(Idx::INT);
    assert!(
        matches!(canonical(&pool, opt_idx), MachineRepr::Enum(_)),
        "[14/29] Option → Enum (tagged union)"
    );
    // TypeInfo::Set → {i64, i64, ptr}
    let set_idx = pool.set(Idx::STR);
    assert!(
        matches!(
            canonical(&pool, set_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "[15/29] Set → {{i64, i64, ptr}}"
    );
    // TypeInfo::Channel → ptr
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(
        canonical(&pool, chan_idx),
        MachineRepr::OpaquePtr,
        "[16/29] Channel → ptr"
    );
    // TypeInfo::Range → {i64, i64, i64, i64}
    let range_idx = pool.range(Idx::INT);
    assert_eq!(
        canonical(&pool, range_idx),
        MachineRepr::Range,
        "[17/29] Range → {{i64, i64, i64, i64}}"
    );
    // TypeInfo::Iterator → unmanaged ptr (Box-allocated, no RC header)
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, iter_idx),
        MachineRepr::UnmanagedPtr,
        "[18/29] Iterator → unmanaged ptr"
    );
    // DoubleEndedIterator → unmanaged ptr (same as Iterator)
    let deiter_idx = pool.double_ended_iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, deiter_idx),
        MachineRepr::UnmanagedPtr,
        "[19/29] DoubleEndedIterator → unmanaged ptr"
    );

    // ── Two-child types (3) ──
    // TypeInfo::Map → {i64, i64, ptr}
    let map_idx = pool.map(Idx::STR, Idx::INT);
    assert!(
        matches!(
            canonical(&pool, map_idx),
            MachineRepr::FatPointer(FatRepr::Map { .. })
        ),
        "[20/29] Map → {{i64, i64, ptr}}"
    );
    // TypeInfo::Result → Enum (via TypeLayoutResolver)
    let result_idx = pool.result(Idx::INT, Idx::STR);
    assert!(
        matches!(canonical(&pool, result_idx), MachineRepr::Enum(_)),
        "[21/29] Result → Enum (tagged union)"
    );
    // Borrowed → None (reserved, non-codegen)
    let borrowed_idx = pool.borrowed(Idx::INT, LifetimeId::from_raw(1));
    cache.clear();
    assert!(
        canonical_cached(&pool, borrowed_idx, &mut cache).is_none(),
        "[22/29] Borrowed → None"
    );

    // ── Complex types (4) ──
    // TypeInfo::Function → {ptr, ptr}
    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, fn_idx), MachineRepr::Closure(_)),
        "[23/29] Function → {{ptr, ptr}}"
    );
    // Tuple → struct of element types (via TypeLayoutResolver)
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, tuple_idx), MachineRepr::Tuple(_)),
        "[24/29] Tuple → struct (element types)"
    );
    // Struct → struct of field types
    let struct_name = Name::new(0, 500);
    let struct_idx = pool.struct_type(struct_name, &[(Name::new(0, 501), Idx::INT)]);
    assert!(
        matches!(canonical(&pool, struct_idx), MachineRepr::Struct(_)),
        "[25/29] Struct → struct (field types)"
    );
    // Enum → {tag, max(variant payloads)}
    let enum_idx = pool.enum_type(
        Name::new(0, 600),
        &[
            EnumVariant {
                name: Name::new(0, 601),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 602),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert!(
        matches!(canonical(&pool, enum_idx), MachineRepr::Enum(_)),
        "[26/29] Enum → {{i64 tag, payload}}"
    );

    // ── Named/resolved types (3) ──
    // Named → resolves through to underlying type
    let named_idx = pool.named(Name::new(0, 700));
    pool.set_resolution(named_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, named_idx), MachineRepr::Struct(_)),
        "[27/29] Named → Struct (resolved)"
    );
    // Applied → resolves through to underlying type
    let applied_idx = pool.applied(Name::new(0, 800), &[Idx::INT]);
    pool.set_resolution(applied_idx, Idx::INT);
    assert_eq!(
        canonical(&pool, applied_idx),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "[28/29] Applied → Int (resolved)"
    );
    // Alias → resolves through chain
    let alias_idx = pool.named(Name::new(0, 900));
    pool.set_resolution(alias_idx, Idx::FLOAT);
    assert_eq!(
        canonical(&pool, alias_idx),
        MachineRepr::Float {
            width: FloatWidth::F64
        },
        "[29/29] Alias → Float (resolved)"
    );

    // ── Non-codegen types (all return None) ──
    // Variables (3): Var, BoundVar, RigidVar
    cache.clear();
    let var_idx = pool.fresh_var();
    assert!(
        canonical_cached(&pool, var_idx, &mut cache).is_none(),
        "Var → None"
    );
    let bound_idx = pool.intern(Tag::BoundVar, 0);
    assert!(
        canonical_cached(&pool, bound_idx, &mut cache).is_none(),
        "BoundVar → None"
    );
    let rigid_idx = pool.rigid_var(Name::new(0, 999));
    assert!(
        canonical_cached(&pool, rigid_idx, &mut cache).is_none(),
        "RigidVar → None"
    );
    // Scheme/Special (5): Scheme, Projection, ModuleNs, Infer, SelfType
    let scheme_idx = pool.scheme(&[0], Idx::INT);
    assert!(
        canonical_cached(&pool, scheme_idx, &mut cache).is_none(),
        "Scheme → None"
    );
    let proj_idx = pool.intern(Tag::Projection, 0);
    assert!(
        canonical_cached(&pool, proj_idx, &mut cache).is_none(),
        "Projection → None"
    );
    let ns_idx = pool.intern(Tag::ModuleNs, 0);
    assert!(
        canonical_cached(&pool, ns_idx, &mut cache).is_none(),
        "ModuleNs → None"
    );
    let infer_idx = pool.intern(Tag::Infer, 0);
    assert!(
        canonical_cached(&pool, infer_idx, &mut cache).is_none(),
        "Infer → None"
    );
    let self_idx = pool.intern(Tag::SelfType, 0);
    assert!(
        canonical_cached(&pool, self_idx, &mut cache).is_none(),
        "SelfType → None"
    );
}

// §02.2b: analyze_triviality() validation pass produces zero mismatches

#[test]
fn analyze_triviality_validation_zero_mismatches() {
    use ori_types::{EnumVariant, Idx, Pool};

    let mut pool = Pool::new();

    // Build diverse types: trivial + non-trivial + compound
    let opt_int = pool.option(Idx::INT);
    let tuple_trivial = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let sn = Name::from_raw(8000);
    let f1 = Name::from_raw(8001);
    let f2 = Name::from_raw(8002);
    let struct_trivial = pool.struct_type(sn, &[(f1, Idx::INT), (f2, Idx::FLOAT)]);
    let result_nontrivial = pool.result(Idx::INT, Idx::STR);
    let enum_trivial = pool.enum_type(
        Name::from_raw(8010),
        &[
            EnumVariant {
                name: Name::from_raw(8011),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(8012),
                field_types: vec![Idx::INT],
            },
        ],
    );

    // Iterator and DoubleEndedIterator: Box-allocated, no RC header → trivial.
    // These are the types that previously triggered a known mismatch when
    // mapped to OpaquePtr (non-trivial). Now mapped to UnmanagedPtr (trivial).
    let iter_int = pool.iterator(Idx::INT);
    let deiter_int = pool.double_ended_iterator(Idx::INT);

    // compute_repr_plan canonicalizes all reachable types and runs
    // analyze_triviality() internally. The debug_assert! in the pass
    // fires on any mismatch between classify_triviality() and is_trivial_repr().
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    // Verify the plan's is_trivial() queries match expectations.
    assert!(plan.is_trivial(opt_int), "Option<int> should be trivial");
    assert!(
        plan.is_trivial(tuple_trivial),
        "(int, float) should be trivial"
    );
    assert!(
        plan.is_trivial(struct_trivial),
        "struct {{int, float}} should be trivial"
    );
    assert!(
        !plan.is_trivial(result_nontrivial),
        "Result<int, str> should be non-trivial"
    );
    assert!(
        plan.is_trivial(iter_int),
        "Iterator<int> should be trivial (UnmanagedPtr)"
    );
    assert!(
        plan.is_trivial(deiter_int),
        "DoubleEndedIterator<int> should be trivial (UnmanagedPtr)"
    );
    assert!(
        plan.is_trivial(enum_trivial),
        "enum {{unit, int}} should be trivial"
    );
}

// §02 TPR-02-005: Idx::ERROR must be trivial in ReprPlan (parity with classify_triviality)

#[test]
fn repr_plan_error_type_is_trivial() {
    // TPR-02-005 semantic pin: ReprPlan::is_trivial(Idx::ERROR) must return true,
    // matching classify_triviality(Idx::ERROR) which returns Triviality::Trivial.
    // ERROR is a sentinel type that should never trigger RC operations.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(
        plan.is_trivial(Idx::ERROR),
        "Idx::ERROR must be trivial — matches classify_triviality() and ArcClassifier"
    );
}

#[test]
fn repr_plan_error_type_has_canonical_repr() {
    // TPR-02-005: ERROR must have a canonical repr so is_trivial() doesn't
    // fall through to the None->false default.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(
        plan.get_repr(Idx::ERROR).is_some(),
        "Idx::ERROR must have a canonical representation"
    );
}

#[test]
fn repr_plan_error_triviality_matches_classify_triviality() {
    // TPR-02-005 parity test: ReprPlan and classify_triviality() must agree
    // for the ERROR sentinel.
    use ori_types::triviality::{classify_triviality, Triviality};

    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    let plan_trivial = plan.is_trivial(Idx::ERROR);
    let classify_trivial = classify_triviality(Idx::ERROR, &pool) == Triviality::Trivial;
    assert_eq!(
        plan_trivial, classify_trivial,
        "ReprPlan::is_trivial(ERROR) = {plan_trivial}, classify_triviality(ERROR) = {classify_trivial} — must agree"
    );
}
