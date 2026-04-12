//! Re-exports of ARC IR instruction/terminator formatters.
//!
//! The canonical formatters live in `ori_arc::ir::format::instr`.
//! This module re-exports them for `oric`-internal consumers
//! (`arc_dot::node` uses `fmt_instr` and `fmt_terminator`).

pub use ori_arc::ir::format::instr::{fmt_instr, fmt_terminator};
