//! Parser - Selective re-exports from `ori_parse`.
//!
//! Re-exports only the types downstream consumers actually need.
//! Internal parser machinery (`Cursor`, `Parser`, `TokenSet`, etc.)
//! remains accessible via `ori_parse` directly if needed.

pub use ori_parse::{parse, ParseError, ParseOutput};
