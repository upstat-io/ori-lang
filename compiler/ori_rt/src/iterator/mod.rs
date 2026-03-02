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
//!
//! # Submodules
//!
//! - `consumers`: Terminal operations (collect, count, fold, find, any, all, `for_each`)

mod consumers;

// Re-export consumer functions at module level (they're `#[no_mangle] extern "C"`)
pub use consumers::{
    ori_iter_all, ori_iter_any, ori_iter_collect, ori_iter_collect_set, ori_iter_count,
    ori_iter_find, ori_iter_fold, ori_iter_for_each,
};

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
    ///
    /// When `cap > 0`, the iterator owns a reference to the RC-managed data
    /// buffer. `Drop` calls `ori_buffer_rc_dec` to release it. When `cap == 0`
    /// (e.g., Rust unit tests with stack data), no cleanup is performed.
    List {
        data: *mut u8,
        len: i64,
        pos: i64,
        cap: i64,
        elem_size: i64,
        elem_dec_fn: Option<extern "C" fn(*mut u8)>,
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

    /// Zips two iterators, yielding `(left_elem, right_elem)` tuples.
    Zipped {
        left: Box<IterState>,
        right: Box<IterState>,
        left_elem_size: i64,
    },

    /// Chains two iterators — yields all of first, then all of second.
    Chained {
        first: Box<IterState>,
        second: Box<IterState>,
        first_done: bool,
    },

    /// Iterates over a UTF-8 string, yielding Unicode codepoints (i32/char).
    ///
    /// When `owns_data` is true, the iterator holds an RC reference to the
    /// string data and `Drop` calls `ori_buffer_rc_dec` to release it (dec
    /// refcount + free when rc reaches 0). When false (e.g., Rust unit
    /// tests), no cleanup is performed.
    Str {
        data: *mut u8,
        len: i64,
        byte_offset: i64,
        owns_data: bool,
    },

    /// Iterates over a map's key-value pairs, yielding `(key, value)` tuples.
    ///
    /// Data layout (hash table): `[metadata | keys | values]`. The iterator
    /// scans metadata for OCCUPIED buckets. `pos` is the current bucket index.
    /// When `owns_data` is true, the iterator holds an RC reference to the
    /// combined data buffer. `Drop` calls `ori_map_buffer_rc_dec` to clean up.
    Map {
        data: *mut u8,
        cap: i64,
        len: i64,
        pos: i64,
        key_size: i64,
        val_size: i64,
        owns_data: bool,
        key_dec_fn: Option<extern "C" fn(*mut u8)>,
        val_dec_fn: Option<extern "C" fn(*mut u8)>,
    },
}

impl Drop for IterState {
    fn drop(&mut self) {
        // Release RC references to data owned by source-level iterators.
        //
        // The ARC pipeline transfers ownership of one RC reference to the
        // iterator when `.iter()` is called (Owned semantics). We release
        // it here so the data is freed when the last iterator reference
        // goes away.
        //
        // For adapter variants (Mapped, Filtered, etc.), Rust automatically
        // drops the inner `Box<IterState>` AFTER this `drop()` returns,
        // cascading the cleanup to the source iterator.
        match self {
            IterState::List {
                data,
                len,
                cap,
                elem_size,
                elem_dec_fn,
                ..
            } => {
                // cap > 0 indicates RC-managed data (from the compiler).
                // cap == 0 indicates test data (stack-allocated, no cleanup).
                if !data.is_null() && *cap > 0 {
                    crate::ori_buffer_rc_dec(*data, *len, *cap, *elem_size, *elem_dec_fn);
                }
            }
            IterState::Str {
                data,
                len,
                owns_data,
                ..
            } => {
                // String data is allocated via ori_rc_alloc (in ori_str_from_raw),
                // so we must use ori_buffer_rc_dec to both dec the refcount AND
                // free the memory when rc reaches 0. ori_rc_dec alone only decs
                // the refcount without freeing.
                // len=0 (no inner RC elements), cap=string byte length, elem_size=1.
                if *owns_data && !data.is_null() {
                    crate::ori_buffer_rc_dec(*data, 0, *len, 1, None);
                }
            }
            IterState::Map {
                data,
                cap,
                len,
                key_size,
                val_size,
                owns_data,
                key_dec_fn,
                val_dec_fn,
                ..
            } => {
                // Map data buffer uses hash table layout [metadata|keys|values].
                // ori_map_buffer_rc_dec decs the refcount, scans metadata for
                // OCCUPIED buckets to clean up key/value children, and frees
                // the buffer when rc reaches 0.
                if *owns_data && !data.is_null() {
                    crate::ori_map_buffer_rc_dec(
                        *data,
                        *cap,
                        *len,
                        *key_size,
                        *val_size,
                        *key_dec_fn,
                        *val_dec_fn,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Trampoline signature for map: `(env, in_ptr, out_ptr) -> void`
type TransformFn = extern "C" fn(*mut u8, *const u8, *mut u8);

/// Trampoline signature for filter/any/all/find: `(env, elem_ptr) -> bool`
type PredicateFn = extern "C" fn(*mut u8, *const u8) -> bool;

/// Trampoline signature for `for_each`: `(env, elem_ptr) -> void`
type ForEachFn = extern "C" fn(*mut u8, *const u8);

/// Trampoline signature for fold: `(env, acc_ptr, elem_ptr, out_ptr) -> void`
type FoldFn = extern "C" fn(*mut u8, *const u8, *const u8, *mut u8);

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
                ..
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
            Self::Zipped {
                left,
                right,
                left_elem_size,
            } => Self::next_zipped(left, right, *left_elem_size, elem_size, out_ptr),
            Self::Chained {
                first,
                second,
                first_done,
            } => Self::next_chained(first, second, first_done, elem_size, out_ptr),
            Self::Str {
                data,
                len,
                byte_offset,
                ..
            } => Self::next_str(*data, *len, byte_offset, out_ptr),
            Self::Map {
                data,
                cap,
                len,
                pos,
                key_size,
                val_size,
                ..
            } => Self::next_map(*data, *cap, *len, pos, *key_size, *val_size, out_ptr),
        }
    }

    unsafe fn next_list(data: *mut u8, len: i64, pos: &mut i64, es: i64, out_ptr: *mut u8) -> bool {
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

    /// Zip: advance both iterators, copy left then right to output.
    ///
    /// Output layout: `[left_elem_bytes | right_elem_bytes]`.
    /// Total output size is `elem_size` (= `left_elem_size` + `right_elem_size`).
    unsafe fn next_zipped(
        left: &mut IterState,
        right: &mut IterState,
        left_elem_size: i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let right_elem_size = elem_size - left_elem_size;
        // Advance left into front of output buffer
        if !left.next(out_ptr, left_elem_size) {
            return false;
        }
        // Advance right into back of output buffer
        let right_ptr = out_ptr.add(left_elem_size as usize);
        if !right.next(right_ptr, right_elem_size) {
            return false;
        }
        true
    }

    /// Chain: yield all of first iterator, then all of second.
    unsafe fn next_chained(
        first: &mut IterState,
        second: &mut IterState,
        first_done: &mut bool,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if !*first_done {
            if first.next(out_ptr, elem_size) {
                return true;
            }
            *first_done = true;
        }
        second.next(out_ptr, elem_size)
    }

    /// Str: decode the next UTF-8 codepoint and write it as i32.
    ///
    /// Output is a single `i32` (4 bytes) — the Unicode scalar value.
    unsafe fn next_str(data: *mut u8, len: i64, byte_offset: &mut i64, out_ptr: *mut u8) -> bool {
        if data.is_null() || *byte_offset >= len {
            return false;
        }
        let result = crate::ori_str_next_char(data, len, *byte_offset);
        if result.codepoint < 0 {
            *byte_offset = len;
            return false;
        }
        let cp = result.codepoint;
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i32>(&cp).cast::<u8>(),
            out_ptr,
            size_of::<i32>(),
        );
        *byte_offset = result.next_offset;
        true
    }

    /// Map: yield the next `(key, value)` pair.
    ///
    /// Data layout (hash table): `[metadata | keys | values]`.
    /// Scans metadata starting at bucket `pos` for the next OCCUPIED entry.
    /// Output layout: `[key_bytes | value_bytes]` (concatenated).
    unsafe fn next_map(
        data: *mut u8,
        cap: i64,
        _len: i64,
        pos: &mut i64,
        key_size: i64,
        val_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let c = cap as usize;
        let ks = key_size as usize;
        let vs = val_size as usize;
        let layout = crate::map::hash_table::HashTableLayout::for_map(c, ks, vs);

        while (*pos as usize) < c {
            let bucket = *pos as usize;
            *pos += 1;
            if crate::map::hash_table::get_meta(data, bucket)
                == crate::map::hash_table::META_OCCUPIED
            {
                let key_ptr = data.add(layout.keys_offset + bucket * ks);
                let val_ptr = data.add(layout.vals_offset + bucket * vs);
                ptr::copy_nonoverlapping(key_ptr, out_ptr, ks);
                ptr::copy_nonoverlapping(val_ptr, out_ptr.add(ks), vs);
                return true;
            }
        }
        false
    }
}

// ── Extern C API — Constructors ─────────────────────────────────────────

/// Create an iterator over a list's data buffer.
///
/// `data` points to the list's contiguous RC-managed element storage.
/// `len` is the number of elements. `cap` is the buffer capacity.
/// `elem_size` is bytes per element. `elem_dec_fn` is the per-element
/// RC cleanup function (null for scalar elements).
///
/// The iterator takes ownership of one RC reference to `data`. When the
/// iterator is dropped (by a consumer function or `ori_iter_drop`),
/// `Drop for IterState` calls `ori_buffer_rc_dec` to release the reference.
///
/// For Rust unit tests with stack-allocated data, pass `cap = 0` and
/// `elem_dec_fn = None` — no cleanup is performed on drop.
#[no_mangle]
pub extern "C" fn ori_iter_from_list(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) -> *mut u8 {
    let state = IterState::List {
        data,
        len,
        pos: 0,
        cap,
        elem_size,
        elem_dec_fn,
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

/// Create an iterator over a UTF-8 string, yielding Unicode codepoints.
///
/// Takes a pointer to an `OriStr` (SSO-safe). For heap strings, the iterator
/// takes an RC reference to the data pointer. For SSO strings, the inline bytes
/// are copied to a heap buffer so the iterator outlives the source `OriStr`.
#[no_mangle]
pub extern "C" fn ori_iter_from_str(s: *const crate::OriStr) -> *mut u8 {
    if s.is_null() {
        let state = IterState::Str {
            data: std::ptr::null_mut(),
            len: 0,
            byte_offset: 0,
            owns_data: false,
        };
        return Box::into_raw(Box::new(state)).cast();
    }
    let str_ref = unsafe { &*s };
    let len = str_ref.len() as i64;

    if str_ref.is_sso() {
        // SSO: copy inline bytes to a heap buffer (the source OriStr may be on
        // the stack, so the inline data won't survive beyond the caller's frame).
        if len <= 0 {
            let state = IterState::Str {
                data: std::ptr::null_mut(),
                len: 0,
                byte_offset: 0,
                owns_data: false,
            };
            return Box::into_raw(Box::new(state)).cast();
        }
        let size = len as usize;
        let heap_copy = crate::ori_rc_alloc(size, 1);
        unsafe {
            std::ptr::copy_nonoverlapping(str_ref.sso.bytes.as_ptr(), heap_copy, size);
        }
        let state = IterState::Str {
            data: heap_copy,
            len,
            byte_offset: 0,
            owns_data: true,
        };
        Box::into_raw(Box::new(state)).cast()
    } else {
        // Heap: take an RC reference to the existing data pointer.
        let data = unsafe { str_ref.heap.data };
        if !data.is_null() {
            crate::ori_rc_inc(data);
        }
        let state = IterState::Str {
            data,
            len,
            byte_offset: 0,
            owns_data: true,
        };
        Box::into_raw(Box::new(state)).cast()
    }
}

/// Create an iterator over a map's key-value pairs.
///
/// `data` points to the map's hash table buffer `[metadata|keys|values]`.
/// `cap` is the number of buckets, `len` is the number of entries.
/// `key_size`/`val_size` are bytes per key/value.
/// Each element yielded is `[key_bytes | val_bytes]` (concatenated).
#[no_mangle]
pub extern "C" fn ori_iter_from_map(
    data: *mut u8,
    cap: i64,
    len: i64,
    key_size: i64,
    val_size: i64,
    owns_data: bool,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
) -> *mut u8 {
    let state = IterState::Map {
        data,
        cap,
        len,
        pos: 0,
        key_size,
        val_size,
        owns_data,
        key_dec_fn,
        val_dec_fn,
    };
    Box::into_raw(Box::new(state)).cast()
}

// ── Extern C API — Core ─────────────────────────────────────────────────

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

// ── Extern C API — Adapters ─────────────────────────────────────────────

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
            ori_iter_drop(left);
        }
        if !right.is_null() {
            ori_iter_drop(right);
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
        // Empty range as placeholder
        Box::new(IterState::Range {
            current: 0,
            end: 0,
            step: 1,
            inclusive: false,
        })
    } else {
        unsafe { Box::from_raw(first.cast::<IterState>()) }
    };
    let second_state = if second.is_null() {
        Box::new(IterState::Range {
            current: 0,
            end: 0,
            step: 1,
            inclusive: false,
        })
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

// ── Extern C API — Cleanup ──────────────────────────────────────────────

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

#[cfg(test)]
mod tests;
