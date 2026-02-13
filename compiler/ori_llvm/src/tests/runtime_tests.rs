//! Additional tests for runtime functions.

#![allow(
    clippy::items_after_statements,
    clippy::cast_ptr_alignment,
    clippy::ptr_cast_constness,
    reason = "FFI test code requires inline drop functions and raw pointer operations"
)]

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
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
fn test_ori_str_from_int() {
    let result = runtime::ori_str_from_int(42);
    assert!(result.len > 0);
    assert!(!result.data.is_null());

    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "42");
}

#[test]
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
fn test_ori_str_from_int_negative() {
    let result = runtime::ori_str_from_int(-123);
    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "-123");
}

#[test]
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
fn test_ori_str_from_bool_true() {
    let result = runtime::ori_str_from_bool(true);
    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "true");
}

#[test]
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
fn test_ori_str_from_bool_false() {
    let result = runtime::ori_str_from_bool(false);
    let slice = unsafe { std::slice::from_raw_parts(result.data, result.len as usize) };
    let s = std::str::from_utf8(slice).unwrap();
    assert_eq!(s, "false");
}

#[test]
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
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
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
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
#[allow(
    unsafe_code,
    reason = "runtime FFI returns raw pointers; unsafe needed to read results"
)]
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
#[allow(
    unsafe_code,
    reason = "tests ARC runtime via raw pointer allocation and deallocation"
)]
fn test_rc_alloc_and_data_pointer() {
    // V2: ori_rc_alloc returns data pointer directly (no separate ori_rc_data)
    let data = runtime::ori_rc_alloc(24, 8); // env with 3 × i64 captures
    assert!(!data.is_null());

    // Refcount at data_ptr - 8 should be 1
    assert_eq!(runtime::ori_rc_count(data), 1);

    // Write captures and read back
    unsafe {
        std::ptr::write_unaligned(data.cast::<i64>(), 111);
        std::ptr::write_unaligned(data.add(8).cast::<i64>(), 222);
        std::ptr::write_unaligned(data.add(16).cast::<i64>(), 333);
        assert_eq!(std::ptr::read_unaligned(data.cast::<i64>()), 111);
        assert_eq!(std::ptr::read_unaligned(data.add(8).cast::<i64>()), 222);
        assert_eq!(std::ptr::read_unaligned(data.add(16).cast::<i64>()), 333);
    }

    // Decrement → refcount reaches 0 → calls drop_fn
    // Use a trivial drop function that calls ori_rc_free
    extern "C" fn drop_env_24(data_ptr: *mut u8) {
        runtime::ori_rc_free(data_ptr, 24, 8);
    }
    runtime::ori_rc_dec(data, Some(drop_env_24));
}

#[test]
#[allow(
    unsafe_code,
    reason = "tests ARC runtime via raw pointer allocation and deallocation"
)]
fn test_rc_inc_dec_lifecycle() {
    let data = runtime::ori_rc_alloc(8, 8);
    assert!(!data.is_null());
    assert_eq!(runtime::ori_rc_count(data), 1);

    // Increment twice
    runtime::ori_rc_inc(data);
    assert_eq!(runtime::ori_rc_count(data), 2);
    runtime::ori_rc_inc(data);
    assert_eq!(runtime::ori_rc_count(data), 3);

    // Decrement — not freed yet (refcount > 0)
    extern "C" fn drop_8(data_ptr: *mut u8) {
        runtime::ori_rc_free(data_ptr, 8, 8);
    }
    runtime::ori_rc_dec(data, Some(drop_8));
    assert_eq!(runtime::ori_rc_count(data), 2);
    runtime::ori_rc_dec(data, Some(drop_8));
    assert_eq!(runtime::ori_rc_count(data), 1);

    // Final dec → refcount 0 → freed via drop_fn
    runtime::ori_rc_dec(data, Some(drop_8));
    // data is now freed — do not access
}

#[test]
fn test_rc_header_is_8_bytes() {
    // V2: strong_count is a single i64 (8 bytes), not 16-byte RcHeader
    let data = runtime::ori_rc_alloc(16, 8);
    assert!(!data.is_null());

    // The strong_count is at data - 8. Verify the offset:
    // base = data - 8, data = base + 8
    let base = unsafe { data.sub(8) };
    let rc_from_base = unsafe { *(base.cast::<i64>()) };
    assert_eq!(rc_from_base, 1, "strong_count at data_ptr - 8 should be 1");

    extern "C" fn drop_16(data_ptr: *mut u8) {
        runtime::ori_rc_free(data_ptr, 16, 8);
    }
    runtime::ori_rc_dec(data, Some(drop_16));
}

#[test]
fn test_ori_args_from_argv_null() {
    // Null argv → empty list
    let list = runtime::ori_args_from_argv(0, std::ptr::null());
    assert_eq!(list.len, 0);
    assert_eq!(list.cap, 0);
    assert!(list.data.is_null());
}

#[test]
fn test_ori_args_from_argv_no_user_args() {
    // argc=1 means only program name → empty list (spec: args excludes program name)
    let prog = b"./my_prog\0";
    let argv = [prog.as_ptr().cast::<i8>()];
    let list = runtime::ori_args_from_argv(1, argv.as_ptr());
    assert_eq!(list.len, 0);
    assert_eq!(list.cap, 0);
    assert!(list.data.is_null());
}

#[test]
#[allow(
    unsafe_code,
    reason = "tests argv FFI which requires constructing raw C string pointers"
)]
fn test_ori_args_from_argv_with_args() {
    let prog = b"./my_prog\0";
    let arg1 = b"hello\0";
    let arg2 = b"world\0";
    let argv = [
        prog.as_ptr().cast::<i8>(),
        arg1.as_ptr().cast::<i8>(),
        arg2.as_ptr().cast::<i8>(),
    ];
    let list = runtime::ori_args_from_argv(3, argv.as_ptr());
    assert_eq!(list.len, 2);
    assert_eq!(list.cap, 2);
    assert!(!list.data.is_null());

    // Read back the OriStr elements
    let elements = list.data.cast::<runtime::OriStr>();
    unsafe {
        let s0 = &*elements;
        let s1 = &*elements.add(1);
        assert_eq!(s0.as_str(), "hello");
        assert_eq!(s1.as_str(), "world");
    }

    // Clean up (in production, the runtime or RC handles this)
    unsafe {
        // Free each string's data
        for i in 0..2 {
            let s = &*elements.add(i);
            if !s.data.is_null() && s.len > 0 {
                let layout = std::alloc::Layout::array::<u8>(s.len as usize).unwrap();
                std::alloc::dealloc(s.data as *mut u8, layout);
            }
        }
        // Free the array
        let layout = std::alloc::Layout::array::<runtime::OriStr>(2).unwrap();
        std::alloc::dealloc(list.data, layout);
    }
}

#[test]
fn test_ori_register_panic_handler_null() {
    // Registering null should be a no-op (no crash)
    runtime::ori_register_panic_handler(std::ptr::null());
}
