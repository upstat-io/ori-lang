//! Tests for the type registry.

use crate::registry::{TypeEntry, TypeKind, TypeRegistry};
use ori_ir::{SharedInterner, Span, TypeId};
use ori_types::Type;

fn make_span() -> Span {
    Span::new(0, 0)
}

#[test]
fn test_registry_creation() {
    let registry = TypeRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_register_struct() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let point_name = interner.intern("Point");
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let fields = vec![(x_name, Type::Int), (y_name, Type::Int)];

    let type_id = registry.register_struct(point_name, fields.clone(), make_span(), vec![]);

    assert!(!type_id.is_primitive());
    assert_eq!(registry.len(), 1);

    let entry = registry.get_by_name(point_name).unwrap();
    assert_eq!(entry.name, point_name);
    assert_eq!(entry.type_id, type_id);

    if let TypeKind::Struct {
        fields: entry_fields,
    } = &entry.kind
    {
        assert_eq!(entry_fields.len(), 2);
        assert_eq!(entry_fields[0].0, x_name);
        // Fields are now stored as TypeId
        assert_eq!(entry_fields[0].1, TypeId::INT);
    } else {
        panic!("Expected struct type");
    }
}

#[test]
fn test_register_enum() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let result_name = interner.intern("MyResult");
    let ok_name = interner.intern("Ok");
    let err_name = interner.intern("Err");
    let value_name = interner.intern("value");

    // Use the new API: (variant_name, fields) tuples
    let variants = vec![
        (ok_name, vec![(value_name, Type::Int)]),
        (err_name, vec![(value_name, Type::Str)]),
    ];

    let type_id = registry.register_enum(result_name, variants, make_span(), vec![]);

    assert!(!type_id.is_primitive());
    assert!(registry.contains(result_name));

    let entry = registry.get_by_id(type_id).unwrap();
    if let TypeKind::Enum {
        variants: entry_variants,
    } = &entry.kind
    {
        assert_eq!(entry_variants.len(), 2);
        assert_eq!(entry_variants[0].name, ok_name);
    } else {
        panic!("Expected enum type");
    }
}

#[test]
fn test_register_alias() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let id_name = interner.intern("UserId");
    let type_id = registry.register_alias(id_name, &Type::Int, make_span(), vec![]);

    assert!(!type_id.is_primitive());

    let entry = registry.get_by_name(id_name).unwrap();
    if let TypeKind::Alias { target } = &entry.kind {
        // Target is now stored as TypeId
        assert_eq!(*target, TypeId::INT);
    } else {
        panic!("Expected alias type");
    }
}

#[test]
fn test_unique_type_ids() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let name1 = interner.intern("Type1");
    let name2 = interner.intern("Type2");
    let name3 = interner.intern("Type3");

    let id1 = registry.register_struct(name1, vec![], make_span(), vec![]);
    let id2 = registry.register_enum(name2, vec![], make_span(), vec![]);
    let id3 = registry.register_alias(name3, &Type::Int, make_span(), vec![]);

    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
}

#[test]
fn test_to_type_struct() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let point_name = interner.intern("Point");
    let type_id = registry.register_struct(point_name, vec![], make_span(), vec![]);

    let typ = registry.to_type(type_id).unwrap();
    assert_eq!(typ, Type::Named(point_name));
}

#[test]
fn test_to_type_alias() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let id_name = interner.intern("UserId");
    let type_id = registry.register_alias(id_name, &Type::Int, make_span(), vec![]);

    let typ = registry.to_type(type_id).unwrap();
    assert_eq!(typ, Type::Int);
}

#[test]
fn test_generic_type_params() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let container_name = interner.intern("Container");
    let t_name = interner.intern("T");

    let type_id = registry.register_struct(container_name, vec![], make_span(), vec![t_name]);

    let entry = registry.get_by_id(type_id).unwrap();
    assert_eq!(entry.type_params.len(), 1);
    assert_eq!(entry.type_params[0], t_name);
}

#[test]
fn test_iterate_types() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let name1 = interner.intern("A");
    let name2 = interner.intern("B");

    registry.register_struct(name1, vec![], make_span(), vec![]);
    registry.register_struct(name2, vec![], make_span(), vec![]);

    let names: Vec<_> = registry.iter().map(|e| e.name).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&name1));
    assert!(names.contains(&name2));
}

#[test]
fn test_salsa_traits() {
    let interner = SharedInterner::default();
    let mut registry1 = TypeRegistry::new();
    let mut registry2 = TypeRegistry::new();

    let name = interner.intern("Point");
    registry1.register_struct(name, vec![], make_span(), vec![]);
    registry2.register_struct(name, vec![], make_span(), vec![]);

    // Clone
    let cloned = registry1.clone();
    assert_eq!(cloned.len(), registry1.len());

    // Eq
    assert_eq!(registry1, registry2);
}

#[test]
fn test_type_entry_hash() {
    use std::collections::HashSet;

    let interner = SharedInterner::default();
    let name = interner.intern("Point");

    let entry1 = TypeEntry {
        name,
        type_id: TypeId::new(TypeId::FIRST_COMPOUND),
        kind: TypeKind::Struct { fields: vec![] },
        span: make_span(),
        type_params: vec![],
    };
    let entry2 = entry1.clone();

    let mut set = HashSet::new();
    set.insert(entry1);
    set.insert(entry2); // duplicate
    assert_eq!(set.len(), 1);
}
