//! COW list element replacement for `IndexSet.updated` (`list.updated(key, value)`).
//!
//! Same consuming semantics as the other COW mutations in [`super::cow`], with
//! two contract differences from `ori_list_set_cow`:
//! - Out-of-bounds keys PANIC (matching `list[index]` / `ori_list_get`).
//! - The replaced element's RC is released on the unique fast path via `dec_fn`
//!   (the new element is moved in; ownership transfers from the caller).

use crate::io::panic_index_out_of_bounds;
use crate::rc::ori_buffer_rc_dec;

use super::cow::slow_copy_replace_element;
use super::cow_context::{CowMode, ElementOps, ListBuffer};

/// COW-aware element replacement with consuming semantics (`IndexSet.updated`).
///
/// Replaces the element at `index` with `elem`. If the data buffer is
/// uniquely owned (RC==1), overwrites in place (O(1)). If shared,
/// copies the entire list and overwrites in the copy (O(n)).
///
/// # Panics
///
/// Panics if `index < 0 || index >= len` — same bounds contract as
/// `ori_list_get` (`list[index]`). Because ownership transfers at call entry,
/// the panicking path releases both the consumed receiver and moved-in value
/// before unwinding.
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

    if data.is_null() || index < 0 || index >= len {
        if let Some(dec) = dec_fn {
            dec(elem_ptr.cast_mut());
        }
        ori_buffer_rc_dec(data, len, cap, elem_size, dec_fn);
        panic_index_out_of_bounds(index, len.max(0));
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let idx = index as usize;

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        // SAFETY: Uniqueness verified; idx < len so idx * es is within the buffer.
        unsafe {
            let dst = data.add(idx * es);
            // Why: Overwriting the unique slot otherwise leaks its owned children.
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

    // Why: The old buffer retains the replaced value; the moved replacement owns its credit.
    slow_copy_replace_element(
        ListBuffer::new(data, len, cap),
        idx,
        elem_ptr,
        ElementOps::new(es, ea, inc_fn),
        out_ptr,
    );
}
