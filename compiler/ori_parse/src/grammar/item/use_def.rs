//! Import/use statement parsing.

use crate::{ParseError, Parser};
use ori_ir::{ImportPath, TokenKind, UseDef, UseItem, Visibility};

impl Parser<'_> {
    /// Parse a use/import statement.
    ///
    /// Syntax variants:
    /// - Item import: `use './path' { item1, item2 as alias }`
    /// - Module import: `use std.math { sqrt }`
    /// - Module alias: `use std.net.http as http`
    ///
    /// The `visibility` parameter tracks whether this is a public re-export (`pub use`).
    pub(crate) fn parse_use_inner(&mut self, visibility: Visibility) -> Result<UseDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Use)?;

        // Parse import path
        let path = if let TokenKind::String(s) = *self.current_kind() {
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
                visibility,
                span: start_span.merge(end_span),
            });
        }

        // Parse imported items: { item1, item2 as alias }
        self.expect(&TokenKind::LBrace)?;

        let items: Vec<UseItem> = self.brace_series(|p| {
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
        })?;

        let end_span = self.previous_span();

        Ok(UseDef {
            path,
            items,
            module_alias: None,
            visibility,
            span: start_span.merge(end_span),
        })
    }
}
