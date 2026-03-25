#![allow(
    clippy::too_many_lines,
    reason = "test setup for LLVM IR requires many sequential steps"
)]

use super::*;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::context::SimpleCx;
use inkwell::context::Context;
use ori_arc::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use ori_arc::{AnnotatedSig, ArcClassifier, ArcFunction};
use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;
use std::mem::ManuallyDrop;

/// Create a basic FunctionSig for testing.
fn make_sig(
    name: Name,
    param_names: Vec<Name>,
    param_types: Vec<Idx>,
    return_type: Idx,
    is_main: bool,
) -> FunctionSig {
    let required_params = param_types.len();
    let param_hashes = vec![0; param_types.len()];
    FunctionSig {
        name,
        type_params: vec![],
        const_params: vec![],
        param_names,
        param_types,
        return_type,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main,
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params,
        param_defaults: vec![],
        param_hashes,
        return_hash: 0,
    }
}

// Note: SimpleCx has a Drop impl (LLVM module), which interacts with the
// drop checker when other locals borrow `&scx`. We use ManuallyDrop to
// suppress the drop checker's conservative analysis. The LLVM context
// outlives all these locals (it owns the actual memory), so this is safe.

#[test]
fn declare_simple_function() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_declare"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("add");
    let a_name = interner.intern("a");
    let b_name = interner.intern("b");

    let sig = make_sig(
        func_name,
        vec![a_name, b_name],
        vec![Idx::INT, Idx::INT],
        Idx::INT,
        false,
    );

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    let (_func_id, abi) = fc.get_function(func_name).unwrap();
    assert_eq!(abi.params.len(), 2);
    assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
    assert_eq!(abi.call_conv, CallConv::Fast);

    // Function is declared with mangled name _ori_add
    assert!(scx.llmod.get_function("_ori_add").is_some());
}

#[test]
fn declare_void_function() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_void"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("do_thing");
    let sig = make_sig(func_name, vec![], vec![], Idx::UNIT, false);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    let (_, abi) = fc.get_function(func_name).unwrap();
    assert_eq!(abi.return_abi.passing, ReturnPassing::Void);
}

#[test]
fn declare_sret_function() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_sret"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("get_list");
    let sig = make_sig(func_name, vec![], vec![], list_int, false);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    let (_, abi) = fc.get_function(func_name).unwrap();
    assert!(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));

    // Must drop borrowers of scx before accessing scx directly
    drop(fc);
    drop(builder);
    drop(resolver);

    // Function is declared with mangled name _ori_get_list
    let llvm_fn = scx.llmod.get_function("_ori_get_list").unwrap();
    assert!(llvm_fn.get_type().get_return_type().is_none());
    assert_eq!(llvm_fn.count_params(), 1);
}

#[test]
fn declare_main_uses_c_calling_convention() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_main_cc"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("main");
    let sig = make_sig(func_name, vec![], vec![], Idx::UNIT, true);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    let (_, abi) = fc.get_function(func_name).unwrap();
    assert_eq!(abi.call_conv, CallConv::C);
}

#[test]
fn generic_functions_are_skipped() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_generic_skip"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("identity");
    let t_name = interner.intern("T");
    let sig = FunctionSig {
        name: func_name,
        type_params: vec![t_name],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::UNIT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 0,
        param_defaults: vec![],
        param_hashes: vec![],
        return_hash: 0,
    };

    let func = Function {
        name: func_name,
        generics: ori_ir::GenericParamRange::EMPTY,
        params: ori_ir::ParamRange::EMPTY,
        return_ty: None,
        capabilities: vec![],
        where_clauses: vec![],
        guard: None,
        pre_contracts: vec![],
        post_contracts: vec![],
        body: ori_ir::ExprId::INVALID,
        span: ori_ir::Span::new(0, 0),
        visibility: ori_ir::Visibility::Private,
        is_fbip: false,
        target_attr: None,
        cfg_attr: None,
    };

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_all(&[func], &[sig]);

    assert!(fc.get_function(func_name).is_none());

    // Must drop borrowers of scx before accessing scx directly
    drop(fc);
    drop(builder);
    drop(resolver);
    // Generic functions are not declared at all (neither mangled nor unmangled)
    assert!(scx.llmod.get_function("identity").is_none());
    assert!(scx.llmod.get_function("_ori_identity").is_none());
}

#[test]
fn function_map_returns_all_declared() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_map"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let add_name = interner.intern("add");
    let sub_name = interner.intern("sub");
    let a_name = interner.intern("a");
    let b_name = interner.intern("b");

    let sig_add = make_sig(
        add_name,
        vec![a_name, b_name],
        vec![Idx::INT, Idx::INT],
        Idx::INT,
        false,
    );
    let sig_sub = make_sig(
        sub_name,
        vec![a_name, b_name],
        vec![Idx::INT, Idx::INT],
        Idx::INT,
        false,
    );

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(add_name, &sig_add, Span::DUMMY);
    fc.declare_function(sub_name, &sig_sub, Span::DUMMY);

    assert_eq!(fc.function_map().len(), 2);
    assert!(fc.function_map().contains_key(&add_name));
    assert!(fc.function_map().contains_key(&sub_name));
}

#[test]
fn compile_impls_populates_method_functions_map() {
    use ori_ir::{GenericParamRange, ImplDef, ImplMethod, ParsedType, ParsedTypeRange, Span};

    let interner = StringInterner::new();
    let point_name = interner.intern("Point");
    let line_name = interner.intern("Line");

    let mut pool = Pool::new();
    // Create named type Idx values for receiver types
    let point_idx = pool.named(point_name);
    let line_idx = pool.named(line_name);

    let ctx = Context::create();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_method_dispatch"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let distance_name = interner.intern("distance");
    let self_name = interner.intern("self");

    // Create two impl blocks with same-name method "distance"
    let impl_point = ImplDef {
        generics: GenericParamRange::EMPTY,
        trait_path: None,
        trait_type_args: ParsedTypeRange::EMPTY,
        self_path: vec![point_name],
        self_ty: ParsedType::Named {
            name: point_name,
            type_args: ParsedTypeRange::EMPTY,
        },
        where_clauses: vec![],
        methods: vec![ImplMethod {
            name: distance_name,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::FLOAT),
            body: ori_ir::ExprId::INVALID,
            span: Span::new(0, 0),
        }],
        assoc_types: vec![],
        span: Span::new(0, 0),
        target_attr: None,
        cfg_attr: None,
    };

    let impl_line = ImplDef {
        generics: GenericParamRange::EMPTY,
        trait_path: None,
        trait_type_args: ParsedTypeRange::EMPTY,
        self_path: vec![line_name],
        self_ty: ParsedType::Named {
            name: line_name,
            type_args: ParsedTypeRange::EMPTY,
        },
        where_clauses: vec![],
        methods: vec![ImplMethod {
            name: distance_name,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::FLOAT),
            body: ori_ir::ExprId::INVALID,
            span: Span::new(0, 0),
        }],
        assoc_types: vec![],
        span: Span::new(0, 0),
        target_attr: None,
        cfg_attr: None,
    };

    // Signatures: distance(self: Point) -> float, distance(self: Line) -> float
    let sig_point = make_sig(
        distance_name,
        vec![self_name],
        vec![point_idx],
        Idx::FLOAT,
        false,
    );
    let sig_line = make_sig(
        distance_name,
        vec![self_name],
        vec![line_idx],
        Idx::FLOAT,
        false,
    );

    let impl_sigs = vec![
        (distance_name, sig_point.clone()),
        (distance_name, sig_line.clone()),
    ];

    // Create a minimal CanonResult for testing (methods have INVALID bodies,
    // which is fine since we're only testing declaration/dispatch, not lowering)
    let canon = ori_ir::canon::CanonResult {
        arena: Default::default(),
        constants: Default::default(),
        decision_trees: ori_ir::canon::DecisionTreePool::new(),
        root: CanId::INVALID,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );

    // Compile Point impl first, then Line impl
    // Note: compile_impls processes all impls; same method name → last one
    // overwrites in bare functions map, but BOTH should be in method_functions
    fc.compile_impls(&[impl_point, impl_line], &impl_sigs, &canon, &[]);

    // The bare functions map has only the LAST one (Line.distance overwrites Point.distance)
    assert!(fc.function_map().contains_key(&distance_name));

    // The type-qualified method map has BOTH
    assert!(
        fc.method_function_map()
            .contains_key(&(point_name, distance_name)),
        "method_functions should contain (Point, distance)"
    );
    assert!(
        fc.method_function_map()
            .contains_key(&(line_name, distance_name)),
        "method_functions should contain (Line, distance)"
    );

    // The type Idx → Name map should have both types
    assert_eq!(
        fc.type_idx_to_name_map().get(&point_idx),
        Some(&point_name),
        "type_idx_to_name should map Point Idx → Point Name"
    );
    assert_eq!(
        fc.type_idx_to_name_map().get(&line_idx),
        Some(&line_name),
        "type_idx_to_name should map Line Idx → Line Name"
    );

    // The two entries in method_functions should have DIFFERENT FunctionIds
    // (because they are different LLVM functions with different mangled names)
    let (point_func_id, _) = fc
        .method_function_map()
        .get(&(point_name, distance_name))
        .unwrap();
    let (line_func_id, _) = fc
        .method_function_map()
        .get(&(line_name, distance_name))
        .unwrap();
    assert_ne!(
        point_func_id, line_func_id,
        "Point.distance and Line.distance should have different FunctionIds"
    );

    // Must drop borrowers before accessing scx
    drop(fc);
    drop(builder);
    drop(resolver);

    // Verify mangled LLVM symbols exist
    assert!(
        scx.llmod.get_function("_ori_Point$distance").is_some(),
        "LLVM module should have _ori_Point$distance"
    );
    assert!(
        scx.llmod.get_function("_ori_Line$distance").is_some(),
        "LLVM module should have _ori_Line$distance"
    );
}

#[test]
fn module_path_appears_in_mangled_name() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_module_mangle"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("add");
    let a_name = interner.intern("a");
    let sig = make_sig(func_name, vec![a_name], vec![Idx::INT], Idx::INT, false);

    // Use "math" as module path
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "math",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    // Must drop borrowers before accessing scx directly
    drop(fc);
    drop(builder);
    drop(resolver);

    // Mangled as _ori_math$add
    assert!(scx.llmod.get_function("_ori_math$add").is_some());
    // Unmangled name should NOT exist
    assert!(scx.llmod.get_function("add").is_none());
}

// Noundef attribute tests (§02.6)

#[test]
fn scalar_params_have_noundef() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_noundef"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("add");
    let a_name = interner.intern("a");
    let b_name = interner.intern("b");

    let sig = make_sig(
        func_name,
        vec![a_name, b_name],
        vec![Idx::INT, Idx::INT],
        Idx::INT,
        false,
    );

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    drop(fc);
    drop(builder);
    drop(resolver);

    let ir = scx.llmod.print_to_string().to_string();

    // Both int parameters and int return should have noundef
    assert!(
        ir.contains("noundef"),
        "scalar int parameters should have noundef attribute:\n{ir}"
    );
}

#[test]
fn scalar_return_has_noundef() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_noundef_ret"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("get_bool");
    let sig = make_sig(func_name, vec![], vec![], Idx::BOOL, false);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    drop(fc);
    drop(builder);
    drop(resolver);

    let ir = scx.llmod.print_to_string().to_string();

    // Bool return (i1) should have noundef
    assert!(
        ir.contains("noundef"),
        "scalar bool return should have noundef attribute:\n{ir}"
    );
}

#[test]
fn indirect_params_have_noundef() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_indirect_params"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("process_str");
    let s_name = interner.intern("s");

    // Str is 24 bytes ({len, cap, data}) → Indirect passing (ptr)
    // The pointer itself is always a defined, valid address — noundef applies.
    let sig = make_sig(func_name, vec![s_name], vec![Idx::STR], Idx::UNIT, false);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    drop(fc);
    drop(builder);
    drop(resolver);

    let ir = scx.llmod.print_to_string().to_string();
    let decl_line = ir
        .lines()
        .find(|l| l.contains("@_ori_process_str"))
        .unwrap();

    // Indirect pointer params get noundef (pointer value is always defined).
    assert!(
        decl_line.contains("noundef"),
        "Indirect (pointer) params should have noundef:\n{decl_line}"
    );
}

#[test]
fn direct_aggregate_params_have_noundef() {
    let mut pool = Pool::new();
    // (int, int) = 16 bytes → Direct passing → should get noundef
    let pair = pool.tuple(&[Idx::INT, Idx::INT]);

    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_direct_aggregate"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("process_pair");
    let p_name = interner.intern("p");

    let sig = make_sig(func_name, vec![p_name], vec![pair], Idx::INT, false);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    drop(fc);
    drop(builder);
    drop(resolver);

    let ir = scx.llmod.print_to_string().to_string();
    let decl_line = ir
        .lines()
        .find(|l| l.contains("@_ori_process_pair"))
        .unwrap();

    // Direct aggregate param (≤16 bytes) AND int return both get noundef.
    let noundef_count = decl_line.matches("noundef").count();
    assert_eq!(
        noundef_count, 2,
        "expected 2 noundef (tuple param + int return), got {noundef_count}:\n{decl_line}"
    );
}

#[test]
fn mixed_params_selective_noundef() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_mixed_params"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("mixed");
    let n_name = interner.intern("n");
    let s_name = interner.intern("s");
    let f_name = interner.intern("f");

    // Mix: int (Direct), str (Indirect, 24 bytes), float (Direct)
    let sig = make_sig(
        func_name,
        vec![n_name, s_name, f_name],
        vec![Idx::INT, Idx::STR, Idx::FLOAT],
        Idx::BOOL,
        false,
    );

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );
    fc.declare_function(func_name, &sig, Span::DUMMY);

    drop(fc);
    drop(builder);
    drop(resolver);

    // Check the declaration line for selective noundef
    let ir = scx.llmod.print_to_string().to_string();
    let decl_line = ir.lines().find(|l| l.contains("@_ori_mixed")).unwrap();

    // All params and Direct return get noundef:
    // - int (Direct), str (Indirect pointer), float (Direct), bool return (Direct)
    let noundef_count = decl_line.matches("noundef").count();
    assert_eq!(
        noundef_count, 4,
        "expected 4 noundef (int + str ptr + float params + bool return), got {noundef_count}:\n{decl_line}"
    );
}

// Nounwind analysis tests

/// Helper: create a minimal FunctionCompiler for nounwind testing.
fn make_nounwind_fc<'a, 'scx: 'ctx, 'ctx, 'tcx>(
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    store: &'a TypeInfoStore<'tcx>,
    resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    interner: &'a StringInterner,
    pool: &'tcx Pool,
    annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    classifier: &'a ArcClassifier<'tcx>,
) -> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    FunctionCompiler::new(
        builder,
        store,
        resolver,
        interner,
        pool,
        "",
        annotated_sigs,
        classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    )
}

/// Helper: build a single-block ArcFunction with the given body instructions.
fn make_arc_func(
    interner: &StringInterner,
    name: &str,
    body: Vec<ArcInstr>,
    terminator: ArcTerminator,
) -> ArcFunction {
    let func_name = interner.intern(name);
    ArcFunction {
        name: func_name,
        params: vec![],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body,
            terminator,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT; 8],
        var_reprs: vec![],
        spans: vec![],
        ..Default::default()
    }
}

#[test]
fn nounwind_empty_function() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_empty"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Empty function (just returns) → nounwind
    let func = make_arc_func(
        &interner,
        "empty",
        vec![],
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );
    assert!(fc.is_arc_function_nounwind(&func));
}

#[test]
fn nounwind_direct_safe_call() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_safe"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Direct call to a safe runtime function (not ori_panic*) → nounwind
    let func = make_arc_func(
        &interner,
        "safe_caller",
        vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: interner.intern("ori_str_len"),
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Borrowed],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    );
    assert!(fc.is_arc_function_nounwind(&func));
}

#[test]
fn nounwind_panic_call_is_not_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_panic"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Direct call to ori_panic → NOT nounwind
    let func = make_arc_func(
        &interner,
        "panicking",
        vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::UNIT,
            func: interner.intern("ori_panic"),
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    );
    assert!(!fc.is_arc_function_nounwind(&func));
}

#[test]
fn nounwind_indirect_call_is_not_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_indirect"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Indirect call through closure → NOT nounwind (the fix for Finding #2)
    let func = make_arc_func(
        &interner,
        "closure_caller",
        vec![ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            closure: ArcVarId::new(0),
            args: vec![ArcVarId::new(1)],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    );
    assert!(
        !fc.is_arc_function_nounwind(&func),
        "functions with indirect calls (closures) must NOT be nounwind"
    );
}

#[test]
fn nounwind_invoke_unknown_callee_is_not_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_invoke"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Register the callee as a declared user function so the intercepted
    // heuristic correctly identifies it as a user function (not a builtin).
    let unknown_name = interner.intern("unknown_fn");
    fc.codegen_ctx
        .functions
        .insert(unknown_name, (FunctionId::NONE, make_test_abi(&pool)));

    // Invoke to a callee NOT in nounwind set → NOT nounwind
    let func = make_arc_func(
        &interner,
        "invoke_caller",
        vec![],
        ArcTerminator::Invoke {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: unknown_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    );
    assert!(
        !fc.is_arc_function_nounwind(&func),
        "invoke to unknown callee must NOT be nounwind"
    );
}

#[test]
fn nounwind_mixed_safe_and_indirect_is_not_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_mixed"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Mix of safe direct call + indirect call → NOT nounwind (indirect poisons it)
    let func = make_arc_func(
        &interner,
        "mixed_caller",
        vec![
            ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: interner.intern("ori_str_len"),
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                closure: ArcVarId::new(2),
                args: vec![ArcVarId::new(1)],
            },
        ],
        ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    );
    assert!(
        !fc.is_arc_function_nounwind(&func),
        "any indirect call in the function makes it NOT nounwind"
    );
}

#[test]
fn nounwind_may_panic_runtime_call_is_not_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_may_panic_rt"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // ori_list_get can panic on OOB — function calling it is NOT nounwind
    let func = make_arc_func(
        &interner,
        "list_getter",
        vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: interner.intern("ori_list_get"),
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Borrowed],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    );
    assert!(
        !fc.is_arc_function_nounwind(&func),
        "ori_list_get may panic on OOB — caller must not be nounwind"
    );
}

#[test]
fn nounwind_unknown_user_function_is_not_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_unknown_user"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Call to user function not in nounwind_functions set → NOT nounwind.
    // Use empty args to avoid the builtin method interception path
    // (which would recognize a call with a builtin-typed first arg as
    // an intercepted builtin method).
    let func = make_arc_func(
        &interner,
        "caller_of_unknown",
        vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: interner.intern("some_user_function"),
            args: vec![],
            arg_ownership: vec![],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    );
    assert!(
        !fc.is_arc_function_nounwind(&func),
        "user function not in nounwind_functions set — caller must not be nounwind"
    );
}

// ── Two-pass nounwind (compute_nounwind_set) tests ─────────────────

#[test]
fn compute_nounwind_set_marks_trivial_nounwind() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_compute_nounwind_trivial"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // A trivially nounwind function (no calls, just returns)
    let func_name = interner.intern("identity_int");
    let arc_func = make_arc_func(
        &interner,
        "identity_int",
        vec![],
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    let prepared = vec![PreparedFunction {
        name: func_name,
        func_id: FunctionId::NONE,
        abi: make_test_abi(&pool),
        arc_func,
        lambdas: vec![],
    }];

    fc.compute_nounwind_set(&prepared);
    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&func_name),
        "trivially nounwind function should be in nounwind set"
    );
}

#[test]
fn compute_nounwind_set_caller_sees_callee() {
    // Simulates the monomorphization ordering fix: a caller that
    // Invoke-calls a nounwind callee should be marked nounwind
    // even though both are in the same prepared batch.
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_compute_nounwind_chain"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Callee: identity$m$int — trivially nounwind
    let callee_name = interner.intern("identity_m_int");
    let callee = make_arc_func(
        &interner,
        "identity_m_int",
        vec![],
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    // Caller: main — Invokes identity$m$int (which is nounwind)
    let caller_name = interner.intern("main");
    let caller = make_arc_func(
        &interner,
        "main",
        vec![],
        ArcTerminator::Invoke {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: callee_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    );

    // Both in the same prepared batch — callee defined AFTER caller
    // in the Vec to exercise the fixed-point iteration.
    let prepared = vec![
        PreparedFunction {
            name: caller_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: caller,
            lambdas: vec![],
        },
        PreparedFunction {
            name: callee_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: callee,
            lambdas: vec![],
        },
    ];

    fc.compute_nounwind_set(&prepared);

    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&callee_name),
        "callee should be in nounwind set"
    );
    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&caller_name),
        "caller of nounwind callee should also be in nounwind set (fixed-point)"
    );
}

#[test]
fn compute_nounwind_set_may_unwind_callee_blocks_caller() {
    // A caller that Invoke-calls a may-unwind callee (not in nounwind set)
    // should NOT be marked nounwind.
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_compute_nounwind_blocked"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Register both functions as declared so the intercepted heuristic
    // correctly identifies them as user functions (not builtins).
    let callee_name = interner.intern("might_panic");
    let caller_name = interner.intern("caller");
    fc.codegen_ctx
        .functions
        .insert(callee_name, (FunctionId::NONE, make_test_abi(&pool)));
    fc.codegen_ctx
        .functions
        .insert(caller_name, (FunctionId::NONE, make_test_abi(&pool)));

    // Callee: panicking function — NOT nounwind
    let callee = make_arc_func(
        &interner,
        "might_panic",
        vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::UNIT,
            func: interner.intern("ori_panic"),
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    );

    // Caller: invokes might_panic
    let caller = make_arc_func(
        &interner,
        "caller",
        vec![],
        ArcTerminator::Invoke {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: callee_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    );

    let prepared = vec![
        PreparedFunction {
            name: caller_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: caller,
            lambdas: vec![],
        },
        PreparedFunction {
            name: callee_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: callee,
            lambdas: vec![],
        },
    ];

    fc.compute_nounwind_set(&prepared);

    assert!(
        !fc.codegen_ctx.nounwind_functions.contains(&callee_name),
        "panicking callee must NOT be in nounwind set"
    );
    assert!(
        !fc.codegen_ctx.nounwind_functions.contains(&caller_name),
        "caller of may-unwind callee must NOT be in nounwind set"
    );
}

#[test]
fn compute_nounwind_set_three_level_chain() {
    // Tests fixed-point iteration with a 3-level chain: A → B → C
    // All are nounwind but require 3 passes to fully propagate.
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_compute_nounwind_3level"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // C: leaf function, trivially nounwind
    let c_name = interner.intern("func_c");
    let c_func = make_arc_func(
        &interner,
        "func_c",
        vec![],
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    // B: invokes C
    let b_name = interner.intern("func_b");
    let b_func = make_arc_func(
        &interner,
        "func_b",
        vec![],
        ArcTerminator::Invoke {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: c_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    );

    // A: invokes B
    let a_name = interner.intern("func_a");
    let a_func = make_arc_func(
        &interner,
        "func_a",
        vec![],
        ArcTerminator::Invoke {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: b_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    );

    // Put A first (worst case for ordering — needs most passes)
    let prepared = vec![
        PreparedFunction {
            name: a_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: a_func,
            lambdas: vec![],
        },
        PreparedFunction {
            name: b_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: b_func,
            lambdas: vec![],
        },
        PreparedFunction {
            name: c_name,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: c_func,
            lambdas: vec![],
        },
    ];

    fc.compute_nounwind_set(&prepared);

    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&c_name),
        "leaf function C should be nounwind"
    );
    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&b_name),
        "B (calls nounwind C) should be nounwind"
    );
    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&a_name),
        "A (calls nounwind B) should be nounwind after fixed-point"
    );
}

#[test]
fn compute_nounwind_set_propagates_to_generic_original_name() {
    // When ALL monomorphizations of a generic are nounwind,
    // the original generic name should also be added to the set.
    // This is critical because ARC IR `Invoke` terminators use the
    // original name (e.g., "identity"), not the mangled name.
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_mono_propagate"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // Monomorphized specialization: identity$m$int — trivially nounwind
    let mangled = interner.intern("identity$m$int");
    let mangled_func = make_arc_func(
        &interner,
        "identity$m$int",
        vec![],
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    // Register the mono dispatch mapping: identity → [(int_params, identity$m$int)]
    let original = interner.intern("identity");
    fc.codegen_ctx
        .mono_dispatch
        .entry(original)
        .or_default()
        .push((vec![Idx::INT], mangled));

    let prepared = vec![PreparedFunction {
        name: mangled,
        func_id: FunctionId::NONE,
        abi: make_test_abi(&pool),
        arc_func: mangled_func,
        lambdas: vec![],
    }];

    fc.compute_nounwind_set(&prepared);

    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&mangled),
        "mangled mono name should be nounwind"
    );
    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&original),
        "original generic name should also be nounwind (all monos are nounwind)"
    );
}

#[test]
fn compute_nounwind_set_does_not_propagate_if_any_mono_may_unwind() {
    // If even ONE monomorphization may unwind, the original generic name
    // must NOT be added to the nounwind set.
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_nounwind_mono_partial"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);
    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();

    let mut fc = make_nounwind_fc(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        &annotated_sigs,
        &classifier,
    );

    // First mono: identity$m$int — nounwind
    let mangled_int = interner.intern("identity$m$int");
    let int_func = make_arc_func(
        &interner,
        "identity$m$int",
        vec![],
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    // Second mono: identity$m$str — may unwind (calls panic)
    let mangled_str = interner.intern("identity$m$str");
    let str_func = make_arc_func(
        &interner,
        "identity$m$str",
        vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::UNIT,
            func: interner.intern("ori_panic"),
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    );

    // Register mono dispatch: identity → [int, str]
    let original = interner.intern("identity");
    fc.codegen_ctx
        .mono_dispatch
        .entry(original)
        .or_default()
        .push((vec![Idx::INT], mangled_int));
    fc.codegen_ctx
        .mono_dispatch
        .entry(original)
        .or_default()
        .push((vec![Idx::STR], mangled_str));

    let prepared = vec![
        PreparedFunction {
            name: mangled_int,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: int_func,
            lambdas: vec![],
        },
        PreparedFunction {
            name: mangled_str,
            func_id: FunctionId::NONE,
            abi: make_test_abi(&pool),
            arc_func: str_func,
            lambdas: vec![],
        },
    ];

    fc.compute_nounwind_set(&prepared);

    assert!(
        fc.codegen_ctx.nounwind_functions.contains(&mangled_int),
        "nounwind mono should be marked nounwind"
    );
    assert!(
        !fc.codegen_ctx.nounwind_functions.contains(&mangled_str),
        "may-unwind mono should NOT be nounwind"
    );
    assert!(
        !fc.codegen_ctx.nounwind_functions.contains(&original),
        "original name must NOT be nounwind when any mono may unwind"
    );
}

/// Helper: create a minimal FunctionAbi for test PreparedFunctions.
fn make_test_abi(pool: &Pool) -> FunctionAbi {
    use super::super::abi::{CallConv, FunctionAbi, ReturnAbi, ReturnPassing};
    let _ = pool; // unused but kept for consistency
    FunctionAbi {
        params: vec![],
        return_abi: ReturnAbi {
            ty: Idx::INT,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    }
}

#[test]
fn main_wrapper_has_noundef_return() {
    let pool = Pool::new();
    let ctx = Context::create();
    let interner = StringInterner::new();
    let store = TypeInfoStore::new(&pool);
    let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_main_wrapper_noundef"));
    let resolver = TypeLayoutResolver::new(&store, &scx, Some(&interner), None);
    let mut builder = IrBuilder::new(&scx);

    let main_name = interner.intern("main");

    // Declare @main () -> void (simplest signature)
    let sig = make_sig(main_name, vec![], vec![], Idx::UNIT, true);

    let classifier = ArcClassifier::new(&pool);
    let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        &annotated_sigs,
        &classifier,
        None,
        FxHashMap::default(),
        FxHashMap::default(),
        false,
    );

    // Declare the Ori _ori_main function so generate_main_wrapper can find it
    fc.declare_function(main_name, &sig, Span::DUMMY);

    // Generate the C main wrapper
    let generated = fc.generate_main_wrapper(main_name, &sig, None);
    assert!(generated, "main wrapper should be generated");

    drop(fc);
    drop(builder);
    drop(resolver);

    let ir = scx.llmod.print_to_string().to_string();
    // Find the C main definition (not _ori_main)
    let main_line = ir
        .lines()
        .find(|l| l.contains("define") && l.contains("@main(") && !l.contains("_ori_main"))
        .unwrap_or_else(|| panic!("no @main definition found in IR:\n{ir}"));

    // The i32 return should have noundef
    assert!(
        main_line.contains("noundef"),
        "C main wrapper return should have noundef attribute:\n{main_line}"
    );
}
