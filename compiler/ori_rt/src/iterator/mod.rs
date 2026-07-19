//! C ABI iterator runtime for AOT-compiled programs.
//!
//! Iterator state remains behind an opaque pointer-sized handle. Sources,
//! adapters, and consumers exchange those handles through `ori_iter_*`
//! functions; closure adapters use the generic `(env, in_ptr, out_ptr)`
//! trampoline ABI. `ori_iter_drop` owns handle and captured-environment cleanup.

mod adapters;
mod consumers;
mod next;
mod next_back;
mod sources;
pub(crate) mod state;

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

pub(crate) use state::{
    AccumulatorDecFn, ElemBuf, ElemIncFn, FoldFn, ForEachFn, IterState, PredicateFn,
};

/// Take ownership of an opaque iterator handle at an ABI ownership boundary.
///
/// Returning the state in a `Box` ensures normal returns and unwinding both
/// release the iterator allocation exactly once.
pub(super) fn take_iter(iter: *mut u8) -> Option<Box<IterState>> {
    if iter.is_null() {
        return None;
    }
    // SAFETY: Each non-null ABI handle is one unrecovered `Box<IterState>` raw pointer.
    Some(unsafe { Box::from_raw(iter.cast::<IterState>()) })
}

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
    // SAFETY: The live handle is an aligned, initialized `IterState` borrowed for this call.
    let state = unsafe { &mut *iter.cast::<IterState>() };
    // SAFETY: The caller provides the output region; `state` owns every source read by `next`.
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
    // SAFETY: The live handle is an aligned, initialized `IterState` borrowed for this call.
    let state = unsafe { &mut *iter.cast::<IterState>() };
    // SAFETY: The caller provides the output region; `state` owns every source read by `next_back`.
    let has_next = unsafe { state.next_back(out_ptr, elem_size) };
    i8::from(has_next)
}

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
