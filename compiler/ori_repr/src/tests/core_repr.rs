use super::*;

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

#[test]
fn closure_repr_preserves_parameter_and_return_shapes() {
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
        niche_variant_idx: 0,
    };
    if let EnumTag::Niche {
        field_index,
        niche_value,
        niche_variant_idx,
    } = tag
    {
        assert_eq!(field_index, 0);
        assert_eq!(niche_value, 0);
        assert_eq!(niche_variant_idx, 0);
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
