//! COW (Copy-on-Write) set mutation functions with hash table backing.
//!
//! All functions follow consuming semantics: they take ownership of the
//! caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.
//!
//! The hash table layout is `[metadata (1 byte/bucket) | elements]` with open
//! addressing and linear probing.

use crate::map::hash_table::{
    get_meta, needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_set, set_meta,
    HashTableLayout, META_OCCUPIED, META_TOMBSTONE,
};
use crate::rc::{ori_rc_dec, ori_rc_free, ori_rc_is_unique};

use super::{alloc_set_hash_buffer, hash_set_contains, write_set_struct};

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
        let new_data = unsafe { rehash_set(data, c, new_cap, es, elem_hash) };
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
    let new_data = if !data.is_null() && n > 0 {
        let rehashed = unsafe { rehash_set(data, c, new_cap, es, elem_hash) };
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
        alloc_set_hash_buffer(new_cap, es)
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
    let new_data = alloc_set_hash_buffer(new_cap, es);
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

/// Rehash `d1` into a new buffer of `new_cap`, then insert all elements from
/// `d2` that are not already in `d1`. Increments RC for each copied element.
#[expect(
    clippy::too_many_arguments,
    reason = "COW set merge helper — all parameters are independent hash table scalars"
)]
fn rehash_and_merge_set2(
    d1: *mut u8,
    cap1: usize,
    d2: *const u8,
    cap2: usize,
    layout2: HashTableLayout,
    new_cap: usize,
    es: usize,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
) -> *mut u8 {
    let new_data = unsafe { rehash_set(d1, cap1, new_cap, es, elem_hash) };
    let new_layout = HashTableLayout::for_set(new_cap, es);
    for b in 0..cap2 {
        if unsafe { get_meta(d2, b) } != META_OCCUPIED {
            continue;
        }
        let elem = unsafe { d2.add(layout2.keys_offset + b * es) };
        if hash_set_contains(d1, cap1, es, elem, elem_eq, elem_hash) {
            continue;
        }
        let h = elem_hash(elem);
        let slot = unsafe { probe_find_slot(new_data, new_cap, h) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                elem,
                new_data.add(new_layout.keys_offset + slot * es),
                es,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }
    }
    inc_copied_set_elements(new_data, new_cap, es, inc_fn);
    new_data
}

/// COW-aware set union with consuming semantics.
///
/// Computes `set1 ∪ set2`. Takes ownership of `d1`, borrows `d2`.
#[no_mangle]
pub extern "C" fn ori_set_union_cow(
    d1: *mut u8,
    l1: i64,
    c1: i64,
    d2: *const u8,
    l2: i64,
    c2: i64,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let _ea = elem_align.max(1) as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;
    let cap1 = c1.max(0) as usize;
    let cap2 = c2.max(0) as usize;

    // set2 empty → return set1 unchanged
    if n2 == 0 || d2.is_null() {
        write_set_struct(out_ptr, l1, c1, d1);
        return;
    }

    // set1 empty → copy set2
    if n1 == 0 || d1.is_null() {
        let new_cap = next_hash_capacity(n2);
        let new_data = unsafe { rehash_set(d2, cap2, new_cap, es, elem_hash) };
        let new_layout = HashTableLayout::for_set(new_cap, es);
        if let Some(inc) = inc_fn {
            for b in 0..new_cap {
                if unsafe { get_meta(new_data, b) } == META_OCCUPIED {
                    inc(unsafe { new_data.add(new_layout.keys_offset + b * es) });
                }
            }
        }
        ori_rc_dec(d1, None);
        write_set_struct(out_ptr, n2 as i64, new_cap as i64, new_data);
        return;
    }

    // Count how many elements from set2 are new
    let layout2 = HashTableLayout::for_set(cap2, es);
    let mut new_count = 0usize;
    for b in 0..cap2 {
        if unsafe { get_meta(d2, b) } != META_OCCUPIED {
            continue;
        }
        let elem = unsafe { d2.add(layout2.keys_offset + b * es) };
        if !hash_set_contains(d1, cap1, es, elem, elem_eq, elem_hash) {
            new_count += 1;
        }
    }

    if new_count == 0 {
        write_set_struct(out_ptr, l1, c1, d1);
        return;
    }

    let result_len = n1 + new_count;
    let is_unique = cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(d1));

    if is_unique && !needs_rehash(result_len, cap1) {
        // FAST PATH: unique, enough capacity — insert new elements in place
        let layout1 = HashTableLayout::for_set(cap1, es);
        for b in 0..cap2 {
            if unsafe { get_meta(d2, b) } != META_OCCUPIED {
                continue;
            }
            let elem = unsafe { d2.add(layout2.keys_offset + b * es) };
            if hash_set_contains(d1, cap1, es, elem, elem_eq, elem_hash) {
                continue;
            }
            let h = elem_hash(elem);
            let slot = unsafe { probe_find_slot(d1, cap1, h) };
            unsafe {
                std::ptr::copy_nonoverlapping(elem, d1.add(layout1.keys_offset + slot * es), es);
                set_meta(d1, slot, META_OCCUPIED);
            }
        }
        write_set_struct(out_ptr, result_len as i64, c1, d1);
        return;
    }

    // Slow path: rehash d1 into new buffer and add unique set2 elements
    let new_cap = next_hash_capacity(result_len);
    let new_data = rehash_and_merge_set2(
        d1, cap1, d2, cap2, layout2, new_cap, es, elem_eq, elem_hash, inc_fn,
    );

    // Release d1: free if unique (RC=1), decrement if shared (RC>1)
    if is_unique {
        let old_layout = HashTableLayout::for_set(cap1, es);
        ori_rc_free(d1, old_layout.total_size, 8);
    } else {
        ori_rc_dec(d1, None);
    }
    write_set_struct(out_ptr, result_len as i64, new_cap as i64, new_data);
}

/// COW-aware set intersection with consuming semantics.
///
/// Computes `set1 ∩ set2`. Takes ownership of `d1`, borrows `d2`.
#[no_mangle]
pub extern "C" fn ori_set_intersection_cow(
    d1: *mut u8,
    l1: i64,
    c1: i64,
    d2: *const u8,
    l2: i64,
    c2: i64,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let _ea = elem_align.max(1) as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;
    let cap1 = c1.max(0) as usize;
    let cap2 = c2.max(0) as usize;

    let d1_is_unique = !d1.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(d1)));

    // Either set empty → result is empty
    if n1 == 0 || d1.is_null() || n2 == 0 || d2.is_null() {
        if !d1.is_null() {
            let layout = HashTableLayout::for_set(cap1, es);
            if d1_is_unique {
                ori_rc_free(d1, layout.total_size, 8);
            } else {
                ori_rc_dec(d1, None);
            }
        }
        write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique — tombstone elements not in set2
    if d1_is_unique {
        let layout1 = HashTableLayout::for_set(cap1, es);
        let mut result_len = 0usize;
        for b in 0..cap1 {
            if unsafe { get_meta(d1, b) } != META_OCCUPIED {
                continue;
            }
            let elem = unsafe { d1.add(layout1.keys_offset + b * es) };
            if hash_set_contains(d2, cap2, es, elem, elem_eq, elem_hash) {
                result_len += 1;
            } else {
                unsafe { set_meta(d1, b, META_TOMBSTONE) };
            }
        }

        if result_len == 0 {
            ori_rc_free(d1, layout1.total_size, 8);
            write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        } else {
            write_set_struct(out_ptr, result_len as i64, c1, d1);
        }
        return;
    }

    // SLOW PATH: shared — build new buffer with intersection
    let layout1 = HashTableLayout::for_set(cap1, es);
    // Count intersection size first
    let mut result_len = 0usize;
    for b in 0..cap1 {
        if unsafe { get_meta(d1, b) } != META_OCCUPIED {
            continue;
        }
        let elem = unsafe { d1.add(layout1.keys_offset + b * es) };
        if hash_set_contains(d2, cap2, es, elem, elem_eq, elem_hash) {
            result_len += 1;
        }
    }

    if result_len == 0 {
        ori_rc_dec(d1, None);
        write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    let new_cap = next_hash_capacity(result_len);
    let new_data = alloc_set_hash_buffer(new_cap, es);
    let new_layout = HashTableLayout::for_set(new_cap, es);

    for b in 0..cap1 {
        if unsafe { get_meta(d1, b) } != META_OCCUPIED {
            continue;
        }
        let elem = unsafe { d1.add(layout1.keys_offset + b * es) };
        if !hash_set_contains(d2, cap2, es, elem, elem_eq, elem_hash) {
            continue;
        }
        let h = elem_hash(elem);
        let slot = unsafe { probe_find_slot(new_data, new_cap, h) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                elem,
                new_data.add(new_layout.keys_offset + slot * es),
                es,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }
    }

    inc_copied_set_elements(new_data, new_cap, es, inc_fn);
    ori_rc_dec(d1, None);
    write_set_struct(out_ptr, result_len as i64, new_cap as i64, new_data);
}

/// COW-aware set difference with consuming semantics.
///
/// Computes `set1 \ set2`. Takes ownership of `d1`, borrows `d2`.
#[no_mangle]
pub extern "C" fn ori_set_difference_cow(
    d1: *mut u8,
    l1: i64,
    c1: i64,
    d2: *const u8,
    l2: i64,
    c2: i64,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let _ea = elem_align.max(1) as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;
    let cap1 = c1.max(0) as usize;
    let cap2 = c2.max(0) as usize;

    // set1 empty → result is empty
    if n1 == 0 || d1.is_null() {
        ori_rc_dec(d1, None);
        write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // set2 empty → result is set1
    if n2 == 0 || d2.is_null() {
        write_set_struct(out_ptr, l1, c1, d1);
        return;
    }

    let is_unique = cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(d1));

    // FAST PATH: unique — tombstone elements in set2
    if is_unique {
        let layout1 = HashTableLayout::for_set(cap1, es);
        let mut result_len = n1;
        for b in 0..cap1 {
            if unsafe { get_meta(d1, b) } != META_OCCUPIED {
                continue;
            }
            let elem = unsafe { d1.add(layout1.keys_offset + b * es) };
            if hash_set_contains(d2, cap2, es, elem, elem_eq, elem_hash) {
                unsafe { set_meta(d1, b, META_TOMBSTONE) };
                result_len -= 1;
            }
        }

        if result_len == 0 {
            ori_rc_free(d1, layout1.total_size, 8);
            write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        } else {
            write_set_struct(out_ptr, result_len as i64, c1, d1);
        }
        return;
    }

    // SLOW PATH: shared — build new buffer with difference
    let layout1 = HashTableLayout::for_set(cap1, es);
    let mut result_len = 0usize;
    for b in 0..cap1 {
        if unsafe { get_meta(d1, b) } != META_OCCUPIED {
            continue;
        }
        let elem = unsafe { d1.add(layout1.keys_offset + b * es) };
        if !hash_set_contains(d2, cap2, es, elem, elem_eq, elem_hash) {
            result_len += 1;
        }
    }

    if result_len == 0 {
        ori_rc_dec(d1, None);
        write_set_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    let new_cap = next_hash_capacity(result_len);
    let new_data = alloc_set_hash_buffer(new_cap, es);
    let new_layout = HashTableLayout::for_set(new_cap, es);

    for b in 0..cap1 {
        if unsafe { get_meta(d1, b) } != META_OCCUPIED {
            continue;
        }
        let elem = unsafe { d1.add(layout1.keys_offset + b * es) };
        if hash_set_contains(d2, cap2, es, elem, elem_eq, elem_hash) {
            continue;
        }
        let h = elem_hash(elem);
        let slot = unsafe { probe_find_slot(new_data, new_cap, h) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                elem,
                new_data.add(new_layout.keys_offset + slot * es),
                es,
            );
            set_meta(new_data, slot, META_OCCUPIED);
        }
    }

    inc_copied_set_elements(new_data, new_cap, es, inc_fn);
    ori_rc_dec(d1, None);
    write_set_struct(out_ptr, result_len as i64, new_cap as i64, new_data);
}

/// Inc RC for all OCCUPIED elements in a set hash table buffer.
fn inc_copied_set_elements(
    data: *mut u8,
    cap: usize,
    elem_size: usize,
    inc_fn: Option<extern "C" fn(*mut u8)>,
) {
    if let Some(inc) = inc_fn {
        let layout = HashTableLayout::for_set(cap, elem_size);
        for b in 0..cap {
            if unsafe { get_meta(data, b) } == META_OCCUPIED {
                inc(unsafe { data.add(layout.keys_offset + b * elem_size) });
            }
        }
    }
}
