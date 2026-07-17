//! Runtime iterator support for AOT-compiled Ori programs.
//!
//! Provides an opaque iterator handle that LLVM code manipulates via C ABI
//! functions. The internal `IterState` enum is never exposed
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
mod next_back;
mod sources;
pub(crate) mod state;

// Re-export all `#[no_mangle]` C-ABI functions at module level.
pub use adapters::{
    ori_iter_chain, ori_iter_cycle, ori_iter_enumerate, ori_iter_filter, ori_iter_flatten,
    ori_iter_map, ori_iter_rev, ori_iter_skip, ori_iter_take, ori_iter_zip,
};
pub use consumers::{
    ori_iter_all, ori_iter_any, ori_iter_collect, ori_iter_collect_set, ori_iter_count,
    ori_iter_find, ori_iter_fold, ori_iter_for_each, ori_iter_join, ori_iter_last, ori_iter_rfind,
    ori_iter_rfold,
};
pub use sources::{
    ori_iter_from_list, ori_iter_from_map, ori_iter_from_option, ori_iter_from_range,
    ori_iter_from_str, ori_iter_repeat, ori_range_contains, ori_range_len,
};

// Re-export types used by submodules (consumers needs these from state).
pub(crate) use state::{ElemBuf, FoldFn, ForEachFn, IterState, PredicateFn};

/// Take ownership of an opaque iterator handle at an ABI ownership boundary.
///
/// Returning the state in a `Box` ensures normal returns and unwinding both
/// release the iterator allocation exactly once.
pub(super) fn take_iter(iter: *mut u8) -> Option<Box<IterState>> {
    if iter.is_null() {
        return None;
    }
    // SAFETY: Every non-null handle comes from `Box::into_raw<IterState>`, and each consuming ABI boundary recovers that allocation exactly once.
    Some(unsafe { Box::from_raw(iter.cast::<IterState>()) })
}

// Extern C API — Core

/// Advance the iterator, writing the next element to `out_ptr`.
///
/// Returns 1 if an element was produced, 0 if the iterator is exhausted.
/// `elem_size` must match the element size of the iterator's output type.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_next(iter: *mut u8, out_ptr: *mut u8, elem_size: i64) -> i8 {
    if iter.is_null() || out_ptr.is_null() {
        return 0;
    }
    state::assert_elem_size(elem_size, "ori_iter_next");
    // SAFETY: The live handle points to an aligned `IterState` allocation that remains caller-owned throughout this borrowed advance.
    let state = unsafe { &mut *iter.cast::<IterState>() };
    // SAFETY: The ABI caller provides `out_ptr` writable for `elem_size` bytes, and `state` preserves its variant-specific source allocation invariants.
    let has_next = unsafe { state.next(out_ptr, elem_size) };
    i8::from(has_next)
}

/// Advance the iterator from the back, writing the element to `out_ptr`.
///
/// Returns 1 if an element was produced, 0 if exhausted. Backs the user-facing
/// `DoubleEndedIterator.next_back()` protocol method. `elem_size` must match the
/// element size of the iterator's output type.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_next_back(iter: *mut u8, out_ptr: *mut u8, elem_size: i64) -> i8 {
    if iter.is_null() || out_ptr.is_null() {
        return 0;
    }
    state::assert_elem_size(elem_size, "ori_iter_next_back");
    // SAFETY: The live handle points to an aligned `IterState` allocation that remains caller-owned throughout this borrowed advance.
    let state = unsafe { &mut *iter.cast::<IterState>() };
    // SAFETY: The ABI caller provides `out_ptr` writable for `elem_size` bytes, and `state` preserves its variant-specific source allocation invariants.
    let has_next = unsafe { state.next_back(out_ptr, elem_size) };
    i8::from(has_next)
}

// Extern C API — Cleanup

/// Drop iterator state and release all resources it owns.
///
/// Generated for-loop cleanup sends every live handle through this consuming
/// boundary exactly once.
#[no_mangle]
pub extern "C" fn ori_iter_drop(iter: *mut u8) {
    drop(take_iter(iter));
}

#[cfg(test)]
mod tests;
