use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, GenericArg, Idx, MonoInstance, Pool};

use super::{collect_mono_functions, mangle_mono_name, ImportSig};

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
        return_projection: None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        &[], // function_sigs empty — forces fall-through
        &[], // impl_sigs empty
        &[ImportSig {
            name: imported_name,
            symbol: "_ori_lib$imported".to_string(),
            sig: generic_sig,
        }], // import_sigs supplies the sig
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
        &[], // function_sigs empty
        &[], // impl_sigs empty — would normally drive the miss
        &[ImportSig {
            name: method_name,
            symbol: "_ori_lib$method".to_string(),
            sig: generic_sig,
        }], // import_sigs HAS the name but methods must NOT consult it
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
        &[ImportSig {
            name,
            symbol: "_ori_lib$imported".to_string(),
            sig: imported_sig,
        }],
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

#[test]
fn mangle_method_distinct_from_top_level() {
    // INVARIANT: identical type args under top-level vs method mangling produce
    // distinct symbols — the trailing $im$ separator is the discriminator.
    let interner = make_interner();
    let pool = Pool::new();
    let identity_name = interner.intern("identity");
    let bar_name = interner.intern("bar");

    let top_level = mangle_mono_name(
        identity_name,
        &[GenericArg::Type(Idx::INT)],
        &[],
        &[],
        None,
        &interner,
        &pool,
    );
    let method_impl_only = mangle_mono_name(
        bar_name,
        &[],
        &[GenericArg::Type(Idx::INT)],
        &[],
        None,
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
        None,
        &interner,
        &pool,
    );

    // impl_args=[int] → "3_int", method_args=[str] → "3_str".
    assert_eq!(interner.lookup(mangled), "bar$m$3_int$im$3_str");
}

#[test]
fn mangle_method_no_impl_args() {
    // INVARIANT: the empty impl_args section between $m$ and $im$ preserves
    // bijectivity when a method-arg payload looks like an impl-arg payload.
    let interner = make_interner();
    let pool = Pool::new();
    let m_name = interner.intern("m");

    let mangled = mangle_mono_name(
        m_name,
        &[],
        &[],
        &[GenericArg::Type(Idx::STR)],
        None,
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
        None,
        &interner,
        &pool,
    );
    let m2 = mangle_mono_name(
        bar_name,
        &[],
        &[GenericArg::Type(Idx::STR)],
        &[GenericArg::Type(Idx::INT)],
        None,
        &interner,
        &pool,
    );

    assert_eq!(interner.lookup(m1), "bar$m$3_int$im$3_str");
    assert_eq!(interner.lookup(m2), "bar$m$3_str$im$3_int");
    assert_ne!(m1, m2);
}

/// Build a method `FunctionSig` whose `self` param has type `self_ty`.
/// The receiver shell of `self_ty` keys the impl-method dispatch table.
fn make_method_sig(interner: &StringInterner, method_name: Name, self_ty: Idx) -> FunctionSig {
    let self_param = interner.intern("self");
    FunctionSig {
        name: method_name,
        type_params: vec![interner.intern("T")],
        const_params: vec![],
        param_names: vec![self_param],
        param_types: vec![self_ty],
        return_type: Idx::INT,
        capabilities: vec![],
        is_public: true,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![vec![]],
        where_clauses: vec![],
        generic_param_mapping: vec![None],
        scheme_var_ids: vec![0],
        required_params: 1,
        param_defaults: vec![],
        param_hashes: vec![0],
        return_hash: 0,
        return_projection: None,
    }
}

#[test]
fn collect_resolves_same_method_name_on_distinct_receiver_shells() {
    // Two impl blocks register method `get` on distinct generic heads
    // (`Box<T>` and `Wrap<T>`). The (name, shell)-keyed table resolves each
    // receiver to its own sig — the name-only first-match would have dropped
    // the second registration and mis-dispatched one of the instances.
    let interner = make_interner();
    let mut pool = Pool::new();
    let get_name = interner.intern("get");
    let box_name = interner.intern("Box");
    let wrap_name = interner.intern("Wrap");

    let bv = pool.bound_var(0);
    let box_generic = pool.applied(box_name, &[bv]);
    let wrap_generic = pool.applied(wrap_name, &[bv]);
    let box_int = pool.applied(box_name, &[Idx::INT]);
    let wrap_int = pool.applied(wrap_name, &[Idx::INT]);

    let sig_box = make_method_sig(&interner, get_name, box_generic);
    let sig_wrap = make_method_sig(&interner, get_name, wrap_generic);
    let impl_sigs = vec![
        (box_generic, get_name, sig_box),
        (wrap_generic, get_name, sig_wrap),
    ];

    let inst_box = MonoInstance::new_method(
        get_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![],
        box_int,
        vec![box_int],
        Idx::INT,
        Vec::new(),
    );
    let inst_wrap = MonoInstance::new_method(
        get_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![],
        wrap_int,
        vec![wrap_int],
        Idx::INT,
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(
        &[inst_box, inst_wrap],
        &[],
        &impl_sigs,
        &[],
        &interner,
        &pool,
    );

    assert_eq!(
        mono_fns.len(),
        2,
        "both receiver-distinct method instances must resolve, got: {mono_fns:?}"
    );
    // Each resolved sig's self-param type must match its own receiver shell.
    let box_fn = mono_fns
        .iter()
        .find(|m| m.sig.param_types == vec![box_int])
        .unwrap_or_else(|| panic!("no MonoFunction for Box<int>.get: {mono_fns:?}"));
    assert_eq!(box_fn.original_name, get_name);
    let wrap_fn = mono_fns
        .iter()
        .find(|m| m.sig.param_types == vec![wrap_int])
        .unwrap_or_else(|| panic!("no MonoFunction for Wrap<int>.get: {mono_fns:?}"));
    assert_eq!(wrap_fn.original_name, get_name);
}

#[test]
fn collect_resolves_no_self_assoc_fn_by_owning_receiver_shell() {
    // Regression: a no-`self` associated function (`@new (v: T) -> Box<T>`) is
    // keyed by its OWNING receiver shell (`Box<_>`), NOT by `param_types.first()`
    // (the value param `v: T`, whose shell differs). Pre-fix the registration
    // keyed on the value param, so the receiver-shell lookup missed and the
    // specialization was dropped (LLVM panicked `no method new on
    // type int`). Reverting the receiver-keyed registration drops this to 0.
    let interner = make_interner();
    let mut pool = Pool::new();
    let new_name = interner.intern("new");
    let box_name = interner.intern("Box");
    let v_name = interner.intern("v");

    let bv = pool.bound_var(0);
    let box_generic = pool.applied(box_name, &[bv]); // owning receiver `Box<T>`
    let box_int = pool.applied(box_name, &[Idx::INT]);

    // `@new (v: T) -> Box<T>` — NO self; param[0] is the value param `v: T`,
    // whose shell differs from the owning receiver shell.
    let assoc_sig = FunctionSig {
        name: new_name,
        type_params: vec![interner.intern("T")],
        const_params: vec![],
        param_names: vec![v_name],
        param_types: vec![bv], // value param `T`, NOT the receiver
        return_type: box_generic,
        capabilities: vec![],
        is_public: true,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![vec![]],
        where_clauses: vec![],
        generic_param_mapping: vec![None],
        scheme_var_ids: vec![0],
        required_params: 1,
        param_defaults: vec![],
        param_hashes: vec![0],
        return_hash: 0,
        return_projection: None,
    };
    let impl_sigs = vec![(box_generic, new_name, assoc_sig)];

    let inst = MonoInstance::new_method(
        new_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![],
        box_int,        // receiver `Box<int>`
        vec![Idx::INT], // concrete value param (no self)
        box_int,        // return `Box<int>`
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(&[inst], &[], &impl_sigs, &[], &interner, &pool);

    assert_eq!(
        mono_fns.len(),
        1,
        "no-self assoc fn must resolve under the owning receiver shell, not the \
         value-param shell, got: {mono_fns:?}"
    );
    assert_eq!(mono_fns[0].original_name, new_name);
}

#[test]
fn collect_skips_method_when_no_shell_matches() {
    // A method instance whose receiver shell matches no registered impl_sig
    // misses cleanly (silent skip), never mis-dispatches to a same-named
    // method on a different receiver.
    let interner = make_interner();
    let mut pool = Pool::new();
    let get_name = interner.intern("get");
    let box_name = interner.intern("Box");
    let other_name = interner.intern("Other");

    let bv = pool.bound_var(0);
    let box_generic = pool.applied(box_name, &[bv]);
    let other_int = pool.applied(other_name, &[Idx::INT]);

    let sig_box = make_method_sig(&interner, get_name, box_generic);
    let impl_sigs = vec![(box_generic, get_name, sig_box)];

    // Receiver is Other<int> — name matches `get` but shell does not.
    let inst = MonoInstance::new_method(
        get_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![],
        other_int,
        vec![other_int],
        Idx::INT,
        Vec::new(),
    );

    let mono_fns = collect_mono_functions(&[inst], &[], &impl_sigs, &[], &interner, &pool);

    assert!(
        mono_fns.is_empty(),
        "receiver with no matching shell must miss cleanly, got: {mono_fns:?}"
    );
}

#[test]
fn mangle_method_distinct_per_receiver_instantiation() {
    // Box<int>.unwrap and Box<str>.unwrap carry distinct impl_args, so their
    // mangled names differ even though method name + receiver shell match.
    let interner = make_interner();
    let pool = Pool::new();
    let unwrap_name = interner.intern("unwrap");

    let m_int = mangle_mono_name(
        unwrap_name,
        &[],
        &[GenericArg::Type(Idx::INT)],
        &[],
        None,
        &interner,
        &pool,
    );
    let m_str = mangle_mono_name(
        unwrap_name,
        &[],
        &[GenericArg::Type(Idx::STR)],
        &[],
        None,
        &interner,
        &pool,
    );

    assert_eq!(interner.lookup(m_int), "unwrap$m$3_int$im$");
    assert_eq!(interner.lookup(m_str), "unwrap$m$3_str$im$");
    assert_ne!(m_int, m_str);
}
