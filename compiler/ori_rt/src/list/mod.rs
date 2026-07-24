//! List storage, lifecycle, queries, and copy-on-write operations.

mod cow;
mod cow_context;
mod cow_sort;
mod cow_structural;
mod cow_updated;
mod prelude;
mod query;
mod reset;
pub mod slice;
mod storage;

pub use prelude::*;
pub use storage::{
    ori_list_alloc_data, ori_list_box_new, ori_list_empty, ori_list_ensure_capacity, ori_list_free,
    ori_list_free_data, ori_list_len, ori_list_new, ori_list_push, ori_list_push_new,
    ori_list_take, OriList,
};

pub(crate) use storage::{
    dec_list_buffer, inc_copied_elements, write_array_to_list, write_list_output,
};
