//! Ori Runtime Library (`libori_rt`)
//!
//! This crate provides runtime support for AOT-compiled Ori programs.
//! It contains C-ABI functions that are called by LLVM-generated code.
//!
//! # Build Modes
//!
//! - **rlib**: For Rust consumers (JIT execution via `ori_llvm`)
//! - **staticlib**: For AOT linking (`libori_rt.a`)
//!
//! # Function Categories
//!
//! - **Memory**: `ori_alloc`, `ori_free`, `ori_realloc`
//! - **Reference Counting**: `ori_rc_new`, `ori_rc_inc`, `ori_rc_dec`
//! - **Strings**: `ori_str_concat`, `ori_str_eq`, etc.
//! - **Collections**: `ori_list_new`, `ori_list_free`, etc.
//! - **I/O**: `ori_print`, `ori_print_int`, etc.
//! - **Panic**: `ori_panic`, `ori_assert`, etc.
//!
//! # Safety
//!
//! All functions use `#[no_mangle]` and `extern "C"` for FFI compatibility.
//! Functions that take raw pointers are called from LLVM-generated code which
//! guarantees valid pointers. They're not marked `unsafe` because they're
//! extern "C" FFI entry points, not Rust API functions.

#![allow(unsafe_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
// FFI code uses i64 for ABI compatibility - casts are intentional and safe
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_ptr_alignment)]
// Prefer explicit match over let-else for clarity in FFI error handling
#![allow(clippy::manual_let_else)]
// Tests use &var to get pointers - this is intentional
#![allow(clippy::borrow_as_ptr)]
#![allow(clippy::ptr_cast_constness)]
#![allow(clippy::cast_slice_from_raw_parts)]

use std::cell::RefCell;
use std::ffi::CStr;

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

/// Reference-counted object header.
///
/// Layout in memory:
/// ```text
/// +------------+--------+------+
/// | refcount   | size   | data |
/// | (i64)      | (i64)  | ...  |
/// +------------+--------+------+
/// ```
#[repr(C)]
pub struct RcHeader {
    /// Current reference count. When this reaches 0, the object is freed.
    pub refcount: i64,
    /// Size of the data following the header (for deallocation).
    pub size: i64,
}

thread_local! {
    static PANIC_OCCURRED: RefCell<bool> = const { RefCell::new(false) };
    static PANIC_MESSAGE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Check if a panic occurred (for test assertions).
#[must_use]
pub fn did_panic() -> bool {
    PANIC_OCCURRED.with(|p| *p.borrow())
}

/// Get the panic message if one occurred.
#[must_use]
pub fn get_panic_message() -> Option<String> {
    PANIC_MESSAGE.with(|m| m.borrow().clone())
}

/// Reset panic state (call before each test).
pub fn reset_panic_state() {
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = false);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = None);
}

/// Set panic state without terminating (for unit tests only).
///
/// Unlike `ori_panic` and `ori_panic_cstr`, this function does NOT call `exit()`,
/// allowing unit tests to verify panic behavior without terminating the test process.
#[cfg(test)]
pub fn set_panic_state_for_test(msg: &str) {
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(msg.to_string()));
}

/// Allocate memory with the given size and alignment.
///
/// Returns a pointer to the allocated memory, or null on failure.
/// The memory is uninitialized.
#[no_mangle]
pub extern "C" fn ori_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }

    let align = align.max(8); // Minimum 8-byte alignment
    let layout = match std::alloc::Layout::from_size_align(size, align) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: Layout is valid (size > 0, alignment is power of 2)
    unsafe { std::alloc::alloc(layout) }
}

/// Free memory previously allocated with `ori_alloc`.
///
/// # Safety
/// - `ptr` must have been returned by `ori_alloc` with the same size and alignment.
/// - `ptr` must not have been freed already.
#[no_mangle]
pub extern "C" fn ori_free(ptr: *mut u8, size: usize, align: usize) {
    if ptr.is_null() || size == 0 {
        return;
    }

    let align = align.max(8);
    let layout = match std::alloc::Layout::from_size_align(size, align) {
        Ok(layout) => layout,
        Err(_) => return,
    };

    // SAFETY: Caller guarantees ptr was allocated with matching layout
    unsafe { std::alloc::dealloc(ptr, layout) }
}

/// Reallocate memory to a new size.
///
/// Returns a pointer to the reallocated memory, or null on failure.
/// The contents are preserved up to the minimum of old and new sizes.
#[no_mangle]
pub extern "C" fn ori_realloc(
    ptr: *mut u8,
    old_size: usize,
    new_size: usize,
    align: usize,
) -> *mut u8 {
    if ptr.is_null() {
        return ori_alloc(new_size, align);
    }

    if new_size == 0 {
        ori_free(ptr, old_size, align);
        return std::ptr::null_mut();
    }

    let align = align.max(8);
    let old_layout = match std::alloc::Layout::from_size_align(old_size, align) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: Caller guarantees ptr was allocated with matching layout
    unsafe { std::alloc::realloc(ptr, old_layout, new_size) }
}

/// Create a new reference-counted object.
///
/// Allocates memory for the header + data, initializes refcount to 1.
/// Returns a pointer to the `RcHeader`, or null on failure.
#[no_mangle]
pub extern "C" fn ori_rc_new(size: usize) -> *mut RcHeader {
    let header_size = std::mem::size_of::<RcHeader>();
    let total_size = header_size + size;
    let align = std::mem::align_of::<RcHeader>().max(8);

    let ptr = ori_alloc(total_size, align);
    if ptr.is_null() {
        return std::ptr::null_mut();
    }

    // SAFETY: ptr is valid and properly aligned for RcHeader
    let header = ptr.cast::<RcHeader>();
    unsafe {
        (*header).refcount = 1;
        (*header).size = size as i64;
    }

    header
}

/// Increment the reference count.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `ori_rc_new`.
#[no_mangle]
pub extern "C" fn ori_rc_inc(ptr: *mut RcHeader) {
    if ptr.is_null() {
        return;
    }

    // SAFETY: Caller guarantees ptr is valid
    unsafe {
        (*ptr).refcount += 1;
    }
}

/// Decrement the reference count. Frees the object if count reaches 0.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `ori_rc_new`.
#[no_mangle]
pub extern "C" fn ori_rc_dec(ptr: *mut RcHeader) {
    if ptr.is_null() {
        return;
    }

    // SAFETY: Caller guarantees ptr is valid
    let should_free = unsafe {
        (*ptr).refcount -= 1;
        (*ptr).refcount <= 0
    };

    if should_free {
        let header_size = std::mem::size_of::<RcHeader>();
        let data_size = unsafe { (*ptr).size as usize };
        let total_size = header_size + data_size;
        let align = std::mem::align_of::<RcHeader>().max(8);

        ori_free(ptr.cast(), total_size, align);
    }
}

/// Get the current reference count.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `ori_rc_new`.
#[no_mangle]
pub extern "C" fn ori_rc_count(ptr: *const RcHeader) -> i64 {
    if ptr.is_null() {
        return 0;
    }

    // SAFETY: Caller guarantees ptr is valid
    unsafe { (*ptr).refcount }
}

/// Get a pointer to the data following the header.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `ori_rc_new`.
#[no_mangle]
pub extern "C" fn ori_rc_data(ptr: *mut RcHeader) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }

    // SAFETY: Data immediately follows the header
    unsafe { ptr.add(1).cast() }
}

/// Print a string to stdout.
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
/// In JIT mode, sets thread-local panic state for test isolation.
/// In AOT mode, prints to stderr and terminates.
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

    // Store panic state in thread-local storage (for JIT tests)
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(msg.clone()));

    // Print to stderr
    eprintln!("ori panic: {msg}");

    // Terminate the process - in JIT mode, the execution engine catches this
    // before it reaches here by checking did_panic() after each execution.
    std::process::exit(1);
}

/// Panic with a C string message.
#[no_mangle]
pub extern "C" fn ori_panic_cstr(s: *const i8) {
    let msg = if s.is_null() {
        "panic!".to_string()
    } else {
        // SAFETY: Caller ensures s points to a valid C string
        let cstr = unsafe { CStr::from_ptr(s) };
        cstr.to_string_lossy().to_string()
    };

    PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
    PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(msg.clone()));

    eprintln!("ori panic: {msg}");

    std::process::exit(1);
}

/// Assert that a condition is true.
///
/// Sets panic state but does NOT terminate - this allows JIT tests to check `did_panic()`.
/// For AOT, the generated code should check the panic state after assertions.
#[no_mangle]
pub extern "C" fn ori_assert(condition: bool) {
    if !condition {
        let msg = "assertion failed";
        eprintln!("ori panic: {msg}");
        PANIC_OCCURRED.with(|p| *p.borrow_mut() = true);
        PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(msg.to_string()));
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

/// Allocate a new list with given capacity.
#[no_mangle]
pub extern "C" fn ori_list_new(capacity: i64, elem_size: i64) -> *mut OriList {
    let cap = capacity.max(0) as usize;
    let size = elem_size.max(1) as usize;

    let list = Box::new(OriList {
        len: 0,
        cap: cap as i64,
        data: if cap > 0 {
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

/// Convert an integer to a string.
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

/// Compare two integers (for sorting, etc.)
/// Returns -1 if a < b, 0 if a == b, 1 if a > b.
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
    ori_alloc(size, 8)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_rc_new_inc_dec() {
        let rc = ori_rc_new(64);
        assert!(!rc.is_null());
        assert_eq!(ori_rc_count(rc), 1);

        ori_rc_inc(rc);
        assert_eq!(ori_rc_count(rc), 2);

        ori_rc_dec(rc);
        assert_eq!(ori_rc_count(rc), 1);

        // Final dec frees it - don't access after
        ori_rc_dec(rc);
    }

    #[test]
    fn test_rc_data() {
        let rc = ori_rc_new(64);
        let data = ori_rc_data(rc);
        assert!(!data.is_null());

        // Write to data
        unsafe {
            std::ptr::write(data, 123u8);
            assert_eq!(std::ptr::read(data), 123u8);
        }

        ori_rc_dec(rc);
    }

    #[test]
    fn test_rc_null_safety() {
        ori_rc_inc(std::ptr::null_mut());
        ori_rc_dec(std::ptr::null_mut());
        assert_eq!(ori_rc_count(std::ptr::null()), 0);
        assert!(ori_rc_data(std::ptr::null_mut()).is_null());
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

    #[test]
    fn test_ori_str_concat() {
        let a = OriStr {
            len: 5,
            data: "hello".as_ptr(),
        };
        let b = OriStr {
            len: 6,
            data: " world".as_ptr(),
        };

        let result = ori_str_concat(&a, &b);
        assert_eq!(result.len, 11);

        let text = unsafe { result.as_str() };
        assert_eq!(text, "hello world");

        // Free the result (it was heap-allocated)
        unsafe {
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                result.data as *mut u8,
                result.len as usize,
            ));
        }
    }

    #[test]
    fn test_ori_str_eq() {
        let a = OriStr {
            len: 5,
            data: "hello".as_ptr(),
        };
        let b = OriStr {
            len: 5,
            data: "hello".as_ptr(),
        };
        let c = OriStr {
            len: 5,
            data: "world".as_ptr(),
        };

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
    fn test_ori_closure_box() {
        let ptr = ori_closure_box(64);
        assert!(!ptr.is_null());
        ori_free(ptr, 64, 8);
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
}
