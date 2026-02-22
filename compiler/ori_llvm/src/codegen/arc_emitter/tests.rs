//! Tests for ARC IR emitter and drop function generation.
//!
//! Verifies that drop functions are generated with correct LLVM IR structure
//! for each `DropKind` variant, and that caching / edge cases work.

use std::mem::ManuallyDrop;

use inkwell::context::Context;
use ori_arc::{ArcClass, ArcClassification, DropInfo, DropKind};
use ori_ir::StringInterner;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use crate::codegen::abi::FunctionAbi;
use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::runtime_decl::declare_runtime;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::codegen::value_id::FunctionId;
use crate::context::SimpleCx;

/// Minimal ARC classifier: `Idx::STR` and index >= 100 are RC'd.
struct TestClassifier;

impl ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if idx == Idx::STR || idx.raw() >= 100 {
            ArcClass::DefiniteRef
        } else {
            ArcClass::Scalar
        }
    }
}

#[test]
fn drop_fn_trivial_generates_rc_free() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_trivial"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let info = DropInfo {
        ty: Idx::STR,
        kind: DropKind::Trivial,
    };
    let fid = super::drop_gen::generate_drop_fn(&mut em, Idx::STR, &info);

    let ir = scx.llmod.print_to_string().to_string();
    // LLVM quotes names with `$`: @"_ori_drop$3"
    let name = format!("\"_ori_drop${}\"", Idx::STR.raw());

    assert!(
        ir.contains(&format!("define void @{name}(ptr")),
        "Missing drop fn:\n{ir}"
    );
    assert!(ir.contains("ori_rc_free"), "Missing ori_rc_free:\n{ir}");
    assert!(ir.contains("nounwind"), "Missing nounwind:\n{ir}");
    assert!(em.drop_fn_cache.contains_key(&Idx::STR));
    assert_eq!(*em.drop_fn_cache.get(&Idx::STR).unwrap(), fid);

    drop(em);
}

#[test]
fn drop_fn_fields_generates_gep_and_rc_dec() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_fields"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let info = DropInfo {
        ty: Idx::STR,
        kind: DropKind::Fields(vec![(1, Idx::STR)]),
    };
    super::drop_gen::generate_drop_fn(&mut em, Idx::STR, &info);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains("getelementptr"), "Missing GEP:\n{ir}");
    assert!(ir.contains("ori_rc_dec"), "Missing ori_rc_dec:\n{ir}");
    assert!(ir.contains("ori_rc_free"), "Missing ori_rc_free:\n{ir}");

    drop(em);
}

#[test]
fn drop_fn_enum_generates_switch_on_tag() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_enum"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    // 2 variants: None (no RC), Some(str) (RC'd at field 1)
    let info = DropInfo {
        ty: Idx::STR,
        kind: DropKind::Enum(vec![vec![], vec![(1, Idx::STR)]]),
    };
    super::drop_gen::generate_drop_fn(&mut em, Idx::STR, &info);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains("switch"), "Missing switch:\n{ir}");
    assert!(ir.contains("drop.done"), "Missing drop.done:\n{ir}");
    assert!(ir.contains("ori_rc_dec"), "Missing ori_rc_dec:\n{ir}");

    drop(em);
}

#[test]
fn drop_fn_collection_generates_loop() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_collection"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let list_ty = Idx::from_raw(100);
    let info = DropInfo {
        ty: list_ty,
        kind: DropKind::Collection {
            element_type: Idx::STR,
        },
    };
    super::drop_gen::generate_drop_fn(&mut em, list_ty, &info);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains(&format!("\"_ori_drop${}\"", list_ty.raw())));
    assert!(ir.contains("phi i64"), "Missing phi for loop:\n{ir}");
    assert!(ir.contains("icmp"), "Missing icmp for bound:\n{ir}");
    assert!(ir.contains("ori_rc_dec"), "Missing ori_rc_dec:\n{ir}");
    assert!(
        ir.contains("ori_list_free_data"),
        "Missing buffer free:\n{ir}"
    );

    drop(em);
}

#[test]
fn drop_fn_map_generates_key_value_dec() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_map"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let map_ty = Idx::from_raw(101);
    let info = DropInfo {
        ty: map_ty,
        kind: DropKind::Map {
            key_type: Idx::STR,
            value_type: Idx::STR,
            dec_keys: true,
            dec_values: true,
        },
    };
    super::drop_gen::generate_drop_fn(&mut em, map_ty, &info);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains(&format!("\"_ori_drop${}\"", map_ty.raw())));
    assert!(ir.contains("phi i64"), "Missing phi for loop:\n{ir}");

    let dec_count = ir.matches("call void @ori_rc_dec").count();
    assert!(
        dec_count >= 2,
        "Need >= 2 ori_rc_dec (key+val), got {dec_count}:\n{ir}"
    );

    drop(em);
}

#[test]
fn drop_fn_closure_env_emits_gep_and_rc_dec() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_closure"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let clos_ty = Idx::from_raw(102);
    let info = DropInfo {
        ty: clos_ty,
        kind: DropKind::ClosureEnv(vec![(0, Idx::STR)]),
    };
    super::drop_gen::generate_drop_fn(&mut em, clos_ty, &info);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains(&format!("\"_ori_drop${}\"", clos_ty.raw())));
    assert!(ir.contains("getelementptr"), "Missing GEP:\n{ir}");
    assert!(ir.contains("ori_rc_dec"), "Missing ori_rc_dec:\n{ir}");

    drop(em);
}

#[test]
fn get_or_generate_returns_null_for_scalars() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_scalar"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let drop_fn = em.get_or_generate_drop_fn(Idx::INT);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        !ir.contains(&format!("\"_ori_drop${}\"", Idx::INT.raw())),
        "No drop for scalar:\n{ir}"
    );
    assert!(!em.drop_fn_cache.contains_key(&Idx::INT));
    assert_ne!(drop_fn, crate::codegen::value_id::ValueId::NONE);

    drop(em);
}

#[test]
fn get_or_generate_caches_across_calls() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_cache"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    // First call generates, second returns from cache
    let _ = em.get_or_generate_drop_fn(Idx::STR);
    let cached_fid = em.drop_fn_cache.get(&Idx::STR).copied();
    let _ = em.get_or_generate_drop_fn(Idx::STR);
    let cached_fid_2 = em.drop_fn_cache.get(&Idx::STR).copied();

    // Same FunctionId both times (cache hit)
    assert_eq!(
        cached_fid, cached_fid_2,
        "Cache must return same FunctionId"
    );

    // Only one definition in the module
    let ir = scx.llmod.print_to_string().to_string();
    let name = format!("\"_ori_drop${}\"", Idx::STR.raw());
    let count = ir.matches(&format!("define void @{name}")).count();
    assert_eq!(count, 1, "Exactly one definition:\n{ir}");

    drop(em);
}

#[test]
fn get_or_generate_returns_null_without_classifier() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_no_cl"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        None, // no classifier
        host,
        &functions,
        &methods,
        &names,
    );

    let drop_fn = em.get_or_generate_drop_fn(Idx::STR);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        !ir.contains(&format!("\"_ori_drop${}\"", Idx::STR.raw())),
        "No drop w/o classifier:\n{ir}"
    );
    assert_ne!(drop_fn, crate::codegen::value_id::ValueId::NONE);

    drop(em);
}

#[test]
fn drop_fn_uses_c_calling_convention() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_ccc"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let info = DropInfo {
        ty: Idx::STR,
        kind: DropKind::Trivial,
    };
    super::drop_gen::generate_drop_fn(&mut em, Idx::STR, &info);

    let ir = scx.llmod.print_to_string().to_string();
    let name = format!("\"_ori_drop${}\"", Idx::STR.raw());
    let drop_line = ir
        .lines()
        .find(|l: &&str| l.contains(&format!("define void @{name}")))
        .expect("drop fn should exist");
    // C convention = LLVM default (no prefix). Must NOT be fastcc.
    assert!(
        !drop_line.contains("fastcc"),
        "Must not use fastcc:\n{drop_line}"
    );

    drop(em);
}

#[test]
fn multiple_drop_fns_for_different_types() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_multi"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let functions: FxHashMap<ori_ir::Name, (FunctionId, FunctionAbi)> = FxHashMap::default();
    let methods: FxHashMap<(ori_ir::Name, ori_ir::Name), (FunctionId, FunctionAbi)> =
        FxHashMap::default();
    let names: FxHashMap<Idx, ori_ir::Name> = FxHashMap::default();
    let cl = TestClassifier;

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        Some(&cl as &dyn ArcClassification),
        host,
        &functions,
        &methods,
        &names,
    );

    let ty_a = Idx::from_raw(100);
    let ty_b = Idx::from_raw(101);

    super::drop_gen::generate_drop_fn(
        &mut em,
        ty_a,
        &DropInfo {
            ty: ty_a,
            kind: DropKind::Trivial,
        },
    );
    super::drop_gen::generate_drop_fn(
        &mut em,
        ty_b,
        &DropInfo {
            ty: ty_b,
            kind: DropKind::Fields(vec![(0, Idx::STR)]),
        },
    );
    super::drop_gen::generate_drop_fn(
        &mut em,
        Idx::STR,
        &DropInfo {
            ty: Idx::STR,
            kind: DropKind::Trivial,
        },
    );

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains(&format!("\"_ori_drop${}\"", ty_a.raw())));
    assert!(ir.contains(&format!("\"_ori_drop${}\"", ty_b.raw())));
    assert!(ir.contains(&format!("\"_ori_drop${}\"", Idx::STR.raw())));
    assert_eq!(em.drop_fn_cache.len(), 3);

    drop(em);
}
