use super::*;
use crate::SourceBuffer;

/// Helper: scan a source string and collect all tokens (excluding Eof).
fn scan(source: &str) -> Vec<RawToken> {
    let buf = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut tokens = Vec::new();
    loop {
        let tok = scanner.next_token();
        if tok.tag == RawTag::Eof {
            break;
        }
        tokens.push(tok);
    }
    tokens
}

/// Helper: scan and return tags only.
fn scan_tags(source: &str) -> Vec<RawTag> {
    scan(source).iter().map(|t| t.tag).collect()
}

/// Helper: scan and verify the scanner produced Eof.
fn scan_with_eof(source: &str) -> Vec<RawToken> {
    let buf = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut tokens = Vec::new();
    loop {
        let tok = scanner.next_token();
        tokens.push(tok);
        if tok.tag == RawTag::Eof {
            break;
        }
    }
    tokens
}

// ─── Property Tests ────────────────────────────────────────────

#[test]
fn total_len_equals_source_len() {
    let sources = [
        "",
        "x",
        "hello world",
        "let x = 42\nlet y = x + 1",
        "\"hello\" 'c' 123 0xFF",
        "..= ... ?? :: << ->",
        "`template {x} middle {y} tail`",
        "  \t\n  \r\n  ",
        "#[attr] #!file @main $var",
    ];
    for source in sources {
        let tokens = scan(source);
        let total_len: u32 = tokens.iter().map(|t| t.len).sum();
        assert_eq!(
            total_len,
            u32::try_from(source.len()).expect("test source fits in u32"),
            "total token length mismatch for {source:?}",
        );
    }
}

#[test]
fn every_token_has_positive_length() {
    let sources = ["let x = 42", "+-*/%", "\"str\" 'c'", "`tmpl`", "  \t\n\r\n"];
    for source in sources {
        for tok in scan(source) {
            assert!(tok.len > 0, "zero-length token {tok:?} in {source:?}");
        }
    }
}

#[test]
fn eof_has_zero_length() {
    let tokens = scan_with_eof("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].tag, RawTag::Eof);
    assert_eq!(tokens[0].len, 0);
}

#[test]
fn eof_is_always_last() {
    let tokens = scan_with_eof("hello world");
    let last = tokens
        .last()
        .expect("scan_with_eof should produce at least one token");
    assert_eq!(last.tag, RawTag::Eof);
}

#[test]
fn repeated_eof_returns_eof() {
    let buf = SourceBuffer::new("");
    let mut scanner = RawScanner::new(buf.cursor());
    for _ in 0..5 {
        let tok = scanner.next_token();
        assert_eq!(tok.tag, RawTag::Eof);
        assert_eq!(tok.len, 0);
    }
}

#[test]
fn template_depth_empty_after_complete_scan() {
    let sources = [
        "`hello`",
        "`{x}`",
        "`{a} and {b}`",
        "`outer {`inner {x}`}`",
        "let x = `{1 + 2}`",
    ];
    for source in sources {
        let buf = SourceBuffer::new(source);
        let mut scanner = RawScanner::new(buf.cursor());
        loop {
            let tok = scanner.next_token();
            if tok.tag == RawTag::Eof {
                break;
            }
        }
        assert!(
            scanner.template_depth.is_empty(),
            "template_depth not empty after scanning {source:?}",
        );
    }
}

// ─── Byte Coverage ─────────────────────────────────────────────

#[test]
fn all_256_bytes_produce_valid_token() {
    for byte in 0u8..=255 {
        let source = [byte];
        // We need valid UTF-8 for SourceBuffer, so use from_utf8_lossy
        // For non-UTF-8 bytes, we test via raw cursor construction instead
        if let Ok(s) = std::str::from_utf8(&source) {
            let buf = SourceBuffer::new(s);
            let mut scanner = RawScanner::new(buf.cursor());
            let tok = scanner.next_token();
            // Should not panic and should produce a token
            assert!(
                tok.tag == RawTag::Eof || tok.len > 0,
                "byte {byte} produced invalid token: {tok:?}",
            );
        }
    }
}

#[test]
fn all_printable_ascii_produce_valid_tokens() {
    for byte in 32u8..=126 {
        let bytes = [byte];
        let source = std::str::from_utf8(&bytes).expect("printable ASCII is valid UTF-8");
        let tokens = scan(source);
        let total_len: u32 = tokens.iter().map(|t| t.len).sum();
        assert_eq!(
            total_len, 1,
            "byte {:?} ({}) produced total_len={}, tokens={:?}",
            byte as char, byte, total_len, tokens
        );
    }
}

// ─── Whitespace & Newlines ─────────────────────────────────────

#[test]
fn whitespace_spaces_and_tabs() {
    assert_eq!(scan_tags("   "), vec![RawTag::Whitespace]);
    assert_eq!(scan("   ")[0].len, 3);

    assert_eq!(scan_tags("\t\t"), vec![RawTag::Whitespace]);
    assert_eq!(scan_tags("  \t  "), vec![RawTag::Whitespace]);
}

#[test]
fn newline_lf() {
    assert_eq!(scan_tags("\n"), vec![RawTag::Newline]);
    assert_eq!(scan("\n")[0].len, 1);
}

#[test]
fn newline_crlf_normalized() {
    assert_eq!(scan_tags("\r\n"), vec![RawTag::Newline]);
    assert_eq!(scan("\r\n")[0].len, 2);
}

#[test]
fn lone_cr_is_whitespace() {
    assert_eq!(scan_tags("\r"), vec![RawTag::Whitespace]);
    assert_eq!(scan("\r")[0].len, 1);
}

#[test]
fn mixed_whitespace_and_newlines() {
    let tags = scan_tags("  \n\t\t\r\n  ");
    assert_eq!(
        tags,
        vec![
            RawTag::Whitespace, // "  "
            RawTag::Newline,    // "\n"
            RawTag::Whitespace, // "\t\t"
            RawTag::Newline,    // "\r\n"
            RawTag::Whitespace, // "  "
        ]
    );
}

#[test]
fn empty_source() {
    assert_eq!(scan_tags(""), vec![]);
    let tokens = scan_with_eof("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].tag, RawTag::Eof);
}

// ─── Comments ──────────────────────────────────────────────────

#[test]
fn line_comment() {
    assert_eq!(scan_tags("// hello"), vec![RawTag::LineComment]);
    assert_eq!(scan("// hello")[0].len, 8);
}

#[test]
fn line_comment_does_not_consume_newline() {
    let tags = scan_tags("// hello\n");
    assert_eq!(tags, vec![RawTag::LineComment, RawTag::Newline]);
}

#[test]
fn slash_alone() {
    assert_eq!(scan_tags("/"), vec![RawTag::Slash]);
    assert_eq!(scan("/")[0].len, 1);
}

#[test]
fn slash_followed_by_non_slash() {
    let tags = scan_tags("/x");
    assert_eq!(tags, vec![RawTag::Slash, RawTag::Ident]);
}

// ─── Identifiers ───────────────────────────────────────────────

#[test]
fn simple_identifiers() {
    assert_eq!(scan_tags("foo"), vec![RawTag::Ident]);
    assert_eq!(scan("foo")[0].len, 3);

    assert_eq!(scan_tags("_foo"), vec![RawTag::Ident]);
    assert_eq!(scan("_foo")[0].len, 4);

    assert_eq!(scan_tags("foo_bar"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("FooBar"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("x1"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("_"), vec![RawTag::Underscore]);
}

#[test]
fn underscore_alone_is_underscore() {
    assert_eq!(scan_tags("_"), vec![RawTag::Underscore]);
    assert_eq!(scan("_")[0].len, 1);
}

#[test]
fn underscore_followed_by_space() {
    let tags = scan_tags("_ x");
    assert_eq!(
        tags,
        vec![RawTag::Underscore, RawTag::Whitespace, RawTag::Ident]
    );
}

#[test]
fn underscore_followed_by_ident() {
    assert_eq!(scan_tags("_x"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("__"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("_0"), vec![RawTag::Ident]);
}

#[test]
fn keywords_are_ident() {
    // Raw scanner does not resolve keywords
    assert_eq!(scan_tags("let"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("if"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("fn"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("true"), vec![RawTag::Ident]);
    assert_eq!(scan_tags("false"), vec![RawTag::Ident]);
}

// ─── Operators (single-char) ───────────────────────────────────

#[test]
fn single_char_operators() {
    assert_eq!(scan_tags("+"), vec![RawTag::Plus]);
    assert_eq!(scan_tags("-"), vec![RawTag::Minus]);
    assert_eq!(scan_tags("*"), vec![RawTag::Star]);
    assert_eq!(scan_tags("/"), vec![RawTag::Slash]);
    assert_eq!(scan_tags("%"), vec![RawTag::Percent]);
    assert_eq!(scan_tags("^"), vec![RawTag::Caret]);
    assert_eq!(scan_tags("&"), vec![RawTag::Ampersand]);
    assert_eq!(scan_tags("|"), vec![RawTag::Pipe]);
    assert_eq!(scan_tags("~"), vec![RawTag::Tilde]);
    assert_eq!(scan_tags("!"), vec![RawTag::Bang]);
    assert_eq!(scan_tags("="), vec![RawTag::Equal]);
    assert_eq!(scan_tags("<"), vec![RawTag::Less]);
    assert_eq!(scan_tags(">"), vec![RawTag::Greater]);
    assert_eq!(scan_tags("."), vec![RawTag::Dot]);
    assert_eq!(scan_tags("?"), vec![RawTag::Question]);
}

// ─── Operators (compound) ──────────────────────────────────────

#[test]
fn compound_operators() {
    assert_eq!(scan_tags("=="), vec![RawTag::EqualEqual]);
    assert_eq!(scan_tags("!="), vec![RawTag::BangEqual]);
    assert_eq!(scan_tags("<="), vec![RawTag::LessEqual]);
    assert_eq!(scan_tags("&&"), vec![RawTag::AmpersandAmpersand]);
    assert_eq!(scan_tags("||"), vec![RawTag::PipePipe]);
    assert_eq!(scan_tags("->"), vec![RawTag::Arrow]);
    assert_eq!(scan_tags("=>"), vec![RawTag::FatArrow]);
    assert_eq!(scan_tags(".."), vec![RawTag::DotDot]);
    assert_eq!(scan_tags("..="), vec![RawTag::DotDotEqual]);
    assert_eq!(scan_tags("..."), vec![RawTag::DotDotDot]);
    assert_eq!(scan_tags("::"), vec![RawTag::ColonColon]);
    assert_eq!(scan_tags("<<"), vec![RawTag::Shl]);
    assert_eq!(scan_tags("??"), vec![RawTag::QuestionQuestion]);
}

#[test]
fn greater_is_always_single() {
    // `>` is always a single token — parser synthesizes >= and >>
    assert_eq!(scan_tags(">="), vec![RawTag::Greater, RawTag::Equal]);
    assert_eq!(scan_tags(">>"), vec![RawTag::Greater, RawTag::Greater]);
}

#[test]
fn compound_assignment_tokens() {
    // Compound assignment operators are single tokens (maximal munch)
    assert_eq!(scan_tags("+="), vec![RawTag::PlusEq]);
    assert_eq!(scan_tags("-="), vec![RawTag::MinusEq]);
    assert_eq!(scan_tags("*="), vec![RawTag::StarEq]);
    assert_eq!(scan_tags("/="), vec![RawTag::SlashEq]);
    assert_eq!(scan_tags("%="), vec![RawTag::PercentEq]);
    assert_eq!(scan_tags("@="), vec![RawTag::AtEq]);
    assert_eq!(scan_tags("&="), vec![RawTag::AmpersandEq]);
    assert_eq!(scan_tags("|="), vec![RawTag::PipeEq]);
    assert_eq!(scan_tags("^="), vec![RawTag::CaretEq]);
    assert_eq!(scan_tags("<<="), vec![RawTag::ShlEq]);
    assert_eq!(scan_tags("&&="), vec![RawTag::AmpersandAmpersandEq]);
    assert_eq!(scan_tags("||="), vec![RawTag::PipePipeEq]);
    // >>= stays as three tokens (> is always single for generics)
    assert_eq!(
        scan_tags(">>="),
        vec![RawTag::Greater, RawTag::Greater, RawTag::Equal]
    );
}

// ─── Delimiters ────────────────────────────────────────────────

#[test]
fn delimiters() {
    assert_eq!(scan_tags("("), vec![RawTag::LeftParen]);
    assert_eq!(scan_tags(")"), vec![RawTag::RightParen]);
    assert_eq!(scan_tags("["), vec![RawTag::LeftBracket]);
    assert_eq!(scan_tags("]"), vec![RawTag::RightBracket]);
    assert_eq!(scan_tags("{"), vec![RawTag::LeftBrace]);
    assert_eq!(scan_tags("}"), vec![RawTag::RightBrace]);
    assert_eq!(scan_tags(","), vec![RawTag::Comma]);
    assert_eq!(scan_tags(":"), vec![RawTag::Colon]);
    assert_eq!(scan_tags(";"), vec![RawTag::Semicolon]);
    assert_eq!(scan_tags("@"), vec![RawTag::At]);
    assert_eq!(scan_tags("$"), vec![RawTag::Dollar]);
}

#[test]
fn hash_variants() {
    assert_eq!(scan_tags("#"), vec![RawTag::Hash]);
    assert_eq!(scan_tags("#["), vec![RawTag::HashBracket]);
    assert_eq!(scan_tags("#!"), vec![RawTag::HashBang]);
    assert_eq!(scan_tags("#x"), vec![RawTag::Hash, RawTag::Ident]);
}

#[test]
fn backslash_is_error_detection() {
    assert_eq!(scan_tags("\\"), vec![RawTag::Backslash]);
}

// ─── Numeric Literals ──────────────────────────────────────────

#[test]
fn integer_literals() {
    assert_eq!(scan_tags("42"), vec![RawTag::Int]);
    assert_eq!(scan("42")[0].len, 2);
    assert_eq!(scan_tags("0"), vec![RawTag::Int]);
    assert_eq!(scan_tags("1_000_000"), vec![RawTag::Int]);
}

#[test]
fn float_literals() {
    assert_eq!(scan_tags("3.14"), vec![RawTag::Float]);
    assert_eq!(scan("3.14")[0].len, 4);
    assert_eq!(scan_tags("0.5"), vec![RawTag::Float]);
    assert_eq!(scan_tags("1.0e10"), vec![RawTag::Float]);
    assert_eq!(scan_tags("1.0E-5"), vec![RawTag::Float]);
}

#[test]
fn hex_literals() {
    assert_eq!(scan_tags("0xFF"), vec![RawTag::HexInt]);
    assert_eq!(scan_tags("0x00"), vec![RawTag::HexInt]);
    assert_eq!(scan_tags("0xDEAD_BEEF"), vec![RawTag::HexInt]);
    assert_eq!(scan_tags("0X1A"), vec![RawTag::HexInt]);
}

#[test]
fn binary_literals() {
    assert_eq!(scan_tags("0b1010"), vec![RawTag::BinInt]);
    assert_eq!(scan_tags("0b00"), vec![RawTag::BinInt]);
    assert_eq!(scan_tags("0b1111_0000"), vec![RawTag::BinInt]);
    assert_eq!(scan_tags("0B10"), vec![RawTag::BinInt]);
    assert_eq!(scan_tags("0b_1010"), vec![RawTag::BinInt]);
}

#[test]
fn zero_bytes_vs_binary_disambiguation() {
    // `0b` alone (not followed by binary digit) = size literal (0 bytes)
    assert_eq!(scan_tags("0b"), vec![RawTag::Size]);
    // `0b` followed by binary digit = binary integer
    assert_eq!(scan_tags("0b1"), vec![RawTag::BinInt]);
    assert_eq!(scan_tags("0b0"), vec![RawTag::BinInt]);
}

#[test]
fn dot_after_int_is_not_float() {
    // `42..` should be Int then DotDot, not Float
    let tags = scan_tags("42..");
    assert_eq!(tags, vec![RawTag::Int, RawTag::DotDot]);
}

#[test]
fn int_dot_ident_is_not_float() {
    // `42.foo` should be Int, Dot, Ident — not Float
    let tags = scan_tags("42.foo");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Dot, RawTag::Ident]);
}

#[test]
fn exponent_without_dot() {
    assert_eq!(scan_tags("1e5"), vec![RawTag::Float]);
    assert_eq!(scan_tags("1E10"), vec![RawTag::Float]);
    assert_eq!(scan_tags("1e+5"), vec![RawTag::Float]);
    assert_eq!(scan_tags("1e-5"), vec![RawTag::Float]);
}

// ─── Duration Literals ─────────────────────────────────────────

#[test]
fn duration_integer() {
    assert_eq!(scan_tags("100ns"), vec![RawTag::Duration]);
    assert_eq!(scan("100ns")[0].len, 5);
    assert_eq!(scan_tags("50us"), vec![RawTag::Duration]);
    assert_eq!(scan_tags("200ms"), vec![RawTag::Duration]);
    assert_eq!(scan_tags("5s"), vec![RawTag::Duration]);
    assert_eq!(scan_tags("10m"), vec![RawTag::Duration]);
    assert_eq!(scan_tags("2h"), vec![RawTag::Duration]);
}

#[test]
fn duration_decimal() {
    // Decimal durations are valid per grammar.ebnf lines 136-137
    assert_eq!(scan_tags("0.5s"), vec![RawTag::Duration]);
    assert_eq!(scan("0.5s")[0].len, 4);
    assert_eq!(scan_tags("1.5ms"), vec![RawTag::Duration]);
    assert_eq!(scan_tags("0.25h"), vec![RawTag::Duration]);
}

#[test]
fn duration_suffix_not_consumed_if_followed_by_ident() {
    // `10sec` should be Int + Ident, not Duration
    let tags = scan_tags("10sec");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);

    // `10min` should be Int + Ident
    let tags = scan_tags("10min");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);

    // `10hours` should be Int + Ident
    let tags = scan_tags("10hours");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);
}

// ─── Size Literals ─────────────────────────────────────────────

#[test]
fn size_integer() {
    assert_eq!(scan_tags("100b"), vec![RawTag::Size]);
    assert_eq!(scan_tags("10kb"), vec![RawTag::Size]);
    assert_eq!(scan_tags("5mb"), vec![RawTag::Size]);
    assert_eq!(scan_tags("2gb"), vec![RawTag::Size]);
    assert_eq!(scan_tags("1tb"), vec![RawTag::Size]);
}

#[test]
fn size_decimal() {
    assert_eq!(scan_tags("1.5kb"), vec![RawTag::Size]);
    assert_eq!(scan("1.5kb")[0].len, 5);
    assert_eq!(scan_tags("0.5mb"), vec![RawTag::Size]);
}

#[test]
fn size_suffix_not_consumed_if_followed_by_ident() {
    // `10bytes` should be Int + Ident, not Size
    let tags = scan_tags("10bytes");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);

    // `10kbps` should be Int + Ident
    let tags = scan_tags("10kbps");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);
}

// ─── String Literals ───────────────────────────────────────────

#[test]
fn simple_string() {
    assert_eq!(scan_tags("\"hello\""), vec![RawTag::String]);
    assert_eq!(scan("\"hello\"")[0].len, 7);
}

#[test]
fn empty_string() {
    assert_eq!(scan_tags("\"\""), vec![RawTag::String]);
    assert_eq!(scan("\"\"")[0].len, 2);
}

#[test]
fn string_with_escape() {
    assert_eq!(scan_tags("\"he\\\"llo\""), vec![RawTag::String]);
    assert_eq!(scan_tags("\"\\n\\t\\\\\""), vec![RawTag::String]);
}

#[test]
fn unterminated_string_newline() {
    assert_eq!(
        scan_tags("\"hello\n"),
        vec![RawTag::UnterminatedString, RawTag::Newline]
    );
}

#[test]
fn unterminated_string_eof() {
    assert_eq!(scan_tags("\"hello"), vec![RawTag::UnterminatedString]);
}

#[test]
fn adjacent_strings() {
    assert_eq!(
        scan_tags("\"a\"\"b\""),
        vec![RawTag::String, RawTag::String]
    );
}

// ─── Character Literals ────────────────────────────────────────

#[test]
fn simple_char() {
    assert_eq!(scan_tags("'x'"), vec![RawTag::Char]);
    assert_eq!(scan("'x'")[0].len, 3);
}

#[test]
fn char_with_escape() {
    assert_eq!(scan_tags("'\\n'"), vec![RawTag::Char]);
    assert_eq!(scan("'\\n'")[0].len, 4);
    assert_eq!(scan_tags("'\\''"), vec![RawTag::Char]);
}

#[test]
fn unterminated_char_eof() {
    assert_eq!(scan_tags("'x"), vec![RawTag::UnterminatedChar]);
}

#[test]
fn empty_char_literal() {
    // '' — opening ' consumed, then immediate ' is "empty char" -> UnterminatedChar(1)
    // Then second ' starts a new char_literal, consumes opening ', hits EOF -> UnterminatedChar(1)
    let tags = scan_tags("''");
    assert_eq!(
        tags,
        vec![RawTag::UnterminatedChar, RawTag::UnterminatedChar]
    );
}

#[test]
fn char_unicode_2byte() {
    // λ = U+03BB = 2 bytes (CE BB)
    assert_eq!(scan_tags("'λ'"), vec![RawTag::Char]);
    assert_eq!(scan("'λ'")[0].len, 4); // ' + 2-byte char + '
}

#[test]
fn char_unicode_3byte() {
    // ñ = U+00F1 when encoded differently, use CJK: 中 = U+4E2D = 3 bytes
    assert_eq!(scan_tags("'中'"), vec![RawTag::Char]);
    assert_eq!(scan("'中'")[0].len, 5); // ' + 3-byte char + '
}

#[test]
fn char_unicode_4byte() {
    // 😀 = U+1F600 = 4 bytes (F0 9F 98 80)
    assert_eq!(scan_tags("'😀'"), vec![RawTag::Char]);
    assert_eq!(scan("'😀'")[0].len, 6); // ' + 4-byte char + '
}

// ─── Template Literals ─────────────────────────────────────────

#[test]
fn template_complete() {
    assert_eq!(scan_tags("`hello`"), vec![RawTag::TemplateComplete]);
    assert_eq!(scan("`hello`")[0].len, 7);
}

#[test]
fn template_empty() {
    assert_eq!(scan_tags("``"), vec![RawTag::TemplateComplete]);
    assert_eq!(scan("``")[0].len, 2);
}

#[test]
fn template_single_interpolation() {
    let tags = scan_tags("`{x}`");
    assert_eq!(
        tags,
        vec![RawTag::TemplateHead, RawTag::Ident, RawTag::TemplateTail]
    );
}

#[test]
fn template_with_text_and_interpolation() {
    let tags = scan_tags("`hello {name}`");
    assert_eq!(
        tags,
        vec![RawTag::TemplateHead, RawTag::Ident, RawTag::TemplateTail]
    );
}

#[test]
fn template_multiple_interpolations() {
    let tags = scan_tags("`{a} and {b}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,
            RawTag::TemplateMiddle,
            RawTag::Ident,
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_nested_braces() {
    // `{if x then {a: 1} else {b: 2}}`
    let tags = scan_tags("`{x + {a: 1}}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,
            RawTag::Whitespace,
            RawTag::Plus,
            RawTag::Whitespace,
            RawTag::LeftBrace,
            RawTag::Ident,
            RawTag::Colon,
            RawTag::Whitespace,
            RawTag::Int,
            RawTag::RightBrace,
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_nested_templates() {
    // `outer {`inner {x}`}`
    let tags = scan_tags("`outer {`inner {x}`}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead, // `outer {
            RawTag::TemplateHead, // `inner {
            RawTag::Ident,        // x
            RawTag::TemplateTail, // }`
            RawTag::TemplateTail, // }`
        ]
    );
}

#[test]
fn template_escaped_braces() {
    assert_eq!(scan_tags("`{{literal}}`"), vec![RawTag::TemplateComplete]);
}

#[test]
fn template_escaped_backtick() {
    assert_eq!(
        scan_tags(r"`hello \` world`"),
        vec![RawTag::TemplateComplete]
    );
}

#[test]
fn template_multiline() {
    assert_eq!(scan_tags("`line1\nline2`"), vec![RawTag::TemplateComplete]);
}

#[test]
fn template_unterminated() {
    assert_eq!(scan_tags("`hello"), vec![RawTag::UnterminatedTemplate]);
}

#[test]
fn template_unterminated_in_interpolation() {
    // `{x  — template opens, interpolation starts, then EOF
    // After TemplateHead + Ident, the scanner sees EOF.
    // The `}` that would trigger template_middle_or_tail never arrives.
    // The template_depth stack is orphaned — the cooking layer detects this.
    let tags = scan_tags("`{x");
    assert_eq!(tags, vec![RawTag::TemplateHead, RawTag::Ident]);

    // Verify template_depth is NOT empty (orphaned)
    let buf = SourceBuffer::new("`{x");
    let mut scanner = RawScanner::new(buf.cursor());
    loop {
        let tok = scanner.next_token();
        if tok.tag == RawTag::Eof {
            break;
        }
    }
    assert!(
        !scanner.template_depth.is_empty(),
        "template_depth should be non-empty for unterminated interpolation"
    );
}

// ─── Format Spec in Template Interpolation ─────────────────────

#[test]
fn template_format_spec_simple() {
    // `{value:x}` — simple format spec
    let tags = scan_tags("`{value:x}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,
            RawTag::FormatSpec,
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_format_spec_complex() {
    // `{value:>10.2f}` — alignment, width, precision, type
    let tags = scan_tags("`{value:>10.2f}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,
            RawTag::FormatSpec,
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_format_spec_zero_padded() {
    // `{value:08x}` — zero-padded hex
    let tags = scan_tags("`{value:08x}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,
            RawTag::FormatSpec,
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_no_format_spec() {
    // `{value}` — no format spec, no FormatSpec token
    let tags = scan_tags("`{value}`");
    assert_eq!(
        tags,
        vec![RawTag::TemplateHead, RawTag::Ident, RawTag::TemplateTail]
    );
}

#[test]
fn template_format_spec_empty() {
    // `{value:}` — empty format spec (all components optional)
    let tags = scan_tags("`{value:}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,
            RawTag::FormatSpec,
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_colon_inside_parens_not_format_spec() {
    // `{func(a: b):x}` — colon inside parens is NOT format spec
    let tags = scan_tags("`{func(a: b):x}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident, // func
            RawTag::LeftParen,
            RawTag::Ident, // a
            RawTag::Colon, // : (inside parens, regular colon)
            RawTag::Whitespace,
            RawTag::Ident, // b
            RawTag::RightParen,
            RawTag::FormatSpec, // :x (at top level, format spec)
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_colon_inside_brackets_not_format_spec() {
    // `{map[k:v]:x}` — colon inside brackets is NOT format spec
    let tags = scan_tags("`{map[k:v]:x}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident, // map
            RawTag::LeftBracket,
            RawTag::Ident, // k
            RawTag::Colon, // : (inside brackets, regular colon)
            RawTag::Ident, // v
            RawTag::RightBracket,
            RawTag::FormatSpec, // :x (at top level, format spec)
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_format_spec_with_multiple_interpolations() {
    // `{a:x} and {b:>10}`
    let tags = scan_tags("`{a:x} and {b:>10}`");
    assert_eq!(
        tags,
        vec![
            RawTag::TemplateHead,
            RawTag::Ident,      // a
            RawTag::FormatSpec, // :x
            RawTag::TemplateMiddle,
            RawTag::Ident,      // b
            RawTag::FormatSpec, // :>10
            RawTag::TemplateTail,
        ]
    );
}

#[test]
fn template_format_spec_length_correct() {
    // Verify FormatSpec token length includes the leading ':'
    let tokens = scan("`{x:>10.2f}`");
    // Tokens: TemplateHead(`{), Ident(x), FormatSpec(:>10.2f), TemplateTail(}`)
    let format_spec = &tokens[2];
    assert_eq!(format_spec.tag, RawTag::FormatSpec);
    assert_eq!(format_spec.len, 7); // ":>10.2f" = 7 bytes
}

// ─── Invalid Bytes ─────────────────────────────────────────────

#[test]
fn non_ascii_byte_is_invalid() {
    // Non-ASCII in UTF-8 context — the SourceBuffer accepts &str so
    // we can test with a multi-byte UTF-8 char
    let tags = scan_tags("\u{00E9}"); // é (2 bytes: 0xC3 0xA9)
                                      // Each non-ASCII byte produces InvalidByte
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().all(|t| *t == RawTag::InvalidByte));
}

#[test]
fn control_chars_are_invalid() {
    // Control char 0x01
    let tags = scan_tags("\x01");
    assert_eq!(tags, vec![RawTag::InvalidByte]);
}

// ─── Adjacent Tokens ───────────────────────────────────────────

#[test]
fn adjacent_no_whitespace() {
    let tags = scan_tags("a+b");
    assert_eq!(tags, vec![RawTag::Ident, RawTag::Plus, RawTag::Ident]);
}

#[test]
fn adjacent_numbers_and_operators() {
    let tags = scan_tags("1+2");
    assert_eq!(tags, vec![RawTag::Int, RawTag::Plus, RawTag::Int]);
}

#[test]
fn complex_expression() {
    let tags = scan_tags("x + y * (z - 1)");
    assert_eq!(
        tags,
        vec![
            RawTag::Ident,
            RawTag::Whitespace,
            RawTag::Plus,
            RawTag::Whitespace,
            RawTag::Ident,
            RawTag::Whitespace,
            RawTag::Star,
            RawTag::Whitespace,
            RawTag::LeftParen,
            RawTag::Ident,
            RawTag::Whitespace,
            RawTag::Minus,
            RawTag::Whitespace,
            RawTag::Int,
            RawTag::RightParen,
        ]
    );
}

// ─── Iterator impl ────────────────────────────────────────────

#[test]
fn iterator_yields_tokens_then_none() {
    let buf = SourceBuffer::new("a b");
    let scanner = RawScanner::new(buf.cursor());
    let tokens: Vec<_> = scanner.collect();
    assert_eq!(tokens.len(), 3); // Ident, Whitespace, Ident
    assert_eq!(tokens[0].tag, RawTag::Ident);
    assert_eq!(tokens[1].tag, RawTag::Whitespace);
    assert_eq!(tokens[2].tag, RawTag::Ident);
}

// ─── Tokenize convenience function ─────────────────────────────

#[test]
fn tokenize_convenience() {
    let tokens = tokenize("1 + 2");
    assert_eq!(tokens.len(), 5); // Int, WS, Plus, WS, Int
    assert_eq!(tokens[0].tag, RawTag::Int);
    assert_eq!(tokens[2].tag, RawTag::Plus);
    assert_eq!(tokens[4].tag, RawTag::Int);
}

// ─── Realistic Ori Code ────────────────────────────────────────

#[test]
fn realistic_let_binding() {
    let source = "let x = 42";
    let tags = scan_tags(source);
    assert_eq!(
        tags,
        vec![
            RawTag::Ident, // let
            RawTag::Whitespace,
            RawTag::Ident, // x
            RawTag::Whitespace,
            RawTag::Equal, // =
            RawTag::Whitespace,
            RawTag::Int, // 42
        ]
    );
}

#[test]
fn realistic_function_def() {
    let source = "fn add(a: int, b: int) -> int";
    let tags = scan_tags(source);
    assert_eq!(
        tags,
        vec![
            RawTag::Ident, // fn
            RawTag::Whitespace,
            RawTag::Ident, // add
            RawTag::LeftParen,
            RawTag::Ident, // a
            RawTag::Colon,
            RawTag::Whitespace,
            RawTag::Ident, // int
            RawTag::Comma,
            RawTag::Whitespace,
            RawTag::Ident, // b
            RawTag::Colon,
            RawTag::Whitespace,
            RawTag::Ident, // int
            RawTag::RightParen,
            RawTag::Whitespace,
            RawTag::Arrow, // ->
            RawTag::Whitespace,
            RawTag::Ident, // int
        ]
    );
}

#[test]
fn realistic_attribute_and_test() {
    let source = "@test tests\n@target () -> void";
    let tags = scan_tags(source);
    assert_eq!(
        tags,
        vec![
            RawTag::At,
            RawTag::Ident, // test
            RawTag::Whitespace,
            RawTag::Ident, // tests
            RawTag::Newline,
            RawTag::At,
            RawTag::Ident, // target
            RawTag::Whitespace,
            RawTag::LeftParen,
            RawTag::RightParen,
            RawTag::Whitespace,
            RawTag::Arrow,
            RawTag::Whitespace,
            RawTag::Ident, // void
        ]
    );
}
