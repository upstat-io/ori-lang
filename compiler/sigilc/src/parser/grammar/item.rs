//! Item parsing (functions, tests, imports, traits, impls).
//!
//! This module extends Parser with methods for parsing top-level items
//! like function definitions, test definitions, import statements,
//! trait definitions, and implementation blocks.

use crate::ir::{
    Function, ImportPath, Name, Param, ParamRange, TestDef, TokenKind, TypeId, UseDef, UseItem,
    GenericParam, GenericParamRange, TraitBound, WhereClause,
    TraitDef, TraitItem, TraitMethodSig, TraitDefaultMethod, TraitAssocType,
    ImplDef, ImplMethod,
    TypeDecl, TypeDeclKind, StructField, Variant, VariantField,
};
use crate::parser::{FunctionOrTest, ParsedAttrs, ParseError, Parser};

impl<'a> Parser<'a> {
    /// Parse a use/import statement.
    /// Syntax: use './path' { item1, item2 as alias } or use std.math { sqrt }
    pub(in crate::parser) fn parse_use(&mut self) -> Result<UseDef, ParseError> {
        let start_span = self.current_span();
        self.expect(TokenKind::Use)?;

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

                if self.check(TokenKind::Dot) {
                    self.advance();
                } else {
                    break;
                }
            }
            ImportPath::Module(segments)
        };

        // Parse imported items: { item1, item2 as alias }
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            // Check for private import prefix ::
            let is_private = if self.check(TokenKind::DoubleColon) {
                self.advance();
                true
            } else {
                false
            };

            // Item name
            let name = self.expect_ident()?;

            // Optional alias: `as alias`
            let alias = if self.check(TokenKind::As) {
                self.advance();
                Some(self.expect_ident()?)
            } else {
                None
            };

            items.push(UseItem { name, alias, is_private });

            // Comma separator (optional before closing brace)
            if self.check(TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                self.skip_newlines();
                break;
            }
        }

        let end_span = self.current_span();
        self.expect(TokenKind::RBrace)?;

        Ok(UseDef {
            path,
            items,
            span: start_span.merge(end_span),
        })
    }


    /// Parse a function or test definition with attributes.
    ///
    /// Function: @name (params) -> Type = body
    /// Targeted test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Free-floating test: @test_name (params) -> void = body
    pub(in crate::parser) fn parse_function_or_test_with_attrs(&mut self, attrs: ParsedAttrs, is_public: bool) -> Result<FunctionOrTest, ParseError> {
        let start_span = self.current_span();

        // @
        self.expect(TokenKind::At)?;

        // name
        let name = self.expect_ident()?;
        let name_str = self.interner().lookup(name);
        let is_test_named = name_str.starts_with("test_");

        // Check if this is a targeted test (has `tests` keyword)
        if self.check(TokenKind::Tests) {
            // Parse test targets: tests @target1 tests @target2 ...
            let mut targets = Vec::new();
            while self.check(TokenKind::Tests) {
                self.advance(); // consume `tests`
                self.expect(TokenKind::At)?;
                let target = self.expect_ident()?;
                targets.push(target);
            }

            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(TokenKind::Eq)?;
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
                compile_fail_expected: attrs.compile_fail_expected,
                fail_expected: attrs.fail_expected,
            }))
        } else if is_test_named {
            // Free-floating test (name starts with test_ but no targets)
            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(TokenKind::Eq)?;
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
                compile_fail_expected: attrs.compile_fail_expected,
                fail_expected: attrs.fail_expected,
            }))
        } else {
            // Regular function
            // Optional generic parameters: <T, U: Bound>
            let generics = if self.check(TokenKind::Lt) {
                self.parse_generics()?
            } else {
                GenericParamRange::EMPTY
            };

            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // Optional where clauses: where T: Clone, U: Default
            let where_clauses = if self.check(TokenKind::Where) {
                self.parse_where_clauses()?
            } else {
                Vec::new()
            };

            // = body
            self.expect(TokenKind::Eq)?;
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
    pub(in crate::parser) fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            let param_span = self.current_span();

            // Accept `self` as a special parameter name for trait/impl methods
            let name = if self.check(TokenKind::SelfLower) {
                self.advance();
                self.interner().intern("self")
            } else {
                self.expect_ident()?
            };

            // : Type (optional, not required for `self`)
            // Capture type annotation name for generic parameter tracking
            let (ty, type_name) = if self.check(TokenKind::Colon) {
                self.advance();
                self.parse_type_with_name()
            } else {
                (None, None)
            };

            params.push(Param { name, ty, type_name, span: param_span });

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        Ok(self.arena.alloc_params(params))
    }

    /// Parse a type annotation and capture the type name if it's an identifier.
    ///
    /// Returns (TypeId, type_name) where type_name is Some if the annotation
    /// was a named type like `T` (used for generic parameter tracking).
    fn parse_type_with_name(&mut self) -> (Option<TypeId>, Option<Name>) {
        // Check for Self type
        if self.check(TokenKind::SelfUpper) {
            self.advance();
            let self_name = self.interner().intern("Self");
            return (Some(TypeId::INFER), Some(self_name));
        }

        // Check for identifier (named type like T, MyType, etc.)
        if self.check_ident() {
            let type_name = if let TokenKind::Ident(name) = &self.current().kind {
                Some(*name)
            } else {
                None
            };
            self.advance();

            // Check for generic parameters like Option<T>
            if self.check(TokenKind::Lt) {
                self.advance(); // <
                while !self.check(TokenKind::Gt) && !self.is_at_end() {
                    // Recursively parse type args (ignore names for now)
                    let _ = self.parse_type_with_name();
                    if self.check(TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.check(TokenKind::Gt) {
                    self.advance(); // >
                }
            }

            return (Some(TypeId::INFER), type_name);
        }

        // Fall back to regular type parsing for primitives
        (self.parse_type(), None)
    }

    // =========================================================================
    // Trait and Impl Parsing
    // =========================================================================

    /// Parse a trait definition.
    /// Syntax: [pub] trait Name [<T>] [: Super] { items }
    #[allow(dead_code)]
    pub(in crate::parser) fn parse_trait(&mut self, is_public: bool) -> Result<TraitDef, ParseError> {
        let start_span = self.current_span();
        self.expect(TokenKind::Trait)?;

        // Trait name
        let name = self.expect_ident()?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Optional super-traits: : Parent + OtherTrait
        let super_traits = if self.check(TokenKind::Colon) {
            self.advance();
            self.parse_bounds()?
        } else {
            Vec::new()
        };

        // Trait body: { items }
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_trait_item() {
                Ok(item) => items.push(item),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(TokenKind::RBrace)?;

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
        if self.check(TokenKind::Type) {
            // Associated type: type Item
            let start_span = self.current_span();
            self.advance(); // consume `type`
            let name = self.expect_ident()?;
            Ok(TraitItem::AssocType(TraitAssocType {
                name,
                span: start_span.merge(self.previous_span()),
            }))
        } else if self.check(TokenKind::At) {
            // Method: @name (params) -> Type [= body]
            let start_span = self.current_span();
            self.advance(); // consume `@`
            let name = self.expect_ident()?;

            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type
            self.expect(TokenKind::Arrow)?;
            let return_ty = self.parse_type_required()?;

            // Check for default implementation: = body
            if self.check(TokenKind::Eq) {
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
                crate::diagnostic::ErrorCode::E1002,
                format!("expected trait item (method or associated type), found {:?}", self.current_kind()),
                self.current_span(),
            ))
        }
    }

    /// Parse an impl block.
    /// Syntax: impl [<T>] Type { methods } or impl [<T>] Trait for Type { methods }
    #[allow(dead_code)]
    pub(in crate::parser) fn parse_impl(&mut self) -> Result<ImplDef, ParseError> {
        let start_span = self.current_span();
        self.expect(TokenKind::Impl)?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the first type path (could be trait or self_ty)
        let first_path = self.parse_type_path()?;
        let first_ty = self.make_type_from_path(&first_path)?;

        // Check for `for` keyword to determine if this is a trait impl
        let (trait_path, self_path, self_ty) = if self.check(TokenKind::For) {
            self.advance();
            // Parse the implementing type as a type path
            let impl_path = self.parse_type_path()?;
            let impl_ty = self.make_type_from_path(&impl_path)?;
            (Some(first_path), impl_path, impl_ty)
        } else {
            (None, first_path, first_ty)
        };

        // Optional where clause
        let where_clauses = if self.check(TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // Impl body: { methods }
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_impl_method() {
                Ok(method) => methods.push(method),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(TokenKind::RBrace)?;

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
    #[allow(dead_code)]
    fn parse_impl_method(&mut self) -> Result<ImplMethod, ParseError> {
        let start_span = self.current_span();

        // @name
        self.expect(TokenKind::At)?;
        let name = self.expect_ident()?;

        // (params)
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;

        // -> Type
        self.expect(TokenKind::Arrow)?;
        let return_ty = self.parse_type_required()?;

        // = body
        self.expect(TokenKind::Eq)?;
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
    // Extension Methods
    // =========================================================================

    /// Parse an extend block.
    /// Syntax: extend [<T>] Type { methods }
    ///
    /// Examples:
    ///   extend [T] { @map... }           - extends all lists
    ///   extend<T> Option<T> { @map... }  - extends Option
    ///   extend str { @reverse... }       - extends str
    pub(in crate::parser) fn parse_extend(&mut self) -> Result<crate::ir::ExtendDef, ParseError> {
        let start_span = self.current_span();
        self.expect(TokenKind::Extend)?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the target type
        // Handle [T] for list types
        let (target_ty, target_type_name) = if self.check(TokenKind::LBracket) {
            self.advance(); // [
            // Parse element type (optional, default to T)
            if !self.check(TokenKind::RBracket) {
                let _ = self.parse_type_required()?;
            }
            self.expect(TokenKind::RBracket)?;
            // List type - method dispatch uses "list"
            (crate::ir::TypeId::INFER, self.interner().intern("list"))
        } else if self.check_type_keyword() {
            // Primitive type keywords: str, int, float, bool, etc.
            let type_name_str = match self.current_kind() {
                TokenKind::StrType => "str",
                TokenKind::IntType => "int",
                TokenKind::FloatType => "float",
                TokenKind::BoolType => "bool",
                TokenKind::CharType => "char",
                TokenKind::ByteType => "byte",
                _ => "unknown",
            };
            self.advance();
            (crate::ir::TypeId::INFER, self.interner().intern(type_name_str))
        } else {
            // Named type like Option<T>, MyType, etc.
            let type_name = self.expect_ident()?;
            // Check for generic parameters like Option<T>
            if self.check(TokenKind::Lt) {
                self.advance(); // <
                while !self.check(TokenKind::Gt) && !self.is_at_end() {
                    let _ = self.parse_type_required()?;
                    if self.check(TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.check(TokenKind::Gt) {
                    self.advance(); // >
                }
            }
            (crate::ir::TypeId::INFER, type_name)
        };

        // Optional where clause
        let where_clauses = if self.check(TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // Extend body: { methods }
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_impl_method() {
                Ok(method) => methods.push(method),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(TokenKind::RBrace)?;

        Ok(crate::ir::ExtendDef {
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

    /// Parse a type, accepting both primitives and named types.
    /// Returns TypeId for primitives, TypeId::INFER for named types (resolved later).
    ///
    /// Note: We check for named types (identifiers, Self) BEFORE calling parse_type()
    /// because parse_type() consumes identifiers and returns None for named types.
    fn parse_type_required(&mut self) -> Result<crate::ir::TypeId, ParseError> {
        // Handle `Self` type (refers to implementing type in traits)
        if self.check(TokenKind::SelfUpper) {
            self.advance();
            // Return INFER as placeholder - type checker will resolve Self
            return Ok(crate::ir::TypeId::INFER);
        }

        // Handle named types (identifiers) - must check before parse_type()
        // since parse_type() consumes identifiers and returns None
        if self.check_ident() {
            // Consume the identifier and any generic args
            self.advance();
            // Check for generic parameters like Option<T>
            if self.check(TokenKind::Lt) {
                self.advance(); // <
                while !self.check(TokenKind::Gt) && !self.is_at_end() {
                    let _ = self.parse_type_required()?;
                    if self.check(TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.check(TokenKind::Gt) {
                    self.advance(); // >
                }
            }
            // Return INFER as placeholder - type checker will resolve
            return Ok(crate::ir::TypeId::INFER);
        }

        // Try primitive types and other built-in constructs
        if let Some(ty) = self.parse_type() {
            return Ok(ty);
        }

        Err(ParseError::new(
            crate::diagnostic::ErrorCode::E1002,
            format!("expected type, found {:?}", self.current_kind()),
            self.current_span(),
        ))
    }

    /// Parse generic parameters: <T, U: Bound>
    pub(in crate::parser) fn parse_generics(&mut self) -> Result<GenericParamRange, ParseError> {
        self.expect(TokenKind::Lt)?;

        let mut params = Vec::new();
        while !self.check(TokenKind::Gt) && !self.is_at_end() {
            let param_span = self.current_span();
            let name = self.expect_ident()?;

            // Optional bounds: : Bound + OtherBound
            let bounds = if self.check(TokenKind::Colon) {
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

            if !self.check(TokenKind::Gt) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::Gt)?;
        Ok(self.arena.alloc_generic_params(params))
    }

    /// Parse trait bounds: Eq + Clone + Printable
    fn parse_bounds(&mut self) -> Result<Vec<TraitBound>, ParseError> {
        let mut bounds = Vec::new();

        loop {
            let bound_span = self.current_span();
            let path = self.parse_type_path()?;

            bounds.push(TraitBound {
                path,
                span: bound_span.merge(self.previous_span()),
            });

            if self.check(TokenKind::Plus) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(bounds)
    }

    /// Parse a type path: Name or std.collections.List
    fn parse_type_path(&mut self) -> Result<Vec<crate::ir::Name>, ParseError> {
        let mut segments = Vec::new();
        let name = self.expect_ident()?;
        segments.push(name);

        while self.check(TokenKind::Dot) {
            self.advance();
            let segment = self.expect_ident()?;
            segments.push(segment);
        }

        Ok(segments)
    }

    /// Convert a type path to a TypeId.
    /// For now, returns INFER as a placeholder - type resolution happens in the type checker.
    #[allow(dead_code)]
    fn make_type_from_path(&mut self, path: &[crate::ir::Name]) -> Result<crate::ir::TypeId, ParseError> {
        if path.is_empty() {
            return Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                "empty type path".to_string(),
                self.current_span(),
            ));
        }

        // TODO: Proper type path resolution in type checker
        // For now, use INFER as a placeholder - the type checker will resolve this
        Ok(crate::ir::TypeId::INFER)
    }

    /// Parse where clauses: where T: Clone, U: Default
    fn parse_where_clauses(&mut self) -> Result<Vec<WhereClause>, ParseError> {
        self.expect(TokenKind::Where)?;

        let mut clauses = Vec::new();
        loop {
            let clause_span = self.current_span();
            let param = self.expect_ident()?;

            self.expect(TokenKind::Colon)?;
            let bounds = self.parse_bounds()?;

            clauses.push(WhereClause {
                param,
                bounds,
                span: clause_span.merge(self.previous_span()),
            });

            if self.check(TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(clauses)
    }

    // =========================================================================
    // Type Declaration Parsing
    // =========================================================================

    /// Parse a type declaration.
    /// Syntax: [pub] [#[derive(...)]] type Name [<T>] [where ...] = body
    ///
    /// body can be:
    /// - struct: { field: Type, ... }
    /// - sum type: Variant1 | Variant2(field: Type)
    /// - newtype: ExistingType
    pub(in crate::parser) fn parse_type_decl(
        &mut self,
        derives: Vec<Name>,
        is_public: bool,
    ) -> Result<TypeDecl, ParseError> {
        let start_span = self.current_span();
        self.expect(TokenKind::Type)?;

        // Type name
        let name = self.expect_ident()?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Optional where clause
        let where_clauses = if self.check(TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // = body
        self.expect(TokenKind::Eq)?;

        // Parse the type body
        let kind = self.parse_type_body()?;
        let end_span = self.previous_span();

        Ok(TypeDecl {
            name,
            generics,
            where_clauses,
            kind,
            span: start_span.merge(end_span),
            is_public,
            derives,
        })
    }

    /// Parse the body of a type declaration.
    /// Returns the TypeDeclKind (struct, sum, or newtype).
    fn parse_type_body(&mut self) -> Result<TypeDeclKind, ParseError> {
        if self.check(TokenKind::LBrace) {
            // Struct type: { field: Type, ... }
            self.parse_struct_body()
        } else if self.check_ident() {
            // Could be a sum type (Variant | ...) or a newtype (ExistingType)
            self.parse_sum_or_newtype()
        } else {
            // Could also be a primitive type as a newtype
            self.parse_newtype_primitive()
        }
    }

    /// Parse a struct body: { field: Type, ... }
    fn parse_struct_body(&mut self) -> Result<TypeDeclKind, ParseError> {
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let field = self.parse_struct_field()?;
            fields.push(field);

            // Comma separator (optional before closing brace)
            if self.check(TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                self.skip_newlines();
                break;
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(TypeDeclKind::Struct(fields))
    }

    /// Parse a struct field: name: Type
    fn parse_struct_field(&mut self) -> Result<StructField, ParseError> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type_required()?;

        Ok(StructField {
            name,
            ty,
            span: start_span.merge(self.previous_span()),
        })
    }

    /// Parse a sum type or newtype starting with an identifier.
    /// Sum type: Variant1 | Variant2(field: Type)
    /// Newtype: ExistingType
    fn parse_sum_or_newtype(&mut self) -> Result<TypeDeclKind, ParseError> {
        let first_name = self.expect_ident()?;
        let first_span = self.previous_span();

        // Check for generic parameters (e.g., `Option<T>` as a newtype)
        let has_generics = self.check(TokenKind::Lt);
        if has_generics {
            // This is a newtype with generics: `type MyOption = Option<T>`
            self.advance(); // <
            while !self.check(TokenKind::Gt) && !self.is_at_end() {
                let _ = self.parse_type_required()?;
                if self.check(TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.check(TokenKind::Gt) {
                self.advance(); // >
            }
            // Return as newtype (TypeId::INFER placeholder)
            return Ok(TypeDeclKind::Newtype(TypeId::INFER));
        }

        // Check for variant fields (parentheses) or pipe (sum type)
        let first_fields = if self.check(TokenKind::LParen) {
            self.parse_variant_fields()?
        } else {
            Vec::new()
        };

        // Check if this is a sum type (has | separator)
        if self.check(TokenKind::Pipe) {
            // Sum type with multiple variants
            let mut variants = vec![Variant {
                name: first_name,
                fields: first_fields,
                span: first_span.merge(self.previous_span()),
            }];

            while self.check(TokenKind::Pipe) {
                self.advance();
                self.skip_newlines();

                let variant_span = self.current_span();
                let variant_name = self.expect_ident()?;
                let variant_fields = if self.check(TokenKind::LParen) {
                    self.parse_variant_fields()?
                } else {
                    Vec::new()
                };

                variants.push(Variant {
                    name: variant_name,
                    fields: variant_fields,
                    span: variant_span.merge(self.previous_span()),
                });
            }

            Ok(TypeDeclKind::Sum(variants))
        } else if first_fields.is_empty() {
            // Single identifier without fields or pipe - this is a newtype
            // e.g., `type UserId = int`
            Ok(TypeDeclKind::Newtype(TypeId::INFER))
        } else {
            // Single variant with fields - still a sum type
            // e.g., `type Wrapper = Value(inner: int)`
            Ok(TypeDeclKind::Sum(vec![Variant {
                name: first_name,
                fields: first_fields,
                span: first_span.merge(self.previous_span()),
            }]))
        }
    }

    /// Parse variant fields: (name: Type, ...)
    fn parse_variant_fields(&mut self) -> Result<Vec<VariantField>, ParseError> {
        self.expect(TokenKind::LParen)?;

        let mut fields = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            let field_span = self.current_span();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type_required()?;

            fields.push(VariantField {
                name,
                ty,
                span: field_span.merge(self.previous_span()),
            });

            // Comma separator
            if self.check(TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(TokenKind::RParen)?;
        Ok(fields)
    }

    /// Parse a newtype based on a primitive type.
    /// e.g., `type UserId = int`
    fn parse_newtype_primitive(&mut self) -> Result<TypeDeclKind, ParseError> {
        // Try to parse a primitive type
        if let Some(ty) = self.parse_type() {
            // Return as newtype with the actual type
            Ok(TypeDeclKind::Newtype(ty))
        } else {
            Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!(
                    "expected type body (struct, sum type, or type), found {:?}",
                    self.current_kind()
                ),
                self.current_span(),
            ))
        }
    }
}
