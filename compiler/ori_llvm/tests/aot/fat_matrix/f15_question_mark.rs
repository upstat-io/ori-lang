//! F15: ? Propagation — fat pointer types used in Option/Result with ? operator.
//!
//! Tests early return codegen, cleanup on error path, and RC handling when fat
//! pointer values are wrapped in Option/Result and propagated with ?.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T15: ? on Option<int> — Some path
#[test]
fn test_fm_question_option_int_some() {
    assert_aot_success(
        r#"
@extract (opt: Option<int>) -> Option<int> = {
    let v = opt?;
    Some(v + 1)
};

@main () -> int = {
    match extract(opt: Some(9)) {
        Some(v) -> if v == 10 then 0 else 1,
        None -> 2,
    }
}
"#,
        "fm_question_option_int_some",
    );
}

// T15: ? on Option<int> — None path
#[test]
fn test_fm_question_option_int_none() {
    assert_aot_success(
        r#"
@extract (opt: Option<int>) -> Option<int> = {
    let v = opt?;
    Some(v + 1)
};

@main () -> int = {
    match extract(opt: None) {
        Some(_) -> 1,
        None -> 0,
    }
}
"#,
        "fm_question_option_int_none",
    );
}

// T16: ? on Option<str> — Some path
#[test]
fn test_fm_question_option_str_some() {
    assert_aot_success(
        r#"
@get_len (opt: Option<str>) -> Option<int> = {
    let s = opt?;
    Some(s.length())
};

@main () -> int = {
    match get_len(opt: Some("hello")) {
        Some(n) -> if n == 5 then 0 else 1,
        None -> 2,
    }
}
"#,
        "fm_question_option_str_some",
    );
}

// T16: ? on Option<str> — None path
#[test]
fn test_fm_question_option_str_none() {
    assert_aot_success(
        r#"
@get_len (opt: Option<str>) -> Option<int> = {
    let s = opt?;
    Some(s.length())
};

@main () -> int = {
    let x: Option<str> = None;
    match get_len(opt: x) {
        Some(_) -> 1,
        None -> 0,
    }
}
"#,
        "fm_question_option_str_none",
    );
}

// ? with fat value in scope that must be cleaned up on early return
#[test]
fn test_fm_question_cleanup_fat_scope() {
    assert_aot_success(
        r#"
@process (opt: Option<int>) -> Option<int> = {
    let name = "abcdefghijklmnopqrstuvwxyz1234";
    let v = opt?;
    Some(v + name.length())
};

@main () -> int = {
    match process(opt: None) {
        Some(_) -> 1,
        None -> 0,
    }
}
"#,
        "fm_question_cleanup_fat_scope",
    );
}

// ? with fat value produced after successful extraction
#[test]
fn test_fm_question_fat_after_extract() {
    assert_aot_success(
        r#"
@process (opt: Option<int>) -> Option<int> = {
    let v = opt?;
    let name = "hello";
    Some(v + name.length())
};

@main () -> int = {
    match process(opt: Some(5)) {
        Some(n) -> if n == 10 then 0 else 1,
        None -> 2,
    }
}
"#,
        "fm_question_fat_after_extract",
    );
}

// Multiple ? in same function with fat values
#[test]
fn test_fm_question_multiple() {
    assert_aot_success(
        r#"
@combine (a: Option<int>, b: Option<int>) -> Option<int> = {
    let x = a?;
    let y = b?;
    Some(x + y)
};

@main () -> int = {
    let r1 = combine(a: Some(3), b: Some(7));
    let r2 = combine(a: Some(3), b: None);
    let r3 = combine(a: None, b: Some(7));
    match r1 {
        Some(n) -> if n == 10 then {
            match r2 {
                Some(_) -> 1,
                None -> match r3 {
                    Some(_) -> 2,
                    None -> 0,
                },
            }
        } else 3,
        None -> 4,
    }
}
"#,
        "fm_question_multiple",
    );
}
