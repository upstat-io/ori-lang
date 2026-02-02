//! Tests for token spacing rules.

use super::*;

mod action_tests {
    use super::*;

    #[test]
    fn space_action_default() {
        assert_eq!(SpaceAction::default(), SpaceAction::None);
    }

    #[test]
    fn space_action_needs_space() {
        assert!(!SpaceAction::None.needs_space());
        assert!(SpaceAction::Space.needs_space());
        assert!(!SpaceAction::Newline.needs_space());
        assert!(!SpaceAction::Preserve.needs_space());
    }

    #[test]
    fn space_action_needs_newline() {
        assert!(!SpaceAction::None.needs_newline());
        assert!(!SpaceAction::Space.needs_newline());
        assert!(SpaceAction::Newline.needs_newline());
        assert!(!SpaceAction::Preserve.needs_newline());
    }

    #[test]
    fn space_action_preserves() {
        assert!(!SpaceAction::None.preserves());
        assert!(!SpaceAction::Space.preserves());
        assert!(!SpaceAction::Newline.preserves());
        assert!(SpaceAction::Preserve.preserves());
    }
}

mod category_tests {
    use super::*;
    use ori_ir::TokenKind;

    #[test]
    fn token_kind_to_category() {
        assert_eq!(TokenCategory::from(&TokenKind::Int(42)), TokenCategory::Int);
        assert_eq!(
            TokenCategory::from(&TokenKind::Float(1.5f64.to_bits())),
            TokenCategory::Float
        );
        assert_eq!(TokenCategory::from(&TokenKind::Plus), TokenCategory::Plus);
        assert_eq!(TokenCategory::from(&TokenKind::Minus), TokenCategory::Minus);
        assert_eq!(
            TokenCategory::from(&TokenKind::LParen),
            TokenCategory::LParen
        );
        assert_eq!(
            TokenCategory::from(&TokenKind::RParen),
            TokenCategory::RParen
        );
    }

    #[test]
    fn category_is_binary_op() {
        assert!(TokenCategory::Plus.is_binary_op());
        assert!(TokenCategory::Minus.is_binary_op());
        assert!(TokenCategory::Star.is_binary_op());
        assert!(TokenCategory::EqEq.is_binary_op());
        assert!(TokenCategory::AmpAmp.is_binary_op());
        assert!(TokenCategory::PipePipe.is_binary_op());

        assert!(!TokenCategory::Ident.is_binary_op());
        assert!(!TokenCategory::LParen.is_binary_op());
        assert!(!TokenCategory::Dot.is_binary_op());
    }

    #[test]
    fn category_is_unary_op() {
        assert!(TokenCategory::Minus.is_unary_op());
        assert!(TokenCategory::Bang.is_unary_op());
        assert!(TokenCategory::Tilde.is_unary_op());

        assert!(!TokenCategory::Plus.is_unary_op());
        assert!(!TokenCategory::Star.is_unary_op());
    }

    #[test]
    fn category_is_open_delim() {
        assert!(TokenCategory::LParen.is_open_delim());
        assert!(TokenCategory::LBrace.is_open_delim());
        assert!(TokenCategory::LBracket.is_open_delim());

        assert!(!TokenCategory::RParen.is_open_delim());
        assert!(!TokenCategory::Ident.is_open_delim());
    }

    #[test]
    fn category_is_close_delim() {
        assert!(TokenCategory::RParen.is_close_delim());
        assert!(TokenCategory::RBrace.is_close_delim());
        assert!(TokenCategory::RBracket.is_close_delim());

        assert!(!TokenCategory::LParen.is_close_delim());
        assert!(!TokenCategory::Ident.is_close_delim());
    }

    #[test]
    fn category_is_literal() {
        assert!(TokenCategory::Int.is_literal());
        assert!(TokenCategory::Float.is_literal());
        assert!(TokenCategory::String.is_literal());
        assert!(TokenCategory::Char.is_literal());
        assert!(TokenCategory::True.is_literal());
        assert!(TokenCategory::False.is_literal());

        assert!(!TokenCategory::Ident.is_literal());
        assert!(!TokenCategory::Plus.is_literal());
    }

    #[test]
    fn category_is_keyword() {
        assert!(TokenCategory::If.is_keyword());
        assert!(TokenCategory::Then.is_keyword());
        assert!(TokenCategory::Else.is_keyword());
        assert!(TokenCategory::For.is_keyword());
        assert!(TokenCategory::Let.is_keyword());
        assert!(TokenCategory::Pub.is_keyword());

        assert!(!TokenCategory::Ident.is_keyword());
        assert!(!TokenCategory::Int.is_keyword());
    }
}

mod matcher_tests {
    use super::*;

    #[test]
    fn matcher_any() {
        let m = TokenMatcher::Any;
        assert!(m.matches(TokenCategory::Int));
        assert!(m.matches(TokenCategory::Plus));
        assert!(m.matches(TokenCategory::LParen));
        assert!(m.matches(TokenCategory::Ident));
    }

    #[test]
    fn matcher_exact() {
        let m = TokenMatcher::Exact(TokenCategory::Plus);
        assert!(m.matches(TokenCategory::Plus));
        assert!(!m.matches(TokenCategory::Minus));
        assert!(!m.matches(TokenCategory::Ident));
    }

    #[test]
    fn matcher_one_of() {
        static ARITH: &[TokenCategory] = &[
            TokenCategory::Plus,
            TokenCategory::Minus,
            TokenCategory::Star,
        ];
        let m = TokenMatcher::OneOf(ARITH);

        assert!(m.matches(TokenCategory::Plus));
        assert!(m.matches(TokenCategory::Minus));
        assert!(m.matches(TokenCategory::Star));
        assert!(!m.matches(TokenCategory::Slash));
        assert!(!m.matches(TokenCategory::Ident));
    }

    #[test]
    fn matcher_category_predicate() {
        let m = TokenMatcher::BINARY_OP;

        assert!(m.matches(TokenCategory::Plus));
        assert!(m.matches(TokenCategory::EqEq));
        assert!(m.matches(TokenCategory::AmpAmp));
        assert!(!m.matches(TokenCategory::Ident));
        assert!(!m.matches(TokenCategory::LParen));
    }

    #[test]
    fn matcher_equality() {
        assert_eq!(TokenMatcher::Any, TokenMatcher::Any);
        assert_eq!(
            TokenMatcher::Exact(TokenCategory::Plus),
            TokenMatcher::Exact(TokenCategory::Plus)
        );
        assert_ne!(
            TokenMatcher::Exact(TokenCategory::Plus),
            TokenMatcher::Exact(TokenCategory::Minus)
        );
        assert_ne!(TokenMatcher::Any, TokenMatcher::Exact(TokenCategory::Plus));
    }
}

mod rules_tests {
    use super::*;

    #[test]
    fn rule_count_reasonable() {
        // Should have a reasonable number of rules
        let count = rules::rule_count();
        assert!(count >= 30, "Expected at least 30 rules, got {count}");
        assert!(count <= 100, "Expected at most 100 rules, got {count}");
    }

    // === Binary Operator Rules (Spec lines 25-30) ===

    #[test]
    fn space_around_plus() {
        // a + b
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Plus),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Plus, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_around_comparison() {
        // x == y, x != y
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::EqEq),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::EqEq, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_around_logical() {
        // a && b, x || y
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::AmpAmp),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::AmpAmp, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_around_assignment() {
        // x = 1
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Eq),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Eq, TokenCategory::Int),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_around_arrow() {
        // (x) -> y
        assert_eq!(
            rules::spacing_between(TokenCategory::RParen, TokenCategory::Arrow),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Arrow, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    // === Delimiter Rules (Spec lines 31-35) ===

    #[test]
    fn no_space_empty_parens() {
        // ()
        assert_eq!(
            rules::spacing_between(TokenCategory::LParen, TokenCategory::RParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_empty_brackets() {
        // []
        assert_eq!(
            rules::spacing_between(TokenCategory::LBracket, TokenCategory::RBracket),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_empty_braces() {
        // {}
        assert_eq!(
            rules::spacing_between(TokenCategory::LBrace, TokenCategory::RBrace),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_after_lparen() {
        // (x
        assert_eq!(
            rules::spacing_between(TokenCategory::LParen, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_before_rparen() {
        // x)
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::RParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_after_lbracket() {
        // [x
        assert_eq!(
            rules::spacing_between(TokenCategory::LBracket, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_before_rbracket() {
        // x]
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::RBracket),
            SpaceAction::None
        );
    }

    // === Punctuation Rules (Spec lines 36-41) ===

    #[test]
    fn comma_spacing() {
        // a, b -> space after, no space before
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Comma),
            SpaceAction::None
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Comma, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn colon_spacing() {
        // x: int -> space after, no space before
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Colon),
            SpaceAction::None
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Colon, TokenCategory::IntType),
            SpaceAction::Space
        );
    }

    #[test]
    fn no_space_around_dot() {
        // x.y
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Dot),
            SpaceAction::None
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Dot, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_around_range() {
        // 0..10
        assert_eq!(
            rules::spacing_between(TokenCategory::Int, TokenCategory::DotDot),
            SpaceAction::None
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::DotDot, TokenCategory::Int),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_before_question() {
        // x?
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Question),
            SpaceAction::None
        );
    }

    // === Keyword Rules (Spec lines 42-47) ===

    #[test]
    fn space_after_pub() {
        // pub @foo
        assert_eq!(
            rules::spacing_between(TokenCategory::Pub, TokenCategory::At),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_after_let() {
        // let x
        assert_eq!(
            rules::spacing_between(TokenCategory::Let, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_after_if() {
        // if condition
        assert_eq!(
            rules::spacing_between(TokenCategory::If, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_after_for() {
        // for x
        assert_eq!(
            rules::spacing_between(TokenCategory::For, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_around_in() {
        // x in items
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::In),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::In, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    #[test]
    fn space_around_as() {
        // x as int
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::As),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::As, TokenCategory::IntType),
            SpaceAction::Space
        );
    }

    // === Pattern Keyword Rules ===

    #[test]
    fn no_space_run_paren() {
        // run(
        assert_eq!(
            rules::spacing_between(TokenCategory::Run, TokenCategory::LParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_try_paren() {
        // try(
        assert_eq!(
            rules::spacing_between(TokenCategory::Try, TokenCategory::LParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_match_paren() {
        // match(
        assert_eq!(
            rules::spacing_between(TokenCategory::Match, TokenCategory::LParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_ok_paren() {
        // Ok(
        assert_eq!(
            rules::spacing_between(TokenCategory::Ok, TokenCategory::LParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_err_paren() {
        // Err(
        assert_eq!(
            rules::spacing_between(TokenCategory::Err, TokenCategory::LParen),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_loop_paren() {
        // loop(
        assert_eq!(
            rules::spacing_between(TokenCategory::Loop, TokenCategory::LParen),
            SpaceAction::None
        );
    }

    // === Special Rules ===

    #[test]
    fn no_space_at_ident() {
        // @foo
        assert_eq!(
            rules::spacing_between(TokenCategory::At, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_dollar_ident() {
        // $FOO
        assert_eq!(
            rules::spacing_between(TokenCategory::Dollar, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_double_colon() {
        // Module::item
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::DoubleColon),
            SpaceAction::None
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::DoubleColon, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn space_around_pipe_sum_type() {
        // A | B
        assert_eq!(
            rules::spacing_between(TokenCategory::Ident, TokenCategory::Pipe),
            SpaceAction::Space
        );
        assert_eq!(
            rules::spacing_between(TokenCategory::Pipe, TokenCategory::Ident),
            SpaceAction::Space
        );
    }

    // === Unary Operator Rules ===

    #[test]
    fn no_space_after_bang() {
        // !x
        assert_eq!(
            rules::spacing_between(TokenCategory::Bang, TokenCategory::Ident),
            SpaceAction::None
        );
    }

    #[test]
    fn no_space_after_tilde() {
        // ~x
        assert_eq!(
            rules::spacing_between(TokenCategory::Tilde, TokenCategory::Ident),
            SpaceAction::None
        );
    }
}

mod lookup_tests {
    use super::*;

    #[test]
    fn rules_map_creation() {
        let map = RulesMap::new();
        // Should have some exact entries
        assert!(
            map.exact_entry_count() > 0,
            "Expected some exact entries, got 0"
        );
        // Should have some fallback rules (Any matchers)
        assert!(
            map.fallback_rule_count() > 0,
            "Expected some fallback rules, got 0"
        );
    }

    #[test]
    fn global_rules_map_consistent() {
        let map1 = global_rules_map();
        let map2 = global_rules_map();
        // Should be the same instance
        assert!(std::ptr::eq(map1, map2));
    }

    #[test]
    fn lookup_matches_direct_rules() {
        let map = RulesMap::new();

        // Test a few lookups match direct rule search
        let cases = [
            (TokenCategory::LParen, TokenCategory::RParen),
            (TokenCategory::Ident, TokenCategory::Plus),
            (TokenCategory::Plus, TokenCategory::Ident),
            (TokenCategory::Run, TokenCategory::LParen),
            (TokenCategory::Comma, TokenCategory::Ident),
        ];

        for (left, right) in cases {
            let map_result = map.lookup(left, right);
            let direct_result = rules::spacing_between(left, right);
            assert_eq!(
                map_result, direct_result,
                "Mismatch for {left:?} {right:?}: map={map_result:?}, direct={direct_result:?}"
            );
        }
    }

    #[test]
    fn lookup_api() {
        // Test the public API
        let action = lookup_spacing(TokenCategory::Ident, TokenCategory::Plus);
        assert_eq!(action, SpaceAction::Space);

        let action = lookup_spacing(TokenCategory::LParen, TokenCategory::Ident);
        assert_eq!(action, SpaceAction::None);
    }
}
