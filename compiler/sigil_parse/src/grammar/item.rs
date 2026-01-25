//! Item parsing (functions, tests, imports, traits, impls).
//!
//! This module extends Parser with methods for parsing top-level items
//! like function definitions, test definitions, import statements,
//! trait definitions, and implementation blocks.

use sigil_ir::{
    ConfigDef, Function, ImportPath, Name, Param, ParamRange, TestDef, TokenKind, ParsedType,
    UseDef, UseItem, GenericParam, GenericParamRange, TraitBound, WhereClause,
    TraitDef, TraitItem, TraitMethodSig, TraitDefaultMethod, TraitAssocType,
    ImplDef, ImplMethod,
    TypeDecl, TypeDeclKind, StructField, Variant, VariantField,
};
use crate::{FunctionOrTest, ParsedAttrs, ParseError, Parser};

impl Parser<'_> {
    /// Parse a use/import statement.
    /// Syntax: use './path' { item1, item2 as alias } or use std.math { sqrt }
    pub(crate) fn parse_use(&mut self) -> Result<UseDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Use)?;

        // Parse import path
        let path = if let TokenKind::String(s) = self.current_kind() {
            // Relative path: './math', '../utils'
            self.advance();
            ImportPath::Relative(s)
        } else {
            // Module path: std.math, std.collections
            let mut segments = Vec::new();
            loop {
                let name = self.expect_ident()?;
                segments.push(name);

                if self.check(&TokenKind::Dot) {
                    self.advance();
                } else {
                    break;
                }
            }
            ImportPath::Module(segments)
        };

        // Parse imported items: { item1, item2 as alias }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            // Check for private import prefix ::
            let is_private = if self.check(&TokenKind::DoubleColon) {
                self.advance();
                true
            } else {
                false
            };

            // Item name
            let name = self.expect_ident()?;

            // Optional alias: `as alias`
            let alias = if self.check(&TokenKind::As) {
                self.advance();
                Some(self.expect_ident()?)
            } else {
                None
            };

            items.push(UseItem { name, alias, is_private });

            // Comma separator (optional before closing brace)
            if self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                self.skip_newlines();
                break;
            }
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(UseDef {
            path,
            items,
            span: start_span.merge(end_span),
        })
    }

    /// Parse a config variable declaration.
    ///
    /// Syntax: `[pub] $name = literal`
    pub(crate) fn parse_config(&mut self, is_public: bool) -> Result<ConfigDef, ParseError> {
        let start_span = self.current_span();

        // $
        self.expect(&TokenKind::Dollar)?;

        // name
        let name = self.expect_ident()?;

        // =
        self.expect(&TokenKind::Eq)?;

        // literal value
        let value = self.parse_literal_expr()?;

        let span = start_span.merge(self.previous_span());

        Ok(ConfigDef {
            name,
            value,
            span,
            is_public,
        })
    }

    /// Parse a literal expression for config values.
    fn parse_literal_expr(&mut self) -> Result<sigil_ir::ExprId, ParseError> {
        use sigil_ir::{Expr, ExprKind};

        let span = self.current_span();
        let kind = match self.current_kind() {
            TokenKind::Int(n) => {
                self.advance();
                ExprKind::Int(n)
            }
            TokenKind::Float(bits) => {
                self.advance();
                ExprKind::Float(bits)
            }
            TokenKind::String(s) => {
                self.advance();
                ExprKind::String(s)
            }
            TokenKind::True => {
                self.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Char(c) => {
                self.advance();
                ExprKind::Char(c)
            }
            // Duration literals (e.g., 100ms, 30s)
            TokenKind::Duration(value, unit) => {
                self.advance();
                ExprKind::Duration { value, unit }
            }
            // Size literals (e.g., 4kb, 10mb)
            TokenKind::Size(value, unit) => {
                self.advance();
                ExprKind::Size { value, unit }
            }
            _ => {
                return Err(ParseError::new(
                    sigil_diagnostic::ErrorCode::E1002,
                    "config variable must be initialized with a literal value".to_string(),
                    span,
                ));
            }
        };

        Ok(self.arena.alloc_expr(Expr::new(kind, span)))
    }

    /// Parse a function or test definition with attributes.
    ///
    /// Function: @name (params) -> Type = body
    /// Targeted test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Free-floating test: @`test_name` (params) -> void = body
    pub(crate) fn parse_function_or_test_with_attrs(&mut self, attrs: ParsedAttrs, is_public: bool) -> Result<FunctionOrTest, ParseError> {
        let start_span = self.current_span();

        // @
        self.expect(&TokenKind::At)?;

        // name
        let name = self.expect_ident()?;
        let name_str = self.interner().lookup(name);
        let is_test_named = name_str.starts_with("test_");

        // Check if this is a targeted test (has `tests` keyword)
        if self.check(&TokenKind::Tests) {
            // Parse test targets: tests @target1 tests @target2 ...
            let mut targets = Vec::new();
            while self.check(&TokenKind::Tests) {
                self.advance(); // consume `tests`
                self.expect(&TokenKind::At)?;
                let target = self.expect_ident()?;
                targets.push(target);
            }

            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Test(TestDef {
                name,
                targets,
                params,
                return_ty,
                body,
                span,
                skip_reason: attrs.skip_reason,
                expected_errors: attrs.expected_errors,
                fail_expected: attrs.fail_expected,
            }))
        } else if is_test_named {
            // Free-floating test (name starts with test_ but no targets)
            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Test(TestDef {
                name,
                targets: Vec::new(), // No targets for free-floating tests
                params,
                return_ty,
                body,
                span,
                skip_reason: attrs.skip_reason,
                expected_errors: attrs.expected_errors,
                fail_expected: attrs.fail_expected,
            }))
        } else {
            // Regular function
            // Optional generic parameters: <T, U: Bound>
            let generics = if self.check(&TokenKind::Lt) {
                self.parse_generics()?
            } else {
                GenericParamRange::EMPTY
            };

            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // Optional where clauses: where T: Clone, U: Default
            let where_clauses = if self.check(&TokenKind::Where) {
                self.parse_where_clauses()?
            } else {
                Vec::new()
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Function(Function {
                name,
                generics,
                params,
                return_ty,
                where_clauses,
                body,
                span,
                is_public,
            }))
        }
    }

    /// Parse parameter list.
    /// Accepts both regular identifiers and `self` for trait methods.
    pub(crate) fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            let param_span = self.current_span();

            // Accept `self` as a special parameter name for trait/impl methods
            let name = if self.check(&TokenKind::SelfLower) {
                self.advance();
                self.interner().intern("self")
            } else {
                self.expect_ident()?
            };

            // : Type (optional, not required for `self`)
            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            params.push(Param { name, ty, span: param_span });

            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
            }
        }

        Ok(self.arena.alloc_params(params))
    }

    // =========================================================================
    // Trait and Impl Parsing
    // =========================================================================

    /// Parse a trait definition.
    /// Syntax: [pub] trait Name [<T>] [: Super] { items }
    pub(crate) fn parse_trait(&mut self, is_public: bool) -> Result<TraitDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Trait)?;

        // Trait name
        let name = self.expect_ident()?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Optional super-traits: : Parent + OtherTrait
        let super_traits = if self.check(&TokenKind::Colon) {
            self.advance();
            self.parse_bounds()?
        } else {
            Vec::new()
        };

        // Trait body: { items }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_trait_item() {
                Ok(item) => items.push(item),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(TraitDef {
            name,
            generics,
            super_traits,
            items,
            span: start_span.merge(end_span),
            is_public,
        })
    }

    /// Parse a single trait item (method signature, default method, or associated type).
    fn parse_trait_item(&mut self) -> Result<TraitItem, ParseError> {
        if self.check(&TokenKind::Type) {
            // Associated type: type Item
            let start_span = self.current_span();
            self.advance(); // consume `type`
            let name = self.expect_ident()?;
            Ok(TraitItem::AssocType(TraitAssocType {
                name,
                span: start_span.merge(self.previous_span()),
            }))
        } else if self.check(&TokenKind::At) {
            // Method: @name (params) -> Type [= body]
            let start_span = self.current_span();
            self.advance(); // consume `@`
            let name = self.expect_ident()?;

            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type
            self.expect(&TokenKind::Arrow)?;
            let return_ty = self.parse_type_required()?;

            // Check for default implementation: = body
            if self.check(&TokenKind::Eq) {
                self.advance();
                let body = self.parse_expr()?;
                let end_span = self.arena.get_expr(body).span;
                Ok(TraitItem::DefaultMethod(TraitDefaultMethod {
                    name,
                    params,
                    return_ty,
                    body,
                    span: start_span.merge(end_span),
                }))
            } else {
                Ok(TraitItem::MethodSig(TraitMethodSig {
                    name,
                    params,
                    return_ty,
                    span: start_span.merge(self.previous_span()),
                }))
            }
        } else {
            Err(ParseError::new(
                sigil_diagnostic::ErrorCode::E1002,
                format!("expected trait item (method or associated type), found {:?}", self.current_kind()),
                self.current_span(),
            ))
        }
    }

    /// Parse an impl block.
    /// Syntax: impl [<T>] Type { methods } or impl [<T>] Trait for Type { methods }
    pub(crate) fn parse_impl(&mut self) -> Result<ImplDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Impl)?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the first type path (could be trait or self_ty)
        let first_path = self.parse_type_path()?;
        let first_ty = self.make_type_from_path(&first_path)?;

        // Check for `for` keyword to determine if this is a trait impl
        let (trait_path, self_path, self_ty) = if self.check(&TokenKind::For) {
            self.advance();
            // Parse the implementing type as a type path
            let impl_path = self.parse_type_path()?;
            let impl_ty = self.make_type_from_path(&impl_path)?;
            (Some(first_path), impl_path, impl_ty)
        } else {
            (None, first_path, first_ty)
        };

        // Optional where clause
        let where_clauses = if self.check(&TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // Impl body: { methods }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_impl_method() {
                Ok(method) => methods.push(method),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(ImplDef {
            generics,
            trait_path,
            self_path,
            self_ty,
            where_clauses,
            methods,
            span: start_span.merge(end_span),
        })
    }

    /// Parse a method in an impl block.
    fn parse_impl_method(&mut self) -> Result<ImplMethod, ParseError> {
        let start_span = self.current_span();

        // @name
        self.expect(&TokenKind::At)?;
        let name = self.expect_ident()?;

        // (params)
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        // -> Type
        self.expect(&TokenKind::Arrow)?;
        let return_ty = self.parse_type_required()?;

        // = body
        self.expect(&TokenKind::Eq)?;
        let body = self.parse_expr()?;

        let end_span = self.arena.get_expr(body).span;

        Ok(ImplMethod {
            name,
            params,
            return_ty,
            body,
            span: start_span.merge(end_span),
        })
    }

    // =========================================================================
    // Type Declarations
    // =========================================================================

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
    fn make_variant(&mut self, name: Name, start_span: sigil_ir::Span) -> Result<Variant, ParseError> {
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

    // =========================================================================
    // Extension Methods
    // =========================================================================

    /// Parse an extend block.
    /// Syntax: extend [<T>] Type { methods }
    ///
    /// Examples:
    ///   extend [T] { @map... }           - extends all lists
    ///   extend<T> Option<T> { @map... }  - extends Option
    ///   extend str { @reverse... }       - extends str
    pub(crate) fn parse_extend(&mut self) -> Result<sigil_ir::ExtendDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Extend)?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the target type
        // Handle [T] for list types
        let (target_ty, target_type_name) = if self.check(&TokenKind::LBracket) {
            self.advance(); // [
            // Parse element type (optional, default to infer)
            let elem_ty = if self.check(&TokenKind::RBracket) {
                ParsedType::Infer
            } else {
                self.parse_type_required()?
            };
            self.expect(&TokenKind::RBracket)?;
            // List type - method dispatch uses "list"
            (ParsedType::List(Box::new(elem_ty)), self.interner().intern("list"))
        } else if self.check_type_keyword() {
            // Primitive type keywords: str, int, float, bool, etc.
            let (ty, type_name_str) = match self.current_kind() {
                TokenKind::StrType => (ParsedType::Primitive(sigil_ir::TypeId::STR), "str"),
                TokenKind::IntType => (ParsedType::Primitive(sigil_ir::TypeId::INT), "int"),
                TokenKind::FloatType => (ParsedType::Primitive(sigil_ir::TypeId::FLOAT), "float"),
                TokenKind::BoolType => (ParsedType::Primitive(sigil_ir::TypeId::BOOL), "bool"),
                TokenKind::CharType => (ParsedType::Primitive(sigil_ir::TypeId::CHAR), "char"),
                TokenKind::ByteType => (ParsedType::Primitive(sigil_ir::TypeId::BYTE), "byte"),
                _ => (ParsedType::Infer, "unknown"),
            };
            self.advance();
            (ty, self.interner().intern(type_name_str))
        } else {
            // Named type like Option<T>, MyType, etc.
            let type_name = self.expect_ident()?;
            // Check for generic parameters like Option<T>
            let type_args = if self.check(&TokenKind::Lt) {
                self.advance(); // <
                let mut args = Vec::new();
                while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                    args.push(self.parse_type_required()?);
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.check(&TokenKind::Gt) {
                    self.advance(); // >
                }
                args
            } else {
                Vec::new()
            };
            (ParsedType::Named { name: type_name, type_args }, type_name)
        };

        // Optional where clause
        let where_clauses = if self.check(&TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // Extend body: { methods }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_impl_method() {
                Ok(method) => methods.push(method),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(sigil_ir::ExtendDef {
            generics,
            target_ty,
            target_type_name,
            where_clauses,
            methods,
            span: start_span.merge(end_span),
        })
    }

    // =========================================================================
    // Generic and Bound Parsing Helpers
    // =========================================================================

    /// Parse a type, accepting all type forms (primitives, named, compounds).
    /// Returns `ParsedType` representing the full type structure.
    fn parse_type_required(&mut self) -> Result<ParsedType, ParseError> {
        if let Some(ty) = self.parse_type() {
            return Ok(ty);
        }

        Err(ParseError::new(
            sigil_diagnostic::ErrorCode::E1002,
            format!("expected type, found {:?}", self.current_kind()),
            self.current_span(),
        ))
    }

    /// Parse generic parameters: <T, U: Bound>
    pub(crate) fn parse_generics(&mut self) -> Result<GenericParamRange, ParseError> {
        self.expect(&TokenKind::Lt)?;

        let mut params = Vec::new();
        while !self.check(&TokenKind::Gt) && !self.is_at_end() {
            let param_span = self.current_span();
            let name = self.expect_ident()?;

            // Optional bounds: : Bound + OtherBound
            let bounds = if self.check(&TokenKind::Colon) {
                self.advance();
                self.parse_bounds()?
            } else {
                Vec::new()
            };

            params.push(GenericParam {
                name,
                bounds,
                span: param_span.merge(self.previous_span()),
            });

            if !self.check(&TokenKind::Gt) {
                self.expect(&TokenKind::Comma)?;
            }
        }

        self.expect(&TokenKind::Gt)?;
        Ok(self.arena.alloc_generic_params(params))
    }

    /// Parse trait bounds: Eq + Clone + Printable
    fn parse_bounds(&mut self) -> Result<Vec<TraitBound>, ParseError> {
        let mut bounds = Vec::new();

        loop {
            let bound_span = self.current_span();
            let (first, rest) = self.parse_type_path_parts()?;

            bounds.push(TraitBound {
                first,
                rest,
                span: bound_span.merge(self.previous_span()),
            });

            if self.check(&TokenKind::Plus) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(bounds)
    }

    /// Parse a type path: Name or std.collections.List
    fn parse_type_path(&mut self) -> Result<Vec<sigil_ir::Name>, ParseError> {
        let (first, rest) = self.parse_type_path_parts()?;
        let mut segments = vec![first];
        segments.extend(rest);
        Ok(segments)
    }

    /// Parse a type path as (`first_segment`, `rest_segments`).
    /// Guarantees at least one segment by returning the first separately.
    fn parse_type_path_parts(&mut self) -> Result<(sigil_ir::Name, Vec<sigil_ir::Name>), ParseError> {
        let first = self.expect_ident()?;
        let mut rest = Vec::new();

        while self.check(&TokenKind::Dot) {
            self.advance();
            let segment = self.expect_ident()?;
            rest.push(segment);
        }

        Ok((first, rest))
    }

    /// Convert a type path to a `ParsedType`.
    /// Creates a Named type with the last segment as the name.
    fn make_type_from_path(&mut self, path: &[sigil_ir::Name]) -> Result<ParsedType, ParseError> {
        // Use the last segment as the type name
        // TODO: Support full path resolution in type checker
        match path.last() {
            Some(&name) => Ok(ParsedType::Named { name, type_args: Vec::new() }),
            None => Err(ParseError::new(
                sigil_diagnostic::ErrorCode::E1002,
                "empty type path".to_string(),
                self.current_span(),
            )),
        }
    }

    /// Parse where clauses: where T: Clone, U: Default
    fn parse_where_clauses(&mut self) -> Result<Vec<WhereClause>, ParseError> {
        self.expect(&TokenKind::Where)?;

        let mut clauses = Vec::new();
        loop {
            let clause_span = self.current_span();
            let param = self.expect_ident()?;

            self.expect(&TokenKind::Colon)?;
            let bounds = self.parse_bounds()?;

            clauses.push(WhereClause {
                param,
                bounds,
                span: clause_span.merge(self.previous_span()),
            });

            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(clauses)
    }
}
