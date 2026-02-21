use super::*;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::context::SimpleCx;
use inkwell::context::Context;
use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_types::{Idx, Pool};
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
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params,
        param_defaults: vec![],
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
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

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("do_thing");
    let sig = make_sig(func_name, vec![], vec![], Idx::UNIT, false);

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("get_list");
    let sig = make_sig(func_name, vec![], vec![], list_int, false);

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("main");
    let sig = make_sig(func_name, vec![], vec![], Idx::UNIT, true);

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
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
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 0,
        param_defaults: vec![],
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
    };

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
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

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
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

    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "",
        None,
        None,
        None,
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
    let resolver = TypeLayoutResolver::new(&store, &scx);
    let mut builder = IrBuilder::new(&scx);

    let func_name = interner.intern("add");
    let a_name = interner.intern("a");
    let sig = make_sig(func_name, vec![a_name], vec![Idx::INT], Idx::INT, false);

    // Use "math" as module path
    let mut fc = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        &interner,
        &pool,
        "math",
        None,
        None,
        None,
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
