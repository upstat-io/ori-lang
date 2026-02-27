//! Runtime function and type mappings for LLVM JIT evaluation.
//!
//! Maps Ori runtime function names to their native function pointers
//! and Ori types to LLVM type representations for JIT execution.

use inkwell::execution_engine::ExecutionEngine;

use super::LLVMEvalError;
use crate::runtime;

/// Runtime functions declared in `runtime_decl` that are intentionally NOT
/// in the JIT mapping table. These are only used in AOT compilation.
#[cfg(test)]
pub(crate) const AOT_ONLY_RUNTIME_FUNCTIONS: &[&str] = &[
    // Iterator runtime — AOT uses opaque handles; JIT uses native IteratorValue
    "ori_iter_all",
    "ori_iter_any",
    "ori_iter_chain",
    "ori_iter_collect",
    "ori_iter_count",
    "ori_iter_drop",
    "ori_iter_enumerate",
    "ori_iter_filter",
    "ori_iter_find",
    "ori_iter_fold",
    "ori_iter_for_each",
    "ori_iter_from_list",
    "ori_iter_from_map",
    "ori_iter_from_range",
    "ori_iter_from_str",
    "ori_iter_map",
    "ori_iter_next",
    "ori_iter_skip",
    "ori_iter_take",
    "ori_iter_zip",
    // List building — AOT for-yield uses heap OriList; JIT uses native Vec
    "ori_list_push",
    "ori_list_take",
    // List methods — AOT uses runtime calls; JIT uses native Rust dispatch
    "ori_list_concat",
    "ori_list_concat_cow",
    "ori_list_insert_cow",
    "ori_list_pop_cow",
    "ori_list_push_cow",
    "ori_list_remove_cow",
    "ori_list_reverse_cow",
    "ori_list_set_cow",
    "ori_list_sort_cow",
    "ori_list_push_new",
    "ori_list_first",
    "ori_list_last",
    "ori_list_contains_int",
    "ori_list_contains_str",
    "ori_list_reverse",
    // Map methods — AOT uses runtime calls; JIT uses native Rust dispatch
    "ori_map_contains_key",
    "ori_map_get",
    "ori_map_insert",
    "ori_map_keys_to_list",
    "ori_map_remove",
    "ori_map_values_to_list",
    // Set methods — AOT uses runtime calls; JIT uses native Rust dispatch
    "ori_set_contains",
    "ori_set_difference",
    "ori_set_insert",
    "ori_set_intersection",
    "ori_set_remove",
    "ori_set_to_list",
    "ori_set_union",
    // String methods — AOT uses runtime calls; JIT uses native Rust dispatch
    "ori_str_contains",
    "ori_str_ends_with",
    "ori_str_repeat",
    "ori_str_replace",
    "ori_str_starts_with",
    "ori_str_chars",
    "ori_str_split",
    "ori_str_to_lowercase",
    "ori_str_to_uppercase",
    "ori_str_trim",
    // Boxed list construction — AOT wraps list structs in RC boxes; JIT uses native Vecs
    "ori_list_box_new",
    // COW primitives — AOT uses runtime calls; JIT uses native Rust dispatch
    "ori_list_empty",
    "ori_list_ensure_capacity",
    "ori_map_empty",
    "ori_memcpy_elements",
    "ori_memmove_elements",
    "ori_rc_is_unique",
    "ori_rc_is_unique_or_null",
    "ori_rc_realloc",
    "ori_set_empty",
    "ori_str_empty",
    // RC leak detection — AOT test infrastructure only
    "ori_rc_live_count",
    "ori_rc_reset_live_count",
    // ori_run_main wraps @main with catch_unwind — JIT compiles tests directly
    "ori_run_main",
    // catch(expr:) — AOT uses invoke/landingpad; JIT catches via ControlAction::Error
    "ori_catch_cleanup",
    "ori_catch_recover",
];

/// Names of all runtime functions registered in the JIT mapping table.
///
/// Used by sync tests to verify declarations and JIT mappings stay aligned.
pub(crate) const JIT_MAPPED_RUNTIME_FUNCTIONS: &[&str] = &[
    "ori_print",
    "ori_print_int",
    "ori_print_float",
    "ori_print_bool",
    "ori_panic",
    "ori_panic_cstr",
    "ori_assert",
    "ori_assert_eq_int",
    "ori_assert_eq_bool",
    "ori_assert_eq_float",
    "ori_list_alloc_data",
    "ori_list_free_data",
    "ori_list_new",
    "ori_list_free",
    "ori_list_len",
    "ori_compare_int",
    "ori_min_int",
    "ori_max_int",
    "ori_str_concat",
    "ori_str_eq",
    "ori_str_ne",
    "ori_str_compare",
    "ori_str_hash",
    "ori_str_next_char",
    "ori_assert_eq_str",
    "ori_str_from_raw",
    "ori_str_from_int",
    "ori_str_from_bool",
    "ori_str_from_float",
    "ori_format_int",
    "ori_format_float",
    "ori_format_str",
    "ori_format_bool",
    "ori_format_char",
    "ori_rc_alloc",
    "ori_rc_inc",
    "ori_rc_dec",
    "ori_rc_free",
    "ori_args_from_argv",
    "ori_register_panic_handler",
    "rust_eh_personality",
];

/// Add runtime function mappings to an execution engine.
///
/// Maps declared function names to actual Rust function addresses so the
/// JIT engine can resolve calls to runtime functions.
pub(super) fn add_runtime_mappings_to_engine(
    engine: &ExecutionEngine<'_>,
    module: &inkwell::module::Module<'_>,
) -> Result<(), LLVMEvalError> {
    let mappings: &[(&str, usize)] = &[
        ("ori_print", runtime::ori_print as *const () as usize),
        (
            "ori_print_int",
            runtime::ori_print_int as *const () as usize,
        ),
        (
            "ori_print_float",
            runtime::ori_print_float as *const () as usize,
        ),
        (
            "ori_print_bool",
            runtime::ori_print_bool as *const () as usize,
        ),
        ("ori_panic", runtime::ori_panic as *const () as usize),
        (
            "ori_panic_cstr",
            runtime::ori_panic_cstr as *const () as usize,
        ),
        ("ori_assert", runtime::ori_assert as *const () as usize),
        (
            "ori_assert_eq_int",
            runtime::ori_assert_eq_int as *const () as usize,
        ),
        (
            "ori_assert_eq_bool",
            runtime::ori_assert_eq_bool as *const () as usize,
        ),
        (
            "ori_assert_eq_float",
            runtime::ori_assert_eq_float as *const () as usize,
        ),
        (
            "ori_list_alloc_data",
            runtime::ori_list_alloc_data as *const () as usize,
        ),
        (
            "ori_list_free_data",
            runtime::ori_list_free_data as *const () as usize,
        ),
        ("ori_list_new", runtime::ori_list_new as *const () as usize),
        (
            "ori_list_free",
            runtime::ori_list_free as *const () as usize,
        ),
        ("ori_list_len", runtime::ori_list_len as *const () as usize),
        (
            "ori_compare_int",
            runtime::ori_compare_int as *const () as usize,
        ),
        ("ori_min_int", runtime::ori_min_int as *const () as usize),
        ("ori_max_int", runtime::ori_max_int as *const () as usize),
        (
            "ori_str_concat",
            runtime::ori_str_concat as *const () as usize,
        ),
        ("ori_str_eq", runtime::ori_str_eq as *const () as usize),
        ("ori_str_ne", runtime::ori_str_ne as *const () as usize),
        (
            "ori_str_compare",
            runtime::ori_str_compare as *const () as usize,
        ),
        ("ori_str_hash", runtime::ori_str_hash as *const () as usize),
        (
            "ori_str_next_char",
            runtime::ori_str_next_char as *const () as usize,
        ),
        (
            "ori_assert_eq_str",
            runtime::ori_assert_eq_str as *const () as usize,
        ),
        (
            "ori_str_from_raw",
            runtime::ori_str_from_raw as *const () as usize,
        ),
        (
            "ori_str_from_int",
            runtime::ori_str_from_int as *const () as usize,
        ),
        (
            "ori_str_from_bool",
            runtime::ori_str_from_bool as *const () as usize,
        ),
        (
            "ori_str_from_float",
            runtime::ori_str_from_float as *const () as usize,
        ),
        // Format functions (§3.16 Formattable trait)
        (
            "ori_format_int",
            runtime::format::ori_format_int as *const () as usize,
        ),
        (
            "ori_format_float",
            runtime::format::ori_format_float as *const () as usize,
        ),
        (
            "ori_format_str",
            runtime::format::ori_format_str as *const () as usize,
        ),
        (
            "ori_format_bool",
            runtime::format::ori_format_bool as *const () as usize,
        ),
        (
            "ori_format_char",
            runtime::format::ori_format_char as *const () as usize,
        ),
        ("ori_rc_alloc", runtime::ori_rc_alloc as *const () as usize),
        ("ori_rc_inc", runtime::ori_rc_inc as *const () as usize),
        ("ori_rc_dec", runtime::ori_rc_dec as *const () as usize),
        ("ori_rc_free", runtime::ori_rc_free as *const () as usize),
        (
            "ori_args_from_argv",
            runtime::ori_args_from_argv as *const () as usize,
        ),
        (
            "ori_register_panic_handler",
            runtime::ori_register_panic_handler as *const () as usize,
        ),
        // Exception handling personality function — required by any function
        // containing `invoke`/`landingpad`. Not in the dynamic symbol table,
        // so MCJIT's dlsym-based resolution can't find it automatically.
        ("rust_eh_personality", rust_eh_personality_addr()),
    ];

    // Verify the mapping array stays in sync with JIT_MAPPED_RUNTIME_FUNCTIONS.
    debug_assert_eq!(
        mappings.len(),
        JIT_MAPPED_RUNTIME_FUNCTIONS.len(),
        "JIT mapping array and JIT_MAPPED_RUNTIME_FUNCTIONS constant have different lengths"
    );

    for &(name, addr) in mappings {
        if let Some(func) = module.get_function(name) {
            engine.add_global_mapping(&func, addr);
        }
        // Silently skip functions not declared in this module — they may not
        // be needed if no code calls them.
    }

    Ok(())
}

/// Get the address of `rust_eh_personality` for JIT symbol mapping.
///
/// This function is defined in the Rust standard library and handles
/// DWARF-based exception handling (Itanium ABI). It's present in the
/// host binary but not exported in the dynamic symbol table, so the
/// LLVM MCJIT can't resolve it via `dlsym`. We provide it explicitly.
fn rust_eh_personality_addr() -> usize {
    extern "C" {
        fn rust_eh_personality();
    }
    rust_eh_personality as *const () as usize
}
