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
        if !self.check(&TokenKind::Use) {
            return ParseOutcome::empty_err_expected(&TokenKind::Use, self.position());
        }

        self.parse_use_body(visibility)
    }

    fn parse_use_body(&mut self, visibility: Visibility) -> ParseOutcome<UseDef> {
        let start_span = self.current_span();
        committed!(self.expect(&TokenKind::Use));

        // Parse import path
        let path = if let TokenKind::String(s) = *self.current_kind() {
            // Relative path: './math', '../utils'
            self.advance();
            ImportPath::Relative(s)
        } else {
            // Module path: std.math, std.collections
            let mut segments = Vec::new();
            loop {
                let name = committed!(self.expect_ident());
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
            let alias = committed!(self.expect_ident());
            let end_span = self.previous_span();
            return ParseOutcome::consumed_ok(UseDef {
                path,
                items: vec![],
                module_alias: Some(alias),
                visibility,
                span: start_span.merge(end_span),
            });
        }

        // Parse imported items: { item1, item2 as alias }
        committed!(self.expect(&TokenKind::LBrace));

        let items: Vec<UseItem> = committed!(self.brace_series(|p| {
            if p.check(&TokenKind::RBrace) {
                return Ok(None);
            }

            // Check for private import prefix ::
            let is_private = if p.check(&TokenKind::DoubleColon) {
                p.advance();
                true
            } else {
                false
            };

            // Item name
            let name = p.expect_ident()?;

            // Optional alias: `as alias`
            let alias = if p.check(&TokenKind::As) {
                p.advance();
                Some(p.expect_ident()?)
            } else {
                None
            };

            Ok(Some(UseItem {
                name,
                alias,
                is_private,
            }))
        }));

        let end_span = self.previous_span();

        ParseOutcome::consumed_ok(UseDef {
            path,
            items,
            module_alias: None,
            visibility,
            span: start_span.merge(end_span),
        })
    }
}
