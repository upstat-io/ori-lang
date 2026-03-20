//! Open-addressing hash table core logic.
//!
//! Pure internal module — no `extern "C"` functions. Provides layout computation,
//! metadata operations, probing, capacity management, and rehashing for both
//! maps and sets.
//!
//! # Memory Layout (Map)
//!
//! ```text
//! [RC header (32 bytes)] [metadata (1 byte/bucket)] [padding] [keys] [values]
//! ```
//!
//! # Memory Layout (Set)
//!
//! ```text
//! [RC header (32 bytes)] [metadata (1 byte/bucket)] [padding] [elements]
//! ```
//!
//! - Metadata: 1 byte per bucket — EMPTY (0x00), OCCUPIED (0x01), TOMBSTONE (0xFF)
//! - Capacity: always power-of-two (minimum 4) for `hash & (cap - 1)` masking
//! - Load factor: 75% — rehash when `len >= cap * 3 / 4`
//! - Probing: linear with wrapping

/// Metadata byte for an empty bucket.
pub(crate) const META_EMPTY: u8 = 0x00;
/// Metadata byte for an occupied bucket.
pub(crate) const META_OCCUPIED: u8 = 0x01;
/// Metadata byte for a tombstone (deleted entry).
pub(crate) const META_TOMBSTONE: u8 = 0xFF;

/// Minimum hash table capacity (must be power of two).
const MIN_HASH_CAPACITY: usize = 4;

/// Load factor numerator (75% = 3/4).
const LOAD_FACTOR_NUM: usize = 3;
/// Load factor denominator.
const LOAD_FACTOR_DEN: usize = 4;

// Layout Computation

/// Describes the byte layout of a hash table data buffer.
///
/// All offsets are relative to the data pointer (after the RC header).
#[derive(Debug, Clone, Copy)]
pub(crate) struct HashTableLayout {
    /// Number of bytes for metadata (cap bytes, rounded up to 8-byte alignment).
    pub metadata_bytes: usize,
    /// Byte offset where keys/elements start.
    pub keys_offset: usize,
    /// Byte offset where values start (0 for sets).
    pub vals_offset: usize,
    /// Total byte size of the data buffer.
    pub total_size: usize,
}

impl HashTableLayout {
    /// Compute layout for a map with given capacity, key size, and value size.
    #[inline]
    pub fn for_map(cap: usize, key_size: usize, val_size: usize) -> Self {
        let metadata_bytes = align_up(cap, 8);
        let keys_offset = metadata_bytes;
        let vals_offset = keys_offset + cap * key_size;
        let total_size = vals_offset + cap * val_size;
        Self {
            metadata_bytes,
            keys_offset,
            vals_offset,
            total_size,
        }
    }

    /// Compute layout for a set with given capacity and element size.
    #[inline]
    pub fn for_set(cap: usize, elem_size: usize) -> Self {
        let metadata_bytes = align_up(cap, 8);
        let keys_offset = metadata_bytes;
        let total_size = keys_offset + cap * elem_size;
        Self {
            metadata_bytes,
            keys_offset,
            vals_offset: 0,
            total_size,
        }
    }
}

/// Round `n` up to the next multiple of `align`.
#[inline]
const fn align_up(n: usize, align: usize) -> usize {
    (n + align - 1) & !(align - 1)
}

// Metadata Operations

/// Read the metadata byte for a given bucket.
///
/// # Safety
/// `data` must be valid and `bucket < cap`.
#[inline]
pub(crate) unsafe fn get_meta(data: *const u8, bucket: usize) -> u8 {
    *data.add(bucket)
}

/// Write a metadata byte for a given bucket.
///
/// # Safety
/// `data` must be valid and `bucket < cap`.
#[inline]
pub(crate) unsafe fn set_meta(data: *mut u8, bucket: usize, value: u8) {
    *data.add(bucket) = value;
}

// Probing

/// Probe for an existing key in the hash table.
///
/// Returns the bucket index if the key is found, `None` otherwise.
/// Uses linear probing with wrapping. Stops at EMPTY buckets (TOMBSTONE
/// is skipped — probing must continue past deleted entries).
///
/// # Safety
/// `data` must point to a valid hash table buffer with `cap` buckets.
pub(crate) unsafe fn probe_find(
    data: *const u8,
    cap: usize,
    keys_offset: usize,
    key: *const u8,
    hash: i64,
    key_size: usize,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> Option<usize> {
    if cap == 0 {
        return None;
    }
    let mask = cap - 1;
    let start = (hash as u64 as usize) & mask;

    for probe in 0..cap {
        let bucket = (start + probe) & mask;
        let meta = get_meta(data, bucket);
        match meta {
            META_EMPTY => return None,
            META_OCCUPIED => {
                let bucket_key = data.add(keys_offset + bucket * key_size);
                if key_eq(bucket_key, key) {
                    return Some(bucket);
                }
            }
            _ => {} // TOMBSTONE — continue probing
        }
    }
    None
}

/// Find an empty or tombstone slot for insertion.
///
/// Returns the bucket index of the first EMPTY or TOMBSTONE slot.
/// This is called AFTER determining the key does not exist (via `probe_find`),
/// so we only need to find any available slot.
///
/// # Safety
/// `data` must point to a valid hash table buffer with `cap` buckets.
/// The table must not be full (caller must check load factor first).
pub(crate) unsafe fn probe_find_slot(data: *const u8, cap: usize, hash: i64) -> usize {
    debug_assert!(cap > 0, "probe_find_slot called on empty table");
    let mask = cap - 1;
    let start = (hash as u64 as usize) & mask;

    for probe in 0..cap {
        let bucket = (start + probe) & mask;
        let meta = get_meta(data, bucket);
        if meta == META_EMPTY || meta == META_TOMBSTONE {
            return bucket;
        }
    }
    // Should never happen if load factor is maintained
    unreachable!("hash table is full — load factor check failed");
}

// Capacity Management

/// Compute the smallest power-of-two capacity that can hold `required` entries
/// at 75% load factor.
///
/// Returns at least `MIN_HASH_CAPACITY` (4).
pub(crate) fn next_hash_capacity(required: usize) -> usize {
    if required == 0 {
        return MIN_HASH_CAPACITY;
    }
    // We need cap such that required <= cap * 3 / 4
    // i.e., cap >= required * 4 / 3 (rounded up)
    let min_cap = required
        .saturating_mul(LOAD_FACTOR_DEN)
        .div_ceil(LOAD_FACTOR_NUM);
    let min_cap = min_cap.max(MIN_HASH_CAPACITY);
    min_cap.next_power_of_two()
}

/// Check whether the table needs rehashing to accommodate one more entry.
///
/// Returns true when `len >= cap * 3 / 4`.
#[inline]
pub(crate) fn needs_rehash(len: usize, cap: usize) -> bool {
    if cap == 0 {
        return true;
    }
    // len >= cap * 3 / 4
    len * LOAD_FACTOR_DEN >= cap * LOAD_FACTOR_NUM
}

// Rehashing

/// Rehash a map's hash table into a new buffer with `new_cap` buckets.
///
/// Allocates a new RC buffer, scans the old table for OCCUPIED entries,
/// and re-inserts them using the provided `key_hash` function.
///
/// Returns the new data pointer (RC-allocated). The old buffer is NOT freed.
///
/// # Safety
/// `old_data` must point to a valid hash table buffer with `old_cap` buckets.
pub(crate) unsafe fn rehash_map(
    old_data: *const u8,
    old_cap: usize,
    new_cap: usize,
    key_size: usize,
    val_size: usize,
    key_hash: extern "C" fn(*const u8) -> i64,
) -> *mut u8 {
    let old_layout = HashTableLayout::for_map(old_cap, key_size, val_size);
    let new_layout = HashTableLayout::for_map(new_cap, key_size, val_size);
    let new_data = crate::rc::ori_rc_alloc(new_layout.total_size, 8);

    // Initialize all metadata to EMPTY
    std::ptr::write_bytes(new_data, META_EMPTY, new_layout.metadata_bytes);

    // Re-insert all OCCUPIED entries from old table
    for bucket in 0..old_cap {
        if get_meta(old_data, bucket) != META_OCCUPIED {
            continue;
        }
        let old_key = old_data.add(old_layout.keys_offset + bucket * key_size);
        let old_val = old_data.add(old_layout.vals_offset + bucket * val_size);
        let hash = key_hash(old_key);
        let new_bucket = probe_find_slot(new_data, new_cap, hash);

        // Copy key and value
        let new_key = new_data.add(new_layout.keys_offset + new_bucket * key_size);
        let new_val = new_data.add(new_layout.vals_offset + new_bucket * val_size);
        std::ptr::copy_nonoverlapping(old_key, new_key, key_size);
        std::ptr::copy_nonoverlapping(old_val, new_val, val_size);

        set_meta(new_data, new_bucket, META_OCCUPIED);
    }

    new_data
}

/// Rehash a set's hash table into a new buffer with `new_cap` buckets.
///
/// Same as `rehash_map` but for single-element (no values) layout.
///
/// # Safety
/// `old_data` must point to a valid hash table buffer with `old_cap` buckets.
pub(crate) unsafe fn rehash_set(
    old_data: *const u8,
    old_cap: usize,
    new_cap: usize,
    elem_size: usize,
    elem_hash: extern "C" fn(*const u8) -> i64,
) -> *mut u8 {
    let old_layout = HashTableLayout::for_set(old_cap, elem_size);
    let new_layout = HashTableLayout::for_set(new_cap, elem_size);
    let new_data = crate::rc::ori_rc_alloc(new_layout.total_size, 8);

    // Initialize all metadata to EMPTY
    std::ptr::write_bytes(new_data, META_EMPTY, new_layout.metadata_bytes);

    // Re-insert all OCCUPIED entries from old table
    for bucket in 0..old_cap {
        if get_meta(old_data, bucket) != META_OCCUPIED {
            continue;
        }
        let old_elem = old_data.add(old_layout.keys_offset + bucket * elem_size);
        let hash = elem_hash(old_elem);
        let new_bucket = probe_find_slot(new_data, new_cap, hash);

        let new_elem = new_data.add(new_layout.keys_offset + new_bucket * elem_size);
        std::ptr::copy_nonoverlapping(old_elem, new_elem, elem_size);
        set_meta(new_data, new_bucket, META_OCCUPIED);
    }

    new_data
}

// Counting

/// Count the number of OCCUPIED entries in a hash table.
///
/// Used for debug assertions and validation.
#[cfg(test)]
pub(crate) unsafe fn count_occupied(data: *const u8, cap: usize) -> usize {
    let mut count = 0;
    for bucket in 0..cap {
        if get_meta(data, bucket) == META_OCCUPIED {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests;
