//! Format specification types — the compile-time spec surface.
//!
//! The types + parser are defined once in the `ori_format` leaf crate (the
//! single source of truth shared by the interpreter and AOT runtime emission
//! backends). This module re-exports them so the established
//! `ori_ir::format_spec::*` consumer paths (type checker, canonicalizer) are
//! preserved.

pub use ori_format::{
    parse_format_spec, Align, FormatSpecError, FormatType, ParsedFormatSpec, Sign,
};
