//! Structural equality and hashing for sets.

use super::OriSet;
use crate::map::hash_table::{get_meta, probe_find, HashTableLayout, META_OCCUPIED};

/// Compare two sets by membership, independent of bucket order.
#[no_mangle]
pub extern "C" fn ori_set_eq(
    a: *const OriSet,
    b: *const OriSet,
    elem_size: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
) -> bool {
    // SAFETY: after the null checks, both pointers satisfy the C-ABI set
    // representation contract.
    let (left, right) = unsafe {
        if a.is_null() || b.is_null() {
            return a.is_null() && b.is_null();
        }
        (&*a, &*b)
    };

    if left.len != right.len {
        return false;
    }
    if left.len == 0 {
        return true;
    }
    if left.data == right.data && left.cap == right.cap {
        return true;
    }
    if left.data.is_null()
        || right.data.is_null()
        || left.cap <= 0
        || right.cap <= 0
        || left.len < 0
    {
        return false;
    }

    let elem_size = elem_size.max(1) as usize;
    let left_cap = left.cap as usize;
    let right_cap = right.cap as usize;
    let left_layout = HashTableLayout::for_set(left_cap, elem_size);
    let right_layout = HashTableLayout::for_set(right_cap, elem_size);
    let mut checked = 0_usize;
    let expected = left.len as usize;

    for bucket in 0..left_cap {
        // SAFETY: `bucket < left_cap`; metadata is the first `left_cap`
        // bytes in a valid set buffer.
        if unsafe { get_meta(left.data, bucket) } != META_OCCUPIED {
            continue;
        }
        // SAFETY: an occupied bucket owns one initialized element slot.
        let element = unsafe { left.data.add(left_layout.keys_offset + bucket * elem_size) };
        let hash = elem_hash(element);
        // SAFETY: the right buffer and layout are valid for `right_cap`
        // buckets; `probe_find` bounds its linear probe by that capacity.
        let found = unsafe {
            probe_find(
                right.data,
                right_cap,
                right_layout.keys_offset,
                element,
                hash,
                elem_size,
                elem_eq,
            )
        };
        if found.is_none() {
            return false;
        }
        checked += 1;
        if checked == expected {
            return true;
        }
    }

    false
}

/// Hash a set independently of bucket order.
#[no_mangle]
pub extern "C" fn ori_set_hash(
    set: *const OriSet,
    elem_size: i64,
    elem_hash: extern "C" fn(*const u8) -> i64,
) -> i64 {
    // SAFETY: after the null check, the pointer satisfies the C-ABI set
    // representation contract.
    let set = unsafe {
        if set.is_null() {
            return 0;
        }
        &*set
    };
    if set.len <= 0 || set.cap <= 0 || set.data.is_null() {
        return 0;
    }

    let elem_size = elem_size.max(1) as usize;
    let capacity = set.cap as usize;
    let layout = HashTableLayout::for_set(capacity, elem_size);
    let expected = set.len as usize;
    let mut seen = 0_usize;
    let mut hash = 0_i64;

    for bucket in 0..capacity {
        // SAFETY: `bucket < capacity`; metadata is the first `capacity`
        // bytes in a valid set buffer.
        if unsafe { get_meta(set.data, bucket) } != META_OCCUPIED {
            continue;
        }
        // SAFETY: an occupied bucket owns one initialized element slot.
        let element = unsafe { set.data.add(layout.keys_offset + bucket * elem_size) };
        hash ^= elem_hash(element);
        seen += 1;
        if seen == expected {
            break;
        }
    }

    hash
}

#[cfg(test)]
mod tests;
