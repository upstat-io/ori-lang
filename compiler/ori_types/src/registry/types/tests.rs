use super::*;
use ori_ir::{Name, Span};

fn test_name(s: &str) -> Name {
    Name::from_raw(
        s.as_bytes()
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b))),
    )
}

fn test_span() -> Span {
    Span::DUMMY
}

#[test]
fn register_and_lookup_struct() {
    let mut registry = TypeRegistry::new();

    let name = test_name("Point");
    let idx = Idx::from_raw(100);
    let fields = vec![
        FieldDef {
            name: test_name("x"),
            ty: Idx::INT,
            span: test_span(),
            visibility: Visibility::Public,
        },
        FieldDef {
            name: test_name("y"),
            ty: Idx::INT,
            span: test_span(),
            visibility: Visibility::Public,
        },
    ];

    registry.register_struct(name, idx, vec![], fields, test_span(), Visibility::Public);

    // Lookup by name
    let entry = registry.get_by_name(name).expect("should find by name");
    assert_eq!(entry.name, name);
    assert_eq!(entry.idx, idx);
    assert!(entry.kind.is_struct());

    // Lookup by idx
    let entry = registry.get_by_idx(idx).expect("should find by idx");
    assert_eq!(entry.name, name);

    // Get fields
    let fields = registry.struct_fields(idx).expect("should get fields");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].ty, Idx::INT);
}

#[test]
fn register_and_lookup_enum() {
    let mut registry = TypeRegistry::new();

    let name = test_name("Option");
    let idx = Idx::from_raw(101);
    let some_name = test_name("Some");
    let none_name = test_name("None");

    let variants = vec![
        VariantDef {
            name: some_name,
            fields: VariantFields::Tuple(vec![Idx::INT]),
            span: test_span(),
        },
        VariantDef {
            name: none_name,
            fields: VariantFields::Unit,
            span: test_span(),
        },
    ];

    registry.register_enum(name, idx, vec![], variants, test_span(), Visibility::Public);

    // Lookup by name
    let entry = registry.get_by_name(name).expect("should find by name");
    assert!(entry.kind.is_enum());

    // Lookup variant
    let (type_idx, variant_idx) = registry
        .lookup_variant(some_name)
        .expect("should find variant");
    assert_eq!(type_idx, idx);
    assert_eq!(variant_idx, 0);

    let (type_idx, variant_idx) = registry
        .lookup_variant(none_name)
        .expect("should find variant");
    assert_eq!(type_idx, idx);
    assert_eq!(variant_idx, 1);

    // Get variants
    let variants = registry.enum_variants(idx).expect("should get variants");
    assert_eq!(variants.len(), 2);
    assert!(variants[0].fields.is_tuple());
    assert!(variants[1].fields.is_unit());
}

#[test]
fn register_newtype() {
    let mut registry = TypeRegistry::new();

    let name = test_name("UserId");
    let idx = Idx::from_raw(102);

    registry.register_newtype(name, idx, vec![], Idx::INT, test_span(), Visibility::Public);

    let entry = registry.get_by_name(name).expect("should find");
    assert!(entry.kind.is_newtype());

    match &entry.kind {
        TypeKind::Newtype { underlying } => {
            assert_eq!(*underlying, Idx::INT);
        }
        _ => panic!("expected newtype"),
    }
}

#[test]
fn register_alias() {
    let mut registry = TypeRegistry::new();

    let name = test_name("IntList");
    let idx = Idx::from_raw(103);
    let target = Idx::from_raw(200); // Some list type

    registry.register_alias(name, idx, vec![], target, test_span(), Visibility::Public);

    let entry = registry.get_by_name(name).expect("should find");
    assert!(entry.kind.is_alias());

    match &entry.kind {
        TypeKind::Alias { target: t } => {
            assert_eq!(*t, target);
        }
        _ => panic!("expected alias"),
    }
}

#[test]
fn variant_fields_helpers() {
    let unit = VariantFields::Unit;
    assert!(unit.is_unit());
    assert_eq!(unit.arity(), 0);

    let tuple = VariantFields::Tuple(vec![Idx::INT, Idx::STR]);
    assert!(tuple.is_tuple());
    assert_eq!(tuple.arity(), 2);
    assert_eq!(tuple.tuple_types(), Some(&[Idx::INT, Idx::STR][..]));

    let record = VariantFields::Record(vec![FieldDef {
        name: test_name("x"),
        ty: Idx::INT,
        span: test_span(),
        visibility: Visibility::Public,
    }]);
    assert!(record.is_record());
    assert_eq!(record.arity(), 1);
    assert!(record.record_fields().is_some());
}

#[test]
fn iteration_is_sorted() {
    let mut registry = TypeRegistry::new();

    // Register in non-alphabetical order
    let name_z = test_name("Zebra");
    let name_a = test_name("Apple");
    let name_m = test_name("Mango");

    registry.register_struct(
        name_z,
        Idx::from_raw(100),
        vec![],
        vec![],
        test_span(),
        Visibility::Public,
    );
    registry.register_struct(
        name_a,
        Idx::from_raw(101),
        vec![],
        vec![],
        test_span(),
        Visibility::Public,
    );
    registry.register_struct(
        name_m,
        Idx::from_raw(102),
        vec![],
        vec![],
        test_span(),
        Visibility::Public,
    );

    // Iteration should be in sorted order (by Name's Ord impl)
    let names: Vec<_> = registry.names().collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn generic_type_params() {
    let mut registry = TypeRegistry::new();

    let name = test_name("Box");
    let idx = Idx::from_raw(104);
    let t_param = test_name("T");

    registry.register_struct(
        name,
        idx,
        vec![t_param],
        vec![FieldDef {
            name: test_name("value"),
            ty: Idx::from_raw(500), // Would be a type variable in real code
            span: test_span(),
            visibility: Visibility::Public,
        }],
        test_span(),
        Visibility::Public,
    );

    let entry = registry.get_by_name(name).expect("should find");
    assert_eq!(entry.type_params.len(), 1);
    assert_eq!(entry.type_params[0], t_param);
}

#[test]
fn struct_field_lookup() {
    let mut registry = TypeRegistry::new();

    let name = test_name("Point");
    let idx = Idx::from_raw(105);
    let x_name = test_name("x");
    let y_name = test_name("y");

    registry.register_struct(
        name,
        idx,
        vec![],
        vec![
            FieldDef {
                name: x_name,
                ty: Idx::INT,
                span: test_span(),
                visibility: Visibility::Public,
            },
            FieldDef {
                name: y_name,
                ty: Idx::FLOAT,
                span: test_span(),
                visibility: Visibility::Public,
            },
        ],
        test_span(),
        Visibility::Public,
    );

    // Find field x
    let (field_idx, field) = registry
        .struct_field(idx, x_name)
        .expect("should find field x");
    assert_eq!(field_idx, 0);
    assert_eq!(field.ty, Idx::INT);

    // Find field y
    let (field_idx, field) = registry
        .struct_field(idx, y_name)
        .expect("should find field y");
    assert_eq!(field_idx, 1);
    assert_eq!(field.ty, Idx::FLOAT);

    // Unknown field
    assert!(registry.struct_field(idx, test_name("z")).is_none());
}
