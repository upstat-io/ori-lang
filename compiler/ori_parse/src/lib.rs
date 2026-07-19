//! Recursive-descent Ori parser producing a flat arena-backed AST.

mod api;
mod context;
mod cursor;
mod dispatch;
mod error;
mod foreign_keywords;
mod grammar;
pub mod incremental;
mod module_parse;
mod outcome;
mod output;
mod parser;
mod parser_capture;
mod parser_context;
mod recovery;
pub mod series;
mod snapshot;

pub use api::{parse, parse_incremental, parse_with_metadata};
pub use context::ParseContext;
pub(crate) use cursor::Cursor;
pub use error::{DetachmentReason, ErrorContext, ParseError, ParseErrorKind, ParseWarning};
pub(crate) use grammar::ParsedAttrs;
pub use outcome::ParseOutcome;
pub use output::ParseOutput;
pub(crate) use parser::FunctionOrTest;
pub use parser::Parser;
pub use recovery::{synchronize, TokenSet, FUNCTION_BOUNDARY, STMT_BOUNDARY};
pub use series::{SeriesConfig, TrailingSeparator};

#[cfg(test)]
mod tests;
