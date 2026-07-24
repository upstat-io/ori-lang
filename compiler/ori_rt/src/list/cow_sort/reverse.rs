//! COW list reversal.

use crate::list::{dec_list_buffer, inc_copied_elements};
use crate::rc::ori_rc_alloc;

use super::header::propagate_header;
use crate::list::cow_context::CowMode;

const STACK_MAX: usize = 24;

/// COW-aware list reverse with consuming semantics.
#[no_mangle]
pub extern "C" fn ori_list_reverse_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;

    if data.is_null() || n <= 1 {
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        let mut stack_buf = [0u8; STACK_MAX];
        let mut heap_buf = Vec::new();
        let tmp: &mut [u8] = if es <= STACK_MAX {
            &mut stack_buf[..es]
        } else {
            heap_buf.resize(es, 0);
            &mut heap_buf
        };
        let mut lo = 0usize;
        let mut hi = n - 1;
        while lo < hi {
            unsafe {
                let lo_ptr = data.add(lo * es);
                let hi_ptr = data.add(hi * es);
                std::ptr::copy_nonoverlapping(lo_ptr, tmp.as_mut_ptr(), es);
                std::ptr::copy_nonoverlapping(hi_ptr, lo_ptr, es);
                std::ptr::copy_nonoverlapping(tmp.as_ptr(), hi_ptr, es);
            }
            lo += 1;
            hi -= 1;
        }
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    let new_data = ori_rc_alloc(n * es, ea);
    for i in 0..n {
        let src_offset = (n - 1 - i) * es;
        let dst_offset = i * es;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_offset), new_data.add(dst_offset), es);
        }
    }
    inc_copied_elements(new_data, n, es, inc_fn);
    unsafe { propagate_header(data, cap, new_data, n as i64) };
    dec_list_buffer(data, len, cap, elem_size);
    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}
