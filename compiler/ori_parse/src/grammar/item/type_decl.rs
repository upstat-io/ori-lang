//! Type declaration parsing (struct, enum, newtype).

use crate::{ParseError, ParseOutcome, ParsedAttrs, Parser};
use ori_ir::{
    GenericParamRange, Name, ParsedType, ParsedTypeId, ParsedTypeRange, Span, StructField,
    TokenKind, TypeDecl, TypeDeclKind, Variant, VariantField, Visibility,
};

impl Parser<'_> {
    /// Parse a type declaration with outcome tracking.
    pub(crate) fn parse_type_decl_with_outcome(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> ParseOutcome<TypeDecl> {
        self.with_outcome(|p| p.parse_type_decl(attrs, visibility))
    }

    /// Parse a type declaration.
    ///
    /// Syntax:
    /// - Struct: `type Name = { field: Type, ... }`
    /// - Sum type: `type Name = Variant1 | Variant2(field: Type) | ...`
    /// - Newtype: `type Name = ExistingType`
    /// - Generic: `type Name<T> = ...`
    /// - With derives: `#[derive(Eq, Clone)] type Name = ...`
    pub(crate) fn parse_type_decl(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> Result<TypeDecl, ParseError> {
        self.in_error_context_result(crate::ErrorContext::TypeDef, |p| {
            p.parse_type_decl_inner(attrs, visibility)
        })
    }

    fn parse_type_decl_inner(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> Result<TypeDecl, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Type)?;

        let name = self.expect_ident()?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        self.expect(&TokenKind::Eq)?;

        // Determine kind based on what follows
        let kind = if self.check(&TokenKind::LBrace) {
            // Struct: { field: Type, ... }
            self.parse_struct_body()?
        } else if self.check_ident() {
            // Could be a sum type (Variant | ...) or a newtype (ExistingType)
            self.parse_sum_or_newtype()?
        } else {
            // Try to parse as a newtype with a primitive type
            let ty = self.parse_type_required()?;
            TypeDeclKind::Newtype(ty)
        };

        // Optional where clause (not common for type decls but supported)
        let where_clauses = if self.check(&TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        let end_span = self.previous_span();

        Ok(TypeDecl {
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
            if p.check(end_token) {
                return Ok(None);
            }

            let field_span = p.current_span();
            let field_name = p.expect_ident()?;
            p.expect(&TokenKind::Colon)?;
            let field_ty = p.parse_type_required()?;

            Ok(Some(make_field(
                field_name,
                field_ty,
                field_span.merge(p.previous_span()),
            )))
        })
    }

    /// Parse struct body: { field: Type, ... }
    fn parse_struct_body(&mut self) -> Result<TypeDeclKind, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let fields = self.parse_typed_fields(&TokenKind::RBrace, |name, ty, span| StructField {
            name,
            ty,
            span,
        })?;

        self.expect(&TokenKind::RBrace)?;
        Ok(TypeDeclKind::Struct(fields))
    }

    /// Parse sum type or newtype starting with an identifier.
    ///
    /// Sum type: `Variant1 | Variant2(field: Type) | ...`
    /// Newtype: `ExistingType` or `ExistingType<Args>`
    fn parse_sum_or_newtype(&mut self) -> Result<TypeDeclKind, ParseError> {
        let first_name = self.expect_ident()?;
        let first_span = self.previous_span();

        // Check for generic args on newtype: MyType<T>
        if self.check(&TokenKind::Lt) {
            use crate::series::SeriesConfig;

            // This is a newtype with generic args
            self.advance(); // <
            let arg_ids: Vec<ParsedTypeId> =
                self.series(&SeriesConfig::comma(TokenKind::Gt).no_newlines(), |p| {
                    if p.check(&TokenKind::Gt) {
                        return Ok(None);
                    }
                    let ty = p.parse_type_required()?;
                    let id = p.arena.alloc_parsed_type(ty);
                    Ok(Some(id))
                })?;
            if self.check(&TokenKind::Gt) {
                self.advance(); // >
            }
            let type_args = self.arena.alloc_parsed_type_list(arg_ids);
            return Ok(TypeDeclKind::Newtype(ParsedType::Named {
                name: first_name,
                type_args,
            }));
        }

        // Check if this is a sum type (has | following)
        if self.check(&TokenKind::Pipe) {
            // Sum type - parse first variant and continue
            let first_variant = self.make_variant(first_name, first_span)?;
            let mut variants = vec![first_variant];

            while self.check(&TokenKind::Pipe) {
                self.advance(); // |
                self.skip_newlines();

                let var_name = self.expect_ident()?;
                let var_span = self.previous_span();
                let variant = self.make_variant(var_name, var_span)?;
                variants.push(variant);
            }

            return Ok(TypeDeclKind::Sum(variants));
        }

        // Check if first variant has fields (indicates sum type)
        if self.check(&TokenKind::LParen) {
            // Sum type with fields on first variant
            let first_variant = self.make_variant(first_name, first_span)?;
            let mut variants = vec![first_variant];

            while self.check(&TokenKind::Pipe) {
                self.advance(); // |
                self.skip_newlines();

                let var_name = self.expect_ident()?;
                let var_span = self.previous_span();
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
        let fields = if self.check(&TokenKind::LParen) {
            self.advance(); // (
            self.skip_newlines();

            let fields = self.parse_typed_fields(&TokenKind::RParen, |name, ty, span| {
                VariantField { name, ty, span }
            })?;

            self.expect(&TokenKind::RParen)?;
            fields
        } else {
            Vec::new()
        };

        Ok(Variant {
            name,
            fields,
            span: start_span.merge(self.previous_span()),
        })
    }
}
