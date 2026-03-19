//! Map operations for AOT-compiled Ori programs.
//!
//! Ori maps use a hash table with open addressing and linear probing.
//! Buffer layout: `[metadata | keys | values]` where metadata is 1 byte
//! per bucket (EMPTY/OCCUPIED/TOMBSTONE), keys and values are indexed by
//! bucket number. Single RC header enables one uniqueness check for COW.
//!
//! All key lookups use `key_hash` + `key_eq` callbacks for type-agnostic
//! hashing and comparison. The codegen generates type-specific thunks.
//!
//! # Submodules
//!
//! - `cow` — COW mutation functions (`ori_map_insert_cow`, etc.) with consuming
//!   semantics: fast path mutates in place when RC==1, slow path copies.
//! - `hash_table` — Core hash table logic (layout, probing, rehashing).

pub mod cow;
pub(crate) mod hash_table;

use crate::list::write_array_to_list;
#[allow(unused_imports, reason = "used by cow.rs after rewrite")]
pub(crate) use hash_table::{
    get_meta, needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_map, set_meta,
    HashTableLayout, META_EMPTY, META_OCCUPIED, META_TOMBSTONE,
};

/// Ori map representation: { i64 len, i64 cap, *mut u8 data }
///
/// Single RC'd buffer: `[metadata (1 byte/bucket) | keys | values]`.
/// `cap` = number of buckets (power-of-two). `len` = number of entries.
/// Single RC header enables one uniqueness check for COW.
#[repr(C)]
pub struct OriMap {
    pub len: i64,
    pub cap: i64,
    pub data: *mut u8,
}

impl OriMap {
    /// Allocate a new hash table data buffer for `cap` buckets.
    ///
    /// The buffer is initialized with all metadata bytes set to EMPTY.
    pub(crate) fn alloc_hash_buffer(cap: usize, key_size: usize, val_size: usize) -> *mut u8 {
        let layout = HashTableLayout::for_map(cap, key_size, val_size);
        if layout.total_size == 0 {
            return std::ptr::null_mut();
        }
        let data = crate::rc::ori_rc_alloc(layout.total_size, 8);
        if !data.is_null() {
            // Initialize all metadata to EMPTY
            unsafe { std::ptr::write_bytes(data, META_EMPTY, layout.metadata_bytes) };
        }
        data
    }
}

/// Return an empty map sentinel (no allocation).
#[no_mangle]
pub extern "C" fn ori_map_empty() -> OriMap {
    OriMap {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    }
}

/// Check if a map contains a key using hash-based lookup.
///
/// Uses `key_hash` to find the starting bucket, then linear probes with
/// `key_eq` for confirmation. Returns 1 if found, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_map_contains_key(
    data: *const u8,
    cap: i64,
    len: i64,
    needle: *const u8,
    key_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() {
        return 0;
    }
    let ks = key_size.max(1) as usize;
    let c = cap as usize;
    let layout = HashTableLayout::for_map(c, ks, 1);
    let hash = key_hash(needle);
    let found = unsafe { probe_find(data, c, layout.keys_offset, needle, hash, ks, key_eq) };
    i64::from(found.is_some())
}

/// Extract map keys as a new list.
///
/// Scans metadata for OCCUPIED buckets, copies keys to a contiguous list.
/// Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_map_keys_to_list(
    data: *const u8,
    cap: i64,
    len: i64,
    key_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    if data.is_null() || len <= 0 {
        write_array_to_list(std::ptr::null(), 0, key_size, out_ptr);
        return;
    }

    let ks = key_size.max(1) as usize;
    let c = cap as usize;
    let n = len as usize;
    let layout = HashTableLayout::for_map(c, ks, 1);

    // Allocate destination list and copy OCCUPIED keys contiguously
    let list_data = crate::rc::ori_rc_alloc(n * ks, 8);
    let mut write_pos = 0usize;
    for bucket in 0..c {
        if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
            unsafe {
                let src = data.add(layout.keys_offset + bucket * ks);
                std::ptr::copy_nonoverlapping(src, list_data.add(write_pos * ks), ks);
            }
            write_pos += 1;
            if write_pos >= n {
                break;
            }
        }
    }

    write_array_to_list_from_data(list_data, write_pos as i64, key_size, out_ptr);
}

/// Extract map values as a new list.
///
/// Scans metadata for OCCUPIED buckets, copies values to a contiguous list.
/// Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_map_values_to_list(
    data: *const u8,
    cap: i64,
    len: i64,
    key_size: i64,
    val_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    if data.is_null() || len <= 0 {
        write_array_to_list(std::ptr::null(), 0, val_size, out_ptr);
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let c = cap as usize;
    let n = len as usize;
    let layout = HashTableLayout::for_map(c, ks, vs);

    // Allocate destination list and copy OCCUPIED values contiguously
    let list_data = crate::rc::ori_rc_alloc(n * vs, 8);
    let mut write_pos = 0usize;
    for bucket in 0..c {
        if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
            unsafe {
                let src = data.add(layout.vals_offset + bucket * vs);
                std::ptr::copy_nonoverlapping(src, list_data.add(write_pos * vs), vs);
            }
            write_pos += 1;
            if write_pos >= n {
                break;
            }
        }
    }

    write_array_to_list_from_data(list_data, write_pos as i64, val_size, out_ptr);
}

/// Look up a key in a map and return `Option<V>` via sret.
///
/// Uses hash-based probing. Writes `{tag: i64, value}` to `out_ptr`.
/// Tag 0 = Some (value copied), tag 1 = None.
#[no_mangle]
pub extern "C" fn ori_map_get(
    data: *const u8,
    cap: i64,
    len: i64,
    needle: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;

    if data.is_null() || len <= 0 {
        unsafe { out_ptr.cast::<i64>().write(1) };
        return;
    }

    let c = cap as usize;
    let layout = HashTableLayout::for_map(c, ks, vs);
    let hash = key_hash(needle);

    if let Some(bucket) =
        unsafe { probe_find(data, c, layout.keys_offset, needle, hash, ks, key_eq) }
    {
        // Some: tag = 0, copy value
        unsafe {
            out_ptr.cast::<i64>().write(0);
            let val_src = data.add(layout.vals_offset + bucket * vs);
            std::ptr::copy_nonoverlapping(val_src, out_ptr.add(8), vs);
        }
    } else {
        // None: tag = 1
        unsafe { out_ptr.cast::<i64>().write(1) };
    }
}

// ── Literal Construction ──────────────────────────────────────────────

/// Allocate a hash table buffer sized for `count` entries.
///
/// Computes the power-of-two capacity, allocates the buffer with all metadata
/// initialized to EMPTY, and writes the capacity to `*out_cap`.
/// Returns the data pointer (null if count is 0).
///
/// Used by LLVM codegen for map literal construction (`{a: 1, b: 2}`).
#[no_mangle]
pub extern "C" fn ori_map_literal_alloc(
    count: i64,
    key_size: i64,
    val_size: i64,
    out_cap: *mut i64,
) -> *mut u8 {
    if count <= 0 {
        if !out_cap.is_null() {
            unsafe { out_cap.write(0) };
        }
        return std::ptr::null_mut();
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let cap = next_hash_capacity(count as usize);
    let data = OriMap::alloc_hash_buffer(cap, ks, vs);
    if !out_cap.is_null() {
        unsafe { out_cap.write(cap as i64) };
    }
    data
}

/// Insert a key-value pair into a hash table during literal construction.
///
/// Hashes the key, finds an empty slot via linear probing, copies the key
/// and value into the slot, and marks it OCCUPIED.
///
/// # Safety
/// - `data` must point to a valid hash table buffer allocated by `ori_map_literal_alloc`.
/// - Caller must ensure no duplicate keys (guaranteed for map literals).
/// - Caller must ensure load factor is not exceeded (guaranteed when `count`
///   passed to `ori_map_literal_alloc` matches the number of inserts).
#[no_mangle]
pub extern "C" fn ori_map_literal_put(
    data: *mut u8,
    cap: i64,
    key: *const u8,
    val: *const u8,
    key_size: i64,
    val_size: i64,
    key_hash: extern "C" fn(*const u8) -> i64,
) {
    if data.is_null() || cap <= 0 {
        return;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let c = cap as usize;
    let layout = HashTableLayout::for_map(c, ks, vs);

    let hash = key_hash(key);
    let bucket = unsafe { probe_find_slot(data, c, hash) };

    unsafe {
        let dst_key = data.add(layout.keys_offset + bucket * ks);
        std::ptr::copy_nonoverlapping(key, dst_key, ks);
        let dst_val = data.add(layout.vals_offset + bucket * vs);
        std::ptr::copy_nonoverlapping(val, dst_val, vs);
        set_meta(data, bucket, META_OCCUPIED);
    }
}

/// Write a map struct `{i64 len, i64 cap, ptr data}` to `out_ptr`.
pub(crate) fn write_map_struct(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

/// Write a list struct from an already-allocated data buffer.
///
/// Unlike `write_array_to_list` which allocates and copies, this takes
/// ownership of an existing RC-allocated buffer.
fn write_array_to_list_from_data(data: *mut u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    if len <= 0 || data.is_null() {
        // Write empty list
        unsafe {
            out_ptr.cast::<i64>().write(0);
            out_ptr.cast::<i64>().add(1).write(0);
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut());
        }
        return;
    }
    let _ = elem_size; // used by caller for sizing
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(len); // cap = len
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

/// Compare two maps for equality.
///
/// Two maps are equal when they have the same length and every key in map A
/// exists in map B with an equal value. Uses `key_eq`/`key_hash` for key
/// lookup and `val_eq` for value comparison.
#[no_mangle]
pub extern "C" fn ori_map_eq(
    a: *const OriMap,
    b: *const OriMap,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    val_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> bool {
    let (a_map, b_map) = unsafe {
        if a.is_null() || b.is_null() {
            return a.is_null() && b.is_null();
        }
        (&*a, &*b)
    };

    // Quick checks: lengths must match
    if a_map.len != b_map.len {
        return false;
    }
    // Both empty
    if a_map.len == 0 {
        return true;
    }
    // Same data pointer → identical
    if a_map.data == b_map.data && a_map.cap == b_map.cap {
        return true;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let a_cap = a_map.cap as usize;
    let b_cap = b_map.cap as usize;
    let a_layout = HashTableLayout::for_map(a_cap, ks, vs);
    let b_layout = HashTableLayout::for_map(b_cap, ks, vs);

    // For each occupied entry in A, look it up in B and compare values
    let a_data = a_map.data.cast_const();
    let b_data = b_map.data.cast_const();
    let mut checked = 0usize;
    let n = a_map.len as usize;

    for bucket in 0..a_cap {
        if unsafe { get_meta(a_data, bucket) } != META_OCCUPIED {
            continue;
        }
        let a_key = unsafe { a_data.add(a_layout.keys_offset + bucket * ks) };
        let a_val = unsafe { a_data.add(a_layout.vals_offset + bucket * vs) };
        let hash = key_hash(a_key);

        // Find the same key in B
        let b_bucket =
            unsafe { probe_find(b_data, b_cap, b_layout.keys_offset, a_key, hash, ks, key_eq) };
        let Some(b_idx) = b_bucket else {
            return false; // key not in B
        };
        let b_val = unsafe { b_data.add(b_layout.vals_offset + b_idx * vs) };
        if !val_eq(a_val, b_val) {
            return false; // values differ
        }
        checked += 1;
        if checked >= n {
            break;
        }
    }

    true
}
