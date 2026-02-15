use super::*;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::context::SimpleCx;
use inkwell::context::Context;
use ori_ir::Name;
use ori_types::{
    EnumVariant, Idx, Pool, StructDef, TypeEntry, TypeKind, ValueCategory, Visibility,
};

/// Create a pool with a Point struct: { x: int, y: float }
/// Returns (pool, struct_idx) where struct_idx is the concrete Struct Idx.
fn make_struct_pool() -> (Pool, Idx) {
    let mut pool = Pool::new();
    let point_name = Name::from_raw(100);
    let x_name = Name::from_raw(101);
    let y_name = Name::from_raw(102);

    let named_idx = pool.named(point_name);
    let struct_idx = pool.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);
    pool.set_resolution(named_idx, struct_idx);

    (pool, struct_idx)
}

/// Create a pool with a Status enum: Pending | Done
/// Returns (pool, enum_idx).
fn make_enum_pool() -> (Pool, Idx) {
    let mut pool = Pool::new();
    let status_name = Name::from_raw(200);
    let pending_name = Name::from_raw(201);
    let done_name = Name::from_raw(202);

    let named_idx = pool.named(status_name);
    let enum_idx = pool.enum_type(
        status_name,
        &[
            EnumVariant {
                name: pending_name,
                field_types: vec![],
            },
            EnumVariant {
                name: done_name,
                field_types: vec![],
            },
        ],
    );
    pool.set_resolution(named_idx, enum_idx);

    (pool, enum_idx)
}

fn make_type_entry(name: Name, idx: Idx, kind: TypeKind) -> TypeEntry {
    TypeEntry {
        name,
        idx,
        kind,
        span: ori_ir::Span::new(0, 0),
        type_params: vec![],
        visibility: Visibility::Public,
    }
}

#[test]
fn register_struct_creates_named_llvm_type() {
    let (pool, struct_idx) = make_struct_pool();
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_struct_reg");
    let store = TypeInfoStore::new(&pool);
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let entry = make_type_entry(
        Name::from_raw(100),
        struct_idx,
        TypeKind::Struct(StructDef {
            fields: vec![],
            category: ValueCategory::default(),
        }),
    );
    register_user_types(&resolver, &[entry]);

    // Verify: the resolved type is a struct type in LLVM
    let resolved = resolver.resolve(struct_idx);
    assert!(
        resolved.is_struct_type(),
        "resolved struct should be an LLVM struct type, got {resolved:?}"
    );
}

#[test]
fn register_enum_creates_named_llvm_type() {
    let (pool, enum_idx) = make_enum_pool();
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_enum_reg");
    let store = TypeInfoStore::new(&pool);
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let entry = make_type_entry(
        Name::from_raw(200),
        enum_idx,
        TypeKind::Enum { variants: vec![] },
    );
    register_user_types(&resolver, &[entry]);

    // Verify: the resolved type is a struct type (enums are { tag, payload })
    let resolved = resolver.resolve(enum_idx);
    assert!(
        resolved.is_struct_type(),
        "resolved enum should be an LLVM struct type, got {resolved:?}"
    );
}

#[test]
fn generic_types_are_skipped() {
    let pool = Pool::new();
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_generic_skip");
    let store = TypeInfoStore::new(&pool);
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let entry = TypeEntry {
        name: Name::from_raw(300),
        idx: Idx::INT, // dummy — won't be resolved
        kind: TypeKind::Struct(StructDef {
            fields: vec![],
            category: ValueCategory::default(),
        }),
        span: ori_ir::Span::new(0, 0),
        type_params: vec![Name::from_raw(301)], // generic param T
        visibility: Visibility::Public,
    };

    // Should not panic — generic types are skipped
    register_user_types(&resolver, &[entry]);
}

#[test]
fn empty_type_list_is_noop() {
    let pool = Pool::new();
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_empty");
    let store = TypeInfoStore::new(&pool);
    let resolver = TypeLayoutResolver::new(&store, &scx);

    register_user_types(&resolver, &[]);
}
