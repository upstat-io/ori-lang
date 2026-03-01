//! COW (Copy-on-Write) map mutation functions.
//!
//! All functions in this module follow consuming semantics: they take ownership
//! of the caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.
//!
//! The key difference from list COW is the split-buffer layout: growing via
//! realloc requires relocating the values section (it shifts right as `cap`
//! increases and the keys section expands).

use crate::list::inc_copied_elements;
use crate::next_capacity;
use crate::rc::{ori_rc_dec, ori_rc_free, ori_rc_is_unique, ori_rc_realloc};

use super::{find_key, write_map_struct, OriMap};

/// COW-aware map insert with consuming semantics.
///
/// Inserts a key-value pair into a map. The data buffer must be RC-allocated
/// (via `ori_rc_alloc`) or null (empty sentinel). This function **takes
/// ownership** of the caller's reference to `data` (consuming semantics):
///
/// - **Fast path** (unique, key exists): Overwrites value in place.
/// - **Fast path** (unique, new key, has capacity): Appends in place.
/// - **Fast path** (unique, new key, needs growth): Realloc + relocate values
///   + append. Values must be moved because the layout is `[keys|values]` and
///     growing changes where the values section starts.
/// - **Slow path** (shared or empty): New buffer allocated, old elements
///   copied, new entry appended. Old buffer's RC decremented.
///
/// # Key equality
///
/// The `key_eq` callback compares two keys pointed to by raw pointers. The
/// codegen generates type-specific comparators (e.g., inline `i64 ==` for int
/// keys, `ori_str_eq` for string keys).
///
/// # Element RC
///
/// On the slow path, byte-copied keys get their RC incremented via `key_inc`,
/// and byte-copied values via `val_inc`. Pass null for scalar types.
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
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
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() || value.is_null() {
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Linear scan for existing key
    let found_idx = find_key(data, n, ks, key, key_eq);

    if let Some(idx) = found_idx {
        // Key exists — overwrite value
        cow_insert_existing(data, n, c, ks, vs, idx, value, key_inc, val_inc, out_ptr);
    } else {
        // New key — append
        cow_insert_new(data, n, c, ks, vs, key, value, key_inc, val_inc, out_ptr);
    }
}

/// COW insert when key already exists at `idx` — overwrite value.
#[expect(
    clippy::too_many_arguments,
    reason = "COW map parameters cannot be grouped — data/len/cap/key_size/val_size/idx/value/inc fns are all independent scalars from the split-buffer layout"
)]
fn cow_insert_existing(
    data: *mut u8,
    len: usize,
    cap: usize,
    ks: usize,
    vs: usize,
    idx: usize,
    value: *const u8,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if !data.is_null() && ori_rc_is_unique(data) {
        // FAST PATH: unique — overwrite value in place
        unsafe {
            let val_dst = data.add(cap * ks + idx * vs);
            std::ptr::copy_nonoverlapping(value, val_dst, vs);
            write_map_struct(out_ptr, len as i64, cap as i64, data);
        }
        return;
    }

    // SLOW PATH: shared — copy all, overwrite value at idx
    let new_data = OriMap::alloc_buffer(len, ks, vs);
    unsafe {
        // Copy all keys
        std::ptr::copy_nonoverlapping(data, new_data, len * ks);
        // Copy all values
        let old_vals = data.add(cap * ks);
        let new_vals = new_data.add(len * ks);
        std::ptr::copy_nonoverlapping(old_vals, new_vals, len * vs);
        // Overwrite value at idx
        std::ptr::copy_nonoverlapping(value, new_vals.add(idx * vs), vs);
    }

    // Inc RC for all copied keys
    inc_copied_elements(new_data, len, ks, key_inc);
    // Inc RC for all copied values EXCEPT the one at idx (it was overwritten)
    let new_vals = unsafe { new_data.add(len * ks) };
    inc_copied_elements(new_vals, idx, vs, val_inc);
    if idx + 1 < len {
        inc_copied_elements(
            unsafe { new_vals.add((idx + 1) * vs) },
            len - idx - 1,
            vs,
            val_inc,
        );
    }

    ori_rc_dec(data, None);
    write_map_struct(out_ptr, len as i64, len as i64, new_data);
}

/// COW insert for a new key — append to end.
#[expect(
    clippy::too_many_arguments,
    reason = "COW map parameters cannot be grouped — data/len/cap/key_size/val_size/key/value/inc fns are all independent scalars from the split-buffer layout"
)]
fn cow_insert_new(
    data: *mut u8,
    len: usize,
    cap: usize,
    ks: usize,
    vs: usize,
    key: *const u8,
    value: *const u8,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    let new_len = len + 1;

    // FAST PATH: unique owner — can mutate in place
    if !data.is_null() && ori_rc_is_unique(data) {
        if cap >= new_len {
            // Has capacity — write key + value in place
            unsafe {
                std::ptr::copy_nonoverlapping(key, data.add(len * ks), ks);
                std::ptr::copy_nonoverlapping(value, data.add(cap * ks + len * vs), vs);
                write_map_struct(out_ptr, new_len as i64, cap as i64, data);
            }
            return;
        }

        // Needs growth — realloc + relocate values
        let new_cap = next_capacity(cap, new_len);
        let old_total = OriMap::buffer_size(cap, ks, vs);
        let new_total = OriMap::buffer_size(new_cap, ks, vs);
        let new_data = ori_rc_realloc(data, old_total, new_total, 8);
        if new_data.is_null() {
            // Realloc failed — return original unchanged
            write_map_struct(out_ptr, len as i64, cap as i64, data);
            return;
        }

        unsafe {
            // Relocate values from old offset to new offset.
            // Source: new_data + cap * ks (old values position after realloc)
            // Dest:   new_data + new_cap * ks (new values position)
            // The regions may overlap (dest > src), so use memmove.
            std::ptr::copy(new_data.add(cap * ks), new_data.add(new_cap * ks), len * vs);

            // Write new key + value
            std::ptr::copy_nonoverlapping(key, new_data.add(len * ks), ks);
            std::ptr::copy_nonoverlapping(value, new_data.add(new_cap * ks + len * vs), vs);
            write_map_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
        }
        return;
    }

    // SLOW PATH: shared or empty — allocate new buffer
    let base_cap = if data.is_null() { 0 } else { cap };
    let new_cap = next_capacity(base_cap, new_len);
    let new_data = OriMap::alloc_buffer(new_cap, ks, vs);

    if !data.is_null() && len > 0 {
        unsafe {
            // Copy existing keys
            std::ptr::copy_nonoverlapping(data, new_data, len * ks);
            // Copy existing values to new offset
            let old_vals = data.add(cap * ks);
            let new_vals = new_data.add(new_cap * ks);
            std::ptr::copy_nonoverlapping(old_vals, new_vals, len * vs);
        }

        // Inc RC for all copied keys and values
        inc_copied_elements(new_data, len, ks, key_inc);
        inc_copied_elements(unsafe { new_data.add(new_cap * ks) }, len, vs, val_inc);
    }

    // Write new key + value
    unsafe {
        std::ptr::copy_nonoverlapping(key, new_data.add(len * ks), ks);
        std::ptr::copy_nonoverlapping(value, new_data.add(new_cap * ks + len * vs), vs);
    }

    // Release old buffer reference
    ori_rc_dec(data, None);

    write_map_struct(out_ptr, new_len as i64, new_cap as i64, new_data);
}

/// COW-aware map remove with consuming semantics.
///
/// Removes the entry with the given key. The data buffer must be RC-allocated
/// (via `ori_rc_alloc`) or null (empty sentinel). This function **takes
/// ownership** of the caller's reference to `data`:
///
/// - **Fast path** (unique, key found): Shifts remaining entries left in both
///   keys and values regions. Same buffer returned with `len - 1`.
/// - **Fast path** (unique, last entry): Frees the buffer, returns empty
///   sentinel.
/// - **No-op** (key not found): Returns input unchanged regardless of sharing.
/// - **Slow path** (shared, key found): Allocates a new buffer, copies all
///   entries except the removed one. Old buffer's RC decremented.
///
/// # Element RC
///
/// On the slow path, byte-copied keys get their RC incremented via `key_inc`,
/// and byte-copied values via `val_inc`. Pass null for scalar types.
/// The removed entry's RC is NOT decremented here — the caller (codegen)
/// is responsible for decrementing the removed key and value.
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_map_remove_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    key: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
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
    let Some(idx) = find_key(data, n, ks, key, key_eq) else {
        // Key not found — return input unchanged
        write_map_struct(out_ptr, len, cap, data);
        return;
    };

    let new_len = n - 1;
    let tail = n - idx - 1; // entries after the removed one

    // Special case: removing last entry → empty sentinel
    if new_len == 0 {
        if !data.is_null() {
            if ori_rc_is_unique(data) {
                ori_rc_free(data, OriMap::buffer_size(c, ks, vs), 8);
            } else {
                ori_rc_dec(data, None);
            }
        }
        write_map_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique owner — shift left in place
    if !data.is_null() && ori_rc_is_unique(data) {
        unsafe {
            if tail > 0 {
                // Shift keys left (overlapping)
                std::ptr::copy(data.add((idx + 1) * ks), data.add(idx * ks), tail * ks);
                // Shift values left (overlapping)
                let vals_base = data.add(c * ks);
                std::ptr::copy(
                    vals_base.add((idx + 1) * vs),
                    vals_base.add(idx * vs),
                    tail * vs,
                );
            }
        }
        write_map_struct(out_ptr, new_len as i64, cap, data);
        return;
    }

    // SLOW PATH: shared — allocate new buffer, copy all except removed entry
    let new_data = OriMap::alloc_buffer(new_len, ks, vs);

    unsafe {
        // Copy keys: [0..idx] then [idx+1..n]
        if idx > 0 {
            std::ptr::copy_nonoverlapping(data, new_data, idx * ks);
        }
        if tail > 0 {
            std::ptr::copy_nonoverlapping(
                data.add((idx + 1) * ks),
                new_data.add(idx * ks),
                tail * ks,
            );
        }

        // Copy values: [0..idx] then [idx+1..n] at new offset
        let old_vals = data.add(c * ks);
        let new_vals = new_data.add(new_len * ks);
        if idx > 0 {
            std::ptr::copy_nonoverlapping(old_vals, new_vals, idx * vs);
        }
        if tail > 0 {
            std::ptr::copy_nonoverlapping(
                old_vals.add((idx + 1) * vs),
                new_vals.add(idx * vs),
                tail * vs,
            );
        }
    }

    // Inc RC for all copied keys and values (contiguous in new buffer)
    inc_copied_elements(new_data, new_len, ks, key_inc);
    let new_vals = unsafe { new_data.add(new_len * ks) };
    inc_copied_elements(new_vals, new_len, vs, val_inc);

    // Release old buffer reference
    ori_rc_dec(data, None);

    write_map_struct(out_ptr, new_len as i64, new_len as i64, new_data);
}

/// COW-aware map update (value replacement) with consuming semantics.
///
/// Replaces the value for an existing key. If the key is not found, returns
/// the input unchanged (no insertion). This is the "update-only" counterpart
/// to `ori_map_insert_cow` — it avoids the "new key" allocation paths and
/// eliminates the double-lookup overhead of `get` + `insert`.
///
/// - **Fast path** (unique, key found): Overwrites value in place.
/// - **No-op** (key not found): Returns input unchanged.
/// - **Slow path** (shared, key found): Copies all entries, overwrites value
///   in the copy. Old buffer's RC decremented.
///
/// # Element RC
///
/// Same semantics as `ori_map_insert_cow`: on the slow path, byte-copied keys
/// and values get their RC incremented via `key_inc` / `val_inc`. The replaced
/// value at `idx` is NOT incremented (it was overwritten with `new_value`).
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
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
    key_inc: Option<extern "C" fn(*mut u8)>,
    val_inc: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() || new_value.is_null() {
        return;
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    let Some(idx) = find_key(data, n, ks, key, key_eq) else {
        // Key not found — return unchanged (no insertion)
        write_map_struct(out_ptr, len, cap, data);
        return;
    };

    // Key found — delegate to the existing "overwrite value at idx" logic
    cow_insert_existing(
        data, n, c, ks, vs, idx, new_value, key_inc, val_inc, out_ptr,
    );
}
