//! Copy-on-write map insertion with consuming value ownership.
//!
//! The key remains borrowed. Ownership of the inserted value transfers from
//! the caller into the returned map.

use super::cow::ori_map_insert_cow;

/// COW-aware map insert-or-replace with consuming value semantics.
///
/// Insert or replace a map entry without panicking.
///
/// The returned map retains its copied key and value. The key remains borrowed,
/// while ownership of `value` transfers to the map and must not be released by
/// the caller.
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

    // Why: The insertion retains its copy; decrementing `value` completes the move.
    if let Some(dec) = val_dec {
        dec(value.cast_mut());
    }
}
