use super::*;

// === LexOutput Tests ===

#[test]
fn test_lex_output_new() {
    let output = LexOutput::new();
    assert!(output.tokens.is_empty());
    assert!(output.comments.is_empty());
    assert!(output.blank_lines.is_empty());
    assert!(output.newlines.is_empty());
}

#[test]
fn test_lex_output_with_capacity() {
    let output = LexOutput::with_capacity(1000);
    assert!(output.tokens.is_empty());
    assert!(output.comments.is_empty());
    // Capacity is allocated but contents are empty
}

#[test]
fn test_lex_output_into_metadata() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// comment\n\nlet x = 1", &interner);
    let metadata = output.into_metadata();

    assert_eq!(metadata.comments.len(), 1);
    assert!(!metadata.blank_lines.is_empty());
    assert!(!metadata.newlines.is_empty());
}

// === Newline Tracking Tests ===

#[test]
fn test_newline_tracking_single() {
    let interner = StringInterner::new();
    let output = lex_with_comments("x\ny", &interner);

    assert_eq!(output.newlines.len(), 1);
    assert_eq!(output.newlines[0], 1); // newline at position 1
}

#[test]
fn test_newline_tracking_multiple() {
    let interner = StringInterner::new();
    let output = lex_with_comments("a\nb\nc", &interner);

    assert_eq!(output.newlines.len(), 2);
    assert_eq!(output.newlines[0], 1); // after 'a'
    assert_eq!(output.newlines[1], 3); // after 'b'
}

#[test]
fn test_newline_tracking_none() {
    let interner = StringInterner::new();
    let output = lex_with_comments("let x = 42", &interner);

    assert!(output.newlines.is_empty());
}

// === Blank Line Detection Tests ===

#[test]
fn test_blank_line_single() {
    let interner = StringInterner::new();
    // "a\n\nb" has a blank line between the two newlines
    let output = lex_with_comments("a\n\nb", &interner);

    assert_eq!(output.blank_lines.len(), 1);
    // The blank line is at position 2 (the second newline)
    assert_eq!(output.blank_lines[0], 2);
}

#[test]
fn test_blank_line_multiple() {
    let interner = StringInterner::new();
    // "a\n\n\nb" has two blank lines
    let output = lex_with_comments("a\n\n\nb", &interner);

    assert_eq!(output.blank_lines.len(), 2);
}

#[test]
fn test_blank_line_none() {
    let interner = StringInterner::new();
    // "a\nb\nc" has no blank lines
    let output = lex_with_comments("a\nb\nc", &interner);

    assert!(output.blank_lines.is_empty());
}

#[test]
fn test_blank_line_at_start() {
    let interner = StringInterner::new();
    // "\n\nlet" starts with a blank line
    let output = lex_with_comments("\n\nlet", &interner);

    assert_eq!(output.blank_lines.len(), 1);
    assert_eq!(output.blank_lines[0], 1); // second newline at position 1
}

#[test]
fn test_blank_line_at_end() {
    let interner = StringInterner::new();
    // "let\n\n" ends with a blank line
    let output = lex_with_comments("let\n\n", &interner);

    assert_eq!(output.blank_lines.len(), 1);
}

#[test]
fn test_blank_line_with_comment_between() {
    let interner = StringInterner::new();
    // Comment between newlines should NOT create a blank line
    // because content exists on the line
    let output = lex_with_comments("a\n// comment\nb", &interner);

    // There's a newline after 'a' and 'comment', but no blank line
    assert!(output.blank_lines.is_empty());
    assert_eq!(output.comments.len(), 1);
}

#[test]
fn test_blank_line_after_comment() {
    let interner = StringInterner::new();
    // Comment followed by blank line
    // "a\n// comment\n\nb"
    // Line 1: a
    // Line 2: // comment
    // Line 3: (blank)
    // Line 4: b
    let output = lex_with_comments("a\n// comment\n\nb", &interner);

    // There should be a blank line (the line after the comment)
    assert_eq!(output.blank_lines.len(), 1);
}

// === Comment Tracking Tests ===

#[test]
fn test_comment_tracking_single() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// hello\nlet x = 1", &interner);

    assert_eq!(output.comments.len(), 1);
}

#[test]
fn test_comment_tracking_multiple() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// first\n// second\nlet x = 1", &interner);

    assert_eq!(output.comments.len(), 2);
}

#[test]
fn test_comment_tracking_with_blank_lines() {
    let interner = StringInterner::new();
    let source = r"// comment 1

// comment 2
let x = 1";
    let output = lex_with_comments(source, &interner);

    assert_eq!(output.comments.len(), 2);
    assert_eq!(output.blank_lines.len(), 1);
}

// === Integration Tests ===

#[test]
fn test_realistic_source() {
    let interner = StringInterner::new();
    let source = r"// #Description
// This is a doc comment

@main () -> void
let x = 42

// Regular comment
x
";
    let output = lex_with_comments(source, &interner);

    // Two doc comments at the top
    assert_eq!(output.comments.len(), 3);

    // Blank line after "doc comment" and after "x = 42"
    assert!(!output.blank_lines.is_empty());

    // Multiple newlines throughout
    assert!(output.newlines.len() >= 5);
}

#[test]
fn test_empty_source() {
    let interner = StringInterner::new();
    let output = lex_with_comments("", &interner);

    assert!(output.comments.is_empty());
    assert!(output.blank_lines.is_empty());
    assert!(output.newlines.is_empty());
    assert_eq!(output.tokens.len(), 1); // Just EOF
}

#[test]
fn test_only_newlines() {
    let interner = StringInterner::new();
    let output = lex_with_comments("\n\n\n", &interner);

    assert_eq!(output.newlines.len(), 3);
    assert_eq!(output.blank_lines.len(), 2); // Two consecutive pairs
}

#[test]
fn test_only_comments() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// a\n// b\n// c", &interner);

    assert_eq!(output.comments.len(), 3);
    assert_eq!(output.newlines.len(), 2);
    assert!(output.blank_lines.is_empty());
}

// === Metadata Conversion Tests ===

#[test]
fn test_metadata_preserves_comments() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// #Description\nfn foo()", &interner);
    let metadata = output.into_metadata();

    assert_eq!(metadata.comments.len(), 1);
    assert!(metadata.comments.get(0).is_some_and(|c| c.kind.is_doc()));
}

#[test]
fn test_metadata_preserves_blank_lines() {
    let interner = StringInterner::new();
    let output = lex_with_comments("a\n\nb", &interner);
    let metadata = output.into_metadata();

    assert_eq!(metadata.blank_lines.len(), 1);
    assert!(metadata.has_blank_line_between(1, 3));
}

#[test]
fn test_metadata_line_number() {
    let interner = StringInterner::new();
    let output = lex_with_comments("line1\nline2\nline3", &interner);
    let metadata = output.into_metadata();

    // Position 0 is line 1
    assert_eq!(metadata.line_number(0), 1);
    // Position 7 (after first newline + "line2") is line 2
    assert_eq!(metadata.line_number(7), 2);
}

// === V2 Entry Point Tests ===

#[test]
fn test_lex_basic() {
    let interner = StringInterner::new();
    let tokens = lex("let x = 42", &interner);
    // let, x, =, 42, EOF
    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens[0].kind, TokenKind::Let);
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert_eq!(tokens[2].kind, TokenKind::Eq);
    assert_eq!(tokens[3].kind, TokenKind::Int(42));
    assert_eq!(tokens[4].kind, TokenKind::Eof);
}

#[test]
fn test_lex_empty() {
    let interner = StringInterner::new();
    let tokens = lex("", &interner);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Eof);
}

#[test]
fn test_lex_newlines() {
    let interner = StringInterner::new();
    let tokens = lex("a\nb", &interner);
    // a, newline, b, EOF
    assert_eq!(tokens.len(), 4);
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert_eq!(tokens[1].kind, TokenKind::Newline);
    assert!(matches!(tokens[2].kind, TokenKind::Ident(_)));
    assert_eq!(tokens[3].kind, TokenKind::Eof);
}

#[test]
fn test_lex_with_comments() {
    let interner = StringInterner::new();
    // Comments should be skipped in lex (same as lex)
    let tokens = lex("// comment\nlet x = 1", &interner);
    // newline, let, x, =, 1, EOF
    assert_eq!(tokens.len(), 6);
    assert_eq!(tokens[0].kind, TokenKind::Newline);
    assert_eq!(tokens[1].kind, TokenKind::Let);
}

#[test]
fn test_lex_with_comments_basic() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// hello\nlet x = 1", &interner);
    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.newlines.len(), 1);
    // newline, let, x, =, 1, EOF
    assert_eq!(output.tokens.len(), 6);
}

#[test]
fn test_lex_with_comments_blank_lines() {
    let interner = StringInterner::new();
    let output = lex_with_comments("a\n\nb", &interner);
    assert_eq!(output.blank_lines.len(), 1);
    assert_eq!(output.blank_lines[0], 2);
}

// === Span coverage property ===

#[test]
fn spans_cover_source() {
    let interner = StringInterner::new();
    let source = "let x = 42 + 3\n// comment\nfoo()";
    let tokens = lex(source, &interner);

    // Every non-EOF token should have start < end
    for token in tokens.iter() {
        if token.kind != TokenKind::Eof {
            assert!(
                token.span.start < token.span.end,
                "zero-width token: {token:?}"
            );
        }
    }

    // EOF span should point to end of source
    let eof = &tokens[tokens.len() - 1];
    assert_eq!(eof.kind, TokenKind::Eof);
    assert_eq!(eof.span.start, source.len() as u32);
}

// === ADJACENT flag tests ===

#[test]
fn adjacent_flag_on_first_token() {
    // First token in file has no preceding trivia → ADJACENT
    let interner = StringInterner::new();
    let tokens = lex("let", &interner);
    let flags = tokens.flags();
    assert!(flags[0].is_adjacent(), "first token should be ADJACENT");
}

#[test]
fn adjacent_flag_with_space() {
    // "a b" — second token has SPACE_BEFORE, NOT ADJACENT
    let interner = StringInterner::new();
    let tokens = lex("a b", &interner);
    let flags = tokens.flags();
    assert!(flags[0].is_adjacent(), "'a' is first token → ADJACENT");
    assert!(
        !flags[1].is_adjacent(),
        "'b' has space before → not ADJACENT"
    );
    assert!(flags[1].has_space_before());
}

#[test]
fn adjacent_flag_no_space() {
    // "a+b" — all three tokens adjacent
    let interner = StringInterner::new();
    let tokens = lex("a+b", &interner);
    let flags = tokens.flags();
    assert!(flags[0].is_adjacent(), "'a' is first → ADJACENT");
    assert!(flags[1].is_adjacent(), "'+' no space → ADJACENT");
    assert!(flags[2].is_adjacent(), "'b' no space → ADJACENT");
}

#[test]
fn adjacent_flag_after_newline() {
    // "a\nb" — 'b' has NEWLINE_BEFORE, not ADJACENT
    let interner = StringInterner::new();
    let tokens = lex("a\nb", &interner);
    let flags = tokens.flags();
    // tokens: a, newline, b, EOF
    assert!(flags[0].is_adjacent());
    // newline token itself has no preceding trivia (follows 'a' directly)
    // 'b' has NEWLINE_BEFORE | LINE_START, not ADJACENT
    assert!(!flags[2].is_adjacent());
    assert!(flags[2].has_newline_before());
}

#[test]
fn adjacent_flag_after_comment() {
    // "a// comment\nb" — 'b' has TRIVIA_BEFORE + NEWLINE_BEFORE
    let interner = StringInterner::new();
    let tokens = lex("a// comment\nb", &interner);
    let flags = tokens.flags();
    assert!(flags[0].is_adjacent());
    // tokens: a (0), newline (1), b (2), EOF (3)
    // 'b' at index 2 has NEWLINE_BEFORE from the newline, not ADJACENT
    assert!(!flags[2].is_adjacent());
}

#[test]
fn adjacent_mutual_exclusion_with_space() {
    // ADJACENT and SPACE_BEFORE should be mutually exclusive
    let interner = StringInterner::new();
    let tokens = lex("a b", &interner);
    let flags = tokens.flags();
    // 'a' — adjacent, no space
    assert!(flags[0].is_adjacent());
    assert!(!flags[0].has_space_before());
    // 'b' — space, not adjacent
    assert!(!flags[1].is_adjacent());
    assert!(flags[1].has_space_before());
}

// === HAS_ERROR flag tests ===

#[test]
fn has_error_on_invalid_byte() {
    let interner = StringInterner::new();
    let tokens = lex("\x01", &interner);
    let flags = tokens.flags();
    assert!(flags[0].has_error(), "invalid byte should have HAS_ERROR");
}

#[test]
fn has_error_on_integer_overflow() {
    let interner = StringInterner::new();
    let tokens = lex("99999999999999999999999", &interner);
    let flags = tokens.flags();
    assert!(flags[0].has_error(), "overflow int should have HAS_ERROR");
}

#[test]
fn no_error_on_valid_token() {
    let interner = StringInterner::new();
    let tokens = lex("let x = 42", &interner);
    let flags = tokens.flags();
    for (i, f) in flags.iter().enumerate() {
        assert!(!f.has_error(), "token {i} should not have HAS_ERROR");
    }
}

#[test]
fn semicolon_is_valid_token() {
    let interner = StringInterner::new();
    let tokens = lex(";", &interner);
    assert_eq!(tokens[0].kind, TokenKind::Semicolon);
    let flags = tokens.flags();
    assert!(!flags[0].has_error(), "semicolon should not have HAS_ERROR");
}

// === CONTEXTUAL_KW flag tests ===

#[test]
fn contextual_kw_on_soft_keyword_with_paren() {
    // "cache (x)" — 'cache' is a soft keyword with ( → CONTEXTUAL_KW
    let interner = StringInterner::new();
    let tokens = lex("cache (x)", &interner);
    let flags = tokens.flags();
    assert_eq!(tokens[0].kind, TokenKind::Cache);
    assert!(
        flags[0].is_contextual_kw(),
        "cache followed by ( should have CONTEXTUAL_KW"
    );
}

#[test]
fn no_contextual_kw_on_reserved_keyword() {
    // "if (x)" — 'if' is a reserved keyword, NOT contextual
    let interner = StringInterner::new();
    let tokens = lex("if (x)", &interner);
    let flags = tokens.flags();
    assert_eq!(tokens[0].kind, TokenKind::If);
    assert!(
        !flags[0].is_contextual_kw(),
        "reserved keyword should not have CONTEXTUAL_KW"
    );
}

#[test]
fn no_contextual_kw_on_identifier() {
    // "cache = 42" — 'cache' without ( is an identifier, no flag
    let interner = StringInterner::new();
    let tokens = lex("cache = 42", &interner);
    let flags = tokens.flags();
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert!(
        !flags[0].is_contextual_kw(),
        "identifier should not have CONTEXTUAL_KW"
    );
}

#[test]
fn contextual_kw_on_all_soft_keywords() {
    let interner = StringInterner::new();
    for kw in &["cache", "catch", "parallel", "spawn", "recurse", "timeout"] {
        let source = format!("{kw}(x)");
        let tokens = lex(&source, &interner);
        let flags = tokens.flags();
        assert!(
            flags[0].is_contextual_kw(),
            "{kw}(x) should have CONTEXTUAL_KW"
        );
    }
}

// === Reserved-future keyword tests ===

#[test]
fn reserved_future_keyword_produces_error() {
    let interner = StringInterner::new();
    let output = lex_with_comments("let static = 1", &interner);
    assert!(
        output.has_errors(),
        "reserved-future keyword should produce error"
    );
    // 'static' is token index 1 (no whitespace tokens — they become flags)
    assert!(matches!(output.tokens[1].kind, TokenKind::Ident(_)));
    // HAS_ERROR should be set on the 'static' token
    assert!(output.tokens.flags()[1].has_error());
}

#[test]
fn all_reserved_future_keywords_produce_errors() {
    let interner = StringInterner::new();
    for kw in &["asm", "inline", "static", "union", "view"] {
        let output = lex_with_comments(kw, &interner);
        assert!(
            output.has_errors(),
            "`{kw}` should produce a reserved-future keyword error"
        );
        assert!(
            matches!(output.tokens[0].kind, TokenKind::Ident(_)),
            "`{kw}` should still lex as identifier"
        );
    }
}

// === IS_DOC flag tests ===

#[test]
fn is_doc_on_token_after_description() {
    // "// #Desc\ndef" — 'def' should have IS_DOC
    let interner = StringInterner::new();
    let output = lex_with_comments("// #Description\ndef", &interner);
    let flags = output.tokens.flags();
    // tokens: newline, def, EOF
    assert_eq!(output.tokens[1].kind, TokenKind::Def);
    assert!(
        flags[1].is_doc(),
        "'def' after doc description should have IS_DOC"
    );
}

#[test]
fn is_doc_on_token_after_member() {
    // "// * x: val\ndef" — 'def' should have IS_DOC
    let interner = StringInterner::new();
    let output = lex_with_comments("// * x: value\ndef", &interner);
    let flags = output.tokens.flags();
    assert_eq!(output.tokens[1].kind, TokenKind::Def);
    assert!(
        flags[1].is_doc(),
        "'def' after doc member should have IS_DOC"
    );
}

#[test]
fn is_doc_on_token_after_warning() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// !Panics\ndef", &interner);
    let flags = output.tokens.flags();
    assert_eq!(output.tokens[1].kind, TokenKind::Def);
    assert!(
        flags[1].is_doc(),
        "'def' after doc warning should have IS_DOC"
    );
}

#[test]
fn is_doc_on_token_after_example() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// >foo()\ndef", &interner);
    let flags = output.tokens.flags();
    assert_eq!(output.tokens[1].kind, TokenKind::Def);
    assert!(
        flags[1].is_doc(),
        "'def' after doc example should have IS_DOC"
    );
}

#[test]
fn no_is_doc_after_regular_comment() {
    // "// regular\ndef" — 'def' should NOT have IS_DOC
    let interner = StringInterner::new();
    let output = lex_with_comments("// regular comment\ndef", &interner);
    let flags = output.tokens.flags();
    assert_eq!(output.tokens[1].kind, TokenKind::Def);
    assert!(
        !flags[1].is_doc(),
        "'def' after regular comment should not have IS_DOC"
    );
}

#[test]
fn is_doc_with_multiple_doc_comments() {
    // Multiple doc comments before a declaration
    let interner = StringInterner::new();
    let output = lex_with_comments("// #Description\n// * x: value\ndef", &interner);
    let flags = output.tokens.flags();
    // tokens: newline, newline, def, EOF
    assert_eq!(output.tokens[2].kind, TokenKind::Def);
    assert!(
        flags[2].is_doc(),
        "'def' after multiple doc comments should have IS_DOC"
    );
}

#[test]
fn no_is_doc_on_newline_token() {
    // IS_DOC should not be set on the newline between doc comment and def
    let interner = StringInterner::new();
    let output = lex_with_comments("// #Description\ndef", &interner);
    let flags = output.tokens.flags();
    // tokens: newline(0), def(1), EOF(2)
    assert_eq!(output.tokens[0].kind, TokenKind::Newline);
    assert!(!flags[0].is_doc(), "newline should not have IS_DOC");
}

#[test]
fn is_doc_on_non_declaration_token() {
    // IS_DOC is positional — set even before non-declaration tokens
    let interner = StringInterner::new();
    let output = lex_with_comments("// #Description\nlet", &interner);
    let flags = output.tokens.flags();
    assert_eq!(output.tokens[1].kind, TokenKind::Let);
    assert!(
        flags[1].is_doc(),
        "'let' after doc comment should have IS_DOC"
    );
}

#[test]
fn no_is_doc_without_comment() {
    // No comments at all — no IS_DOC
    let interner = StringInterner::new();
    let output = lex_with_comments("def foo", &interner);
    let flags = output.tokens.flags();
    assert!(
        !flags[0].is_doc(),
        "'def' without preceding doc should not have IS_DOC"
    );
}

#[test]
fn is_doc_set_in_simple_lex() {
    // lex() delegates to lex_with_comments(), so IS_DOC is set
    let interner = StringInterner::new();
    let tokens = lex("// #Description\ndef", &interner);
    let flags = tokens.flags();
    // tokens: newline, def, EOF
    assert!(
        flags[1].is_doc(),
        "lex() should set IS_DOC (delegates to lex_with_comments)"
    );
}

// === LexOutput Salsa Trait Tests ===

#[test]
fn lex_output_equality() {
    let interner = StringInterner::new();
    let a = lex_with_comments("let x = 42", &interner);
    let b = lex_with_comments("let x = 42", &interner);
    assert_eq!(a, b);
}

#[test]
fn lex_output_inequality() {
    let interner = StringInterner::new();
    let a = lex_with_comments("let x = 42", &interner);
    let b = lex_with_comments("let y = 42", &interner);
    assert_ne!(a, b);
}

#[test]
fn lex_output_hashset_insertion() {
    use std::collections::HashSet;
    let interner = StringInterner::new();

    let a = lex_with_comments("let x = 1", &interner);
    let b = lex_with_comments("let x = 1", &interner);
    let c = lex_with_comments("let y = 2", &interner);

    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b); // duplicate
    set.insert(c);
    assert_eq!(set.len(), 2);
}

#[test]
fn lex_output_debug_format() {
    let interner = StringInterner::new();
    let output = lex_with_comments("// comment\nlet x = 42", &interner);
    let debug = format!("{output:?}");
    assert!(debug.contains("LexOutput"));
    assert!(debug.contains("tokens"));
    assert!(debug.contains("comments"));
}

// === Encoding issue detection tests ===

#[test]
fn utf8_bom_produces_error() {
    let interner = StringInterner::new();
    let source = "\u{FEFF}let x = 1";
    let output = lex_with_comments(source, &interner);
    assert!(output.has_errors(), "UTF-8 BOM should produce an error");
    let bom_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| e.kind == lex_error::LexErrorKind::Utf8Bom)
        .collect();
    assert_eq!(bom_errors.len(), 1);
    assert_eq!(bom_errors[0].span, Span::new(0, 3));
}

#[test]
fn utf8_bom_only_produces_error() {
    // BOM-only file should still produce the error
    let interner = StringInterner::new();
    let source = "\u{FEFF}";
    let output = lex_with_comments(source, &interner);
    let bom_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| e.kind == lex_error::LexErrorKind::Utf8Bom)
        .collect();
    assert_eq!(bom_errors.len(), 1);
}

#[test]
fn clean_source_no_encoding_errors() {
    let interner = StringInterner::new();
    let output = lex_with_comments("let x = 42", &interner);
    let encoding_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                lex_error::LexErrorKind::Utf8Bom
                    | lex_error::LexErrorKind::Utf16LeBom
                    | lex_error::LexErrorKind::Utf16BeBom
                    | lex_error::LexErrorKind::InvalidNullByte
            )
        })
        .collect();
    assert!(
        encoding_errors.is_empty(),
        "clean source should have no encoding errors"
    );
}

#[test]
fn interior_null_produces_error() {
    let interner = StringInterner::new();
    let source = "let\0x";
    let output = lex_with_comments(source, &interner);
    let null_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| e.kind == lex_error::LexErrorKind::InvalidNullByte)
        .collect();
    assert_eq!(
        null_errors.len(),
        1,
        "interior null should produce InvalidNullByte error"
    );
    assert_eq!(null_errors[0].span, Span::new(3, 4));
}

#[test]
fn multiple_interior_nulls_produce_errors() {
    let interner = StringInterner::new();
    let source = "\0a\0";
    let output = lex_with_comments(source, &interner);
    let null_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| e.kind == lex_error::LexErrorKind::InvalidNullByte)
        .collect();
    assert_eq!(null_errors.len(), 2, "each null should produce an error");
}

#[test]
fn interior_null_no_duplicate_error() {
    // Interior null bytes should produce exactly ONE error (InvalidNullByte
    // from SourceBuffer encoding detection), NOT a second InvalidByte { byte: 0 }
    // from the scanner/cooker path.
    let interner = StringInterner::new();
    let source = "let\0x";
    let output = lex_with_comments(source, &interner);
    let invalid_byte_zero: Vec<_> = output
        .errors
        .iter()
        .filter(|e| e.kind == lex_error::LexErrorKind::InvalidByte { byte: 0 })
        .collect();
    assert!(
        invalid_byte_zero.is_empty(),
        "interior null should not produce InvalidByte {{ byte: 0 }} — \
         the specific InvalidNullByte error already covers it"
    );
}

#[test]
fn interior_null_total_error_count() {
    // A single interior null should produce exactly 1 error total.
    let interner = StringInterner::new();
    let source = "let\0x";
    let output = lex_with_comments(source, &interner);
    assert_eq!(
        output.errors.len(),
        1,
        "interior null should produce exactly 1 error, got: {:?}",
        output.errors.iter().map(|e| &e.kind).collect::<Vec<_>>()
    );
}

#[test]
fn multiple_interior_nulls_no_duplicates() {
    // Two interior nulls should produce exactly 2 errors (one per null),
    // not 4 (which would happen with duplicate reporting).
    let interner = StringInterner::new();
    let source = "\0a\0";
    let output = lex_with_comments(source, &interner);
    assert_eq!(
        output.errors.len(),
        2,
        "two interior nulls should produce exactly 2 errors, got: {:?}",
        output.errors.iter().map(|e| &e.kind).collect::<Vec<_>>()
    );
}

// === HashBang token tests ===

#[test]
fn hashbang_produces_token() {
    let interner = StringInterner::new();
    let source = "#!compiler_version";
    let output = lex_with_comments(source, &interner);
    // #! should produce HashBang token, not Error
    assert_eq!(output.tokens[0].kind, TokenKind::HashBang);
    assert_eq!(output.tokens[0].span, Span::new(0, 2));
}

#[test]
fn hashbang_no_error() {
    let interner = StringInterner::new();
    let source = "#!foo";
    let output = lex_with_comments(source, &interner);
    // HashBang should not produce any error
    let hashbang_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| e.span == Span::new(0, 2))
        .collect();
    assert!(
        hashbang_errors.is_empty(),
        "#! should not produce errors, got: {hashbang_errors:?}"
    );
}

#[test]
fn hashbang_followed_by_ident() {
    let interner = StringInterner::new();
    let source = "#!version";
    let tokens = lex(source, &interner);
    // tokens: #!, version, EOF
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].kind, TokenKind::HashBang);
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert_eq!(tokens[2].kind, TokenKind::Eof);
}
