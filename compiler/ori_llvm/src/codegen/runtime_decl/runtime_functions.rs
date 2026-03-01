//! Runtime function declaration table.
//!
//! Single source of truth for all runtime functions declared by the Ori compiler.
//! Each entry specifies the function's name, parameter types, return type,
//! and any LLVM attributes.
//!
//! This file is exempt from the 500-line limit: it is a pure static data table
//! (no logic, no branching). Splitting would scatter the single source of truth
//! across files, making audits harder and increasing sync risk.

// ---------------------------------------------------------------------------
// Type and attribute descriptors
// ---------------------------------------------------------------------------

/// Primitive type descriptor for runtime function signatures.
#[derive(Clone, Copy)]
pub(crate) enum Ty {
    I64,
    I32,
    I8,
    F64,
    Bool,
    Ptr,
    /// `{i64, i64, ptr}` — Ori string representation (len, cap, data).
    Str,
    /// `{i64, i64, ptr}` — Ori list/set representation.
    List,
    /// `{i64, i64, ptr}` — Ori map representation (len, cap, data).
    Map,
    /// `{i32, i64}` — char iteration result `{codepoint, next_offset}`.
    CharResult,
}

impl Ty {
    /// Whether this return type exceeds the x86-64 `SysV` ABI register return
    /// threshold (16 bytes) and must use `sret` convention.
    ///
    /// `Str` (24 bytes), `List` (24 bytes), and `Map` (24 bytes) all exceed it.
    /// `CharResult` (16 bytes) fits exactly and uses direct return.
    pub(crate) const fn needs_sret(self) -> bool {
        matches!(self, Self::Str | Self::List | Self::Map)
    }
}

/// Function attribute applied after declaration.
#[derive(Clone, Copy)]
pub(crate) enum Attr {
    Nounwind,
    Cold,
    NoaliasReturn,
    MemArgmemRW,
}

/// Runtime function specification: name, signature, and attributes.
pub(crate) struct RtFn {
    pub(crate) name: &'static str,
    pub(crate) params: &'static [Ty],
    pub(crate) ret: Option<Ty>,
    pub(crate) attrs: &'static [Attr],
}

// ---------------------------------------------------------------------------
// Runtime function table (single source of truth)
// ---------------------------------------------------------------------------

/// All runtime functions declared by the Ori compiler.
///
/// Each entry specifies the function's name, parameter types, return type,
/// and any LLVM attributes. This table is the single source of truth —
/// both `declare_single()` and `declare_runtime()` use it.
pub(crate) static RT_FUNCTIONS: &[RtFn] = &[
    // I/O
    RtFn {
        name: "ori_print",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_print_int",
        params: &[Ty::I64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_print_float",
        params: &[Ty::F64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_print_bool",
        params: &[Ty::Bool],
        ret: None,
        attrs: &[],
    },
    // Panic (cold, NOT nounwind — must unwind for RC cleanup)
    RtFn {
        name: "ori_panic",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Cold],
    },
    RtFn {
        name: "ori_panic_cstr",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Cold],
    },
    // Entry point wrapper
    RtFn {
        name: "ori_run_main",
        params: &[Ty::Ptr],
        ret: Some(Ty::I32),
        attrs: &[],
    },
    // Assertions
    RtFn {
        name: "ori_assert",
        params: &[Ty::Bool],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_assert_eq_int",
        params: &[Ty::I64, Ty::I64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_assert_eq_bool",
        params: &[Ty::Bool, Ty::Bool],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_assert_eq_float",
        params: &[Ty::F64, Ty::F64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_assert_eq_str",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    // List
    RtFn {
        name: "ori_list_alloc_data",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_list_free_data",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_new",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_list_free",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_len",
        params: &[Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_list_push",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_take",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_box_new",
        params: &[Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_list_push_new",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_push_cow",
        // data, len, cap, elem_ptr, elem_size, elem_align, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_pop_cow",
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
        attrs: &[],
    },
    RtFn {
        name: "ori_list_set_cow",
        // data, len, cap, index, elem_ptr, elem_size, elem_align, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_insert_cow",
        // data, len, cap, index, elem_ptr, elem_size, elem_align, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_remove_cow",
        // data, len, cap, index, elem_size, elem_align, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_concat_cow",
        // data1, len1, cap1, data2, len2, cap2, elem_size, elem_align, inc_fn, out_ptr
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
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_reverse_cow",
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
        attrs: &[],
    },
    RtFn {
        name: "ori_list_sort_cow",
        // data, len, cap, elem_size, elem_align, compare_fn, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_sort_stable_cow",
        // Same signature as ori_list_sort_cow (stable TimSort variant)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_first",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_last",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_contains_int",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_list_contains_str",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_list_concat",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_list_reverse",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    // Map — single-buffer layout: data = [keys...|vals...]
    RtFn {
        name: "ori_map_contains_key",
        // (data, len, needle, key_size, key_eq) -> i64
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_map_keys_to_list",
        // (data, len, key_size, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_values_to_list",
        // (data, cap, len, key_size, val_size, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_get",
        // (data, cap, len, needle, key_size, val_size, key_eq, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_insert_cow",
        // (data, len, cap, key, value, key_size, val_size, key_eq, key_inc, val_inc, out_ptr)
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
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_remove_cow",
        // (data, len, cap, key, key_size, val_size, key_eq, key_inc, val_inc, out_ptr)
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
        ],
        ret: None,
        attrs: &[],
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
        attrs: &[],
    },
    // Set
    RtFn {
        name: "ori_set_contains",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_set_insert",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_remove",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_union",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_intersection",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_difference",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_insert_cow",
        // (data, len, cap, elem, elem_size, elem_align, elem_eq, inc_fn, out_ptr)
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
        attrs: &[],
    },
    RtFn {
        name: "ori_set_remove_cow",
        // (data, len, cap, elem, elem_size, elem_align, elem_eq, inc_fn, out_ptr)
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
        attrs: &[],
    },
    RtFn {
        name: "ori_set_union_cow",
        // (d1, l1, c1, d2, l2, elem_size, elem_align, elem_eq, inc_fn, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_intersection_cow",
        // (d1, l1, c1, d2, l2, elem_size, elem_align, elem_eq, inc_fn, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_difference_cow",
        // (d1, l1, c1, d2, l2, elem_size, elem_align, elem_eq, inc_fn, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_set_to_list",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    // String iteration
    RtFn {
        name: "ori_str_chars",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_str_split",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    // Comparison
    RtFn {
        name: "ori_compare_int",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::I32),
        attrs: &[],
    },
    RtFn {
        name: "ori_min_int",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_max_int",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    // String operations
    RtFn {
        name: "ori_str_concat",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_eq",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_ne",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_compare",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::I8),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_hash",
        params: &[Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    // String property access (SSO-safe)
    RtFn {
        name: "ori_str_len",
        params: &[Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_data",
        params: &[Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    // String iteration (char-by-char)
    RtFn {
        name: "ori_str_next_char",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::CharResult),
        attrs: &[],
    },
    // Type conversion
    RtFn {
        name: "ori_str_from_raw",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_from_int",
        params: &[Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_from_bool",
        params: &[Ty::Bool],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_from_float",
        params: &[Ty::F64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    // String methods
    RtFn {
        name: "ori_str_contains",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_starts_with",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_ends_with",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_trim",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_to_uppercase",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_to_lowercase",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_replace",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_str_repeat",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    // Format (section 3.16 Formattable trait)
    RtFn {
        name: "ori_format_int",
        params: &[Ty::I64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_format_float",
        params: &[Ty::F64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_format_str",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_format_bool",
        params: &[Ty::Bool, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    RtFn {
        name: "ori_format_char",
        params: &[Ty::I32, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
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
    },
    RtFn {
        name: "ori_list_slice_take",
        // data, len, cap, n, elem_size, out_ptr
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_list_slice_drop",
        // data, len, cap, n, elem_size, out_ptr
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
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
    },
    RtFn {
        name: "ori_str_substring",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    // Reference counting (ARC-safe attributes are CRITICAL — see section 11.3)
    RtFn {
        name: "ori_rc_alloc",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind, Attr::NoaliasReturn],
    },
    RtFn {
        name: "ori_rc_inc",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
    },
    RtFn {
        name: "ori_rc_dec",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
    },
    RtFn {
        name: "ori_rc_free",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
    },
    // Slice-aware RC inc for list/set data buffers.
    // If cap has slice flag, finds original buffer and inc's that.
    RtFn {
        name: "ori_list_rc_inc",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
    },
    // Buffer-aware RC dec for collection data buffers.
    // (data, len, cap, elem_size, elem_dec_fn)
    RtFn {
        name: "ori_buffer_rc_dec",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
    },
    RtFn {
        name: "ori_rc_live_count",
        params: &[],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_rc_reset_live_count",
        params: &[],
        ret: None,
        attrs: &[Attr::Nounwind],
    },
    // COW primitives
    RtFn {
        name: "ori_rc_is_unique",
        params: &[Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_rc_is_unique_or_null",
        params: &[Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_rc_realloc",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_memcpy_elements",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_memmove_elements",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_list_ensure_capacity",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
    },
    // Empty collection sentinels
    RtFn {
        name: "ori_list_empty",
        params: &[],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_str_empty",
        params: &[],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_map_empty",
        params: &[],
        ret: Some(Ty::Map),
        attrs: &[Attr::Nounwind],
    },
    RtFn {
        name: "ori_set_empty",
        params: &[],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
    },
    // Args conversion
    RtFn {
        name: "ori_args_from_argv",
        params: &[Ty::I32, Ty::Ptr],
        ret: Some(Ty::List),
        attrs: &[],
    },
    // Iterator constructors
    RtFn {
        name: "ori_iter_from_list",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        //        data   len   cap   es     elem_dec_fn
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_from_range",
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Bool],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_from_str",
        params: &[Ty::Ptr],
        //        *const OriStr (SSO-safe)
        ret: Some(Ty::Ptr),
        attrs: &[],
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
        attrs: &[],
    },
    // Iterator next
    RtFn {
        name: "ori_iter_next",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
    },
    // Iterator adapters
    RtFn {
        name: "ori_iter_map",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_filter",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_take",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_skip",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_enumerate",
        params: &[Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_zip",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_chain",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    // Iterator consumers
    RtFn {
        name: "ori_iter_collect",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_count",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_any",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_all",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_find",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_for_each",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[],
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
        attrs: &[],
    },
    // Iterator cleanup
    RtFn {
        name: "ori_iter_drop",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    // Panic handler registration
    RtFn {
        name: "ori_register_panic_handler",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    // Catch recovery
    RtFn {
        name: "ori_catch_cleanup",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_catch_recover",
        params: &[],
        ret: Some(Ty::Str),
        attrs: &[],
    },
    // EH personality (Itanium ABI — required by invoke/landingpad)
    RtFn {
        name: "rust_eh_personality",
        params: &[Ty::I32],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
    },
];
