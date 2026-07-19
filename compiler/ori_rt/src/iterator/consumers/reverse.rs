//! Reverse fold and reverse search consumers.

use std::ptr;

use super::super::state::{assert_elem_size, YieldGuard};
use super::super::{AccumulatorDecFn, ElemBuf, ElemIncFn, FoldFn, PredicateFn};
use super::fold::AccumulatorOwner;
use super::take_iter;
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

// Rfold

/// Fold the iterator from right-to-left, consuming it.
///
/// Advances the double-ended iterator directly from the back.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_rfold(
    iter: *mut u8,
    init_ptr: *const u8,
    fold_fn: FoldFn,
    fold_env: *mut u8,
    elem_size: i64,
    acc_size: i64,
    acc_dec_fn: Option<AccumulatorDecFn>,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_rfold");
    assert_elem_size(acc_size, "ori_iter_rfold(acc)");
    if out_ptr.is_null() {
        let _state = take_iter(iter);
        if let (Some(dec), false) = (acc_dec_fn, init_ptr.is_null()) {
            dec(init_ptr.cast_mut());
        }
        return;
    }

    let as_ = acc_size.max(1) as usize;
    if iter.is_null() {
        if !init_ptr.is_null() {
            unsafe { ptr::copy_nonoverlapping(init_ptr, out_ptr, as_) };
        }
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };

    let mut acc_a = ElemBuf::new();
    let mut acc_b = ElemBuf::new();
    let mut elem_buf = ElemBuf::new();

    if !init_ptr.is_null() {
        unsafe { ptr::copy_nonoverlapping(init_ptr, acc_a.as_mut_ptr(), as_) };
    }

    let mut accumulator =
        AccumulatorOwner::new(acc_a.as_mut_ptr(), acc_dec_fn, !init_ptr.is_null());

    let mut current = &mut acc_a;
    let mut next = &mut acc_b;
    while unsafe { state.next_back(elem_buf.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        (fold_fn)(
            fold_env,
            current.as_ptr(),
            elem_buf.as_ptr(),
            next.as_mut_ptr(),
        );
        accumulator.replace_with(next.as_mut_ptr());
        std::mem::swap(&mut current, &mut next);
    }

    unsafe { ptr::copy_nonoverlapping(current.as_ptr(), out_ptr, as_) };
    accumulator.transfer_to_output();
}

// Rfind

/// Find the last element matching a predicate, consuming the iterator.
///
/// Searches directly from the back of the double-ended iterator.
/// Writes `Option<T>` to `out_ptr`: `{ i64 tag, T payload }`.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_rfind(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<ElemIncFn>,
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
    let mut elem_buf = ElemBuf::new();
    let payload_ptr = unsafe { out_ptr.add(8) };

    while unsafe { state.next_back(elem_buf.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        if (pred_fn)(pred_env, elem_buf.as_ptr()) {
            unsafe {
                ptr::copy_nonoverlapping(elem_buf.as_ptr(), payload_ptr, elem_size as usize);
                out_ptr.cast::<i64>().write(OPTION_TAG_SOME);
            }
            if let Some(inc) = elem_inc_fn {
                inc(payload_ptr);
            }
            return;
        }
    }

    unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
}
