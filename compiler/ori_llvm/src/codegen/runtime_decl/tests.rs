use std::collections::BTreeSet;

use super::*;
use crate::aot::TargetConfig;
use crate::context::SimpleCx;
use inkwell::context::Context;
use inkwell::targets::{TargetData, TargetTriple};

#[test]
fn runtime_functions_declared() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_runtime");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    // Verify ALL runtime functions from RT_FUNCTIONS are in the module
    for name in all_names() {
        assert!(
            scx.llmod.get_function(name).is_some(),
            "runtime function '{name}' should be declared"
        );
    }
}

#[test]
fn empty_module_has_no_runtime_declarations() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_empty");
    let _builder = IrBuilder::new(&scx);

    // Without calling declare_runtime or runtime_fn, no functions exist
    assert!(
        scx.llmod.get_first_function().is_none(),
        "empty module should have zero function declarations"
    );
}

#[test]
fn lazy_declares_only_requested_function() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_lazy");
    let mut builder = IrBuilder::new(&scx);

    // Request only ori_print
    builder.runtime_fn("ori_print");

    // Count functions in module — should be exactly 1
    let mut n = 0;
    let mut func = scx.llmod.get_first_function();
    while let Some(f) = func {
        n += 1;
        func = f.get_next_function();
    }
    assert_eq!(n, 1, "should have exactly 1 declaration, got {n}");
    assert!(scx.llmod.get_function("ori_print").is_some());
    assert!(scx.llmod.get_function("ori_rc_alloc").is_none());
}

#[test]
fn iter_map_declaration_includes_output_release_thunk() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_iter_map_abi");
    let mut builder = IrBuilder::new(&scx);

    builder.runtime_fn("ori_iter_map");

    let map = scx.llmod.get_function("ori_iter_map").unwrap();
    assert_eq!(map.count_params(), 7);
    for hook_index in [3, 4] {
        assert!(
            matches!(map.get_nth_param(hook_index), Some(param) if param.is_pointer_value()),
            "ori_iter_map's callback-environment hooks must be pointers"
        );
    }
    assert!(
        matches!(map.get_nth_param(6), Some(param) if param.is_pointer_value()),
        "ori_iter_map's seventh ABI slot must carry the output release thunk"
    );
}

#[test]
fn iterator_callback_abi_is_stable_on_non_host_targets() {
    for triple in ["aarch64-apple-darwin", "wasm32-unknown-unknown"] {
        let target = TargetConfig::from_triple(triple)
            .unwrap_or_else(|error| panic!("{triple} must be an initialized LLVM target: {error}"));
        let layout = target
            .data_layout()
            .unwrap_or_else(|error| panic!("{triple} must expose a data layout: {error}"));

        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_iterator_callback_cross_target_abi");
        scx.llmod.set_triple(&TargetTriple::create(target.triple()));
        let target_data = TargetData::create(&layout);
        scx.llmod.set_data_layout(&target_data.get_data_layout());
        let mut builder = IrBuilder::new(&scx);

        for name in [
            "ori_iter_map",
            "ori_iter_filter",
            "ori_iter_collect",
            "ori_iter_collect_set",
            "ori_iter_find",
            "ori_iter_last",
            "ori_iter_rfind",
            "ori_iter_join",
        ] {
            builder.runtime_fn(name);
        }

        let ir = scx.llmod.print_to_string().to_string();
        assert!(
            ir.contains("declare ptr @ori_iter_map(ptr, ptr, ptr, ptr, ptr, i64, ptr)"),
            "{triple} must preserve map callback ownership and output cleanup:\n{ir}"
        );
        assert!(
            ir.contains("declare ptr @ori_iter_filter(ptr, ptr, ptr, ptr, ptr, i64)"),
            "{triple} must preserve filter callback-environment ownership:\n{ir}"
        );
        assert!(
            ir.contains("declare void @ori_iter_collect(ptr, i64, ptr, ptr, ptr)"),
            "{triple} must preserve the collect element-release-thunk slot:\n{ir}"
        );
        assert!(
            ir.contains("declare void @ori_iter_collect_set(ptr, i64, ptr, ptr, ptr, ptr, ptr)"),
            "{triple} must preserve the set-collect element-release-thunk slot:\n{ir}"
        );
        assert!(
            ir.contains("declare void @ori_iter_find(ptr, ptr, ptr, i64, ptr, ptr)"),
            "{triple} must preserve find's retain-before-output ABI:\n{ir}"
        );
        assert!(
            ir.contains("declare void @ori_iter_last(ptr, i64, ptr, ptr)"),
            "{triple} must preserve last's retain-before-output ABI:\n{ir}"
        );
        assert!(
            ir.contains("declare void @ori_iter_rfind(ptr, ptr, ptr, i64, ptr, ptr)"),
            "{triple} must preserve rfind's retain-before-output ABI:\n{ir}"
        );
        assert!(
            ir.contains("declare void @ori_iter_join(ptr, i64, i64, ptr, ptr, ptr, i64, ptr)"),
            "{triple} join ABI must leave yield provenance inside IterState:\n{ir}"
        );
        assert!(
            scx.llmod.verify().is_ok(),
            "iterator callback declarations must verify for {triple}:\n{ir}"
        );
    }
}

#[test]
fn runtime_fn_caches_function_id() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_cache");
    let mut builder = IrBuilder::new(&scx);

    let id1 = builder.runtime_fn("ori_rc_inc");
    let id2 = builder.runtime_fn("ori_rc_inc");

    assert_eq!(
        id1, id2,
        "repeated runtime_fn() calls should return same ID"
    );

    // Still only 1 function in the module
    let mut n = 0;
    let mut func = scx.llmod.get_first_function();
    while let Some(f) = func {
        n += 1;
        func = f.get_next_function();
    }
    assert_eq!(n, 1, "cached call should not create duplicates");
}

#[test]
fn lazy_declaration_preserves_attributes() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_lazy_attrs");
    let mut builder = IrBuilder::new(&scx);

    // Declare only RC functions via lazy path
    builder.runtime_fn("ori_rc_alloc");
    builder.runtime_fn("ori_rc_inc");

    let ir = scx.llmod.print_to_string().to_string();

    // ori_rc_alloc should have noalias return
    assert!(
        ir.contains("noalias") && ir.contains("ori_rc_alloc"),
        "lazily declared ori_rc_alloc should have noalias: {ir}"
    );

    // nounwind should be present (both functions have it)
    assert!(
        ir.contains("nounwind"),
        "lazily declared RC functions should have nounwind: {ir}"
    );
}

#[test]
fn str_functions_use_sret_convention() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_str_types");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    let ir = scx.llmod.print_to_string().to_string();

    // String-returning functions use sret convention (24-byte OriStr > 16-byte
    // x86-64 SysV register return threshold). The sret pointer is the first
    // parameter, and the LLVM return type is void.
    let concat = scx.llmod.get_function("ori_str_concat").unwrap();
    assert!(
        concat.get_type().get_return_type().is_none(),
        "ori_str_concat should return void (sret), got {:?}",
        concat.get_type().get_return_type()
    );

    let from_int = scx.llmod.get_function("ori_str_from_int").unwrap();
    assert!(
        from_int.get_type().get_return_type().is_none(),
        "ori_str_from_int should return void (sret), got {:?}",
        from_int.get_type().get_return_type()
    );

    // The sret attribute should appear in the IR for string functions
    assert!(
        ir.contains("sret"),
        "string-returning functions should have sret attribute in IR:\n{ir}"
    );
}

#[test]
fn void_functions_have_no_return() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_void_fns");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    // Void functions should have no return type
    let print = scx.llmod.get_function("ori_print").unwrap();
    assert!(
        print.get_type().get_return_type().is_none(),
        "ori_print should return void"
    );

    let panic = scx.llmod.get_function("ori_panic").unwrap();
    assert!(
        panic.get_type().get_return_type().is_none(),
        "ori_panic should return void"
    );
}

#[test]
fn declare_runtime_is_idempotent() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_idempotent");
    let mut builder = IrBuilder::new(&scx);

    // Calling twice should not panic or duplicate
    declare_runtime(&mut builder);
    declare_runtime(&mut builder);

    assert!(scx.llmod.get_function("ori_print").is_some());
}

#[test]
fn rc_functions_have_arc_safe_attributes() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_rc_attrs");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    let ir = scx.llmod.print_to_string().to_string();

    // ori_rc_alloc: nounwind + noalias return
    assert!(
        ir.contains("noalias") && ir.contains("ori_rc_alloc"),
        "ori_rc_alloc should have noalias return attribute in IR:\n{ir}"
    );

    // ori_rc_inc: nounwind + memory(argmem: readwrite)
    // ori_rc_dec: nounwind + memory(argmem: readwrite)
    // The `memory` attribute should appear as an enum attribute, not string
    assert!(
        ir.contains("ori_rc_inc"),
        "ori_rc_inc should be declared in IR"
    );
    assert!(
        ir.contains("ori_rc_dec"),
        "ori_rc_dec should be declared in IR"
    );

    // INVARIANT: RC declarations carry `nounwind` through an LLVM attribute group.
    assert!(
        ir.contains("nounwind"),
        "RC functions should have nounwind attribute in IR:\n{ir}"
    );
}

#[test]
fn panic_functions_have_cold_and_noreturn() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_panic_attrs");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    let ir = scx.llmod.print_to_string().to_string();

    // Panic functions should have cold + noreturn but NOT nounwind
    assert!(
        ir.contains("cold"),
        "panic functions should have cold attribute in IR:\n{ir}"
    );
    assert!(
        ir.contains("noreturn"),
        "panic functions should have noreturn attribute in IR:\n{ir}"
    );
}

#[test]
fn panic_functions_noreturn_not_nounwind() {
    use runtime_functions::is_rt_fn_noreturn;

    // ori_panic_cstr: noreturn = true, nounwind = false
    assert_eq!(
        runtime_functions::is_rt_fn_nounwind("ori_panic_cstr"),
        Some(false),
        "ori_panic_cstr must NOT be nounwind (must unwind for RC cleanup)"
    );
    assert_eq!(
        is_rt_fn_noreturn("ori_panic_cstr"),
        Some(true),
        "ori_panic_cstr must be noreturn (never returns to caller)"
    );

    // ori_panic: same — noreturn + not nounwind
    assert_eq!(
        runtime_functions::is_rt_fn_nounwind("ori_panic"),
        Some(false),
        "ori_panic must NOT be nounwind"
    );
    assert_eq!(
        is_rt_fn_noreturn("ori_panic"),
        Some(true),
        "ori_panic must be noreturn"
    );

    // ori_rc_inc: nounwind but not noreturn
    assert_eq!(
        runtime_functions::is_rt_fn_nounwind("ori_rc_inc"),
        Some(true),
        "ori_rc_inc should be nounwind"
    );
    assert_eq!(
        is_rt_fn_noreturn("ori_rc_inc"),
        Some(false),
        "ori_rc_inc should not be noreturn"
    );

    // Unknown function: None for both
    assert_eq!(
        is_rt_fn_noreturn("not_a_real_fn"),
        None,
        "unknown function should return None"
    );
}

/// Verifies that every function in `RT_FUNCTIONS` has a `jit_allowed` classification
/// and that JIT + AOT-only partitions cover all entries exactly.
///
/// The `jit_allowed` field places every entry in exactly one partition.
#[test]
fn declared_functions_all_have_jit_classification() {
    let total = all_names().count();
    let jit_count = runtime_functions::jit_allowed_names().count();
    let aot_count = RT_FUNCTIONS.iter().filter(|f| !f.jit_allowed).count();

    assert_eq!(
        jit_count + aot_count,
        total,
        "jit_allowed partition doesn't cover all RT_FUNCTIONS: \
         {jit_count} JIT + {aot_count} AOT = {} != {total}",
        jit_count + aot_count,
    );
    assert!(jit_count > 0, "no JIT-allowed functions — likely a bug");
    assert!(aot_count > 0, "no AOT-only functions — likely a bug");
}

/// Verifies that `jit_symbol_mappings` names exactly match
/// `jit_allowed_names` from `RT_FUNCTIONS`.
///
/// This catches drift where a new `jit_allowed: true` entry is added to
/// `RT_FUNCTIONS` but not to `jit_symbol_mappings` (or vice versa).
#[test]
fn jit_symbol_mappings_match_jit_allowed() {
    use crate::evaluator::jit_symbol_mappings;

    let mapping_names: BTreeSet<&str> = jit_symbol_mappings()
        .iter()
        .map(|(name, _)| *name)
        .collect();
    let allowed_names: BTreeSet<&str> = runtime_functions::jit_allowed_names().collect();

    let missing: BTreeSet<_> = allowed_names.difference(&mapping_names).collect();
    let extra: BTreeSet<_> = mapping_names.difference(&allowed_names).collect();

    assert!(
        missing.is_empty(),
        "jit_allowed but missing from jit_symbol_mappings(): {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "in jit_symbol_mappings() but not jit_allowed: {extra:?}"
    );
}

/// Verifies that `elem_dec`/`elem_count` buffer helpers are registered
/// as JIT-allowed symbols, so MCJIT can resolve them when compiling list/set
/// literals.
#[test]
fn jit_symbols_include_elem_header_helpers() {
    use crate::evaluator::jit_symbol_mappings;

    let mapping_names: BTreeSet<&str> = jit_symbol_mappings()
        .iter()
        .map(|(name, _)| *name)
        .collect();

    let required = ["ori_buffer_store_elem_dec", "ori_buffer_store_elem_count"];

    for name in &required {
        assert!(
            mapping_names.contains(name),
            "elem header helper '{name}' missing from JIT symbol mappings — \
             list/set literals will fail in MCJIT"
        );
    }
}

/// Validates that every runtime function has either `Nounwind` or documented
/// justification for omitting it.
///
/// After the audit, only `extern "C-unwind"` functions should lack
/// `Nounwind`. All `extern "C"` functions cannot unwind by ABI contract and
/// must have `Nounwind`.
#[test]
fn all_non_unwinding_functions_have_nounwind() {
    // These functions are extern "C-unwind" and intentionally lack nounwind
    // because they call ori_panic on failure (assertions, OOB access).
    // Panic functions have Noreturn instead.
    let may_unwind: &[&str] = &[
        "ori_assert",
        "ori_assert_eq_int",
        "ori_assert_eq_bool",
        "ori_assert_eq_float",
        "ori_assert_eq_str",
        "ori_list_get",
        // extern "C-unwind": panics on out-of-bounds keys (IndexSet.updated,
        // matching ori_list_get's list[key] contract).
        "ori_list_updated_cow",
        // extern "C-unwind": panics on out-of-bounds codepoint index
        // (str[i], matching ori_list_get's list[key] contract).
        "ori_str_index",
        "ori_panic",
        "ori_panic_cstr",
        // extern "C-unwind": drop fn called directly so a user-@drop foreign
        // exception unwinds through to the caller's cleanup pad.
        "ori_rc_dec_unwind",
        // INVARIANT: Eager iteration may cross callbacks stored in its source adapter.
        "ori_iter_next",
        "ori_iter_next_back",
        "ori_iter_rev",
        "ori_iter_collect",
        "ori_iter_collect_set",
        "ori_iter_count",
        "ori_iter_any",
        "ori_iter_all",
        "ori_iter_find",
        "ori_iter_for_each",
        "ori_iter_fold",
        "ori_iter_last",
        "ori_iter_rfind",
        "ori_iter_rfold",
        "ori_iter_join",
    ];

    let mut missing_nounwind = Vec::new();
    for spec in RT_FUNCTIONS.iter() {
        if may_unwind.contains(&spec.name) {
            // Verify these do NOT have nounwind
            let has_nounwind = spec.attrs.iter().any(|a| matches!(a, Attr::Nounwind));
            assert!(
                !has_nounwind,
                "{} is extern \"C-unwind\" and should NOT have Nounwind",
                spec.name
            );
            continue;
        }

        let has_nounwind = spec.attrs.iter().any(|a| matches!(a, Attr::Nounwind));
        if !has_nounwind {
            missing_nounwind.push(spec.name);
        }
    }

    assert!(
        missing_nounwind.is_empty(),
        "extern \"C\" runtime functions missing Nounwind: {missing_nounwind:?}"
    );
}

/// Ensures no production code uses raw `get_function("ori_*")` to look up
/// runtime functions. All call sites should use `runtime_fn` instead,
/// which lazily declares + caches. Raw `get_function` bypasses the cache,
/// creating duplicate `FunctionId`s that violate lazy declaration.
///
/// Test files are excluded — they legitimately use `get_function` to
/// verify declarations exist.
#[test]
fn no_raw_get_function_for_runtime_fns() {
    use std::path::PathBuf;

    fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path.file_name().is_none_or(|name| name != "tests.rs")
            {
                out.push(path);
            }
        }
    }

    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);

    let mut violations = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap();
        for (line_no, line) in content.lines().enumerate() {
            if line.contains("get_function(\"ori_") {
                let rel = path.strip_prefix(&src_dir).unwrap();
                violations.push(format!(
                    "  {}:{}: {}",
                    rel.display(),
                    line_no + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found raw get_function(\"ori_*\") calls in production code.\n\
         Use `self.builder.runtime_fn(\"ori_...\")` instead.\n\
         Violations:\n{}",
        violations.join("\n")
    );
}

/// The COW out-pointer convention is declared per-entry in the table
/// (`Attr::NoaliasLastParam`), never derived from the `_cow` name suffix.
/// Pin the table: every `*_cow` entry declares the attribute, and every
/// entry declaring it has at least one parameter to attach it to.
#[test]
fn cow_entries_declare_noalias_last_param_in_table() {
    for spec in RT_FUNCTIONS.iter() {
        let has_attr = spec
            .attrs
            .iter()
            .any(|a| matches!(a, Attr::NoaliasLastParam));
        if spec.name.ends_with("_cow") {
            assert!(
                has_attr,
                "{}: COW runtime fn must declare Attr::NoaliasLastParam in the table",
                spec.name
            );
        }
        if has_attr {
            assert!(
                !spec.params.is_empty(),
                "{}: Attr::NoaliasLastParam requires at least one parameter",
                spec.name
            );
        }
    }
}

/// `Ty::needs_sret()` gates the x86-64 `SysV` ABI's 16-byte direct-passing
/// threshold on RETURN types (`Str`/`List`/`Map` at 24 bytes get sret + a
/// prepended out-pointer). No analogous gate exists for PARAMS: a
/// `Str`/`List`/`Map` param declared by value silently mis-classifies the
/// `SysV` eightbytes and shifts every following argument.
///
/// Regression: a runtime function declaring a >16-byte type (`Str`/`List`/
/// `Map`) by value in a params slot instead of `Ty::Ptr` corrupts the calling
/// convention (SIGABRT on a heap-message trace injection). Pin every table
/// entry's params against the same `needs_sret()` predicate the return side
/// already enforces.
#[test]
fn no_rtfn_param_uses_a_needs_sret_type_directly() {
    let offenders: Vec<&str> = RT_FUNCTIONS
        .iter()
        .filter(|spec| spec.params.iter().any(|ty| ty.needs_sret()))
        .map(|spec| spec.name)
        .collect();

    assert!(
        offenders.is_empty(),
        "runtime fns passing a >16-byte type (Str/List/Map) by value in params \
         (declare as Ty::Ptr instead): {offenders:?}"
    );
}
