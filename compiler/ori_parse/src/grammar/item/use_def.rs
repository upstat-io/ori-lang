//! Import/use statement parsing.

use ori_ir::{ImportPath, TokenKind, UseDef, UseItem};
use crate::{ParseError, Parser};

impl Parser<'_> {
    /// Parse a use/import statement.
    /// Syntax: use './path' { item1, item2 as alias } or use std.math { sqrt }
    pub(crate) fn parse_use(&mut self) -> Result<UseDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Use)?;

        // Parse import path
        let path = if let TokenKind::String(s) = self.current_kind() {
            // Relative path: './math', '../utils'
            self.advance();
            ImportPath::Relative(s)
        } else {
            // Module path: std.math, std.collections
            let mut segments = Vec::new();
            loop {
                let name = self.expect_ident()?;
                segments.push(name);

                if self.check(&TokenKind::Dot) {
                    self.advance();
                } else {
                    break;
                }
            }
            ImportPath::Module(segments)
        };

        // Parse imported items: { item1, item2 as alias }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            // Check for private import prefix ::
            let is_private = if self.check(&TokenKind::DoubleColon) {
                self.advance();
                true
            } else {
                false
            };

            // Item name
            let name = self.expect_ident()?;

            // Optional alias: `as alias`
            let alias = if self.check(&TokenKind::As) {
                self.advance();
                Some(self.expect_ident()?)
            } else {
                None
            };

            items.push(UseItem { name, alias, is_private });

            // Comma separator (optional before closing brace)
            if self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                self.skip_newlines();
                break;
            }
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(UseDef {
            path,
            items,
            span: start_span.merge(end_span),
        })
    }
}
