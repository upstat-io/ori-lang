//! Import/use statement parsing.

use crate::{ParseError, Parser};
use ori_ir::{ImportPath, TokenKind, UseDef, UseItem};

impl Parser<'_> {
    /// Parse a use/import statement.
    ///
    /// Syntax variants:
    /// - Item import: `use './path' { item1, item2 as alias }`
    /// - Module import: `use std.math { sqrt }`
    /// - Module alias: `use std.net.http as http`
    ///
    /// The `is_public` parameter tracks whether this is a public re-export (`pub use`).
    pub(crate) fn parse_use_inner(&mut self, is_public: bool) -> Result<UseDef, ParseError> {
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

        // Check for module alias: `use path as alias`
        if self.check(&TokenKind::As) {
            self.advance();
            let alias = self.expect_ident()?;
            let end_span = self.previous_span();
            return Ok(UseDef {
                path,
                items: vec![],
                module_alias: Some(alias),
                is_public,
                span: start_span.merge(end_span),
            });
        }

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

            items.push(UseItem {
                name,
                alias,
                is_private,
            });

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
            module_alias: None,
            is_public,
            span: start_span.merge(end_span),
        })
    }
}
