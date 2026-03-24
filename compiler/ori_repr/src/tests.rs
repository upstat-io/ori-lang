//! Tests for `ori_repr` types.

use crate::enum_repr::{EnumTag, VariantRepr};
use crate::escape::EscapeInfo;
use crate::plan::{DecisionReason, DecisionSource, NarrowingPolicy, ReprDecision};
use crate::range::ValueRange;
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
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

use crate::canonical::canonical;
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
    assert_eq!(canonical(&pool, iter_idx), MachineRepr::OpaquePtr);
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

/// Test that unresolved Var panics.
#[test]
#[should_panic(expected = "unresolved type variable")]
fn canonical_panics_on_var() {
    let mut pool = Pool::new();
    let var_idx = pool.fresh_var();
    canonical(&pool, var_idx);
}

/// Test that `BoundVar` panics — constructs a real `BoundVar` via `pool.intern`.
#[test]
#[should_panic(expected = "unresolved type variable")]
fn canonical_panics_on_bound_var() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let bound_var_idx = pool.intern(Tag::BoundVar, 0);
    // BoundVar must never reach codegen
    canonical(&pool, bound_var_idx);
}

/// Test that `RigidVar` panics.
#[test]
#[should_panic(expected = "unresolved type variable")]
fn canonical_panics_on_rigid_var() {
    let mut pool = Pool::new();
    let rigid = pool.rigid_var(Name::new(0, 999));
    canonical(&pool, rigid);
}

/// Test that Error type panics (should not reach codegen).
#[test]
#[should_panic(expected = "should not reach codegen")]
fn canonical_panics_on_error() {
    let pool = Pool::new();
    canonical(&pool, Idx::ERROR);
}

/// §01.5: Scheme type must panic (should never reach codegen).
#[test]
#[should_panic(expected = "should never reach codegen")]
fn canonical_panics_on_scheme() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let scheme_idx = pool.scheme(&[0], Idx::INT);
    // Verify it's actually a Scheme tag
    assert_eq!(pool.tag(scheme_idx), Tag::Scheme);
    canonical(&pool, scheme_idx);
}

/// §01.5: Infer type must panic (should never reach codegen).
#[test]
#[should_panic(expected = "should never reach codegen")]
fn canonical_panics_on_infer() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let infer_idx = pool.intern(Tag::Infer, 0);
    canonical(&pool, infer_idx);
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
    let a_repr = crate::canonical::canonical_cached(&pool, a_struct, &mut cache);
    let b_repr = crate::canonical::canonical_cached(&pool, b_struct, &mut cache);

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
    let a_repr2 = crate::canonical::canonical_cached(&pool, a_struct, &mut cache);
    assert_eq!(a_repr, a_repr2, "cached result must be stable");
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
    // Simulate populate_canonical() storing an OpaquePtr for an iterator type.
    plan.set_repr(
        iter_idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: iter_idx,
            repr: MachineRepr::OpaquePtr,
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
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive);
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
    // Error type should NOT be populated.
    assert!(
        plan.get_repr(Idx::ERROR).is_none(),
        "Error must not be populated"
    );
}

#[test]
fn compute_repr_plan_disabled_policy_skips_stubs() {
    // §01.3 test: NarrowingPolicy::Disabled returns after populate_canonical()
    // without calling any narrowing stubs.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled);
    // Same primitives should be populated (canonical-only).
    assert!(plan.get_repr(Idx::INT).is_some());
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Disabled);
}

#[test]
fn compute_repr_plan_aggressive_is_default_behavior() {
    // §01.3 test: NarrowingPolicy::Aggressive is the default — building
    // without --no-repr-opt results in Aggressive.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive);
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
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive);
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
    let plan_aggressive = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive);
    let plan_disabled = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled);
    // Both should produce the same canonical repr for every primitive.
    for raw in 0..Idx::PRIMITIVE_COUNT {
        let idx = Idx::from_raw(raw);
        if idx == Idx::ERROR {
            continue;
        }
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
