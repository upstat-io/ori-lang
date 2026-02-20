//! Type declaration parsing (struct, enum, newtype).

use crate::{committed, ParseError, ParseOutcome, ParsedAttrs, Parser};
use ori_ir::{
    GenericParamRange, Name, ParsedType, ParsedTypeId, ParsedTypeRange, Span, StructField,
    TokenKind, TypeDecl, TypeDeclKind, Variant, VariantField, Visibility,
};

impl Parser<'_> {
    /// Parse a type declaration.
    ///
    /// Syntax:
    /// - Struct: `type Name = { field: Type, ... }`
    /// - Sum type: `type Name = Variant1 | Variant2(field: Type) | ...`
    /// - Newtype: `type Name = ExistingType`
    /// - Generic: `type Name<T> = ...`
    /// - With derives: `#[derive(Eq, Clone)] type Name = ...`
    ///
    /// Returns `EmptyErr` if no `type` keyword is present.
    pub(crate) fn parse_type_decl(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> ParseOutcome<TypeDecl> {
        if !self.cursor.check(&TokenKind::Type) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Type,
                self.cursor.current_span().start as usize,
            );
        }

        self.in_error_context(crate::ErrorContext::TypeDef, |p| {
            p.parse_type_decl_body(attrs, visibility)
        })
    }

    fn parse_type_decl_body(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> ParseOutcome<TypeDecl> {
        let start_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Type));

        let name = committed!(self.cursor.expect_ident());

        // Optional generics: <T, U: Bound>
        let generics = if self.cursor.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
        } else {
            GenericParamRange::EMPTY
        };

        committed!(self.cursor.expect(&TokenKind::Eq));

        // Determine kind based on what follows
        let kind = if self.cursor.check(&TokenKind::LBrace) {
            // Struct: { field: Type, ... }
            committed!(self.parse_struct_body())
        } else if self.cursor.check_ident() {
            // Could be a sum type (Variant | ...) or a newtype (ExistingType)
            committed!(self.parse_sum_or_newtype())
        } else {
            // Try to parse as a newtype with a primitive type
            let ty = committed!(self.parse_type_required().into_result());
            TypeDeclKind::Newtype(ty)
        };

        // Optional where clause (not common for type decls but supported)
        let where_clauses = if self.cursor.check(&TokenKind::Where) {
            committed!(self.parse_where_clauses().into_result())
        } else {
            Vec::new()
        };

        let end_span = self.cursor.previous_span();

        self.eat_optional_item_semicolon();

        ParseOutcome::consumed_ok(TypeDecl {
            name,
            generics,
            where_clauses,
            kind,
            span: start_span.merge(end_span),
            visibility,
            derives: attrs.derive_traits,
        })
    }

    /// Parse typed fields with a common structure: `name: Type, ...`
    ///
    /// Used for both struct fields and variant fields.
    fn parse_typed_fields<T, F>(
        &mut self,
        end_token: &TokenKind,
        make_field: F,
    ) -> Result<Vec<T>, ParseError>
    where
        F: Fn(Name, ParsedType, Span) -> T,
    {
        use crate::series::SeriesConfig;

        self.series(&SeriesConfig::comma(end_token.clone()), |p| {
            if p.cursor.check(end_token) {
                return Ok(None);
            }

            let field_span = p.cursor.current_span();
            let field_name = p.cursor.expect_ident()?;
            p.cursor.expect(&TokenKind::Colon)?;
            let field_ty = p.parse_type_required().into_result()?;

            Ok(Some(make_field(
                field_name,
                field_ty,
                field_span.merge(p.cursor.previous_span()),
            )))
        })
    }

    /// Parse struct body: { field: Type, ... }
    fn parse_struct_body(&mut self) -> Result<TypeDeclKind, ParseError> {
        self.cursor.expect(&TokenKind::LBrace)?;
        self.cursor.skip_newlines();

        let fields = self.parse_typed_fields(&TokenKind::RBrace, |name, ty, span| StructField {
            name,
            ty,
            span,
        })?;

        self.cursor.expect(&TokenKind::RBrace)?;
        Ok(TypeDeclKind::Struct(fields))
    }

    /// Parse sum type or newtype starting with an identifier.
    ///
    /// Sum type: `Variant1 | Variant2(field: Type) | ...`
    /// Newtype: `ExistingType` or `ExistingType<Args>`
    fn parse_sum_or_newtype(&mut self) -> Result<TypeDeclKind, ParseError> {
        let first_name = self.cursor.expect_ident()?;
        let first_span = self.cursor.previous_span();

        // Check for generic args on newtype: MyType<T>
        if self.cursor.check(&TokenKind::Lt) {
            use crate::series::SeriesConfig;

            // This is a newtype with generic args
            self.cursor.advance(); // <
                                   // Type arg lists use a Vec because nested generic args share the
                                   // same `parsed_type_lists` buffer (e.g., `NewType<Option<T>>`).
            let mut type_arg_list: Vec<ParsedTypeId> = Vec::new();
            self.series_direct(&SeriesConfig::comma(TokenKind::Gt).no_newlines(), |p| {
                if p.cursor.check(&TokenKind::Gt) {
                    return Ok(false);
                }
                let ty = p.parse_type_required().into_result()?;
                type_arg_list.push(p.arena.alloc_parsed_type(ty));
                Ok(true)
            })?;
            if self.cursor.check(&TokenKind::Gt) {
                self.cursor.advance(); // >
            }
            let type_args = self.arena.alloc_parsed_type_list(type_arg_list);
            return Ok(TypeDeclKind::Newtype(ParsedType::Named {
                name: first_name,
                type_args,
            }));
        }

        // Check if this is a sum type (has | following)
        if self.cursor.check(&TokenKind::Pipe) {
            // Sum type - parse first variant and continue
            let first_variant = self.make_variant(first_name, first_span)?;
            let mut variants = vec![first_variant];

            while self.cursor.check(&TokenKind::Pipe) {
                self.cursor.advance(); // |
                self.cursor.skip_newlines();

                let var_name = self.cursor.expect_ident()?;
                let var_span = self.cursor.previous_span();
                let variant = self.make_variant(var_name, var_span)?;
                variants.push(variant);
            }

            return Ok(TypeDeclKind::Sum(variants));
        }

        // Check if first variant has fields (indicates sum type)
        if self.cursor.check(&TokenKind::LParen) {
            // Sum type with fields on first variant
            let first_variant = self.make_variant(first_name, first_span)?;
            let mut variants = vec![first_variant];

            while self.cursor.check(&TokenKind::Pipe) {
                self.cursor.advance(); // |
                self.cursor.skip_newlines();

                let var_name = self.cursor.expect_ident()?;
                let var_span = self.cursor.previous_span();
                let variant = self.make_variant(var_name, var_span)?;
                variants.push(variant);
            }

            return Ok(TypeDeclKind::Sum(variants));
        }

        // Single identifier without | or ( - newtype referring to another type
        Ok(TypeDeclKind::Newtype(ParsedType::Named {
            name: first_name,
            type_args: ParsedTypeRange::EMPTY,
        }))
    }

    /// Create a Variant, parsing optional fields.
    fn make_variant(&mut self, name: Name, start_span: Span) -> Result<Variant, ParseError> {
        // Check for variant fields: (field: Type, ...)
        let fields = if self.cursor.check(&TokenKind::LParen) {
            self.cursor.advance(); // (
            self.cursor.skip_newlines();

            let fields = self.parse_typed_fields(&TokenKind::RParen, |name, ty, span| {
                VariantField { name, ty, span }
            })?;

            self.cursor.expect(&TokenKind::RParen)?;
            fields
        } else {
            Vec::new()
        };

        Ok(Variant {
            name,
            fields,
            span: start_span.merge(self.cursor.previous_span()),
        })
    }
}
