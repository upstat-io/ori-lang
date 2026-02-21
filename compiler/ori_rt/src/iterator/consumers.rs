//! Iterator consumer functions — terminal operations that consume the iterator.
//!
//! All consumers take ownership of the iterator handle and free it when done.
//! Functions are `#[no_mangle] extern "C"` for LLVM codegen.

use std::ptr;

use super::{FoldFn, ForEachFn, IterState, PredicateFn, MAX_ELEM_SIZE};

// ── Collect ─────────────────────────────────────────────────────────────

/// Collect all remaining elements into a new list.
///
/// Returns an `OriList { len: i64, cap: i64, data: *mut u8 }` by writing
/// to the caller-provided `out_ptr` (sret pattern to avoid >16 byte return).
///
/// `elem_size` is the byte size of each element.
#[no_mangle]
pub extern "C" fn ori_iter_collect(iter: *mut u8, elem_size: i64, out_ptr: *mut u8) {
    if iter.is_null() || out_ptr.is_null() {
        // Write empty list
        if !out_ptr.is_null() {
            unsafe {
                out_ptr.cast::<i64>().write(0); // len
                out_ptr.cast::<i64>().add(1).write(0); // cap
                out_ptr.add(16).cast::<*mut u8>().write(ptr::null_mut()); // data
            }
        }
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let es = elem_size.max(1) as usize;

    // Start with capacity 8, grow by doubling
    let mut cap: usize = 8;
    let mut len: usize = 0;
    let mut data = crate::ori_alloc(cap * es, 8);

    let mut elem_buf = [0u8; MAX_ELEM_SIZE];
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        if len >= cap {
            let new_cap = cap * 2;
            let new_data = crate::ori_realloc(data, cap * es, new_cap * es, 8);
            if new_data.is_null() {
                break;
            }
            data = new_data;
            cap = new_cap;
        }
        unsafe {
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), data.add(len * es), es);
        }
        len += 1;
    }

    // Write OriList { len, cap, data } to out_ptr
    unsafe {
        out_ptr.cast::<i64>().write(len as i64);
        out_ptr.cast::<i64>().add(1).write(cap as i64);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }

    // Drop the iterator
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// ── Count ───────────────────────────────────────────────────────────────

/// Count the remaining elements in the iterator, consuming it.
#[no_mangle]
pub extern "C" fn ori_iter_count(iter: *mut u8, elem_size: i64) -> i64 {
    if iter.is_null() {
        return 0;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut count: i64 = 0;
    let mut discard = [0u8; MAX_ELEM_SIZE];

    while unsafe { state.next(discard.as_mut_ptr(), elem_size) } {
        count += 1;
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
    count
}

// ── Any ─────────────────────────────────────────────────────────────────

/// Test if any element satisfies the predicate, consuming the iterator.
///
/// Short-circuits on the first match. Returns 1 if any element matches, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_iter_any(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
) -> i8 {
    if iter.is_null() {
        return 0;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut elem_buf = [0u8; MAX_ELEM_SIZE];

    let result = loop {
        if !unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
            break false;
        }
        if (pred_fn)(pred_env, elem_buf.as_ptr()) {
            break true;
        }
    };

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
    i8::from(result)
}

// ── All ─────────────────────────────────────────────────────────────────

/// Test if all elements satisfy the predicate, consuming the iterator.
///
/// Short-circuits on the first non-match. Returns 1 if all match (or empty), 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_iter_all(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
) -> i8 {
    if iter.is_null() {
        return 1; // vacuously true for empty
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut elem_buf = [0u8; MAX_ELEM_SIZE];

    let result = loop {
        if !unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
            break true;
        }
        if !(pred_fn)(pred_env, elem_buf.as_ptr()) {
            break false;
        }
    };

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
    i8::from(result)
}

// ── Find ────────────────────────────────────────────────────────────────

/// Find the first element satisfying the predicate, consuming the iterator.
///
/// Writes an `Option<T>` to `out_ptr`: `{ i8 tag, T payload }`.
/// Tag = 1 (Some) if found, 0 (None) if no match.
#[no_mangle]
pub extern "C" fn ori_iter_find(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
        }
        return;
    }

    if iter.is_null() {
        // Write None: tag = 0
        unsafe { out_ptr.write(0) };
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    // Element goes at offset 8 (after i8 tag, aligned to 8 bytes for i64 payload)
    let payload_ptr = unsafe { out_ptr.add(8) };
    let mut elem_buf = [0u8; MAX_ELEM_SIZE];

    let found = loop {
        if !unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
            break false;
        }
        if (pred_fn)(pred_env, elem_buf.as_ptr()) {
            // Copy found element to payload slot
            unsafe {
                ptr::copy_nonoverlapping(elem_buf.as_ptr(), payload_ptr, elem_size as usize);
            }
            break true;
        }
    };

    // Write tag
    unsafe {
        out_ptr.write(u8::from(found));
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// ── For Each ────────────────────────────────────────────────────────────

/// Apply a function to each element, consuming the iterator.
///
/// The function receives each element by pointer. Returns void.
#[no_mangle]
pub extern "C" fn ori_iter_for_each(
    iter: *mut u8,
    each_fn: ForEachFn,
    each_env: *mut u8,
    elem_size: i64,
) {
    if iter.is_null() {
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut elem_buf = [0u8; MAX_ELEM_SIZE];

    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        (each_fn)(each_env, elem_buf.as_ptr());
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// ── Fold ────────────────────────────────────────────────────────────────

/// Fold (reduce) the iterator with an accumulator, consuming it.
///
/// `init_ptr` points to the initial accumulator value (`acc_size` bytes).
/// `fold_fn` is a trampoline: `(env, acc_ptr, elem_ptr, out_ptr) -> void`.
/// The final accumulator is written to `out_ptr` (`acc_size` bytes).
#[no_mangle]
pub extern "C" fn ori_iter_fold(
    iter: *mut u8,
    init_ptr: *const u8,
    fold_fn: FoldFn,
    fold_env: *mut u8,
    elem_size: i64,
    acc_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
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

    let state = unsafe { &mut *iter.cast::<IterState>() };

    // Two accumulator buffers: current and next (double-buffered)
    let mut acc_a = [0u8; MAX_ELEM_SIZE];
    let mut acc_b = [0u8; MAX_ELEM_SIZE];
    let mut elem_buf = [0u8; MAX_ELEM_SIZE];

    // Initialize acc_a with init value
    if !init_ptr.is_null() {
        unsafe { ptr::copy_nonoverlapping(init_ptr, acc_a.as_mut_ptr(), as_) };
    }

    let mut current = &mut acc_a;
    let mut next = &mut acc_b;

    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        // fold_fn(env, current_acc, elem, next_acc)
        (fold_fn)(
            fold_env,
            current.as_ptr(),
            elem_buf.as_ptr(),
            next.as_mut_ptr(),
        );
        std::mem::swap(&mut current, &mut next);
    }

    // Copy final accumulator to output
    unsafe { ptr::copy_nonoverlapping(current.as_ptr(), out_ptr, as_) };

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}
