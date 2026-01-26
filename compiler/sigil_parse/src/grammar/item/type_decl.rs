//! Type declaration parsing (struct, enum, newtype).

use sigil_ir::{
    GenericParamRange, Name, ParsedType, Span, StructField, TokenKind, TypeDecl, TypeDeclKind,
    Variant, VariantField,
};
use crate::{ParsedAttrs, ParseError, Parser};

impl Parser<'_> {
    /// Parse a type declaration.
    ///
    /// Syntax:
    /// - Struct: `type Name = { field: Type, ... }`
    /// - Sum type: `type Name = Variant1 | Variant2(field: Type) | ...`
    /// - Newtype: `type Name = ExistingType`
    /// - Generic: `type Name<T> = ...`
    /// - With derives: `#[derive(Eq, Clone)] type Name = ...`
    pub(crate) fn parse_type_decl(&mut self, attrs: ParsedAttrs, is_public: bool) -> Result<TypeDecl, ParseError> {
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
            is_public,
            derives: attrs.derive_traits,
        })
    }

    /// Parse struct body: { field: Type, ... }
    fn parse_struct_body(&mut self) -> Result<TypeDeclKind, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let field_span = self.current_span();
            let field_name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let field_ty = self.parse_type_required()?;

            fields.push(StructField {
                name: field_name,
                ty: field_ty,
                span: field_span.merge(self.previous_span()),
            });

            // Comma separator (optional before closing brace)
            if self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                self.skip_newlines();
                break;
            }
        }

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
            // This is a newtype with generic args
            self.advance(); // <
            let mut type_args = Vec::new();
            while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                type_args.push(self.parse_type_required()?);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.check(&TokenKind::Gt) {
                self.advance(); // >
            }
            return Ok(TypeDeclKind::Newtype(ParsedType::Named { name: first_name, type_args }));
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
        Ok(TypeDeclKind::Newtype(ParsedType::Named { name: first_name, type_args: Vec::new() }))
    }

    /// Create a Variant, parsing optional fields.
    fn make_variant(&mut self, name: Name, start_span: Span) -> Result<Variant, ParseError> {
        // Check for variant fields: (field: Type, ...)
        let fields = if self.check(&TokenKind::LParen) {
            self.advance(); // (
            self.skip_newlines();

            let mut fields = Vec::new();
            while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                let field_span = self.current_span();
                let field_name = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let field_ty = self.parse_type_required()?;

                fields.push(VariantField {
                    name: field_name,
                    ty: field_ty,
                    span: field_span.merge(self.previous_span()),
                });

                // Comma separator
                if self.check(&TokenKind::Comma) {
                    self.advance();
                    self.skip_newlines();
                } else {
                    break;
                }
            }
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
