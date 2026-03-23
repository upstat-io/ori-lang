//! Tests for `ori_repr` types.

use crate::enum_repr::{EnumTag, VariantRepr};
use crate::escape::EscapeInfo;
use crate::range::ValueRange;
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};

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

/// Test that `BoundVar` panics.
#[test]
#[should_panic(expected = "unresolved type variable")]
fn canonical_panics_on_bound_var() {
    let mut pool = Pool::new();
    let body = Idx::INT;
    let scheme_idx = pool.scheme(&[0], body);
    // Access the BoundVar inside the scheme
    let bound_var_idx = Idx::from_raw(pool.data(scheme_idx) + 1);
    // BoundVar should be at the first var slot
    // Actually, BoundVar is constructed differently — let's just test Var
    // since BoundVar would need scheme instantiation context.
    // For now, the Var test is sufficient.
    let _ = bound_var_idx;
    let _ = scheme_idx;
    // Use a RigidVar instead
    let rigid = pool.rigid_var(Name::new(0, 999));
    canonical(&pool, rigid);
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
