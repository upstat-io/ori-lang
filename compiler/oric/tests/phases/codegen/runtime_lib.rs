//! Tests for the Ori runtime library (`ori_rt`).
//!
//! These tests verify the C-ABI functions used by AOT-compiled Ori programs:
//! - Memory allocation (`alloc`, `free`, `realloc`)
//! - Reference counting (`rc_alloc`, `rc_inc`, `rc_dec`, `rc_free`)
//! - String operations (`concat`, `eq`, `from_int`, etc.)
//! - List operations (`new`, `free`, `len`)
//! - Panic/assertion handling

#![allow(
    unsafe_code,
    clippy::borrow_as_ptr,
    clippy::ptr_cast_constness,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::from_raw_with_void_ptr,
    clippy::cast_slice_from_raw_parts,
    reason = "FFI tests require unsafe code and raw pointer/cast operations"
)]

use ori_rt::{
    did_panic, get_panic_message, ori_alloc, ori_args_from_argv, ori_assert_eq_int,
    ori_compare_int, ori_free, ori_list_free, ori_list_len, ori_list_new, ori_max_int, ori_min_int,
    ori_print_int, ori_rc_alloc, ori_rc_count, ori_rc_dec, ori_rc_free, ori_rc_inc, ori_realloc,
    ori_register_panic_handler, ori_str_concat, ori_str_eq, ori_str_ne, reset_panic_state,
    set_panic_state_for_test, OriStr,
};
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
use ori_rt::{enter_jit_mode, leave_jit_mode, JmpBuf};

#[test]
fn test_ori_alloc_free() {
    let ptr = ori_alloc(1024, 8);
    assert!(!ptr.is_null());
    ori_free(ptr, 1024, 8);
}

#[test]
fn test_ori_alloc_zero_size() {
    let ptr = ori_alloc(0, 8);
    assert!(ptr.is_null());
}

#[test]
fn test_ori_realloc() {
    let ptr = ori_alloc(64, 8);
    assert!(!ptr.is_null());

    // Write some data
    unsafe {
        std::ptr::write(ptr, 42u8);
    }

    // Grow
    let new_ptr = ori_realloc(ptr, 64, 128, 8);
    assert!(!new_ptr.is_null());

    // Check data preserved
    unsafe {
        assert_eq!(std::ptr::read(new_ptr), 42u8);
    }

    ori_free(new_ptr, 128, 8);
}

#[test]
fn test_ori_realloc_null() {
    // Realloc null is like alloc
    let ptr = ori_realloc(std::ptr::null_mut(), 0, 64, 8);
    assert!(!ptr.is_null());
    ori_free(ptr, 64, 8);
}

#[test]
fn test_ori_realloc_to_zero() {
    let ptr = ori_alloc(64, 8);
    assert!(!ptr.is_null());

    // Shrink to 0 is like free
    let new_ptr = ori_realloc(ptr, 64, 0, 8);
    assert!(new_ptr.is_null());
}

#[test]
fn test_rc_alloc_inc_dec() {
    // V2: ori_rc_alloc returns data pointer directly (8-byte header at ptr-8)
    extern "C" fn drop_64(data_ptr: *mut u8) {
        ori_rc_free(data_ptr, 64, 8);
    }

    let data = ori_rc_alloc(64, 8);
    assert!(!data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2);

    ori_rc_dec(data, Some(drop_64));
    assert_eq!(ori_rc_count(data), 1);

    // Final dec frees it - don't access after
    ori_rc_dec(data, Some(drop_64));
}

#[test]
fn test_rc_data_write_read() {
    extern "C" fn drop_64(data_ptr: *mut u8) {
        ori_rc_free(data_ptr, 64, 8);
    }

    let data = ori_rc_alloc(64, 8);
    assert!(!data.is_null());

    // Write directly to data pointer (no separate ori_rc_data call in V2)
    unsafe {
        std::ptr::write(data, 123u8);
        assert_eq!(std::ptr::read(data), 123u8);
    }

    ori_rc_dec(data, Some(drop_64));
}

#[test]
fn test_rc_null_safety() {
    ori_rc_inc(std::ptr::null_mut());
    ori_rc_dec(std::ptr::null_mut(), None);
    assert_eq!(ori_rc_count(std::ptr::null()), 0);
    ori_rc_free(std::ptr::null_mut(), 0, 8); // Should not crash
}

#[test]
fn test_ori_print_int() {
    ori_print_int(42);
}

#[test]
fn test_ori_assert_eq_int_pass() {
    reset_panic_state();
    ori_assert_eq_int(42, 42);
    assert!(!did_panic());
}

// On MSVC, use ori_try_call (C++ try/catch) to catch the OriPanicException.
// On MSVC, _setjmp is a compiler intrinsic that expects a hidden frame pointer
// argument — calling it via Rust FFI corrupts the stack (exit code 0xc0000028).
// The thunk must be `C-unwind` because ori_panic throws a C++ exception that
// must propagate through the thunk into ori_try_call's catch handler.
#[cfg(all(target_os = "windows", target_env = "msvc"))]
#[test]
fn test_ori_assert_eq_int_fail() {
    extern "C" {
        fn ori_try_call(thunk: unsafe extern "C-unwind" fn(*mut u8), ctx: *mut u8) -> i64;
    }

    unsafe extern "C-unwind" fn assert_42_43_thunk(_ctx: *mut u8) {
        ori_assert_eq_int(42, 43);
    }

    reset_panic_state();
    // ori_try_call returns 1 on success, 0 if an Ori panic was caught
    let result = unsafe { ori_try_call(assert_42_43_thunk, std::ptr::null_mut()) };
    assert_eq!(result, 0, "ori_assert_eq_int(42, 43) should have panicked");
    assert!(
        did_panic(),
        "panic state should be set after assertion failure"
    );
}

// On Itanium targets, use JIT mode (setjmp/longjmp) to recover from the panic.
// catch_unwind cannot catch the Itanium foreign exceptions raised by
// ori_raise_exception on non-MSVC targets.
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
#[test]
fn test_ori_assert_eq_int_fail() {
    extern "C" {
        #[link_name = "_setjmp"]
        fn c_setjmp_direct(buf: *mut JmpBuf) -> i32;
    }

    reset_panic_state();
    // Call _setjmp directly (not through jit_setjmp wrapper) so the setjmp
    // save point is in THIS function's frame, not an intermediate wrapper.
    let mut jmp_buf = JmpBuf::new();
    let buf_ptr: *mut JmpBuf = &raw mut jmp_buf;
    enter_jit_mode(buf_ptr);

    // SAFETY: jmp_buf is stack-allocated and valid for the duration of this call.
    let val = unsafe { c_setjmp_direct(buf_ptr) };

    if val != 0 {
        // longjmp returned us here — ori_assert_eq_int hit a panic
        leave_jit_mode();
        assert!(
            did_panic(),
            "panic state should be set after assertion failure"
        );
        return;
    }

    // Normal path: this should panic (42 != 43)
    ori_assert_eq_int(42, 43);

    // Should not reach here
    leave_jit_mode();
    panic!("ori_assert_eq_int(42, 43) should have panicked");
}

#[test]
fn test_ori_compare_int() {
    assert_eq!(ori_compare_int(1, 2), -1);
    assert_eq!(ori_compare_int(2, 2), 0);
    assert_eq!(ori_compare_int(3, 2), 1);
}

#[test]
fn test_ori_min_max() {
    assert_eq!(ori_min_int(3, 5), 3);
    assert_eq!(ori_max_int(3, 5), 5);
}

#[test]
fn test_ori_str_concat() {
    let a = OriStr::from_sso(b"hello");
    let b = OriStr::from_sso(b" world");

    let result = ori_str_concat(&a, &b);
    assert_eq!(result.len(), 11);

    let text = unsafe { result.as_str() };
    assert_eq!(text, "hello world");
    // Result is SSO (11 ≤ 23), no heap to free
}

#[test]
fn test_ori_str_eq() {
    let a = OriStr::from_sso(b"hello");
    let b = OriStr::from_sso(b"hello");
    let c = OriStr::from_sso(b"world");

    assert!(ori_str_eq(&a, &b));
    assert!(!ori_str_eq(&a, &c));
    assert!(ori_str_ne(&a, &c));
}

#[test]
fn test_ori_list_new_free() {
    let list = ori_list_new(10, 8);
    assert!(!list.is_null());
    assert_eq!(ori_list_len(list), 0);
    ori_list_free(list, 8);
}

#[test]
fn test_ori_list_null_safety() {
    assert_eq!(ori_list_len(std::ptr::null()), 0);
    ori_list_free(std::ptr::null_mut(), 8); // Should not crash
}

#[test]
fn test_closure_env_via_rc() {
    extern "C" fn drop_64(data_ptr: *mut u8) {
        ori_rc_free(data_ptr, 64, 8);
    }

    // V2: ori_rc_alloc returns data pointer directly
    let data = ori_rc_alloc(64, 8);
    assert!(!data.is_null());
    assert_eq!(ori_rc_count(data), 1);
    ori_rc_dec(data, Some(drop_64)); // frees
}

#[test]
fn test_ori_args_empty() {
    let list = ori_args_from_argv(0, std::ptr::null());
    assert_eq!(list.len, 0);
}

#[test]
fn test_ori_register_panic_handler_null_safe() {
    ori_register_panic_handler(std::ptr::null());
}

#[test]
fn test_panic_state() {
    reset_panic_state();
    assert!(!did_panic());
    assert!(get_panic_message().is_none());

    // Use the test helper instead of ori_panic_cstr (which would exit the process)
    set_panic_state_for_test("test panic");
    assert!(did_panic());
    assert_eq!(get_panic_message(), Some("test panic".to_string()));

    reset_panic_state();
    assert!(!did_panic());
}
