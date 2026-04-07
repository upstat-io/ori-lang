//! Module-level parsing: `parse_module`, `parse_imports`, and incremental parsing.
//!
//! Contains the top-level module parsing loop, import section parsing,
//! and incremental parsing with AST reuse.

use crate::grammar::ParsedAttrs;
use crate::{ParseError, ParseOutput, Parser};

use ori_ir::{Module, ModuleExtra, SharedArena, Span, TokenKind, Visibility};
use tracing::debug;

impl Parser<'_> {
    /// Parse a module (collection of function definitions and tests).
    ///
    /// Uses progress-aware parsing for improved error recovery:
    /// - If parsing fails without progress (no tokens consumed), we skip unknown tokens
    /// - If parsing fails with progress (tokens consumed), we synchronize to a recovery point
    pub fn parse_module(mut self) -> ParseOutput {
        debug!(
            token_count = self.cursor.token_count(),
            "parse_module start"
        );
        let mut module = Module::with_capacity_hint(self.estimated_source_len());
        let mut errors = Vec::new();

        // File-level attribute must appear before imports and declarations.
        // Grammar: source_file = [ file_attribute ] { import } { declaration } .
        module.file_attr = self.parse_file_attribute(&mut errors);

        // parse_imports returns leftover attrs if it consumed attrs before a non-import token.
        let mut leftover_attrs = self.parse_imports(&mut module, &mut errors);

        // Parse declarations (functions, tests, traits, impls, types, etc.)
        while !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.is_at_end() {
                break;
            }

            // Use leftover attrs from import parsing on the first declaration,
            // otherwise parse fresh attributes.
            let attrs = leftover_attrs
                .take()
                .unwrap_or_else(|| self.parse_attributes(&mut errors));
            let visibility = if self.cursor.check(&TokenKind::Pub) {
                self.cursor.advance();
                Visibility::Public
            } else {
                Visibility::Private
            };

            self.dispatch_declaration(attrs, visibility, &mut module, &mut errors);
        }

        // Diagnose orphaned attrs at EOF. If parse_imports() returned
        // leftover attrs but the declaration loop never consumed them (file ended
        // before any declaration), emit E1006 so users get the expected placement
        // diagnostic instead of silent acceptance.
        if let Some(orphan_attrs) = leftover_attrs {
            if !orphan_attrs.is_empty() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
                    self.cursor.previous_span(),
                ));
            }
        }

        // Drain deferred errors/warnings from sub-parsers.
        errors.append(&mut self.deferred_errors);
        let warnings = self.deferred_warnings;

        debug!(
            functions = module.functions.len(),
            tests = module.tests.len(),
            types = module.types.len(),
            traits = module.traits.len(),
            impls = module.impls.len(),
            imports = module.imports.len(),
            expressions = self.arena.expr_count(),
            errors = errors.len(),
            warnings = warnings.len(),
            "parse_module complete"
        );

        ParseOutput {
            module,
            arena: SharedArena::new(self.arena),
            errors,
            warnings,
            // Note: For metadata support, use parse_with_metadata() which
            // overwrites this with lexer-captured metadata
            metadata: ModuleExtra::new(),
        }
    }

    /// Parse the import block at the top of a module.
    ///
    /// Imports must appear at the beginning of the file per spec.
    /// Parses `use`, `pub use`, `extension`, and `pub extension` statements.
    /// Parse the import section at the top of a module.
    ///
    /// Returns `Some(attrs)` if attributes were consumed but the next token
    /// is not an import — the caller should use these as the attrs for the
    /// first declaration. Returns `None` on normal exit.
    ///
    /// Spec §25.4: imports support item-level `#target`/`#cfg` attributes.
    pub(super) fn parse_imports(
        &mut self,
        module: &mut Module,
        errors: &mut Vec<ParseError>,
    ) -> Option<ParsedAttrs> {
        loop {
            self.cursor.skip_newlines();
            if self.cursor.is_at_end() {
                return None;
            }

            // Spec §25.4: conditional compilation on imports.
            // Parse any attributes before the import statement.
            let has_attr_prefix =
                self.cursor.check(&TokenKind::Hash) || self.cursor.check(&TokenKind::HashBracket);
            let attrs = if has_attr_prefix {
                self.parse_attributes(errors)
            } else {
                ParsedAttrs::default()
            };

            let is_pub_use = self.cursor.check(&TokenKind::Pub)
                && matches!(self.cursor.peek_next_kind(), TokenKind::Use);

            let is_pub_extension = self.cursor.check(&TokenKind::Pub)
                && matches!(self.cursor.peek_next_kind(), TokenKind::Extension);

            if self.cursor.check(&TokenKind::Use) || is_pub_use {
                // Spec §25.4: imports support #target/#cfg only.
                if attrs.has_non_conditional_attrs() {
                    errors.push(ParseError::new(
                        ori_diagnostic::ErrorCode::E1006,
                        format!(
                            "{} not supported on imports; only #target and #cfg are allowed",
                            attrs.non_conditional_attr_names()
                        ),
                        self.cursor.current_span(),
                    ));
                }
                let visibility = if is_pub_use {
                    self.cursor.advance();
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                let outcome = self.parse_use(attrs, visibility);
                self.handle_outcome(
                    outcome,
                    &mut module.imports,
                    errors,
                    Self::recover_to_next_statement,
                );
            } else if self.cursor.check(&TokenKind::Extension) || is_pub_extension {
                // Spec §25.4: extension imports do NOT support any attributes
                // (only functions, types, trait implementations, constants, and
                // regular imports support item-level conditional attrs).
                if has_attr_prefix {
                    errors.push(ParseError::new(
                        ori_diagnostic::ErrorCode::E1006,
                        "attributes not supported on extension imports".to_string(),
                        self.cursor.current_span(),
                    ));
                }
                let visibility = if is_pub_extension {
                    self.cursor.advance();
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                let outcome = self.parse_extension_import(visibility);
                self.handle_outcome(
                    outcome,
                    &mut module.extension_imports,
                    errors,
                    Self::recover_to_next_statement,
                );
            } else if has_attr_prefix {
                // Attributes were parsed but next token is not an import —
                // these attrs belong to the first declaration.
                return Some(attrs);
            } else {
                return None;
            }
        }
    }

    /// Parse a module with incremental reuse from a previous parse.
    ///
    /// This method attempts to reuse unchanged declarations from the old AST,
    /// only re-parsing declarations that overlap with the text change.
    pub(super) fn parse_module_incremental(
        mut self,
        mut state: crate::incremental::IncrementalState<'_>,
        old_arena: &ori_ir::ExprArena,
    ) -> ParseOutput {
        use crate::incremental::AstCopier;

        let mut module = Module::with_capacity_hint(self.estimated_source_len());
        let mut errors = Vec::new();

        // File-level attribute must appear before imports and declarations.
        // Grammar: source_file = [ file_attribute ] { import } { declaration } .
        // (was missing from incremental path)
        module.file_attr = self.parse_file_attribute(&mut errors);

        // Imports always get re-parsed since they affect resolution.
        // Capture leftover attrs for the first declaration.
        let mut leftover_attrs = self.parse_imports(&mut module, &mut errors);

        // Parse remaining declarations with potential reuse
        while !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.is_at_end() {
                break;
            }

            let pos = self.cursor.current_span().start;

            // Try to find a reusable declaration at this position
            if let Some(decl_ref) = state.cursor.find_at(pos) {
                // Check if this declaration is outside the change region
                if !state.cursor.marker().intersects(decl_ref.span) {
                    let copier = AstCopier::new(old_arena, state.cursor.marker().clone());
                    copier.copy_declaration_to_module(
                        decl_ref,
                        state.cursor.module(),
                        &mut module,
                        &mut self.arena,
                    );

                    state.stats.reused_count += 1;
                    self.skip_to_span_end(decl_ref.span);
                    // Consume trailing `;` that was eaten by the original parse
                    // but not included in the declaration span.
                    self.eat_optional_semicolon();
                    // Consume leftover attrs when the first declaration
                    // slot is satisfied by reuse. The reused declaration already has
                    // its attrs baked into the AST node from the original full parse.
                    // Without this, leftover_attrs leaks to the next fresh-parsed
                    // declaration, synthesizing attrs onto unrelated declarations.
                    leftover_attrs.take();
                    continue;
                }
            }

            // Cannot reuse: parse fresh
            state.stats.reparsed_count += 1;

            // Use leftover attrs from import parsing on the first declaration,
            // otherwise parse fresh attributes (mirrors full parser behavior).
            let attrs = leftover_attrs
                .take()
                .unwrap_or_else(|| self.parse_attributes(&mut errors));
            let visibility = if self.cursor.check(&TokenKind::Pub) {
                self.cursor.advance();
                Visibility::Public
            } else {
                Visibility::Private
            };

            self.dispatch_declaration(attrs, visibility, &mut module, &mut errors);
        }

        // Diagnose orphaned attrs at EOF on incremental path too.
        if let Some(orphan_attrs) = leftover_attrs {
            if !orphan_attrs.is_empty() {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
                    self.cursor.previous_span(),
                ));
            }
        }

        // Drain deferred errors/warnings from sub-parsers.
        errors.append(&mut self.deferred_errors);
        let warnings = self.deferred_warnings;

        ParseOutput {
            module,
            arena: SharedArena::new(self.arena),
            errors,
            warnings,
            // Note: Incremental metadata merging not yet implemented.
            // For now, caller should re-lex with lex_with_comments() and
            // pass to parse_with_metadata() for full metadata support.
            metadata: ModuleExtra::new(),
        }
    }

    /// Skip tokens until we're past the given span end.
    ///
    /// Used during incremental parsing to skip over reused declarations.
    fn skip_to_span_end(&mut self, span: Span) {
        // Adjust the span end for the change delta to get the new end position
        let adjusted_end = self.cursor.current_span().start.max(span.end);

        while !self.cursor.is_at_end() && self.cursor.current_span().start < adjusted_end {
            self.cursor.advance();
        }
    }
}
