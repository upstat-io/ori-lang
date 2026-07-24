//! COW (Copy-on-Write) map mutation functions with hash table backing.
//!
//! All functions use consuming semantics: they take ownership of their input
//! buffer reference and return a new `{len, cap, data}` triple through an sret
//! output pointer.

mod insert;
mod merge_remove;

pub use insert::ori_map_insert_cow;
#[cfg(test)]
pub(crate) use insert::ori_map_update_cow;
pub use merge_remove::{ori_map_merge_cow, ori_map_remove_cow};
