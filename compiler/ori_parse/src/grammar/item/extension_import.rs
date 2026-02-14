//! Extension import parsing.
//!
//! Grammar: `extension_import = "extension" import_path "{" extension_item { "," extension_item } "}" .`
//! Grammar: `extension_item = identifier "." identifier .`

use crate::{committed, ParseOutcome, Parser};
use ori_ir::{ExtensionImport, ExtensionImportItem, ImportPath, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse an extension import statement.
    ///
    /// Syntax: `extension std.iter.extensions { Iterator.count, Iterator.last }`
    ///
    /// Returns `EmptyErr` if no `extension` keyword is present.
    pub(crate) fn parse_extension_import(
        &mut self,
        visibility: Visibility,
    ) -> ParseOutcome<ExtensionImport> {
        if !self.cursor.check(&TokenKind::Extension) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Extension,
                self.cursor.current_span().start as usize,
            );
        }

        let start_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Extension));

        // Parse import path (same as regular imports)
        let path = if let TokenKind::String(s) = *self.cursor.current_kind() {
            self.cursor.advance();
            ImportPath::Relative(s)
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
            ImportPath::Module(segments)
        };

        // Parse extension items: { Type.method, Type.method }
        committed!(self.cursor.expect(&TokenKind::LBrace));

        let items: Vec<ExtensionImportItem> = committed!(self.brace_series(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(None);
            }

            let item_start = p.cursor.current_span();

            // Type name
            let type_name = p.cursor.expect_ident()?;

            // Dot separator
            p.cursor.expect(&TokenKind::Dot)?;

            // Method name
            let method_name = p.cursor.expect_ident()?;

            let item_end = p.cursor.previous_span();

            Ok(Some(ExtensionImportItem {
                type_name,
                method_name,
                span: item_start.merge(item_end),
            }))
        }));

        let end_span = self.cursor.previous_span();

        ParseOutcome::consumed_ok(ExtensionImport {
            path,
            items,
            visibility,
            span: start_span.merge(end_span),
        })
    }
}
