//! COW list element replacement for `IndexSet.updated` (`list.updated(key, value)`).
//!
//! Same consuming semantics as the other COW mutations in [`super::cow`], with
//! two contract differences from `ori_list_set_cow`:
//! - Out-of-bounds keys PANIC (matching `list[index]` / `ori_list_get`).
//! - The replaced element's RC is released on the unique fast path via `dec_fn`
//!   (the new element is moved in; ownership transfers from the caller).

use crate::io::ori_panic_cstr;
use crate::rc::ori_rc_is_unique;
use crate::slice_encoding::is_slice_cap;

use super::cow::slow_copy_replace_element;

/// COW-aware element replacement with consuming semantics (`IndexSet.updated`).
///
/// Replaces the element at `index` with `elem`. If the data buffer is
/// uniquely owned (RC==1), overwrites in place (O(1)). If shared,
/// copies the entire list and overwrites in the copy (O(n)).
///
/// # Panics
///
/// Panics if `index < 0 || index >= len` (via `ori_panic_cstr`, which unwinds)
/// — same bounds contract as `ori_list_get` (`list[index]`).
///
/// # Element RC
///
/// The new element is **moved in**: ownership transfers from the caller's
/// temporary to the list, so the caller must NOT release it after the call.
/// On the unique fast path the replaced element's RC children are released
/// via `dec_fn` before the overwrite. On the shared slow path the old buffer
/// retains the replaced element; `dec_fn` is not called (the old buffer's
/// reference release covers it).
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C-unwind" fn ori_list_updated_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    index: i64,
    elem_ptr: *const u8,
    elem_size: i64,
    elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    dec_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_ptr.is_null() {
        return;
    }

    // Bounds check — panics like `list[index]` (a null buffer is the empty
    // sentinel, so every index is out of bounds for it).
    if data.is_null() || index < 0 || index >= len {
        ori_panic_cstr(c"index out of bounds".as_ptr());
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let idx = index as usize;

    // FAST PATH: unique owner, non-slice — overwrite in place.
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique =
        !is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        // SAFETY: Uniqueness verified; idx < len so idx * es is within the buffer.
        unsafe {
            let dst = data.add(idx * es);
            // Release the replaced element's RC children before overwriting.
            if let Some(dec) = dec_fn {
                dec(dst);
            }
            std::ptr::copy_nonoverlapping(elem_ptr, dst, es);
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // SLOW PATH: shared — copy entire list, overwrite in copy. The old
    // buffer keeps its reference to the replaced element; the moved-in
    // element carries the caller's reference (no inc).
    slow_copy_replace_element(data, len, cap, idx, elem_ptr, es, ea, inc_fn, out_ptr);
}
