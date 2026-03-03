use super::*;

// === Operator mapping ===

#[test]
fn direct_map_operators() {
    let source = "+-*/%^&|~!=<>.?";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

    assert_eq!(cooker.cook(RawTag::Plus, 0, 1).kind, TokenKind::Plus);
    assert_eq!(cooker.cook(RawTag::Minus, 1, 1).kind, TokenKind::Minus);
    assert_eq!(cooker.cook(RawTag::Star, 2, 1).kind, TokenKind::Star);
    assert_eq!(cooker.cook(RawTag::Slash, 3, 1).kind, TokenKind::Slash);
    assert_eq!(cooker.cook(RawTag::Percent, 4, 1).kind, TokenKind::Percent);
    assert_eq!(cooker.cook(RawTag::Caret, 5, 1).kind, TokenKind::Caret);
    assert_eq!(cooker.cook(RawTag::Ampersand, 6, 1).kind, TokenKind::Amp);
    assert_eq!(cooker.cook(RawTag::Pipe, 7, 1).kind, TokenKind::Pipe);
    assert_eq!(cooker.cook(RawTag::Tilde, 8, 1).kind, TokenKind::Tilde);
    assert_eq!(cooker.cook(RawTag::Bang, 9, 1).kind, TokenKind::Bang);
    assert_eq!(cooker.cook(RawTag::Equal, 10, 1).kind, TokenKind::Eq);
    assert_eq!(cooker.cook(RawTag::Less, 11, 1).kind, TokenKind::Lt);
    assert_eq!(cooker.cook(RawTag::Greater, 12, 1).kind, TokenKind::Gt);
    assert_eq!(cooker.cook(RawTag::Dot, 13, 1).kind, TokenKind::Dot);
    assert_eq!(
        cooker.cook(RawTag::Question, 14, 1).kind,
        TokenKind::Question
    );
    assert!(cooker.errors().is_empty());
}

#[test]
fn compound_operators() {
    let source = "== != <= && || -> => .. ..= ... :: << ??";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

    assert_eq!(cooker.cook(RawTag::EqualEqual, 0, 2).kind, TokenKind::EqEq);
    assert_eq!(cooker.cook(RawTag::BangEqual, 3, 2).kind, TokenKind::NotEq);
    assert_eq!(cooker.cook(RawTag::LessEqual, 6, 2).kind, TokenKind::LtEq);
    assert_eq!(
        cooker.cook(RawTag::AmpersandAmpersand, 9, 2).kind,
        TokenKind::AmpAmp
    );
    assert_eq!(
        cooker.cook(RawTag::PipePipe, 12, 2).kind,
        TokenKind::PipePipe
    );
    assert_eq!(cooker.cook(RawTag::Arrow, 15, 2).kind, TokenKind::Arrow);
    assert_eq!(
        cooker.cook(RawTag::FatArrow, 18, 2).kind,
        TokenKind::FatArrow
    );
    assert_eq!(cooker.cook(RawTag::DotDot, 21, 2).kind, TokenKind::DotDot);
    assert_eq!(
        cooker.cook(RawTag::DotDotEqual, 24, 3).kind,
        TokenKind::DotDotEq
    );
    assert_eq!(
        cooker.cook(RawTag::DotDotDot, 28, 3).kind,
        TokenKind::DotDotDot
    );
    assert_eq!(
        cooker.cook(RawTag::ColonColon, 32, 2).kind,
        TokenKind::DoubleColon
    );
    assert_eq!(cooker.cook(RawTag::Shl, 35, 2).kind, TokenKind::Shl);
    assert_eq!(
        cooker.cook(RawTag::QuestionQuestion, 38, 2).kind,
        TokenKind::DoubleQuestion
    );
}

// === Identifiers and keywords ===

#[test]
fn identifier_interning() {
    let source = "foo";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Ident, 0, 3);
    match result.kind {
        TokenKind::Ident(name) => assert_eq!(interner.lookup(name), "foo"),
        other => panic!("expected Ident, got {other:?}"),
    }
}

#[test]
fn keyword_resolution() {
    let source = "if";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::Ident, 0, 2).kind, TokenKind::If);
}

#[test]
fn str_type_keyword() {
    let source = "str";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::Ident, 0, 3).kind, TokenKind::StrType);
}

// === Numeric literals ===

#[test]
fn integer_literal() {
    let source = "42";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::Int, 0, 2).kind, TokenKind::Int(42));
}

#[test]
fn integer_with_underscores() {
    let source = "1_000_000";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Int, 0, 9).kind,
        TokenKind::Int(1_000_000)
    );
}

#[test]
fn hex_integer() {
    let source = "0xFF";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::HexInt, 0, 4).kind, TokenKind::Int(255));
}

#[test]
fn binary_integer() {
    let source = "0b1010";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::BinInt, 0, 6).kind, TokenKind::Int(10));
}

#[test]
fn binary_integer_with_underscores() {
    let source = "0b1111_0000";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::BinInt, 0, 11).kind, TokenKind::Int(240));
}

#[test]
#[expect(clippy::approx_constant, reason = "testing float parsing")]
fn float_literal() {
    let source = "3.14";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Float, 0, 4).kind,
        TokenKind::Float(3.14f64.to_bits())
    );
}

#[test]
fn integer_overflow() {
    let source = "99999999999999999999999";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Int, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
    assert_eq!(cooker.errors().len(), 1);
}

// === Float overflow (Finding 2) ===

#[test]
fn float_overflow_is_error() {
    // 1e999 overflows to infinity → error per spec 7.7.2
    let source = "1e999";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Float, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn float_max_finite_is_valid() {
    // Regression guard: max finite f64 must still parse
    let source = "1.7976931348623157e308";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Float, 0, source.len() as u32);
    assert!(matches!(result.kind, TokenKind::Float(_)));
    assert!(!result.had_error);
}

// === Malformed numeric literals (Finding 1) ===

#[test]
fn hex_int_no_digits_is_error() {
    // 0x with no hex digits → error, not Int(0)
    let source = "0x";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::HexInt, 0, 2);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn bin_int_only_underscores_is_error() {
    // 0b_ with only underscores after prefix → error
    let source = "0b_";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::BinInt, 0, 3);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn hex_int_zero_is_valid() {
    // Regression guard: 0x0 must still produce Int(0)
    let source = "0x0";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(cooker.cook(RawTag::HexInt, 0, 3).kind, TokenKind::Int(0));
}

// === Duration/Size ===

#[test]
fn duration_milliseconds() {
    let source = "100ms";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 5).kind,
        TokenKind::Duration(100, DurationUnit::Milliseconds)
    );
}

#[test]
fn duration_seconds() {
    let source = "5s";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 2).kind,
        TokenKind::Duration(5, DurationUnit::Seconds)
    );
}

#[test]
fn duration_hours() {
    let source = "2h";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 2).kind,
        TokenKind::Duration(2, DurationUnit::Hours)
    );
}

#[test]
fn size_kilobytes() {
    let source = "4kb";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Size, 0, 3).kind,
        TokenKind::Size(4, SizeUnit::Kilobytes)
    );
}

#[test]
fn size_bytes() {
    let source = "100b";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Size, 0, 4).kind,
        TokenKind::Size(100, SizeUnit::Bytes)
    );
}

// === Decimal duration/size (spec: compile-time sugar) ===

#[test]
fn decimal_duration_seconds() {
    // 1.5s = 1,500,000,000 nanoseconds
    let source = "1.5s";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 4).kind,
        TokenKind::Duration(1_500_000_000, DurationUnit::Nanoseconds)
    );
    assert!(cooker.errors().is_empty());
}

#[test]
fn decimal_duration_milliseconds() {
    // 2.5ms = 2,500,000 nanoseconds
    let source = "2.5ms";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 5).kind,
        TokenKind::Duration(2_500_000, DurationUnit::Nanoseconds)
    );
    assert!(cooker.errors().is_empty());
}

#[test]
fn decimal_duration_hours() {
    // 2.25h = 8,100,000,000,000 nanoseconds (2h 15m)
    let source = "2.25h";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 5).kind,
        TokenKind::Duration(8_100_000_000_000, DurationUnit::Nanoseconds)
    );
    assert!(cooker.errors().is_empty());
}

#[test]
fn decimal_duration_half_second() {
    // 0.5s = 500,000,000 nanoseconds
    let source = "0.5s";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, 4).kind,
        TokenKind::Duration(500_000_000, DurationUnit::Nanoseconds)
    );
}

#[test]
fn decimal_duration_many_digits() {
    // 1.123456789s = 1,123,456,789 nanoseconds (9 decimal places, still whole)
    let source = "1.123456789s";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Duration, 0, source.len() as u32).kind,
        TokenKind::Duration(1_123_456_789, DurationUnit::Nanoseconds)
    );
}

#[test]
fn decimal_duration_nanoseconds_error() {
    // 1.5ns = 1.5 nanoseconds — not a whole number → error
    let source = "1.5ns";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Duration, 0, 5);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
    assert_eq!(cooker.errors().len(), 1);
}

#[test]
fn decimal_size_kilobytes() {
    // 1.5kb = 1,500 bytes
    let source = "1.5kb";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Size, 0, 5).kind,
        TokenKind::Size(1_500, SizeUnit::Bytes)
    );
    assert!(cooker.errors().is_empty());
}

#[test]
fn decimal_size_megabytes() {
    // 0.25mb = 250,000 bytes
    let source = "0.25mb";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Size, 0, 6).kind,
        TokenKind::Size(250_000, SizeUnit::Bytes)
    );
}

#[test]
fn decimal_size_bytes_error() {
    // 0.5b = 0.5 bytes — not a whole number → error
    let source = "0.5b";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Size, 0, 4);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
    assert_eq!(cooker.errors().len(), 1);
}

// === String literals ===

#[test]
fn string_simple() {
    let source = r#""hello""#;
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::String, 0, source.len() as u32);
    match result.kind {
        TokenKind::String(name) => assert_eq!(interner.lookup(name), "hello"),
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn string_with_escapes() {
    let source = r#""hello\nworld""#;
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::String, 0, source.len() as u32);
    match result.kind {
        TokenKind::String(name) => assert_eq!(interner.lookup(name), "hello\nworld"),
        other => panic!("expected String, got {other:?}"),
    }
}

// === Char literals ===

#[test]
fn char_simple() {
    let source = "'a'";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Char, 0, source.len() as u32).kind,
        TokenKind::Char('a')
    );
}

#[test]
fn char_escape() {
    let source = r"'\n'";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::Char, 0, source.len() as u32).kind,
        TokenKind::Char('\n')
    );
}

// === Error tokens ===

#[test]
fn error_tags_produce_error_kind() {
    let source = "\x01";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

    let result = cooker.cook(RawTag::InvalidByte, 0, 1);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
    assert_eq!(cooker.errors().len(), 1);
}

// === Delimiter mapping ===

#[test]
fn delimiters() {
    let source = "()[]{},:;@#_$#[";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

    assert_eq!(cooker.cook(RawTag::LeftParen, 0, 1).kind, TokenKind::LParen);
    assert_eq!(
        cooker.cook(RawTag::RightParen, 1, 1).kind,
        TokenKind::RParen
    );
    assert_eq!(
        cooker.cook(RawTag::LeftBracket, 2, 1).kind,
        TokenKind::LBracket
    );
    assert_eq!(
        cooker.cook(RawTag::RightBracket, 3, 1).kind,
        TokenKind::RBracket
    );
    assert_eq!(cooker.cook(RawTag::LeftBrace, 4, 1).kind, TokenKind::LBrace);
    assert_eq!(
        cooker.cook(RawTag::RightBrace, 5, 1).kind,
        TokenKind::RBrace
    );
    assert_eq!(cooker.cook(RawTag::Comma, 6, 1).kind, TokenKind::Comma);
    assert_eq!(cooker.cook(RawTag::Colon, 7, 1).kind, TokenKind::Colon);
    assert_eq!(
        cooker.cook(RawTag::Semicolon, 8, 1).kind,
        TokenKind::Semicolon
    );
    assert_eq!(cooker.cook(RawTag::At, 9, 1).kind, TokenKind::At);
    assert_eq!(cooker.cook(RawTag::Hash, 10, 1).kind, TokenKind::Hash);
    assert_eq!(
        cooker.cook(RawTag::Underscore, 11, 1).kind,
        TokenKind::Underscore
    );
    assert_eq!(cooker.cook(RawTag::Dollar, 12, 1).kind, TokenKind::Dollar);
    assert_eq!(
        cooker.cook(RawTag::HashBracket, 13, 2).kind,
        TokenKind::HashBracket
    );
}

#[test]
fn hashbang_mapping() {
    let source = "#!foo";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    assert_eq!(
        cooker.cook(RawTag::HashBang, 0, 2).kind,
        TokenKind::HashBang
    );
    assert!(cooker.errors().is_empty());
}

// === Duration/Size overflow (Finding 3) ===

#[test]
fn duration_integer_overflow_is_error() {
    // 3000000h overflows i64 nanoseconds (~292yr limit)
    let source = "3000000h";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Duration, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn duration_max_valid_hours() {
    // ~2562047h is near the i64 nanosecond limit but valid
    let source = "2562047h";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Duration, 0, source.len() as u32);
    assert_eq!(
        result.kind,
        TokenKind::Duration(2_562_047, DurationUnit::Hours)
    );
    assert!(!result.had_error);
}

#[test]
fn duration_seconds_overflow_is_error() {
    // 10000000000s overflows i64 nanoseconds
    let source = "10000000000s";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Duration, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn size_terabytes_overflow_is_error() {
    // 18446745tb overflows u64
    let source = "18446745tb";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Size, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn size_bytes_exceeding_i64_is_error() {
    // 9223372036854775808b fits u64 but exceeds i64::MAX
    let source = "9223372036854775808b";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Size, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn decimal_duration_overflow_is_error() {
    // Decimal path: result nanos exceed i64
    let source = "99999999999.0s";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Duration, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

#[test]
fn decimal_size_overflow_is_error() {
    // Decimal path: result bytes exceed i64
    let source = "9999999999999999.0tb";
    let interner = StringInterner::new();
    let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
    let result = cooker.cook(RawTag::Size, 0, source.len() as u32);
    assert_eq!(result.kind, TokenKind::Error);
    assert!(result.had_error);
}

// === Suffix detection ===

#[test]
fn duration_suffix_detection() {
    assert_eq!(
        detect_duration_suffix("100ns"),
        (2, DurationUnit::Nanoseconds)
    );
    assert_eq!(
        detect_duration_suffix("50us"),
        (2, DurationUnit::Microseconds)
    );
    assert_eq!(
        detect_duration_suffix("100ms"),
        (2, DurationUnit::Milliseconds)
    );
    assert_eq!(detect_duration_suffix("5s"), (1, DurationUnit::Seconds));
    assert_eq!(detect_duration_suffix("10m"), (1, DurationUnit::Minutes));
    assert_eq!(detect_duration_suffix("2h"), (1, DurationUnit::Hours));
}

#[test]
fn size_suffix_detection() {
    assert_eq!(detect_size_suffix("100b"), (1, SizeUnit::Bytes));
    assert_eq!(detect_size_suffix("4kb"), (2, SizeUnit::Kilobytes));
    assert_eq!(detect_size_suffix("10mb"), (2, SizeUnit::Megabytes));
    assert_eq!(detect_size_suffix("1gb"), (2, SizeUnit::Gigabytes));
    assert_eq!(detect_size_suffix("1tb"), (2, SizeUnit::Terabytes));
}

// === Decimal unit value parsing ===

#[test]
fn parse_decimal_unit_value_basic() {
    // 1.5 * 1_000_000_000 (seconds→ns) = 1,500,000,000
    assert_eq!(
        parse_decimal_unit_value("1.5", 1_000_000_000),
        Some(1_500_000_000)
    );
}

#[test]
fn parse_decimal_unit_value_quarter() {
    // 0.25 * 3_600_000_000_000 (hours→ns) = 900,000,000,000
    assert_eq!(
        parse_decimal_unit_value("0.25", 3_600_000_000_000),
        Some(900_000_000_000)
    );
}

#[test]
fn parse_decimal_unit_value_not_representable() {
    // 1.5 * 1 (ns→ns) = 1.5 — not whole
    assert_eq!(parse_decimal_unit_value("1.5", 1), None);
}

#[test]
fn parse_decimal_unit_value_no_fraction() {
    // 5. * 1000 = 5000 (degenerate: dot with no fractional digits)
    assert_eq!(parse_decimal_unit_value("5.", 1000), Some(5000));
}

#[test]
fn parse_decimal_unit_value_many_digits() {
    // 1.123456789 * 1_000_000_000 = 1,123,456,789
    assert_eq!(
        parse_decimal_unit_value("1.123456789", 1_000_000_000),
        Some(1_123_456_789)
    );
}

#[test]
fn parse_decimal_unit_value_with_underscores() {
    // 1_000.5 * 1_000 = 1,000,500
    assert_eq!(parse_decimal_unit_value("1_000.5", 1_000), Some(1_000_500));
}
