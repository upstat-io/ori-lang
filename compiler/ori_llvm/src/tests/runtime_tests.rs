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
    runtime::ori_print(&raw const s);
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
    // Use 1.23456 to avoid clippy::approx_constant warning (for PI, E, etc.)
    runtime::ori_print_float(1.23456);
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

    assert!(runtime::ori_str_eq(&raw const s1, &raw const s2));
    assert!(!runtime::ori_str_eq(&raw const s1, &raw const s3));
}

#[test]
fn test_ori_str_ne() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"world");

    assert!(runtime::ori_str_ne(&raw const s1, &raw const s2));
}

#[test]
fn test_ori_str_eq_different_lengths() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"hell");

    assert!(!runtime::ori_str_eq(&raw const s1, &raw const s2));
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
    runtime::ori_assert_eq_str(&raw const s1, &raw const s2);
}

#[test]
fn test_ori_assert_eq_str_fails() {
    runtime::reset_panic_state();
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"world");
    runtime::ori_assert_eq_str(&raw const s1, &raw const s2);
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
    // Use 2.5 instead of 3.14 to avoid clippy::approx_constant warning
    let result = runtime::ori_str_from_float(2.5);
    assert!(result.len > 0);
    assert!(!result.data.is_null());

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert!(s.starts_with("2.5"));
}

#[test]
#[allow(unsafe_code)]
fn test_ori_str_concat() {
    let s1 = make_ori_str(b"hello");
    let s2 = make_ori_str(b"world");

    let result = runtime::ori_str_concat(&raw const s1, &raw const s2);
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

    let result = runtime::ori_str_concat(&raw const s1, &raw const s2);
    assert_eq!(result.len, 4);

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "test");
}

#[test]
#[allow(unsafe_code)]
fn test_ori_closure_box_allocates_memory() {
    // Test allocation of various sizes
    let ptr1 = runtime::ori_closure_box(24); // closure with 1 capture: 1 + 8 + 8 + 8 (aligned)
    let ptr2 = runtime::ori_closure_box(40); // closure with 3 captures

    assert!(!ptr1.is_null());
    assert!(!ptr2.is_null());

    // Verify pointers are aligned to 8 bytes
    assert_eq!(ptr1 as usize % 8, 0);
    assert_eq!(ptr2 as usize % 8, 0);

    // Write and read back to verify memory is usable
    // We're guaranteed 8-byte alignment from ori_closure_box.
    unsafe {
        // Write a pattern to the memory
        *ptr1 = 42;

        // For the i64 write at offset 8, we need to ensure alignment.
        // Since ptr1 is 8-byte aligned and we add 8 bytes, the result is 8-byte aligned.
        // Use write_unaligned to satisfy clippy, even though we know it's aligned.
        let i64_ptr = ptr1.add(8);
        std::ptr::write_unaligned(i64_ptr.cast::<i64>(), 12345);

        // Read back
        assert_eq!(*ptr1, 42);
        assert_eq!(std::ptr::read_unaligned(i64_ptr.cast::<i64>()), 12345);
    }

    // Clean up (normally closures would be freed by GC or explicit dealloc)
    unsafe {
        let layout1 = std::alloc::Layout::from_size_align(24, 8).unwrap();
        let layout2 = std::alloc::Layout::from_size_align(40, 8).unwrap();
        std::alloc::dealloc(ptr1, layout1);
        std::alloc::dealloc(ptr2, layout2);
    }
}

#[test]
#[allow(unsafe_code)]
fn test_ori_closure_box_minimum_size() {
    // Even if size is very small, should allocate at least 8 bytes
    let ptr = runtime::ori_closure_box(1);
    assert!(!ptr.is_null());

    // Clean up
    unsafe {
        let layout = std::alloc::Layout::from_size_align(8, 8).unwrap();
        std::alloc::dealloc(ptr, layout);
    }
}
