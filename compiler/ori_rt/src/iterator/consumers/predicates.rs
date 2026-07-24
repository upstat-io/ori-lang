//! Predicate, search, counting, and side-effect consumers.

use std::ptr;

use super::super::state::{assert_elem_size, YieldGuard};
use super::super::{ElemBuf, ElemIncFn, ForEachFn, PredicateFn};
use super::take_iter;
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

// Count

/// Count the remaining elements in the iterator, consuming it.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_count(iter: *mut u8, elem_size: i64) -> i64 {
    assert_elem_size(elem_size, "ori_iter_count");
    if iter.is_null() {
        return 0;
    }

    let Some(mut state) = take_iter(iter) else {
        return 0;
    };
    let mut count: i64 = 0;
    let mut discard = ElemBuf::new();

    while unsafe { state.next(discard.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, discard.as_mut_ptr());
        count += 1;
    }

    count
}

// Any

/// Test if any element satisfies the predicate, consuming the iterator.
///
/// Short-circuits on the first match. Returns 1 if any element matches, 0 otherwise.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_any(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
) -> i8 {
    assert_elem_size(elem_size, "ori_iter_any");
    if iter.is_null() {
        return 0;
    }

    let Some(mut state) = take_iter(iter) else {
        return 0;
    };
    let mut elem_buf = ElemBuf::new();

    let result = loop {
        if !unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
            break false;
        }
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        if (pred_fn)(pred_env, elem_buf.as_ptr()) {
            break true;
        }
    };

    i8::from(result)
}

// All

/// Test if all elements satisfy the predicate, consuming the iterator.
///
/// Short-circuits on the first non-match. Returns 1 if all match (or empty), 0 otherwise.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_all(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
) -> i8 {
    assert_elem_size(elem_size, "ori_iter_all");
    if iter.is_null() {
        return 1; // vacuously true for empty
    }

    let Some(mut state) = take_iter(iter) else {
        return 1;
    };
    let mut elem_buf = ElemBuf::new();

    let result = loop {
        if !unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
            break true;
        }
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        if !(pred_fn)(pred_env, elem_buf.as_ptr()) {
            break false;
        }
    };

    i8::from(result)
}

// Find

/// Find the first element satisfying the predicate, consuming the iterator.
///
/// Writes an `Option<T>` to `out_ptr`: `{ i64 tag, T payload }`.
/// Uses ARC enum convention: Some=0 (first variant), None=1 (second variant).
///
/// Layout matches LLVM codegen's `{i64, T}` enum representation.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_find(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<ElemIncFn>,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_find");
    if out_ptr.is_null() {
        drop(take_iter(iter));
        return;
    }

    if iter.is_null() {
        unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };
    // Payload at offset 8 (after i64 tag)
    let payload_ptr = unsafe { out_ptr.add(8) };
    let mut elem_buf = ElemBuf::new();

    let found = loop {
        if !unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
            break false;
        }
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        if (pred_fn)(pred_env, elem_buf.as_ptr()) {
            // Copy found element to payload slot
            unsafe {
                ptr::copy_nonoverlapping(elem_buf.as_ptr(), payload_ptr, elem_size as usize);
            }
            if let Some(inc) = elem_inc_fn {
                inc(payload_ptr);
            }
            break true;
        }
    };

    unsafe {
        out_ptr.cast::<i64>().write(if found {
            OPTION_TAG_SOME
        } else {
            OPTION_TAG_NONE
        });
    }
}

// For Each

/// Apply a function to each element, consuming the iterator.
///
/// The function receives each element by pointer. Returns void.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_for_each(
    iter: *mut u8,
    each_fn: ForEachFn,
    each_env: *mut u8,
    elem_size: i64,
) {
    assert_elem_size(elem_size, "ori_iter_for_each");
    if iter.is_null() {
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };
    let mut elem_buf = ElemBuf::new();

    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        (each_fn)(each_env, elem_buf.as_ptr());
    }
}
