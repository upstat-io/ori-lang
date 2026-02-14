//! Import/use statement parsing.

use crate::{committed, ParseOutcome, Parser};
use ori_ir::{ImportPath, TokenKind, UseDef, UseItem, Visibility};

impl Parser<'_> {
    /// Parse an import path: either a relative string or dot-separated module path.
    ///
    /// Used by both `use` and `extension` import statements.
    ///
    /// Grammar: `import_path = STRING | identifier { "." identifier } .`
    pub(crate) fn parse_import_path(&mut self) -> ParseOutcome<ImportPath> {
        if let TokenKind::String(s) = *self.cursor.current_kind() {
            self.cursor.advance();
            ParseOutcome::consumed_ok(ImportPath::Relative(s))
        } else {
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
            ParseOutcome::consumed_ok(ImportPath::Module(segments))
        }
    }

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

        let path = committed!(self.parse_import_path().into_result());

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

            // Constant import: `$NAME`
            if p.cursor.check(&TokenKind::Dollar) {
                p.cursor.advance();
                let name = p.cursor.expect_ident()?;
                return Ok(Some(UseItem {
                    name,
                    alias: None,
                    is_private: false,
                    without_def: false,
                    is_constant: true,
                }));
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

            // Optional `without def` modifier for traits
            let without_def = if let TokenKind::Ident(n) = *p.cursor.current_kind() {
                if p.cursor.interner().lookup(n) == "without" {
                    p.cursor.advance();
                    p.cursor.expect(&TokenKind::Def)?;
                    true
                } else {
                    false
                }
            } else {
                false
            };

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
                without_def,
                is_constant: false,
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
