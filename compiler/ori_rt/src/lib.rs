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
//! - **Reference Counting**: `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free`
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

#![warn(clippy::allow_attributes_without_reason)]
#![allow(
    unsafe_code,
    reason = "C-ABI runtime functions require unsafe for raw pointer operations"
)]
#![allow(
    clippy::not_unsafe_ptr_arg_deref,
    reason = "FFI entry points receive pointers from LLVM-generated code which guarantees validity"
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_ptr_alignment,
    reason = "FFI code uses i64 for ABI compatibility — casts are intentional and safe"
)]
#![allow(
    clippy::manual_let_else,
    reason = "explicit match preferred for clarity in FFI error handling"
)]
#![allow(
    clippy::borrow_as_ptr,
    clippy::ptr_cast_constness,
    clippy::cast_slice_from_raw_parts,
    reason = "tests use &var to get pointers — intentional for FFI testing"
)]

use std::cell::{Cell, RefCell};
use std::ffi::CStr;
use std::panic;

/// Ori panic payload for stack unwinding (AOT mode).
///
/// Wrapped in `std::panic::panic_any` so the Itanium EH ABI
/// unwinds through LLVM-generated `invoke`/`landingpad` pairs,
/// giving cleanup handlers a chance to release RC'd resources.
///
/// The entry point wrapper catches this with `catch_unwind`.
pub struct OriPanic {
    pub message: String,
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

// ── Reference Counting (V2: 8-byte header, data-pointer style) ───────────
//
// Heap layout for RC'd objects:
//
//   +──────────────────+───────────────────────────────+
//   | strong_count: i64 | data bytes ...               |
//   +──────────────────+───────────────────────────────+
//   ^                   ^
//   base (ptr - 8)      data_ptr (returned by ori_rc_alloc)
//
// The data pointer points directly to user data, NOT to the header.
// strong_count lives at `data_ptr - 8`.
//
// Advantages:
// - Data pointer can be passed to C FFI without adjustment
// - Single pointer on stack (no separate header pointer)
// - 8 bytes smaller than old 16-byte RcHeader (no size field)
// - Size tracked at compile time via TypeInfo, not at runtime
//
// When refcount reaches zero, a type-specialized drop function handles:
// 1. Decrementing reference counts of RC'd child fields
// 2. Calling ori_rc_free(data_ptr, size, align) to release memory

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
    JIT_MODE.with(std::cell::Cell::get)
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

/// Allocate a new reference-counted object.
///
/// Allocates `size + 8` bytes with the given alignment, initializes
/// `strong_count` to 1, and returns a pointer to the data area.
///
/// Layout: `[strong_count: i64 | data bytes ...]`
///          ^                    ^
///          base (ptr - 8)       returned `data_ptr`
///
/// Returns null on allocation failure.
#[no_mangle]
pub extern "C" fn ori_rc_alloc(size: usize, align: usize) -> *mut u8 {
    let align = align.max(8); // Minimum 8-byte alignment for strong_count
    let total_size = size + 8;

    let base = ori_alloc(total_size, align);
    if base.is_null() {
        return std::ptr::null_mut();
    }

    // Initialize strong_count to 1
    // SAFETY: base is valid and 8-byte aligned
    unsafe {
        base.cast::<i64>().write(1);
    }

    // Return data pointer (8 bytes past the strong_count)
    // SAFETY: base is valid for total_size bytes, so base + 8 is valid
    unsafe { base.add(8) }
}

/// Increment the reference count of an RC'd object.
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
#[no_mangle]
pub extern "C" fn ori_rc_inc(data_ptr: *mut u8) {
    if data_ptr.is_null() {
        return;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    unsafe {
        let rc_ptr = data_ptr.sub(8).cast::<i64>();
        *rc_ptr += 1;
    }
}

/// Decrement the reference count. If it reaches zero, call the drop function.
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
///
/// `drop_fn` is a type-specialized function generated at compile time that:
/// 1. Decrements reference counts of any RC'd child fields
/// 2. Calls `ori_rc_free(data_ptr, size, align)` to release the memory
///
/// If `drop_fn` is null, the memory is leaked when refcount reaches zero.
/// This should not happen in well-formed programs — every RC type must have
/// a drop function.
#[no_mangle]
pub extern "C" fn ori_rc_dec(data_ptr: *mut u8, drop_fn: Option<extern "C" fn(*mut u8)>) {
    if data_ptr.is_null() {
        return;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    let should_drop = unsafe {
        let rc_ptr = data_ptr.sub(8).cast::<i64>();
        *rc_ptr -= 1;
        *rc_ptr <= 0
    };

    if should_drop {
        if let Some(f) = drop_fn {
            f(data_ptr);
        }
    }
}

/// Free a reference-counted allocation unconditionally.
///
/// Deallocates from `data_ptr - 8` with total size `size + 8`.
/// Typically called as the last step of a type-specialized drop function.
///
/// `size` and `align` are the data size and alignment (same values passed
/// to `ori_rc_alloc`). The 8-byte header is accounted for internally.
#[no_mangle]
pub extern "C" fn ori_rc_free(data_ptr: *mut u8, size: usize, align: usize) {
    if data_ptr.is_null() {
        return;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is the base
    let base = unsafe { data_ptr.sub(8) };
    let total_size = size + 8;
    let align = align.max(8);

    ori_free(base, total_size, align);
}

/// Get the current reference count (for testing and debugging).
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
#[no_mangle]
pub extern "C" fn ori_rc_count(data_ptr: *const u8) -> i64 {
    if data_ptr.is_null() {
        return 0;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    unsafe { *data_ptr.sub(8).cast::<i64>() }
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
/// Dispatch order:
/// 1. Store panic state (for JIT test assertions)
/// 2. If user `@panic` handler registered and not re-entrant: call trampoline
/// 3. If JIT mode: `longjmp` back to test runner
/// 4. AOT default: print to stderr and `exit(1)`
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

    // Call user @panic handler if registered (AOT only, not re-entrant)
    call_panic_trampoline(&msg);

    // In JIT mode, longjmp back to the test runner instead of terminating
    if is_jit_mode() {
        let buf = JIT_RECOVERY_BUF.with(std::cell::Cell::get);
        if !buf.is_null() {
            // SAFETY: buf is valid — set by enter_jit_mode, stack-allocated in run_test
            unsafe { longjmp(buf, 1) };
        }
    }

    // AOT path: unwind via Rust panic infrastructure.
    // LLVM invoke/landingpad in the caller will catch this and run
    // RC cleanup before re-raising or terminating.
    eprintln!("ori panic: {msg}");
    panic::panic_any(OriPanic { message: msg });
}

/// Panic with a C string message.
///
/// Same dispatch order as `ori_panic`: user handler → JIT longjmp → unwind.
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

    // Call user @panic handler if registered (AOT only, not re-entrant)
    call_panic_trampoline(&msg);

    // In JIT mode, longjmp back to the test runner instead of terminating
    if is_jit_mode() {
        let buf = JIT_RECOVERY_BUF.with(std::cell::Cell::get);
        if !buf.is_null() {
            // SAFETY: buf is valid — set by enter_jit_mode, stack-allocated in run_test
            unsafe { longjmp(buf, 1) };
        }
    }

    // AOT path: unwind via Rust panic infrastructure
    eprintln!("ori panic: {msg}");
    panic::panic_any(OriPanic { message: msg });
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

/// Assert that two floats are equal.
#[no_mangle]
pub extern "C" fn ori_assert_eq_float(actual: f64, expected: f64) {
    #[allow(
        clippy::float_cmp,
        reason = "assertion intentionally uses exact equality"
    )]
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

/// Allocate a raw data buffer for a list with given capacity.
///
/// Returns a pointer to a contiguous buffer of `capacity * elem_size` bytes,
/// suitable for storing list elements directly. The caller manages the list
/// header (`{len, cap, data}`) as a stack struct in LLVM IR.
///
/// This is the JIT/codegen allocation path. For AOT code that needs a full
/// `OriList` struct on the heap, use `ori_list_new`.
#[no_mangle]
pub extern "C" fn ori_list_alloc_data(capacity: i64, elem_size: i64) -> *mut u8 {
    let cap = capacity.max(0) as usize;
    let size = elem_size.max(1) as usize;
    if cap > 0 {
        let Ok(layout) = std::alloc::Layout::array::<u8>(cap * size) else {
            return std::ptr::null_mut();
        };
        // SAFETY: Layout is non-zero size (cap > 0, size >= 1)
        unsafe { std::alloc::alloc(layout) }
    } else {
        std::ptr::null_mut()
    }
}

/// Allocate a new list with given capacity (full `OriList` struct on heap).
///
/// Used by AOT code. JIT codegen should use `ori_list_alloc_data` instead.
#[no_mangle]
pub extern "C" fn ori_list_new(capacity: i64, elem_size: i64) -> *mut OriList {
    let cap = capacity.max(0) as usize;
    let size = elem_size.max(1) as usize;

    let list = Box::new(OriList {
        len: 0,
        cap: cap as i64,
        data: if cap > 0 {
            let Ok(layout) = std::alloc::Layout::array::<u8>(cap * size) else {
                return std::ptr::null_mut();
            };
            // SAFETY: Layout is non-zero size (cap > 0, size >= 1)
            unsafe { std::alloc::alloc(layout) }
        } else {
            std::ptr::null_mut()
        },
    });

    Box::into_raw(list)
}

/// Free a heap-allocated `OriList` (from `ori_list_new`).
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
            if let Ok(layout) = std::alloc::Layout::array::<u8>(list.cap as usize * size) {
                std::alloc::dealloc(list.data, layout);
            }
        }
    }
}

/// Free a raw data buffer allocated by `ori_list_alloc_data`.
///
/// For stack-struct lists (`{len, cap, data}`) where only the data buffer
/// is heap-allocated. The list header lives on the stack and doesn't need
/// freeing. Used by ARC cleanup when decrementing list refcounts.
#[no_mangle]
pub extern "C" fn ori_list_free_data(data: *mut u8, capacity: i64, elem_size: i64) {
    if data.is_null() || capacity <= 0 {
        return;
    }
    let cap = capacity as usize;
    let size = elem_size.max(1) as usize;
    if let Ok(layout) = std::alloc::Layout::array::<u8>(cap * size) {
        // SAFETY: data was allocated by ori_list_alloc_data with same layout
        unsafe { std::alloc::dealloc(data, layout) };
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

/// Compare two strings lexicographically.
///
/// Returns Ordering tag: 0 (Less), 1 (Equal), 2 (Greater).
#[no_mangle]
pub extern "C" fn ori_str_compare(a: *const OriStr, b: *const OriStr) -> i8 {
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

    match a_str.cmp(b_str) {
        core::cmp::Ordering::Less => 0,
        core::cmp::Ordering::Equal => 1,
        core::cmp::Ordering::Greater => 2,
    }
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

/// Convert C `argc`/`argv` to an Ori `[str]` list.
///
/// Skips `argv[0]` (program name) per the Ori spec: `@main(args)` receives
/// only user-supplied arguments. Returns `OriList { len, cap, data }` by value.
///
/// Each element is an `OriStr { len: i64, data: *const u8 }` (16 bytes).
/// String data is copied to owned allocations so the caller doesn't depend
/// on the lifetime of the original `argv` strings.
#[no_mangle]
#[allow(
    clippy::similar_names,
    reason = "argc/argv are standard C parameter names"
)]
pub extern "C" fn ori_args_from_argv(argc: i32, argv: *const *const i8) -> OriList {
    // Empty list if no user args or null argv
    if argc <= 1 || argv.is_null() {
        return OriList {
            len: 0,
            cap: 0,
            data: std::ptr::null_mut(),
        };
    }

    let count = (argc - 1) as usize; // skip argv[0]
                                     // Allocate contiguous array for OriStr elements (16 bytes each)
    let layout = std::alloc::Layout::array::<OriStr>(count)
        .unwrap_or_else(|_| std::alloc::Layout::new::<u8>());
    // SAFETY: Layout is valid (count > 0, OriStr has standard alignment)
    let data = unsafe { std::alloc::alloc(layout) };
    if data.is_null() {
        return OriList {
            len: 0,
            cap: 0,
            data: std::ptr::null_mut(),
        };
    }

    let elements = data.cast::<OriStr>();
    for i in 0..count {
        // SAFETY: argv is valid for argc entries; we access argv[i+1]
        let c_str = unsafe { CStr::from_ptr(*argv.add(i + 1)) };
        let bytes = c_str.to_bytes();
        let len = bytes.len();

        // Copy string data to owned allocation
        let str_data = if len > 0 {
            let str_layout = std::alloc::Layout::array::<u8>(len)
                .unwrap_or_else(|_| std::alloc::Layout::new::<u8>());
            // SAFETY: Layout is valid
            let ptr = unsafe { std::alloc::alloc(str_layout) };
            if !ptr.is_null() {
                // SAFETY: bytes and ptr are valid for len bytes
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
            }
            ptr
        } else {
            std::ptr::null_mut()
        };

        // SAFETY: elements[i] is within the allocated array
        unsafe {
            elements.add(i).write(OriStr {
                len: len as i64,
                data: str_data,
            });
        }
    }

    OriList {
        len: count as i64,
        cap: count as i64,
        data: data.cast::<u8>(),
    }
}

// ── Panic handler registration ──────────────────────────────────────────

/// Type for the panic trampoline function.
///
/// The trampoline is an LLVM-generated function that receives raw C values
/// and constructs the Ori `PanicInfo` struct before calling the user's
/// `@panic` handler. Signature:
/// `(msg_ptr, msg_len, file_ptr, file_len, line, col) -> void`
type PanicTrampoline = extern "C" fn(*const u8, i64, *const u8, i64, i64, i64);

/// Global panic trampoline function pointer.
///
/// Set by `ori_register_panic_handler` during `main()` initialization.
/// Called by `ori_panic`/`ori_panic_cstr` before default behavior.
///
/// # Safety
///
/// Access is limited to single-threaded AOT initialization (`main()` before
/// spawning threads). Thread-local `IN_PANIC_HANDLER` provides re-entrancy
/// protection.
static mut ORI_PANIC_TRAMPOLINE: Option<PanicTrampoline> = None;

thread_local! {
    /// Re-entrancy guard: prevents infinite recursion if the user's `@panic`
    /// handler itself panics.
    static IN_PANIC_HANDLER: Cell<bool> = const { Cell::new(false) };
}

/// Call the user's panic trampoline if registered and not re-entrant.
///
/// The trampoline receives raw C values (message pointer, length, empty
/// file/location) and constructs the Ori `PanicInfo` struct in LLVM IR
/// before calling the user's `@panic` function.
///
/// If the handler returns normally, we proceed with default behavior.
/// If the handler itself panics (re-entrancy), we skip it to avoid loops.
fn call_panic_trampoline(msg: &str) {
    // SAFETY: Read of global set during single-threaded main() init
    let trampoline = unsafe { ORI_PANIC_TRAMPOLINE };
    let Some(trampoline) = trampoline else {
        return;
    };

    // Re-entrancy guard: if @panic handler panics, skip it
    let already_in_handler = IN_PANIC_HANDLER.with(std::cell::Cell::get);
    if already_in_handler {
        return;
    }

    IN_PANIC_HANDLER.with(|h| h.set(true));

    let msg_ptr = msg.as_ptr();
    let msg_len = msg.len() as i64;
    // Empty file/location — populated when debug info infrastructure arrives (Section 13)
    let empty_ptr = c"".as_ptr().cast::<u8>();
    trampoline(msg_ptr, msg_len, empty_ptr, 0, 0, 0);

    IN_PANIC_HANDLER.with(|h| h.set(false));
}

/// Register a panic trampoline function.
///
/// Called from the generated `main()` wrapper when the user defines `@panic`.
/// The trampoline is an LLVM-generated function that bridges C values to Ori
/// `PanicInfo` struct construction.
#[no_mangle]
pub extern "C" fn ori_register_panic_handler(handler: *const ()) {
    if handler.is_null() {
        return;
    }
    // SAFETY: Called once during single-threaded main() initialization
    unsafe {
        ORI_PANIC_TRAMPOLINE = Some(std::mem::transmute::<*const (), PanicTrampoline>(handler));
    }
}

// ── AOT entry point wrapper ─────────────────────────────────────────────

/// Wrap an AOT `@main` call with `catch_unwind` to handle Ori panics.
///
/// The LLVM-generated `main()` calls this instead of calling `@main` directly.
/// This catches the `OriPanic` payload from `panic_any` and converts it to
/// `exit(1)`, preventing the Rust runtime from printing an ugly panic message.
///
/// `main_fn` is a function pointer to the user's compiled `@main` (void → void
/// or void → int variant).
///
/// Returns 0 on success, 1 on panic.
#[no_mangle]
pub extern "C" fn ori_run_main(main_fn: extern "C" fn()) -> i32 {
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        main_fn();
    }));

    match result {
        Ok(()) => 0,
        Err(payload) => {
            // Check if this is our structured OriPanic
            if payload.downcast_ref::<OriPanic>().is_some() {
                // Message already printed by ori_panic/ori_panic_cstr
                1
            } else {
                // Unknown panic (shouldn't happen in well-formed programs)
                eprintln!("ori panic: unexpected error");
                1
            }
        }
    }
}
