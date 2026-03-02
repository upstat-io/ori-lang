//! COW (Copy-on-Write) map mutation functions with hash table backing.
//!
//! All functions follow consuming semantics: they take ownership of the
//! caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.
//!
//! The hash table layout is `[metadata | keys | values]` with open addressing
//! and linear probing. Growth triggers a full rehash into a new buffer.

use crate::rc::{ori_rc_dec, ori_rc_free, ori_rc_is_unique};

use super::{
    get_meta, needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_map, set_meta,
    write_map_struct, HashTableLayout, OriMap, META_OCCUPIED, META_TOMBSTONE,
};

/// COW-aware map insert with consuming semantics.
///
/// Inserts a key-value pair using hash-based lookup. The data buffer must be
/// RC-allocated or null (empty sentinel). This function **takes ownership**
/// of the caller's reference to `data`:
///
/// - **Fast path** (unique, key exists): Overwrites value in place.
/// - **Fast path** (unique, new key, under load): Writes to empty/tombstone slot.
/// - **Fast path** (unique, new key, needs rehash): Rehash to 2x, then insert.
/// - **Slow path** (shared or empty): Rehash all entries into new buffer, insert.
///
/// # Key lookup
///
/// `key_hash` computes the hash. `key_eq` confirms key identity.
///
/// # Element RC
///
/// On the slow path, rehashed keys and values get their RC incremented via
/// `key_inc` / `val_inc`. Pass null for scalar types.
#[no_mangle]
pub extern "C" fn ori_map_insert_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    key: *const u8,
    value: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() || value.is_null() {
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;
    let hash = key_hash(key);

    // Hash-probe for existing key
    let found_bucket = if c > 0 && !data.is_null() {
        let layout = HashTableLayout::for_map(c, ks, vs);
        unsafe { probe_find(data, c, layout.keys_offset, key, hash, ks, key_eq) }
    } else {
        None
    };

    if let Some(bucket) = found_bucket {
        // Key exists — overwrite value
        cow_insert_existing(
            data, n, c, ks, vs, bucket, value, key_inc, val_inc, cow_mode, out_ptr,
        );
    } else {
        // New key — insert into hash table
        cow_insert_new(
            data, n, c, ks, vs, key, value, hash, key_hash, key_inc, val_inc, cow_mode, out_ptr,
        );
    }
}

/// COW insert when key exists at `bucket` — overwrite value.
#[expect(
    clippy::too_many_arguments,
    reason = "COW map parameters cannot be grouped — all independent ABI scalars"
)]
fn cow_insert_existing(
    data: *mut u8,
    len: usize,
    cap: usize,
    ks: usize,
    vs: usize,
    bucket: usize,
    value: *const u8,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    let layout = HashTableLayout::for_map(cap, ks, vs);

    if is_unique {
        // FAST PATH: unique — overwrite value in place
        unsafe {
            let val_dst = data.add(layout.vals_offset + bucket * vs);
            std::ptr::copy_nonoverlapping(value, val_dst, vs);
        }
        write_map_struct(out_ptr, len as i64, cap as i64, data);
        return;
    }

    // SLOW PATH: shared — copy buffer and overwrite one value.
    // We don't have key_hash here, so we can't rehash. Instead, copy the raw
    // buffer (preserving hash table structure) and overwrite the value at bucket.
    slow_copy_overwrite_value(
        data, len, cap, ks, vs, bucket, value, key_inc, val_inc, out_ptr,
    );
}

/// Slow path: copy existing hash table buffer, overwrite one value.
///
/// Used when the buffer is shared and we need to overwrite a value at a
/// known bucket. Copies the entire buffer (metadata + keys + values) and
/// overwrites the value at `bucket`.
#[expect(
    clippy::too_many_arguments,
    reason = "COW map copy parameters — all independent"
)]
fn slow_copy_overwrite_value(
    data: *mut u8,
    len: usize,
    cap: usize,
    ks: usize,
    vs: usize,
    bucket: usize,
    value: *const u8,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    let layout = HashTableLayout::for_map(cap, ks, vs);
    let new_data = crate::rc::ori_rc_alloc(layout.total_size, 8);

    // Copy entire buffer
    unsafe { std::ptr::copy_nonoverlapping(data, new_data, layout.total_size) };

    // Overwrite value at bucket
    unsafe {
        let val_dst = new_data.add(layout.vals_offset + bucket * vs);
        std::ptr::copy_nonoverlapping(value, val_dst, vs);
    }

    // Inc RC for all OCCUPIED keys and values (except overwritten value at bucket)
    for b in 0..cap {
        if unsafe { get_meta(new_data, b) } != META_OCCUPIED {
            continue;
        }
        if let Some(inc) = key_inc {
            inc(unsafe { new_data.add(layout.keys_offset + b * ks) });
        }
        if b != bucket {
            if let Some(inc) = val_inc {
                inc(unsafe { new_data.add(layout.vals_offset + b * vs) });
            }
        }
    }

    ori_rc_dec(data, None);
    write_map_struct(out_ptr, len as i64, cap as i64, new_data);
}

/// COW insert for a new key — add to hash table.
#[expect(
    clippy::too_many_arguments,
    reason = "COW map hash insert parameters — all independent ABI scalars"
)]
fn cow_insert_new(
    data: *mut u8,
    len: usize,
    cap: usize,
    ks: usize,
    vs: usize,
    key: *const u8,
    value: *const u8,
    hash: i64,
    key_hash: extern "C" fn(*const u8) -> i64,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    let new_len = len + 1;

    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));

    if is_unique {
        // FAST PATH: unique owner
        if !needs_rehash(new_len, cap) {
            // Has capacity — write key + value into empty/tombstone slot
            let layout = HashTableLayout::for_map(cap, ks, vs);
            let slot = unsafe { probe_find_slot(data, cap, hash) };
            unsafe {
                std::ptr::copy_nonoverlapping(key, data.add(layout.keys_offset + slot * ks), ks);
                std::ptr::copy_nonoverlapping(value, data.add(layout.vals_offset + slot * vs), vs);
                set_meta(data, slot, META_OCCUPIED);
            }
            write_map_struct(out_ptr, new_len as i64, cap as i64, data);
            return;
        }

        // Needs rehash — rehash to 2x capacity, then insert new key
        let new_cap = next_hash_capacity(new_len);
        let new_data = unsafe { rehash_map(data, cap, new_cap, ks, vs, key_hash) };
        let new_layout = HashTableLayout::for_map(new_cap, ks, vs);

        // Insert new key into rehashed table
        let slot = unsafe { probe_find_slot(new_data, new_cap, hash) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                key,
                new_data.add(new_layout.keys_offset + slot * ks),
                ks,
            );
            std::ptr::copy_nonoverlapping(
                value,
                new_data.add(new_layout.vals_offset + slot * vs),
                vs,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }

        // Free old buffer (unique, so direct free)
        let old_layout = HashTableLayout::for_map(cap, ks, vs);
        ori_rc_free(data, old_layout.total_size, 8);

        write_map_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
        return;
    }

    // SLOW PATH: shared or empty — rehash all entries into new buffer + insert
    let new_cap = next_hash_capacity(new_len);
    let new_data = if !data.is_null() && len > 0 {
        let rehashed = unsafe { rehash_map(data, cap, new_cap, ks, vs, key_hash) };
        // Inc RC for all rehashed keys and values
        let new_layout = HashTableLayout::for_map(new_cap, ks, vs);
        for b in 0..new_cap {
            if unsafe { get_meta(rehashed, b) } == META_OCCUPIED {
                if let Some(inc) = key_inc {
                    inc(unsafe { rehashed.add(new_layout.keys_offset + b * ks) });
                }
                if let Some(inc) = val_inc {
                    inc(unsafe { rehashed.add(new_layout.vals_offset + b * vs) });
                }
            }
        }
        rehashed
    } else {
        OriMap::alloc_hash_buffer(new_cap, ks, vs)
    };

    // Insert new key
    let new_layout = HashTableLayout::for_map(new_cap, ks, vs);
    let slot = unsafe { probe_find_slot(new_data, new_cap, hash) };
    unsafe {
        std::ptr::copy_nonoverlapping(key, new_data.add(new_layout.keys_offset + slot * ks), ks);
        std::ptr::copy_nonoverlapping(value, new_data.add(new_layout.vals_offset + slot * vs), vs);
        set_meta(new_data, slot, META_OCCUPIED);
    }

    // Release old buffer reference
    ori_rc_dec(data, None);

    write_map_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
}

/// COW-aware map remove with consuming semantics.
///
/// Removes the entry with the given key using hash-based lookup.
///
/// - **Fast path** (unique, key found): Sets metadata to TOMBSTONE. O(1).
/// - **Fast path** (unique, last entry): Frees buffer, returns empty sentinel.
/// - **No-op** (key not found): Returns input unchanged.
/// - **Slow path** (shared, key found): Rehash all entries except removed one
///   into a new buffer.
#[no_mangle]
pub extern "C" fn ori_map_remove_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    key: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() {
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Find the key
    let found_bucket = if c > 0 && !data.is_null() {
        let layout = HashTableLayout::for_map(c, ks, vs);
        let hash = key_hash(key);
        unsafe { probe_find(data, c, layout.keys_offset, key, hash, ks, key_eq) }
    } else {
        None
    };

    let Some(bucket) = found_bucket else {
        // Key not found — return input unchanged
        write_map_struct(out_ptr, len, cap, data);
        return;
    };

    let new_len = n - 1;
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));

    // Special case: removing last entry → empty sentinel
    if new_len == 0 {
        if !data.is_null() {
            let layout = HashTableLayout::for_map(c, ks, vs);
            if is_unique {
                ori_rc_free(data, layout.total_size, 8);
            } else {
                ori_rc_dec(data, None);
            }
        }
        write_map_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique owner — tombstone in place
    if is_unique {
        unsafe { set_meta(data, bucket, META_TOMBSTONE) };
        write_map_struct(out_ptr, new_len as i64, cap, data);
        return;
    }

    // SLOW PATH: shared — copy all entries except removed one into new buffer
    let new_cap = next_hash_capacity(new_len);
    let new_layout = HashTableLayout::for_map(new_cap, ks, vs);
    let new_data = OriMap::alloc_hash_buffer(new_cap, ks, vs);
    let old_layout = HashTableLayout::for_map(c, ks, vs);

    for b in 0..c {
        if b == bucket {
            continue; // skip removed entry
        }
        if unsafe { get_meta(data, b) } != META_OCCUPIED {
            continue;
        }
        let old_key = unsafe { data.add(old_layout.keys_offset + b * ks) };
        let old_val = unsafe { data.add(old_layout.vals_offset + b * vs) };
        let h = key_hash(old_key);
        let slot = unsafe { probe_find_slot(new_data, new_cap, h) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                old_key,
                new_data.add(new_layout.keys_offset + slot * ks),
                ks,
            );
            std::ptr::copy_nonoverlapping(
                old_val,
                new_data.add(new_layout.vals_offset + slot * vs),
                vs,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }
        // Inc RC for copied key and value
        if let Some(inc) = key_inc {
            inc(unsafe { new_data.add(new_layout.keys_offset + slot * ks) });
        }
        if let Some(inc) = val_inc {
            inc(unsafe { new_data.add(new_layout.vals_offset + slot * vs) });
        }
    }

    ori_rc_dec(data, None);
    write_map_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
}

/// COW-aware map update (value replacement) with consuming semantics.
///
/// Replaces the value for an existing key. If the key is not found, returns
/// unchanged (no insertion).
#[no_mangle]
pub extern "C" fn ori_map_update_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    key: *const u8,
    new_value: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() || new_value.is_null() {
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Find the key
    let found_bucket = if c > 0 && !data.is_null() {
        let layout = HashTableLayout::for_map(c, ks, vs);
        let hash = key_hash(key);
        unsafe { probe_find(data, c, layout.keys_offset, key, hash, ks, key_eq) }
    } else {
        None
    };

    let Some(bucket) = found_bucket else {
        write_map_struct(out_ptr, len, cap, data);
        return;
    };

    cow_insert_existing(
        data, n, c, ks, vs, bucket, new_value, key_inc, val_inc, cow_mode, out_ptr,
    );
}
