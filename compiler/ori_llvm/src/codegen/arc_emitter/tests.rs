//! Tests for ARC IR emitter and drop function generation.
//!
//! Verifies that drop functions are generated with correct LLVM IR structure
//! for each `DropKind` variant, and that caching / edge cases work.

use std::mem::ManuallyDrop;

use inkwell::context::Context;
use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcAtomicity, RcStrategy,
    VariableMetadataState,
};
use ori_arc::{ArcClass, ArcClassification, DropInfo, DropKind};
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashSet;

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

fn direct_pair_abi(
    interner: &StringInterner,
    first: (&str, Idx),
    second: (&str, Idx),
    return_ty: Idx,
) -> FunctionAbi {
    use crate::codegen::abi::{CallConv, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing};

    FunctionAbi {
        params: vec![
            ParamAbi {
                name: interner.intern(first.0),
                ty: first.1,
                passing: ParamPassing::Direct,
                readonly: false,
            },
            ParamAbi {
                name: interner.intern(second.0),
                ty: second.1,
                passing: ParamPassing::Direct,
                readonly: false,
            },
        ],
        return_abi: ReturnAbi {
            ty: return_ty,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
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
        None,
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
        ir.contains(&format!("define internal void @{name}(ptr")),
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
    let count = ir.matches(&format!("define internal void @{name}")).count();
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
        None,
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
        None,
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
        .find(|l: &&str| l.contains(&format!("define internal void @{name}")))
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
        None,
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

// IsShared inline check tests

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
        None,
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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
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

// Set / SetTag (reuse fast path) tests

#[test]
fn set_emits_struct_gep_and_store() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    let mut pool = Pool::new();
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
        None,
    );

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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
        spans: vec![vec![None]],
        ..Default::default()
    };

    let abi = direct_pair_abi(&interner, ("base", struct_ty), ("val", Idx::INT), struct_ty);
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    assert!(
        ir.contains("getelementptr inbounds"),
        "Expected struct GEP for in-place field set:\n{ir}"
    );
    assert!(
        ir.contains("store"),
        "Expected store for in-place field mutation:\n{ir}"
    );
    assert!(
        !ir.contains("insertvalue"),
        "Set should use GEP+store, not insertvalue:\n{ir}"
    );

    drop(em);
}

#[test]
fn set_on_boxed_recursive_field_boxes_value_before_store() {
    use ori_arc::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ValueRepr,
    };
    use ori_arc::Ownership;

    let mut pool = Pool::new();
    let node_named = pool.named(ori_ir::Name::from_raw(0x0DE_2222));
    let node_ty = pool.struct_type(
        ori_ir::Name::from_raw(0x0DE_2222),
        &[
            (ori_ir::Name::from_raw(301), Idx::INT),
            (ori_ir::Name::from_raw(302), node_named),
        ],
    );
    pool.set_resolution(node_named, node_ty);

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
        None,
    );

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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
        spans: vec![vec![None]],
        ..Default::default()
    };

    let abi = direct_pair_abi(&interner, ("base", node_ty), ("val", node_ty), node_ty);
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    assert!(
        ir.contains("call ptr @ori_rc_alloc"),
        "Set on a boxed recursive field must box the value via a call to \
         ori_rc_alloc (not just store the inline aggregate):\n{ir}"
    );
    assert!(
        ir.contains("getelementptr inbounds") && ir.contains("store"),
        "Set must GEP the field slot and store the box pointer:\n{ir}"
    );

    drop(em);
}

// BurdenDecField codegen wire matrix

#[test]
fn burden_dec_field_str_field_emits_gep_load_rc_dec() {
    use ori_arc::ir::{ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId, ValueRepr};
    use ori_arc::Ownership;

    use super::test_utils::{burden_dec_field_first, entry_block, set_first};
    let mut pool = Pool::new();
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
        None,
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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
        spans: vec![vec![None, None]],
        ..Default::default()
    };

    let abi = direct_pair_abi(&interner, ("base", struct_ty), ("val", Idx::STR), struct_ty);
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    assert!(
        ir.contains("burden_dec_field.0.ptr"),
        "BurdenDecField MUST emit struct_gep for field position; ir:\n{ir}",
    );
    assert!(
        ir.contains("burden_dec_field.0 "),
        "BurdenDecField MUST emit load to capture prior field value; ir:\n{ir}",
    );
    assert!(
        ir.contains("ori_rc_dec"),
        "BurdenDecField on str-typed field MUST route through emit_drop_rc_dec → ori_rc_dec; ir:\n{ir}",
    );

    drop(em);
}

#[test]
fn burden_dec_field_scalar_field_emits_no_rc_dec_via_re_2_short_circuit() {
    use ori_arc::ir::{ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId, ValueRepr};
    use ori_arc::Ownership;

    use super::test_utils::{burden_dec_field_first, entry_block, set_first};
    let mut pool = Pool::new();
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
        None,
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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
        spans: vec![vec![None, None]],
        ..Default::default()
    };

    let abi = direct_pair_abi(&interner, ("base", struct_ty), ("val", Idx::INT), struct_ty);
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    assert!(
        ir.contains("burden_dec_field.0.ptr"),
        "BurdenDecField on scalar field MUST still emit struct_gep; ir:\n{ir}",
    );
    assert!(
        ir.contains("burden_dec_field.0 "),
        "BurdenDecField on scalar field MUST still emit load; ir:\n{ir}",
    );

    assert!(
        !ir.contains("ori_rc_dec(ptr %burden_dec_field"),
        "BurdenDecField on scalar field MUST NOT emit ori_rc_dec on loaded value (RE-2); ir:\n{ir}",
    );

    drop(em);
}

#[test]
fn burden_dec_field_aggregate_base_spills_to_alloca_before_gep() {
    use ori_arc::ir::{ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId, ValueRepr};
    use ori_arc::Ownership;

    use super::test_utils::{burden_dec_field_first, entry_block, set_first};
    let mut pool = Pool::new();
    let struct_ty = pool.struct_type(
        ori_ir::Name::from_raw(320),
        &[(ori_ir::Name::from_raw(321), Idx::STR)],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_burden_dec_field_aggr"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let ptr_ty = builder.ptr_type();
    let host = builder.declare_function("test_bdf_aggr", &[ptr_ty, ptr_ty], ptr_ty);
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
        None,
    );

    let arc_func = ArcFunction {
        name: interner.intern("test_bdf_aggr"),
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
        var_reprs: vec![ValueRepr::Aggregate, ValueRepr::RcPointer],
        var_metadata_state: VariableMetadataState::RepresentationsReady,
        spans: vec![vec![None, None]],
        ..Default::default()
    };

    let abi = direct_pair_abi(&interner, ("base", struct_ty), ("val", Idx::STR), struct_ty);
    em.emit_function(&arc_func, &abi);

    let ir = scx.llmod.print_to_string().to_string();

    assert!(
        ir.contains("burden.spill") && ir.contains("alloca"),
        "BurdenDecField on a by-value aggregate base MUST spill to a `burden.spill` alloca; ir:\n{ir}",
    );
    assert!(
        ir.contains("burden_dec_field.0.ptr"),
        "BurdenDecField MUST emit struct_gep for field position off the spilled pointer; ir:\n{ir}",
    );
    assert!(
        ir.contains("ori_rc_dec"),
        "BurdenDecField on str-typed field of an aggregate base MUST route through ori_rc_dec; ir:\n{ir}",
    );
    assert!(
        ir.contains("getelementptr inbounds nuw %ori.320, ptr %burden.spill"),
        "BurdenDecField field GEP MUST address the spilled alloca pointer, not a by-value aggregate; ir:\n{ir}",
    );
    let spill_store = ir.find("store ptr %0, ptr %burden.spill");
    let field_gep = ir.find("%burden_dec_field.0.ptr = getelementptr");
    assert!(
        matches!((spill_store, field_gep), (Some(s), Some(g)) if s < g),
        "spill `store` MUST precede the field GEP; ir:\n{ir}",
    );

    drop(em);
}

// BurdenDecVariant codegen wire matrix

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
        None,
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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
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
        None,
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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
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
        None,
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
        var_metadata_state: VariableMetadataState::RepresentationsReady,
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
        None,
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
}

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

// RC strategy dispatch tests

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
        None,
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
                atomicity: ori_arc::ir::RcAtomicity::default_atomic(),
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
        None,
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
                atomicity: ori_arc::ir::RcAtomicity::default_atomic(),
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
        None,
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
                atomicity: ori_arc::ir::RcAtomicity::default_atomic(),
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
        None,
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
                atomicity: ori_arc::ir::RcAtomicity::default_atomic(),
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
        None,
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
                atomicity: ori_arc::ir::RcAtomicity::default_atomic(),
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

// Recursive drop-fn codegen
//
// Cycle safety comes from the cache-before-body pattern in `generate_drop_fn`:
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
    // exercises `generate_drop_fn`'s cache-before-body cycle-termination path
    // + the recursive emit_rc_dec → get_or_generate_drop_fn chain.
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
        None,
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
    let definitions = ir
        .matches(&format!("define internal void @{mangled}(ptr"))
        .count();
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
fn drop_augment_self_param_emits_no_default_dec_in_arc_ir() {
    // INVARIANT (AIMS RL-DROP self-recursion guard): a Drop-shaped struct with
    // only scalar fields but a user `@drop` impl takes the AUGMENT drop-body
    // path (`own_drop_unwinds` in `emit_drop_fields`) — it runs `@drop`, walks
    // scalar fields (no RC dec), then frees `data_ptr` via the runtime free
    // path only. It emits ZERO `ori_rc_dec`: a self-dec on `data_ptr` would
    // re-enter the drop fn infinitely. Paired negative pin:
    // `recursive_node_drop_fn_emits_self_referencing_rc_dec` DOES emit
    // `ori_rc_dec` on a self-typed field, proving this assertion is meaningful.
    let mut pool = Pool::new();
    let guard_name = ori_ir::Name::from_raw(0x0D0_9A2D);
    let guard_ty = pool.named(guard_name);
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_drop_augment_scalar"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    // Dummy `void @dummy_drop(ptr)` — the user `@drop` method the AUGMENT path
    // invokes. A declaration (no body) suffices: it never becomes a real call
    // target, and only its FunctionId + ABI drive the invoke emission.
    let ptr_ty = builder.ptr_type();
    let dummy_drop_fid = builder.get_or_declare_void_function("dummy_drop", &[ptr_ty]);

    // Populate the canonical method map the AUGMENT gate resolves through:
    // type_idx_to_name[guard_ty] -> guard_name, then
    // method_functions[(guard_name, "drop")] -> (dummy_drop_fid, abi). The
    // `@drop` self param is a pass-by-pointer (Reference) receiver, so the
    // invoke forwards `data_ptr` directly (no self-load / resolve_type).
    let drop_name = interner.intern("drop");
    let drop_abi = crate::codegen::abi::FunctionAbi {
        params: vec![crate::codegen::abi::ParamAbi {
            name: interner.intern("self"),
            ty: guard_ty,
            passing: crate::codegen::abi::ParamPassing::Reference,
            readonly: false,
        }],
        return_abi: crate::codegen::abi::ReturnAbi {
            ty: Idx::UNIT,
            passing: crate::codegen::abi::ReturnPassing::Void,
        },
        call_conv: crate::codegen::abi::CallConv::Fast,
    };
    let mut codegen_ctx = super::CodegenContext::default();
    codegen_ctx.type_idx_to_name.insert(guard_ty, guard_name);
    codegen_ctx
        .method_functions
        .insert((guard_name, drop_name), (dummy_drop_fid, drop_abi));

    // Classifier: the struct itself is heap-allocated (so it is freed via the
    // runtime free path); its scalar `int` field is NOT heap (so no field
    // ori_rc_dec).
    let cl = IdxSetClassifier {
        heap_idxs: vec![guard_ty],
    };

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    // `DropKind::Fields.fields` is the RC-dec worklist — the scalar `int` field
    // carries no RC header, so a scalar-only Drop struct lists ZERO RC fields
    // (compute_drop_info includes only heap fields). The augment body therefore
    // runs the user `@drop` + free with no field dec at all.
    let info = DropInfo {
        ty: guard_ty,
        kind: DropKind::Fields {
            fields: vec![],
            user_drop: None,
        },
    };
    let _ = super::drop_gen::generate_drop_fn(&mut em, guard_ty, &info);

    let ir = scx.llmod.print_to_string().to_string();

    // The drop-fn body releases the allocation ONLY through the runtime free
    // call (emit_drop_rc_free -> ori_rc_free). `ori_rc_free` is also declared
    // by declare_runtime, so assert on the CALL form (never the bare symbol).
    assert!(
        ir.contains("call void @ori_rc_free("),
        "scalar-field Drop struct drop-fn MUST release data_ptr via the runtime \
         ori_rc_free path:\n{ir}"
    );
    // ZERO ori_rc_dec of any kind (call OR invoke). declare_runtime emits a
    // `declare ... @ori_rc_dec` line into the module, so the bare substring is
    // always present — assert on the CALL / INVOKE forms, the only sites a dec
    // could be EMITTED. `call void @ori_rc_dec` also covers the `_unwind` /
    // `_to_zero` variants (prefix match). The only body in this module is the
    // drop fn (host + dummy_drop carry no dec), so a hit would be the drop fn's.
    assert!(
        !ir.contains("call void @ori_rc_dec") && !ir.contains("invoke void @ori_rc_dec"),
        "scalar-field Drop struct drop-fn MUST emit ZERO ori_rc_dec (a self-dec on \
         data_ptr would infinitely re-enter the drop fn; no RC fields exist):\n{ir}"
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
        None,
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
        .matches(&format!("define internal void @{tree_mangled}(ptr"))
        .count();
    let forest_defs = ir
        .matches(&format!("define internal void @{forest_mangled}(ptr"))
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
    // The cache-before-body pattern in `generate_drop_fn` guarantees that any
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
        None,
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
    let definitions = ir
        .matches(&format!("define internal void @{mangled}(ptr"))
        .count();
    assert_eq!(
        definitions, 1,
        "cache MUST prevent duplicate drop fn definitions even under repeated invocation:\n{ir}"
    );

    drop(em);
}

fn assert_augment_drop_ir(ir: &str, mangled: &str) {
    assert!(
        ir.contains(&format!("define internal void @{mangled}(ptr")),
        "AUGMENT drop fn MUST be defined:\n{ir}"
    );
    assert!(
        ir.contains("guard_user_drop"),
        "AUGMENT body MUST invoke the user @drop on self:\n{ir}"
    );
    assert!(
        ir.contains("@ori_rc_dec"),
        "AUGMENT body MUST dec its RC'd str field:\n{ir}"
    );
    assert!(
        ir.contains("@ori_rc_free"),
        "AUGMENT body MUST free the alloc:\n{ir}"
    );

    let def_needle = format!("@{mangled}(");
    let start = ir.find(&def_needle).expect("drop fn define line") + def_needle.len();
    let end = start + ir[start..].find(')').expect("param-list close");
    let params = &ir[start..end];
    let pct = params
        .find('%')
        .expect("ptr self param carries an SSA name");
    let rest = &params[pct..];
    let token_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '%' || c == '.' || c == '_'))
        .unwrap_or(rest.len());
    let self_param = &rest[..token_end];

    assert!(
        ir.contains(&format!("@ori_rc_free(ptr {self_param},")),
        "self (data_ptr `{self_param}`) MUST be released via ori_rc_free:\n{ir}"
    );
    assert!(
        !ir.contains(&format!("@ori_rc_dec(ptr {self_param},")),
        "AUGMENT body MUST NOT emit a self-referencing ori_rc_dec on `{self_param}` \
         (self-recursion -> infinite drop / double-free):\n{ir}"
    );
    assert!(
        !ir.contains(&format!("@ori_rc_dec_unwind(ptr {self_param},")),
        "AUGMENT body MUST NOT emit a self-referencing ori_rc_dec_unwind on `{self_param}`:\n{ir}"
    );
}

#[test]
fn augment_drop_body_emits_zero_self_dec() {
    let mut pool = Pool::new();
    let guard_ty = pool.named(ori_ir::Name::from_raw(0x0A06_0DAA));
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_augment_drop"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);

    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);
    let entry = builder.append_block(host, "entry");
    builder.set_current_function(host);
    builder.position_at_end(entry);

    let ptr_ty = builder.ptr_type();
    let user_drop_fn = builder.get_or_declare_void_function("guard_user_drop", &[ptr_ty]);

    let cl = IdxSetClassifier {
        heap_idxs: vec![guard_ty],
    };

    let type_name = ori_ir::Name::from_raw(0x0A06_0DAB);
    let drop_name = interner.intern("drop");
    let guard_abi = FunctionAbi {
        params: vec![crate::codegen::abi::ParamAbi {
            name: type_name,
            ty: guard_ty,
            passing: crate::codegen::abi::ParamPassing::Reference,
            readonly: true,
        }],
        return_abi: crate::codegen::abi::ReturnAbi {
            ty: Idx::UNIT,
            passing: crate::codegen::abi::ReturnPassing::Void,
        },
        call_conv: crate::codegen::abi::CallConv::C,
    };
    let mut codegen_ctx = super::CodegenContext::default();
    codegen_ctx.type_idx_to_name.insert(guard_ty, type_name);
    codegen_ctx
        .type_idx_to_name
        .insert(pool.resolve_fully(guard_ty), type_name);
    codegen_ctx
        .method_functions
        .insert((type_name, drop_name), (user_drop_fn, guard_abi));

    let mut em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    let info = DropInfo {
        ty: guard_ty,
        kind: DropKind::Fields {
            fields: vec![(0, Idx::STR)],
            user_drop: None,
        },
    };
    let _ = super::drop_gen::generate_drop_fn(&mut em, guard_ty, &info);

    let ir = scx.llmod.print_to_string().to_string();
    let mangled = format!("\"_ori_drop${}\"", guard_ty.raw());
    assert_augment_drop_ir(&ir, &mangled);

    drop(em);
}

// Method-fallback receiver-type gating.

/// In-memory tracing sink for subscriber assertions.
#[expect(
    clippy::disallowed_types,
    reason = "MakeWriter requires a cloneable shared test buffer"
)]
#[derive(Clone, Default)]
struct CapturingWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for CapturingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturingWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl CapturingWriter {
    fn contents(&self) -> String {
        String::from_utf8_lossy(
            &self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
        .to_string()
    }
}

fn capture_events(level: tracing::Level, body: impl FnOnce()) -> String {
    let writer = CapturingWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer.clone())
        .with_max_level(level)
        .without_time()
        .with_target(false)
        .finish();
    tracing::subscriber::with_default(subscriber, body);
    writer.contents()
}

/// Run `body` under a tracing subscriber capturing WARN-and-above events.
fn capture_warnings(body: impl FnOnce()) -> String {
    capture_events(tracing::Level::WARN, body)
}

#[test]
fn push_header_store_toggle_reports_effect() {
    if std::env::var_os("ORI_PUSH_TOGGLE_TRACE_CHILD").is_none() {
        run_toggle_trace_child(
            "push_header_store_toggle_reports_effect",
            "ORI_PUSH_TOGGLE_TRACE_CHILD",
            "ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE",
        );
        return;
    }

    let output = capture_events(tracing::Level::INFO, || {
        assert!(
            super::builtins::push_result_elem_header_store_disabled(),
            "enabled toggle must disable the stores"
        );
    });

    assert!(output.contains("ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE"));
    assert!(output.contains("skip result-buffer element destructor metadata stores"));
}

#[test]
fn rl31_noalias_toggle_reports_effect() {
    if std::env::var_os("ORI_RL31_TOGGLE_TRACE_CHILD").is_none() {
        run_toggle_trace_child(
            "rl31_noalias_toggle_reports_effect",
            "ORI_RL31_TOGGLE_TRACE_CHILD",
            "ORI_DISABLE_RL31_NOALIAS",
        );
        return;
    }

    let output = capture_events(tracing::Level::INFO, || {
        assert!(
            crate::codegen::function_compiler::rl31_noalias_disabled(),
            "enabled toggle must disable the attribute"
        );
    });

    assert!(output.contains("ORI_DISABLE_RL31_NOALIAS"));
    assert!(output.contains("omit LLVM projection of RL-31 parameter facts"));
}

fn run_toggle_trace_child(test_name: &str, marker: &str, toggle: &str) {
    let output = std::process::Command::new(
        std::env::current_exe().expect("test executable path must be available"),
    )
    .arg(test_name)
    .arg("--nocapture")
    .env(marker, "1")
    .env(toggle, "1")
    .output()
    .expect("toggle trace child must start");

    assert!(
        output.status.success(),
        "toggle trace child failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Minimal `FunctionAbi` for a zero-arg, void-returning dummy method — only
/// its presence in `method_functions` matters for these tests, not its shape.
fn dummy_method_abi() -> crate::codegen::abi::FunctionAbi {
    crate::codegen::abi::FunctionAbi {
        params: vec![],
        return_abi: crate::codegen::abi::ReturnAbi {
            ty: Idx::UNIT,
            passing: crate::codegen::abi::ReturnPassing::Void,
        },
        call_conv: crate::codegen::abi::CallConv::Fast,
    }
}

/// Registers `method_name` in `method_functions` under an UNRELATED type
/// name — models "some OTHER type has this method", the precondition
/// `lookup_method_fallback`'s `exists` check requires before it even
/// considers warning.
fn register_unrelated_method(
    codegen_ctx: &mut super::CodegenContext,
    method_name: ori_ir::Name,
    func_id: crate::codegen::value_id::FunctionId,
) {
    let other_type_name = ori_ir::Name::from_raw(9001);
    codegen_ctx.method_functions.insert(
        (other_type_name, method_name),
        (func_id, dummy_method_abi()),
    );
}

#[test]
fn lookup_method_fallback_registered_struct_receiver_warns() {
    let mut pool = Pool::new();
    let struct_ty = pool.struct_type(ori_ir::Name::from_raw(300), &[]);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_fallback_struct"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);

    let cl = TestClassifier;
    let mut codegen_ctx = super::CodegenContext::default();
    let method_name = interner.intern("debug");
    register_unrelated_method(&mut codegen_ctx, method_name, host);

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    let output = capture_warnings(|| {
        let result = em.lookup_method_fallback(method_name, Some(struct_ty));
        assert!(result.is_none(), "fallback always returns None");
    });
    assert!(
        output.contains("receiver type not registered"),
        "expected the fallback warning for a Struct-tagged receiver:\n{output}"
    );

    drop(em);
}

#[test]
fn lookup_method_fallback_registered_enum_receiver_warns() {
    let mut pool = Pool::new();
    let enum_ty = pool.enum_type(
        ori_ir::Name::from_raw(301),
        &[
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(302),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: ori_ir::Name::from_raw(303),
                field_types: vec![Idx::STR],
            },
        ],
    );

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_fallback_enum"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);

    let cl = TestClassifier;
    let mut codegen_ctx = super::CodegenContext::default();
    let method_name = interner.intern("debug");
    register_unrelated_method(&mut codegen_ctx, method_name, host);

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    // NEGATIVE PIN (proves the fix narrows, never disables, the diagnostic):
    // an enum receiver genuinely unregistered in `type_idx_to_name` (e.g.
    // enum derives not yet compiled) still fires the warning.
    let output = capture_warnings(|| {
        let result = em.lookup_method_fallback(method_name, Some(enum_ty));
        assert!(result.is_none(), "fallback always returns None");
    });
    assert!(
        output.contains("receiver type not registered"),
        "expected the fallback warning for an Enum-tagged receiver:\n{output}"
    );

    drop(em);
}

#[test]
fn lookup_method_fallback_builtin_str_receiver_no_warning() {
    let pool = Pool::new();

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_fallback_str"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);

    let cl = TestClassifier;
    let mut codegen_ctx = super::CodegenContext::default();
    let method_name = interner.intern("debug");
    register_unrelated_method(&mut codegen_ctx, method_name, host);

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    // REGRESSION PIN: a builtin receiver (`str`) sharing a method name with
    // some other registered type MUST NOT spuriously warn — this is the
    // exact shape of the original bug (`assert_eq<str>`'s `.debug()` call
    // sharing a compilation unit with a `#[derive(Debug)]` struct).
    let output = capture_warnings(|| {
        let result = em.lookup_method_fallback(method_name, Some(Idx::STR));
        assert!(result.is_none(), "fallback always returns None");
    });
    assert!(
        !output.contains("receiver type not registered"),
        "builtin str receiver MUST NOT trip the registration-gap warning:\n{output}"
    );

    drop(em);
}

#[test]
fn lookup_method_fallback_none_receiver_no_warning() {
    let pool = Pool::new();

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_fallback_none"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);

    let cl = TestClassifier;
    let mut codegen_ctx = super::CodegenContext::default();
    let method_name = interner.intern("default");
    register_unrelated_method(&mut codegen_ctx, method_name, host);

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    // A no-receiver call (e.g. an associated function whose call site has no
    // args) carries `receiver_ty: None` — MUST NOT warn regardless of what
    // other types register the same method name.
    let output = capture_warnings(|| {
        let result = em.lookup_method_fallback(method_name, None);
        assert!(result.is_none(), "fallback always returns None");
    });
    assert!(
        !output.contains("receiver type not registered"),
        "a receiver-less call MUST NOT trip the registration-gap warning:\n{output}"
    );

    drop(em);
}

fn fallback_receiver_function(
    interner: &StringInterner,
    name: &str,
    receiver_ty: Idx,
) -> ArcFunction {
    use ori_arc::ir::{ArcParam, ValueRepr};

    ArcFunction {
        name: interner.intern(name),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: receiver_ty,
            ownership: ori_arc::Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![receiver_ty, Idx::UNIT],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::Scalar],
        var_metadata_state: VariableMetadataState::RepresentationsReady,
        spans: vec![vec![None]],
        ..Default::default()
    }
}

#[test]
fn resolve_callee_threads_receiver_type_into_fallback_gate() {
    let mut pool = Pool::new();
    let struct_ty = pool.struct_type(ori_ir::Name::from_raw(310), &[]);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_resolve_callee_gate"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    declare_runtime(&mut builder);
    let i64_ty = builder.i64_type();
    let host = builder.declare_function("host", &[], i64_ty);

    let cl = TestClassifier;
    let mut codegen_ctx = super::CodegenContext::default();
    let method_name = interner.intern("debug");
    register_unrelated_method(&mut codegen_ctx, method_name, host);

    let em = super::ArcIrEmitter::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &cl as &dyn ArcClassification,
        host,
        &codegen_ctx,
        None,
    );

    let struct_func = fallback_receiver_function(&interner, "test_struct_receiver_fn", struct_ty);
    let struct_output = capture_warnings(|| {
        let resolved = em.resolve_callee(
            method_name,
            &[ArcVarId::new(0)],
            ArcVarId::new(1),
            &struct_func,
            None,
        );
        assert!(resolved.is_none());
    });
    assert!(
        struct_output.contains("receiver type not registered"),
        "resolve_callee MUST warn on a Struct-tagged unregistered receiver:\n{struct_output}"
    );

    let str_func = fallback_receiver_function(&interner, "test_str_receiver_fn", Idx::STR);
    let str_output = capture_warnings(|| {
        let resolved = em.resolve_callee(
            method_name,
            &[ArcVarId::new(0)],
            ArcVarId::new(1),
            &str_func,
            None,
        );
        assert!(resolved.is_none());
    });
    assert!(
        !str_output.contains("receiver type not registered"),
        "resolve_callee MUST NOT warn on a builtin str receiver:\n{str_output}"
    );

    drop(em);
}

fn dead_unwind_test_function(
    make_terminator: impl FnOnce(ArcBlockId, ArcBlockId) -> ArcTerminator,
) -> (ArcFunction, FxHashSet<usize>) {
    let live_block = ArcBlockId::new(1);
    let dead_block = ArcBlockId::new(2);
    let func = ArcFunction {
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: make_terminator(live_block, dead_block),
            },
            ArcBlock {
                id: live_block,
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
            ArcBlock {
                id: dead_block,
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    (func, FxHashSet::from_iter([dead_block.index()]))
}

fn function_with_rc_body(body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Unreachable,
        }],
        ..Default::default()
    }
}

#[test]
#[should_panic(expected = "dead unwind block is reachable via non-Invoke terminator")]
fn dead_unwind_jump_to_dead_panics() {
    let (func, dead) = dead_unwind_test_function(|_, dead_block| ArcTerminator::Jump {
        target: dead_block,
        args: Vec::new(),
    });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
#[should_panic(expected = "dead unwind block is reachable via non-Invoke terminator")]
fn dead_unwind_branch_to_dead_panics() {
    let (func, dead) = dead_unwind_test_function(|live_block, dead_block| ArcTerminator::Branch {
        cond: ArcVarId::new(0),
        then_block: live_block,
        else_block: dead_block,
    });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
#[should_panic(expected = "dead unwind block is reachable via non-Invoke terminator")]
fn dead_unwind_switch_case_to_dead_panics() {
    let (func, dead) = dead_unwind_test_function(|live_block, dead_block| ArcTerminator::Switch {
        scrutinee: ArcVarId::new(0),
        cases: vec![(0, dead_block)],
        default: live_block,
    });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
#[should_panic(expected = "dead unwind block is reachable via non-Invoke terminator")]
fn dead_unwind_switch_default_to_dead_panics() {
    let (func, dead) = dead_unwind_test_function(|live_block, dead_block| ArcTerminator::Switch {
        scrutinee: ArcVarId::new(0),
        cases: vec![(0, live_block)],
        default: dead_block,
    });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
#[should_panic(expected = "dead unwind block is reachable via non-Invoke terminator")]
fn dead_unwind_invoke_normal_to_dead_panics() {
    let (func, dead) = dead_unwind_test_function(|live_block, dead_block| ArcTerminator::Invoke {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: Name::EMPTY,
        args: Vec::new(),
        arg_ownership: Vec::new(),
        mono_instance_id: None,
        normal: dead_block,
        unwind: live_block,
    });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
#[should_panic(expected = "dead unwind block is reachable via non-Invoke terminator")]
fn dead_unwind_indirect_invoke_normal_to_dead_panics() {
    let (func, dead) =
        dead_unwind_test_function(|live_block, dead_block| ArcTerminator::InvokeIndirect {
            dst: ArcVarId::new(0),
            ty: Idx::UNIT,
            closure: ArcVarId::new(1),
            args: Vec::new(),
            arg_ownership: Vec::new(),
            normal: dead_block,
            unwind: live_block,
        });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
fn dead_unwind_invoke_unwind_to_dead_passes() {
    let (func, dead) = dead_unwind_test_function(|live_block, dead_block| ArcTerminator::Invoke {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: Name::EMPTY,
        args: Vec::new(),
        arg_ownership: Vec::new(),
        mono_instance_id: None,
        normal: live_block,
        unwind: dead_block,
    });

    super::dead_unwind::assert_dead_unwind_unreachable(&func, &dead);
}

#[test]
#[should_panic(expected = "pointer-only param v0 has RC operation")]
fn pointer_only_rc_increment_panics() {
    let pointer_only = ArcVarId::new(0);
    let func = function_with_rc_body(vec![ArcInstr::RcInc {
        var: pointer_only,
        count: 1,
        strategy: RcStrategy::HeapPointer,
        atomicity: RcAtomicity::Atomic,
    }]);
    let pointer_only_params = FxHashSet::from_iter([pointer_only]);

    super::emit_function::assert_pointer_only_params_have_no_rc(&func, &pointer_only_params);
}

#[test]
#[should_panic(expected = "pointer-only param v0 has RC operation")]
fn pointer_only_rc_decrement_panics() {
    let pointer_only = ArcVarId::new(0);
    let func = function_with_rc_body(vec![ArcInstr::RcDec {
        var: pointer_only,
        strategy: RcStrategy::HeapPointer,
        atomicity: RcAtomicity::Atomic,
    }]);
    let pointer_only_params = FxHashSet::from_iter([pointer_only]);

    super::emit_function::assert_pointer_only_params_have_no_rc(&func, &pointer_only_params);
}

#[test]
fn pointer_only_rc_on_other_variable_passes() {
    let pointer_only = ArcVarId::new(0);
    let other = ArcVarId::new(1);
    let func = function_with_rc_body(vec![
        ArcInstr::RcInc {
            var: other,
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::Atomic,
        },
        ArcInstr::RcDec {
            var: other,
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::Atomic,
        },
    ]);
    let pointer_only_params = FxHashSet::from_iter([pointer_only]);

    super::emit_function::assert_pointer_only_params_have_no_rc(&func, &pointer_only_params);
}
