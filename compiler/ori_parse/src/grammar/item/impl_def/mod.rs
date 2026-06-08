//! Impl block parsing.

use crate::context::ParseContext;
use crate::grammar::attr::ParsedAttrs;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    DefImplDef, GenericParamRange, ImplAssocType, ImplDef, ImplMethod, ParsedType, ParsedTypeRange,
    TokenKind, Visibility,
};

impl Parser<'_> {
    /// Parse an impl block.
    ///
    /// Syntax: impl [<T>] Type { methods } (inherent) or impl [<T>] Type: Trait { methods } (trait impl)
    ///
    /// Returns `EmptyErr` if no `impl` keyword is present.
    pub(crate) fn parse_impl(&mut self, attrs: ParsedAttrs) -> ParseOutcome<ImplDef> {
        if !self.cursor.check(&TokenKind::Impl) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Impl,
                self.cursor.current_span().start as usize,
            );
        }

        self.in_error_context(crate::ErrorContext::ImplBlock, |p| p.parse_impl_body(attrs))
    }

    fn parse_impl_body(&mut self, attrs: ParsedAttrs) -> ParseOutcome<ImplDef> {
        let start_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Impl));

        // Optional generics: <T, U: Bound>
        let generics = if self.cursor.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
        } else {
            GenericParamRange::EMPTY
        };

        // Subject may be a primitive type-name keyword (`impl str: Trait`); gate on the
        // 6-primitive helper, not the broader check_type_keyword, so Never/void reject.
        // The post-colon trait path stays Ident-only, so `impl Foo: str` also rejects.
        let (first_path, first_ty) = if let Some((type_id, name_str)) =
            crate::grammar::item::generics::primitive_type_keyword(self.cursor.current_kind())
        {
            let name = self.cursor.interner().intern(name_str);
            self.cursor.advance();
            (vec![name], ParsedType::Primitive(type_id))
        } else {
            require!(self, self.parse_impl_type(), "type after `impl`")
        };

        // Determine trait vs inherent impl.
        let (trait_path, trait_type_args, self_path, self_ty) =
            if self.cursor.check(&TokenKind::Colon) {
                // Spec: grammar.ebnf trait_impl — subject-first `impl Type: Trait`.
                // First type is the subject (self_ty); the post-colon type is the trait.
                self.cursor.advance();
                let (trait_path, trait_ty) =
                    require!(self, self.parse_impl_type(), "trait path after `:`");
                // trait_type_args come from the post-colon trait, not the subject.
                let trait_type_args = match &trait_ty {
                    ori_ir::ParsedType::Named { type_args, .. } => *type_args,
                    _ => ParsedTypeRange::EMPTY,
                };
                (Some(trait_path), trait_type_args, first_path, first_ty)
            } else if self.cursor.check(&TokenKind::For) {
                // Spec: grammar.ebnf trait_impl has no `for`-form — reject with migration help.
                let for_span = self.cursor.current_span();
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1019,
                        "trait impl must be written `impl Type: Trait`, not `impl Trait for Type`",
                        for_span,
                    )
                    .with_help("rewrite `impl Trait for Type` as `impl Type: Trait`"),
                    for_span,
                );
            } else {
                (None, ParsedTypeRange::EMPTY, first_path, first_ty)
            };

        // Optional where clause
        let where_clauses = if self.cursor.check(&TokenKind::Where) {
            committed!(self.parse_where_clauses().into_result())
        } else {
            Vec::new()
        };

        // Impl body: { methods and associated types }
        committed!(self.cursor.expect(&TokenKind::LBrace));
        self.cursor.skip_newlines();

        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();

        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            if self.cursor.check(&TokenKind::Type) {
                // Associated type definition: type Item = T
                let at = committed!(self.parse_impl_assoc_type());
                assoc_types.push(at);
            } else if self.cursor.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                let method = committed!(self.parse_impl_method(false));
                methods.push(method);
            } else {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1001,
                        format!(
                            "expected method definition (@name) or associated type definition (type Name = ...), found {}",
                            self.cursor.current_kind().display_name()
                        ),
                        self.cursor.current_span(),
                    ),
                    self.cursor.current_span(),
                );
            }
            self.cursor.skip_newlines();
        }

        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(ImplDef {
            generics,
            trait_path,
            trait_type_args,
            self_path,
            self_ty,
            where_clauses,
            methods,
            assoc_types,
            span: start_span.merge(end_span),
            target_attr: attrs.target,
            cfg_attr: attrs.cfg,
        })
    }

    /// Parse a method in an impl block.
    ///
    /// Grammar (per `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf`):
    /// ```text
    /// method          = "@" identifier [ generics ] params "->" type [ uses_clause ] [ where_clause ] "=" expression [ ";" ] .
    /// def_impl_method = "@" identifier [ generics ] params "->" type [ where_clause ] "=" expression [ ";" ] .  /* no self, no uses */
    /// ```
    ///
    /// `def_impl_context` selects the `def impl` variant: `uses_clause` is
    /// rejected with a parse-error diagnostic. Stateless-default contract.
    pub(crate) fn parse_impl_method(
        &mut self,
        def_impl_context: bool,
    ) -> Result<ImplMethod, ParseError> {
        let start_span = self.cursor.current_span();

        // @name
        self.cursor.expect(&TokenKind::At)?;
        let name = self.cursor.expect_ident()?;

        // Optional method-level generics: <T, U: Bound>
        let generics = if self.cursor.check(&TokenKind::Lt) {
            self.parse_generics().into_result()?
        } else {
            GenericParamRange::EMPTY
        };

        // (params)
        self.cursor.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.cursor.expect(&TokenKind::RParen)?;

        // -> Type
        self.cursor.expect(&TokenKind::Arrow)?;
        let return_ty = self.parse_type_required().into_result()?;

        // Optional uses clause: uses Http, FileSystem
        // Forbidden in `def impl` methods (stateless-default contract).
        let capabilities = if self.cursor.check(&TokenKind::Uses) {
            if def_impl_context {
                let uses_span = self.cursor.current_span();
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "`uses` clause not allowed in `def impl` method (def-impl methods are stateless by definition)".to_string(),
                    uses_span,
                ));
            }
            self.parse_uses_clause().into_result()?
        } else {
            Vec::new()
        };

        // Optional method-level where clauses: where T: Clone
        let where_clauses = if self.cursor.check(&TokenKind::Where) {
            self.parse_where_clauses().into_result()?
        } else {
            Vec::new()
        };

        // = body
        self.cursor.expect(&TokenKind::Eq)?;
        self.cursor.skip_newlines();
        let body = self
            .with_context(ParseContext::IN_FUNCTION, Self::parse_expr)
            .into_result()?;

        let end_span = self.arena.get_expr(body).span;

        self.eat_optional_item_semicolon();

        Ok(ImplMethod {
            name,
            generics,
            params,
            return_ty,
            capabilities,
            where_clauses,
            body,
            span: start_span.merge(end_span),
        })
    }

    /// Parse an associated type definition in an impl block.
    /// Syntax: type Name = Type
    fn parse_impl_assoc_type(&mut self) -> Result<ImplAssocType, ParseError> {
        let start_span = self.cursor.current_span();

        // type
        self.cursor.expect(&TokenKind::Type)?;

        // Name
        let name = self.cursor.expect_ident()?;

        // = Type
        self.cursor.expect(&TokenKind::Eq)?;
        let ty = self.parse_type_required().into_result()?;

        let end_span = self.cursor.current_span();

        self.eat_optional_semicolon();

        Ok(ImplAssocType {
            name,
            ty,
            span: start_span.merge(end_span),
        })
    }

    /// Parse a default implementation block.
    ///
    /// Syntax: `[pub] def impl TraitName { methods }`
    ///
    /// Returns `EmptyErr` if no `def` keyword is present.
    ///
    /// Unlike regular `impl`:
    /// - No type parameters (no generics)
    /// - No `for Type` clause (anonymous implementation)
    /// - Methods must not have `self` parameter (stateless)
    pub(crate) fn parse_def_impl(&mut self, visibility: Visibility) -> ParseOutcome<DefImplDef> {
        if !self.cursor.check(&TokenKind::Def) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Def,
                self.cursor.current_span().start as usize,
            );
        }

        self.parse_def_impl_body(visibility)
    }

    fn parse_def_impl_body(&mut self, visibility: Visibility) -> ParseOutcome<DefImplDef> {
        let start_span = self.cursor.current_span();

        // def
        committed!(self.cursor.expect(&TokenKind::Def));

        // impl
        committed!(self.cursor.expect(&TokenKind::Impl));

        // TraitName (simple identifier, no path for now)
        let trait_name = committed!(self.cursor.expect_ident());

        // Body: { methods }
        committed!(self.cursor.expect(&TokenKind::LBrace));
        self.cursor.skip_newlines();

        let mut methods = Vec::new();

        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            if self.cursor.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body (def-impl context: forbids `uses` clause)
                let method = committed!(self.parse_impl_method(true));
                methods.push(method);
            } else {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1001,
                        format!(
                            "expected method definition (@name) in def impl block, found {}",
                            self.cursor.current_kind().display_name()
                        ),
                        self.cursor.current_span(),
                    ),
                    self.cursor.current_span(),
                );
            }
            self.cursor.skip_newlines();
        }

        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(DefImplDef {
            trait_name,
            methods,
            span: start_span.merge(end_span),
            visibility,
        })
    }
}

#[cfg(test)]
mod tests;
