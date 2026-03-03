//! Runtime function and type mappings for LLVM JIT evaluation.
//!
//! Maps Ori runtime function names to their native function pointers
//! so the MCJIT engine can resolve calls to runtime functions.
//!
//! # Symbol Resolution in MCJIT
//!
//! MCJIT resolves external symbols via `DynamicLibrary::SearchForAddressOfSymbol`,
//! NOT via `ExecutionEngine::GlobalAddressMap` (which `addGlobalMapping` populates).
//! Since `ori_rt` is linked statically, runtime function symbols are not in the
//! process's dynamic symbol table. We use `LLVMAddSymbol` to register them in
//! LLVM's internal symbol search path, which MCJIT's `RuntimeDyld` does check.

use std::ffi::CString;
use std::sync::Once;

use crate::codegen::runtime_decl::runtime_functions::RT_FUNCTIONS;
use crate::runtime;

/// Register all runtime function symbols in LLVM's dynamic library.
///
/// Uses `LLVMAddSymbol` to make runtime functions discoverable by MCJIT's
/// `RuntimeDyld` during relocation resolution. Called once per process via
/// `Once` guard — symbols persist for the process lifetime.
///
/// This replaces the previous `addGlobalMapping` approach, which populated
/// `ExecutionEngine::GlobalAddressMap` — a map that MCJIT's `RuntimeDyld`
/// does NOT check during symbol resolution (it uses `DynamicLibrary` instead).
pub(super) fn ensure_runtime_symbols_registered() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mappings = jit_symbol_mappings();

        for &(name, addr) in &mappings {
            let c_name = CString::new(name).expect("runtime function name contains null byte");
            // SAFETY: addr is a valid function pointer obtained from `fn as *const ()`.
            // LLVMAddSymbol stores the name→address mapping in LLVM's internal
            // DynamicLibrary, which persists for the process lifetime.
            unsafe {
                llvm_sys::support::LLVMAddSymbol(c_name.as_ptr(), addr as *mut std::ffi::c_void);
            }
            tracing::trace!(name, addr, "LLVMAddSymbol");
        }

        tracing::debug!(count = mappings.len(), "registered JIT runtime symbols");
    });
}

/// Build the (name, address) mapping table for JIT runtime symbols.
///
/// Generated from `RT_FUNCTIONS` entries with `jit_allowed: true`.
/// Each entry's address is resolved via [`lookup_jit_address`], which
/// panics on unrecognized names — catching drift at startup rather than
/// at the point of a JIT call.
pub(crate) fn jit_symbol_mappings() -> Vec<(&'static str, usize)> {
    RT_FUNCTIONS
        .iter()
        .filter(|f| f.jit_allowed)
        .map(|f| (f.name, lookup_jit_address(f.name)))
        .collect()
}

/// Resolve a runtime function name to its native address.
///
/// This is the single point where function names are mapped to Rust function
/// pointers. Adding a new `jit_allowed: true` entry to `RT_FUNCTIONS` requires
/// adding a corresponding arm here — the `_` arm panics to catch omissions.
fn lookup_jit_address(name: &str) -> usize {
    match name {
        // Print
        "ori_print" => runtime::ori_print as usize,
        "ori_print_int" => runtime::ori_print_int as usize,
        "ori_print_float" => runtime::ori_print_float as usize,
        "ori_print_bool" => runtime::ori_print_bool as usize,
        // Panic / assert
        "ori_panic" => runtime::ori_panic as usize,
        "ori_panic_cstr" => runtime::ori_panic_cstr as usize,
        "ori_assert" => runtime::ori_assert as usize,
        "ori_assert_eq_int" => runtime::ori_assert_eq_int as usize,
        "ori_assert_eq_bool" => runtime::ori_assert_eq_bool as usize,
        "ori_assert_eq_float" => runtime::ori_assert_eq_float as usize,
        "ori_assert_eq_str" => runtime::ori_assert_eq_str as usize,
        // List core
        "ori_list_alloc_data" => runtime::ori_list_alloc_data as usize,
        "ori_list_free_data" => runtime::ori_list_free_data as usize,
        "ori_list_new" => runtime::ori_list_new as usize,
        "ori_list_free" => runtime::ori_list_free as usize,
        "ori_list_get" => runtime::ori_list_get as usize,
        "ori_list_len" => runtime::ori_list_len as usize,
        "ori_list_rc_inc" => runtime::ori_list_rc_inc as usize,
        // List mutation
        "ori_list_push" => runtime::ori_list_push as usize,
        "ori_list_take" => runtime::ori_list_take as usize,
        "ori_list_box_new" => runtime::ori_list_box_new as usize,
        "ori_list_push_new" => runtime::ori_list_push_new as usize,
        // List COW
        "ori_list_push_cow" => runtime::ori_list_push_cow as usize,
        "ori_list_pop_cow" => runtime::ori_list_pop_cow as usize,
        "ori_list_set_cow" => runtime::ori_list_set_cow as usize,
        "ori_list_insert_cow" => runtime::ori_list_insert_cow as usize,
        "ori_list_remove_cow" => runtime::ori_list_remove_cow as usize,
        "ori_list_reverse_cow" => runtime::ori_list_reverse_cow as usize,
        "ori_list_sort_cow" => runtime::ori_list_sort_cow as usize,
        "ori_list_sort_stable_cow" => runtime::ori_list_sort_stable_cow as usize,
        "ori_list_concat_cow" => runtime::ori_list_concat_cow as usize,
        // List query
        "ori_list_first" => runtime::ori_list_first as usize,
        "ori_list_last" => runtime::ori_list_last as usize,
        "ori_list_contains_int" => runtime::ori_list_contains_int as usize,
        "ori_list_contains_str" => runtime::ori_list_contains_str as usize,
        "ori_list_concat" => runtime::ori_list_concat as usize,
        "ori_list_reverse" => runtime::ori_list_reverse as usize,
        // Slice operations
        "ori_list_slice" => runtime::ori_list_slice as usize,
        "ori_list_slice_take" => runtime::ori_list_slice_take as usize,
        "ori_list_slice_drop" => runtime::ori_list_slice_drop as usize,
        "ori_list_materialize_slice" => runtime::ori_list_materialize_slice as usize,
        // Comparison
        "ori_compare_int" => runtime::ori_compare_int as usize,
        "ori_min_int" => runtime::ori_min_int as usize,
        "ori_max_int" => runtime::ori_max_int as usize,
        // String core
        "ori_str_concat" => runtime::ori_str_concat as usize,
        "ori_str_eq" => runtime::ori_str_eq as usize,
        "ori_str_ne" => runtime::ori_str_ne as usize,
        "ori_str_compare" => runtime::ori_str_compare as usize,
        "ori_str_hash" => runtime::ori_str_hash as usize,
        "ori_str_len" => runtime::ori_str_len as usize,
        "ori_str_data" => runtime::ori_str_data as usize,
        "ori_str_next_char" => runtime::ori_str_next_char as usize,
        "ori_str_from_raw" => runtime::ori_str_from_raw as usize,
        "ori_str_from_int" => runtime::ori_str_from_int as usize,
        "ori_str_from_bool" => runtime::ori_str_from_bool as usize,
        "ori_str_from_float" => runtime::ori_str_from_float as usize,
        // String methods
        "ori_str_chars" => runtime::ori_str_chars as usize,
        "ori_str_split" => runtime::ori_str_split as usize,
        "ori_str_contains" => runtime::ori_str_contains as usize,
        "ori_str_starts_with" => runtime::ori_str_starts_with as usize,
        "ori_str_ends_with" => runtime::ori_str_ends_with as usize,
        "ori_str_trim" => runtime::ori_str_trim as usize,
        "ori_str_to_uppercase" => runtime::ori_str_to_uppercase as usize,
        "ori_str_to_lowercase" => runtime::ori_str_to_lowercase as usize,
        "ori_str_replace" => runtime::ori_str_replace as usize,
        "ori_str_repeat" => runtime::ori_str_repeat as usize,
        "ori_str_substring" => runtime::ori_str_substring as usize,
        // Format (§3.16 Formattable trait)
        "ori_format_int" => runtime::format::ori_format_int as usize,
        "ori_format_float" => runtime::format::ori_format_float as usize,
        "ori_format_str" => runtime::format::ori_format_str as usize,
        "ori_format_bool" => runtime::format::ori_format_bool as usize,
        "ori_format_char" => runtime::format::ori_format_char as usize,
        // RC core
        "ori_rc_alloc" => runtime::ori_rc_alloc as usize,
        "ori_rc_inc" => runtime::ori_rc_inc as usize,
        "ori_rc_dec" => runtime::ori_rc_dec as usize,
        "ori_rc_free" => runtime::ori_rc_free as usize,
        "ori_buffer_rc_dec" => runtime::ori_buffer_rc_dec as usize,
        "ori_buffer_drop_unique" => runtime::ori_buffer_drop_unique as usize,
        // RC utilities
        "ori_rc_live_count" => runtime::ori_rc_live_count as usize,
        "ori_rc_reset_live_count" => runtime::ori_rc_reset_live_count as usize,
        "ori_rc_is_unique" => runtime::ori_rc_is_unique as usize,
        "ori_rc_is_unique_or_null" => runtime::ori_rc_is_unique_or_null as usize,
        "ori_rc_realloc" => runtime::ori_rc_realloc as usize,
        "ori_memcpy_elements" => runtime::ori_memcpy_elements as usize,
        "ori_memmove_elements" => runtime::ori_memmove_elements as usize,
        "ori_list_ensure_capacity" => runtime::ori_list_ensure_capacity as usize,
        // Args / panic handler
        "ori_args_from_argv" => runtime::ori_args_from_argv as usize,
        "ori_register_panic_handler" => runtime::ori_register_panic_handler as usize,
        // Map operations
        "ori_map_literal_alloc" => runtime::map::ori_map_literal_alloc as usize,
        "ori_map_literal_put" => runtime::map::ori_map_literal_put as usize,
        "ori_map_contains_key" => runtime::map::ori_map_contains_key as usize,
        "ori_map_keys_to_list" => runtime::map::ori_map_keys_to_list as usize,
        "ori_map_values_to_list" => runtime::map::ori_map_values_to_list as usize,
        "ori_map_get" => runtime::map::ori_map_get as usize,
        "ori_map_insert_cow" => runtime::map::cow::ori_map_insert_cow as usize,
        "ori_map_remove_cow" => runtime::map::cow::ori_map_remove_cow as usize,
        "ori_map_buffer_rc_dec" => runtime::ori_map_buffer_rc_dec as usize,
        "ori_map_buffer_drop_unique" => runtime::ori_map_buffer_drop_unique as usize,
        // Set operations
        "ori_set_contains" => runtime::set::ori_set_contains as usize,
        "ori_set_insert_cow" => runtime::set::cow::ori_set_insert_cow as usize,
        "ori_set_remove_cow" => runtime::set::cow::ori_set_remove_cow as usize,
        "ori_set_union_cow" => runtime::set::cow::ori_set_union_cow as usize,
        "ori_set_intersection_cow" => runtime::set::cow::ori_set_intersection_cow as usize,
        "ori_set_difference_cow" => runtime::set::cow::ori_set_difference_cow as usize,
        "ori_set_to_list" => runtime::set::ori_set_to_list as usize,
        "ori_set_literal_alloc" => runtime::set::ori_set_literal_alloc as usize,
        "ori_set_literal_put" => runtime::set::ori_set_literal_put as usize,
        "ori_set_buffer_rc_dec" => runtime::ori_set_buffer_rc_dec as usize,
        "ori_set_buffer_drop_unique" => runtime::ori_set_buffer_drop_unique as usize,
        // Iterator sources
        "ori_iter_from_list" => runtime::iterator::ori_iter_from_list as usize,
        "ori_iter_from_range" => runtime::iterator::ori_iter_from_range as usize,
        "ori_iter_from_str" => runtime::iterator::ori_iter_from_str as usize,
        "ori_iter_from_map" => runtime::iterator::ori_iter_from_map as usize,
        // Iterator core
        "ori_iter_next" => runtime::iterator::ori_iter_next as usize,
        "ori_iter_drop" => runtime::iterator::ori_iter_drop as usize,
        // Iterator adapters
        "ori_iter_map" => runtime::iterator::ori_iter_map as usize,
        "ori_iter_filter" => runtime::iterator::ori_iter_filter as usize,
        "ori_iter_take" => runtime::iterator::ori_iter_take as usize,
        "ori_iter_skip" => runtime::iterator::ori_iter_skip as usize,
        "ori_iter_enumerate" => runtime::iterator::ori_iter_enumerate as usize,
        "ori_iter_zip" => runtime::iterator::ori_iter_zip as usize,
        "ori_iter_chain" => runtime::iterator::ori_iter_chain as usize,
        // Iterator consumers
        "ori_iter_collect" => runtime::iterator::ori_iter_collect as usize,
        "ori_iter_collect_set" => runtime::iterator::ori_iter_collect_set as usize,
        "ori_iter_count" => runtime::iterator::ori_iter_count as usize,
        "ori_iter_any" => runtime::iterator::ori_iter_any as usize,
        "ori_iter_all" => runtime::iterator::ori_iter_all as usize,
        "ori_iter_find" => runtime::iterator::ori_iter_find as usize,
        "ori_iter_for_each" => runtime::iterator::ori_iter_for_each as usize,
        "ori_iter_fold" => runtime::iterator::ori_iter_fold as usize,
        // Empty collection sentinels
        "ori_list_empty" => runtime::ori_list_empty as usize,
        "ori_str_empty" => runtime::ori_str_empty as usize,
        "ori_map_empty" => runtime::map::ori_map_empty as usize,
        "ori_set_empty" => runtime::set::ori_set_empty as usize,
        // Buffer reuse
        "ori_list_reset_buffer" => runtime::ori_list_reset_buffer as usize,
        // Catch recovery (Itanium EH)
        "ori_catch_cleanup" => runtime::ori_catch_cleanup as usize,
        "ori_catch_recover" => runtime::ori_catch_recover as usize,
        // Exception handling personality
        "rust_eh_personality" => rust_eh_personality_addr(),
        // Catch-all: panics immediately to surface drift between
        // RT_FUNCTIONS and this lookup table.
        other => panic!(
            "unknown JIT function {other:?} — add it to lookup_jit_address() \
             in evaluator/runtime_mappings.rs"
        ),
    }
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
