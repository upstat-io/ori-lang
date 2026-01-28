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
fn test_register_newtype() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let id_name = interner.intern("UserId");
    let type_id = registry.register_newtype(id_name, &Type::Int, make_span(), vec![]);

    assert!(!type_id.is_primitive());

    let entry = registry.get_by_name(id_name).unwrap();
    if let TypeKind::Newtype { underlying } = &entry.kind {
        // Underlying type is now stored as TypeId
        assert_eq!(*underlying, TypeId::INT);
    } else {
        panic!("Expected newtype");
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
    let id3 = registry.register_newtype(name3, &Type::Int, make_span(), vec![]);

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
fn test_to_type_newtype() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let id_name = interner.intern("UserId");
    let type_id = registry.register_newtype(id_name, &Type::Int, make_span(), vec![]);

    // Newtypes are nominally distinct - they return Type::Named, not the underlying type
    let typ = registry.to_type(type_id).unwrap();
    assert_eq!(typ, Type::Named(id_name));
}

#[test]
fn test_get_newtype_underlying() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let id_name = interner.intern("UserId");
    let type_id = registry.register_newtype(id_name, &Type::Int, make_span(), vec![]);

    // Can retrieve the underlying type
    let underlying = registry.get_newtype_underlying(type_id).unwrap();
    assert_eq!(underlying, Type::Int);
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

// =============================================================================
// Variant Constructor Tests (Phase 5.2)
// =============================================================================

#[test]
fn test_lookup_variant_constructor_unit_variant() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let status_name = interner.intern("Status");
    let pending_name = interner.intern("Pending");
    let running_name = interner.intern("Running");
    let completed_name = interner.intern("Completed");

    // Register enum with unit variants
    let variants = vec![
        (pending_name, vec![]),
        (running_name, vec![]),
        (completed_name, vec![]),
    ];
    registry.register_enum(status_name, variants, make_span(), vec![]);

    // Look up unit variant
    let info = registry.lookup_variant_constructor(pending_name);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.enum_name, status_name);
    assert_eq!(info.variant_name, pending_name);
    assert!(info.field_types.is_empty());
    assert!(info.type_params.is_empty());

    // Look up another unit variant
    let info = registry.lookup_variant_constructor(running_name);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.enum_name, status_name);
    assert_eq!(info.variant_name, running_name);
}

#[test]
fn test_lookup_variant_constructor_with_fields() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let message_name = interner.intern("Message");
    let text_name = interner.intern("Text");
    let empty_name = interner.intern("Empty");
    let content_name = interner.intern("content");

    // Register enum with variant that has fields
    let variants = vec![
        (text_name, vec![(content_name, Type::Str)]),
        (empty_name, vec![]),
    ];
    registry.register_enum(message_name, variants, make_span(), vec![]);

    // Look up variant with field
    let info = registry.lookup_variant_constructor(text_name);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.enum_name, message_name);
    assert_eq!(info.variant_name, text_name);
    assert_eq!(info.field_types.len(), 1);
    assert_eq!(info.field_types[0], Type::Str);

    // Look up unit variant
    let info = registry.lookup_variant_constructor(empty_name);
    assert!(info.is_some());
    let info = info.unwrap();
    assert!(info.field_types.is_empty());
}

#[test]
fn test_lookup_variant_constructor_multiple_fields() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let result_name = interner.intern("Result2");
    let success_name = interner.intern("Success");
    let failure_name = interner.intern("Failure");
    let value_name = interner.intern("value");
    let code_name = interner.intern("code");
    let msg_name = interner.intern("msg");

    // Register enum with variants that have multiple fields
    let variants = vec![
        (success_name, vec![(value_name, Type::Int)]),
        (
            failure_name,
            vec![(code_name, Type::Int), (msg_name, Type::Str)],
        ),
    ];
    registry.register_enum(result_name, variants, make_span(), vec![]);

    // Look up variant with multiple fields
    let info = registry.lookup_variant_constructor(failure_name);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.field_types.len(), 2);
    assert_eq!(info.field_types[0], Type::Int);
    assert_eq!(info.field_types[1], Type::Str);
}

#[test]
fn test_lookup_variant_constructor_not_found() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let status_name = interner.intern("Status");
    let pending_name = interner.intern("Pending");
    let unknown_name = interner.intern("Unknown");

    // Register enum
    let variants = vec![(pending_name, vec![])];
    registry.register_enum(status_name, variants, make_span(), vec![]);

    // Look up non-existent variant
    let info = registry.lookup_variant_constructor(unknown_name);
    assert!(info.is_none());
}

#[test]
fn test_lookup_variant_constructor_generic_enum() {
    let interner = SharedInterner::default();
    let mut registry = TypeRegistry::new();

    let maybe_name = interner.intern("Maybe");
    let just_name = interner.intern("Just");
    let nothing_name = interner.intern("Nothing");
    let value_name = interner.intern("value");
    let t_name = interner.intern("T");

    // For generic enums, the field type uses the type param name as a placeholder.
    // In actual use, this would be resolved during type checking.
    // Here we use a concrete type for simplicity - the important part is testing
    // that type_params are correctly recorded.
    let variants = vec![
        (just_name, vec![(value_name, Type::Int)]), // Simplified for testing
        (nothing_name, vec![]),
    ];
    registry.register_enum(maybe_name, variants, make_span(), vec![t_name]);

    // Look up variant from generic enum
    let info = registry.lookup_variant_constructor(just_name);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.enum_name, maybe_name);
    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0], t_name);
    assert_eq!(info.field_types.len(), 1);
}
