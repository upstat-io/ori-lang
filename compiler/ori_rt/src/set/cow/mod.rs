//! COW (Copy-on-Write) set mutation functions with hash table backing.
//!
//! All functions follow consuming semantics: they take ownership of the
//! caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.
//!
//! The hash table layout is `[metadata (1 byte/bucket) | elements]` with open
//! addressing and linear probing.
//!
//! # Submodules
//!
//! - `basic` — insert and remove operations
//! - `algebra` — union, intersection, and difference operations

mod algebra;
mod basic;

pub use algebra::{ori_set_difference_cow, ori_set_intersection_cow, ori_set_union_cow};
pub use basic::{ori_set_insert_cow, ori_set_remove_cow};

use crate::map::hash_table::{get_meta, HashTableLayout, META_OCCUPIED};

/// Inc RC for all OCCUPIED elements in a set hash table buffer.
fn inc_copied_set_elements(
    data: *mut u8,
    cap: usize,
    elem_size: usize,
    inc_fn: Option<extern "C" fn(*mut u8)>,
) {
    if let Some(inc) = inc_fn {
        let layout = HashTableLayout::for_set(cap, elem_size);
        for b in 0..cap {
            if unsafe { get_meta(data, b) } == META_OCCUPIED {
                inc(unsafe { data.add(layout.keys_offset + b * elem_size) });
            }
        }
    }
}
