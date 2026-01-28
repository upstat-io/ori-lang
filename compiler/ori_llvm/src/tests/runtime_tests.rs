//! Additional tests for runtime functions.

use crate::runtime::{self, OriStr};

fn make_ori_str(s: &[u8]) -> OriStr {
    OriStr {
        len: s.len() as i64,
        data: s.as_ptr(),
    }
}

#[test]
fn test_ori_print() {
    let s = make_ori_str(b"hello");
    // Just verify it doesn't panic
    runtime::ori_print(&s);
}

#[test]
fn test_ori_print_bool_true() {
    runtime::ori_print_bool(true);
}

#[test]
fn test_ori_print_bool_false() {
    runtime::ori_print_bool(false);
}

#[test]
fn test_ori_print_float() {
    runtime::ori_print_float(3.14159);
}

#[test]
fn test_ori_list_operations() {
    let list = runtime::ori_list_new(10, 8); // 10 capacity, 8 bytes per element
    assert!(!list.is_null());

    let len = runtime::ori_list_len(list);
    assert_eq!(len, 0);

    runtime::ori_list_free(list, 8);
}

#[test]
fn test_ori_compare_int_less() {
    let result = runtime::ori_compare_int(1, 5);
    assert_eq!(result, -1);
}

#[test]
fn test_ori_compare_int_equal() {
    let result = runtime::ori_compare_int(5, 5);
    assert_eq!(result, 0);
}

#[test]
fn test_ori_compare_int_greater() {
    let result = runtime::ori_compare_int(10, 5);
    assert_eq!(result, 1);
}

#[test]
fn test_ori_min_int() {
    assert_eq!(runtime::ori_min_int(3, 7), 3);
    assert_eq!(runtime::ori_min_int(7, 3), 3);
    assert_eq!(runtime::ori_min_int(5, 5), 5);
    assert_eq!(runtime::ori_min_int(-10, 5), -10);
}

#[test]
fn test_ori_max_int() {
    assert_eq!(runtime::ori_max_int(3, 7), 7);
    assert_eq!(runtime::ori_max_int(7, 3), 7);
    assert_eq!(runtime::ori_max_int(5, 5), 5);
    assert_eq!(runtime::ori_max_int(-10, 5), 5);
}

#[test]
fn test_ori_str_eq() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"hello");
    let s3 = make_ori_str(b"world");

    assert!(runtime::ori_str_eq(&s1, &s2));
    assert!(!runtime::ori_str_eq(&s1, &s3));
}

#[test]
fn test_ori_str_ne() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"world");

    assert!(runtime::ori_str_ne(&s1, &s2));
}

#[test]
fn test_ori_str_eq_different_lengths() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"hell");

    assert!(!runtime::ori_str_eq(&s1, &s2));
}

#[test]
fn test_ori_assert_passes() {
    runtime::ori_assert(true);
}

#[test]
fn test_ori_assert_fails() {
    runtime::reset_panic_state();
    runtime::ori_assert(false);
    assert!(runtime::did_panic());
}

#[test]
fn test_ori_assert_eq_int_passes() {
    runtime::ori_assert_eq_int(42, 42);
}

#[test]
fn test_ori_assert_eq_int_fails_different() {
    runtime::reset_panic_state();
    runtime::ori_assert_eq_int(1, 2);
    assert!(runtime::did_panic());
}

#[test]
fn test_ori_assert_eq_bool_passes() {
    runtime::ori_assert_eq_bool(true, true);
    runtime::ori_assert_eq_bool(false, false);
}

#[test]
fn test_ori_assert_eq_bool_fails() {
    runtime::reset_panic_state();
    runtime::ori_assert_eq_bool(true, false);
    assert!(runtime::did_panic());
}

#[test]
fn test_ori_assert_eq_str_passes() {
    let s1 = make_ori_str(b"test");
    let s2 = make_ori_str(b"test");
    runtime::ori_assert_eq_str(&s1, &s2);
}

#[test]
fn test_ori_assert_eq_str_fails() {
    runtime::reset_panic_state();
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"world");
    runtime::ori_assert_eq_str(&s1, &s2);
    assert!(runtime::did_panic());
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_from_int() {
    let result = runtime::ori_str_from_int(42);
    assert!(result.len > 0);
    assert!(!result.data.is_null());

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "42");
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_from_int_negative() {
    let result = runtime::ori_str_from_int(-123);
    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "-123");
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_from_bool_true() {
    let result = runtime::ori_str_from_bool(true);
    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "true");
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_from_bool_false() {
    let result = runtime::ori_str_from_bool(false);
    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "false");
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_from_float() {
    let result = runtime::ori_str_from_float(3.14);
    assert!(result.len > 0);
    assert!(!result.data.is_null());

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert!(s.starts_with("3.14"));
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_concat() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"world");

    let result = runtime::ori_str_concat(&s1, &s2);
    assert_eq!(result.len, 10);

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "helloworld");
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_concat_empty() {
    let s1 = make_ori_str(b"");
    let s2 = make_ori_str(b"test");

    let result = runtime::ori_str_concat(&s1, &s2);
    assert_eq!(result.len, 4);

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "test");
}
