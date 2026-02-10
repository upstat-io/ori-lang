//! Extend block parsing.

use crate::{committed, ParseOutcome, Parser};
use ori_ir::{
    ExtendDef, GenericParamRange, ParsedType, ParsedTypeId, ParsedTypeRange, TokenKind, TypeId,
};

impl Parser<'_> {
    /// Parse an extend block.
    ///
    /// Syntax: extend [<T>] Type { methods }
    ///
    /// Returns `EmptyErr` if no `extend` keyword is present.
    ///
    /// Examples:
    ///   extend [T] { @map... }           - extends all lists
    ///   extend<T> Option<T> { @map... }  - extends Option
    ///   extend str { @reverse... }       - extends str
    pub(crate) fn parse_extend(&mut self) -> ParseOutcome<ExtendDef> {
        if !self.cursor.check(&TokenKind::Extend) {
            return ParseOutcome::empty_err_expected(&TokenKind::Extend, self.cursor.position());
        }

        self.parse_extend_body()
    }

    fn parse_extend_body(&mut self) -> ParseOutcome<ExtendDef> {
        let start_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Extend));

        // Optional generics: <T, U: Bound>
        let generics = if self.cursor.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the target type
        // Handle [T] for list types
        let (target_ty, target_type_name) = if self.cursor.check(&TokenKind::LBracket) {
            self.cursor.advance(); // [
                                   // Parse element type (optional, default to infer)
            let elem_ty = if self.cursor.check(&TokenKind::RBracket) {
                ParsedType::Infer
            } else {
                committed!(self.parse_type_required().into_result())
            };
            committed!(self.cursor.expect(&TokenKind::RBracket));
            // List type - method dispatch uses "list"
            let elem_id = self.arena.alloc_parsed_type(elem_ty);
            (
                ParsedType::List(elem_id),
                self.cursor.interner().intern("list"),
            )
        } else if self.cursor.check_type_keyword() {
            // Primitive type keywords: str, int, float, bool, etc.
            let (ty, type_name_str) = match self.cursor.current_kind() {
                TokenKind::StrType => (ParsedType::Primitive(TypeId::STR), "str"),
                TokenKind::IntType => (ParsedType::Primitive(TypeId::INT), "int"),
                TokenKind::FloatType => (ParsedType::Primitive(TypeId::FLOAT), "float"),
                TokenKind::BoolType => (ParsedType::Primitive(TypeId::BOOL), "bool"),
                TokenKind::CharType => (ParsedType::Primitive(TypeId::CHAR), "char"),
                TokenKind::ByteType => (ParsedType::Primitive(TypeId::BYTE), "byte"),
                _ => (ParsedType::Infer, "unknown"),
            };
            self.cursor.advance();
            (ty, self.cursor.interner().intern(type_name_str))
        } else {
            // Named type like Option<T>, MyType, etc.
            let type_name = committed!(self.cursor.expect_ident());
            // Check for generic parameters like Option<T>
            let type_args = if self.cursor.check(&TokenKind::Lt) {
                self.cursor.advance(); // <
                                       // Type arg lists use a Vec because nested generic args share the
                                       // same `parsed_type_lists` buffer (e.g., `extend Option<List<T>>`).
                let mut type_arg_list: Vec<ParsedTypeId> = Vec::new();
                while !self.cursor.check(&TokenKind::Gt) && !self.cursor.is_at_end() {
                    let ty = committed!(self.parse_type_required().into_result());
                    type_arg_list.push(self.arena.alloc_parsed_type(ty));
                    if self.cursor.check(&TokenKind::Comma) {
                        self.cursor.advance();
                    } else {
                        break;
                    }
                }
                if self.cursor.check(&TokenKind::Gt) {
                    self.cursor.advance(); // >
                }
                self.arena.alloc_parsed_type_list(type_arg_list)
            } else {
                ParsedTypeRange::EMPTY
            };
            (
                ParsedType::Named {
                    name: type_name,
                    type_args,
                },
                type_name,
            )
        };

        // Optional where clause
        let where_clauses = if self.cursor.check(&TokenKind::Where) {
            committed!(self.parse_where_clauses().into_result())
        } else {
            Vec::new()
        };

        // Extend body: { methods }
        committed!(self.cursor.expect(&TokenKind::LBrace));
        self.cursor.skip_newlines();

        let mut methods = Vec::new();
        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            let method = committed!(self.parse_impl_method());
            methods.push(method);
            self.cursor.skip_newlines();
        }

        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(ExtendDef {
            generics,
            target_ty,
            target_type_name,
            where_clauses,
            methods,
            span: start_span.merge(end_span),
        })
    }
}
