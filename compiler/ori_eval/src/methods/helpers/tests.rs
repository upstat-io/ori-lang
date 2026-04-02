use ori_ir::StringInterner;

use super::*;

fn interner() -> StringInterner {
    StringInterner::new()
}

#[test]
fn escape_debug_str_no_special_chars() {
    assert_eq!(escape_debug_str("hello"), "hello");
}

#[test]
fn escape_debug_str_newline() {
    assert_eq!(escape_debug_str("a\nb"), "a\\nb");
}

#[test]
fn escape_debug_str_tab() {
    assert_eq!(escape_debug_str("a\tb"), "a\\tb");
}

#[test]
fn escape_debug_str_backslash() {
    assert_eq!(escape_debug_str("a\\b"), "a\\\\b");
}

#[test]
fn escape_debug_str_double_quote() {
    assert_eq!(escape_debug_str("a\"b"), "a\\\"b");
}

#[test]
fn escape_debug_str_carriage_return() {
    assert_eq!(escape_debug_str("a\rb"), "a\\rb");
}

#[test]
fn escape_debug_str_null() {
    assert_eq!(escape_debug_str("a\0b"), "a\\0b");
}

#[test]
fn escape_debug_str_mixed() {
    assert_eq!(
        escape_debug_str("line1\nline2\t\"quoted\""),
        "line1\\nline2\\t\\\"quoted\\\""
    );
}

#[test]
fn escape_debug_char_basic() {
    assert_eq!(escape_debug_char('a'), "a");
}

#[test]
fn escape_debug_char_newline() {
    assert_eq!(escape_debug_char('\n'), "\\n");
}

#[test]
fn escape_debug_char_tab() {
    assert_eq!(escape_debug_char('\t'), "\\t");
}

#[test]
fn escape_debug_char_backslash() {
    assert_eq!(escape_debug_char('\\'), "\\\\");
}

#[test]
fn escape_debug_char_single_quote() {
    assert_eq!(escape_debug_char('\''), "\\'");
}

#[test]
fn escape_debug_char_null() {
    assert_eq!(escape_debug_char('\0'), "\\0");
}

#[test]
fn debug_value_int() {
    assert_eq!(debug_value(&Value::int(42), &interner()), "42");
}

#[test]
fn debug_value_float() {
    assert_eq!(debug_value(&Value::Float(2.72), &interner()), "2.72");
}

#[test]
fn debug_value_bool() {
    assert_eq!(debug_value(&Value::Bool(true), &interner()), "true");
    assert_eq!(debug_value(&Value::Bool(false), &interner()), "false");
}

#[test]
fn debug_value_string_escapes() {
    assert_eq!(
        debug_value(&Value::string("hello\nworld".to_string()), &interner()),
        "\"hello\\nworld\""
    );
}

#[test]
fn debug_value_char_escapes() {
    assert_eq!(debug_value(&Value::Char('\n'), &interner()), "'\\n'");
    assert_eq!(debug_value(&Value::Char('a'), &interner()), "'a'");
}

#[test]
fn debug_value_byte() {
    assert_eq!(debug_value(&Value::Byte(0x2a), &interner()), "0x2a");
}

#[test]
fn debug_value_void() {
    assert_eq!(debug_value(&Value::Void, &interner()), "void");
}

#[test]
fn debug_value_none() {
    assert_eq!(debug_value(&Value::None, &interner()), "None");
}

#[test]
fn debug_value_some() {
    assert_eq!(
        debug_value(&Value::some(Value::int(42)), &interner()),
        "Some(42)"
    );
}

#[test]
fn debug_value_ok() {
    assert_eq!(debug_value(&Value::ok(Value::int(1)), &interner()), "Ok(1)");
}

#[test]
fn debug_value_err() {
    assert_eq!(
        debug_value(&Value::err(Value::string("oops".to_string())), &interner()),
        "Err(\"oops\")"
    );
}

#[test]
fn debug_value_list() {
    let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
    assert_eq!(debug_value(&list, &interner()), "[1, 2, 3]");
}

#[test]
fn debug_value_list_with_strings() {
    let list = Value::list(vec![
        Value::string("a".to_string()),
        Value::string("b".to_string()),
    ]);
    assert_eq!(debug_value(&list, &interner()), "[\"a\", \"b\"]");
}

#[test]
fn debug_value_tuple() {
    let tuple = Value::tuple(vec![Value::int(1), Value::Bool(true)]);
    assert_eq!(debug_value(&tuple, &interner()), "(1, true)");
}

#[test]
fn debug_value_nested_some() {
    let val = Value::some(Value::string("hi\tthere".to_string()));
    assert_eq!(debug_value(&val, &interner()), "Some(\"hi\\tthere\")");
}
