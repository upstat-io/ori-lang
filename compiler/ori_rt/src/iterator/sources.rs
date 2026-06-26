//! Iterator source constructors — create iterators from data sources.
//!
//! These are `extern "C"` functions called from LLVM-generated code to create
//! iterators over lists, ranges, strings, and maps.

use super::state::{assert_elem_size, IterState};

/// Create an iterator over a list's data buffer.
///
/// `data` points to the list's contiguous RC-managed element storage.
/// `len` is the number of elements. `cap` is the buffer capacity.
/// `elem_size` is bytes per element.
///
/// The iterator takes ownership of one RC reference to `data`. When the
/// iterator is dropped (by a consumer function or `ori_iter_drop`),
/// `Drop for IterState` calls `ori_buffer_rc_dec` to release the reference.
/// Element cleanup is entirely header-based: `ori_buffer_rc_dec` reads the
/// `elem_dec_fn` from the V5 RC header at cleanup time.
///
/// `owns_data` records the ARC `@iter` arg-ownership: `true` when the iterator
/// received its own RC reference (moved/inc'd source), `false` when the source
/// is borrowed-co-owned (the flatten inner `sub.iter()` runs inside an opaque
/// map trampoline so the ARC pipeline cannot inc; the outer container retains
/// the single RC). `Drop` decs only when `owns_data`. The ARC pipeline emits
/// `RcInc` before `.iter()` when the list has additional liveness (the inc
/// gives the iterator its own reference, so `owns_data` is `true`).
///
/// For Rust unit tests with stack-allocated data, pass `cap = 0` — no cleanup
/// is performed on drop regardless of `owns_data`.
#[no_mangle]
pub extern "C" fn ori_iter_from_list(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    owns_data: bool,
) -> *mut u8 {
    assert_elem_size(elem_size, "ori_iter_from_list");
    let state = IterState::List {
        data,
        len,
        pos: 0,
        cap,
        elem_size,
        owns_data,
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

/// Compute the element count of a bounded integer range.
///
/// Mirrors `RangeValue::len`: a zero step yields 0; otherwise the span from
/// `start` to the inclusivity-adjusted `end` is ceiling-divided by `|step|`.
/// The `.len()` lowering reaches this only for bounded ranges.
#[no_mangle]
pub extern "C" fn ori_range_len(start: i64, end: i64, step: i64, inclusive: bool) -> i64 {
    if step == 0 {
        return 0;
    }
    let adjusted_end = if inclusive {
        if step > 0 {
            end.saturating_add(1)
        } else {
            end.saturating_sub(1)
        }
    } else {
        end
    };
    let diff = if step > 0 {
        adjusted_end.saturating_sub(start).max(0)
    } else {
        start.saturating_sub(adjusted_end).max(0)
    };
    // step != 0, so |step| >= 1 and the division below is well-defined.
    let step_abs = step.saturating_abs();
    diff.saturating_add(step_abs).saturating_sub(1) / step_abs
}

/// Test membership of `value` in a bounded integer range.
///
/// Mirrors `RangeValue::contains`: a zero step contains nothing; otherwise the
/// value must lie within the directional bounds and be step-aligned with
/// `start`.
#[no_mangle]
pub extern "C" fn ori_range_contains(
    start: i64,
    end: i64,
    step: i64,
    inclusive: bool,
    value: i64,
) -> bool {
    if step == 0 {
        return false;
    }
    let in_bounds = if step > 0 {
        let upper_ok = if inclusive { value <= end } else { value < end };
        value >= start && upper_ok
    } else {
        let upper_ok = if inclusive { value >= end } else { value > end };
        value <= start && upper_ok
    };
    if !in_bounds {
        return false;
    }
    // step != 0, so the remainder is well-defined.
    value.wrapping_sub(start) % step == 0
}

/// Create an iterator over a UTF-8 string, yielding Unicode codepoints.
///
/// Takes a pointer to an `OriStr` (SSO-safe). For heap strings, the iterator
/// takes an RC reference via `ori_str_rc_inc` (slice-aware). For SSO strings,
/// the inline bytes are copied to a heap buffer so the iterator outlives the
/// source `OriStr`.
#[no_mangle]
pub extern "C" fn ori_iter_from_str(s: *const crate::OriStr) -> *mut u8 {
    if s.is_null() {
        let state = IterState::Str {
            data: std::ptr::null_mut(),
            len: 0,
            cap: 0,
            byte_offset: 0,
            owns_data: false,
        };
        return Box::into_raw(Box::new(state)).cast();
    }
    // SAFETY: s validated non-null above; caller guarantees valid OriStr pointer.
    let str_ref = unsafe { &*s };
    let len = str_ref.len() as i64;

    if str_ref.is_sso() {
        // SSO: copy inline bytes to a heap buffer (the source OriStr may be on
        // the stack, so the inline data won't survive beyond the caller's frame).
        if len <= 0 {
            let state = IterState::Str {
                data: std::ptr::null_mut(),
                len: 0,
                cap: 0,
                byte_offset: 0,
                owns_data: false,
            };
            return Box::into_raw(Box::new(state)).cast();
        }
        let size = len as usize;
        let heap_copy = crate::ori_rc_alloc(size, 1);
        // SAFETY: str_ref is SSO with len > 0; heap_copy is freshly allocated with `size` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(str_ref.sso.bytes.as_ptr(), heap_copy, size);
        }
        let state = IterState::Str {
            data: heap_copy,
            len,
            cap: len,
            byte_offset: 0,
            owns_data: true,
        };
        Box::into_raw(Box::new(state)).cast()
    } else {
        // Heap: take an RC reference via slice-aware ori_str_rc_inc.
        // For slice strings from str.split(), data is an interior pointer
        // and cap has SLICE_FLAG set — ori_str_rc_inc finds the original
        // buffer and increments that.
        // SAFETY: str_ref.is_sso() is false, so the heap union variant is active.
        let data = unsafe { str_ref.heap.data };
        // SAFETY: Same as above — heap union variant is active.
        let cap = unsafe { str_ref.heap.cap };
        if !data.is_null() {
            crate::ori_str_rc_inc(data, cap);
        }
        let state = IterState::Str {
            data,
            len,
            cap,
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
    assert_elem_size(key_size + val_size, "ori_iter_from_map (key+val)");
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

/// Create an infinite iterator that yields copies of `value`.
///
/// Mirrors the interpreter's `IteratorValue::from_repeat`. Copies `elem_size`
/// bytes from `value_ptr` into the master buffer. The CALLER (codegen) inc'd
/// the value's RC once before this call, so the master owns one reference and
/// stays live independent of the (borrowed) source binding; `Drop` releases it
/// once via `elem_dec_fn` (null for scalar elements). Each `next()` yields a
/// bitwise copy WITHOUT incrementing — the consumer's per-yield inc covers the
/// yielded aliases (identical to the `Cycled` / `ori_iter_from_option`
/// protocol). The result is infinite; bound it with `.take(n)` before any
/// eager consumer.
#[no_mangle]
pub extern "C" fn ori_iter_repeat(
    value_ptr: *const u8,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) -> *mut u8 {
    assert_elem_size(elem_size, "ori_iter_repeat");
    let size = elem_size.max(0) as usize;
    let mut value = vec![0u8; size];
    if !value_ptr.is_null() && size > 0 {
        // SAFETY: caller guarantees `value_ptr` is valid for `size` bytes;
        // `value` is freshly allocated with `size` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(value_ptr, value.as_mut_ptr(), size);
        }
    }
    let state = IterState::Repeat {
        value,
        elem_size,
        elem_dec_fn,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create an iterator from an `Option<T>`.
///
/// Mirrors the interpreter's `IteratorValue::from_value()` for Option:
/// `Some(v)` → 1-element list iterator, `None` → empty list iterator.
///
/// For `Some`: allocates a 1-element RC buffer, copies `elem_size` bytes
/// from `payload_ptr`, stores `elem_dec_fn` and `elem_count=1` in the
/// V5 header. The iterator owns this buffer (refcount starts at 1).
///
/// For `None`: creates an empty `IterState::List` with no allocation.
#[no_mangle]
pub extern "C" fn ori_iter_from_option(
    is_some: bool,
    payload_ptr: *const u8,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) -> *mut u8 {
    assert_elem_size(elem_size, "ori_iter_from_option");
    if is_some && !payload_ptr.is_null() {
        let size = elem_size as usize;
        let data = crate::ori_rc_alloc(size, 8);
        // SAFETY: ori_rc_alloc returned valid allocation of `size` bytes.
        // store_elem_dec_fn/store_elem_count write to V5 header fields at
        // negative offsets from the data pointer, which ori_rc_alloc guarantees.
        unsafe {
            std::ptr::copy_nonoverlapping(payload_ptr, data, size);
            crate::store_elem_dec_fn(data, elem_dec_fn);
            crate::store_elem_count(data, 1);
        }
        let state = IterState::List {
            data,
            len: 1,
            pos: 0,
            cap: 1,
            elem_size,
            owns_data: true,
        };
        Box::into_raw(Box::new(state)).cast()
    } else {
        let state = IterState::List {
            data: std::ptr::null_mut(),
            len: 0,
            pos: 0,
            cap: 0,
            elem_size,
            owns_data: true,
        };
        Box::into_raw(Box::new(state)).cast()
    }
}
