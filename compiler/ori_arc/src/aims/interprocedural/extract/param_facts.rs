//! Per-parameter fact detection: iter-consume transfer and
//! borrowed-read-only forwarding safety.

mod borrowed_facts;
mod iter_consume;

pub(super) use borrowed_facts::{
    find_borrowed_cow_consumed_params, find_borrowed_read_only_params, CowConsumeScope,
};
pub(crate) use iter_consume::find_iter_consume_call_args;
pub(super) use iter_consume::{find_aggregate_iter_consume_fields, find_iter_consume_params};
