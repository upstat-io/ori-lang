use super::*;
use ori_ir::{StringInterner, TokenList};

/// Owns the token list and interner so `Cursor` can borrow them
/// without `Box::leak`.
struct TestCtx {
    tokens: TokenList,
    interner: StringInterner,
}

impl TestCtx {
    fn new(source: &str) -> Self {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        Self { tokens, interner }
    }

    fn cursor(&self) -> Cursor<'_> {
        Cursor::new(&self.tokens, &self.interner)
    }
}

#[test]
fn test_token_set_empty() {
    let set = TokenSet::new();
    assert!(set.is_empty());
    assert_eq!(set.count(), 0);
    assert!(!set.contains(&TokenKind::At));
}

#[test]
fn test_token_set_single() {
    let set = TokenSet::single(TokenKind::At);
    assert!(!set.is_empty());
    assert_eq!(set.count(), 1);
    assert!(set.contains(&TokenKind::At));
    assert!(!set.contains(&TokenKind::Use));
}

#[test]
fn test_token_set_with() {
    let set = TokenSet::new()
        .with(TokenKind::At)
        .with(TokenKind::Use)
        .with(TokenKind::Let);

    assert_eq!(set.count(), 3);
    assert!(set.contains(&TokenKind::At));
    assert!(set.contains(&TokenKind::Use));
    assert!(set.contains(&TokenKind::Let));
    assert!(!set.contains(&TokenKind::Plus));
}

#[test]
fn test_token_set_union() {
    let set1 = TokenSet::new().with(TokenKind::At).with(TokenKind::Use);
    let set2 = TokenSet::new().with(TokenKind::Let).with(TokenKind::Use);

    let union = set1.union(set2);
    assert_eq!(union.count(), 3);
    assert!(union.contains(&TokenKind::At));
    assert!(union.contains(&TokenKind::Use));
    assert!(union.contains(&TokenKind::Let));
}

#[test]
fn test_token_set_intersection() {
    let set1 = TokenSet::new().with(TokenKind::At).with(TokenKind::Use);
    let set2 = TokenSet::new().with(TokenKind::Let).with(TokenKind::Use);

    let intersection = set1.intersection(set2);
    assert_eq!(intersection.count(), 1);
    assert!(!intersection.contains(&TokenKind::At));
    assert!(intersection.contains(&TokenKind::Use));
    assert!(!intersection.contains(&TokenKind::Let));
}

#[test]
fn test_token_set_data_variants() {
    // Data-carrying variants should work based on discriminant only
    let set = TokenSet::new()
        .with(TokenKind::Int(42))
        .with(TokenKind::Ident(ori_ir::Name::EMPTY));

    // Different values, same discriminant - should match
    assert!(set.contains(&TokenKind::Int(999)));
    assert!(set.contains(&TokenKind::Ident(ori_ir::Name::EMPTY)));
    assert!(!set.contains(&TokenKind::Float(0)));
}

#[test]
fn test_stmt_boundary_contains() {
    assert!(STMT_BOUNDARY.contains(&TokenKind::At));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Use));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Type));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Pub));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Dollar));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Extern));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Def));
    assert!(STMT_BOUNDARY.contains(&TokenKind::Extension));
    assert!(!STMT_BOUNDARY.contains(&TokenKind::Plus));
}

/// Completeness test: every token that can start a top-level declaration or import
/// must be present in `STMT_BOUNDARY`. This prevents drift when new declaration
/// kinds are added to `dispatch_declaration()` or `parse_imports()`.
#[test]
fn stmt_boundary_covers_all_declaration_starters() {
    // Every token checked at the top of dispatch_declaration() or parse_imports()
    // as a declaration/import start. Keep sorted by dispatch order.
    //
    // When adding a new declaration kind to the parser, add its leading token here.
    // The test will fail if STMT_BOUNDARY is not updated to match.
    let declaration_starters: &[(TokenKind, &str)] = &[
        // parse_imports()
        (TokenKind::Use, "import statement"),
        (TokenKind::Extension, "extension import"),
        // dispatch_declaration()
        (TokenKind::At, "function/test definition"),
        (TokenKind::Trait, "trait definition"),
        (TokenKind::Def, "default impl block (def impl)"),
        (TokenKind::Impl, "impl block"),
        (TokenKind::Extend, "extension"),
        (TokenKind::Type, "type declaration"),
        (TokenKind::Let, "module-level constant (let $name)"),
        (TokenKind::Dollar, "constant declaration ($name = value)"),
        (TokenKind::Extern, "extern block"),
        // Visibility modifier (can prefix any declaration)
        (TokenKind::Pub, "public declaration"),
        // Always present
        (TokenKind::Eof, "end of file"),
    ];

    for (kind, description) in declaration_starters {
        assert!(
            STMT_BOUNDARY.contains(kind),
            "STMT_BOUNDARY missing {kind:?} ({description}) — \
             add `.with(TokenKind::{kind:?})` to the STMT_BOUNDARY definition",
        );
    }
}

/// Reverse completeness: `STMT_BOUNDARY` must not contain stale tokens.
/// If a declaration kind is removed from the parser, its token must be removed
/// from `STMT_BOUNDARY` too.
#[test]
fn stmt_boundary_has_no_stale_entries() {
    // The complete set of tokens that should be in STMT_BOUNDARY.
    // If this test fails after removing a declaration kind, remove the token
    // from STMT_BOUNDARY in recovery/mod.rs.
    let expected = TokenSet::new()
        .with(TokenKind::At)
        .with(TokenKind::Use)
        .with(TokenKind::Type)
        .with(TokenKind::Trait)
        .with(TokenKind::Impl)
        .with(TokenKind::Def)
        .with(TokenKind::Pub)
        .with(TokenKind::Let)
        .with(TokenKind::Dollar)
        .with(TokenKind::Extend)
        .with(TokenKind::Extern)
        .with(TokenKind::Extension)
        .with(TokenKind::Eof);

    assert_eq!(
        STMT_BOUNDARY, expected,
        "STMT_BOUNDARY has entries not in the expected set (stale token?) \
         or is missing entries (drift?). Update both STMT_BOUNDARY and this test.",
    );
}

// Behavioral recovery tests: verify synchronize() actually stops at
// the declaration-start tokens that were added to STMT_BOUNDARY.

#[test]
fn synchronize_stops_at_dollar_constant() {
    let ctx = TestCtx::new("broken stuff $MAX = 100");
    let mut cursor = ctx.cursor();

    let found = synchronize(&mut cursor, STMT_BOUNDARY);
    assert!(found);
    assert!(cursor.check(&TokenKind::Dollar));
}

#[test]
fn synchronize_stops_at_extern_block() {
    let ctx = TestCtx::new("broken stuff extern \"c\" {}");
    let mut cursor = ctx.cursor();

    let found = synchronize(&mut cursor, STMT_BOUNDARY);
    assert!(found);
    assert!(cursor.check(&TokenKind::Extern));
}

#[test]
fn synchronize_stops_at_def_impl() {
    let ctx = TestCtx::new("broken stuff def impl Eq for Foo {}");
    let mut cursor = ctx.cursor();

    let found = synchronize(&mut cursor, STMT_BOUNDARY);
    assert!(found);
    assert!(cursor.check(&TokenKind::Def));
}

#[test]
fn synchronize_stops_at_extension_import() {
    let ctx = TestCtx::new("broken stuff extension my_ext");
    let mut cursor = ctx.cursor();

    let found = synchronize(&mut cursor, STMT_BOUNDARY);
    assert!(found);
    assert!(cursor.check(&TokenKind::Extension));
}

#[test]
fn test_expr_follow_contains() {
    assert!(EXPR_FOLLOW.contains(&TokenKind::RParen));
    assert!(EXPR_FOLLOW.contains(&TokenKind::Comma));
    assert!(!EXPR_FOLLOW.contains(&TokenKind::Plus));
}

#[test]
fn test_synchronize_to_function() {
    let ctx = TestCtx::new("let x = broken + @next_func () -> int = 42");
    let mut cursor = ctx.cursor();

    // Start parsing, encounter error, need to sync
    cursor.advance(); // let
    cursor.advance(); // x
    cursor.advance(); // =
    cursor.advance(); // broken
    cursor.advance(); // +

    // Synchronize to next function
    let found = synchronize(&mut cursor, FUNCTION_BOUNDARY);
    assert!(found);
    assert!(cursor.check(&TokenKind::At));
}

#[test]
fn test_synchronize_to_expr_follow() {
    let ctx = TestCtx::new("(broken + , next)");
    let mut cursor = ctx.cursor();

    cursor.advance(); // (
    cursor.advance(); // broken
    cursor.advance(); // +

    // Synchronize to expression follow
    let found = synchronize(&mut cursor, EXPR_FOLLOW);
    assert!(found);
    assert!(cursor.check(&TokenKind::Comma));
}

#[test]
fn test_synchronize_eof() {
    let ctx = TestCtx::new("let x = 42");
    let mut cursor = ctx.cursor();

    // Try to sync to non-existent token
    let found = synchronize(&mut cursor, FUNCTION_BOUNDARY);
    assert!(!found);
    assert!(cursor.is_at_end());
}

#[test]
fn test_synchronize_counted() {
    let ctx = TestCtx::new("a b c @func");
    let mut cursor = ctx.cursor();

    let result = synchronize_counted(&mut cursor, FUNCTION_BOUNDARY);
    assert_eq!(result, Some(3)); // Skipped: a, b, c
    assert!(cursor.check(&TokenKind::At));
}

#[test]
fn test_const_token_sets() {
    // Verify const token sets are computed at compile time
    const TEST_SET: TokenSet = TokenSet::new().with(TokenKind::Plus).with(TokenKind::Minus);

    assert!(TEST_SET.contains(&TokenKind::Plus));
    assert!(TEST_SET.contains(&TokenKind::Minus));
    assert!(!TEST_SET.contains(&TokenKind::Star));
}

#[test]
fn test_token_set_iterator() {
    let set = TokenSet::new()
        .with(TokenKind::Plus)
        .with(TokenKind::Minus)
        .with(TokenKind::Star);

    let indices: Vec<u8> = set.iter_indices().collect();
    assert_eq!(indices.len(), 3);

    // Verify all expected indices are present
    assert!(indices.contains(&TokenKind::Plus.discriminant_index()));
    assert!(indices.contains(&TokenKind::Minus.discriminant_index()));
    assert!(indices.contains(&TokenKind::Star.discriminant_index()));
}

#[test]
fn test_token_set_iterator_empty() {
    let set = TokenSet::new();
    let indices: Vec<u8> = set.iter_indices().collect();
    assert!(indices.is_empty());
}

#[test]
fn test_token_set_iterator_exact_size() {
    let set = TokenSet::new()
        .with(TokenKind::LParen)
        .with(TokenKind::RParen)
        .with(TokenKind::Comma);

    let iter = set.iter_indices();
    assert_eq!(iter.len(), 3);
}

#[test]
fn test_token_set_insert() {
    let mut set = TokenSet::new();
    assert!(set.is_empty());

    set.insert(&TokenKind::Plus);
    assert!(set.contains(&TokenKind::Plus));
    assert_eq!(set.count(), 1);

    set.insert(&TokenKind::Minus);
    assert!(set.contains(&TokenKind::Minus));
    assert_eq!(set.count(), 2);

    // Inserting duplicate doesn't change count
    set.insert(&TokenKind::Plus);
    assert_eq!(set.count(), 2);
}

#[test]
fn test_token_set_union_with() {
    let mut set1 = TokenSet::new().with(TokenKind::Plus);
    let set2 = TokenSet::new().with(TokenKind::Minus).with(TokenKind::Star);

    set1.union_with(&set2);
    assert_eq!(set1.count(), 3);
    assert!(set1.contains(&TokenKind::Plus));
    assert!(set1.contains(&TokenKind::Minus));
    assert!(set1.contains(&TokenKind::Star));
}

#[test]
fn test_format_expected_empty() {
    let set = TokenSet::new();
    assert_eq!(set.format_expected(), "nothing");
}

#[test]
fn test_format_expected_single() {
    let set = TokenSet::new().with(TokenKind::LParen);
    assert_eq!(set.format_expected(), "`(`");
}

#[test]
fn test_format_expected_two() {
    let set = TokenSet::new()
        .with(TokenKind::LParen)
        .with(TokenKind::LBracket);
    // Order depends on discriminant indices
    let result = set.format_expected();
    assert!(result.contains("or"));
    assert!(result.contains("`(`"));
    assert!(result.contains("`[`"));
}

#[test]
fn test_format_expected_multiple() {
    let set = TokenSet::new()
        .with(TokenKind::Comma)
        .with(TokenKind::RParen)
        .with(TokenKind::RBrace);
    let result = set.format_expected();
    // Should have "or" before the last item
    assert!(result.contains(", or `"));
    assert!(result.contains("`,`"));
    assert!(result.contains("`)`"));
    assert!(result.contains("`}`"));
}
