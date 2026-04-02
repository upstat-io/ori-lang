//! Iterator adapter constructors — create adapter iterators that wrap sources.
//!
//! These are `extern "C"` functions called from LLVM-generated code to create
//! map, filter, take, skip, enumerate, zip, chain, and cycle adapters.

use std::ptr;

use super::state::{assert_elem_size, empty_range, IterState, PredicateFn, TransformFn};

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
    assert_elem_size(in_size, "ori_iter_map");
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
    assert_elem_size(elem_size, "ori_iter_filter");
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
    assert_elem_size(left_elem_size, "ori_iter_zip");
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

/// Create a flatten adapter — flattens iterator of iterators into a single stream.
///
/// The source iterator must yield iterator pointers (ptr-sized = 8 bytes).
/// `inner_elem_size` is the byte size of elements in the inner iterators.
#[no_mangle]
pub extern "C" fn ori_iter_flatten(iter: *mut u8, inner_elem_size: i64) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    assert_elem_size(inner_elem_size, "ori_iter_flatten");
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::Flattened {
        source,
        inner: None,
        inner_elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a cycle adapter — repeats the source iterator infinitely.
///
/// Buffers elements on the first pass, then replays from the buffer.
/// Empty sources produce an empty cycle (no infinite loop).
#[no_mangle]
pub extern "C" fn ori_iter_cycle(iter: *mut u8, elem_size: i64) -> *mut u8 {
    if iter.is_null() {
        return ptr::null_mut();
    }
    assert_elem_size(elem_size, "ori_iter_cycle");
    let source = unsafe { Box::from_raw(iter.cast::<IterState>()) };
    let state = IterState::Cycled {
        source: Some(source),
        buffer: Vec::new(),
        buf_pos: 0,
        elem_size,
        source_exhausted: false,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a reversed iterator by collecting all elements and iterating backward.
///
/// This is an eager operation — all elements are collected into memory.
#[no_mangle]
pub extern "C" fn ori_iter_rev(iter: *mut u8, elem_size: i64) -> *mut u8 {
    use super::state::MAX_ELEM_SIZE;

    if iter.is_null() {
        return ptr::null_mut();
    }
    assert_elem_size(elem_size, "ori_iter_rev");
    let state = unsafe { &mut *iter.cast::<IterState>() };
    let es = elem_size.max(1) as usize;

    // Collect all elements
    let mut elements = Vec::new();
    let mut elem_buf = [0u8; MAX_ELEM_SIZE];
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        elements.extend_from_slice(&elem_buf[..es]);
    }

    let count = (elements.len() / es) as i64;

    // Free the source iterator
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });

    let rev_state = IterState::Reversed {
        elements,
        pos: count,
        elem_size,
    };
    Box::into_raw(Box::new(rev_state)).cast()
}
