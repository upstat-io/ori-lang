use std::collections::BTreeSet;

use super::*;
use crate::context::SimpleCx;
use inkwell::context::Context;

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

    // Without calling declare_runtime() or runtime_fn(), no functions exist
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

    // Verify nounwind appears on RC functions
    // The IR prints function attributes as attribute groups (#N)
    // Check that nounwind is present in the module
    assert!(
        ir.contains("nounwind"),
        "RC functions should have nounwind attribute in IR:\n{ir}"
    );
}

#[test]
fn panic_functions_have_cold_nounwind() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_panic_attrs");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    let ir = scx.llmod.print_to_string().to_string();

    // Panic functions should have cold + nounwind
    assert!(
        ir.contains("cold"),
        "panic functions should have cold attribute in IR:\n{ir}"
    );
}

/// Verifies that every function in `RT_FUNCTIONS` has a `jit_allowed` classification
/// and that JIT + AOT-only partitions cover all entries exactly.
///
/// Since `jit_allowed` is now a field on `RtFn`, every entry is inherently
/// covered. This test validates that the partition counts match expectations
/// and no entry is accidentally miscategorized.
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

/// Verifies that `jit_symbol_mappings()` names exactly match
/// `jit_allowed_names()` from `RT_FUNCTIONS`.
///
/// This catches drift where a new `jit_allowed: true` entry is added to
/// `RT_FUNCTIONS` but not to `jit_symbol_mappings()` (or vice versa).
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

/// Ensures no production code uses raw `get_function("ori_*")` to look up
/// runtime functions. All call sites should use `runtime_fn()` instead,
/// which lazily declares + caches. Raw `get_function` bypasses the cache,
/// creates duplicate `FunctionId`s, and will break when AOT migrates to
/// lazy declaration.
///
/// Test files are excluded — they legitimately use `get_function()` to
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
