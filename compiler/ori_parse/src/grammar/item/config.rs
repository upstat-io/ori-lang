//! Constant parsing.

use crate::{committed, require, ParseOutcome, Parser};
use ori_ir::{ConstDef, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse a constant declaration.
    ///
    /// Grammar: `constant_decl = "let" "$" identifier [ ":" type ] "=" const_expr`
    /// Syntax: `[pub] let $name = expr` or `[pub] let $name: type = expr`
    ///
    /// The initializer is parsed as a general expression. Constness validation
    /// (ensuring only const-compatible constructs) happens in later phases.
    ///
    /// Returns `EmptyErr` if no `$` is present.
    pub(crate) fn parse_const(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        if !self.cursor.check(&TokenKind::Dollar) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Dollar,
                self.cursor.current_span().start as usize,
            );
        }

        self.parse_const_body(visibility)
    }

    fn parse_const_body(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        let start_span = self.cursor.current_span();

        // $
        committed!(self.cursor.expect(&TokenKind::Dollar));

        // name
        let name = committed!(self.cursor.expect_ident());

        // Optional type annotation: `: type`
        let ty = if self.cursor.check(&TokenKind::Colon) {
            self.cursor.advance();
            self.parse_type()
        } else {
            None
        };

        // =
        committed!(self.cursor.expect(&TokenKind::Eq));

        // constant expression (parsed as general expression; constness validated later)
        let value = require!(self, self.parse_expr(), "constant expression");

        let span = start_span.merge(self.cursor.previous_span());

        ParseOutcome::consumed_ok(ConstDef {
            name,
            ty,
            value,
            span,
            visibility,
        })
    }
}

#[cfg(test)]
mod tests;
