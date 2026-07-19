//! Structural trait codegen targets for maps: `Eq` (`ori_map_eq`) and
//! `Hashable` (`ori_map_hash`). Both take key/value comparison/hash thunks so
//! the runtime stays element-type-agnostic. `ori_map_hash` is order-independent
//! (XOR over entries) to honor the Hashable invariant across iteration orders.

use super::hash_table::probe_find;
use super::hash_table::{get_meta, HashTableLayout, META_OCCUPIED};
use super::OriMap;

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
    // SAFETY: After null checks, a and b are valid OriMap pointers per C-ABI contract.
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
        // SAFETY: bucket < a_cap; metadata region is within a_data buffer.
        if unsafe { get_meta(a_data, bucket) } != META_OCCUPIED {
            continue;
        }
        // SAFETY: bucket is OCCUPIED, so key and value slots are initialized.
        let a_key = unsafe { a_data.add(a_layout.keys_offset + bucket * ks) };
        let a_val = unsafe { a_data.add(a_layout.vals_offset + bucket * vs) };
        let hash = key_hash(a_key);

        // Find the same key in B
        // SAFETY: b_data/b_cap are valid; layout offsets derived from b_cap/sizes.
        let b_bucket =
            unsafe { probe_find(b_data, b_cap, b_layout.keys_offset, a_key, hash, ks, key_eq) };
        let Some(b_idx) = b_bucket else {
            return false; // key not in B
        };
        // SAFETY: b_idx is a valid OCCUPIED bucket index within b_data.
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

/// Boost `hash_combine(seed, value)`.
///
/// Identical formula to the codegen inline emitter (`emit_hash_combine`) and
/// the interpreter (`hash_value` map arm) so all three backends agree.
fn map_hash_combine(seed: i64, value: i64) -> i64 {
    seed ^ value
        .wrapping_add(0x9e37_79b9_i64)
        .wrapping_add(seed << 6)
        .wrapping_add(seed >> 2)
}

/// Hash a map, order-independently.
///
/// XORs `hash_combine(key_hash(k), val_hash(v))` over every occupied entry.
/// Order-independence (XOR) matches the interpreter's `Value::Map` hash arm,
/// preserving the Hashable invariant `a == b => hash(a) == hash(b)` for maps
/// whose entry iteration order differs.
#[no_mangle]
pub extern "C" fn ori_map_hash(
    m: *const OriMap,
    key_size: i64,
    val_size: i64,
    key_hash: extern "C" fn(*const u8) -> i64,
    val_hash: extern "C" fn(*const u8) -> i64,
) -> i64 {
    // SAFETY: after the null check, m is a valid OriMap pointer per C-ABI contract.
    let map = unsafe {
        if m.is_null() {
            return 0;
        }
        &*m
    };
    if map.len == 0 {
        return 0;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let cap = map.cap as usize;
    let layout = HashTableLayout::for_map(cap, ks, vs);
    let data = map.data.cast_const();

    let mut acc = 0_i64;
    let mut seen = 0usize;
    let n = map.len as usize;
    for bucket in 0..cap {
        // SAFETY: bucket < cap; metadata region is within the data buffer.
        if unsafe { get_meta(data, bucket) } != META_OCCUPIED {
            continue;
        }
        // SAFETY: bucket is OCCUPIED, so key and value slots are initialized.
        let key = unsafe { data.add(layout.keys_offset + bucket * ks) };
        let val = unsafe { data.add(layout.vals_offset + bucket * vs) };
        let entry = map_hash_combine(key_hash(key), val_hash(val));
        acc ^= entry;
        seen += 1;
        if seen >= n {
            break;
        }
    }

    acc
}
