//! Runtime iterator support for AOT-compiled Ori programs.
//!
//! Provides an opaque iterator handle that LLVM code manipulates via
//! `extern "C"` functions. The internal `IterState` enum is never exposed
//! to LLVM — all interaction goes through pointer-sized handles.
//!
//! # Architecture
//!
//! - LLVM sees iterators as `ptr` (opaque handle)
//! - Each `ori_iter_*` function takes/returns `ptr` handles
//! - Adapters (map, filter) accept trampoline function pointers that bridge
//!   typed closures to the runtime's generic `(env, in_ptr, out_ptr)` ABI
//! - `ori_iter_drop` frees the handle and any captured environment pointers

use std::ptr;

/// Maximum element size for stack scratch buffers in `next()`.
///
/// Covers all current Ori types. Asserted at adapter creation time.
const MAX_ELEM_SIZE: usize = 256;

// ── Internal state (never exposed to LLVM) ──────────────────────────────

/// Iterator state machine. Each variant corresponds to an iterator source
/// or adapter from the evaluator's `IteratorValue` enum.
enum IterState {
    /// Iterates over a contiguous array of elements (list data buffer).
    List {
        data: *const u8,
        len: i64,
        pos: i64,
        elem_size: i64,
    },

    /// Iterates over an integer range with step.
    Range {
        current: i64,
        end: i64,
        step: i64,
        inclusive: bool,
    },

    /// Transforms each element via a trampoline function.
    Mapped {
        source: Box<IterState>,
        transform_fn: TransformFn,
        transform_env: *mut u8,
        in_size: i64,
    },

    /// Filters elements via a predicate trampoline.
    Filtered {
        source: Box<IterState>,
        predicate_fn: PredicateFn,
        predicate_env: *mut u8,
        elem_size: i64,
    },

    /// Takes at most N elements from source.
    TakeN {
        source: Box<IterState>,
        remaining: i64,
    },

    /// Skips N elements then delegates to source.
    SkipN {
        source: Box<IterState>,
        remaining: i64,
    },

    /// Wraps each element with its index: (index, element).
    Enumerated { source: Box<IterState>, index: i64 },
}

/// Trampoline signature for map: `(env, in_ptr, out_ptr) -> void`
type TransformFn = extern "C" fn(*mut u8, *const u8, *mut u8);

/// Trampoline signature for filter: `(env, elem_ptr) -> bool`
type PredicateFn = extern "C" fn(*mut u8, *const u8) -> bool;

// ── IterState::next() ───────────────────────────────────────────────────

impl IterState {
    /// Advance the iterator, writing the next element to `out_ptr`.
    ///
    /// Returns `true` if an element was produced, `false` if exhausted.
    ///
    /// # Safety
    ///
    /// `out_ptr` must be valid for `elem_size` bytes (varies by variant).
    unsafe fn next(&mut self, out_ptr: *mut u8, elem_size: i64) -> bool {
        match self {
            Self::List {
                data,
                len,
                pos,
                elem_size: es,
            } => Self::next_list(*data, *len, pos, *es, out_ptr),
            Self::Range {
                current,
                end,
                step,
                inclusive,
            } => Self::next_range(current, *end, *step, *inclusive, out_ptr),
            Self::Mapped {
                source,
                transform_fn,
                transform_env,
                in_size,
            } => Self::next_mapped(source, *transform_fn, *transform_env, *in_size, out_ptr),
            Self::Filtered {
                source,
                predicate_fn,
                predicate_env,
                elem_size: es,
            } => Self::next_filtered(source, *predicate_fn, *predicate_env, *es, out_ptr),
            Self::TakeN { source, remaining } => {
                Self::next_take(source, remaining, elem_size, out_ptr)
            }
            Self::SkipN { source, remaining } => {
                Self::next_skip(source, remaining, elem_size, out_ptr)
            }
            Self::Enumerated { source, index } => {
                Self::next_enumerated(source, index, elem_size, out_ptr)
            }
        }
    }

    unsafe fn next_list(
        data: *const u8,
        len: i64,
        pos: &mut i64,
        es: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if *pos >= len {
            return false;
        }
        let offset = *pos * es;
        ptr::copy_nonoverlapping(data.add(offset as usize), out_ptr, es as usize);
        *pos += 1;
        true
    }

    unsafe fn next_range(
        current: &mut i64,
        end: i64,
        step: i64,
        inclusive: bool,
        out_ptr: *mut u8,
    ) -> bool {
        let in_bounds = if inclusive {
            if step > 0 {
                *current <= end
            } else {
                *current >= end
            }
        } else if step > 0 {
            *current < end
        } else {
            *current > end
        };
        if !in_bounds {
            return false;
        }
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i64>(current).cast::<u8>(),
            out_ptr,
            size_of::<i64>(),
        );
        *current += step;
        true
    }

    unsafe fn next_mapped(
        source: &mut IterState,
        transform_fn: TransformFn,
        transform_env: *mut u8,
        in_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let mut scratch = [0u8; MAX_ELEM_SIZE];
        if !source.next(scratch.as_mut_ptr(), in_size) {
            return false;
        }
        (transform_fn)(transform_env, scratch.as_ptr(), out_ptr);
        true
    }

    unsafe fn next_filtered(
        source: &mut IterState,
        predicate_fn: PredicateFn,
        predicate_env: *mut u8,
        es: i64,
        out_ptr: *mut u8,
    ) -> bool {
        loop {
            if !source.next(out_ptr, es) {
                return false;
            }
            if (predicate_fn)(predicate_env, out_ptr) {
                return true;
            }
        }
    }

    unsafe fn next_take(
        source: &mut IterState,
        remaining: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if *remaining <= 0 {
            return false;
        }
        if !source.next(out_ptr, elem_size) {
            *remaining = 0;
            return false;
        }
        *remaining -= 1;
        true
    }

    unsafe fn next_skip(
        source: &mut IterState,
        remaining: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        while *remaining > 0 {
            let mut discard = [0u8; MAX_ELEM_SIZE];
            if !source.next(discard.as_mut_ptr(), elem_size) {
                *remaining = 0;
                return false;
            }
            *remaining -= 1;
        }
        source.next(out_ptr, elem_size)
    }

    unsafe fn next_enumerated(
        source: &mut IterState,
        index: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        // Layout: first 8 bytes = index, then elem_size - 8 bytes = element
        let inner_size = elem_size - size_of::<i64>() as i64;
        if inner_size < 0 {
            return false;
        }
        let elem_ptr = out_ptr.add(size_of::<i64>());
        if !source.next(elem_ptr, inner_size) {
            return false;
        }
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i64>(index).cast::<u8>(),
            out_ptr,
            size_of::<i64>(),
        );
        *index += 1;
        true
    }
}

// ── Extern C API ────────────────────────────────────────────────────────

/// Create an iterator over a list's data buffer.
///
/// `data` points to the list's contiguous element storage.
/// `len` is the number of elements. `elem_size` is bytes per element.
/// The iterator borrows the data — the list must outlive the iterator.
#[no_mangle]
pub extern "C" fn ori_iter_from_list(data: *const u8, len: i64, elem_size: i64) -> *mut u8 {
    let state = IterState::List {
        data,
        len,
        pos: 0,
        elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create an iterator over an integer range.
///
/// Iterates from `start` to `end` with step `step`.
/// If `inclusive` is true, the range includes `end`.
#[no_mangle]
pub extern "C" fn ori_iter_from_range(start: i64, end: i64, step: i64, inclusive: bool) -> *mut u8 {
    let state = IterState::Range {
        current: start,
        end,
        step: if step == 0 { 1 } else { step },
        inclusive,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Advance the iterator, writing the next element to `out_ptr`.
///
/// Returns 1 if an element was produced, 0 if the iterator is exhausted.
/// `elem_size` must match the element size of the iterator's output type.
#[no_mangle]
pub extern "C" fn ori_iter_next(iter: *mut u8, out_ptr: *mut u8, elem_size: i64) -> i8 {
    if iter.is_null() || out_ptr.is_null() {
        return 0;
    }
    let state = unsafe { &mut *iter.cast::<IterState>() };
    let has_next = unsafe { state.next(out_ptr, elem_size) };
    i8::from(has_next)
}

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

    // Drop the iterator
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
    count
}

/// Drop (free) an iterator handle and all its internal state.
///
/// Must be called when the iterator is no longer needed to prevent leaks.
/// Called automatically at the end of for-loops over iterators.
#[no_mangle]
pub extern "C" fn ori_iter_drop(iter: *mut u8) {
    if iter.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(iter.cast::<IterState>()) });
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
