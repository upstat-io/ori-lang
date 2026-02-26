//! Runtime function declarations for V2 codegen.
//!
//! Declares extern functions provided by the Ori runtime library (`ori_rt`).
//! These are resolved at link time (AOT) or via symbol mapping (JIT).
//!
//! # Lazy declaration (production)
//!
//! `declare_single()` declares one runtime function on demand, with the
//! correct signature and attributes from the `RT_FUNCTIONS` table.
//! Called via `IrBuilder::runtime_fn()`, which caches the result.
//!
//! A trivial `@main () -> int = 33` produces zero runtime declarations.
//!
//! # Eager declaration (tests)
//!
//! `declare_runtime()` eagerly declares all functions. Used by unit tests
//! that need the full set available in the LLVM module.

use super::ir_builder::IrBuilder;
use super::value_id::{FunctionId, LLVMTypeId};

// ---------------------------------------------------------------------------
// Type and attribute descriptors
// ---------------------------------------------------------------------------

/// Primitive type descriptor for runtime function signatures.
#[derive(Clone, Copy)]
enum Ty {
    I64,
    I32,
    I8,
    F64,
    Bool,
    Ptr,
    /// `{i64, ptr}` — Ori string representation.
    Str,
    /// `{i64, i64, ptr}` — Ori list/set representation.
    List,
    /// `{i32, i64}` — char iteration result `{codepoint, next_offset}`.
    CharResult,
}

/// Function attribute applied after declaration.
#[derive(Clone, Copy)]
enum Attr {
    Nounwind,
    Cold,
    NoaliasReturn,
    MemArgmemRW,
}

/// Runtime function specification: name, signature, and attributes.
struct RtFn {
    name: &'static str,
    params: &'static [Ty],
    ret: Option<Ty>,
    attrs: &'static [Attr],
}

// ---------------------------------------------------------------------------
// Runtime function table (single source of truth)
// ---------------------------------------------------------------------------

/// All runtime functions declared by the Ori compiler.
///
/// Each entry specifies the function's name, parameter types, return type,
/// and any LLVM attributes. This table is the single source of truth —
/// both `declare_single()` and `declare_runtime()` use it.
static RT_FUNCTIONS: &[RtFn] = &[
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
        name: "ori_list_push_new",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
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
    // Map
    RtFn {
        name: "ori_map_contains_key",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[],
    },
    RtFn {
        name: "ori_map_keys_to_list",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_values_to_list",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_get",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_insert",
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
    },
    RtFn {
        name: "ori_map_remove",
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
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
    // Format (§3.16 Formattable trait)
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
    // Reference counting (ARC-safe attributes are CRITICAL — see §11.3)
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
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
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
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[],
    },
    RtFn {
        name: "ori_iter_from_map",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
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

// ---------------------------------------------------------------------------
// Type and attribute resolution
// ---------------------------------------------------------------------------

/// Resolve a type descriptor to an LLVM type ID.
fn resolve_ty(builder: &mut IrBuilder, ty: Ty) -> LLVMTypeId {
    match ty {
        Ty::I64 => builder.i64_type(),
        Ty::I32 => builder.i32_type(),
        Ty::I8 => builder.i8_type(),
        Ty::F64 => builder.f64_type(),
        Ty::Bool => builder.bool_type(),
        Ty::Ptr => builder.ptr_type(),
        Ty::Str => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
        Ty::List => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
        Ty::CharResult => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i32().into(),
                    builder.scx().type_i64().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
    }
}

/// Apply an attribute to a declared function.
fn apply_attr(builder: &mut IrBuilder, func: FunctionId, attr: Attr) {
    match attr {
        Attr::Nounwind => builder.add_nounwind_attribute(func),
        Attr::Cold => builder.add_cold_attribute(func),
        Attr::NoaliasReturn => builder.add_noalias_return_attribute(func),
        Attr::MemArgmemRW => builder.add_memory_argmem_readwrite_attribute(func),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Declare a single runtime function by name (lazy, on first use).
///
/// Looks up the function in `RT_FUNCTIONS`, declares it in the LLVM module
/// with the correct signature and attributes, and returns the `FunctionId`.
///
/// Called by `IrBuilder::runtime_fn()` which caches the result. If the
/// function already exists in the LLVM module, reuses the existing
/// declaration (via `declare_extern_function`'s idempotency).
///
/// # Panics
///
/// Panics if `name` is not a known runtime function.
pub fn declare_single(builder: &mut IrBuilder, name: &str) -> FunctionId {
    let spec = RT_FUNCTIONS
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("unknown runtime function: {name}"));
    let params: Vec<LLVMTypeId> = spec
        .params
        .iter()
        .map(|&t| resolve_ty(builder, t))
        .collect();
    let ret = spec.ret.map(|t| resolve_ty(builder, t));
    let id = builder.declare_extern_function(name, &params, ret);
    for &attr in spec.attrs {
        apply_attr(builder, id, attr);
    }
    id
}

/// Like [`declare_single`], but returns `None` if the name is not a known
/// runtime function (instead of panicking).
///
/// Returns `(static_name, func_id)` where `static_name` is the `&'static str`
/// from the `RT_FUNCTIONS` table — suitable for use as a cache key in
/// `IrBuilder::try_runtime_fn()`.
pub fn try_declare_single(
    builder: &mut IrBuilder,
    name: &str,
) -> Option<(&'static str, FunctionId)> {
    let spec = RT_FUNCTIONS.iter().find(|f| f.name == name)?;
    let params: Vec<LLVMTypeId> = spec
        .params
        .iter()
        .map(|&t| resolve_ty(builder, t))
        .collect();
    let ret = spec.ret.map(|t| resolve_ty(builder, t));
    let id = builder.declare_extern_function(name, &params, ret);
    for &attr in spec.attrs {
        apply_attr(builder, id, attr);
    }
    Some((spec.name, id))
}

/// Eagerly declare all runtime functions in the LLVM module.
///
/// Used by tests that need the full set available. Production code uses
/// `IrBuilder::runtime_fn()` for on-demand declaration.
pub fn declare_runtime(builder: &mut IrBuilder<'_, '_>) {
    for spec in RT_FUNCTIONS {
        declare_single(builder, spec.name);
    }
}

/// Returns the names of all known runtime functions.
///
/// Used by tests to verify sync with JIT/AOT-only function lists.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    RT_FUNCTIONS.iter().map(|f| f.name)
}

/// Returns the number of runtime functions in the table.
#[cfg(test)]
pub fn count() -> usize {
    RT_FUNCTIONS.len()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
