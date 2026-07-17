//! Tests for the production AOT `ImportedMonoFn` carrier + builder.
//!
//! These tests verify the carrier shape + builder behavior post-promotion
//! from the JIT test-runner equivalent at `oric/src/test/runner/imported_mono.rs`.
//! The shared production type is reachable via `crate::commands::ImportedMonoFn`;
//! the builder produces identical output to the JIT version on identical input.

use rustc_hash::FxHashMap;

use ori_types::{FunctionSig, GenericArg, Idx, MonoInstance, Pool, TypeCheckResult, TypedModule};

use super::{build_imported_mono_functions, ImportedMonoFn};
use crate::ir::StringInterner;

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
fn carrier_is_reachable_via_crate_commands_export() {
    // ImportedMonoFn is `pub(crate)` in the production module; it must be
    // reachable from the test runner side via `crate::commands::ImportedMonoFn`.
    // This test pins the re-export.
    let interner = StringInterner::new();
    let imp_name = interner.intern("imported");
    let src_name = interner.intern("imported");
    let mono_fn = ori_repr::monomorphize::MonoFunction {
        mangled_name: imp_name,
        original_name: imp_name,
        origin: ori_repr::monomorphize::MonoFunctionOrigin::Source,
        sig: make_generic_sig(&interner),
        body_type_map: FxHashMap::default(),
        instance_ids: vec![],
        is_imported: true,
        receiver_type: None,
        receiver_type_name: None,
    };
    // The carrier shape is a 3-tuple (MonoFunction, module_idx, source_name).
    let entry: crate::commands::ImportedMonoFn = (mono_fn, 0usize, src_name);
    assert_eq!(entry.1, 0);
    assert_eq!(entry.2, src_name);
}

#[test]
fn build_imported_mono_functions_empty_input() {
    // Empty mono_instances + empty imported_generic_sigs → empty output.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let typed_module = TypedModule::default();
    let type_result = TypeCheckResult::ok(typed_module);
    let imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();

    let result: Vec<ImportedMonoFn> = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &mut pool,
        &interner,
    );

    assert!(result.is_empty());
}

#[test]
fn build_imported_mono_functions_skips_local_instances() {
    // Mono instance whose fn_name is NOT in imported_generic_sigs → skipped.
    // (Local mono instances are handled by the host's regular collect_mono_functions path.)
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let local_name = interner.intern("local_fn");

    let instance = MonoInstance::new_top_level(
        local_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);
    let type_result = TypeCheckResult::ok(typed_module);

    // Empty imported_generic_sigs — local_name is not imported.
    let imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();

    let result = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &mut pool,
        &interner,
    );

    assert!(
        result.is_empty(),
        "local mono instances must not appear in imported_mono_fns"
    );
}

#[test]
fn build_imported_mono_functions_constructs_imported_entry() {
    // A mono instance whose fn_name IS in imported_generic_sigs → entry built.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);
    let imported_local_name = generic_sig.name; // local == source for this test
    let source_name = generic_sig.name;

    let instance = MonoInstance::new_top_level(
        imported_local_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);
    let type_result = TypeCheckResult::ok(typed_module);

    let mut imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();
    imported_generic_sigs.insert(imported_local_name, (generic_sig, 5usize, source_name));

    let result = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &mut pool,
        &interner,
    );

    assert_eq!(result.len(), 1);
    let (mono_fn, module_idx, source_body_name) = &result[0];
    assert_eq!(
        interner.lookup(mono_fn.mangled_name),
        "identity$m$3_int",
        "mangled name follows the same scheme as collect_mono_functions"
    );
    assert_eq!(mono_fn.original_name, imported_local_name);
    assert!(mono_fn.is_imported);
    assert_eq!(*module_idx, 5usize);
    assert_eq!(*source_body_name, source_name);
}

#[test]
fn build_imported_mono_functions_dedups_by_mangled_name() {
    // Two instances with the same generic args → one MonoFunction with
    // both instance_ids appended.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);
    let name = generic_sig.name;

    let mut typed_module = TypedModule::default();
    for _ in 0..2 {
        typed_module
            .mono_instances
            .push(MonoInstance::new_top_level(
                name,
                vec![GenericArg::Type(Idx::INT)],
                vec![Idx::INT],
                Idx::INT,
                Vec::new(),
            ));
    }
    let type_result = TypeCheckResult::ok(typed_module);

    let mut imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();
    imported_generic_sigs.insert(name, (generic_sig, 0usize, name));

    let result = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &mut pool,
        &interner,
    );

    assert_eq!(
        result.len(),
        1,
        "duplicate (fn, args) deduped by mangled name"
    );
    assert_eq!(
        result[0].0.instance_ids.len(),
        2,
        "both instance_ids accumulated on the single MonoFunction"
    );
}
