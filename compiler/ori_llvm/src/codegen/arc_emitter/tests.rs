//! Tests for ARC IR emitter and drop function generation.
//!
//! Verifies that drop functions are generated with correct LLVM IR structure
//! for each `DropKind` variant, and that caching / edge cases work.
#![allow(
    clippy::too_many_lines,
    reason = "test setup for LLVM IR requires many sequential steps"
)]

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

/// Classifier parameterised by an explicit set of heap-allocated Idx
/// values + `Idx::STR`. Used by recursive-drop codegen tests where
/// the recursive type's Idx is created via `pool.named(...)` and
/// therefore cannot be statically pre-classified by `TestClassifier`.
struct IdxSetClassifier {
    heap_idxs: Vec<Idx>,
}

impl ArcClassification for IdxSetClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if idx == Idx::STR || self.heap_idxs.contains(&idx) {
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        kind: DropKind::Fields {
            fields: vec![(1, Idx::STR)],
            user_drop: None,
        },
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        kind: DropKind::Enum {
            variants: vec![vec![], vec![(1, Idx::STR)]],
            user_drop: None,
        },
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    // AOT mode: aggregate load + extractvalue (no GEP).
    // JIT mode would use GEP + per-field load.
    assert!(
        ir.contains("getelementptr") || ir.contains("extractvalue"),
        "Missing GEP or extractvalue:\n{ir}"
    );
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
            kind: DropKind::Fields {
                fields: vec![(0, Idx::STR)],
                user_drop: None,
            },
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("data"),
            ty: Idx::STR,
            passing: ParamPassing::Direct,
            readonly: false,
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![
            ParamAbi {
                name: interner.intern("base"),
                ty: struct_ty,
                passing: ParamPassing::Direct,
                readonly: false,
            },
            ParamAbi {
                name: interner.intern("val"),
                ty: Idx::INT,
                passing: ParamPassing::Direct,
                readonly: false,
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
fn set_on_boxed_recursive_field_boxes_value_before_store() {
    // Recursive struct `Node = { value: int, next: Node }` whose field 1 is a
    // boxed recursive back-edge. An `ArcInstr::Set { base, field: 1, value }`
    // mutates that field in place. The field slot holds an RC box pointer (per
    // the boxing oracle), so the emitted IR MUST allocate a box (`ori_rc_alloc`),
    // store `value` into it, then store the box pointer into the field slot —
    // mirroring the Construct/Project box-and-load discipline. Reverting the fix
    // (storing `value` directly) removes the `ori_rc_alloc` call: the negative
    // assertion below pins the boxing.
    //
    // Reachability note: surface Ori does not yet emit `Set` on a recursive
    // boxed field today — field assignment is unimplemented and AIMS reuse does
    // not reconstruct recursive structs in place. This is a codegen-level pin on
    // the emission path so the fix cannot silently regress when that path
    // becomes reachable.
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // Build the self-recursive struct: `next` field's type is a Named
    // placeholder resolved back to the struct itself, exactly how the type
    // checker models a recursive back-edge. `resolve_fully` then maps the
    // field type onto the owner, so the boxing oracle's self-cycle arm fires.
    let node_named = pool.named(ori_ir::Name::from_raw(0x0DE_2222));
    let node_ty = pool.struct_type(
        ori_ir::Name::from_raw(0x0DE_2222),
        &[
            (ori_ir::Name::from_raw(301), Idx::INT),
            (ori_ir::Name::from_raw(302), node_named),
        ],
    );
    pool.set_resolution(node_named, node_ty);

    // Sanity: the boxing oracle MUST classify field 1 (`next`) as boxed, else
    // the test would assert against the non-boxed path and silently pass.
    assert!(
        crate::codegen::type_info::repr_box_oracle::position_is_rc_boxed(
            &pool, node_ty, node_named
        ),
        "test precondition: the recursive `next` field must be a boxed back-edge"
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_set_boxed"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_set_boxed_fn", &[ptr_ty, ptr_ty], ptr_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    // Route the recursive node Idx through DefiniteRef so it is treated as
    // heap-allocated (the Set fast path requires an RcPointer base).
    let cl = IdxSetClassifier {
        heap_idxs: vec![node_ty, node_named],
    };
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

    // v0 = base node (RcPointer); v1 = replacement child node (also a Node).
    // Set v0.next(field 1) = v1, then return v0.
    let arc_func = ArcFunction {
        name: interner.intern("test_set_boxed_fn"),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: node_ty,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: node_ty,
                ownership: Ownership::Owned,
            },
        ],
        return_type: node_ty,
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
        var_types: vec![node_ty, node_ty],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::RcPointer],
        spans: vec![vec![None]],
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![
            ParamAbi {
                name: interner.intern("base"),
                ty: node_ty,
                passing: ParamPassing::Direct,
                readonly: false,
            },
            ParamAbi {
                name: interner.intern("val"),
                ty: node_ty,
                passing: ParamPassing::Direct,
                readonly: false,
            },
        ],
        return_abi: ReturnAbi {
            ty: node_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // The boxed-field Set MUST allocate a box before storing into the slot.
    // Assert the CALL site (not the runtime `declare`), so the pin fails when
    // the boxing is reverted to a direct store — the `declare noalias ptr
    // @ori_rc_alloc` line is always present and is NOT evidence of boxing.
    assert!(
        ir.contains("call ptr @ori_rc_alloc"),
        "Set on a boxed recursive field must box the value via a call to \
         ori_rc_alloc (not just store the inline aggregate):\n{ir}"
    );
    // The in-place field write is still GEP + store of the box pointer.
    assert!(
        ir.contains("getelementptr inbounds") && ir.contains("store"),
        "Set must GEP the field slot and store the box pointer:\n{ir}"
    );

    drop(em);
} // set_on_boxed_recursive_field_boxes_value_before_store

// ─── BurdenDecField codegen wire matrix ───

/// Positive pin: `BurdenDecField` on a heap-typed (str) field MUST emit
/// GEP + load + `RcDec` on the prior field value BEFORE the upstream `Set`
/// store clobbers the slot. The `emit_drop_rc_dec` SSOT dispatcher
/// handles the heap-pointer extraction + drop-fn lookup; the
/// `emit_drop_field_loop` shared SSOT-helper handles the GEP+load shape.
#[test]
fn burden_dec_field_str_field_emits_gep_load_rc_dec() {
    use ori_arc::ir::{ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId, ValueRepr};
    use ori_arc::Ownership;

    use super::test_utils::{burden_dec_field_first, entry_block, set_first};
    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // Struct with one str field — heap-burdened; MUST emit RcDec.
    let struct_ty = pool.struct_type(
        ori_ir::Name::from_raw(300),
        &[(ori_ir::Name::from_raw(301), Idx::STR)],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_burden_dec_field_str"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_bdf_str", &[ptr_ty, ptr_ty], ptr_ty);
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
        name: interner.intern("test_bdf_str"),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: struct_ty,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
        ],
        return_type: struct_ty,
        blocks: vec![entry_block(
            vec![
                burden_dec_field_first(ArcVarId::new(0)),
                set_first(ArcVarId::new(0), ArcVarId::new(1)),
            ],
            ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        )],
        entry: ArcBlockId::new(0),
        var_types: vec![struct_ty, Idx::STR],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::RcPointer],
        spans: vec![vec![None, None]],
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![
            ParamAbi {
                name: interner.intern("base"),
                ty: struct_ty,
                passing: ParamPassing::Direct,
                readonly: false,
            },
            ParamAbi {
                name: interner.intern("val"),
                ty: Idx::STR,
                passing: ParamPassing::Direct,
                readonly: false,
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

    // Pin 1: BurdenDecField emitted struct_gep for the field position.
    assert!(
        ir.contains("burden_dec_field.0.ptr"),
        "BurdenDecField MUST emit struct_gep for field position; ir:\n{ir}",
    );
    // Pin 2: BurdenDecField emitted load to capture the field value.
    assert!(
        ir.contains("burden_dec_field.0 "),
        "BurdenDecField MUST emit load to capture prior field value; ir:\n{ir}",
    );
    // Pin 3: emit_drop_rc_dec routed to ori_rc_dec for the heap-typed field.
    // The loaded str value extracts data ptr + RcDec call lands in IR.
    assert!(
        ir.contains("ori_rc_dec"),
        "BurdenDecField on str-typed field MUST route through emit_drop_rc_dec → ori_rc_dec; ir:\n{ir}",
    );

    drop(em);
} // burden_dec_field_str_field_emits_gep_load_rc_dec

/// Negative case per AIMS rule RE-2 scalar exemption: `BurdenDecField`
/// on a scalar-typed (int) field MUST NOT emit `ori_rc_dec` — the
/// `emit_drop_rc_dec` SSOT dispatcher returns early via the canonical
/// scalar short-circuit. The GEP+load still emit (mechanical IR
/// positioning via `emit_drop_field_loop`), but no `RcDec` call lands.
/// Clamps the RE-2 invariant against a regression that inlined a scalar
/// check would only pass if the inlined check matched the canonical SSOT
/// semantics; routing through the SSOT keeps both paths in lockstep.
#[test]
fn burden_dec_field_scalar_field_emits_no_rc_dec_via_re_2_short_circuit() {
    use ori_arc::ir::{ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId, ValueRepr};
    use ori_arc::Ownership;

    use super::test_utils::{burden_dec_field_first, entry_block, set_first};
    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // Struct with one scalar (int) field — NO heap burden; MUST
    // skip RcDec emission per RE-2 scalar exemption.
    let struct_ty = pool.struct_type(
        ori_ir::Name::from_raw(310),
        &[(ori_ir::Name::from_raw(311), Idx::INT)],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_burden_dec_field_scalar"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("test_bdf_scalar", &[ptr_ty, i64_ty], ptr_ty);
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
        name: interner.intern("test_bdf_scalar"),
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
        blocks: vec![entry_block(
            vec![
                burden_dec_field_first(ArcVarId::new(0)),
                set_first(ArcVarId::new(0), ArcVarId::new(1)),
            ],
            ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        )],
        entry: ArcBlockId::new(0),
        var_types: vec![struct_ty, Idx::INT],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::Scalar],
        spans: vec![vec![None, None]],
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![
            ParamAbi {
                name: interner.intern("base"),
                ty: struct_ty,
                passing: ParamPassing::Direct,
                readonly: false,
            },
            ParamAbi {
                name: interner.intern("val"),
                ty: Idx::INT,
                passing: ParamPassing::Direct,
                readonly: false,
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

    // Pin: GEP + load STILL emit (mechanical IR positioning).
    assert!(
        ir.contains("burden_dec_field.0.ptr"),
        "BurdenDecField on scalar field MUST still emit struct_gep; ir:\n{ir}",
    );
    assert!(
        ir.contains("burden_dec_field.0 "),
        "BurdenDecField on scalar field MUST still emit load; ir:\n{ir}",
    );

    // Negative assertion: NO ori_rc_dec call lands for the scalar field
    // (RE-2 scalar exemption via emit_drop_rc_dec → extract_rc_data_ptrs
    // empty-return short-circuit at drop_gen.rs:337-339).
    // The function may still contain other ori_rc_dec calls from drop
    // function emission for the struct itself (its return path); we
    // specifically check there's no rc_dec attributable to the
    // burden_dec_field load — the loaded i64 is not a pointer so
    // extract_rc_data_ptrs returns empty, no RcDec emitted at this site.
    // A regression that re-implemented scalar checks inline incorrectly
    // would emit an ori_rc_dec call referencing the burden_dec_field
    // loaded value.
    assert!(
        !ir.contains("ori_rc_dec(ptr %burden_dec_field"),
        "BurdenDecField on scalar field MUST NOT emit ori_rc_dec on loaded value (RE-2); ir:\n{ir}",
    );

    drop(em);
} // burden_dec_field_scalar_field_emits_no_rc_dec_via_re_2_short_circuit

// ─── BurdenDecVariant codegen wire matrix ───

/// Test-only classifier that classifies a specific enum `Idx` as
/// `DefiniteRef` so `compute_drop_info` walks per-variant fields rather
/// than short-circuiting on the `TestClassifier` `raw >= 100` rule
/// (which would reject any fresh `pool.enum_type` `Idx` at raw 12+).
/// Field `Idx` classification mirrors `TestClassifier` so `Idx::STR`
/// remains `DefiniteRef` and scalar field types remain `Scalar`.
struct EnumDefiniteRef {
    enum_idx: Idx,
}

impl ArcClassification for EnumDefiniteRef {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if idx == self.enum_idx || idx == Idx::STR || idx.raw() >= 100 {
            ArcClass::DefiniteRef
        } else {
            ArcClass::Scalar
        }
    }
}

/// Positive pin: `BurdenDecVariant` on an explicit-tag enum (3 variants
/// mixed unit/scalar/heap) MUST emit a tag switch + per-variant
/// drop blocks + `RcDec` on heap-typed payload fields. Delegates to
/// `emit_variant_burden_walk` — the SSOT helper at `drop_enum.rs`
/// shared with the drop-fn codegen path. Per `canonical_enum` at
/// `compiler/ori_repr/src/canonical/type_repr.rs`, 3-variant enums
/// without tagged-pointer eligibility yield `EnumTag::Explicit { width: I8 }`
/// via `min_tag_width(3) = I8`.
#[test]
fn burden_dec_variant_explicit_tag_enum_emits_switch_and_rc_dec() {
    use ori_arc::ir::{
        ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    use super::test_utils::entry_block;
    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // 3-variant enum mixed unit/scalar/heap → EnumTag::Explicit { width: I8 }.
    let enum_ty = pool.enum_type(
        ori_ir::Name::from_raw(400),
        &[
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(401),
                field_types: vec![Idx::INT],
            },
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(402),
                field_types: vec![Idx::STR],
            },
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(403),
                field_types: vec![Idx::STR, Idx::INT],
            },
        ],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_burden_dec_variant_explicit"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_bdv_explicit", &[ptr_ty], ptr_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = EnumDefiniteRef { enum_idx: enum_ty };
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
        name: interner.intern("test_bdv_explicit"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: enum_ty,
            ownership: Ownership::Owned,
        }],
        return_type: enum_ty,
        blocks: vec![entry_block(
            vec![ArcInstr::BurdenDecVariant {
                var: ArcVarId::new(0),
            }],
            ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        )],
        entry: ArcBlockId::new(0),
        var_types: vec![enum_ty],
        var_reprs: vec![ValueRepr::RcPointer],
        spans: vec![vec![None]],
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("base"),
            ty: enum_ty,
            passing: ParamPassing::Direct,
            readonly: false,
        }],
        return_abi: ReturnAbi {
            ty: enum_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Pin: explicit-tag dispatch emits a switch instruction on the tag.
    assert!(
        ir.contains("switch"),
        "BurdenDecVariant on explicit-tag enum MUST emit tag switch; ir:\n{ir}",
    );
    // Pin: convergence block from emit_variant_burden_walk SSOT helper.
    assert!(
        ir.contains("drop.done"),
        "BurdenDecVariant MUST emit drop.done convergence block from emit_variant_burden_walk SSOT; ir:\n{ir}",
    );
    // Pin: heap-typed (str) payload fields trigger ori_rc_dec via
    // emit_drop_rc_dec dispatcher on the per-variant walk.
    assert!(
        ir.contains("ori_rc_dec"),
        "BurdenDecVariant on enum with str payload MUST emit ori_rc_dec on per-variant walk; ir:\n{ir}",
    );

    drop(em);
} // burden_dec_variant_explicit_tag_enum_emits_switch_and_rc_dec

/// Positive pin: `BurdenDecVariant` on an `Option<str>` MUST route
/// through the Option/Result-specific dispatch in `emit_variant_burden_walk`
/// (`drop_enum.rs` `Tag::Option | Tag::Result` arm) — payload accessed
/// as a typed field at struct index 1, not via byte-offset GEP into a
/// `[M x i64]` payload array (the general-enum arm). With
/// `NICHE_CODEGEN_READY = false` in `canonical/type_repr.rs`, the niche
/// optimization path is gated off; `canonical_option` falls back to
/// `default_option_repr_public` yielding `EnumTag::Explicit { width: I64 }`.
/// This pin clamps the Option/Result payload-as-typed-field arm, distinct
/// from Pin 1's general-enum byte-offset arm.
#[test]
fn burden_dec_variant_option_str_emits_typed_payload_rc_dec() {
    use ori_arc::ir::{
        ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    use super::test_utils::entry_block;
    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let mut pool = Pool::new();
    // Option<str> canonical form via Pool::option. The Option<RcPointer>
    // shape lands in EnumTag::Niche via canonical_enum_for_type fallback
    // when repr_plan is absent.
    let option_str_ty = pool.option(Idx::STR);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_burden_dec_variant_niche"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_bdv_niche", &[ptr_ty], ptr_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = EnumDefiniteRef {
        enum_idx: option_str_ty,
    };
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
        name: interner.intern("test_bdv_niche"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: option_str_ty,
            ownership: Ownership::Owned,
        }],
        return_type: option_str_ty,
        blocks: vec![entry_block(
            vec![ArcInstr::BurdenDecVariant {
                var: ArcVarId::new(0),
            }],
            ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        )],
        entry: ArcBlockId::new(0),
        var_types: vec![option_str_ty],
        var_reprs: vec![ValueRepr::RcPointer],
        spans: vec![vec![None]],
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("base"),
            ty: option_str_ty,
            passing: ParamPassing::Direct,
            readonly: false,
        }],
        return_abi: ReturnAbi {
            ty: option_str_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Pin: tag switch dispatches on the discriminant — Option/Result
    // explicit-tag path emits switch, not niche-encoded conditional.
    assert!(
        ir.contains("switch"),
        "BurdenDecVariant on Option<str> MUST emit tag switch (explicit-tag path); ir:\n{ir}",
    );
    // Pin: convergence block from emit_variant_burden_walk SSOT helper.
    assert!(
        ir.contains("drop.done"),
        "BurdenDecVariant on Option<str> MUST emit drop.done convergence block; ir:\n{ir}",
    );
    // Pin: Option/Result arm of emit_variant_burden_walk emits typed
    // payload access (payload.ptr / payload) rather than the general-enum
    // [M x i64] byte-offset GEPs (f0.ptr / f0 etc.). The struct_idx = 1
    // + field_index GEP into a typed field at drop_enum.rs Tag::Option
    // arm yields the canonical "payload" naming.
    assert!(
        ir.contains("payload.ptr"),
        "BurdenDecVariant on Option<str> MUST emit payload.ptr GEP via Tag::Option arm; ir:\n{ir}",
    );
    assert!(
        ir.contains("payload"),
        "BurdenDecVariant on Option<str> MUST emit payload load via Tag::Option arm; ir:\n{ir}",
    );
    // Pin: heap-typed (str) payload triggers ori_rc_dec on the variant
    // walk via emit_drop_rc_dec.
    assert!(
        ir.contains("ori_rc_dec"),
        "BurdenDecVariant on Option<str> MUST emit ori_rc_dec on Some(str) payload; ir:\n{ir}",
    );

    drop(em);
} // burden_dec_variant_option_str_emits_typed_payload_rc_dec

/// Negative case per AIMS rule RE-2 scalar exemption: `BurdenDecVariant`
/// on a scalar-classified enum (`Idx::ORDERING`, raw=11, all variants
/// unit) MUST NOT emit any per-variant codegen — `compute_drop_info`
/// returns None for scalars, triggering the short-circuit at
/// `instr_dispatch.rs` (`BurdenDecVariant` arm `Some(drop_info) else return`).
/// Clamps AIMS Invariant 5 `RcOnScalar`: no switch, no `ori_rc_dec`, no
/// per-variant blocks emit for scalar enum bases. `TestClassifier`
/// correctly classifies `Idx::ORDERING` as `Scalar` because raw < 100
/// AND not `Idx::STR`.
#[test]
fn burden_dec_variant_scalar_enum_emits_no_codegen_via_re_2_short_circuit() {
    use ori_arc::ir::{
        ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    use super::test_utils::entry_block;
    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    let pool = Pool::new();

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_burden_dec_variant_scalar"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("test_bdv_scalar", &[i64_ty], i64_ty);
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
        name: interner.intern("test_bdv_scalar"),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::ORDERING,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::ORDERING,
        blocks: vec![entry_block(
            vec![ArcInstr::BurdenDecVariant {
                var: ArcVarId::new(0),
            }],
            ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        )],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::ORDERING],
        var_reprs: vec![ValueRepr::Scalar],
        spans: vec![vec![None]],
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("base"),
            ty: Idx::ORDERING,
            passing: ParamPassing::Direct,
            readonly: false,
        }],
        return_abi: ReturnAbi {
            ty: Idx::ORDERING,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // Pin: function shell still emits (entry block + return).
    assert!(
        ir.contains("define"),
        "Function shell MUST still emit even when BurdenDecVariant short-circuits; ir:\n{ir}",
    );
    assert!(
        ir.contains("entry:"),
        "Entry block MUST still emit; ir:\n{ir}",
    );

    // Negative case: scalar short-circuit at compute_drop_info None
    // means no per-variant codegen lands inside the test function. The
    // module contains an `ori_rc_dec` runtime declaration emitted by
    // `declare_runtime`; assertions target `call` instructions (which
    // only emit at use sites) rather than bare substring matches.
    // Convergence block name `drop.done` and per-variant block prefix
    // `variant.` only appear in emitted bodies, not declarations.
    assert!(
        !ir.contains("switch"),
        "BurdenDecVariant on scalar enum MUST NOT emit tag switch (RE-2); ir:\n{ir}",
    );
    assert!(
        !ir.contains("call void @ori_rc_dec"),
        "BurdenDecVariant on scalar enum MUST NOT emit ori_rc_dec call (RE-2); ir:\n{ir}",
    );
    assert!(
        !ir.contains("drop.done"),
        "BurdenDecVariant on scalar enum MUST NOT emit drop.done convergence block (RE-2); ir:\n{ir}",
    );
    assert!(
        !ir.contains("variant."),
        "BurdenDecVariant on scalar enum MUST NOT emit per-variant block prefix (RE-2); ir:\n{ir}",
    );

    drop(em);
} // burden_dec_variant_scalar_enum_emits_no_codegen_via_re_2_short_circuit

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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("obj"),
            ty: enum_ty,
            passing: ParamPassing::Direct,
            readonly: false,
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    // Str LLVM type: {i64 len, i64 cap, ptr data}
    let str_llvm = scx.type_struct(
        &[
            scx.type_i64().into(),
            scx.type_i64().into(),
            scx.type_ptr().into(),
        ],
        false,
    );
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("s"),
            ty: Idx::STR,
            passing: ParamPassing::Direct,
            readonly: false,
        }],
        return_abi: ReturnAbi {
            ty: Idx::STR,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // FatPointer Dec extracts data_ptr at field 2 (SSO layout: {len, cap, data})
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("f"),
            ty: fn_ty,
            passing: ParamPassing::Direct,
            readonly: false,
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
fn rc_inc_inline_enum_emits_tag_switch() {
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("r"),
            ty: result_ty,
            passing: ParamPassing::Direct,
            readonly: false,
        }],
        return_abi: ReturnAbi {
            ty: result_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    };
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    // InlineEnum Inc emits a tag-switch with per-variant field inc.
    // For Result<int, str>: the Err variant has an RC-typed field (str),
    // so the switch should have a case that calls ori_rc_inc.
    // The Ok variant (int) has no RC fields → no case.
    assert!(
        ir.contains("rc_inc.tag"),
        "InlineEnum RcInc should emit tag-switch, missing rc_inc.tag:\n{ir}"
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("r"),
            ty: result_ty,
            passing: ParamPassing::Direct,
            readonly: false,
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
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
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
        ..Default::default()
    };

    let abi = FunctionAbi {
        params: vec![ParamAbi {
            name: interner.intern("data"),
            ty: Idx::STR,
            passing: ParamPassing::Direct,
            readonly: false,
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

// Idx-to-TypeTag bridge tests

#[test]
fn idx_to_type_tag_maps_all_primitive_constants() {
    use ori_registry::TypeTag;

    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_type_tag"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // All 11 well-known Idx constants (excluding ERROR) must map correctly.
    let cases: &[(Idx, TypeTag)] = &[
        (Idx::INT, TypeTag::Int),
        (Idx::FLOAT, TypeTag::Float),
        (Idx::BOOL, TypeTag::Bool),
        (Idx::STR, TypeTag::Str),
        (Idx::CHAR, TypeTag::Char),
        (Idx::BYTE, TypeTag::Byte),
        (Idx::UNIT, TypeTag::Unit),
        (Idx::NEVER, TypeTag::Never),
        (Idx::DURATION, TypeTag::Duration),
        (Idx::SIZE, TypeTag::Size),
        (Idx::ORDERING, TypeTag::Ordering),
    ];

    for &(idx, expected_tag) in cases {
        let result = em.idx_to_type_tag(idx);
        assert_eq!(
            result,
            Some(expected_tag),
            "Idx {idx:?} should map to TypeTag::{expected_tag:?}",
        );
    }

    drop(em);
}

#[test]
fn idx_to_type_tag_returns_error_tag_for_error_idx() {
    use ori_registry::TypeTag;

    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_error_tag"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // ERROR idx (8) falls through to dynamic path → TypeInfo::Error → TypeTag::Error.
    let result = em.idx_to_type_tag(Idx::ERROR);
    assert_eq!(
        result,
        Some(TypeTag::Error),
        "Idx::ERROR should map to TypeTag::Error via dynamic path",
    );

    drop(em);
}

#[test]
fn idx_to_type_tag_maps_dynamic_list_type() {
    use ori_registry::TypeTag;

    let mut pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();

    // Create a dynamic list type (Idx >= 64)
    let list_idx = pool.list(Idx::INT);
    assert!(
        list_idx.raw() >= Idx::FIRST_DYNAMIC,
        "list type should be dynamically allocated"
    );

    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_dynamic_tag"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = TestClassifier;
    let codegen_ctx = super::CodegenContext::default();

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
    );

    // Dynamic list type should map to TypeTag::List via TypeInfoStore
    assert_eq!(em.idx_to_type_tag(list_idx), Some(TypeTag::List));

    drop(em);
}

// ─── Recursive drop-fn codegen ─────────────────────────────────────
//
// Cycle safety comes from the cache-before-body pattern at drop_gen.rs:69:
// `drop_fn_cache.insert(ty, func_id)` runs BEFORE the body is generated, so
// `emit_rc_dec` on a field of the same type hits the cache and the recursion
// terminates structurally — no runtime type discriminants.

#[test]
fn recursive_node_drop_fn_emits_self_referencing_rc_dec() {
    // Recursive type Node { value: int, next: Option<Node> } modelled as
    // a single heap-allocated struct whose RC'd field references itself.
    // The drop body MUST emit a GEP + load + ori_rc_dec on the self-typed
    // field. Cycle safety: only ONE drop function definition appears,
    // even though the field type recurses.
    //
    // Construct the "recursive" Idx through `pool.named` so it is a
    // well-formed pool entry; the field-type self-reference in DropKind
    // exercises the cache-before-body cycle-termination path at
    // drop_gen.rs:69 + the recursive emit_rc_dec → get_or_generate_drop_fn
    // chain.
    let mut pool = Pool::new();
    let node_ty = pool.named(ori_ir::Name::from_raw(0x0DE_0001));
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_recursive_node"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    // Classifier: route the recursive node Idx through DefiniteRef so
    // emit_drop_rc_dec actually emits an ori_rc_dec call (scalar would
    // short-circuit via RE-2).
    let cl = IdxSetClassifier {
        heap_idxs: vec![node_ty],
    };
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
        ty: node_ty,
        kind: DropKind::Fields {
            fields: vec![(0, node_ty)],
            user_drop: None,
        },
    };
    let _ = super::drop_gen::generate_drop_fn(&mut em, node_ty, &info);

    let ir = scx.llmod.print_to_string().to_string();
    let mangled = format!("\"_ori_drop${}\"", node_ty.raw());
    let definitions = ir.matches(&format!("define void @{mangled}(ptr")).count();
    assert_eq!(
        definitions, 1,
        "recursive type yields exactly ONE drop fn definition (cache prevents duplicate generation):\n{ir}"
    );
    assert!(
        ir.contains("ori_rc_dec"),
        "recursive node drop fn MUST emit ori_rc_dec on the self-typed field:\n{ir}"
    );

    drop(em);
}

#[test]
fn mutually_recursive_tree_forest_drop_fns_cross_reference() {
    // Mutually-recursive pair: Tree's drop fn references Forest's drop fn
    // (via field decrement) and vice versa. Both drop fns MUST be emitted
    // exactly once; their bodies MUST cross-reference each other through
    // the runtime ori_rc_dec call. The cache reservation order during SCC
    // processing — insert ALL SCC members into drop_fn_cache BEFORE
    // generating bodies — keeps the cross-reference acyclic at LLVM level.
    let mut pool = Pool::new();
    let tree_ty = pool.named(ori_ir::Name::from_raw(0x07EE_0001));
    let forest_ty = pool.named(ori_ir::Name::from_raw(0x07EE_0002));
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_tree_forest"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    // Classifier: route both Tree and Forest through DefiniteRef so the
    // cross-reference RC decs actually emit (vs short-circuiting via RE-2).
    let cl = IdxSetClassifier {
        heap_idxs: vec![tree_ty, forest_ty],
    };
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

    let tree_info = DropInfo {
        ty: tree_ty,
        kind: DropKind::Fields {
            fields: vec![(0, forest_ty)],
            user_drop: None,
        },
    };
    let forest_info = DropInfo {
        ty: forest_ty,
        kind: DropKind::Fields {
            fields: vec![(0, tree_ty)],
            user_drop: None,
        },
    };

    // Generate Tree first; its body references Forest which triggers a
    // recursive get_or_generate_drop_fn call. Forest's body in turn
    // references Tree — the cache prevents another regeneration.
    let tree_fn = super::drop_gen::generate_drop_fn(&mut em, tree_ty, &tree_info);
    // After Tree, generate Forest explicitly to exercise the explicit
    // emission path (the recursive get_or_generate path may already have
    // declared Forest's prototype without a body — generate_drop_fn handles
    // both cases via the `function_has_body` guard).
    let forest_fn = super::drop_gen::generate_drop_fn(&mut em, forest_ty, &forest_info);

    // Distinct FunctionIds — each type gets its own drop function.
    assert_ne!(
        tree_fn, forest_fn,
        "Tree and Forest MUST receive distinct FunctionIds (per-type mangling)"
    );

    let ir = scx.llmod.print_to_string().to_string();
    let tree_mangled = format!("\"_ori_drop${}\"", tree_ty.raw());
    let forest_mangled = format!("\"_ori_drop${}\"", forest_ty.raw());

    let tree_defs = ir
        .matches(&format!("define void @{tree_mangled}(ptr"))
        .count();
    let forest_defs = ir
        .matches(&format!("define void @{forest_mangled}(ptr"))
        .count();
    assert_eq!(tree_defs, 1, "Tree drop fn defined exactly once:\n{ir}");
    assert_eq!(forest_defs, 1, "Forest drop fn defined exactly once:\n{ir}");

    // Cross-reference: both bodies must call ori_rc_dec (the recursive
    // child decrement that resolves through the cache).
    assert!(
        ir.contains("ori_rc_dec"),
        "mutually-recursive drop fns MUST call ori_rc_dec on cross-typed fields:\n{ir}"
    );

    drop(em);
}

#[test]
fn drop_fn_cache_prevents_infinite_generation() {
    // The cache-before-body pattern at drop_gen.rs:69 guarantees that any
    // recursive descent through `emit_rc_dec → get_or_generate_drop_fn →
    // generate_drop_fn` terminates: the cache entry exists BEFORE body
    // emission, so the inner call returns the cached FunctionId.
    //
    // We exercise this by invoking generate_drop_fn TWICE on the same
    // recursive type. The second call MUST be a cache hit (no second
    // body emitted).
    let mut pool = Pool::new();
    let node_ty = pool.named(ori_ir::Name::from_raw(0x0CAC_4E55));
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_drop_fn_cache"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let cl = IdxSetClassifier {
        heap_idxs: vec![node_ty],
    };
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
        ty: node_ty,
        kind: DropKind::Fields {
            fields: vec![(0, node_ty)],
            user_drop: None,
        },
    };

    let _first = super::drop_gen::generate_drop_fn(&mut em, node_ty, &info);
    // After the first generation, the cache MUST hold an entry for node_ty.
    let cached_before_second = em.drop_fn_cache.get(&node_ty).copied();
    assert!(
        cached_before_second.is_some(),
        "after first invocation, the cache MUST contain an entry for the recursive type"
    );
    let _second = super::drop_gen::generate_drop_fn(&mut em, node_ty, &info);
    // The cache entry MUST remain stable across repeated invocations
    // (the FunctionId arena handle may differ across `push_function`
    // calls, but the cached entry tracks the most recent reservation).
    assert!(
        em.drop_fn_cache.contains_key(&node_ty),
        "cache entry MUST persist across repeated invocations"
    );

    let ir = scx.llmod.print_to_string().to_string();
    let mangled = format!("\"_ori_drop${}\"", node_ty.raw());
    let definitions = ir.matches(&format!("define void @{mangled}(ptr")).count();
    assert_eq!(
        definitions, 1,
        "cache MUST prevent duplicate drop fn definitions even under repeated invocation:\n{ir}"
    );

    drop(em);
}
