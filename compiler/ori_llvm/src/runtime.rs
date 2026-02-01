//! Runtime functions for LLVM-compiled Ori code.
//!
//! These functions are called by LLVM-compiled code at runtime.
//! They're declared as `extern "C"` so LLVM can link to them.
//!
//! # Safety
//!
//! This module uses `#[no_mangle]` for FFI compatibility with LLVM JIT.
//! All functions are safe Rust but need stable symbol names.
//!
//! Functions that take raw pointers are called from LLVM-generated code which
//! guarantees valid pointers. They're not marked `unsafe` because they're
//! extern "C" FFI entry points, not Rust API functions.

#![allow(unsafe_code)]
// FFI functions dereference pointers from LLVM-generated code (always valid)
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::cell::RefCell;
use std::ffi::CStr;

// Thread-local panic state for test isolation.
// Each test thread gets its own panic tracking, preventing race conditions.
thread_local! {
    static PANIC_OCCURRED: RefCell<bool> = const { RefCell::new(false) };
    static PANIC_MESSAGE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Ori string representation: { i64 len, *const u8 data }
#[repr(C)]
pub struct OriStr {
    pub len: i64,
    pub data: *const u8,
}

impl OriStr {
    /// Convert to Rust string slice.
    ///
    /// # Safety
    /// Caller must ensure data pointer is valid and len is correct.
    #[allow(unsafe_code)]
    #[must_use]
    pub unsafe fn as_str(&self) -> &str {
        if self.data.is_null() || self.len <= 0 {
            return "";
        }
        let slice = std::slice::from_raw_parts(self.data, self.len as usize);
        std::str::from_utf8_unchecked(slice)
    }
}

/// Ori list representation: { i64 len, i64 cap, *mut u8 data }
#[repr(C)]
pub struct OriList {
    pub len: i64,
    pub cap: i64,
    pub data: *mut u8,
}

/// Ori Option representation: { i8 tag, T value }
/// tag = 0: None, tag = 1: Some
#[repr(C)]
pub struct OriOption<T> {
    pub tag: i8,
    pub value: T,
}

/// Ori Result representation: { i8 tag, T value }
/// tag = 0: Ok, tag = 1: Err
#[repr(C)]
pub struct OriResult<T> {
    pub tag: i8,
    pub value: T,
}

/// Print a string to stdout.
///
/// # Safety
/// This function is called from LLVM-compiled code.
#[no_mangle]
pub extern "C" fn ori_print(s: *const OriStr) {
    if s.is_null() {
        println!();
        return;
    }

    // SAFETY: Caller ensures s points to a valid OriStr
    let ori_str = unsafe { &*s };
    let text = unsafe { ori_str.as_str() };
    println!("{text}");
}

/// Print an integer to stdout.
#[no_mangle]
pub extern "C" fn ori_print_int(n: i64) {
    println!("{n}");
}

/// Print a float to stdout.
#[no_mangle]
pub extern "C" fn ori_print_float(f: f64) {
    println!("{f}");
}

/// Print a boolean to stdout.
#[no_mangle]
pub extern "C" fn ori_print_bool(b: bool) {
    println!("{b}");
}

/// Panic with a message.
///
/// # Safety
/// This function is called from LLVM-compiled code.
#[no_mangle]
pub extern "C" fn ori_panic(s: *const OriStr) {
    let msg = if s.is_null() {
        "panic!".to_string()
    } else {
        // SAFETY: Caller ensures s points to a valid OriStr
        let ori_str = unsafe { &*s };
        let text = unsafe { ori_str.as_str() };
        text.to_string()
    };

    // Store panic state in thread-local storage
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(msg.clone()));

    // Don't actually panic - just set the flag
    // This allows tests to check for expected panics
    eprintln!("ori panic: {msg}");
}

/// Panic with a C string message.
///
/// # Safety
/// This function is called from LLVM-compiled code.
#[no_mangle]
pub extern "C" fn ori_panic_cstr(s: *const i8) {
    let msg = if s.is_null() {
        "panic!".to_string()
    } else {
        // SAFETY: Caller ensures s points to a valid C string
        let cstr = unsafe { CStr::from_ptr(s) };
        cstr.to_string_lossy().to_string()
    };

    // Store panic state in thread-local storage
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(msg.clone()));

    eprintln!("ori panic: {msg}");
}

/// Check if a panic occurred (for test assertions).
pub fn did_panic() -> bool {
    PANIC_OCCURRED.with(|p| *p.borrow())
}

/// Get the panic message if one occurred.
pub fn get_panic_message() -> Option<String> {
    PANIC_MESSAGE.with(|m| m.borrow().clone())
}

/// Reset panic state (call before each test).
pub fn reset_panic_state() {
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = false);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = None);
}

/// Assert that a condition is true.
#[no_mangle]
pub extern "C" fn ori_assert(condition: bool) {
    if !condition {
        ori_panic_cstr(c"assertion failed".as_ptr());
    }
}

/// Assert that two integers are equal.
#[no_mangle]
pub extern "C" fn ori_assert_eq_int(actual: i64, expected: i64) {
    if actual != expected {
        eprintln!("assertion failed: {actual} != {expected}");
        PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
        PANIC_MESSAGE.with(|m| {
            *m.borrow_mut() = Some(format!("assertion failed: {actual} != {expected}"));
        });
    }
}

/// Assert that two booleans are equal.
#[no_mangle]
pub extern "C" fn ori_assert_eq_bool(actual: bool, expected: bool) {
    if actual != expected {
        eprintln!("assertion failed: {actual} != {expected}");
        PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
        PANIC_MESSAGE.with(|m| {
            *m.borrow_mut() = Some(format!("assertion failed: {actual} != {expected}"));
        });
    }
}

/// Allocate a new list with given capacity.
#[no_mangle]
pub extern "C" fn ori_list_new(capacity: i64, elem_size: i64) -> *mut OriList {
    let cap = capacity.max(0) as usize;
    let size = elem_size.max(1) as usize;

    // Allocate the list struct
    let list = Box::new(OriList {
        len: 0,
        cap: cap as i64,
        data: if cap > 0 {
            // Allocate data buffer
            let layout = std::alloc::Layout::array::<u8>(cap * size)
                .unwrap_or_else(|_| std::alloc::Layout::new::<u8>());
            // SAFETY: Layout is valid
            unsafe { std::alloc::alloc(layout) }
        } else {
            std::ptr::null_mut()
        },
    });

    Box::into_raw(list)
}

/// Free a list.
#[no_mangle]
pub extern "C" fn ori_list_free(list: *mut OriList, elem_size: i64) {
    if list.is_null() {
        return;
    }

    // SAFETY: Caller ensures list is valid
    unsafe {
        let list = Box::from_raw(list);
        if !list.data.is_null() && list.cap > 0 {
            let size = elem_size.max(1) as usize;
            let layout = std::alloc::Layout::array::<u8>(list.cap as usize * size)
                .unwrap_or_else(|_| std::alloc::Layout::new::<u8>());
            std::alloc::dealloc(list.data, layout);
        }
    }
}

/// Get the length of a list.
#[no_mangle]
pub extern "C" fn ori_list_len(list: *const OriList) -> i64 {
    if list.is_null() {
        return 0;
    }
    // SAFETY: Caller ensures list is valid
    unsafe { (*list).len }
}

/// Compare two integers (for sorting, etc.)
#[no_mangle]
pub extern "C" fn ori_compare_int(a: i64, b: i64) -> i32 {
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Get minimum of two integers.
#[no_mangle]
pub extern "C" fn ori_min_int(a: i64, b: i64) -> i64 {
    a.min(b)
}

/// Get maximum of two integers.
#[no_mangle]
pub extern "C" fn ori_max_int(a: i64, b: i64) -> i64 {
    a.max(b)
}

/// Allocate memory for a closure struct.
///
/// Used when a closure has captures and needs to be boxed for returning.
/// The size should be the total size of the closure struct in bytes.
#[no_mangle]
pub extern "C" fn ori_closure_box(size: i64) -> *mut u8 {
    let size = size.max(8) as usize;
    let layout = std::alloc::Layout::from_size_align(size, 8)
        .unwrap_or_else(|_| std::alloc::Layout::new::<u64>());
    // SAFETY: Layout is valid
    unsafe { std::alloc::alloc(layout) }
}

/// Concatenate two strings.
///
/// Returns a new `OriStr` with the concatenated result.
/// The caller is responsible for freeing the result.
#[no_mangle]
pub extern "C" fn ori_str_concat(a: *const OriStr, b: *const OriStr) -> OriStr {
    let a_str = if a.is_null() {
        ""
    } else {
        unsafe { (*a).as_str() }
    };
    let b_str = if b.is_null() {
        ""
    } else {
        unsafe { (*b).as_str() }
    };

    let result = format!("{a_str}{b_str}");
    let len = result.len() as i64;
    let data = result.into_boxed_str();
    let ptr = Box::into_raw(data) as *const u8;

    OriStr { len, data: ptr }
}

/// Compare two strings for equality.
#[no_mangle]
pub extern "C" fn ori_str_eq(a: *const OriStr, b: *const OriStr) -> bool {
    let a_str = if a.is_null() {
        ""
    } else {
        unsafe { (*a).as_str() }
    };
    let b_str = if b.is_null() {
        ""
    } else {
        unsafe { (*b).as_str() }
    };

    a_str == b_str
}

/// Compare two strings for inequality.
#[no_mangle]
pub extern "C" fn ori_str_ne(a: *const OriStr, b: *const OriStr) -> bool {
    !ori_str_eq(a, b)
}

/// Assert two strings are equal.
#[no_mangle]
pub extern "C" fn ori_assert_eq_str(actual: *const OriStr, expected: *const OriStr) {
    let actual_str = if actual.is_null() {
        ""
    } else {
        unsafe { (*actual).as_str() }
    };
    let expected_str = if expected.is_null() {
        ""
    } else {
        unsafe { (*expected).as_str() }
    };

    if actual_str != expected_str {
        eprintln!("assertion failed: \"{actual_str}\" != \"{expected_str}\"");
        PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
        PANIC_MESSAGE.with(|m| {
            *m.borrow_mut() = Some(format!(
                "assertion failed: \"{actual_str}\" != \"{expected_str}\""
            ));
        });
    }
}

// -- Type Conversion Functions --

/// Convert an integer to a string.
///
/// Returns an `OriStr` with the string representation.
#[no_mangle]
pub extern "C" fn ori_str_from_int(n: i64) -> OriStr {
    let result = n.to_string();
    let len = result.len() as i64;
    let data = result.into_boxed_str();
    let ptr = Box::into_raw(data) as *const u8;
    OriStr { len, data: ptr }
}

/// Convert a boolean to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_bool(b: bool) -> OriStr {
    let result = if b { "true" } else { "false" };
    // Use static string - no allocation needed
    OriStr {
        len: result.len() as i64,
        data: result.as_ptr(),
    }
}

/// Convert a float to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_float(f: f64) -> OriStr {
    let result = f.to_string();
    let len = result.len() as i64;
    let data = result.into_boxed_str();
    let ptr = Box::into_raw(data) as *const u8;
    OriStr { len, data: ptr }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_ori_assert_eq_int_fail() {
        reset_panic_state();
        ori_assert_eq_int(42, 43);
        assert!(did_panic());
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
}
