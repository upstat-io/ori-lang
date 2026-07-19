//! Iterator consumer functions — terminal operations that consume the iterator.
//!
//! All consumers take ownership of the iterator handle and free it when done.
//! Functions are `#[no_mangle] extern "C-unwind"` for LLVM codegen because
//! advancing an adapter chain may invoke a user closure that panics.

mod collect;
mod fold;
mod join;
mod predicates;
mod reverse;

pub use collect::{ori_iter_collect, ori_iter_collect_set};
pub use fold::{ori_iter_fold, ori_iter_last};
pub use join::ori_iter_join;
pub use predicates::{
    ori_iter_all, ori_iter_any, ori_iter_count, ori_iter_find, ori_iter_for_each,
};
pub use reverse::{ori_iter_rfind, ori_iter_rfold};

pub(super) use super::take_iter;
