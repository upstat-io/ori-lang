//! Runtime function declaration table.
//!
//! Single source of truth for all runtime functions declared by the Ori compiler.
//! Each entry specifies the function's name, parameter types, return type,
//! and any LLVM attributes.
//!
//! Entries live in per-family `families/` submodules; `RT_FUNCTIONS`
//! concatenates them here, so consumers, the O(1) index, and the
//! `jit_allowed` partition test still see exactly one table.
//!
//! Type definitions (`Ty`, `Attr`, `RtFn`) live in the sibling `types` module.

use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) use super::types::{Attr, RtFn, Ty};

/// O(1) lookup index for runtime functions by name.
static RT_INDEX: LazyLock<HashMap<&'static str, &'static RtFn>> =
    LazyLock::new(|| RT_FUNCTIONS.iter().map(|f| (f.name, *f)).collect());

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
/// Used to skip codegen after noreturn calls.
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
pub(crate) static RT_FUNCTIONS: LazyLock<Vec<&'static RtFn>> =
    LazyLock::new(|| RT_FAMILY_TABLES.iter().flat_map(|fam| fam.iter()).collect());

/// Per-family declaration slices, concatenated in declaration order.
///
/// Families live in `families/` submodules; this list is the one place
/// that enumerates them, so the concatenation order (and the audit
/// surface) stays single-sourced.
static RT_FAMILY_TABLES: &[&[RtFn]] = &[
    super::families::base::BASE,
    super::families::list::LIST,
    super::families::map::MAP,
    super::families::set::SET,
    super::families::strings::STRINGS,
    super::families::format::FORMAT,
    super::families::slice::SLICE,
    super::families::rc::RC,
    super::families::cow_args::COW_ARGS,
    super::families::iterator::ITERATOR,
    super::families::eh::EH,
];
