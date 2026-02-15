use super::*;

// === Reserved keyword tests ===

#[test]
fn control_flow_keywords() {
    assert_eq!(lookup("if"), Some(TokenKind::If));
    assert_eq!(lookup("else"), Some(TokenKind::Else));
    assert_eq!(lookup("for"), Some(TokenKind::For));
    assert_eq!(lookup("in"), Some(TokenKind::In));
    assert_eq!(lookup("match"), Some(TokenKind::Match));
    assert_eq!(lookup("loop"), Some(TokenKind::Loop));
    assert_eq!(lookup("break"), Some(TokenKind::Break));
    assert_eq!(lookup("continue"), Some(TokenKind::Continue));
    assert_eq!(lookup("return"), Some(TokenKind::Return));
}

#[test]
fn declaration_keywords() {
    assert_eq!(lookup("let"), Some(TokenKind::Let));
    assert_eq!(lookup("def"), Some(TokenKind::Def));
    assert_eq!(lookup("type"), Some(TokenKind::Type));
    assert_eq!(lookup("trait"), Some(TokenKind::Trait));
    assert_eq!(lookup("impl"), Some(TokenKind::Impl));
    assert_eq!(lookup("pub"), Some(TokenKind::Pub));
    assert_eq!(lookup("mut"), Some(TokenKind::Mut));
}

#[test]
fn value_keywords() {
    assert_eq!(lookup("true"), Some(TokenKind::True));
    assert_eq!(lookup("false"), Some(TokenKind::False));
    assert_eq!(lookup("void"), Some(TokenKind::Void));
}

#[test]
fn type_keywords() {
    assert_eq!(lookup("int"), Some(TokenKind::IntType));
    assert_eq!(lookup("float"), Some(TokenKind::FloatType));
    assert_eq!(lookup("bool"), Some(TokenKind::BoolType));
    assert_eq!(lookup("str"), Some(TokenKind::StrType));
    assert_eq!(lookup("char"), Some(TokenKind::CharType));
    assert_eq!(lookup("byte"), Some(TokenKind::ByteType));
    assert_eq!(lookup("Never"), Some(TokenKind::NeverType));
}

#[test]
fn constructor_keywords() {
    assert_eq!(lookup("Ok"), Some(TokenKind::Ok));
    assert_eq!(lookup("Err"), Some(TokenKind::Err));
    assert_eq!(lookup("Some"), Some(TokenKind::Some));
    assert_eq!(lookup("None"), Some(TokenKind::None));
}

#[test]
fn always_resolved_pattern_keywords() {
    // run and try are always keywords (not soft)
    assert_eq!(lookup("run"), Some(TokenKind::Run));
    assert_eq!(lookup("try"), Some(TokenKind::Try));
    assert_eq!(lookup("by"), Some(TokenKind::By));
}

#[test]
fn builtin_keywords() {
    assert_eq!(lookup("print"), Some(TokenKind::Print));
    assert_eq!(lookup("panic"), Some(TokenKind::Panic));
    assert_eq!(lookup("todo"), Some(TokenKind::Todo));
    assert_eq!(lookup("unreachable"), Some(TokenKind::Unreachable));
}

#[test]
fn misc_keywords() {
    assert_eq!(lookup("async"), Some(TokenKind::Async));
    assert_eq!(lookup("do"), Some(TokenKind::Do));
    assert_eq!(lookup("then"), Some(TokenKind::Then));
    assert_eq!(lookup("yield"), Some(TokenKind::Yield));
    assert_eq!(lookup("tests"), Some(TokenKind::Tests));
    assert_eq!(lookup("dyn"), Some(TokenKind::Dyn));
    assert_eq!(lookup("extend"), Some(TokenKind::Extend));
    assert_eq!(lookup("extension"), Some(TokenKind::Extension));
    assert_eq!(lookup("skip"), Some(TokenKind::Skip));
    assert_eq!(lookup("div"), Some(TokenKind::Div));
    assert_eq!(lookup("self"), Some(TokenKind::SelfLower));
    assert_eq!(lookup("Self"), Some(TokenKind::SelfUpper));
    assert_eq!(lookup("use"), Some(TokenKind::Use));
    assert_eq!(lookup("uses"), Some(TokenKind::Uses));
    assert_eq!(lookup("as"), Some(TokenKind::As));
    assert_eq!(lookup("where"), Some(TokenKind::Where));
    assert_eq!(lookup("with"), Some(TokenKind::With));
    assert_eq!(lookup("suspend"), Some(TokenKind::Suspend));
    assert_eq!(lookup("unsafe"), Some(TokenKind::Unsafe));
    assert_eq!(lookup("extern"), Some(TokenKind::Extern));
}

// === Soft keywords are NOT in the reserved table ===

#[test]
fn soft_keywords_not_in_reserved_table() {
    assert_eq!(lookup("cache"), None);
    assert_eq!(lookup("catch"), None);
    assert_eq!(lookup("parallel"), None);
    assert_eq!(lookup("spawn"), None);
    assert_eq!(lookup("recurse"), None);
    assert_eq!(lookup("timeout"), None);
}

// === Soft keyword lookup tests ===

#[test]
fn soft_keyword_with_lparen() {
    assert_eq!(soft_keyword_lookup("cache", b"(x)"), Some(TokenKind::Cache));
    assert_eq!(
        soft_keyword_lookup("catch", b"(err)"),
        Some(TokenKind::Catch)
    );
    assert_eq!(
        soft_keyword_lookup("parallel", b"(tasks)"),
        Some(TokenKind::Parallel)
    );
    assert_eq!(
        soft_keyword_lookup("spawn", b"(task)"),
        Some(TokenKind::Spawn)
    );
    assert_eq!(
        soft_keyword_lookup("recurse", b"(n)"),
        Some(TokenKind::Recurse)
    );
    assert_eq!(
        soft_keyword_lookup("timeout", b"(5s, task)"),
        Some(TokenKind::Timeout)
    );
}

#[test]
fn soft_keyword_without_lparen() {
    // No `(` follows → identifier
    assert_eq!(soft_keyword_lookup("cache", b" = 42"), None);
    assert_eq!(soft_keyword_lookup("catch", b".field"), None);
    assert_eq!(soft_keyword_lookup("parallel", b""), None);
    assert_eq!(soft_keyword_lookup("spawn", b"\n(x)"), None);
    assert_eq!(soft_keyword_lookup("recurse", b" + 1"), None);
    assert_eq!(soft_keyword_lookup("timeout", b": int"), None);
}

#[test]
fn soft_keyword_with_space_before_lparen() {
    // Space before `(` → still keyword
    assert_eq!(
        soft_keyword_lookup("cache", b" (x)"),
        Some(TokenKind::Cache)
    );
    assert_eq!(
        soft_keyword_lookup("catch", b"  (err)"),
        Some(TokenKind::Catch)
    );
}

#[test]
fn soft_keyword_with_tab_before_lparen() {
    // Tab before `(` → still keyword
    assert_eq!(
        soft_keyword_lookup("cache", b"\t(x)"),
        Some(TokenKind::Cache)
    );
    assert_eq!(
        soft_keyword_lookup("parallel", b"\t\t(tasks)"),
        Some(TokenKind::Parallel)
    );
}

#[test]
fn soft_keyword_with_newline_before_lparen() {
    // Newline before `(` → identifier (not keyword)
    assert_eq!(soft_keyword_lookup("cache", b"\n(x)"), None);
    assert_eq!(soft_keyword_lookup("spawn", b"\r\n(x)"), None);
}

#[test]
fn soft_keyword_non_keyword_text() {
    // Text that isn't a soft keyword at all
    assert_eq!(soft_keyword_lookup("foo", b"(x)"), None);
    assert_eq!(soft_keyword_lookup("let", b"(x)"), None);
    assert_eq!(soft_keyword_lookup("if", b"(x)"), None);
}

// === Edge cases ===

#[test]
fn non_keywords_return_none() {
    assert_eq!(lookup("foo"), None);
    assert_eq!(lookup("bar"), None);
    assert_eq!(lookup("x"), None);
    assert_eq!(lookup("my_var"), None);
}

#[test]
fn case_sensitivity() {
    // Keywords are case-sensitive
    assert_eq!(lookup("If"), None);
    assert_eq!(lookup("IF"), None);
    assert_eq!(lookup("TRUE"), None);
    assert_eq!(lookup("False"), None);

    // But Self is uppercase
    assert_eq!(lookup("Self"), Some(TokenKind::SelfUpper));
    assert_eq!(lookup("self"), Some(TokenKind::SelfLower));

    // Never is uppercase
    assert_eq!(lookup("Never"), Some(TokenKind::NeverType));
    assert_eq!(lookup("never"), None);
}

#[test]
fn reserved_keywords_recognized() {
    assert_eq!(lookup("extern"), Some(TokenKind::Extern));
    assert_eq!(lookup("suspend"), Some(TokenKind::Suspend));
    assert_eq!(lookup("unsafe"), Some(TokenKind::Unsafe));
}

#[test]
fn empty_string_is_not_keyword() {
    assert_eq!(lookup(""), None);
}

#[test]
fn single_char_is_not_keyword() {
    assert_eq!(lookup("a"), None);
    assert_eq!(lookup("i"), None);
    assert_eq!(lookup("x"), None);
}

#[test]
fn length_boundary_rejection() {
    // Strings longer than 11 chars are rejected immediately
    assert_eq!(lookup("unreachable_"), None);
    assert_eq!(lookup("unreachables"), None);
}

#[test]
fn non_alpha_start_rejection() {
    // Keywords must start with ASCII alpha
    assert_eq!(lookup("_if"), None);
    assert_eq!(lookup("1let"), None);
}

// === has_lparen_lookahead edge cases ===

#[test]
fn lparen_lookahead_empty_rest() {
    assert!(!has_lparen_lookahead(b""));
}

#[test]
fn lparen_lookahead_immediate() {
    assert!(has_lparen_lookahead(b"("));
}

#[test]
fn lparen_lookahead_with_mixed_whitespace() {
    assert!(has_lparen_lookahead(b" \t (x)"));
}

#[test]
fn lparen_lookahead_stops_at_non_whitespace() {
    assert!(!has_lparen_lookahead(b"x("));
    assert!(!has_lparen_lookahead(b"// comment\n("));
}

// === Reserved-future keyword tests ===

#[test]
fn reserved_future_keywords_detected() {
    assert_eq!(reserved_future_lookup("asm"), Some("asm"));
    assert_eq!(reserved_future_lookup("inline"), Some("inline"));
    assert_eq!(reserved_future_lookup("static"), Some("static"));
    assert_eq!(reserved_future_lookup("union"), Some("union"));
    assert_eq!(reserved_future_lookup("view"), Some("view"));
}

#[test]
fn non_reserved_future_returns_none() {
    assert_eq!(reserved_future_lookup("let"), None);
    assert_eq!(reserved_future_lookup("foo"), None);
    assert_eq!(reserved_future_lookup(""), None);
    assert_eq!(reserved_future_lookup("Static"), None); // case-sensitive
}

// === Pre-filter tests ===

#[test]
fn could_be_soft_keyword_accepts_all_soft_keywords() {
    assert!(could_be_soft_keyword("cache")); // len=5, starts with 'c'
    assert!(could_be_soft_keyword("catch")); // len=5, starts with 'c'
    assert!(could_be_soft_keyword("spawn")); // len=5, starts with 's'
    assert!(could_be_soft_keyword("recurse")); // len=7, starts with 'r'
    assert!(could_be_soft_keyword("timeout")); // len=7, starts with 't'
    assert!(could_be_soft_keyword("parallel")); // len=8, starts with 'p'
}

#[test]
fn could_be_soft_keyword_rejects_wrong_length() {
    assert!(!could_be_soft_keyword("if")); // len=2
    assert!(!could_be_soft_keyword("let")); // len=3
    assert!(!could_be_soft_keyword("self")); // len=4
    assert!(!could_be_soft_keyword("return")); // len=6
    assert!(!could_be_soft_keyword("extension")); // len=9
}

#[test]
fn could_be_soft_keyword_rejects_wrong_first_byte() {
    assert!(!could_be_soft_keyword("match")); // len=5, starts with 'm'
    assert!(!could_be_soft_keyword("break")); // len=5, starts with 'b'
    assert!(!could_be_soft_keyword("async")); // len=5, starts with 'a'
}

#[test]
fn could_be_reserved_future_accepts_all_reserved_future() {
    assert!(could_be_reserved_future("asm")); // len=3, starts with 'a'
    assert!(could_be_reserved_future("view")); // len=4, starts with 'v'
    assert!(could_be_reserved_future("union")); // len=5, starts with 'u'
    assert!(could_be_reserved_future("inline")); // len=6, starts with 'i'
    assert!(could_be_reserved_future("static")); // len=6, starts with 's'
}

#[test]
fn could_be_reserved_future_rejects_wrong_length() {
    assert!(!could_be_reserved_future("if")); // len=2
    assert!(!could_be_reserved_future("suspend")); // len=7
    assert!(!could_be_reserved_future("parallel")); // len=8
}

#[test]
fn could_be_reserved_future_rejects_wrong_first_byte() {
    assert!(!could_be_reserved_future("def")); // len=3, starts with 'd'
    assert!(!could_be_reserved_future("loop")); // len=4, starts with 'l'
    assert!(!could_be_reserved_future("match")); // len=5, starts with 'm'
}
