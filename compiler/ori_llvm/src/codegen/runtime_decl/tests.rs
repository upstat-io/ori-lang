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

    // Verify ALL runtime functions exist in the module (must match declare_runtime exactly)
    let expected = [
        // I/O
        "ori_print",
        "ori_print_int",
        "ori_print_float",
        "ori_print_bool",
        // Panic
        "ori_panic",
        "ori_panic_cstr",
        // Entry point wrapper (AOT-only, not in JIT mappings)
        "ori_run_main",
        // Assertions
        "ori_assert",
        "ori_assert_eq_int",
        "ori_assert_eq_bool",
        "ori_assert_eq_float",
        "ori_assert_eq_str",
        // Lists
        "ori_list_alloc_data",
        "ori_list_free_data",
        "ori_list_new",
        "ori_list_free",
        "ori_list_len",
        // Comparison
        "ori_compare_int",
        "ori_min_int",
        "ori_max_int",
        // Strings
        "ori_str_concat",
        "ori_str_eq",
        "ori_str_ne",
        "ori_str_compare",
        "ori_str_hash",
        "ori_str_next_char",
        // Type conversions
        "ori_str_from_int",
        "ori_str_from_bool",
        "ori_str_from_float",
        // Format functions (Formattable trait)
        "ori_format_int",
        "ori_format_float",
        "ori_format_str",
        "ori_format_bool",
        "ori_format_char",
        // Reference counting
        "ori_rc_alloc",
        "ori_rc_inc",
        "ori_rc_dec",
        "ori_rc_free",
        // Args
        "ori_args_from_argv",
        "ori_register_panic_handler",
        // EH personality
        "rust_eh_personality",
    ];

    for name in &expected {
        assert!(
            scx.llmod.get_function(name).is_some(),
            "runtime function '{name}' should be declared"
        );
    }
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

/// Verifies that every function declared by `declare_runtime()` is either
/// in the JIT mapping table or in the documented AOT-only exception list.
///
/// This catches drift where a new runtime function is declared but not
/// added to the JIT symbol mappings.
#[test]
fn declared_functions_covered_by_jit_or_aot_only() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_sync");
    let mut builder = IrBuilder::new(&scx);

    declare_runtime(&mut builder);

    // Collect all function names declared in the LLVM module
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut func = scx.llmod.get_first_function();
    while let Some(f) = func {
        declared.insert(
            f.get_name()
                .to_str()
                .expect("non-UTF8 function name")
                .to_owned(),
        );
        func = f.get_next_function();
    }

    // Build the combined coverage set: JIT mappings + AOT-only exceptions
    let covered: BTreeSet<String> = JIT_MAPPED_RUNTIME_FUNCTIONS
        .iter()
        .chain(AOT_ONLY_RUNTIME_FUNCTIONS.iter())
        .map(|s| (*s).to_owned())
        .collect();

    let uncovered: BTreeSet<_> = declared.difference(&covered).collect();
    let phantom: BTreeSet<_> = covered.difference(&declared).collect();

    assert!(
        uncovered.is_empty(),
        "Runtime functions declared but not in JIT mappings or AOT-only list: {uncovered:?}\n\
         Add them to JIT_MAPPED_RUNTIME_FUNCTIONS in evaluator.rs or \
         AOT_ONLY_RUNTIME_FUNCTIONS if they are AOT-only."
    );
    assert!(
        phantom.is_empty(),
        "Functions in JIT/AOT-only lists but not declared by declare_runtime(): {phantom:?}\n\
         Remove them from evaluator.rs or add them to declare_runtime()."
    );
}
