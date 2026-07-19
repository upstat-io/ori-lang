//! Forward fold and last-element consumers.

use std::ptr;

use super::super::state::{assert_elem_size, YieldGuard};
use super::super::{AccumulatorDecFn, ElemBuf, ElemIncFn, FoldFn};
use super::take_iter;
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

/// Owns the accumulator credit held by the runtime's current scratch slot.
///
/// Generated fold wrappers retain the pointer-loaded accumulator before passing
/// it to an owned callback parameter. After a successful callback, its returned
/// credit lives in the next slot and the runtime must release the prior one.
pub(super) struct AccumulatorOwner {
    ptr: *mut u8,
    dec_fn: Option<AccumulatorDecFn>,
    armed: bool,
}

impl AccumulatorOwner {
    pub(super) fn new(ptr: *mut u8, dec_fn: Option<AccumulatorDecFn>, armed: bool) -> Self {
        Self { ptr, dec_fn, armed }
    }

    pub(super) fn replace_with(&mut self, next: *mut u8) {
        let previous = self.ptr;
        let previous_was_armed = self.armed;
        // Arm the callback-produced value before releasing the previous one.
        // If a user-defined accumulator destructor unwinds, `Drop` must still
        // protect the newly produced credit.
        self.ptr = next;
        self.armed = true;
        if previous_was_armed {
            if let Some(dec) = self.dec_fn {
                dec(previous);
            }
        }
    }

    pub(super) fn transfer_to_output(&mut self) {
        self.armed = false;
    }

    fn release(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        if let Some(dec) = self.dec_fn {
            dec(self.ptr);
        }
    }
}

impl Drop for AccumulatorOwner {
    fn drop(&mut self) {
        self.release();
    }
}

// Fold

/// Fold (reduce) the iterator with an accumulator, consuming it.
///
/// `init_ptr` points to the initial accumulator value (`acc_size` bytes).
/// `fold_fn` is a trampoline: `(env, acc_ptr, elem_ptr, out_ptr) -> void`.
/// `acc_dec_fn` releases superseded managed accumulator values and is null for scalars.
/// The final accumulator is written to `out_ptr` (`acc_size` bytes).
#[no_mangle]
pub extern "C-unwind" fn ori_iter_fold(
    iter: *mut u8,
    init_ptr: *const u8,
    fold_fn: FoldFn,
    fold_env: *mut u8,
    elem_size: i64,
    acc_size: i64,
    acc_dec_fn: Option<AccumulatorDecFn>,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_fold");
    assert_elem_size(acc_size, "ori_iter_fold(acc)");
    if out_ptr.is_null() {
        // Acquire the iterator guard before running potentially-unwinding
        // accumulator teardown so both owned inputs remain protected.
        let _state = take_iter(iter);
        if let (Some(dec), false) = (acc_dec_fn, init_ptr.is_null()) {
            dec(init_ptr.cast_mut());
        }
        return;
    }

    let as_ = acc_size.max(1) as usize;

    if iter.is_null() {
        // No elements — copy init to output
        if !init_ptr.is_null() {
            unsafe { ptr::copy_nonoverlapping(init_ptr, out_ptr, as_) };
        }
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };

    // Two accumulator buffers: current and next (double-buffered)
    let mut acc_a = ElemBuf::new();
    let mut acc_b = ElemBuf::new();
    let mut elem_buf = ElemBuf::new();

    // Initialize acc_a with init value
    if !init_ptr.is_null() {
        unsafe { ptr::copy_nonoverlapping(init_ptr, acc_a.as_mut_ptr(), as_) };
    }

    let mut accumulator =
        AccumulatorOwner::new(acc_a.as_mut_ptr(), acc_dec_fn, !init_ptr.is_null());

    let mut current = &mut acc_a;
    let mut next = &mut acc_b;

    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        // fold_fn(env, current_acc, elem, next_acc)
        (fold_fn)(
            fold_env,
            current.as_ptr(),
            elem_buf.as_ptr(),
            next.as_mut_ptr(),
        );
        accumulator.replace_with(next.as_mut_ptr());
        std::mem::swap(&mut current, &mut next);
    }

    // Copy final accumulator to output
    unsafe { ptr::copy_nonoverlapping(current.as_ptr(), out_ptr, as_) };
    accumulator.transfer_to_output();
}

// Last

/// Return the last element of the iterator, consuming it.
///
/// Writes `Option<T>` to `out_ptr`: `{ i64 tag, T payload }`.
/// Tag convention: Some=0, None=1. Advances from the back exactly once.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_last(
    iter: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<ElemIncFn>,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_last");
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
    let payload_ptr = unsafe { out_ptr.add(8) };
    let mut elem_buf = ElemBuf::new();
    let found = unsafe { state.next_back(elem_buf.as_mut_ptr(), elem_size) };
    if found {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        unsafe {
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), payload_ptr, elem_size as usize);
        }
        if let Some(inc) = elem_inc_fn {
            inc(payload_ptr);
        }
    }

    unsafe {
        out_ptr.cast::<i64>().write(if found {
            OPTION_TAG_SOME
        } else {
            OPTION_TAG_NONE
        });
    }
}
