use rustc_hash::FxHashMap;

use ori_ir::StringInterner;
use ori_types::{FunctionSig, GenericArg, Idx, MonoInstance, Pool};

use super::{collect_mono_functions, mangle_mono_name};

fn make_interner() -> StringInterner {
    StringInterner::new()
}

fn make_generic_sig(interner: &StringInterner) -> FunctionSig {
    let name = interner.intern("identity");
    let t_name = interner.intern("T");
    let x_name = interner.intern("x");

    FunctionSig {
        name,
        type_params: vec![t_name],
        const_params: vec![],
        param_names: vec![x_name],
        param_types: vec![Idx::from_raw(100)], // generic T placeholder
        return_type: Idx::from_raw(100),
        capabilities: vec![],
        is_public: true,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![vec![]],
        where_clauses: vec![],
        generic_param_mapping: vec![Some(0)],
        scheme_var_ids: vec![0],
        required_params: 1,
        param_defaults: vec![],
    }
}

#[test]
fn mangle_single_type_param_int() {
    let interner = make_interner();
    let pool = Pool::new();
    let fn_name = interner.intern("identity");

    let mangled = mangle_mono_name(fn_name, &[GenericArg::Type(Idx::INT)], &interner, &pool);

    assert_eq!(interner.lookup(mangled), "identity$m$int");
}

#[test]
fn mangle_two_type_params() {
    let interner = make_interner();
    let pool = Pool::new();
    let fn_name = interner.intern("make_pair");

    let mangled = mangle_mono_name(
        fn_name,
        &[GenericArg::Type(Idx::INT), GenericArg::Type(Idx::BOOL)],
        &interner,
        &pool,
    );

    assert_eq!(interner.lookup(mangled), "make_pair$m$int_bool");
}

#[test]
fn mangle_list_type() {
    let interner = make_interner();
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let fn_name = interner.intern("filter");

    let mangled = mangle_mono_name(fn_name, &[GenericArg::Type(list_int)], &interner, &pool);

    assert_eq!(interner.lookup(mangled), "filter$m$Lint");
}

#[test]
fn mangle_option_type() {
    let interner = make_interner();
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let fn_name = interner.intern("unwrap");

    let mangled = mangle_mono_name(fn_name, &[GenericArg::Type(opt_str)], &interner, &pool);

    assert_eq!(interner.lookup(mangled), "unwrap$m$Ostr");
}

#[test]
fn mangle_tuple_type() {
    let interner = make_interner();
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::BOOL]);
    let fn_name = interner.intern("swap");

    let mangled = mangle_mono_name(fn_name, &[GenericArg::Type(tup)], &interner, &pool);

    assert_eq!(interner.lookup(mangled), "swap$m$Tint_bool");
}

#[test]
fn collect_produces_concrete_sig() {
    let interner = make_interner();
    let pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);

    let instance = MonoInstance {
        fn_name: generic_sig.name,
        generic_args: vec![GenericArg::Type(Idx::INT)],
        concrete_param_types: vec![Idx::INT],
        concrete_return_type: Idx::INT,
        body_type_map: FxHashMap::default(),
    };

    let mono_fns = collect_mono_functions(&[instance], &[generic_sig], &interner, &pool);

    assert_eq!(mono_fns.len(), 1);
    let mf = &mono_fns[0];
    assert_eq!(interner.lookup(mf.mangled_name), "identity$m$int");
    assert_eq!(interner.lookup(mf.original_name), "identity");
    assert!(
        mf.sig.type_params.is_empty(),
        "concrete sig should have no type params"
    );
    assert!(!mf.sig.is_generic());
    assert_eq!(mf.sig.param_types, vec![Idx::INT]);
    assert_eq!(mf.sig.return_type, Idx::INT);
}

#[test]
fn collect_skips_unknown_function() {
    let interner = make_interner();
    let pool = Pool::new();
    let unknown_name = interner.intern("nonexistent");

    let instance = MonoInstance {
        fn_name: unknown_name,
        generic_args: vec![GenericArg::Type(Idx::INT)],
        concrete_param_types: vec![Idx::INT],
        concrete_return_type: Idx::INT,
        body_type_map: FxHashMap::default(),
    };

    let mono_fns = collect_mono_functions(
        &[instance],
        &[], // no sigs
        &interner,
        &pool,
    );

    assert!(
        mono_fns.is_empty(),
        "should skip instances for unknown functions"
    );
}
