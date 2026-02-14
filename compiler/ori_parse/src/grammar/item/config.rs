//! Constant parsing.

use crate::{committed, require, ParseOutcome, Parser};
use ori_ir::{ConstDef, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse a constant declaration.
    ///
    /// Grammar: `constant_decl = "let" "$" identifier [ ":" type ] "=" const_expr`
    /// Syntax: `[pub] let $name = expr` or `[pub] let $name: type = expr`
    ///
    /// The initializer is parsed as a general expression. Constness validation
    /// (ensuring only const-compatible constructs) happens in later phases.
    ///
    /// Returns `EmptyErr` if no `$` is present.
    pub(crate) fn parse_const(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        if !self.cursor.check(&TokenKind::Dollar) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Dollar,
                self.cursor.current_span().start as usize,
            );
        }

        self.parse_const_body(visibility)
    }

    fn parse_const_body(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        let start_span = self.cursor.current_span();

        // $
        committed!(self.cursor.expect(&TokenKind::Dollar));

        // name
        let name = committed!(self.cursor.expect_ident());

        // Optional type annotation: `: type`
        let ty = if self.cursor.check(&TokenKind::Colon) {
            self.cursor.advance();
            self.parse_type()
        } else {
            None
        };

        // =
        committed!(self.cursor.expect(&TokenKind::Eq));

        // constant expression (parsed as general expression; constness validated later)
        let value = require!(self, self.parse_expr(), "constant expression");

        let span = start_span.merge(self.cursor.previous_span());

        ParseOutcome::consumed_ok(ConstDef {
            name,
            ty,
            value,
            span,
            visibility,
        })
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::StringInterner;

    fn parse_module(source: &str) -> crate::ParseOutput {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let parser = crate::Parser::new(&tokens, &interner);
        parser.parse_module()
    }

    #[test]
    fn test_const_without_type() {
        // Regression guard: let $PI = 3.14 (no type annotation)
        let output = parse_module("let $PI = 3.14");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_none());
    }

    #[test]
    fn test_const_with_type_int() {
        // Typed constant: let $MAX_SIZE: int = 1000
        let output = parse_module("let $MAX_SIZE: int = 1000");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    #[test]
    fn test_const_with_type_str() {
        // Typed string constant: let $NAME: str = "ori"
        let output = parse_module(r#"let $NAME: str = "ori""#);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    #[test]
    fn test_const_with_type_bool() {
        // Typed bool constant: let $DEBUG: bool = false
        let output = parse_module("let $DEBUG: bool = false");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    #[test]
    fn test_pub_const_with_type() {
        // Pub typed constant: pub let $MAX: int = 100
        let output = parse_module("pub let $MAX: int = 100");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    // ─── Computed constant expression tests ───

    #[test]
    fn test_const_arithmetic_add() {
        // Spec: const_expr = const_expr arith_op const_expr
        let output = parse_module("let $A = 10\nlet $D = $A + 1");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_arithmetic_multiply() {
        let output = parse_module("let $A = 10\nlet $E = $A * 2");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_comparison() {
        // Spec: const_expr = const_expr comp_op const_expr
        let output = parse_module("let $A = 10\nlet $F = $A > 0");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_logical() {
        // Spec: const_expr = const_expr logic_op const_expr
        let output = parse_module("let $A = true\nlet $B = false\nlet $G = $A && $B");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 3);
    }

    #[test]
    fn test_const_grouped() {
        // Spec: const_expr = "(" const_expr ")"
        let output = parse_module("let $A = 10\nlet $H = ($A + 1) * 2");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_unary_negation() {
        // Spec: const_expr = unary_op const_expr
        let output = parse_module("let $A = 10\nlet $NEG = -$A");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_reference_only() {
        // Simple reference to another constant
        let output = parse_module("let $A = 42\nlet $B = $A");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_string_concat() {
        // Spec: string concatenation with +
        let output = parse_module("let $PREFIX = \"hello\"\nlet $FULL = $PREFIX + \"_world\"");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    #[test]
    fn test_const_conditional() {
        // Spec: if/then/else in constant context
        let output = parse_module("let $DEBUG = true\nlet $TIMEOUT = if $DEBUG then 60 else 30");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 2);
    }

    // Regression guards: existing literal constants must keep working

    #[test]
    fn test_const_duration_literal() {
        let output = parse_module("let $TIMEOUT = 30s");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
    }

    #[test]
    fn test_const_size_literal() {
        let output = parse_module("let $BUFFER = 4kb");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
    }

    #[test]
    fn test_const_char_literal() {
        let output = parse_module("let $NEWLINE = '\\n'");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
    }
}
