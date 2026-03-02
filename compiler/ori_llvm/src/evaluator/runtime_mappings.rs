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

use crate::codegen::runtime_decl::runtime_functions;
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

        assert_eq!(
            mappings.len(),
            runtime_functions::jit_allowed_names().count(),
            "jit_symbol_mappings() and RT_FUNCTIONS jit_allowed entries have different counts"
        );

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
/// Each entry maps a runtime function name to its native function pointer address.
/// The set of entries must match `RT_FUNCTIONS` entries with `jit_allowed: true`.
pub(crate) fn jit_symbol_mappings() -> Vec<(&'static str, usize)> {
    vec![
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
        ("ori_list_get", runtime::ori_list_get as *const () as usize),
        ("ori_list_len", runtime::ori_list_len as *const () as usize),
        (
            "ori_list_rc_inc",
            runtime::ori_list_rc_inc as *const () as usize,
        ),
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
        ("ori_str_len", runtime::ori_str_len as *const () as usize),
        ("ori_str_data", runtime::ori_str_data as *const () as usize),
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
            "ori_buffer_rc_dec",
            runtime::ori_buffer_rc_dec as *const () as usize,
        ),
        (
            "ori_buffer_drop_unique",
            runtime::ori_buffer_drop_unique as *const () as usize,
        ),
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
    ]
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
