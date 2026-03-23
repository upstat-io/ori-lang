//! Struct/sum/tuple iteration tests — elements with Drop fields.

use crate::util::assert_aot_success;

// Struct with string fields

#[test]
fn test_struct_with_str_field_iteration() {
    assert_aot_success(
        r#"
type Person = { name: str, age: int }

@main () -> int = {
    let people = [
        Person { name: "this is a very long name exceeding SSO threshold", age: 30 },
        Person { name: "another very long name exceeding SSO threshold too", age: 25 }
    ];
    let total_age = 0;
    for p in people do {
        total_age = total_age + p.age;
    };
    if total_age == 55 then 0 else 1
}
"#,
        "struct_with_str_field_iteration",
    );
}

// T6: [Result<str, str>] — fat pointer in both Ok and Err variants

#[test]
fn test_result_str_list_iteration() {
    // Both Ok and Err payloads are heap strings needing elem_dec_fn cleanup.
    assert_aot_success(
        r#"
@main () -> int = {
    let results: [Result<str, str>] = [
        Ok("this is a very long ok string that exceeds SSO threshold"),
        Err("this is a very long error string exceeding SSO threshold"),
        Ok("another very long ok string also exceeds the SSO thresh")
    ];
    let total = 0;
    for r in results do {
        match r {
            Ok(s) -> { total = total + s.len(); },
            Err(e) -> { total = total + e.len(); }
        };
    };
    // 56 + 56 + 55 = 167
    if total == 167 then 0 else 1
}
"#,
        "result_str_list_iteration",
    );
}

// T7: [(str, int)] — tuple with a string component

#[test]
fn test_tuple_str_list_iteration() {
    // Tuple elements with a fat pointer component need elem_dec_fn for the str field.
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        ("this is a very long string that exceeds SSO threshold", 10),
        ("another very long string that also exceeds the threshold", 20)
    ];
    let total = 0;
    for (s, n) in items do {
        total = total + s.len() + n;
    };
    // (53 + 10) + (56 + 20) = 139
    if total == 139 then 0 else 1
}
"#,
        "tuple_str_list_iteration",
    );
}

// F2: Break — partial iteration with un-consumed element cleanup

#[test]
fn test_option_str_list_break() {
    // T5-F2: Option<str> partial iteration — un-consumed Some(str) must be cleaned up.
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [Option<str>] = [
        Some("this is a very long string that exceeds SSO threshold"),
        Some("stop marker string that is also long enough to be heap"),
        None,
        Some("fourth long string not visited because we broke early")
    ];
    let count = 0;
    for item in items do {
        match item {
            Some(s) -> {
                if s.starts_with(prefix: "stop") then break;
                count = count + 1;
            },
            None -> { count = count + 1; }
        };
    };
    if count == 1 then 0 else 1
}
"#,
        "option_str_list_break",
    );
}

#[test]
fn test_result_str_list_break() {
    // T6-F2: Result<str, str> partial iteration — un-consumed variants cleaned up.
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [Result<str, str>] = [
        Ok("this is a very long ok string that exceeds SSO threshold"),
        Err("stop error string that is also long enough for the heap"),
        Ok("third long string not visited because we broke on error")
    ];
    let count = 0;
    for r in items do {
        match r {
            Err(_) -> break,
            Ok(_) -> { count = count + 1; }
        };
    };
    if count == 1 then 0 else 1
}
"#,
        "result_str_list_break",
    );
}

#[test]
fn test_tuple_str_list_break() {
    // T7-F2: (str, int) partial iteration — un-consumed tuple elements cleaned up.
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        ("this is a very long string that exceeds SSO threshold", 1),
        ("stop marker string that is also long enough to be heap", 2),
        ("third long string not visited because we broke early ok", 3)
    ];
    let total = 0;
    for (s, n) in items do {
        if n == 2 then break;
        total = total + s.len();
    };
    // Only first tuple: 53
    if total == 53 then 0 else 1
}
"#,
        "tuple_str_list_break",
    );
}

// F5: Function parameter — pass collection to function, call twice

#[test]
fn test_option_str_list_two_calls() {
    // T5-F5: [Option<str>] passed to function twice.
    assert_aot_success(
        r#"
@count_some (items: [Option<str>]) -> int = {
    let count = 0;
    for item in items do {
        match item {
            Some(_) -> { count = count + 1; },
            None -> { 0; }
        };
    };
    count
}

@main () -> int = {
    let items: [Option<str>] = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string that also exceeds the threshold")
    ];
    let a = count_some(items: items);
    let b = count_some(items: items);
    if a == 2 && b == 2 then 0 else 1
}
"#,
        "option_str_list_two_calls",
    );
}

#[test]
fn test_result_str_list_two_calls() {
    // T6-F5: [Result<str, str>] passed to function twice.
    assert_aot_success(
        r#"
@count_ok (items: [Result<str, str>]) -> int = {
    let count = 0;
    for r in items do {
        match r {
            Ok(_) -> { count = count + 1; },
            Err(_) -> { 0; }
        };
    };
    count
}

@main () -> int = {
    let items: [Result<str, str>] = [
        Ok("this is a very long ok string that exceeds SSO threshold"),
        Err("this is a very long error string exceeding SSO threshold"),
        Ok("another very long ok string also exceeds the SSO thresh")
    ];
    let a = count_ok(items: items);
    let b = count_ok(items: items);
    if a == 2 && b == 2 then 0 else 1
}
"#,
        "result_str_list_two_calls",
    );
}

#[test]
fn test_tuple_str_list_two_calls() {
    // T7-F5: [(str, int)] passed to function twice.
    assert_aot_success(
        r#"
@sum_values (items: [(str, int)]) -> int = {
    let total = 0;
    for (_, n) in items do {
        total = total + n;
    };
    total
}

@main () -> int = {
    let items = [
        ("this is a very long string that exceeds SSO threshold", 10),
        ("another very long string that also exceeds the threshold", 20)
    ];
    let a = sum_values(items: items);
    let b = sum_values(items: items);
    if a == 30 && b == 30 then 0 else 1
}
"#,
        "tuple_str_list_two_calls",
    );
}
