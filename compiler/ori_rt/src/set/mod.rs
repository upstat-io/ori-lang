//! Set operations for AOT-compiled Ori programs.
//!
//! Sets use an open-addressing hash table with the same probing scheme as maps.
//! Buffer layout: `[metadata (1 byte/bucket) | elements]`.
//! Elements are compared with `elem_eq` + `elem_hash` callbacks.
//!
//! # Submodules
//!
//! - `cow` — COW mutation functions (`ori_set_insert_cow`, etc.) with consuming
//!   semantics: fast path mutates in place when RC==1, slow path copies.

pub mod cow;

use crate::list::write_array_to_list;
use crate::map::hash_table::{get_meta, probe_find, HashTableLayout, META_EMPTY, META_OCCUPIED};

/// Return an empty set sentinel (no allocation).
///
/// Sets use the same `{len, cap, data}` struct as lists — null is the empty sentinel.
#[no_mangle]
pub extern "C" fn ori_set_empty() -> *mut u8 {
    std::ptr::null_mut()
}

/// Check if a set contains an element using hash-based lookup.
///
/// Uses `elem_hash` to find the starting bucket, then probes with `elem_eq`.
/// Returns 1 if found, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_set_contains(
    data: *const u8,
    cap: i64,
    len: i64,
    needle: *const u8,
    elem_size: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() || elem_size <= 0 {
        return 0;
    }
    let es = elem_size as usize;
    let c = cap as usize;
    let layout = HashTableLayout::for_set(c, es);
    let hash = elem_hash(needle);
    let found = unsafe { probe_find(data, c, layout.keys_offset, needle, hash, es, elem_eq) };
    i64::from(found.is_some())
}

/// Convert a set to a list.
///
/// Scans metadata for OCCUPIED buckets, copies elements to a contiguous list.
/// Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_set_to_list(
    data: *const u8,
    cap: i64,
    len: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    if data.is_null() || len <= 0 {
        write_array_to_list(std::ptr::null(), 0, elem_size, None, out_ptr);
        return;
    }

    let es = elem_size.max(1) as usize;
    let c = cap as usize;
    let n = len as usize;
    let layout = HashTableLayout::for_set(c, es);

    let list_data = crate::rc::ori_rc_alloc(n * es, 8);
    let mut write_pos = 0usize;
    for bucket in 0..c {
        if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
            unsafe {
                let src = data.add(layout.keys_offset + bucket * es);
                std::ptr::copy_nonoverlapping(src, list_data.add(write_pos * es), es);
            }
            write_pos += 1;
            if write_pos >= n {
                break;
            }
        }
    }

    // Store elem_dec_fn and elem_count in RC header (this is a list buffer)
    // SAFETY: list_data was returned by ori_rc_alloc — header offsets are valid.
    unsafe {
        crate::rc::store_elem_dec_fn(list_data, elem_dec_fn);
        crate::rc::store_elem_count(list_data, write_pos as i64);
        out_ptr.cast::<i64>().write(write_pos as i64);
        out_ptr.cast::<i64>().add(1).write(write_pos as i64); // cap = len
        out_ptr.add(16).cast::<*mut u8>().write(list_data);
    }
}

// Literal Construction

/// Allocate a hash table buffer sized for `count` elements.
///
/// Computes the power-of-two capacity, allocates the buffer with all metadata
/// initialized to EMPTY, and writes the capacity to `*out_cap`.
/// Returns the data pointer (null if count is 0).
///
/// Used by LLVM codegen for set literal construction.
#[no_mangle]
pub extern "C" fn ori_set_literal_alloc(count: i64, elem_size: i64, out_cap: *mut i64) -> *mut u8 {
    if count <= 0 {
        if !out_cap.is_null() {
            unsafe { out_cap.write(0) };
        }
        return std::ptr::null_mut();
    }
    let es = elem_size.max(1) as usize;
    let cap = crate::map::hash_table::next_hash_capacity(count as usize);
    // elem_dec_fn is stored by codegen after literal construction completes
    let data = alloc_set_hash_buffer(cap, es, None);
    if !out_cap.is_null() {
        unsafe { out_cap.write(cap as i64) };
    }
    data
}

/// Insert an element into a hash table during literal construction.
///
/// Hashes the element, finds an empty slot via linear probing, copies the
/// element into the slot, and marks it OCCUPIED.
///
/// # Safety
/// - `data` must point to a valid hash table buffer allocated by `ori_set_literal_alloc`.
/// - Caller must ensure no duplicate elements (guaranteed for set literals).
/// - Caller must ensure load factor is not exceeded.
#[no_mangle]
pub extern "C" fn ori_set_literal_put(
    data: *mut u8,
    cap: i64,
    elem: *const u8,
    elem_size: i64,
    elem_hash: extern "C" fn(*const u8) -> i64,
) {
    if data.is_null() || cap <= 0 {
        return;
    }
    let es = elem_size.max(1) as usize;
    let c = cap as usize;
    let layout = HashTableLayout::for_set(c, es);

    let hash = elem_hash(elem);
    let bucket = unsafe { crate::map::hash_table::probe_find_slot(data, c, hash) };

    unsafe {
        let dst = data.add(layout.keys_offset + bucket * es);
        std::ptr::copy_nonoverlapping(elem, dst, es);
        crate::map::hash_table::set_meta(data, bucket, META_OCCUPIED);
    }
}

/// Write a set struct `{i64 len, i64 cap, ptr data}` to `out_ptr`.
pub(crate) fn write_set_struct(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

/// Allocate a new hash table set buffer with all metadata initialized to EMPTY.
///
/// Stores `elem_dec_fn` in the RC header for defense-in-depth cleanup.
/// Sets use metadata scanning (not `elem_count`) for element cleanup,
/// so `elem_count` is not stored for set hash table buffers.
pub(crate) fn alloc_set_hash_buffer(
    cap: usize,
    elem_size: usize,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) -> *mut u8 {
    let layout = HashTableLayout::for_set(cap, elem_size);
    if layout.total_size == 0 {
        return std::ptr::null_mut();
    }
    let data = crate::rc::ori_rc_alloc(layout.total_size, 8);
    if !data.is_null() {
        unsafe {
            std::ptr::write_bytes(data, META_EMPTY, layout.metadata_bytes);
            crate::rc::store_elem_dec_fn(data, elem_dec_fn);
        }
    }
    data
}

/// Internal: check if a hash table set contains an element.
pub(crate) fn hash_set_contains(
    data: *const u8,
    cap: usize,
    elem_size: usize,
    needle: *const u8,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
) -> bool {
    if data.is_null() || cap == 0 {
        return false;
    }
    let layout = HashTableLayout::for_set(cap, elem_size);
    let hash = elem_hash(needle);
    unsafe {
        probe_find(
            data,
            cap,
            layout.keys_offset,
            needle,
            hash,
            elem_size,
            elem_eq,
        )
        .is_some()
    }
}
