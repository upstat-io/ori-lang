//! `#skip(backend: "<name>", reason: "<why>")` attribute parsing.
//!
//! The backend-conditional skip: the named backend skips this test while the
//! other backend still runs it. Both `backend:` and a non-blank `reason:` are
//! mandatory. The unkeyed `#skip("reason")` form (skips both backends) is
//! handled by `parse_string_attr`, never here.

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::{BackendSkip, Name, TestBackend, TokenKind};

use super::ParsedAttrs;

impl Parser<'_> {
    /// Parse a keyed `#skip(backend: "<name>", reason: "<why>")` attribute.
    ///
    /// The dispatcher routes here only when `#skip(` is followed by an
    /// identifier (a keyed param); a string after `(` is the unkeyed reason and
    /// stays on `parse_string_attr`. Cursor is positioned at `(`.
    pub(super) fn parse_skip_backend_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        let attr_span = self.cursor.current_span();
        self.cursor.advance(); // consume (

        let mut backend: Option<TestBackend> = None;
        let mut reason: Option<Name> = None;

        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            let param_name = if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                self.cursor.advance();
                name
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected parameter name in skip",
                    self.cursor.current_span(),
                ));
                self.skip_to_attr_end(uses_brackets);
                return;
            };

            if !self.cursor.check(&TokenKind::Colon) {
                let name_str = self.cursor.interner().lookup(param_name);
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!("expected ':' after '{name_str}'"),
                    self.cursor.current_span(),
                ));
                self.skip_to_attr_end(uses_brackets);
                return;
            }
            self.cursor.advance(); // consume :

            self.parse_skip_backend_param(param_name, &mut backend, &mut reason, errors);

            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RParen) {
                break;
            }
        }

        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ')' after skip parameters",
                self.cursor.current_span(),
            ));
        }
        self.finish_attr_bracket(uses_brackets, errors);

        self.commit_skip_backend(attr_span, backend, reason, attrs, errors);
    }

    /// Parse a single `backend:` / `reason:` value in the keyed skip form.
    fn parse_skip_backend_param(
        &mut self,
        param_name: Name,
        backend: &mut Option<TestBackend>,
        reason: &mut Option<Name>,
        errors: &mut Vec<ParseError>,
    ) {
        let param_str = self.cursor.interner().lookup(param_name);
        match param_str {
            "backend" => {
                if let TokenKind::String(s) = *self.cursor.current_kind() {
                    match self.cursor.interner().lookup(s) {
                        "interpreter" | "interp" => *backend = Some(TestBackend::Interpreter),
                        "llvm" => *backend = Some(TestBackend::Llvm),
                        other => {
                            errors.push(ParseError::new(
                                ErrorCode::E1006,
                                format!(
                                    "unknown skip backend '{other}' \
                                     (expected \"interpreter\" or \"llvm\")"
                                ),
                                self.cursor.current_span(),
                            ));
                        }
                    }
                    self.cursor.advance();
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected string for 'backend'",
                        self.cursor.current_span(),
                    ));
                }
            }
            "reason" => {
                if let TokenKind::String(s) = *self.cursor.current_kind() {
                    *reason = Some(s);
                    self.cursor.advance();
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected string for 'reason'",
                        self.cursor.current_span(),
                    ));
                }
            }
            _ => {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!("unknown skip parameter '{param_str}'"),
                    self.cursor.previous_span(),
                ));
            }
        }
    }

    /// Validate the keyed skip (backend mandatory; reason mandatory + non-blank)
    /// and push the `BackendSkip` on success.
    fn commit_skip_backend(
        &self,
        attr_span: ori_ir::Span,
        backend: Option<TestBackend>,
        reason: Option<Name>,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
    ) {
        let Some(backend) = backend else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "skip 'backend' is required for a backend-conditional skip",
                attr_span,
            ));
            return;
        };
        let reason = match reason {
            Some(r) if !self.cursor.interner().lookup(r).is_empty() => r,
            Some(_) => {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "skip 'reason' must be non-empty",
                    attr_span,
                ));
                return;
            }
            None => {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "skip 'reason' is required for a backend-conditional skip",
                    attr_span,
                ));
                return;
            }
        };
        attrs.skip_backends.push(BackendSkip { backend, reason });
    }
}
