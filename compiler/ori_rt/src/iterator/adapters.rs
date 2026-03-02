//! Iterator adapter constructors — create adapter iterators that wrap sources.
//!
//! These are `extern "C"` functions called from LLVM-generated code to create
//! map, filter, take, skip, enumerate, zip, chain, and cycle adapters.

use std::ptr;

use super::state::{empty_range, IterState, PredicateFn, TransformFn};

/// Create a mapped iterator adapter.
///
/// `transform_fn` is a trampoline: `(env, in_ptr, out_ptr) -> void`.
/// `transform_env` is the closure environment pointer (may be null).
/// `in_size` is the byte size of input elements (for scratch buffer sizing).
#[no_mangle]
pub extern "C" fn ori_iter_map(
    iter: *mut u8,
    transform_fn: TransformFn,
    transform_env: *mut u8,
    in_size: i64,
) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::Mapped {
        source,
        transform_fn,
        transform_env,
        in_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a filtered iterator adapter.
///
/// `predicate_fn` is a trampoline: `(env, elem_ptr) -> bool`.
/// `predicate_env` is the closure environment pointer (may be null).
#[no_mangle]
pub extern "C" fn ori_iter_filter(
    iter: *mut u8,
    predicate_fn: PredicateFn,
    predicate_env: *mut u8,
    elem_size: i64,
) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::Filtered {
        source,
        predicate_fn,
        predicate_env,
        elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a take(n) adapter — yields at most `n` elements from source.
#[no_mangle]
pub extern "C" fn ori_iter_take(iter: *mut u8, n: i64) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::TakeN {
        source,
        remaining: n.max(0),
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a skip(n) adapter — skips `n` elements then yields the rest.
#[no_mangle]
pub extern "C" fn ori_iter_skip(iter: *mut u8, n: i64) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::SkipN {
        source,
        remaining: n.max(0),
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create an enumerate adapter — wraps each element with its 0-based index.
///
/// Output element layout: `{ i64 index, T element }`.
#[no_mangle]
pub extern "C" fn ori_iter_enumerate(iter: *mut u8) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::Enumerated { source, index: 0 };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a zip adapter — pairs elements from two iterators.
///
/// Output element layout: `[left_elem | right_elem]` (concatenated bytes).
/// Stops when either iterator is exhausted.
#[no_mangle]
pub extern "C" fn ori_iter_zip(left: *mut u8, right: *mut u8, left_elem_size: i64) -> *mut u8 {
    if left.is_null() || right.is_null() {
        if !left.is_null() {
            super::ori_iter_drop(left);
        }
        if !right.is_null() {
            super::ori_iter_drop(right);
        }
        return ptr::null_mut();
    }
    let left_state = unsafe { Box::from_raw(left.cast::<IterState>()) };
    let right_state = unsafe { Box::from_raw(right.cast::<IterState>()) };
    let state = IterState::Zipped {
        left: left_state,
        right: right_state,
        left_elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a chain adapter — yields all elements from first, then all from second.
#[no_mangle]
pub extern "C" fn ori_iter_chain(first: *mut u8, second: *mut u8) -> *mut u8 {
    if first.is_null() && second.is_null() {
        return ptr::null_mut();
    }
    // If one is null, still chain — the null side yields nothing
    let first_state = if first.is_null() {
        Box::new(empty_range())
    } else {
        unsafe { Box::from_raw(first.cast::<IterState>()) }
    };
    let second_state = if second.is_null() {
        Box::new(empty_range())
    } else {
        unsafe { Box::from_raw(second.cast::<IterState>()) }
    };
    let state = IterState::Chained {
        first: first_state,
        second: second_state,
        first_done: false,
    };
    Box::into_raw(Box::new(state)).cast()
}
