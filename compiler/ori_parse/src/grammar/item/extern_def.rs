//! Extern block parsing.
//!
//! Grammar:
//! ```ebnf
//! extern_block  = [ "pub" ] "extern" string_literal [ "from" string_literal ] [ free_attr ] "{" { extern_item } "}" .
//! free_attr     = "#" "free" "(" identifier ")" .
//! extern_item   = "@" identifier extern_params "->" type [ "as" string_literal ] .
//! extern_params = "(" [ extern_param { "," extern_param } ] [ c_variadic ] ")" .
//! extern_param  = identifier ":" type .
//! c_variadic    = "," "..." .
//! ```
//!
//! Spec: Annex E §FFI — `#free(fn)` declares the user-supplied
//! deallocation function for opaque extern types per proposal:643-645.

use crate::{committed, ParseError, ParseOutcome, ParseWarning, Parser};
use ori_ir::{ExternBlock, ExternItem, ExternParam, Name, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse an extern block.
    ///
    /// Grammar: `extern_block = [ "pub" ] "extern" string_literal [ "from" string_literal ] "{" { extern_item } "}" .`
    ///
    /// The `pub` keyword and visibility have already been consumed by the caller
    /// (`dispatch_declaration`).
    pub(crate) fn parse_extern_block(
        &mut self,
        visibility: Visibility,
    ) -> ParseOutcome<ExternBlock> {
        if !self.cursor.check(&TokenKind::Extern) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Extern,
                self.cursor.current_span().start as usize,
            );
        }

        self.in_error_context(crate::ErrorContext::ExternBlock, |p| {
            p.parse_extern_block_body(visibility)
        })
    }

    fn parse_extern_block_body(&mut self, visibility: Visibility) -> ParseOutcome<ExternBlock> {
        let start_span = self.cursor.current_span();

        // extern
        committed!(self.cursor.expect(&TokenKind::Extern));

        // Convention string: "c" or "js"
        let convention = if let TokenKind::String(name) = *self.cursor.current_kind() {
            let conv_span = self.cursor.current_span();
            self.cursor.advance();

            // Validate known conventions
            let conv_str = self.cursor.interner().lookup(name);
            if conv_str != "c" && conv_str != "js" {
                self.deferred_warnings
                    .push(ParseWarning::UnknownCallingConvention {
                        span: conv_span,
                        convention: conv_str.to_owned(),
                    });
            }

            name
        } else {
            let span = self.cursor.current_span();
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "expected calling convention string (\"c\" or \"js\") after `extern`",
                    span,
                ),
                span,
            );
        };

        // Optional: from "library"
        // `from` is a contextual keyword — check if current ident is "from"
        let library = if self.check_contextual_keyword("from") {
            self.cursor.advance(); // consume `from`
            if let TokenKind::String(name) = *self.cursor.current_kind() {
                self.cursor.advance();
                Some(name)
            } else {
                let span = self.cursor.current_span();
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "expected library path string after `from`",
                        span,
                    ),
                    span,
                );
            }
        } else {
            None
        };

        // Optional: #free(symbol)
        // Names the user-supplied deallocation function for opaque types
        // declared by this block. See Spec: Annex E §FFI (proposal:643-645).
        let free_fn = match self.parse_extern_free_attr() {
            Ok(value) => value,
            Err(error) => {
                let span = error.span;
                return ParseOutcome::consumed_err(error, span);
            }
        };

        // { ... }
        committed!(self.cursor.expect(&TokenKind::LBrace));

        let mut items = Vec::new();
        loop {
            self.cursor.skip_newlines();

            if self.cursor.check(&TokenKind::RBrace) || self.cursor.is_at_end() {
                break;
            }

            match self.parse_extern_item() {
                Ok(item) => items.push(item),
                Err(error) => {
                    let span = error.span;
                    return ParseOutcome::consumed_err(error, span);
                }
            }
        }

        committed!(self.cursor.expect(&TokenKind::RBrace));
        let end_span = self.cursor.previous_span();

        ParseOutcome::consumed_ok(ExternBlock {
            convention,
            library,
            free_fn,
            items,
            visibility,
            span: start_span.merge(end_span),
        })
    }

    /// Parse an optional `#free(symbol)` attribute on an extern block.
    ///
    /// Grammar: `free_attr = "#" "free" "(" identifier ")" .`
    ///
    /// Returns `Ok(Some(name))` on a well-formed `#free(symbol)`,
    /// `Ok(None)` when the attribute is absent, or `Err` for malformed
    /// shapes (`#free` without parens, empty parens, non-identifier arg).
    /// Spec: Annex E §FFI (proposal:643-645).
    fn parse_extern_free_attr(&mut self) -> Result<Option<Name>, ParseError> {
        if !self.cursor.check(&TokenKind::Hash) {
            return Ok(None);
        }
        let hash_span = self.cursor.current_span();
        self.cursor.advance(); // consume `#`

        // After `#`, MUST see an identifier that is "free".
        // If not, this is malformed for the extern-block attribute slot.
        let TokenKind::Ident(name) = *self.cursor.current_kind() else {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected `free` after `#` in extern block attribute",
                self.cursor.current_span(),
            ));
        };
        let attr_name = self.cursor.interner().lookup(name);
        if attr_name != "free" {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "unknown extern block attribute (only `#free` is supported)",
                self.cursor.current_span(),
            ));
        }
        self.cursor.advance(); // consume `free`

        // Require `(`
        if !self.cursor.check(&TokenKind::LParen) {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected `(` after `#free`",
                self.cursor.current_span(),
            ));
        }
        self.cursor.advance(); // consume `(`

        // Reject empty `#free()`.
        if self.cursor.check(&TokenKind::RParen) {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1004,
                "`#free` requires a function-symbol identifier",
                self.cursor.current_span(),
            ));
        }

        // Require an identifier symbol — reject literals (`#free(123)`),
        // strings, anything non-ident.
        let TokenKind::Ident(symbol_name) = *self.cursor.current_kind() else {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1004,
                "expected function-symbol identifier in `#free(...)`",
                self.cursor.current_span(),
            ));
        };
        self.cursor.advance(); // consume identifier

        // Require `)`
        if !self.cursor.check(&TokenKind::RParen) {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected `)` after `#free(symbol)`",
                self.cursor.current_span(),
            ));
        }
        self.cursor.advance(); // consume `)`

        let _ = hash_span; // span tracked via outer extern-block span
        Ok(Some(symbol_name))
    }

    /// Parse a single extern item (function declaration).
    ///
    /// Grammar: `extern_item = "@" identifier extern_params "->" type [ "as" string_literal ] .`
    fn parse_extern_item(&mut self) -> Result<ExternItem, ParseError> {
        let start_span = self.cursor.current_span();

        // @
        self.cursor.expect(&TokenKind::At)?;

        // name
        let name = self.cursor.expect_ident()?;

        // (params)
        self.cursor.expect(&TokenKind::LParen)?;
        let (params, is_c_variadic) = self.parse_extern_params()?;
        self.cursor.expect(&TokenKind::RParen)?;

        // -> Type
        self.cursor.expect(&TokenKind::Arrow)?;
        let return_ty = self.parse_type_required().into_result()?;

        // Optional: as "foreign_name"
        let alias = if self.cursor.check(&TokenKind::As) {
            self.cursor.advance();
            match *self.cursor.current_kind() {
                TokenKind::String(name) => {
                    self.cursor.advance();
                    Some(name)
                }
                _ => {
                    return Err(ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "expected string literal after `as` in extern item",
                        self.cursor.current_span(),
                    ));
                }
            }
        } else {
            None
        };

        let end_span = self.cursor.previous_span();

        Ok(ExternItem {
            name,
            params,
            return_ty,
            alias,
            is_c_variadic,
            span: start_span.merge(end_span),
        })
    }

    /// Parse extern parameter list.
    ///
    /// Grammar: `extern_params = "(" [ extern_param { "," extern_param } ] [ c_variadic ] ")" .`
    ///
    /// Returns the params and whether a C variadic marker was found.
    fn parse_extern_params(&mut self) -> Result<(Vec<ExternParam>, bool), ParseError> {
        let mut params = Vec::new();
        let mut is_c_variadic = false;

        if self.cursor.check(&TokenKind::RParen) {
            return Ok((params, false));
        }

        loop {
            // Check for C variadic: `...` (after at least one param or at start)
            if self.cursor.check(&TokenKind::DotDotDot) {
                self.cursor.advance();
                is_c_variadic = true;
                break;
            }

            let param_span = self.cursor.current_span();

            // name
            let name = self.cursor.expect_ident()?;

            // : Type
            self.cursor.expect(&TokenKind::Colon)?;
            let ty = self.parse_type_required().into_result()?;

            let end_span = self.cursor.previous_span();
            params.push(ExternParam {
                name,
                ty,
                span: param_span.merge(end_span),
            });

            // , or end
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else {
                break;
            }
        }

        Ok((params, is_c_variadic))
    }

    /// Check if the current token is an identifier matching a contextual keyword.
    fn check_contextual_keyword(&self, keyword: &str) -> bool {
        if let TokenKind::Ident(name) = *self.cursor.current_kind() {
            self.cursor.interner().lookup(name) == keyword
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests;
