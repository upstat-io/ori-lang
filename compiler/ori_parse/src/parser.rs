//! Parser state and contextual-name cache.

use ori_ir::{ExprArena, Function, Name, StringInterner, TestDef, TokenList};

use crate::{Cursor, ParseContext, ParseError, ParseWarning};

/// Result of parsing an `@`-prefixed definition.
pub(crate) enum FunctionOrTest {
    Function(Function),
    Test(TestDef),
}

/// Pre-interned contextual-keyword names used by parser comparisons.
#[derive(Debug)]
pub(crate) struct KnownNames {
    pub(crate) channel: Name,
    pub(crate) channel_in: Name,
    pub(crate) channel_out: Name,
    pub(crate) channel_all: Name,
    pub(crate) over: Name,
    pub(crate) map: Name,
    pub(crate) match_: Name,
    pub(crate) default: Name,
    pub(crate) max: Name,
}

impl KnownNames {
    fn new(interner: &StringInterner) -> Self {
        Self {
            channel: interner.intern("channel"),
            channel_in: interner.intern("channel_in"),
            channel_out: interner.intern("channel_out"),
            channel_all: interner.intern("channel_all"),
            over: interner.intern("over"),
            map: interner.intern("map"),
            match_: interner.intern("match"),
            default: interner.intern("default"),
            max: interner.intern("max"),
        }
    }
}

/// Mutable state for one recursive-descent parse.
#[derive(Debug)]
pub struct Parser<'a> {
    pub(crate) cursor: Cursor<'a>,
    pub(crate) arena: ExprArena,
    pub(crate) context: ParseContext,
    pub(crate) known: KnownNames,
    pub(crate) deferred_errors: Vec<ParseError>,
    pub(crate) deferred_warnings: Vec<ParseWarning>,
}

impl<'a> Parser<'a> {
    /// Create parser state at the start of `tokens`.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        Self {
            cursor: Cursor::new(tokens, interner),
            arena: ExprArena::with_capacity(tokens.len() * 5),
            context: ParseContext::new(),
            known: KnownNames::new(interner),
            deferred_errors: Vec::new(),
            deferred_warnings: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn estimated_source_len(&self) -> usize {
        self.cursor.token_count() * 5
    }

    #[cfg(test)]
    pub fn take_arena(&mut self) -> ExprArena {
        std::mem::take(&mut self.arena)
    }
}
