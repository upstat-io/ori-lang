//! COW map insert-or-replace for `IndexSet.updated` (`map.updated(key, value)`).
//!
//! Same hash-table COW substrate as [`super::cow::ori_map_insert_cow`], with
//! one contract difference: the inserted `value` is **moved in** (ownership
//! transfers from the caller), matching `ori_list_updated_cow`.

use super::cow::ori_map_insert_cow;

/// COW-aware map insert-or-replace with consuming value semantics.
///
/// Behaves exactly like `ori_map_insert_cow` (insert-or-replace, never
/// panics), then releases the caller's reference to `value` — the insert
/// paths copy the value bytes into the buffer and increment its RC children,
/// so the extra release completes the ownership transfer (move) from the
/// caller's temporary into the map.
///
/// # Element RC
///
/// - `key` is borrowed: the buffer copy gets its own increment; the caller
///   still owns (and releases) its reference.
/// - `value` is moved: the caller must NOT release it after the call.
#[no_mangle]
pub extern "C" fn ori_map_updated_cow(
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
    key_dec: Option<extern "C" fn(*mut u8)>,
    val_dec: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() || value.is_null() {
        return;
    }

    ori_map_insert_cow(
        data, len, cap, key, value, key_size, val_size, key_eq, key_hash, key_inc, val_inc,
        key_dec, val_dec, cow_mode, out_ptr,
    );

    // Move semantics: the buffer holds its own incremented reference; release
    // the caller's reference to complete the transfer.
    if let Some(dec) = val_dec {
        dec(value.cast_mut());
    }
}
