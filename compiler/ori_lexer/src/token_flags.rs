//! Per-token whitespace and metadata flags.
//!
//! Re-exported from `ori_ir::TokenFlags`. The canonical definition lives in
//! `ori_ir` so the parser cursor can access flags directly without depending
//! on the lexer crate.

pub use ori_ir::TokenFlags;
