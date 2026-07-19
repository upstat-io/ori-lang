//! Scalar format-spec parsing + emission — the single source of truth for
//! `{value:spec}` template interpolation.
//!
//! Both execution backends consume this crate: the interpreter
//! (`ori_eval::interpreter::format`) and the AOT runtime (`ori_rt::format`
//! C-ABI entry points). Hosting the spec types, the parser, and the emission
//! kernel here makes interpreter<->runtime formatting parity a structural
//! guarantee rather than a hand-maintained mirror.
//!
//! Std-only, zero workspace dependencies, so `ori_rt` can link it into lean
//! AOT binaries.
//!
//! Spec: format spec syntax `[[fill]align][sign][#][0][width][.precision][type]`.

mod emit;
mod spec;

pub use emit::{format_bool, format_char, format_float, format_int, format_str};
pub use spec::{parse_format_spec, Align, FormatSpecError, FormatType, ParsedFormatSpec, Sign};

#[cfg(test)]
mod tests;
