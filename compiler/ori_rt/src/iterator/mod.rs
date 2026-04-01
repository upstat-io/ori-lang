//! Runtime iterator support for AOT-compiled Ori programs.
//!
//! Provides an opaque iterator handle that LLVM code manipulates via
//! `extern "C"` functions. The internal `IterState` enum is never exposed
//! to LLVM — all interaction goes through pointer-sized handles.
//!
//! # Architecture
//!
//! - LLVM sees iterators as `ptr` (opaque handle)
//! - Each `ori_iter_*` function takes/returns `ptr` handles
//! - Adapters (map, filter) accept trampoline function pointers that bridge
//!   typed closures to the runtime's generic `(env, in_ptr, out_ptr)` ABI
//! - `ori_iter_drop` frees the handle and any captured environment pointers
//!
//! # Submodules
//!
//! - `state` — `IterState` enum, `Drop` impl, type aliases for trampolines
//! - `next` — `IterState::next()` dispatch and per-variant advancement
//! - `sources` — Source constructors (`ori_iter_from_list`, `from_range`, etc.)
//! - `adapters` — Adapter constructors (`ori_iter_map`, `filter`, `take`, etc.)
//! - `consumers` — Terminal operations (`collect`, `count`, `fold`, `find`, etc.)

mod adapters;
mod consumers;
mod next;
mod sources;
pub(crate) mod state;

// Re-export all `#[no_mangle] extern "C"` functions at module level.
pub use adapters::{
    ori_iter_chain, ori_iter_enumerate, ori_iter_filter, ori_iter_map, ori_iter_skip,
    ori_iter_take, ori_iter_zip,
};
pub use consumers::{
    ori_iter_all, ori_iter_any, ori_iter_collect, ori_iter_collect_set, ori_iter_count,
    ori_iter_find, ori_iter_fold, ori_iter_for_each,
};
pub use sources::{
    ori_iter_from_list, ori_iter_from_map, ori_iter_from_option, ori_iter_from_range,
    ori_iter_from_str,
};

// Re-export types used by submodules (consumers needs these from state).
pub(crate) use state::{FoldFn, ForEachFn, IterState, PredicateFn, MAX_ELEM_SIZE};

// Extern C API — Core

/// Advance the iterator, writing the next element to `out_ptr`.
///
/// Returns 1 if an element was produced, 0 if the iterator is exhausted.
/// `elem_size` must match the element size of the iterator's output type.
#[no_mangle]
pub extern "C" fn ori_iter_next(iter: *mut u8, out_ptr: *mut u8, elem_size: i64) -> i8 {
    if iter.is_null() || out_ptr.is_null() {
        return 0;
    }
    state::assert_elem_size(elem_size, "ori_iter_next");
    let state = unsafe { &mut *iter.cast::<IterState>() };
    let has_next = unsafe { state.next(out_ptr, elem_size) };
    i8::from(has_next)
}

// Extern C API — Cleanup

/// Drop (free) an iterator handle and all its internal state.
///
/// Must be called when the iterator is no longer needed to prevent leaks.
/// Called automatically at the end of for-loops over iterators.
#[no_mangle]
pub extern "C" fn ori_iter_drop(iter: *mut u8) {
    if iter.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

#[cfg(test)]
mod tests;
