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

use std::cell::{Cell, RefCell};
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

// ── setjmp/longjmp JIT recovery ──────────────────────────────────────────

/// Buffer for `setjmp`/`longjmp` JIT error recovery.
///
/// Oversized to accommodate all platform `jmp_buf` layouts:
/// - x86-64 Linux: 200 bytes (8 × 25)
/// - x86-64 macOS: 148 bytes (4 × 37)
/// - aarch64: ~392 bytes
///
/// 512 bytes with 64-byte alignment covers all targets with margin.
#[repr(C, align(64))]
pub struct JmpBuf {
    _buf: [u8; 512],
}

impl JmpBuf {
    /// Create a zero-initialized jump buffer.
    #[must_use]
    pub fn new() -> Self {
        JmpBuf { _buf: [0u8; 512] }
    }
}

impl Default for JmpBuf {
    fn default() -> Self {
        Self::new()
    }
}

extern "C" {
    /// Save the current execution state. Returns 0 on direct call,
    /// non-zero when returning via `longjmp`.
    ///
    /// Uses `_setjmp` (POSIX): does NOT save the signal mask, which is faster
    /// and sufficient for JIT error recovery.
    #[link_name = "_setjmp"]
    fn c_setjmp(buf: *mut JmpBuf) -> i32;

    /// Restore execution state saved by `setjmp`. Never returns to caller.
    fn longjmp(buf: *mut JmpBuf, val: i32) -> !;
}

thread_local! {
    /// Whether the current thread is running JIT-compiled code.
    /// When true, `ori_panic`/`ori_panic_cstr` will `longjmp` instead of `exit(1)`.
    static JIT_MODE: Cell<bool> = const { Cell::new(false) };

    /// Pointer to the active `JmpBuf` for JIT recovery.
    /// Only valid when `JIT_MODE` is true.
    static JIT_RECOVERY_BUF: Cell<*mut JmpBuf> = const { Cell::new(std::ptr::null_mut()) };
}

/// Enter JIT mode: panics will `longjmp` to `buf` instead of terminating.
///
/// # Safety
///
/// `buf` must point to a valid `JmpBuf` that outlives the JIT execution.
/// The caller must call `leave_jit_mode()` when done (even on `longjmp` return).
pub fn enter_jit_mode(buf: *mut JmpBuf) {
    JIT_MODE.with(|m| m.set(true));
    JIT_RECOVERY_BUF.with(|b| b.set(buf));
}

/// Leave JIT mode: panics will `exit(1)` again (AOT behavior).
pub fn leave_jit_mode() {
    JIT_MODE.with(|m| m.set(false));
    JIT_RECOVERY_BUF.with(|b| b.set(std::ptr::null_mut()));
}

/// Check if we're currently in JIT mode.
fn is_jit_mode() -> bool {
    JIT_MODE.with(|m| m.get())
}

/// Call `setjmp` on a `JmpBuf`. Returns 0 on direct call, non-zero on `longjmp`.
///
/// # Safety
///
/// `buf` must point to a valid, properly aligned `JmpBuf`.
pub unsafe fn jit_setjmp(buf: *mut JmpBuf) -> i32 {
    c_setjmp(buf)
}

// ── Thread-local panic state ─────────────────────────────────────────────

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

/// Set panic state without terminating (for tests only).
///
/// Unlike `ori_panic` and `ori_panic_cstr`, this function does NOT call `exit()`,
/// allowing tests to verify panic behavior without terminating the test process.
///
/// This is intentionally not gated on `#[cfg(test)]` so integration tests in
/// other crates can use it.
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
/// In JIT mode, sets thread-local panic state and `longjmp`s back to the
/// test runner. In AOT mode, prints to stderr and terminates.
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

    // In JIT mode, longjmp back to the test runner instead of terminating
    if is_jit_mode() {
        let buf = JIT_RECOVERY_BUF.with(|b| b.get());
        if !buf.is_null() {
            // SAFETY: buf is valid — set by enter_jit_mode, stack-allocated in run_test
            unsafe { longjmp(buf, 1) };
        }
    }

    // AOT path: print and terminate
    eprintln!("ori panic: {msg}");
    std::process::exit(1);
}

/// Panic with a C string message.
///
/// In JIT mode, sets panic state and `longjmp`s back to the test runner.
/// In AOT mode, prints to stderr and terminates.
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

    // In JIT mode, longjmp back to the test runner instead of terminating
    if is_jit_mode() {
        let buf = JIT_RECOVERY_BUF.with(|b| b.get());
        if !buf.is_null() {
            // SAFETY: buf is valid — set by enter_jit_mode, stack-allocated in run_test
            unsafe { longjmp(buf, 1) };
        }
    }

    // AOT path: print and terminate
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
