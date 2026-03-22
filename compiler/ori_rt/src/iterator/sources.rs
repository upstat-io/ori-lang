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
/// The ARC pipeline is responsible for emitting `RcInc` before calling
/// `.iter()` when the list variable has additional liveness (the inc gives
/// the iterator its own reference). Dead list parameters in exit blocks
/// are cleaned up by `emit_dead_at_entry_decs`.
///
/// For Rust unit tests with stack-allocated data, pass `cap = 0` — no
/// cleanup is performed on drop.
#[no_mangle]
pub extern "C" fn ori_iter_from_list(data: *mut u8, len: i64, cap: i64, elem_size: i64) -> *mut u8 {
    assert_elem_size(elem_size, "ori_iter_from_list");
    let state = IterState::List {
        data,
        len,
        pos: 0,
        cap,
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
        let data = unsafe { str_ref.heap.data };
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
