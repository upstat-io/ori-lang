//! Iterator consumer functions — terminal operations that consume the iterator.
//!
//! All consumers take ownership of the iterator handle and free it when done.
//! Functions are `#[no_mangle] extern "C"` for LLVM codegen.

use std::ptr;

use super::state::assert_elem_size;
use super::{ElemBuf, FoldFn, ForEachFn, IterState, PredicateFn};
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

// Collect

/// Collect all remaining elements into a new list.
///
/// Returns an `OriList { len: i64, cap: i64, data: *mut u8 }` by writing
/// to the caller-provided `out_ptr` (sret pattern to avoid >16 byte return).
///
/// `elem_size` is the byte size of each element.
/// `elem_inc_fn` increments child RCs of each copied element. Required because
/// the iterator's Drop will fire `elem_dec_fn` on the source buffer — without
/// `RcInc`, both source and collected buffer share children with only one RC.
#[no_mangle]
pub extern "C" fn ori_iter_collect(
    iter: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_collect");
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
    let mut data = crate::ori_rc_alloc(cap * es, 8);

    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        if len >= cap {
            let new_cap = cap * 2;
            let new_data = crate::ori_rc_realloc(data, cap * es, new_cap * es, 8);
            if new_data.is_null() {
                break;
            }
            data = new_data;
            cap = new_cap;
        }
        unsafe {
            let dst = data.add(len * es);
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), dst, es);
            // Increment child RCs so collected element survives iterator Drop
            if let Some(inc) = elem_inc_fn {
                inc(dst);
            }
        }
        len += 1;
    }

    // Store elem_count in the new buffer's header (codegen stores elem_dec_fn
    // after the collect call returns — it's an LLVM-generated thunk)
    if !data.is_null() {
        unsafe { crate::rc::store_elem_count(data, len as i64) };
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

// Collect Set

/// Collect all remaining elements into a new hash table set.
///
/// Uses `elem_hash`/`elem_eq` for hash-based deduplication during collection.
/// Returns a set `{ len: i64, cap: i64, data: *mut u8 }` by writing
/// to the caller-provided `out_ptr` (sret pattern). Duplicates are
/// skipped — only the first occurrence is kept.
///
/// `elem_inc_fn` increments child RCs of each inserted element. Required
/// because the iterator's Drop fires `elem_dec_fn` on the source buffer.
#[no_mangle]
pub extern "C" fn ori_iter_collect_set(
    iter: *mut u8,
    elem_size: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    elem_inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    use crate::map::hash_table::{
        needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_set, set_meta,
        HashTableLayout, META_OCCUPIED,
    };
    use crate::set::{alloc_set_hash_buffer, write_set_struct};
    assert_elem_size(elem_size, "ori_iter_collect_set");

    if iter.is_null() || out_ptr.is_null() {
        if !out_ptr.is_null() {
            write_set_struct(out_ptr, 0, 0, ptr::null_mut());
        }
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let es = elem_size.max(1) as usize;

    let mut cap = next_hash_capacity(0);
    let mut len: usize = 0;
    // elem_dec_fn stored by codegen after collect returns (LLVM-generated thunk)
    let mut data = alloc_set_hash_buffer(cap, es, None);

    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        let layout = HashTableLayout::for_set(cap, es);
        let hash = elem_hash(elem_buf.as_ptr());

        // Deduplicate via hash probe
        if unsafe {
            probe_find(
                data,
                cap,
                layout.keys_offset,
                elem_buf.as_ptr(),
                hash,
                es,
                elem_eq,
            )
        }
        .is_some()
        {
            continue;
        }

        // Rehash if load factor exceeded
        if needs_rehash(len + 1, cap) {
            let new_cap = cap * 2;
            // Read elem_dec_fn from old buffer (may be None if codegen hasn't stored yet)
            let old_dec = unsafe { crate::rc::load_elem_dec_fn(data) };
            let new_data = unsafe { rehash_set(data, cap, new_cap, es, elem_hash, old_dec) };
            crate::ori_rc_free(data, layout.total_size, 8);
            data = new_data;
            cap = new_cap;
        }

        // Find empty slot and insert
        let bucket = unsafe { probe_find_slot(data, cap, hash) };
        let layout = HashTableLayout::for_set(cap, es);
        unsafe {
            let dst = data.add(layout.keys_offset + bucket * es);
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), dst, es);
            set_meta(data, bucket, META_OCCUPIED);
            // Increment child RCs so collected element survives iterator Drop
            if let Some(inc) = elem_inc_fn {
                inc(dst);
            }
        }
        len += 1;
    }

    write_set_struct(out_ptr, len as i64, cap as i64, data);
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// Count

/// Count the remaining elements in the iterator, consuming it.
#[no_mangle]
pub extern "C" fn ori_iter_count(iter: *mut u8, elem_size: i64) -> i64 {
    assert_elem_size(elem_size, "ori_iter_count");
    if iter.is_null() {
        return 0;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut count: i64 = 0;
    let mut discard = ElemBuf::new();

    while unsafe { state.next(discard.as_mut_ptr(), elem_size) } {
        count += 1;
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
    count
}

// Any

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
    assert_elem_size(elem_size, "ori_iter_any");
    if iter.is_null() {
        return 0;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut elem_buf = ElemBuf::new();

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

// All

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
    assert_elem_size(elem_size, "ori_iter_all");
    if iter.is_null() {
        return 1; // vacuously true for empty
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut elem_buf = ElemBuf::new();

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

// Find

/// Find the first element satisfying the predicate, consuming the iterator.
///
/// Writes an `Option<T>` to `out_ptr`: `{ i64 tag, T payload }`.
/// Uses ARC enum convention: Some=0 (first variant), None=1 (second variant).
///
/// Layout matches LLVM codegen's `{i64, T}` enum representation.
#[no_mangle]
pub extern "C" fn ori_iter_find(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_find");
    if out_ptr.is_null() {
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
        }
        return;
    }

    if iter.is_null() {
        unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    // Payload at offset 8 (after i64 tag)
    let payload_ptr = unsafe { out_ptr.add(8) };
    let mut elem_buf = ElemBuf::new();

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

    unsafe {
        out_ptr.cast::<i64>().write(if found {
            OPTION_TAG_SOME
        } else {
            OPTION_TAG_NONE
        });
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// For Each

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
    assert_elem_size(elem_size, "ori_iter_for_each");
    if iter.is_null() {
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let mut elem_buf = ElemBuf::new();

    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        (each_fn)(each_env, elem_buf.as_ptr());
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// Fold

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
    assert_elem_size(elem_size, "ori_iter_fold");
    assert_elem_size(acc_size, "ori_iter_fold(acc)");
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
    let mut acc_a = ElemBuf::new();
    let mut acc_b = ElemBuf::new();
    let mut elem_buf = ElemBuf::new();

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

// Last

/// Return the last element of the iterator, consuming it.
///
/// Writes `Option<T>` to `out_ptr`: `{ i64 tag, T payload }`.
/// Tag convention: Some=0, None=1. Iterates forward keeping the last element.
#[no_mangle]
pub extern "C" fn ori_iter_last(iter: *mut u8, elem_size: i64, out_ptr: *mut u8) {
    assert_elem_size(elem_size, "ori_iter_last");
    if out_ptr.is_null() {
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
        }
        return;
    }

    if iter.is_null() {
        unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let payload_ptr = unsafe { out_ptr.add(8) };
    let mut elem_buf = ElemBuf::new();
    let mut found = false;

    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        unsafe {
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), payload_ptr, elem_size as usize);
        }
        found = true;
    }

    unsafe {
        out_ptr.cast::<i64>().write(if found {
            OPTION_TAG_SOME
        } else {
            OPTION_TAG_NONE
        });
    }
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// Join

/// Join iterator elements into a single string with separator.
///
/// Each element is expected to already be an `OriStr` (24 bytes).
/// The `to_str_fn` trampoline converts non-string elements to strings.
/// If `to_str_fn` is null, elements are assumed to be strings.
///
/// The separator is passed as its 3 struct fields (matching LLVM `{i64, i64, ptr}`
/// layout for `OriStr`). This is SSO-safe because the runtime reconstructs the
/// full `OriStr` union from the raw fields.
///
/// `elem_dec_fn` releases each CONSUMED element after its bytes are copied
/// into the result. Codegen passes it non-null only when it proves every
/// element reaching join is adapter-produced (consumer-owned, RC 1,
/// owned-by-nobody-else); it stays null for source-borrowed chains, whose
/// elements the source buffer's own cleanup releases. The runtime never
/// infers ownership — it only honors the verdict it was handed.
///
/// Writes the resulting `OriStr` to `out_ptr` (sret pattern, 24 bytes).
#[no_mangle]
pub extern "C" fn ori_iter_join(
    iter: *mut u8,
    sep_field0: i64,
    sep_field1: i64,
    sep_field2: *const u8,
    to_str_fn: Option<extern "C" fn(*mut u8, *const u8, *mut u8)>,
    to_str_env: *mut u8,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    use crate::string::OriStr;

    assert_elem_size(elem_size, "ori_iter_join");

    if out_ptr.is_null() {
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
        }
        return;
    }

    if iter.is_null() {
        // Empty iterator — return empty string (SSO empty)
        let empty = OriStr::from_bytes(b"");
        unsafe {
            ptr::copy_nonoverlapping(
                std::ptr::from_ref::<OriStr>(&empty).cast::<u8>(),
                out_ptr,
                std::mem::size_of::<OriStr>(),
            );
        }
        let _ = empty;
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    // Reconstruct OriStr from the 3 struct fields passed by LLVM codegen.
    // The fields are the raw bits of {len, cap, data} — for SSO strings these
    // are the inline bytes reinterpreted through the heap layout. OriStr's
    // is_sso() detects SSO by checking byte 23 (the MSB of the data pointer
    // in heap layout, or the flags byte in SSO layout).
    // NOTE: SSO discrimination relies on canonical 64-bit addressing (pointer
    // MSB clear for heap pointers). This is true on all current targets
    // (x86_64, aarch64) but would break on exotic architectures with
    // high-bit-set user-space pointers.
    let sep_str = OriStr {
        heap: crate::string::OriStrHeap {
            len: sep_field0,
            cap: sep_field1,
            data: sep_field2 as *mut u8,
        },
    };
    // Borrow directly — no allocation needed. SSO bytes are inline in
    // sep_str (stack-local), heap bytes are RC-managed by the caller.
    // Both outlive the loop below.
    // SAFETY: sep_str was reconstructed from valid OriStr fields passed
    // by codegen; the data pointer (heap) or inline bytes (SSO) are valid.
    let sep = unsafe { sep_str.as_str() };

    let mut result = String::new();
    let mut elem_buf = ElemBuf::new();
    let mut first = true;

    // SAFETY: `state` is a live `IterState` (constructed by codegen, freed
    // below), `elem_buf` is 16-byte aligned (`ElemBuf` repr, covers OriStr's
    // 8-byte alignment), and `elem_size` was asserted `<= MAX_ELEM_SIZE`; each
    // `next` writes at most `elem_size` bytes into the buffer.
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        if !first {
            result.push_str(sep);
        }

        // Convert element to string via trampoline or direct string read.
        if let Some(to_str) = to_str_fn {
            // `MaybeUninit<OriStr>` gives OriStr's 8-byte alignment, so the
            // trampoline's sret store lands aligned (a bare `[u8; N]` would be
            // align-1 — a misaligned OriStr write is UB).
            let mut str_buf = std::mem::MaybeUninit::<OriStr>::uninit();
            (to_str)(
                to_str_env,
                elem_buf.as_ptr(),
                str_buf.as_mut_ptr().cast::<u8>(),
            );
            // SAFETY: the trampoline wrote a complete OriStr to the aligned
            // buffer; OriStr is Copy (no Drop), so reading it out is sound.
            let s = unsafe { str_buf.assume_init() };
            // SAFETY: `s` is a valid OriStr whose data pointer (heap) or inline
            // bytes (SSO) are valid for the borrow's lifetime (until the next
            // loop iteration overwrites `str_buf`, after `push_str` copies out).
            result.push_str(unsafe { s.as_str() });
            // Free the temporary OriStr created by the trampoline.
            // OriStr is Copy (no Drop), so heap-backed temporaries leak without
            // explicit cleanup. ori_str_rc_dec handles SSO (no-op) and slice
            // encoding. ori_str_drop_buffer reads the size from the RC header
            // and calls ori_rc_free to deallocate.
            // SAFETY: `s.heap.data`/`cap` are the trampoline-produced buffer's
            // fields; releasing exactly the value this branch produced.
            unsafe {
                crate::rc::ori_str_rc_dec(
                    s.heap.data,
                    s.heap.cap,
                    Some(crate::rc::ori_str_drop_buffer),
                );
            }
            // Release the consumed INPUT element — a SEPARATE obligation from
            // the produced-string dec above. Null for scalar element types by
            // construction, so the int/float/bool path is behavior-unchanged.
            // `dec` is codegen's element-type-matched release thunk over the
            // (aligned) `elem_buf`.
            if let Some(dec) = elem_dec_fn {
                (dec)(elem_buf.as_mut_ptr());
            }
        } else {
            // Element is already an OriStr (24 bytes).
            // SAFETY: `elem_buf` holds a complete OriStr (str-element chain);
            // read_unaligned tolerates the buffer's storage layout. OriStr is
            // Copy, so the read-out is sound.
            let s = unsafe { ptr::read_unaligned(elem_buf.as_ptr().cast::<OriStr>()) };
            // SAFETY: `s` is a valid OriStr; its bytes are copied out by
            // push_str before the buffer is reused.
            result.push_str(unsafe { s.as_str() });
            // Release the consumed element after its bytes were copied into
            // the accumulator. Non-null only for chains codegen proved
            // adapter-produced; source-borrowed chains pass null (the source
            // buffer's cleanup owns those elements).
            if let Some(dec) = elem_dec_fn {
                (dec)(elem_buf.as_mut_ptr());
            }
        }

        first = false;
    }

    let ori_str = OriStr::from_owned(&result);
    unsafe {
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<OriStr>(&ori_str).cast::<u8>(),
            out_ptr,
            std::mem::size_of::<OriStr>(),
        );
    }
    let _ = ori_str;

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// Rfold

/// Fold the iterator from right-to-left, consuming it.
///
/// Collects all elements into a buffer, then folds right-to-left.
/// This is the simplest correct implementation without DEI runtime support.
#[no_mangle]
pub extern "C" fn ori_iter_rfold(
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
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
        }
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

    let state = unsafe { &mut *iter.cast::<IterState>() };

    // Collect all elements into a Vec
    let mut elements: Vec<u8> = Vec::new();
    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        elements.extend_from_slice(&elem_buf[..es]);
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });

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
pub extern "C" fn ori_iter_rfind(
    iter: *mut u8,
    pred_fn: PredicateFn,
    pred_env: *mut u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_rfind");
    if out_ptr.is_null() {
        if !iter.is_null() {
            drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
        }
        return;
    }

    if iter.is_null() {
        unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
        return;
    }

    let state = unsafe { &mut *iter.cast::<IterState>() };
    let es = elem_size.max(1) as usize;

    // Collect all elements
    let mut elements: Vec<u8> = Vec::new();
    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        elements.extend_from_slice(&elem_buf[..es]);
    }

    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });

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
