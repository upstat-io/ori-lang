//! Top-level declaration dispatch and error handling for the Parser.
//!
//! Contains `dispatch_declaration` (routes each declaration kind),
//! `handle_declaration_error` (misplaced imports, foreign keywords, orphans),
//! semicolon handling, and recovery helpers.

use crate::error;
use crate::grammar::ParsedAttrs;
use crate::outcome::ParseOutcome;
use crate::{FunctionOrTest, ParseError, Parser};

use ori_ir::{Module, TokenKind, Visibility};
use tracing::trace;

impl Parser<'_> {
    /// Dispatch a single top-level declaration.
    ///
    /// Handles all declaration kinds: functions, tests, traits, impls,
    /// extends, type declarations, constants, and error cases (misplaced
    /// imports, orphaned attributes, unknown tokens).
    ///
    /// Returns `true` if a token was consumed (used by the caller to
    /// detect infinite loops).
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive top-level declaration kind dispatch"
    )]
    pub(super) fn dispatch_declaration(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
        module: &mut Module,
        errors: &mut Vec<ParseError>,
    ) {
        trace!(
            pos = self.cursor.position(),
            kind = self.cursor.current_kind().display_name(),
            "dispatch_declaration"
        );
        if self.cursor.check(&TokenKind::At) {
            let outcome = self.parse_function_or_test(attrs, visibility);
            match outcome {
                ParseOutcome::ConsumedOk { value } | ParseOutcome::EmptyOk { value } => match value
                {
                    FunctionOrTest::Function(func) => module.functions.push(func),
                    FunctionOrTest::Test(test) => module.tests.push(test),
                },
                ParseOutcome::ConsumedErr { error, .. } => {
                    self.recover_to_function();
                    errors.push(error);
                }
                ParseOutcome::EmptyErr { expected, position } => {
                    errors.push(ParseError::from_expected_tokens(&expected, position));
                }
            }
        } else if self.cursor.check(&TokenKind::Trait) {
            // Spec §25.4: traits do not support item-level attributes.
            if !attrs.is_empty() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes are not supported on trait declarations",
                    self.cursor.current_span(),
                ));
            }
            let outcome = self.parse_trait(visibility);
            self.handle_outcome(
                outcome,
                &mut module.traits,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Def)
            && matches!(self.cursor.peek_next_kind(), TokenKind::Impl)
        {
            // Spec §25.4: def impls do not support item-level attributes.
            if !attrs.is_empty() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes are not supported on default implementation declarations",
                    self.cursor.current_span(),
                ));
            }
            let outcome = self.parse_def_impl(visibility);
            self.handle_outcome(
                outcome,
                &mut module.def_impls,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Impl) {
            // Spec §25.4: impls support #target/#cfg only.
            if attrs.has_non_conditional_attrs() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    format!(
                        "{} not supported on impl blocks; only #target and #cfg are allowed",
                        attrs.non_conditional_attr_names()
                    ),
                    self.cursor.current_span(),
                ));
            }
            let outcome = self.parse_impl(attrs);
            self.handle_outcome(
                outcome,
                &mut module.impls,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Extend) {
            // Spec §25.4: extends do not support item-level attributes.
            if !attrs.is_empty() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes are not supported on extension declarations",
                    self.cursor.current_span(),
                ));
            }
            let outcome = self.parse_extend();
            self.handle_outcome(
                outcome,
                &mut module.extends,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Type) {
            let outcome = self.parse_type_decl(attrs, visibility);
            self.handle_outcome(
                outcome,
                &mut module.types,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Let) {
            // `let $name = value` — constant declaration (spec §04-constants)
            // Spec §25.4: constants support #target/#cfg only.
            if attrs.has_non_conditional_attrs() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    format!(
                        "{} not supported on constants; only #target and #cfg are allowed",
                        attrs.non_conditional_attr_names()
                    ),
                    self.cursor.current_span(),
                ));
            }
            self.cursor.advance(); // consume `let`
            if self.cursor.check(&TokenKind::Dollar) {
                let outcome = self.parse_const(attrs, visibility);
                self.handle_outcome(
                    outcome,
                    &mut module.consts,
                    errors,
                    Self::recover_to_function,
                );
            } else {
                // `let name` without `$` — module-level bindings must be immutable
                errors.push(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "module-level bindings must be immutable".to_string(),
                        self.cursor.current_span(),
                    )
                    .with_help(
                        "Use `let $name = value` with the `$` prefix for module-level constants"
                            .to_string(),
                    ),
                );
                self.recover_to_function();
            }
        } else if self.cursor.check(&TokenKind::Dollar) {
            // Also accept `$name = value` without `let` for backwards compatibility
            // Spec §25.4: constants support #target/#cfg only.
            if attrs.has_non_conditional_attrs() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    format!(
                        "{} not supported on constants; only #target and #cfg are allowed",
                        attrs.non_conditional_attr_names()
                    ),
                    self.cursor.current_span(),
                ));
            }
            let outcome = self.parse_const(attrs, visibility);
            self.handle_outcome(
                outcome,
                &mut module.consts,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Extern) {
            // Spec §25.4: extern blocks do not support item-level attributes.
            if !attrs.is_empty() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes are not supported on extern declarations",
                    self.cursor.current_span(),
                ));
            }
            let outcome = self.parse_extern_block(visibility);
            self.handle_outcome(
                outcome,
                &mut module.extern_blocks,
                errors,
                Self::recover_to_function,
            );
        } else {
            self.handle_declaration_error(&attrs, errors);
        }
    }

    /// Handle error cases in declaration dispatch.
    ///
    /// Covers: misplaced imports, lexer error tokens, reserved keywords
    /// (`return`), foreign keywords (`fn`, `func`, etc.), orphaned attributes,
    /// and unknown tokens at module level.
    fn handle_declaration_error(&mut self, attrs: &ParsedAttrs, errors: &mut Vec<ParseError>) {
        if self.cursor.check(&TokenKind::Use) || self.cursor.check(&TokenKind::Extension) {
            // Import or extension import after declarations
            errors.push(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "import statements must appear at the beginning of the file",
                self.cursor.current_span(),
            ));
            // Skip the entire import statement to avoid infinite loop
            self.cursor.advance();
            while !self.cursor.is_at_end()
                && !self.cursor.check(&TokenKind::At)
                && !self.cursor.check(&TokenKind::Trait)
                && !self.cursor.check(&TokenKind::Impl)
                && !self.cursor.check(&TokenKind::Type)
                && !self.cursor.check(&TokenKind::Use)
                && !self.cursor.check(&TokenKind::Extension)
            {
                self.cursor.advance();
            }
        } else if self.cursor.current_tag() == TokenKind::TAG_ERROR {
            // Error tokens from the lexer — skip without emitting a parse error.
            // The real diagnostic was already emitted by the lex error pipeline.
            self.cursor.advance();
        } else if self.cursor.check(&TokenKind::Return) {
            // `return` is reserved so users get a targeted error, not "unexpected identifier"
            let kind = error::ParseErrorKind::UnsupportedKeyword {
                keyword: TokenKind::Return,
                reason: "Ori is expression-based: the last expression in a block is its value",
            };
            errors.push(ParseError::from_kind(&kind, self.cursor.current_span()));
            self.cursor.advance();
        } else if self.cursor.current_tag() == TokenKind::TAG_IDENT {
            // Check for foreign keywords from other languages at declaration position.
            // e.g., `fn main()` → "use `@name (params) -> type = body` in Ori"
            if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                let ident_str = self.cursor.interner().lookup(name);
                if let Some(suggestion) = crate::foreign_keywords::lookup_foreign_keyword(ident_str)
                {
                    errors.push(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            format!("`{ident_str}` is not an Ori keyword"),
                            self.cursor.current_span(),
                        )
                        .with_help(String::from(suggestion)),
                    );
                    self.cursor.advance();
                    return;
                }
            }
            // Not a foreign keyword — emit error for unexpected identifier
            if attrs.is_empty() {
                let kind = error::ParseErrorKind::ExpectedDeclaration {
                    found: self.cursor.current_kind().clone(),
                };
                errors.push(ParseError::from_kind(&kind, self.cursor.current_span()));
            } else {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
                    self.cursor.current_span(),
                ));
            }
            self.cursor.advance();
        } else if !attrs.is_empty() {
            // Attributes without a following declaration
            errors.push(ParseError::new(
                ori_diagnostic::ErrorCode::E1006,
                "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
                self.cursor.current_span(),
            ));
            self.cursor.advance();
        } else {
            // Unknown token at module level — not a valid declaration start
            let kind = error::ParseErrorKind::ExpectedDeclaration {
                found: self.cursor.current_kind().clone(),
            };
            errors.push(ParseError::from_kind(&kind, self.cursor.current_span()));
            self.cursor.advance();
        }
    }

    /// Consume a trailing semicolon if present.
    ///
    /// Used after items like `use`, `capset`, and trait method signatures
    /// where `;` terminates the declaration but is not enforced by the parser.
    pub(crate) fn eat_optional_semicolon(&mut self) {
        if self.cursor.check(&TokenKind::Semicolon) {
            self.cursor.advance();
        }
    }

    /// Consume a required trailing semicolon after an item with an expression body.
    ///
    /// Per grammar: function/test/method bodies that end with `}` (block body)
    /// don't need a trailing `;`. Non-block bodies (e.g., `@f () -> int = 42;`)
    /// require `;` to terminate the declaration.
    pub(crate) fn eat_optional_item_semicolon(&mut self) {
        if self.cursor.check(&TokenKind::Semicolon) {
            self.cursor.advance();
        } else if !self.cursor.previous_non_newline_is_rbrace() {
            self.deferred_errors.push(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1016,
                    "expected `;` after item declaration",
                    self.cursor.current_span(),
                )
                .with_help("Block bodies ending with `}` don't need `;`, but expression bodies do"),
            );
        }
    }

    /// Recovery: skip to next statement (@ or use or EOF)
    pub(super) fn recover_to_next_statement(&mut self) {
        crate::recovery::synchronize(&mut self.cursor, crate::recovery::STMT_BOUNDARY);
    }

    pub(super) fn recover_to_function(&mut self) {
        crate::recovery::synchronize(&mut self.cursor, crate::recovery::FUNCTION_BOUNDARY);
    }
}
