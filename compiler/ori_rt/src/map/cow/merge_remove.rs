//! COW merge and removal operations.

use crate::rc::{ori_map_buffer_rc_dec, ori_rc_free, ori_rc_is_unique};

use super::super::{
    get_meta, next_hash_capacity, probe_find, probe_find_slot, set_meta, write_map_struct,
    HashTableLayout, OriMap, META_OCCUPIED, META_TOMBSTONE,
};
use super::insert::ori_map_insert_cow;

/// COW-aware map merge (`{...a, ...b}` desugar `a.merge(b)`).
///
/// Consuming semantics on BOTH maps (matching list `concat`): the receiver `a`
/// becomes the accumulator; each occupied entry of `b` is inserted via
/// `ori_map_insert_cow` (which increments the borrowed key/value into the
/// result buffer), then `b`'s buffer is released via `ori_map_buffer_rc_dec`.
/// On key collision `b` wins ("later wins"), matching the interpreter's
/// `dispatch_map_method_str`.
///
/// `key_inc`/`val_inc`/`key_dec`/`val_dec` are null for scalar element types.
#[no_mangle]
pub extern "C" fn ori_map_merge_cow(
    a_data: *mut u8,
    a_len: i64,
    a_cap: i64,
    b_data: *mut u8,
    b_len: i64,
    b_cap: i64,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_hash: extern "C" fn(*const u8) -> i64,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    key_dec: Option<extern "C" fn(*mut u8)>,
    val_dec: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let bc = b_cap.max(0) as usize;

    // Accumulator starts as the consumed receiver `a`.
    let mut acc = OriMap {
        len: a_len,
        cap: a_cap,
        data: a_data,
    };

    if !b_data.is_null() && bc > 0 {
        let layout = HashTableLayout::for_map(bc, ks, vs);
        let mut first = true;
        for bucket in 0..bc {
            // SAFETY: bucket < bc and b_data was returned by ori_rc_alloc.
            if unsafe { get_meta(b_data, bucket) } != META_OCCUPIED {
                continue;
            }
            // SAFETY: keys_offset/vals_offset + bucket * stride are within the
            // key/value regions of b's hash buffer.
            let key_ptr = unsafe { b_data.add(layout.keys_offset + bucket * ks) };
            let val_ptr = unsafe { b_data.add(layout.vals_offset + bucket * vs) };

            // The first insert consumes `a` under its analyzed cow_mode; every
            // later insert operates on the uniquely-owned accumulator (mode 1).
            let mode = if first { cow_mode } else { 1 };
            let mut tmp = OriMap {
                len: 0,
                cap: 0,
                data: std::ptr::null_mut(),
            };
            ori_map_insert_cow(
                acc.data,
                acc.len,
                acc.cap,
                key_ptr.cast_const(),
                val_ptr.cast_const(),
                key_size,
                val_size,
                key_eq,
                key_hash,
                key_inc,
                val_inc,
                key_dec,
                val_dec,
                mode,
                std::ptr::from_mut(&mut tmp).cast(),
            );
            acc = tmp;
            first = false;
        }
    }

    // Release `b` (consumed argument). The per-entry inserts already gave the
    // accumulator its own references to b's keys/values, so this dec balances
    // b's original references.
    crate::rc::ori_map_buffer_rc_dec(b_data, b_cap, b_len, key_size, val_size, key_dec, val_dec);

    write_map_struct(out_ptr, acc.len, acc.cap, acc.data);
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
    key_dec: Option<extern "C" fn(*mut u8)>,
    val_dec: Option<extern "C" fn(*mut u8)>,
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
            // Dec RC children of removed entry before freeing buffer
            if is_unique {
                if let Some(dec) = key_dec {
                    dec(unsafe { data.add(layout.keys_offset + bucket * ks) });
                }
                if let Some(dec) = val_dec {
                    dec(unsafe { data.add(layout.vals_offset + bucket * vs) });
                }
                ori_rc_free(data, layout.total_size, 8);
            } else {
                ori_map_buffer_rc_dec(data, cap, len, key_size, val_size, key_dec, val_dec);
            }
        }
        write_map_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique owner — dec removed entry, then tombstone
    if is_unique {
        let layout = HashTableLayout::for_map(c, ks, vs);
        if let Some(dec) = key_dec {
            dec(unsafe { data.add(layout.keys_offset + bucket * ks) });
        }
        if let Some(dec) = val_dec {
            dec(unsafe { data.add(layout.vals_offset + bucket * vs) });
        }
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

    ori_map_buffer_rc_dec(data, cap, len, key_size, val_size, key_dec, val_dec);
    write_map_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
}
