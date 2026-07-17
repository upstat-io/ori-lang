//! Reverse fold and reverse search consumers.

use std::ptr;

use super::super::state::assert_elem_size;
use super::super::{ElemBuf, FoldFn, PredicateFn};
use super::take_iter;
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

// Rfold

/// Fold the iterator from right-to-left, consuming it.
///
/// Collects all elements into a buffer, then folds right-to-left.
/// This is the simplest correct implementation without DEI runtime support.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_rfold(
    iter: *mut u8,
    init_ptr: *const u8,
    fold_fn: FoldFn,
    fold_env: *mut u8,
    elem_size: i64,
    acc_size: i64,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_rfold");
    assert_elem_size(acc_size, "ori_iter_rfold(acc)");
    if out_ptr.is_null() {
        drop(take_iter(iter));
        return;
    }

    let as_ = acc_size.max(1) as usize;
    let es = elem_size.max(1) as usize;

    if iter.is_null() {
        if !init_ptr.is_null() {
            unsafe { ptr::copy_nonoverlapping(init_ptr, out_ptr, as_) };
        }
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };

    // Collect all elements into a Vec
    let mut elements: Vec<u8> = Vec::new();
    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        elements.extend_from_slice(&elem_buf[..es]);
    }

    drop(state);

    // Fold right-to-left
    let mut acc_a = ElemBuf::new();
    let mut acc_b = ElemBuf::new();

    if !init_ptr.is_null() {
        unsafe { ptr::copy_nonoverlapping(init_ptr, acc_a.as_mut_ptr(), as_) };
    }

    let mut current = &mut acc_a;
    let mut next = &mut acc_b;
    let count = elements.len() / es;

    for i in (0..count).rev() {
        let elem_ptr = elements[i * es..].as_ptr();
        (fold_fn)(fold_env, current.as_ptr(), elem_ptr, next.as_mut_ptr());
        std::mem::swap(&mut current, &mut next);
    }

    unsafe { ptr::copy_nonoverlapping(current.as_ptr(), out_ptr, as_) };
}

// Rfind

/// Find the last element matching a predicate, consuming the iterator.
///
/// Collects all elements, then searches right-to-left.
/// Writes `Option<T>` to `out_ptr`: `{ i64 tag, T payload }`.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_rfind(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_rfind");
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
    let es = elem_size.max(1) as usize;

    // Collect all elements
    let mut elements: Vec<u8> = Vec::new();
    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        elements.extend_from_slice(&elem_buf[..es]);
    }

    drop(state);

    // Search right-to-left
    let count = elements.len() / es;
    let payload_ptr = unsafe { out_ptr.add(8) };

    for i in (0..count).rev() {
        let elem_ptr = elements[i * es..].as_ptr();
        if (pred_fn)(pred_env, elem_ptr) {
            unsafe {
                ptr::copy_nonoverlapping(elem_ptr, payload_ptr, es);
                out_ptr.cast::<i64>().write(OPTION_TAG_SOME);
            }
            return;
        }
    }

    unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
}
