//! Flat runtime ABI exports consumed by compiler backends and integration tests.

pub use crate::abi::{OriOption, OriResult};
pub use crate::allocation::{ori_alloc, ori_free, ori_realloc};
pub use crate::entry::{
    ori_args_cleanup, ori_args_from_argv, ori_check_leaks, ori_eh_personality_addr, ori_run_main,
    ori_thread_id,
};
pub use crate::integer::{ori_compare_int, ori_max_int, ori_min_int};
pub use crate::io::*;
pub use crate::list::*;
pub use crate::rc::*;
pub use crate::string::*;
