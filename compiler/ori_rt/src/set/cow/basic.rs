//! Basic COW set mutations: insert and remove.

use crate::map::hash_table::{
    get_meta, needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_set, set_meta,
    HashTableLayout, META_OCCUPIED, META_TOMBSTONE,
};
use crate::rc::{ori_rc_dec, ori_rc_free, ori_rc_is_unique};

use crate::set::{alloc_set_hash_buffer, write_set_struct};

/// COW-aware set insert with consuming semantics.
///
/// Inserts an element using hash-based lookup.
///
/// - **No-op** (element exists): Returns input unchanged.
/// - **Fast path** (unique, under load): Writes to empty/tombstone slot.
/// - **Fast path** (unique, needs rehash): Rehash to 2x, then insert.
/// - **Slow path** (shared or empty): Rehash into new buffer, insert.
#[no_mangle]
pub extern "C" fn ori_set_insert_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem: *const u8,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let _ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;
    let hash = elem_hash(elem);

    // Check if element already exists
    if c > 0 && !data.is_null() {
        let layout = HashTableLayout::for_set(c, es);
        if unsafe { probe_find(data, c, layout.keys_offset, elem, hash, es, elem_eq) }.is_some() {
            write_set_struct(out_ptr, len, cap, data);
            return;
        }
    }

    let new_len = n + 1;
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));

    if is_unique {
        // FAST PATH: unique owner
        if !needs_rehash(new_len, c) {
            // Has capacity — write element into empty/tombstone slot
            let layout = HashTableLayout::for_set(c, es);
            let slot = unsafe { probe_find_slot(data, c, hash) };
            unsafe {
                std::ptr::copy_nonoverlapping(elem, data.add(layout.keys_offset + slot * es), es);
                set_meta(data, slot, META_OCCUPIED);
            }
            write_set_struct(out_ptr, new_len as i64, cap, data);
            return;
        }

        // Needs rehash — rehash to 2x capacity, then insert
        let new_cap = next_hash_capacity(new_len);
        let old_dec = unsafe { crate::rc::load_elem_dec_fn(data) };
        let new_data = unsafe { rehash_set(data, c, new_cap, es, elem_hash, old_dec) };
        let new_layout = HashTableLayout::for_set(new_cap, es);

        let slot = unsafe { probe_find_slot(new_data, new_cap, hash) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                elem,
                new_data.add(new_layout.keys_offset + slot * es),
                es,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }

        // Free old buffer
        let old_layout = HashTableLayout::for_set(c, es);
        ori_rc_free(data, old_layout.total_size, 8);

        write_set_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
        return;
    }

    // SLOW PATH: shared or empty — rehash into new buffer + insert
    let new_cap = next_hash_capacity(new_len);
    let old_dec = if data.is_null() {
        None
    } else {
        unsafe { crate::rc::load_elem_dec_fn(data) }
    };
    let new_data = if !data.is_null() && n > 0 {
        let rehashed = unsafe { rehash_set(data, c, new_cap, es, elem_hash, old_dec) };
        // Inc RC for all rehashed elements
        let new_layout = HashTableLayout::for_set(new_cap, es);
        if let Some(inc) = inc_fn {
            for b in 0..new_cap {
                if unsafe { get_meta(rehashed, b) } == META_OCCUPIED {
                    inc(unsafe { rehashed.add(new_layout.keys_offset + b * es) });
                }
            }
        }
        rehashed
    } else {
        alloc_set_hash_buffer(new_cap, es, old_dec)
    };

    // Insert new element
    let new_layout = HashTableLayout::for_set(new_cap, es);
    let slot = unsafe { probe_find_slot(new_data, new_cap, hash) };
    unsafe {
        std::ptr::copy_nonoverlapping(elem, new_data.add(new_layout.keys_offset + slot * es), es);
        set_meta(new_data, slot, META_OCCUPIED);
    }

    ori_rc_dec(data, None);
    write_set_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
}

/// COW-aware set remove with consuming semantics.
///
/// - **No-op** (element not found): Returns input unchanged.
/// - **Fast path** (unique, found): Sets metadata to TOMBSTONE. O(1).
/// - **Fast path** (unique, last element): Frees buffer, returns empty.
/// - **Slow path** (shared): Rehash all except removed into new buffer.
#[no_mangle]
pub extern "C" fn ori_set_remove_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem: *const u8,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let _ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Find the element
    let found_bucket = if c > 0 && !data.is_null() {
        let layout = HashTableLayout::for_set(c, es);
        let hash = elem_hash(elem);
        unsafe { probe_find(data, c, layout.keys_offset, elem, hash, es, elem_eq) }
    } else {
        None
    };

    let Some(bucket) = found_bucket else {
        write_set_struct(out_ptr, len, cap, data);
        return;
    };

    let new_len = n - 1;
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));

    // Special case: removing last element → empty sentinel
    if new_len == 0 {
        if !data.is_null() {
            let layout = HashTableLayout::for_set(c, es);
            if is_unique {
                ori_rc_free(data, layout.total_size, 8);
            } else {
                ori_rc_dec(data, None);
            }
        }
        write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique — tombstone in place
    if is_unique {
        unsafe { set_meta(data, bucket, META_TOMBSTONE) };
        write_set_struct(out_ptr, new_len as i64, cap, data);
        return;
    }

    // SLOW PATH: shared — rehash all except removed into new buffer
    let new_cap = next_hash_capacity(new_len);
    let new_layout = HashTableLayout::for_set(new_cap, es);
    let old_dec = unsafe { crate::rc::load_elem_dec_fn(data) };
    let new_data = alloc_set_hash_buffer(new_cap, es, old_dec);
    let old_layout = HashTableLayout::for_set(c, es);

    for b in 0..c {
        if b == bucket {
            continue;
        }
        if unsafe { get_meta(data, b) } != META_OCCUPIED {
            continue;
        }
        let old_elem = unsafe { data.add(old_layout.keys_offset + b * es) };
        let h = elem_hash(old_elem);
        let slot = unsafe { probe_find_slot(new_data, new_cap, h) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                old_elem,
                new_data.add(new_layout.keys_offset + slot * es),
                es,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }
        if let Some(inc) = inc_fn {
            inc(unsafe { new_data.add(new_layout.keys_offset + slot * es) });
        }
    }

    ori_rc_dec(data, None);
    write_set_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
}
