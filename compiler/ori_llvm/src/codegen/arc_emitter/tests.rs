//! Tests for ARC IR emitter and drop function generation.
//!
//! Verifies that drop functions are generated with correct LLVM IR structure
//! for each `DropKind` variant, and that caching / edge cases work.

use std::mem::ManuallyDrop;

use inkwell::context::Context;
use ori_arc::{ArcClass, ArcClassification, DropInfo, DropKind};
use ori_ir::StringInterner;
use ori_types::{Idx, Pool};

use crate::codegen::abi::FunctionAbi;
use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::runtime_decl::declare_runtime;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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
fn get_or_generate_returns_null_for_scalar_type() {
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // Scalar types (like int) don't need drop — should return null pointer
    let drop_fn = em.get_or_generate_drop_fn(Idx::INT);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        !ir.contains(&format!("\"_ori_drop${}\"", Idx::INT.raw())),
        "Scalar types should not generate drop functions:\n{ir}"
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
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

// ─── IsShared inline check tests ───

#[test]
fn is_shared_emits_gep_load_icmp() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_is_shared"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    // Declare a function: (ptr) -> i1
    let ptr_ty = builder.ptr_type();
    let bool_ty = builder.bool_type();
    let host = builder.declare_function("test_fn", &[ptr_ty], bool_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // Build a minimal ArcFunction: param v0 (ptr), IsShared dst=v1 var=v0, Return v1
    // v0 must have RcPointer repr — IsShared only emits the GEP/load/icmp
    // sequence for heap-allocated RC'd values (pointer-typed).
    let arc_func = ArcFunction {
        name: interner.intern("test_fn"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::BOOL,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::IsShared {
                dst: ArcVarId::new(1),
                var: ArcVarId::new(0),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::BOOL],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::Scalar],
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("data"),
            ty: Idx::STR,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: Idx::BOOL,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Verify the 3-instruction inline sequence:
    // 1. GEP i8 with -8 offset to reach refcount header
    assert!(
        ir.contains("getelementptr") && ir.contains("i8") && ir.contains("-8"),
        "Expected GEP i8 with -8 offset for RC header:\n{ir}"
    );
    // 2. Load i64 refcount
    assert!(
        ir.contains("load i64"),
        "Expected i64 load for refcount:\n{ir}"
    );
    // 3. icmp sgt for > 1 comparison
    assert!(
        ir.contains("icmp sgt i64"),
        "Expected signed greater-than comparison:\n{ir}"
    );

    drop(em);
}

// ─── Set / SetTag (reuse fast path) tests ───

#[test]
fn set_emits_struct_gep_and_store() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // Create a struct type with 2 int fields
    let struct_ty = pool.struct_type(
        ori_ir::Name::from_raw(200),
        &[
            (ori_ir::Name::from_raw(201), Idx::INT),
            (ori_ir::Name::from_raw(202), Idx::INT),
        ],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_set"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("test_set_fn", &[ptr_ty, i64_ty], ptr_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // ArcFunction: v0 (struct ptr), v1 (int), then Set v0.field(1) = v1, return v0
    // v0 must have RcPointer repr — Set uses GEP+store which requires a pointer.
    let arc_func = ArcFunction {
        name: interner.intern("test_set_fn"),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: struct_ty,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
        ],
        return_type: struct_ty,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Set {
                base: ArcVarId::new(0),
                field: 1,
                value: ArcVarId::new(1),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![struct_ty, Idx::INT],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::Scalar],
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![
            ParamAbi {
                name: interner.intern("base"),
                ty: struct_ty,
                passing: ParamPassing::Direct,
            },
            ParamAbi {
                name: interner.intern("val"),
                ty: Idx::INT,
                passing: ParamPassing::Direct,
            },
        ],
        return_abi: ReturnAbi {
            ty: struct_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Verify GEP-based field access (struct_gep for field 1)
    assert!(
        ir.contains("getelementptr inbounds"),
        "Expected struct GEP for in-place field set:\n{ir}"
    );
    // Verify store instruction
    assert!(
        ir.contains("store"),
        "Expected store for in-place field mutation:\n{ir}"
    );
    // Should NOT contain insert_value (old value-level approach)
    assert!(
        !ir.contains("insertvalue"),
        "Set should use GEP+store, not insertvalue:\n{ir}"
    );

    drop(em);
} // set_emits_struct_gep_and_store

#[test]
fn set_tag_emits_gep_and_store() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // Create an enum type
    let enum_ty = pool.enum_type(
        ori_ir::Name::from_raw(210),
        &[
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(211),
                field_types: vec![Idx::INT],
            },
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(212),
                field_types: vec![Idx::FLOAT],
            },
        ],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_set_tag"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_tag_fn", &[ptr_ty], ptr_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // ArcFunction: v0 (enum ptr), then SetTag v0 tag=1, return v0
    let arc_func = ArcFunction {
        name: interner.intern("test_tag_fn"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: enum_ty,
            ownership: Ownership::Owned,
        }],
        return_type: enum_ty,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::SetTag {
                base: ArcVarId::new(0),
                tag: 1,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![enum_ty],
        var_reprs: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("obj"),
            ty: enum_ty,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: enum_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Verify GEP to tag field (field 0)
    assert!(
        ir.contains("getelementptr inbounds"),
        "Expected struct GEP for tag field:\n{ir}"
    );
    // Verify store of the tag value
    assert!(
        ir.contains("store"),
        "Expected store for tag mutation:\n{ir}"
    );

    drop(em);
} // set_tag_emits_gep_and_store

// ─── EmittedValue helper method tests ───

#[test]
fn emitted_value_into_raw_single_variants() {
    let ctx = Context::create();
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_ev_raw"));
    let mut builder = IrBuilder::new(&scx);

    let v1 = builder.const_i64(1);
    let v2 = builder.const_i64(2);
    let v3 = builder.const_i64(3);

    assert_eq!(super::EmittedValue::Immediate(v1).into_raw(), v1);
    assert_eq!(super::EmittedValue::RcPointer(v2).into_raw(), v2);
    assert_eq!(super::EmittedValue::Aggregate(v3).into_raw(), v3);
}

#[test]
#[should_panic(expected = "Pair has no single ValueId")]
fn emitted_value_into_raw_panics_on_pair() {
    use crate::codegen::value_id::ValueId;

    super::EmittedValue::Pair {
        first: ValueId::NONE,
        second: ValueId::NONE,
    }
    .into_raw();
}

#[test]
#[should_panic(expected = "ZeroSized has no ValueId")]
fn emitted_value_into_raw_panics_on_zero_sized() {
    super::EmittedValue::ZeroSized.into_raw();
}

#[test]
fn emitted_value_rc_data_ptr() {
    let ctx = Context::create();
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_ev_rc"));
    let mut builder = IrBuilder::new(&scx);

    let v1 = builder.const_i64(10);
    let v2 = builder.const_i64(20);
    let v3 = builder.const_i64(30);

    // RcPointer returns the pointer itself
    assert_eq!(super::EmittedValue::RcPointer(v1).rc_data_ptr(), Some(v1));

    // Pair returns the second component (the RC-managed pointer)
    assert_eq!(
        super::EmittedValue::Pair {
            first: v2,
            second: v3
        }
        .rc_data_ptr(),
        Some(v3)
    );

    // Non-RC variants return None
    assert_eq!(super::EmittedValue::Immediate(v1).rc_data_ptr(), None);
    assert_eq!(super::EmittedValue::Aggregate(v2).rc_data_ptr(), None);
    assert_eq!(super::EmittedValue::ZeroSized.rc_data_ptr(), None);
}

#[test]
fn emitted_value_is_rc_managed() {
    let ctx = Context::create();
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_ev_managed"));
    let mut builder = IrBuilder::new(&scx);
    let v = builder.const_i64(0);

    assert!(super::EmittedValue::RcPointer(v).is_rc_managed());
    assert!(super::EmittedValue::Pair {
        first: v,
        second: v
    }
    .is_rc_managed());
    assert!(!super::EmittedValue::Immediate(v).is_rc_managed());
    assert!(!super::EmittedValue::Aggregate(v).is_rc_managed());
    assert!(!super::EmittedValue::ZeroSized.is_rc_managed());
}

#[test]
fn emitted_value_from_repr() {
    use ori_arc::ir::ValueRepr;

    let ctx = Context::create();
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_ev_repr"));
    let mut builder = IrBuilder::new(&scx);
    let v = builder.const_i64(42);

    // Scalar → Immediate
    assert!(matches!(
        super::EmittedValue::from_repr(ValueRepr::Scalar, v),
        super::EmittedValue::Immediate(_)
    ));

    // RcPointer → RcPointer
    assert!(matches!(
        super::EmittedValue::from_repr(ValueRepr::RcPointer, v),
        super::EmittedValue::RcPointer(_)
    ));

    // Aggregate → Aggregate
    assert!(matches!(
        super::EmittedValue::from_repr(ValueRepr::Aggregate, v),
        super::EmittedValue::Aggregate(_)
    ));

    // FatValue → Aggregate (fat values packed as single LLVM struct)
    assert!(matches!(
        super::EmittedValue::from_repr(ValueRepr::FatValue, v),
        super::EmittedValue::Aggregate(_)
    ));
}

// ─── RC strategy dispatch tests ───

/// Verify `FatPointer` `RcDec` extracts `data_ptr` at field 1 and calls `ori_rc_dec`.
#[test]
fn rc_dec_fat_pointer_extracts_data_ptr() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_fat_dec"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    // Str LLVM type: {i64, ptr}
    let str_llvm = scx.type_struct(&[scx.type_i64().into(), scx.type_ptr().into()], false);
    let str_param_ty = builder.register_type(str_llvm.into());
    let str_ret_ty = builder.register_type(str_llvm.into());
    let host = builder.declare_function("test_fat_dec", &[str_param_ty], str_ret_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    let arc_func = ArcFunction {
        name: interner.intern("test_fat_dec"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::STR,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::FatPointer,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR],
        var_reprs: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("s"),
            ty: Idx::STR,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: Idx::STR,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // FatPointer Dec extracts data_ptr at field 1
    assert!(
        ir.contains("rc_dec.fat_data"),
        "Expected extractvalue for str data_ptr:\n{ir}"
    );
    // Calls ori_rc_dec on the extracted data ptr
    assert!(ir.contains("ori_rc_dec"), "Expected ori_rc_dec call:\n{ir}");

    drop(em);
}

/// Verify Closure `RcDec` extracts `env_ptr`, null-checks, and calls `ori_rc_dec`.
#[test]
fn rc_dec_closure_null_checks_env() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    let fn_ty = pool.function(&[Idx::INT], Idx::INT);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_clos_dec"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    // Closure LLVM type: {ptr, ptr}
    let closure_llvm_ty = builder.closure_type();
    let closure_ret_ty = builder.closure_type();
    let host = builder.declare_function("test_clos_dec", &[closure_llvm_ty], closure_ret_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    let arc_func = ArcFunction {
        name: interner.intern("test_clos_dec"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: fn_ty,
            ownership: Ownership::Owned,
        }],
        return_type: fn_ty,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::Closure,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![fn_ty],
        var_reprs: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("f"),
            ty: fn_ty,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: fn_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Closure Dec extracts env_ptr at field 1
    assert!(
        ir.contains("rc_dec.env"),
        "Expected extractvalue for closure env_ptr:\n{ir}"
    );
    // Null-checks the env_ptr (zero-capture closures have null env)
    assert!(
        ir.contains("rc_dec.null"),
        "Expected null check on env_ptr:\n{ir}"
    );
    // Branches: rc_dec.do (non-null) and rc_dec.skip (null)
    assert!(
        ir.contains("rc_dec.do") && ir.contains("rc_dec.skip"),
        "Expected branch blocks for null check:\n{ir}"
    );
    // Calls ori_rc_dec in the do branch
    assert!(ir.contains("ori_rc_dec"), "Expected ori_rc_dec call:\n{ir}");

    drop(em);
}

/// Verify `InlineEnum` `RcInc` is a no-op (no `ori_rc_inc` call generated).
#[test]
fn rc_inc_inline_enum_is_noop() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    let result_ty = pool.result(Idx::INT, Idx::STR);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_enum_inc"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    // Result LLVM type: {i64, {i64, ptr}} — tag + max(ok, err) payload
    let payload = scx.type_struct(&[scx.type_i64().into(), scx.type_ptr().into()], false);
    let result_llvm = scx.type_struct(&[scx.type_i64().into(), payload.into()], false);
    let result_param_ty = builder.register_type(result_llvm.into());
    let result_ret_ty = builder.register_type(result_llvm.into());
    let host = builder.declare_function("test_enum_inc", &[result_param_ty], result_ret_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    let arc_func = ArcFunction {
        name: interner.intern("test_enum_inc"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: result_ty,
            ownership: Ownership::Owned,
        }],
        return_type: result_ty,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::RcInc {
                var: ArcVarId::new(0),
                count: 1,
                strategy: RcStrategy::InlineEnum,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![result_ty],
        var_reprs: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("r"),
            ty: result_ty,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: result_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // InlineEnum Inc is intentionally a no-op — no *call* to ori_rc_inc should appear.
    // (The module still has a `declare void @ori_rc_inc(ptr)` from declare_runtime.)
    assert!(
        !ir.contains("call void @ori_rc_inc"),
        "InlineEnum RcInc should be no-op but found call to ori_rc_inc:\n{ir}"
    );

    drop(em);
}

/// Verify `InlineEnum` `RcDec` generates a tag-switch with per-variant cleanup.
#[test]
fn rc_dec_inline_enum_tag_switches() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    let result_ty = pool.result(Idx::INT, Idx::STR);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_enum_dec"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    // Result LLVM type: {i64, {i64, ptr}} — tag + max(ok, err) payload
    let payload = scx.type_struct(&[scx.type_i64().into(), scx.type_ptr().into()], false);
    let result_llvm = scx.type_struct(&[scx.type_i64().into(), payload.into()], false);
    let result_param_ty = builder.register_type(result_llvm.into());
    let result_ret_ty = builder.register_type(result_llvm.into());
    let host = builder.declare_function("test_enum_dec", &[result_param_ty], result_ret_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    let arc_func = ArcFunction {
        name: interner.intern("test_enum_dec"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: result_ty,
            ownership: Ownership::Owned,
        }],
        return_type: result_ty,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::InlineEnum,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![result_ty],
        var_reprs: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("r"),
            ty: result_ty,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: result_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // InlineEnum Dec stores to alloca for GEP access
    assert!(
        ir.contains("rc_dec.enum"),
        "Expected alloca for enum value:\n{ir}"
    );
    // Loads tag from field 0
    assert!(ir.contains("rc_dec.tag"), "Expected tag load:\n{ir}");
    // Switch on tag for per-variant cleanup
    assert!(
        ir.contains("switch"),
        "Expected switch instruction for tag dispatch:\n{ir}"
    );
    // Convergence block
    assert!(
        ir.contains("rc_dec.done"),
        "Expected convergence block:\n{ir}"
    );
    // Err variant (tag 1) has str which needs ori_rc_dec
    assert!(
        ir.contains("ori_rc_dec"),
        "Expected ori_rc_dec for RC'd err variant:\n{ir}"
    );

    drop(em);
}

/// Verify `HeapPointer` `RcDec` calls `ori_rc_dec` with a drop function.
#[test]
fn rc_dec_heap_pointer_calls_ori_rc_dec() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_heap_dec"));
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_heap_dec", &[ptr_ty], ptr_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // Use Idx::STR as the type — TestClassifier marks it as DefiniteRef.
    // HeapPointer handler falls through to default path (treats value as the RC ptr).
    let arc_func = ArcFunction {
        name: interner.intern("test_heap_dec"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::STR,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR],
        var_reprs: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("data"),
            ty: Idx::STR,
            passing: ParamPassing::Direct,
        }],
        return_abi: ReturnAbi {
            ty: Idx::STR,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // HeapPointer Dec calls ori_rc_dec with a drop function
    assert!(ir.contains("ori_rc_dec"), "Expected ori_rc_dec call:\n{ir}");
    // Drop function should be generated for the str type
    let name = format!("\"_ori_drop${}\"", Idx::STR.raw());
    assert!(
        ir.contains(&name),
        "Expected drop function for str type:\n{ir}"
    );

    drop(em);
}
