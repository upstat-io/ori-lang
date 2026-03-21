//! Runtime function declaration table.
//!
//! Single source of truth for all runtime functions declared by the Ori compiler.
//! Each entry specifies the function's name, parameter types, return type,
//! and any LLVM attributes.
//!
//! This file is exempt from the 500-line limit: it is a pure static data table
//! (no logic, no branching). Splitting would scatter the single source of truth
//! across files, making audits harder and increasing sync risk.
//!
//! Type definitions (`Ty`, `Attr`, `RtFn`) live in the sibling `types` module.

use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) use super::types::{Attr, RtFn, Ty};

/// O(1) lookup index for runtime functions by name.
static RT_INDEX: LazyLock<HashMap<&'static str, &'static RtFn>> =
    LazyLock::new(|| RT_FUNCTIONS.iter().map(|f| (f.name, f)).collect());

/// Look up a runtime function spec by name (O(1)).
pub(crate) fn lookup(name: &str) -> Option<&'static RtFn> {
    RT_INDEX.get(name).copied()
}

/// Check whether a runtime function is allowed in JIT mode.
pub(crate) fn is_jit_allowed(name: &str) -> bool {
    lookup(name).is_some_and(|f| f.jit_allowed)
}

/// Check whether a runtime function is known to be nounwind.
///
/// Returns `Some(true)` if the function has `Attr::Nounwind` (provably never
/// unwinds), `Some(false)` if it is a known runtime function WITHOUT nounwind
/// (may call `ori_panic` internally), or `None` if the name is not a runtime
/// function at all.
///
/// Used by `is_arc_function_nounwind` to determine whether calling a runtime
/// function preserves the nounwind guarantee. Runtime functions without the
/// `Nounwind` attribute may panic (e.g., `ori_list_get` on OOB, `ori_assert`
/// on failure, allocating functions on OOM).
pub(crate) fn is_rt_fn_nounwind(name: &str) -> Option<bool> {
    lookup(name).map(|spec| spec.attrs.iter().any(|a| matches!(a, Attr::Nounwind)))
}

/// Check whether a runtime function is known to be noreturn.
///
/// Returns `Some(true)` if the function has `Attr::Noreturn` (never returns
/// to its caller), `Some(false)` if it is a known runtime function WITHOUT
/// noreturn, or `None` if the name is not a runtime function at all.
///
/// Used by §06.2 to skip codegen after noreturn calls.
pub(crate) fn is_rt_fn_noreturn(name: &str) -> Option<bool> {
    lookup(name).map(|spec| spec.attrs.iter().any(|a| matches!(a, Attr::Noreturn)))
}

/// Iterate over names of all JIT-allowed runtime functions.
#[cfg(test)]
pub(crate) fn jit_allowed_names() -> impl Iterator<Item = &'static str> {
    RT_FUNCTIONS
        .iter()
        .filter(|f| f.jit_allowed)
        .map(|f| f.name)
}

// Runtime function table (single source of truth)

/// All runtime functions declared by the Ori compiler.
///
/// Each entry specifies the function's name, parameter types, return type,
/// and any LLVM attributes. This table is the single source of truth —
/// both `declare_single()` and `declare_runtime()` use it.
pub(crate) static RT_FUNCTIONS: &[RtFn] = &[
    // I/O — extern "C" (panics abort at ABI boundary, never unwind)
    RtFn {
        name: "ori_print",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_print_int",
        params: &[Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_print_float",
        params: &[Ty::F64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_print_bool",
        params: &[Ty::Bool],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Panic (cold + noreturn, NOT nounwind — must unwind for RC cleanup)
    RtFn {
        name: "ori_panic",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Cold, Attr::Noreturn],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_panic_cstr",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Cold, Attr::Noreturn],
        jit_allowed: true,
    },
    // Entry point wrapper (AOT-only — JIT runs functions directly).
    // Nounwind: catches all panics internally via catch_unwind (Itanium) or
    // __try/__except SEH (MSVC). Never unwinds to its caller.
    RtFn {
        name: "ori_run_main",
        params: &[Ty::Ptr],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // Leak check for AOT main wrapper. Reads ORI_CHECK_LEAKS env var,
    // returns 0 if clean or disabled, 2 if leaks detected.
    RtFn {
        name: "ori_check_leaks",
        params: &[],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // Element header helpers — store elem_dec_fn and elem_count in RC header
    // at collection construction time (Section 02 of rc-header-elem-dec plan).
    RtFn {
        name: "ori_buffer_store_elem_dec",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_buffer_store_elem_count",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Assertions — extern "C-unwind" (call ori_panic on failure, must unwind)
    RtFn {
        name: "ori_assert",
        params: &[Ty::Bool],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_int",
        params: &[Ty::I64, Ty::I64],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_bool",
        params: &[Ty::Bool, Ty::Bool],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_float",
        params: &[Ty::F64, Ty::F64],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_str",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    // List — extern "C" unless noted
    RtFn {
        name: "ori_list_alloc_data",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_free_data",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_new",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_free",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_len",
        params: &[Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_push",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_take",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_box_new",
        params: &[Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_push_new",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_push_cow",
        // data, len, cap, elem_ptr, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_pop_cow",
        // data, len, cap, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_set_cow",
        // data, len, cap, index, elem_ptr, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_insert_cow",
        // data, len, cap, index, elem_ptr, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_remove_cow",
        // data, len, cap, index, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_concat_cow",
        // data1, len1, cap1, data2, len2, cap2, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_reverse_cow",
        // data, len, cap, elem_size, elem_align, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_sort_cow",
        // data, len, cap, elem_size, elem_align, compare_fn, inc_fn, cow_mode, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_sort_stable_cow",
        // Same signature as ori_list_sort_cow (stable TimSort variant, with cow_mode)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_first",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_last",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // extern "C-unwind" — panics on out-of-bounds access
    RtFn {
        name: "ori_list_get",
        // (data, len, index, elem_size, out_ptr) — panics on OOB
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_contains_int",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_contains_str",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_concat",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_reverse",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Map literal construction — hash table allocation + per-entry insert
    RtFn {
        name: "ori_map_literal_alloc",
        // (count, key_size, val_size, out_cap) -> ptr
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_literal_put",
        // (data, cap, key, val, key_size, val_size, key_hash)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Map — hash table layout: data = [metadata|keys|values]
    RtFn {
        name: "ori_map_contains_key",
        // (data, cap, len, needle, key_size, key_eq, key_hash) -> i64
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_keys_to_list",
        // (data, cap, len, key_size, key_dec_fn, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_values_to_list",
        // (data, cap, len, key_size, val_size, val_dec_fn, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_get",
        // (data, cap, len, needle, key_size, val_size, key_eq, key_hash, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_insert_cow",
        // (data, len, cap, key, value, key_size, val_size, key_eq, key_hash, key_inc, val_inc, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_remove_cow",
        // (data, len, cap, key, key_size, val_size, key_eq, key_hash, key_inc, val_inc, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_buffer_rc_dec",
        // (data, cap, len, key_size, val_size, key_dec_fn, val_dec_fn)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Set — all extern "C"
    RtFn {
        name: "ori_set_contains",
        // (data, cap, len, needle, elem_size, elem_eq, elem_hash) -> i64
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_insert_cow",
        // (data, len, cap, elem, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_remove_cow",
        // (data, len, cap, elem, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_union_cow",
        // (d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_intersection_cow",
        // (d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_difference_cow",
        // (d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_to_list",
        // (data, cap, len, elem_size, elem_dec_fn, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Set literal construction — hash table allocation + per-entry insert
    RtFn {
        name: "ori_set_literal_alloc",
        // (count, elem_size, out_cap) -> ptr
        params: &[Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_literal_put",
        // (data, cap, elem, elem_size, elem_hash)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Set buffer RC cleanup (hash table layout — cap and len swapped vs list)
    RtFn {
        name: "ori_set_buffer_rc_dec",
        // (data, cap, len, elem_size, elem_dec_fn)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_buffer_drop_unique",
        // (data, cap, len, elem_size, elem_dec_fn)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // String iteration — extern "C"
    RtFn {
        name: "ori_str_chars",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_split",
        // (str_ptr, str_len, sep_ptr, sep_len, elem_dec_fn, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Comparison — pure functions, cannot panic
    RtFn {
        name: "ori_compare_int",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_min_int",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_max_int",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // String operations — all extern "C"
    RtFn {
        name: "ori_str_concat",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_eq",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_eq_scalar",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // ori_list_eq_deep(a: ptr, b: ptr, elem_size, elem_eq) -> bool
    RtFn {
        name: "ori_list_eq_deep",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // ori_map_eq(a: ptr, b: ptr, key_size, val_size, key_eq, key_hash, val_eq) -> bool
    RtFn {
        name: "ori_map_eq",
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    RtFn {
        name: "ori_str_ne",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_compare",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_hash",
        params: &[Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // String property access (SSO-safe, cannot panic)
    RtFn {
        name: "ori_str_len",
        params: &[Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_data",
        params: &[Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // String iteration (char-by-char, pure read)
    RtFn {
        name: "ori_str_next_char",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::CharResult),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Type conversion — all extern "C"
    RtFn {
        name: "ori_str_from_raw",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_from_int",
        params: &[Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_from_bool",
        params: &[Ty::Bool],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_from_float",
        params: &[Ty::F64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // String methods — all extern "C"
    RtFn {
        name: "ori_str_contains",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_starts_with",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_ends_with",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_trim",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_to_uppercase",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_to_lowercase",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_replace",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_repeat",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Format (section 3.16 Formattable trait) — all extern "C"
    RtFn {
        name: "ori_format_int",
        params: &[Ty::I64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_float",
        params: &[Ty::F64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_str",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_bool",
        params: &[Ty::Bool, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_char",
        params: &[Ty::I32, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Slice operations
    RtFn {
        name: "ori_list_slice",
        // data, len, cap, start, end, elem_size, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_slice_take",
        // data, len, cap, n, elem_size, out_ptr
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_slice_drop",
        // data, len, cap, n, elem_size, out_ptr
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_materialize_slice",
        // data, len, cap, elem_size, elem_align, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_substring",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Reference counting (ARC-safe attributes are CRITICAL — see section 11.3)
    RtFn {
        name: "ori_rc_alloc",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind, Attr::NoaliasReturn],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_inc",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_dec",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_free",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Slice-aware RC inc for list/set data buffers.
    // If cap has slice flag, finds original buffer and inc's that.
    RtFn {
        name: "ori_list_rc_inc",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Buffer-aware RC dec for collection data buffers.
    // (data, len, cap, elem_size, elem_dec_fn)
    RtFn {
        name: "ori_buffer_rc_dec",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Unique-drop for list/set buffers: skip atomic RC dec, directly
    // clean elements + free. Same signature as ori_buffer_rc_dec.
    RtFn {
        name: "ori_buffer_drop_unique",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Unique-drop for map buffers: skip atomic RC dec, directly
    // clean keys + values + free.
    // (data, cap, len, key_size, val_size, key_dec_fn, val_dec_fn)
    RtFn {
        name: "ori_map_buffer_drop_unique",
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Collection buffer reuse: reset a list/set buffer for new elements.
    // Checks uniqueness internally — if unique, reuses buffer; if shared,
    // decs old and allocates fresh.
    // (old_data, old_len, old_cap, new_len, elem_size, elem_dec_fn, out_cap) -> ptr
    RtFn {
        name: "ori_list_reset_buffer",
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_live_count",
        params: &[],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_reset_live_count",
        params: &[],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // COW primitives
    RtFn {
        name: "ori_rc_is_unique",
        params: &[Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_is_unique_or_null",
        params: &[Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_realloc",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_memcpy_elements",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_memmove_elements",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_ensure_capacity",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Empty collection sentinels
    RtFn {
        name: "ori_list_empty",
        params: &[],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_empty",
        params: &[],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_empty",
        params: &[],
        ret: Some(Ty::Map),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_empty",
        params: &[],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Args conversion — extern "C"
    RtFn {
        name: "ori_args_from_argv",
        params: &[Ty::I32, Ty::Ptr],
        ret: Some(Ty::List),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator constructors — all extern "C"
    RtFn {
        name: "ori_iter_from_list",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        //        data   len   cap   es     elem_dec_fn
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_range",
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Bool],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_str",
        params: &[Ty::Ptr],
        //        *const OriStr (SSO-safe)
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_map",
        // (data, cap, len, key_size, val_size, owns_data, key_dec_fn, val_dec_fn)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Bool,
            Ty::Ptr,
            Ty::Ptr,
        ],
        //        data   cap    len   ks     vs     owns_data  k_dec  v_dec
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator next — extern "C" (callbacks called inside, panics abort at boundary)
    RtFn {
        name: "ori_iter_next",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator adapters — extern "C" (store callback, don't call it)
    RtFn {
        name: "ori_iter_map",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_filter",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_take",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_skip",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_enumerate",
        params: &[Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_zip",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_chain",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator consumers — extern "C" (call callbacks internally, panics abort at boundary)
    RtFn {
        name: "ori_iter_collect",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_collect_set",
        // (iter, elem_size, elem_eq, elem_hash, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_count",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_any",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_all",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_find",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_for_each",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_fold",
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator cleanup — extern "C"
    RtFn {
        name: "ori_iter_drop",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Panic handler registration — extern "C" (stores function pointer, no panic)
    RtFn {
        name: "ori_register_panic_handler",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Catch recovery — extern "C" (called after unwinding completes, no panic)
    RtFn {
        name: "ori_catch_cleanup",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_catch_recover",
        params: &[],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // SEH/MSVC catch trampoline — wraps a function call in __try/__except.
    // extern "C" (catches exceptions internally, returns status code)
    RtFn {
        name: "ori_try_call",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // EH personality (Itanium ABI — required by invoke/landingpad)
    // Implemented in ori_rt/src/eh_personality.c
    RtFn {
        name: "ori_eh_personality",
        params: &[Ty::I32],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // EH personality (Windows SEH — required by invoke/catchswitch/catchpad)
    // AOT-only: JIT uses Itanium EH with ori_eh_personality.
    RtFn {
        name: "__CxxFrameHandler3",
        params: &[],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
];
