use ori_ir::{StringInterner, TokenList};

use crate::driver::lex_driver;
use crate::{LexOutput, LexResult};

/// Lex source into tokens and accumulated errors.
#[must_use]
pub fn lex_full(source: &str, interner: &StringInterner) -> LexResult {
    let output = lex_driver::<false>(source, interner);
    LexResult {
        tokens: output.tokens,
        errors: output.errors,
    }
}

/// Lex source into a token list, discarding accumulated errors.
#[must_use]
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    lex_full(source, interner).tokens
}

/// Lex source with comments and formatting metadata preserved.
#[must_use]
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    lex_driver::<true>(source, interner)
}
