//! C ABI hash-map runtime for AOT-compiled programs.
//!
//! Open addressing uses `[metadata | keys | values]` buffers with one metadata
//! byte per bucket and a shared RC header for COW uniqueness. Type-specific
//! hash and equality thunks drive lookup. Consuming mutations update unique
//! buffers in place and copy shared buffers before insertion or replacement.

pub mod cow;
pub mod cow_updated;
pub(crate) mod hash_table;
mod structural;

pub use structural::{ori_map_eq, ori_map_hash};

use crate::list::{write_array_to_list, write_list_output};
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};
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
#[derive(Debug)]
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
            // SAFETY: data was just returned by ori_rc_alloc with total_size >= metadata_bytes.
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
    // SAFETY: data/cap validated non-null and positive; layout offsets derived from cap/key_size.
    let found = unsafe { probe_find(data, c, layout.keys_offset, needle, hash, ks, key_eq) };
    i64::from(found.is_some())
}

/// Extract map keys as a new list.
///
/// Scans metadata for OCCUPIED buckets, copies keys to a contiguous list.
/// Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
///
/// `key_inc_fn` is called on each copied key to increment its RC children
/// (e.g., string data pointers). Without this, the new list and the map
/// share ownership of the same RC-tracked data with only one reference,
/// causing a double-free when both are cleaned up.
#[no_mangle]
pub extern "C" fn ori_map_keys_to_list(
    data: *const u8,
    cap: i64,
    len: i64,
    key_size: i64,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    key_inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    if data.is_null() || len <= 0 {
        write_array_to_list(std::ptr::null(), 0, key_size, None, out_ptr);
        return;
    }

    let ks = key_size.max(1) as usize;
    let c = cap as usize;
    let n = len as usize;
    let layout = HashTableLayout::for_map(c, ks, 1);

    // Allocate destination list and copy OCCUPIED keys contiguously
    let list_data = crate::rc::ori_rc_alloc(n * ks, 8);
    // SAFETY: list_data was just returned by ori_rc_alloc — header offsets are valid.
    unsafe {
        crate::rc::store_elem_dec_fn(list_data, key_dec_fn);
    }
    let mut write_pos = 0usize;
    for bucket in 0..c {
        // SAFETY: bucket < c, so metadata index is within the allocated buffer.
        if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
            // SAFETY: write_pos < n and list_data has n*ks bytes allocated.
            let dst = unsafe { list_data.add(write_pos * ks) };
            // SAFETY: src is within the map's key region (bucket < c); dst within list_data.
            unsafe {
                let src = data.add(layout.keys_offset + bucket * ks);
                std::ptr::copy_nonoverlapping(src, dst, ks);
            }
            // RcInc copied element's children (e.g., string data pointers).
            // The copy is a shallow bitwise copy — RC-tracked data is now
            // shared between the map and the new list.
            if let Some(inc) = key_inc_fn {
                inc(dst);
            }
            write_pos += 1;
            if write_pos >= n {
                break;
            }
        }
    }

    // SAFETY: list_data was returned by ori_rc_alloc.
    unsafe { crate::rc::store_elem_count(list_data, write_pos as i64) };
    // SAFETY: `out_ptr` is a live sret slot and `list_data` transfers the
    // initialized `write_pos`-element allocation to the result.
    unsafe {
        write_list_output(out_ptr, write_pos as i64, write_pos as i64, list_data);
    }
}

/// Extract map values as a new list.
///
/// Scans metadata for OCCUPIED buckets, copies values to a contiguous list.
/// Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
///
/// `val_inc_fn` increments RC children of each copied value (see
/// `ori_map_keys_to_list` for rationale).
#[no_mangle]
pub extern "C" fn ori_map_values_to_list(
    data: *const u8,
    cap: i64,
    len: i64,
    key_size: i64,
    val_size: i64,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    if data.is_null() || len <= 0 {
        write_array_to_list(std::ptr::null(), 0, val_size, None, out_ptr);
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let c = cap as usize;
    let n = len as usize;
    let layout = HashTableLayout::for_map(c, ks, vs);

    // Allocate destination list and copy OCCUPIED values contiguously
    let list_data = crate::rc::ori_rc_alloc(n * vs, 8);
    // SAFETY: list_data was just returned by ori_rc_alloc — header offsets are valid.
    unsafe {
        crate::rc::store_elem_dec_fn(list_data, val_dec_fn);
    }
    let mut write_pos = 0usize;
    for bucket in 0..c {
        // SAFETY: bucket < c, so metadata index is within the allocated buffer.
        if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
            // SAFETY: write_pos < n and list_data has n*vs bytes allocated.
            let dst = unsafe { list_data.add(write_pos * vs) };
            // SAFETY: src is within the map's value region (bucket < c); dst within list_data.
            unsafe {
                let src = data.add(layout.vals_offset + bucket * vs);
                std::ptr::copy_nonoverlapping(src, dst, vs);
            }
            if let Some(inc) = val_inc_fn {
                inc(dst);
            }
            write_pos += 1;
            if write_pos >= n {
                break;
            }
        }
    }

    // SAFETY: list_data was returned by ori_rc_alloc.
    unsafe { crate::rc::store_elem_count(list_data, write_pos as i64) };
    // SAFETY: `out_ptr` is a live sret slot and `list_data` transfers the
    // initialized `write_pos`-element allocation to the result.
    unsafe {
        write_list_output(out_ptr, write_pos as i64, write_pos as i64, list_data);
    }
}

/// Look up a key in a map and return `Option<V>` via sret.
///
/// Uses hash-based probing. Writes `{tag: i64, value}` to `out_ptr`.
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
        // SAFETY: `out_ptr` is non-null, and the ABI supplies i64-aligned output storage.
        unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
        return;
    }

    let c = cap as usize;
    let layout = HashTableLayout::for_map(c, ks, vs);
    let hash = key_hash(needle);

    let bucket = {
        // SAFETY: `data` is a live map allocation, and layout offsets derive from `c`, `ks`, and `vs`.
        unsafe { probe_find(data, c, layout.keys_offset, needle, hash, ks, key_eq) }
    };
    if let Some(bucket) = bucket {
        // SAFETY: bucket is a valid index within the hash table; out_ptr is non-null.
        unsafe {
            out_ptr.cast::<i64>().write(OPTION_TAG_SOME);
            let val_src = data.add(layout.vals_offset + bucket * vs);
            std::ptr::copy_nonoverlapping(val_src, out_ptr.add(8), vs);
        }
    } else {
        // SAFETY: `out_ptr` is non-null, and the ABI supplies i64-aligned output storage.
        unsafe { out_ptr.cast::<i64>().write(OPTION_TAG_NONE) };
    }
}

// Literal Construction

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
            // SAFETY: `out_cap` is non-null and writable for one i64.
            unsafe { out_cap.write(0) };
        }
        return std::ptr::null_mut();
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let cap = next_hash_capacity(count as usize);
    let data = OriMap::alloc_hash_buffer(cap, ks, vs);
    if !out_cap.is_null() {
        // SAFETY: `out_cap` is non-null and writable for one i64.
        unsafe { out_cap.write(cap as i64) };
    }
    data
}

/// Insert a key-value pair into a hash table during literal construction.
///
/// Hashes the key and probes for an equal entry before selecting an empty slot.
/// Equal keys replace the earlier owned key and value, so later literal entries
/// win. Returns 1 for a new entry and 0 for a replacement.
///
/// # Safety
/// - `data` must point to a valid hash table buffer allocated by `ori_map_literal_alloc`.
/// - Caller must ensure load factor is not exceeded (guaranteed when `count`
///   passed to `ori_map_literal_alloc` matches the number of inserts).
/// - `key` and `val` must point to initialized values that do not overlap `data`.
#[no_mangle]
pub extern "C" fn ori_map_literal_put(
    data: *mut u8,
    cap: i64,
    key: *const u8,
    val: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    key_dec: Option<extern "C" fn(*mut u8)>,
    val_dec: Option<extern "C" fn(*mut u8)>,
) -> i64 {
    if data.is_null() || cap <= 0 {
        return 0;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let c = cap as usize;
    let layout = HashTableLayout::for_map(c, ks, vs);

    let hash = key_hash(key);
    // SAFETY: data is a valid hash table buffer and cap is positive.
    let existing = unsafe { probe_find(data, c, layout.keys_offset, key, hash, ks, key_eq) };
    let (bucket, inserted) = if let Some(bucket) = existing {
        (bucket, false)
    } else {
        // SAFETY: allocation capacity is based on the total number of source
        // entries, so a new distinct key always has an available slot.
        (unsafe { probe_find_slot(data, c, hash) }, true)
    };

    // SAFETY: bucket is within [0, c); key/val regions are non-overlapping within the buffer.
    unsafe {
        let dst_key = data.add(layout.keys_offset + bucket * ks);
        let dst_val = data.add(layout.vals_offset + bucket * vs);
        if !inserted {
            if let Some(dec) = key_dec {
                dec(dst_key);
            }
            if let Some(dec) = val_dec {
                dec(dst_val);
            }
        }
        std::ptr::copy_nonoverlapping(key, dst_key, ks);
        std::ptr::copy_nonoverlapping(val, dst_val, vs);
        if inserted {
            set_meta(data, bucket, META_OCCUPIED);
        }
    }

    i64::from(inserted)
}

/// Write a map struct `{i64 len, i64 cap, ptr data}` to `out_ptr`.
pub(crate) fn write_map_struct(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    // SAFETY: out_ptr is caller-provided sret buffer with space for {i64, i64, *mut u8}.
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}
