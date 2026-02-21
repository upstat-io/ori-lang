use super::*;

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive TokenKind variant enumeration"
)]
fn test_discriminant_index_uniqueness() {
    // Verify all discriminant indices are unique and within range
    let mut seen = [false; 128];

    // Test representative tokens from each category
    let tokens = [
        TokenKind::Int(0),
        TokenKind::Float(0),
        TokenKind::String(crate::Name::EMPTY),
        TokenKind::Char('a'),
        TokenKind::Duration(0, DurationUnit::Seconds),
        TokenKind::Size(0, SizeUnit::Bytes),
        TokenKind::Ident(crate::Name::EMPTY),
        TokenKind::Async,
        TokenKind::Break,
        TokenKind::Continue,
        TokenKind::Return,
        TokenKind::Def,
        TokenKind::Do,
        TokenKind::Else,
        TokenKind::False,
        TokenKind::For,
        TokenKind::If,
        TokenKind::Impl,
        TokenKind::In,
        TokenKind::Let,
        TokenKind::Loop,
        TokenKind::Match,
        TokenKind::Pub,
        TokenKind::SelfLower,
        TokenKind::SelfUpper,
        TokenKind::Then,
        TokenKind::Trait,
        TokenKind::True,
        TokenKind::Type,
        TokenKind::Use,
        TokenKind::Uses,
        TokenKind::Void,
        TokenKind::Where,
        TokenKind::With,
        TokenKind::Yield,
        TokenKind::Suspend,
        TokenKind::Unsafe,
        TokenKind::Tests,
        TokenKind::As,
        TokenKind::Dyn,
        TokenKind::Extend,
        TokenKind::Extension,
        TokenKind::Skip,
        TokenKind::Extern,
        TokenKind::IntType,
        TokenKind::FloatType,
        TokenKind::BoolType,
        TokenKind::StrType,
        TokenKind::CharType,
        TokenKind::ByteType,
        TokenKind::NeverType,
        TokenKind::Ok,
        TokenKind::Err,
        TokenKind::Some,
        TokenKind::None,
        TokenKind::Cache,
        TokenKind::Catch,
        TokenKind::Parallel,
        TokenKind::Spawn,
        TokenKind::Recurse,
        TokenKind::Run,
        TokenKind::Timeout,
        TokenKind::Try,
        TokenKind::By,
        TokenKind::Print,
        TokenKind::Panic,
        TokenKind::Todo,
        TokenKind::Unreachable,
        TokenKind::HashBracket,
        TokenKind::HashBang,
        TokenKind::At,
        TokenKind::Dollar,
        TokenKind::Hash,
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::LBracket,
        TokenKind::RBracket,
        TokenKind::Colon,
        TokenKind::DoubleColon,
        TokenKind::Comma,
        TokenKind::Dot,
        TokenKind::DotDot,
        TokenKind::DotDotEq,
        TokenKind::DotDotDot,
        TokenKind::Arrow,
        TokenKind::FatArrow,
        TokenKind::Pipe,
        TokenKind::Question,
        TokenKind::DoubleQuestion,
        TokenKind::Underscore,
        TokenKind::Semicolon,
        TokenKind::Eq,
        TokenKind::EqEq,
        TokenKind::NotEq,
        TokenKind::Lt,
        TokenKind::LtEq,
        TokenKind::Shl,
        TokenKind::Gt,
        TokenKind::GtEq,
        TokenKind::Shr,
        TokenKind::Plus,
        TokenKind::Minus,
        TokenKind::Star,
        TokenKind::Slash,
        TokenKind::Percent,
        TokenKind::Bang,
        TokenKind::Tilde,
        TokenKind::Amp,
        TokenKind::AmpAmp,
        TokenKind::PipePipe,
        TokenKind::Caret,
        TokenKind::Div,
        TokenKind::Newline,
        TokenKind::Eof,
        TokenKind::Error,
        TokenKind::TemplateHead(crate::Name::EMPTY),
        TokenKind::TemplateMiddle(crate::Name::EMPTY),
        TokenKind::TemplateTail(crate::Name::EMPTY),
        TokenKind::TemplateFull(crate::Name::EMPTY),
        TokenKind::FormatSpec(crate::Name::EMPTY),
    ];

    assert_eq!(
        tokens.len(),
        TOKEN_KIND_COUNT,
        "Test should cover all {TOKEN_KIND_COUNT} token kinds",
    );

    for token in &tokens {
        let idx = token.discriminant_index() as usize;
        assert!(
            idx < 128,
            "Discriminant index {idx} out of range for {token:?}",
        );
        assert!(
            !seen[idx],
            "Duplicate discriminant index {idx} for {token:?}",
        );
        seen[idx] = true;
    }
}

#[test]
fn test_token_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let t1 = Token::new(TokenKind::Int(42), Span::new(0, 2));
    let t2 = Token::new(TokenKind::Int(42), Span::new(0, 2)); // same
    let t3 = Token::new(TokenKind::Int(43), Span::new(0, 2)); // different

    set.insert(t1);
    set.insert(t2);
    set.insert(t3);

    assert_eq!(set.len(), 2);
}

#[test]
fn test_token_kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    set.insert(TokenKind::Int(1));
    set.insert(TokenKind::Int(1)); // duplicate
    set.insert(TokenKind::Int(2));
    set.insert(TokenKind::Plus);
    set.insert(TokenKind::Plus); // duplicate

    assert_eq!(set.len(), 3);
}

#[test]
fn test_token_list_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let list1 = TokenList::from_vec(vec![
        Token::new(TokenKind::Int(1), Span::new(0, 1)),
        Token::new(TokenKind::Plus, Span::new(2, 3)),
    ]);
    let list2 = TokenList::from_vec(vec![
        Token::new(TokenKind::Int(1), Span::new(0, 1)),
        Token::new(TokenKind::Plus, Span::new(2, 3)),
    ]);
    let list3 = TokenList::from_vec(vec![Token::new(TokenKind::Int(2), Span::new(0, 1))]);

    set.insert(list1);
    set.insert(list2); // same as list1
    set.insert(list3);

    assert_eq!(set.len(), 2);
}

#[test]
#[expect(
    clippy::approx_constant,
    reason = "testing float literal parsing, not using PI"
)]
fn test_float_bits_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let bits1 = 3.14f64.to_bits();
    let bits2 = 3.14f64.to_bits();
    let bits3 = 2.71f64.to_bits();

    set.insert(TokenKind::Float(bits1));
    set.insert(TokenKind::Float(bits2)); // same
    set.insert(TokenKind::Float(bits3));

    assert_eq!(set.len(), 2);
}

#[test]
fn test_duration_unit() {
    assert_eq!(DurationUnit::Nanoseconds.to_nanos(100), 100);
    assert_eq!(DurationUnit::Microseconds.to_nanos(50), 50_000);
    assert_eq!(DurationUnit::Milliseconds.to_nanos(100), 100_000_000);
    assert_eq!(DurationUnit::Seconds.to_nanos(5), 5_000_000_000);
    assert_eq!(DurationUnit::Minutes.to_nanos(1), 60_000_000_000);
    assert_eq!(DurationUnit::Hours.suffix(), "h");
    assert_eq!(DurationUnit::Nanoseconds.suffix(), "ns");
    assert_eq!(DurationUnit::Microseconds.suffix(), "us");
}

#[test]
fn test_size_unit() {
    // SI units: decimal notation (1kb = 1000 bytes)
    assert_eq!(SizeUnit::Kilobytes.to_bytes(4), 4_000);
    assert_eq!(SizeUnit::Megabytes.to_bytes(1), 1_000_000);
    assert_eq!(SizeUnit::Gigabytes.to_bytes(1), 1_000_000_000);
    assert_eq!(SizeUnit::Terabytes.to_bytes(1), 1_000_000_000_000);
    assert_eq!(SizeUnit::Bytes.suffix(), "b");
    assert_eq!(SizeUnit::Terabytes.suffix(), "tb");
}

#[test]
fn test_token_list_operations() {
    let mut list = TokenList::new();
    assert!(list.is_empty());

    list.push(Token::new(TokenKind::Int(1), Span::new(0, 1)));
    list.push(Token::new(TokenKind::Plus, Span::new(2, 3)));

    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
    assert_eq!(list[0].kind, TokenKind::Int(1));
    assert_eq!(list.get(1).unwrap().kind, TokenKind::Plus);
}

#[test]
fn test_friendly_name_from_index() {
    // Test literals (use TokenTag values)
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Ident as u8),
        Some("identifier")
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Int as u8),
        Some("integer")
    );

    // Test keywords
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::KwIf as u8),
        Some("if")
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::KwLet as u8),
        Some("let")
    );

    // Test punctuation
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::LParen as u8),
        Some("(")
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::RParen as u8),
        Some(")")
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Comma as u8),
        Some(",")
    );

    // Test operators
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Plus as u8),
        Some("+")
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Minus as u8),
        Some("-")
    );

    // Test internal tokens (should return None)
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Newline as u8),
        None
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Eof as u8),
        None
    );
    assert_eq!(
        TokenKind::friendly_name_from_index(TokenTag::Error as u8),
        None
    );

    // Test non-gap indices in the 74-75 range
    assert_eq!(TokenKind::friendly_name_from_index(74), Some("format spec"));
    assert_eq!(TokenKind::friendly_name_from_index(75), Some("#!"));

    // Test out of range
    assert_eq!(TokenKind::friendly_name_from_index(200), None);
}

#[test]
fn test_friendly_name_matches_discriminant() {
    // Verify that friendly_name_from_index returns correct names
    // for the corresponding discriminant indices
    let test_cases = [
        (TokenKind::Int(42), "integer"),
        (TokenKind::Ident(crate::Name::EMPTY), "identifier"),
        (TokenKind::If, "if"),
        (TokenKind::Let, "let"),
        (TokenKind::Plus, "+"),
        (TokenKind::LParen, "("),
        (TokenKind::Comma, ","),
    ];

    for (token, expected_name) in test_cases {
        let index = token.discriminant_index();
        let friendly = TokenKind::friendly_name_from_index(index);
        assert_eq!(
            friendly,
            Some(expected_name),
            "Mismatch for {token:?} at index {index}"
        );
    }
}

// TokenCapture tests
#[test]
fn test_token_capture_none() {
    let capture = TokenCapture::None;
    assert!(capture.is_empty());
    assert_eq!(capture.len(), 0);

    let list = TokenList::new();
    assert_eq!(list.get_range(capture), &[]);
}

#[test]
fn test_token_capture_range() {
    let capture = TokenCapture::Range { start: 1, end: 3 };
    assert!(!capture.is_empty());
    assert_eq!(capture.len(), 2);
}

#[test]
fn test_token_capture_new() {
    // Empty range becomes None
    assert_eq!(TokenCapture::new(5, 5), TokenCapture::None);

    // Non-empty range becomes Range
    assert_eq!(
        TokenCapture::new(1, 4),
        TokenCapture::Range { start: 1, end: 4 }
    );
}

#[test]
fn test_token_capture_default() {
    let capture = TokenCapture::default();
    assert!(matches!(capture, TokenCapture::None));
}

#[test]
fn test_token_list_get_range() {
    let mut list = TokenList::new();
    list.push(Token::new(TokenKind::Let, Span::new(0, 3)));
    list.push(Token::new(
        TokenKind::Ident(crate::Name::EMPTY),
        Span::new(4, 5),
    ));
    list.push(Token::new(TokenKind::Eq, Span::new(6, 7)));
    list.push(Token::new(TokenKind::Int(42), Span::new(8, 10)));

    // Get range [1, 3) = tokens at indices 1 and 2
    let capture = TokenCapture::Range { start: 1, end: 3 };
    let range = list.get_range(capture);
    assert_eq!(range.len(), 2);
    assert!(matches!(range[0].kind, TokenKind::Ident(_)));
    assert!(matches!(range[1].kind, TokenKind::Eq));
}

#[test]
fn test_token_list_try_get_range() {
    let mut list = TokenList::new();
    list.push(Token::new(TokenKind::Let, Span::new(0, 3)));

    // Valid range
    let capture = TokenCapture::Range { start: 0, end: 1 };
    assert!(list.try_get_range(capture).is_some());

    // Invalid range (out of bounds)
    let capture = TokenCapture::Range { start: 0, end: 5 };
    assert!(list.try_get_range(capture).is_none());
}

#[test]
fn test_token_capture_span() {
    let mut list = TokenList::new();
    list.push(Token::new(TokenKind::Let, Span::new(0, 3)));
    list.push(Token::new(
        TokenKind::Ident(crate::Name::EMPTY),
        Span::new(4, 5),
    ));
    list.push(Token::new(TokenKind::Eq, Span::new(6, 7)));

    // Span of range [0, 3) should merge first and last token spans
    let capture = TokenCapture::Range { start: 0, end: 3 };
    let span = capture.span(&list).unwrap();
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 7);

    // None capture has no span
    assert!(TokenCapture::None.span(&list).is_none());
}

#[test]
fn test_token_capture_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    set.insert(TokenCapture::None);
    set.insert(TokenCapture::None); // duplicate
    set.insert(TokenCapture::Range { start: 0, end: 3 });
    set.insert(TokenCapture::Range { start: 0, end: 3 }); // duplicate
    set.insert(TokenCapture::Range { start: 1, end: 4 });

    assert_eq!(set.len(), 3);
}

#[test]
fn test_token_tag_stability() {
    // Pin key discriminant values that external code may depend on.
    // If these change, update all downstream consumers.
    assert_eq!(TokenTag::Ident as u8, 0);
    assert_eq!(TokenTag::Int as u8, 1);
    assert_eq!(TokenTag::Float as u8, 2);
    assert_eq!(TokenTag::String as u8, 3);
    assert_eq!(TokenTag::KwIf as u8, 20);
    assert_eq!(TokenTag::KwLet as u8, 23);
    assert_eq!(TokenTag::KwIntType as u8, 50);
    assert_eq!(TokenTag::KwNeverType as u8, 56);
    assert_eq!(TokenTag::LParen as u8, 80);
    assert_eq!(TokenTag::Pipe as u8, 95);
    assert_eq!(TokenTag::Eq as u8, 100);
    assert_eq!(TokenTag::Eof as u8, 127);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive TokenTag variant enumeration"
)]
fn test_token_tag_name_non_empty() {
    // Every TokenTag variant must return a non-empty name
    let all_tags: &[TokenTag] = &[
        TokenTag::Ident,
        TokenTag::Int,
        TokenTag::Float,
        TokenTag::String,
        TokenTag::Char,
        TokenTag::Duration,
        TokenTag::Size,
        TokenTag::TemplateHead,
        TokenTag::TemplateMiddle,
        TokenTag::TemplateTail,
        TokenTag::TemplateComplete,
        TokenTag::FormatSpec,
        TokenTag::HashBang,
        TokenTag::KwAsync,
        TokenTag::KwBreak,
        TokenTag::KwContinue,
        TokenTag::KwReturn,
        TokenTag::KwDef,
        TokenTag::KwDo,
        TokenTag::KwElse,
        TokenTag::KwFalse,
        TokenTag::KwFor,
        TokenTag::KwIf,
        TokenTag::KwImpl,
        TokenTag::KwIn,
        TokenTag::KwLet,
        TokenTag::KwLoop,
        TokenTag::KwMatch,
        TokenTag::KwPub,
        TokenTag::KwSelfLower,
        TokenTag::KwSelfUpper,
        TokenTag::KwSuspend,
        TokenTag::KwThen,
        TokenTag::KwTrait,
        TokenTag::KwTrue,
        TokenTag::KwType,
        TokenTag::KwUnsafe,
        TokenTag::KwUse,
        TokenTag::KwUses,
        TokenTag::KwVoid,
        TokenTag::KwWhere,
        TokenTag::KwWith,
        TokenTag::KwYield,
        TokenTag::KwTests,
        TokenTag::KwAs,
        TokenTag::KwDyn,
        TokenTag::KwExtend,
        TokenTag::KwExtension,
        TokenTag::KwSkip,
        TokenTag::KwExtern,
        TokenTag::KwIntType,
        TokenTag::KwFloatType,
        TokenTag::KwBoolType,
        TokenTag::KwStrType,
        TokenTag::KwCharType,
        TokenTag::KwByteType,
        TokenTag::KwNeverType,
        TokenTag::KwOk,
        TokenTag::KwErr,
        TokenTag::KwSome,
        TokenTag::KwNone,
        TokenTag::KwCache,
        TokenTag::KwCatch,
        TokenTag::KwParallel,
        TokenTag::KwSpawn,
        TokenTag::KwRecurse,
        TokenTag::KwRun,
        TokenTag::KwTimeout,
        TokenTag::KwTry,
        TokenTag::KwBy,
        TokenTag::Div,
        TokenTag::KwPrint,
        TokenTag::KwPanic,
        TokenTag::KwTodo,
        TokenTag::KwUnreachable,
        TokenTag::HashBracket,
        TokenTag::HashBang,
        TokenTag::At,
        TokenTag::Dollar,
        TokenTag::Hash,
        TokenTag::LParen,
        TokenTag::RParen,
        TokenTag::LBrace,
        TokenTag::RBrace,
        TokenTag::LBracket,
        TokenTag::RBracket,
        TokenTag::Colon,
        TokenTag::DoubleColon,
        TokenTag::Comma,
        TokenTag::Dot,
        TokenTag::DotDot,
        TokenTag::DotDotEq,
        TokenTag::DotDotDot,
        TokenTag::Arrow,
        TokenTag::FatArrow,
        TokenTag::Underscore,
        TokenTag::Semicolon,
        TokenTag::Pipe,
        TokenTag::Eq,
        TokenTag::EqEq,
        TokenTag::NotEq,
        TokenTag::Lt,
        TokenTag::LtEq,
        TokenTag::Shl,
        TokenTag::Gt,
        TokenTag::GtEq,
        TokenTag::Shr,
        TokenTag::Plus,
        TokenTag::Minus,
        TokenTag::Star,
        TokenTag::Slash,
        TokenTag::Percent,
        TokenTag::Bang,
        TokenTag::Tilde,
        TokenTag::Amp,
        TokenTag::AmpAmp,
        TokenTag::PipePipe,
        TokenTag::Question,
        TokenTag::DoubleQuestion,
        TokenTag::Caret,
        TokenTag::Newline,
        TokenTag::Error,
        TokenTag::Eof,
    ];

    for tag in all_tags {
        let name = tag.name();
        assert!(!name.is_empty(), "TokenTag::{tag:?} returned empty name",);
    }
}

#[test]
fn test_token_list_push_invariant() {
    // After every push, tags[i] must equal tokens[i].kind.discriminant_index()
    let mut list = TokenList::new();

    let tokens_to_push = [
        Token::new(TokenKind::Int(42), Span::new(0, 2)),
        Token::new(TokenKind::Plus, Span::new(3, 4)),
        Token::new(TokenKind::Ident(crate::Name::EMPTY), Span::new(5, 6)),
        Token::new(TokenKind::If, Span::new(7, 9)),
        Token::new(TokenKind::LParen, Span::new(10, 11)),
    ];

    for token in tokens_to_push {
        list.push(token);
    }

    let tags = list.tags();
    for (i, token) in list.iter().enumerate() {
        assert_eq!(
            tags[i],
            token.kind.discriminant_index(),
            "Tag mismatch at index {i}: tags[{i}]={} but discriminant={}",
            tags[i],
            token.kind.discriminant_index(),
        );
    }
}

#[test]
fn test_token_list_flags_parallel_invariant() {
    // flags.len() must always equal tokens.len()
    let mut list = TokenList::new();
    assert_eq!(list.flags().len(), list.len());

    list.push(Token::new(TokenKind::Int(1), Span::new(0, 1)));
    assert_eq!(list.flags().len(), list.len());

    list.push(Token::new(TokenKind::Plus, Span::new(2, 3)));
    assert_eq!(list.flags().len(), list.len());

    list.push(Token::new(TokenKind::Int(2), Span::new(4, 5)));
    assert_eq!(list.flags().len(), list.len());
}

#[test]
fn test_token_flags_operations() {
    let empty = TokenFlags::EMPTY;
    assert!(!empty.contains(TokenFlags::SPACE_BEFORE));
    assert!(!empty.contains(TokenFlags::NEWLINE_BEFORE));
    assert!(!empty.has_space_before());
    assert!(!empty.has_newline_before());

    let with_space = TokenFlags::from_bits(TokenFlags::SPACE_BEFORE);
    assert!(with_space.contains(TokenFlags::SPACE_BEFORE));
    assert!(!with_space.contains(TokenFlags::NEWLINE_BEFORE));
    assert!(with_space.has_space_before());

    let with_both = TokenFlags::from_bits(TokenFlags::SPACE_BEFORE | TokenFlags::NEWLINE_BEFORE);
    assert!(with_both.has_space_before());
    assert!(with_both.has_newline_before());
}

#[test]
fn position_independent_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(list: &TokenList) -> u64 {
        let mut h = DefaultHasher::new();
        list.hash(&mut h);
        h.finish()
    }

    // Same kinds, different spans → same hash (position-independent)
    let list_a = TokenList::from_vec(vec![
        Token::new(TokenKind::Int(1), Span::new(0, 1)),
        Token::new(TokenKind::Plus, Span::new(2, 3)),
        Token::new(TokenKind::Int(2), Span::new(4, 5)),
    ]);
    let list_b = TokenList::from_vec(vec![
        Token::new(TokenKind::Int(1), Span::new(10, 11)),
        Token::new(TokenKind::Plus, Span::new(20, 21)),
        Token::new(TokenKind::Int(2), Span::new(30, 31)),
    ]);

    assert_eq!(hash_of(&list_a), hash_of(&list_b));
    assert_eq!(list_a, list_b);
}

#[test]
fn different_kinds_not_equal() {
    // Different token kinds → not equal even at same positions
    let list_a = TokenList::from_vec(vec![
        Token::new(TokenKind::Int(1), Span::new(0, 1)),
        Token::new(TokenKind::Plus, Span::new(2, 3)),
    ]);
    let list_b = TokenList::from_vec(vec![
        Token::new(TokenKind::Int(1), Span::new(0, 1)),
        Token::new(TokenKind::Minus, Span::new(2, 3)),
    ]);

    assert_ne!(list_a, list_b);
}

#[test]
fn position_independent_hash_in_hashset() {
    use std::collections::HashSet;

    let list_pos1 = TokenList::from_vec(vec![
        Token::new(TokenKind::Let, Span::new(0, 3)),
        Token::new(TokenKind::Ident(crate::Name::EMPTY), Span::new(4, 5)),
    ]);
    let list_pos2 = TokenList::from_vec(vec![
        Token::new(TokenKind::Let, Span::new(100, 103)),
        Token::new(TokenKind::Ident(crate::Name::EMPTY), Span::new(200, 201)),
    ]);

    let mut set = HashSet::new();
    set.insert(list_pos1);
    set.insert(list_pos2); // same kinds/flags, different positions → deduped
    assert_eq!(set.len(), 1, "position-shifted lists should be equal");
}
