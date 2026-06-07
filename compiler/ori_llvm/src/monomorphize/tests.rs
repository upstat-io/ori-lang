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
        param_hashes: vec![0],
        return_hash: 0,
    }
}

#[test]
fn mangle_single_type_param_int() {
    let interner = make_interner();
    let pool = Pool::new();
    let fn_name = interner.intern("identity");

    let mangled = mangle_mono_name(
        fn_name,
        &[GenericArg::Type(Idx::INT)],
        &[],
        &[],
        &interner,
        &pool,
    );

    // Length-prefixed top-level: "int" is 3 bytes → "3_int".
    assert_eq!(interner.lookup(mangled), "identity$m$3_int");
}

#[test]
fn mangle_two_type_params() {
    let interner = make_interner();
    let pool = Pool::new();
    let fn_name = interner.intern("make_pair");

    let mangled = mangle_mono_name(
        fn_name,
        &[GenericArg::Type(Idx::INT), GenericArg::Type(Idx::BOOL)],
        &[],
        &[],
        &interner,
        &pool,
    );

    // Length prefix replaces inter-arg "_" separator: each arg carries its
    // own byte length, so successive prefixed payloads concatenate
    // unambiguously: "3_int" then "4_bool".
    assert_eq!(interner.lookup(mangled), "make_pair$m$3_int4_bool");
}

#[test]
fn mangle_list_type() {
    let interner = make_interner();
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let fn_name = interner.intern("filter");

    let mangled = mangle_mono_name(
        fn_name,
        &[GenericArg::Type(list_int)],
        &[],
        &[],
        &interner,
        &pool,
    );

    // encode_type produces "Lint" (4 bytes) for [int].
    assert_eq!(interner.lookup(mangled), "filter$m$4_Lint");
}

#[test]
fn mangle_option_type() {
    let interner = make_interner();
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let fn_name = interner.intern("unwrap");

    let mangled = mangle_mono_name(
        fn_name,
        &[GenericArg::Type(opt_str)],
        &[],
        &[],
        &interner,
        &pool,
    );

    // encode_type produces "Ostr" (4 bytes) for Option<str>.
    assert_eq!(interner.lookup(mangled), "unwrap$m$4_Ostr");
}

#[test]
fn mangle_tuple_type() {
    let interner = make_interner();
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::BOOL]);
    let fn_name = interner.intern("swap");

    let mangled = mangle_mono_name(
        fn_name,
        &[GenericArg::Type(tup)],
        &[],
        &[],
        &interner,
        &pool,
    );

    // encode_type produces "Tint_bool" (9 bytes) for (int, bool).
    assert_eq!(interner.lookup(mangled), "swap$m$9_Tint_bool");
}

#[test]
fn collect_produces_concrete_sig() {
    let interner = make_interner();
    let pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);

    let instance = MonoInstance::new_top_level(
        generic_sig.name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(&[instance], &[generic_sig], &[], &[], &interner, &pool);

    assert_eq!(mono_fns.len(), 1);
    let mf = &mono_fns[0];
    assert_eq!(interner.lookup(mf.mangled_name), "identity$m$3_int");
    assert_eq!(interner.lookup(mf.original_name), "identity");
    assert!(
        mf.sig.type_params.is_empty(),
        "concrete sig should have no type params"
    );
    assert!(!mf.sig.is_generic());
    assert_eq!(mf.sig.param_types, vec![Idx::INT]);
    assert_eq!(mf.sig.return_type, Idx::INT);
    assert!(
        !mf.is_imported,
        "function_sigs hit should yield is_imported=false"
    );
}

#[test]
fn collect_skips_unknown_function() {
    let interner = make_interner();
    let pool = Pool::new();
    let unknown_name = interner.intern("nonexistent");

    let instance = MonoInstance::new_top_level(
        unknown_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(
        &[instance],
        &[], // no top-level sigs
        &[], // no impl sigs either
        &[], // no import sigs either
        &interner,
        &pool,
    );

    assert!(
        mono_fns.is_empty(),
        "should skip instances for unknown functions"
    );
}

#[test]
fn collect_resolves_top_level_via_import_sigs() {
    // Positive: a top-level (receiver_type=None) instance whose fn_name is
    // missing from function_sigs falls through to import_sigs and yields a
    // MonoFunction with is_imported=true.
    let interner = make_interner();
    let pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);
    let imported_name = generic_sig.name;

    let instance = MonoInstance::new_top_level(
        imported_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(
        &[instance],
        &[],                             // function_sigs empty — forces fall-through
        &[],                             // impl_sigs empty
        &[(imported_name, generic_sig)], // import_sigs supplies the sig
        &interner,
        &pool,
    );

    assert_eq!(mono_fns.len(), 1);
    let mf = &mono_fns[0];
    assert_eq!(interner.lookup(mf.mangled_name), "identity$m$3_int");
    assert!(
        mf.is_imported,
        "import_sigs hit should yield is_imported=true"
    );
    assert!(!mf.sig.is_generic());
    assert_eq!(mf.sig.param_types, vec![Idx::INT]);
}

#[test]
fn collect_does_not_consult_import_sigs_for_methods() {
    // INVARIANT: a method instance (receiver_type=Some) MUST NOT fall
    // through to import_sigs even when the name matches — methods are
    // dispatched via the impl_sigs / inherent-method path, never the
    // top-level imported-fn path.
    let interner = make_interner();
    let pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);
    let method_name = generic_sig.name;

    // Method instance: receiver_type = Some(Idx::INT).
    let instance = MonoInstance::new_method(
        method_name,
        vec![GenericArg::Type(Idx::INT)], // impl_args
        vec![],                           // method_args
        Idx::INT,                         // receiver_type
        vec![Idx::INT],                   // concrete_param_types
        Idx::INT,                         // concrete_return_type
        Vec::new(),                       // body_type_map
    );

    let mono_fns = collect_mono_functions(
        &[instance],
        &[],                           // function_sigs empty
        &[],                           // impl_sigs empty — would normally drive the miss
        &[(method_name, generic_sig)], // import_sigs HAS the name but methods must NOT consult it
        &interner,
        &pool,
    );

    assert!(
        mono_fns.is_empty(),
        "method instances must not fall through to import_sigs — methods are dispatched via the impl_sigs / inherent-method path"
    );
}

#[test]
fn collect_prefers_function_sigs_over_import_sigs_on_name_collision() {
    // INVARIANT: when the same fn name is present in BOTH function_sigs and
    // import_sigs, the top-level lookup chain resolves via function_sigs —
    // the local definition wins and is_imported stays false.
    let interner = make_interner();
    let pool = Pool::new();
    let local_sig = make_generic_sig(&interner);
    let name = local_sig.name;
    let local_param = interner.intern("x");

    // Imported twin under the SAME name with a distinguishable param name so
    // the assertion proves which sig supplied the metadata.
    let mut imported_sig = make_generic_sig(&interner);
    imported_sig.param_names = vec![interner.intern("y")];

    let instance = MonoInstance::new_top_level(
        name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(
        &[instance],
        std::slice::from_ref(&local_sig),
        &[],
        &[(name, imported_sig)],
        &interner,
        &pool,
    );

    assert_eq!(mono_fns.len(), 1);
    let mf = &mono_fns[0];
    assert!(
        !mf.is_imported,
        "function_sigs must win the lookup on a name collision with import_sigs"
    );
    assert_eq!(
        mf.sig.param_names,
        vec![local_param],
        "concrete sig metadata must come from the function_sigs entry, not the import_sigs twin"
    );
    assert_eq!(interner.lookup(mf.mangled_name), "identity$m$3_int");
}

// Supplementary mangling edge cases: these unit-tests verify the four
// mangling shapes — top-level, method-impl-only, method-with-method-args,
// and method-no-impl-args — at the API surface, independent of upstream
// typeck wiring.

#[test]
fn mangle_method_distinct_from_top_level() {
    // Top-level vs method distinction.
    // A top-level instance with one type arg and a method instance whose
    // impl_args carry the same type arg MUST produce different symbols —
    // the trailing $im$ separator on the method form is the discriminator.
    let interner = make_interner();
    let pool = Pool::new();
    let identity_name = interner.intern("identity");
    let bar_name = interner.intern("bar");

    let top_level = mangle_mono_name(
        identity_name,
        &[GenericArg::Type(Idx::INT)],
        &[],
        &[],
        &interner,
        &pool,
    );
    let method_impl_only = mangle_mono_name(
        bar_name,
        &[],
        &[GenericArg::Type(Idx::INT)],
        &[],
        &interner,
        &pool,
    );

    assert_eq!(interner.lookup(top_level), "identity$m$3_int");
    assert_eq!(interner.lookup(method_impl_only), "bar$m$3_int$im$");
    assert_ne!(top_level, method_impl_only);
}

#[test]
fn mangle_method_impl_and_method_args() {
    // Mangling-no-collision: a method instance with both impl_args and
    // method_args produces the canonical
    // <fn>$m$<L0(impl_args)>$im$<L0(method_args)> shape.
    let interner = make_interner();
    let pool = Pool::new();
    let bar_name = interner.intern("bar");

    let mangled = mangle_mono_name(
        bar_name,
        &[],
        &[GenericArg::Type(Idx::INT)],
        &[GenericArg::Type(Idx::STR)],
        &interner,
        &pool,
    );

    // impl_args=[int] → "3_int", method_args=[str] → "3_str".
    assert_eq!(interner.lookup(mangled), "bar$m$3_int$im$3_str");
}

#[test]
fn mangle_method_no_impl_args() {
    // Method-level only (no impl-level generics)
    // — `impl Box<int> { @m<U> ... }` shape. Empty impl_args section
    // between `$m$` and `$im$` is load-bearing — it preserves bijectivity
    // when the method-arg payload happens to look like an impl-arg payload.
    let interner = make_interner();
    let pool = Pool::new();
    let m_name = interner.intern("m");

    let mangled = mangle_mono_name(
        m_name,
        &[],
        &[],
        &[GenericArg::Type(Idx::STR)],
        &interner,
        &pool,
    );

    // Empty impl_args section → "m$m$" then "$im$" then "3_str".
    assert_eq!(interner.lookup(mangled), "m$m$$im$3_str");
}

#[test]
fn mangle_method_swapped_args_distinct() {
    // INVARIANT (injection bijectivity): swapping impl_args and method_args
    // produces structurally distinct mangled symbols even when the type
    // sets are identical. With length-prefix encoding plus the $im$
    // separator there is no possible collision between the two shapes.
    let interner = make_interner();
    let pool = Pool::new();
    let bar_name = interner.intern("bar");

    let m1 = mangle_mono_name(
        bar_name,
        &[],
        &[GenericArg::Type(Idx::INT)],
        &[GenericArg::Type(Idx::STR)],
        &interner,
        &pool,
    );
    let m2 = mangle_mono_name(
        bar_name,
        &[],
        &[GenericArg::Type(Idx::STR)],
        &[GenericArg::Type(Idx::INT)],
        &interner,
        &pool,
    );

    assert_eq!(interner.lookup(m1), "bar$m$3_int$im$3_str");
    assert_eq!(interner.lookup(m2), "bar$m$3_str$im$3_int");
    assert_ne!(m1, m2);
}
