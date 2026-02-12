//! Import/use statement parsing.

use crate::{committed, ParseOutcome, Parser};
use ori_ir::{ImportPath, TokenKind, UseDef, UseItem, Visibility};

impl Parser<'_> {
    /// Parse a use/import statement.
    ///
    /// Syntax variants:
    /// - Item import: `use './path' { item1, item2 as alias }`
    /// - Module import: `use std.math { sqrt }`
    /// - Module alias: `use std.net.http as http`
    ///
    /// Returns `EmptyErr` if no `use` keyword is present.
    ///
    /// The `visibility` parameter tracks whether this is a public re-export (`pub use`).
    pub(crate) fn parse_use(&mut self, visibility: Visibility) -> ParseOutcome<UseDef> {
        if !self.cursor.check(&TokenKind::Use) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Use,
                self.cursor.current_span().start as usize,
            );
        }

        self.parse_use_body(visibility)
    }

    fn parse_use_body(&mut self, visibility: Visibility) -> ParseOutcome<UseDef> {
        let start_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Use));

        // Parse import path
        let path = if let TokenKind::String(s) = *self.cursor.current_kind() {
            // Relative path: './math', '../utils'
            self.cursor.advance();
            ImportPath::Relative(s)
        } else {
            // Module path: std.math, std.collections
            let mut segments = Vec::new();
            loop {
                let name = committed!(self.cursor.expect_ident());
                segments.push(name);

                if self.cursor.check(&TokenKind::Dot) {
                    self.cursor.advance();
                } else {
                    break;
                }
            }
            ImportPath::Module(segments)
        };

        // Check for module alias: `use path as alias`
        if self.cursor.check(&TokenKind::As) {
            self.cursor.advance();
            let alias = committed!(self.cursor.expect_ident());
            let end_span = self.cursor.previous_span();
            return ParseOutcome::consumed_ok(UseDef {
                path,
                items: vec![],
                module_alias: Some(alias),
                visibility,
                span: start_span.merge(end_span),
            });
        }

        // Parse imported items: { item1, item2 as alias }
        committed!(self.cursor.expect(&TokenKind::LBrace));

        let items: Vec<UseItem> = committed!(self.brace_series(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(None);
            }

            // Check for private import prefix ::
            let is_private = if p.cursor.check(&TokenKind::DoubleColon) {
                p.cursor.advance();
                true
            } else {
                false
            };

            // Item name
            let name = p.cursor.expect_ident()?;

            // Optional alias: `as alias`
            let alias = if p.cursor.check(&TokenKind::As) {
                p.cursor.advance();
                Some(p.cursor.expect_ident()?)
            } else {
                None
            };

            Ok(Some(UseItem {
                name,
                alias,
                is_private,
            }))
        }));

        let end_span = self.cursor.previous_span();

        ParseOutcome::consumed_ok(UseDef {
            path,
            items,
            module_alias: None,
            visibility,
            span: start_span.merge(end_span),
        })
    }
}
