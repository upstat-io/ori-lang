//! Element-header propagation for copied COW buffers.

use crate::rc::{load_elem_dec_fn, store_elem_count, store_elem_dec_fn};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

pub(super) unsafe fn propagate_header(
    source: *mut u8,
    source_cap: i64,
    destination: *mut u8,
    count: i64,
) {
    let header_source = if is_slice_cap(source_cap) {
        slice_original_data(source, source_cap)
    } else {
        source
    };
    store_elem_dec_fn(destination, load_elem_dec_fn(header_source));
    store_elem_count(destination, count);
}
