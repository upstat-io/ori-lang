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

    registry.register_struct(
        name,
        idx,
        vec![],
        fields,
        test_span(),
        Visibility::Public,
        0,
        None,
        None, // burden
    );

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

    registry.register_enum(
        name,
        idx,
        vec![],
        variants,
        test_span(),
        Visibility::Public,
        0,
        None,
        None, // burden
    );

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

    registry.register_newtype(
        name,
        idx,
        vec![],
        Idx::INT,
        test_span(),
        Visibility::Public,
        0,
        None,
        None, // burden
    );

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

    registry.register_alias(
        name,
        idx,
        vec![],
        target,
        test_span(),
        Visibility::Public,
        0,
        None, // burden
    );

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
        0,
        None,
        None, // burden
    );
    registry.register_struct(
        name_a,
        Idx::from_raw(101),
        vec![],
        vec![],
        test_span(),
        Visibility::Public,
        0,
        None,
        None, // burden
    );
    registry.register_struct(
        name_m,
        Idx::from_raw(102),
        vec![],
        vec![],
        test_span(),
        Visibility::Public,
        0,
        None,
        None, // burden
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
        0,
        None,
        None, // burden
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
        0,
        None,
        None, // burden
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

// Burden integration tests.
//
// Exercise the full registration path: compute_*_burden helpers → register_*
// → TypeRegistry::burden(idx). Matrix covers (struct × field-class),
// (enum × variant-shape), (newtype × inheritance-branch).

use crate::check::registration::burden_compute::{
    compute_enum_burden, compute_newtype_burden, compute_struct_burden,
};
use crate::Pool;

fn heap_str_field(name: &str) -> FieldDef {
    FieldDef {
        name: test_name(name),
        ty: Idx::STR,
        span: test_span(),
        visibility: Visibility::Public,
    }
}

fn scalar_int_field(name: &str) -> FieldDef {
    FieldDef {
        name: test_name(name),
        ty: Idx::INT,
        span: test_span(),
        visibility: Visibility::Public,
    }
}

fn scalar_bool_field(name: &str) -> FieldDef {
    FieldDef {
        name: test_name(name),
        ty: Idx::BOOL,
        span: test_span(),
        visibility: Visibility::Public,
    }
}

#[test]
fn burden_struct_heap_str_plus_scalar_int_has_one_owned_zero_borrowed() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let name = test_name("Person");
    let idx = Idx::from_raw(200);
    let fields = vec![heap_str_field("name"), scalar_int_field("age")];
    let burden = compute_struct_burden(&fields, &pool);
    registry.register_struct(
        name,
        idx,
        vec![],
        fields,
        test_span(),
        Visibility::Public,
        0,
        None,
        burden,
    );
    let spec = registry
        .burden(idx)
        .expect("heap field should yield burden");
    assert_eq!(spec.owned_fields.len(), 1, "str field is owned-heap");
    assert_eq!(
        spec.borrowed_fields.len(),
        0,
        "Tag::Borrowed not yet constructible"
    );
    assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
    assert_eq!(spec.owned_fields[0].field_path, vec![0]);
    assert!(spec.self_heap_alloc);
}

#[test]
fn burden_struct_all_scalar_returns_none() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let name = test_name("Point3D");
    let idx = Idx::from_raw(201);
    let fields = vec![
        scalar_int_field("x"),
        scalar_int_field("y"),
        scalar_bool_field("z_flag"),
    ];
    let burden = compute_struct_burden(&fields, &pool);
    registry.register_struct(
        name,
        idx,
        vec![],
        fields,
        test_span(),
        Visibility::Public,
        0,
        None,
        burden,
    );
    assert!(
        registry.burden(idx).is_none(),
        "all-scalar struct must have no burden"
    );
}

#[test]
fn burden_struct_two_heap_fields_accumulates_owned() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let name = test_name("Pair");
    let idx = Idx::from_raw(202);
    let fields = vec![heap_str_field("first"), heap_str_field("second")];
    let burden = compute_struct_burden(&fields, &pool);
    registry.register_struct(
        name,
        idx,
        vec![],
        fields,
        test_span(),
        Visibility::Public,
        0,
        None,
        burden,
    );
    let spec = registry.burden(idx).expect("two heap fields yield burden");
    assert_eq!(spec.owned_fields.len(), 2, "both str fields are owned-heap");
    assert_eq!(spec.owned_fields[0].field_path, vec![0]);
    assert_eq!(spec.owned_fields[1].field_path, vec![1]);
}

#[test]
fn burden_enum_two_heap_payload_variants_yields_two_variant_burdens() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let name = test_name("Message");
    let idx = Idx::from_raw(203);
    let v1 = VariantDef {
        name: test_name("Text"),
        fields: VariantFields::Tuple(vec![Idx::STR]),
        span: test_span(),
    };
    let v2 = VariantDef {
        name: test_name("Code"),
        fields: VariantFields::Tuple(vec![Idx::STR]),
        span: test_span(),
    };
    let variants = vec![v1, v2];
    let burden = compute_enum_burden(&variants, &pool);
    registry.register_enum(
        name,
        idx,
        vec![],
        variants,
        test_span(),
        Visibility::Public,
        0,
        None,
        burden,
    );
    let spec = registry.burden(idx).expect("heap variants yield burden");
    assert_eq!(spec.variant_burdens.len(), 2);
    for vb in &spec.variant_burdens {
        assert!(!vb.retained_owned.is_empty(), "heap payload retains owned");
    }
}

#[test]
fn burden_enum_all_unit_returns_none() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let name = test_name("Color");
    let idx = Idx::from_raw(204);
    let variants = vec![
        VariantDef {
            name: test_name("Red"),
            fields: VariantFields::Unit,
            span: test_span(),
        },
        VariantDef {
            name: test_name("Green"),
            fields: VariantFields::Unit,
            span: test_span(),
        },
        VariantDef {
            name: test_name("Blue"),
            fields: VariantFields::Unit,
            span: test_span(),
        },
    ];
    let burden = compute_enum_burden(&variants, &pool);
    registry.register_enum(
        name,
        idx,
        vec![],
        variants,
        test_span(),
        Visibility::Public,
        0,
        None,
        burden,
    );
    assert!(
        registry.burden(idx).is_none(),
        "all-unit enum must have no burden"
    );
}

#[test]
fn burden_enum_mixed_unit_and_payload_partitions_per_variant() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let name = test_name("OptionStr");
    let idx = Idx::from_raw(205);
    let variants = vec![
        VariantDef {
            name: test_name("None"),
            fields: VariantFields::Unit,
            span: test_span(),
        },
        VariantDef {
            name: test_name("Some"),
            fields: VariantFields::Tuple(vec![Idx::STR]),
            span: test_span(),
        },
    ];
    let burden = compute_enum_burden(&variants, &pool);
    registry.register_enum(
        name,
        idx,
        vec![],
        variants,
        test_span(),
        Visibility::Public,
        0,
        None,
        burden,
    );
    let spec = registry.burden(idx).expect("payload variant yields burden");
    assert_eq!(spec.variant_burdens.len(), 2);
    assert!(
        spec.variant_burdens[0].retained_owned.is_empty(),
        "Unit variant has no retained"
    );
    assert_eq!(
        spec.variant_burdens[1].retained_owned.len(),
        1,
        "Some(str) retains one"
    );
}

#[test]
fn burden_newtype_around_int_returns_none() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let burden = compute_newtype_burden(Idx::INT, &pool, &registry);
    assert!(burden.is_none(), "newtype around scalar int has no burden");
}

#[test]
fn burden_newtype_around_str_inherits_owned() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let burden = compute_newtype_burden(Idx::STR, &pool, &registry);
    let spec = burden.expect("newtype around str inherits owned");
    assert_eq!(spec.owned_fields.len(), 1);
    assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
    assert_eq!(spec.owned_fields[0].field_path, vec![0]);
}

#[test]
fn burden_newtype_around_registered_user_struct_inherits_burden() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();

    // First register a user struct with one heap field.
    let struct_name = test_name("Inner");
    let struct_idx = Idx::from_raw(210);
    let fields = vec![heap_str_field("payload")];
    let struct_burden = compute_struct_burden(&fields, &pool);
    registry.register_struct(
        struct_name,
        struct_idx,
        vec![],
        fields,
        test_span(),
        Visibility::Public,
        0,
        None,
        struct_burden,
    );

    // Now compute burden for a newtype wrapping the user struct.
    let newtype_burden = compute_newtype_burden(struct_idx, &pool, &registry);
    let spec = newtype_burden.expect("newtype around heap-bearing user type inherits");
    assert_eq!(
        spec.owned_fields.len(),
        1,
        "inherits Inner's one owned field"
    );
}

// Phase-boundary regression: ori_types is Phase 2/3 (parse/typeck) — ori_arc
// is Phase 5+ (lowering). burden computation MUST classify fields via
// `Triviality` + `Tag` (both ori_types-native), NOT `ArcClass` (ori_arc).
// Structural regression on Cargo.toml guards against future drift that
// cargo c cannot catch on silent omissions.
const ORI_TYPES_CARGO_TOML: &str = include_str!("../../../Cargo.toml");

#[test]
fn ori_types_cargo_does_not_depend_on_ori_arc() {
    let mut in_deps_section = false;
    for raw in ORI_TYPES_CARGO_TOML.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_deps_section = line == "[dependencies]";
            continue;
        }
        if in_deps_section {
            let normalized = line.trim_start_matches('#').trim();
            assert!(
                !normalized.starts_with("ori_arc"),
                "ori_types must not declare ori_arc as a runtime dependency (phase-boundary; \
                 burden uses Triviality + Tag, not ArcClass)"
            );
        }
    }
}

// Collection-instance burden side-table.
//
// Monomorphized generic-builtin instances (`[T]`, `{K: V}`, `Set<T>`) have NO
// `TypeEntry` — they are structural pool types, not user-declared nominals.
// `register_user_burden` lands their spec in the `collection_burdens`
// side-table, read by `burden(idx)` as a fallback after `types_by_idx`. Matrix
// covers (element-class: scalar vs heap) × (container shape: list vs map vs set)
// plus negative pins (unregistered instance → None) and the nominal-path
// regression (struct burden still resolves via the TypeEntry path).

use crate::registry::burden_compose::compose_user_burden;
use ori_registry::burden::table::{BurdenRegistry, TYPE_ID_LIST, TYPE_ID_MAP, TYPE_ID_SET};

/// Build a real `[T]` burden via the composer, mirroring what
/// monomorphization produces. `elem` is the concrete element `Idx`.
fn list_burden(elem: Idx, pool: &Pool, registry: &TypeRegistry) -> UserBurdenSpec {
    let Some(template) = BurdenRegistry::lookup_builtin(TYPE_ID_LIST) else {
        panic!("List template missing from BURDEN_TABLE");
    };
    compose_user_burden(template, &[elem], pool, registry)
}

fn map_burden(key: Idx, val: Idx, pool: &Pool, registry: &TypeRegistry) -> UserBurdenSpec {
    let Some(template) = BurdenRegistry::lookup_builtin(TYPE_ID_MAP) else {
        panic!("Map template missing from BURDEN_TABLE");
    };
    compose_user_burden(template, &[key, val], pool, registry)
}

fn set_burden(elem: Idx, pool: &Pool, registry: &TypeRegistry) -> UserBurdenSpec {
    let Some(template) = BurdenRegistry::lookup_builtin(TYPE_ID_SET) else {
        panic!("Set template missing from BURDEN_TABLE");
    };
    compose_user_burden(template, &[elem], pool, registry)
}

#[test]
fn burden_list_instance_without_type_entry_resolves_via_side_table() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    // A monomorphized `[str]` instance has no TypeEntry — collection pool idx.
    let list_idx = Idx::from_raw(300);
    assert!(
        registry.burden(list_idx).is_none(),
        "negative pin: unregistered collection instance has no burden"
    );
    let spec = list_burden(Idx::STR, &pool, &registry);
    registry.register_user_burden(list_idx, spec);
    let resolved = registry
        .burden(list_idx)
        .expect("registered [str] instance must resolve via side-table");
    assert!(
        resolved.self_heap_alloc,
        "[str] backing buffer is always heap-allocated"
    );
}

#[test]
fn burden_list_scalar_element_is_buffer_only() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let list_idx = Idx::from_raw(301);
    let spec = list_burden(Idx::INT, &pool, &registry);
    registry.register_user_burden(list_idx, spec);
    let resolved = registry
        .burden(list_idx)
        .expect("[int] instance resolves via side-table");
    assert!(
        resolved.self_heap_alloc,
        "[int] backing buffer is heap (buffer-only free)"
    );
    assert!(
        resolved.element_burden.is_none() || resolved.element_burden == Some(Idx::INT),
        "scalar element carries no per-element heap obligation beyond buffer free"
    );
}

#[test]
fn burden_list_heap_element_carries_element_burden() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let list_idx = Idx::from_raw(302);
    let spec = list_burden(Idx::STR, &pool, &registry);
    registry.register_user_burden(list_idx, spec);
    let resolved = registry
        .burden(list_idx)
        .expect("[str] instance resolves via side-table");
    assert!(resolved.self_heap_alloc, "buffer is heap");
    assert_eq!(
        resolved.element_burden,
        Some(Idx::STR),
        "[str] carries per-element burden = STR (buffer free + element dec)"
    );
}

#[test]
fn burden_map_instance_resolves_via_side_table() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let map_idx = Idx::from_raw(303);
    let spec = map_burden(Idx::INT, Idx::STR, &pool, &registry);
    registry.register_user_burden(map_idx, spec);
    let resolved = registry
        .burden(map_idx)
        .expect("{int: str} instance resolves via side-table");
    assert!(resolved.self_heap_alloc, "map backing buffer is heap");
}

#[test]
fn burden_set_instance_resolves_via_side_table() {
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let set_idx = Idx::from_raw(304);
    let spec = set_burden(Idx::STR, &pool, &registry);
    registry.register_user_burden(set_idx, spec);
    let resolved = registry
        .burden(set_idx)
        .expect("Set<str> instance resolves via side-table");
    assert!(resolved.self_heap_alloc, "set backing buffer is heap");
    assert_eq!(
        resolved.element_burden,
        Some(Idx::STR),
        "Set<str> carries per-element burden = STR"
    );
}

#[test]
fn burden_side_table_does_not_shadow_nominal_type_entry() {
    // Regression: the side-table fallback must NOT intercept a nominal
    // TypeEntry burden. A registered struct still resolves via types_by_idx.
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let struct_name = test_name("HasHeap");
    let struct_idx = Idx::from_raw(305);
    let fields = vec![heap_str_field("payload")];
    let struct_burden = compute_struct_burden(&fields, &pool);
    registry.register_struct(
        struct_name,
        struct_idx,
        vec![],
        fields,
        test_span(),
        Visibility::Public,
        0,
        None,
        struct_burden,
    );
    let spec = registry
        .burden(struct_idx)
        .expect("nominal struct burden resolves via TypeEntry, not side-table");
    assert_eq!(spec.owned_fields.len(), 1, "struct's one owned heap field");
}
