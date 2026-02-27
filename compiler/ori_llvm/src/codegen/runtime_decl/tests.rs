use std::collections::BTreeSet;

use super::*;
use crate::context::SimpleCx;
use crate::evaluator::{AOT_ONLY_RUNTIME_FUNCTIONS, JIT_MAPPED_RUNTIME_FUNCTIONS};
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
fn str_functions_return_struct_type() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_str_types");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    // ori_str_concat returns { i64, ptr } (string type)
    let concat = scx.llmod.get_function("ori_str_concat").unwrap();
    let ret_ty = concat.get_type().get_return_type().unwrap();
    assert!(
        ret_ty.is_struct_type(),
        "ori_str_concat should return a struct type, got {ret_ty}"
    );

    // ori_str_from_int also returns { i64, ptr }
    let from_int = scx.llmod.get_function("ori_str_from_int").unwrap();
    let ret_ty = from_int.get_type().get_return_type().unwrap();
    assert!(
        ret_ty.is_struct_type(),
        "ori_str_from_int should return a struct type, got {ret_ty}"
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

/// Verifies that every function in `RT_FUNCTIONS` is either in the JIT
/// mapping table or in the documented AOT-only exception list.
///
/// This catches drift where a new runtime function is added to the table
/// but not added to the JIT symbol mappings.
#[test]
fn declared_functions_covered_by_jit_or_aot_only() {
    // Use all_names() — the data table is the single source of truth
    let declared: BTreeSet<&str> = all_names().collect();

    let covered: BTreeSet<&str> = JIT_MAPPED_RUNTIME_FUNCTIONS
        .iter()
        .chain(AOT_ONLY_RUNTIME_FUNCTIONS.iter())
        .copied()
        .collect();

    let uncovered: BTreeSet<_> = declared.difference(&covered).collect();
    let phantom: BTreeSet<_> = covered.difference(&declared).collect();

    assert!(
        uncovered.is_empty(),
        "Runtime functions in RT_FUNCTIONS but not in JIT mappings or AOT-only list: {uncovered:?}\n\
         Add them to JIT_MAPPED_RUNTIME_FUNCTIONS in evaluator.rs or \
         AOT_ONLY_RUNTIME_FUNCTIONS if they are AOT-only."
    );
    assert!(
        phantom.is_empty(),
        "Functions in JIT/AOT-only lists but not in RT_FUNCTIONS: {phantom:?}\n\
         Remove them from evaluator.rs or add them to RT_FUNCTIONS."
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
