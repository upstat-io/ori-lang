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

pub mod format;
pub mod iterator;

use std::cell::{Cell, RefCell};
use std::ffi::CStr;
use std::panic;
#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic::AtomicI64;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::OnceLock;

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

    /// Create an `OriStr` from an owned `String` using RC-managed allocation.
    ///
    /// The string data is copied into an `ori_rc_alloc` buffer so that the
    /// ARC pipeline's `RcInc`/`RcDec` operations work correctly (they expect
    /// a refcount header at `data_ptr - 8`).
    #[must_use]
    pub fn from_owned(s: String) -> Self {
        let bytes = s.into_bytes();
        let len = bytes.len() as i64;
        if bytes.is_empty() {
            return Self {
                len: 0,
                data: std::ptr::null(),
            };
        }
        let data = ori_rc_alloc(bytes.len(), 1);
        if !data.is_null() {
            // SAFETY: ori_rc_alloc returned a valid pointer of at least `bytes.len()` bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
            }
        }
        Self {
            len,
            data: data as *const u8,
        }
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

/// Live RC allocation counter for debugging and testing.
///
/// Incremented by `ori_rc_alloc`, decremented by `ori_rc_free`.
/// Read by `ori_rc_live_count` to verify all allocations were freed.
static RC_LIVE_COUNT: AtomicI64 = AtomicI64::new(0);

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

    // Initialize strong_count to 1.
    // No ordering needed — this allocation is not yet visible to other threads.
    // SAFETY: base is valid and 8-byte aligned
    unsafe {
        #[cfg(not(feature = "single-threaded"))]
        base.cast::<AtomicI64>().write(AtomicI64::new(1));
        #[cfg(feature = "single-threaded")]
        base.cast::<i64>().write(1);
    }

    RC_LIVE_COUNT.fetch_add(1, Ordering::Relaxed);

    // Return data pointer (8 bytes past the strong_count)
    // SAFETY: base is valid for total_size bytes, so base + 8 is valid
    unsafe { base.add(8) }
}

/// Increment the reference count of an RC'd object.
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
///
/// Uses `Relaxed` ordering: the increment only needs to be atomic, not
/// ordered with respect to other memory operations. The incrementing
/// thread already holds a valid reference, so no synchronization is
/// needed. Matches Swift's `swift_retain` and Rust's `Arc::clone`.
#[no_mangle]
pub extern "C" fn ori_rc_inc(data_ptr: *mut u8) {
    if data_ptr.is_null() {
        return;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    unsafe {
        #[cfg(not(feature = "single-threaded"))]
        {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_add(1, Ordering::Relaxed);
        }
        #[cfg(feature = "single-threaded")]
        {
            let rc_ptr = data_ptr.sub(8).cast::<i64>();
            *rc_ptr += 1;
        }
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
///
/// Uses `Release` ordering on the decrement: ensures all writes to the
/// object through this reference are visible before any thread deallocates.
/// An `Acquire` fence before calling the drop function ensures the
/// deallocating thread sees all prior writes from all threads.
///
/// This is the standard ARC pattern from Rust's `Arc::drop` and Swift's
/// `swift_release`.
#[no_mangle]
pub extern "C" fn ori_rc_dec(data_ptr: *mut u8, drop_fn: Option<extern "C" fn(*mut u8)>) {
    if data_ptr.is_null() {
        return;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    #[cfg(not(feature = "single-threaded"))]
    {
        let prev = unsafe {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_sub(1, Ordering::Release)
        };

        debug_assert!(
            prev > 0,
            "ori_rc_dec: refcount was already zero (use-after-free)"
        );

        if prev <= 1 {
            // Acquire fence: synchronize with all Release decrements from other
            // threads. This ensures the drop function sees all writes that any
            // thread made through their reference before decrementing.
            std::sync::atomic::fence(Ordering::Acquire);

            if let Some(f) = drop_fn {
                call_drop_fn(f, data_ptr);
            }
        }
    }

    #[cfg(feature = "single-threaded")]
    {
        let should_drop = unsafe {
            let rc_ptr = data_ptr.sub(8).cast::<i64>();
            debug_assert!(
                *rc_ptr > 0,
                "ori_rc_dec: refcount was already zero (use-after-free)"
            );
            *rc_ptr -= 1;
            *rc_ptr <= 0
        };

        if should_drop {
            if let Some(f) = drop_fn {
                call_drop_fn(f, data_ptr);
            }
        }
    }
}

/// Call a drop function with abort-on-panic guard.
///
/// `ori_rc_dec` is declared `nounwind` in LLVM IR, meaning unwinding through
/// it is UB. This wrapper ensures that if a drop function panics, we abort
/// immediately rather than unwinding through the `nounwind` boundary.
/// Matches Rust's `Drop` + `nounwind` contract.
fn call_drop_fn(f: extern "C" fn(*mut u8), data_ptr: *mut u8) {
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        f(data_ptr);
    }));
    if result.is_err() {
        eprintln!("ori: drop function panicked — aborting (drop must not unwind)");
        std::process::abort();
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

    RC_LIVE_COUNT.fetch_sub(1, Ordering::Relaxed);
}

/// Get the number of live RC allocations (for testing and debugging).
///
/// Returns the number of `ori_rc_alloc` calls minus `ori_rc_free` calls.
/// Should be 0 at program exit if all memory was properly freed.
#[no_mangle]
pub extern "C" fn ori_rc_live_count() -> i64 {
    RC_LIVE_COUNT.load(Ordering::Relaxed)
}

/// Reset the live RC allocation counter to zero.
///
/// Used by JIT test runners where multiple tests execute in the same process.
/// Each test can start with a fresh counter by calling this before execution.
#[no_mangle]
pub extern "C" fn ori_rc_reset_live_count() {
    RC_LIVE_COUNT.store(0, Ordering::Relaxed);
}

/// Get the current reference count (for testing and debugging).
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
/// Uses `Relaxed` ordering — this is a diagnostic read, not a synchronization point.
#[no_mangle]
pub extern "C" fn ori_rc_count(data_ptr: *const u8) -> i64 {
    if data_ptr.is_null() {
        return 0;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    unsafe {
        #[cfg(not(feature = "single-threaded"))]
        {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).load(Ordering::Relaxed)
        }
        #[cfg(feature = "single-threaded")]
        {
            *data_ptr.sub(8).cast::<i64>()
        }
    }
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
pub extern "C-unwind" fn ori_panic(s: *const OriStr) {
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
pub extern "C-unwind" fn ori_panic_cstr(s: *const i8) {
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
/// On failure, routes through `ori_panic_cstr` which handles both JIT
/// (longjmp to test runner) and AOT (unwind via `panic_any`) paths.
#[no_mangle]
pub extern "C-unwind" fn ori_assert(condition: bool) {
    if !condition {
        ori_panic_cstr(c"assertion failed".as_ptr());
    }
}

/// Assert that two integers are equal.
///
/// On failure, formats a message and routes through `ori_panic_cstr`.
#[no_mangle]
pub extern "C-unwind" fn ori_assert_eq_int(actual: i64, expected: i64) {
    if actual != expected {
        let msg = format!("assertion failed: {actual} != {expected}\0");
        ori_panic_cstr(msg.as_ptr().cast::<i8>());
    }
}

/// Assert that two booleans are equal.
///
/// On failure, formats a message and routes through `ori_panic_cstr`.
#[no_mangle]
pub extern "C-unwind" fn ori_assert_eq_bool(actual: bool, expected: bool) {
    if actual != expected {
        let msg = format!("assertion failed: {actual} != {expected}\0");
        ori_panic_cstr(msg.as_ptr().cast::<i8>());
    }
}

/// Assert that two floats are equal.
///
/// On failure, formats a message and routes through `ori_panic_cstr`.
#[no_mangle]
pub extern "C-unwind" fn ori_assert_eq_float(actual: f64, expected: f64) {
    #[allow(
        clippy::float_cmp,
        reason = "assertion intentionally uses exact equality"
    )]
    if actual != expected {
        let msg = format!("assertion failed: {actual} != {expected}\0");
        ori_panic_cstr(msg.as_ptr().cast::<i8>());
    }
}

/// Assert two strings are equal.
///
/// On failure, formats a message and routes through `ori_panic_cstr`.
#[no_mangle]
pub extern "C-unwind" fn ori_assert_eq_str(actual: *const OriStr, expected: *const OriStr) {
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
        let msg = format!("assertion failed: \"{actual_str}\" != \"{expected_str}\"\0");
        ori_panic_cstr(msg.as_ptr().cast::<i8>());
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
        let total = cap * size;
        // Minimum 8-byte alignment matches ori_alloc's discipline. Handles i64, f64,
        // and pointer elements correctly. Layout::array::<u8> would give alignment 1.
        let Ok(layout) = std::alloc::Layout::from_size_align(total, 8) else {
            return std::ptr::null_mut();
        };
        // SAFETY: Layout is non-zero size (cap > 0, size >= 1), alignment is valid
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
            let total = cap * size;
            let Ok(layout) = std::alloc::Layout::from_size_align(total, 8) else {
                return std::ptr::null_mut();
            };
            // SAFETY: Layout is non-zero size (cap > 0, size >= 1), alignment is valid
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
            let total = list.cap as usize * size;
            if let Ok(layout) = std::alloc::Layout::from_size_align(total, 8) {
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
    let total = cap * size;
    if let Ok(layout) = std::alloc::Layout::from_size_align(total, 8) {
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

/// Push an element onto a heap-allocated list, growing capacity if needed.
///
/// `list` is a pointer to a heap-allocated `OriList` (from `ori_list_new`).
/// `elem_ptr` points to the raw element bytes to copy.
/// `elem_size` is the byte size of each element.
#[no_mangle]
pub extern "C" fn ori_list_push(list: *mut u8, elem_ptr: *const u8, elem_size: i64) {
    if list.is_null() || elem_ptr.is_null() {
        return;
    }
    let list = unsafe { &mut *list.cast::<OriList>() };
    let es = elem_size.max(1) as usize;

    // Grow if needed
    if list.len >= list.cap {
        let new_cap = if list.cap <= 0 {
            8
        } else {
            list.cap as usize * 2
        };
        let old_total = list.cap.max(0) as usize * es;
        let new_total = new_cap * es;
        let new_data = if list.data.is_null() {
            crate::ori_alloc(new_total, 8)
        } else {
            crate::ori_realloc(list.data, old_total, new_total, 8)
        };
        list.data = new_data;
        list.cap = new_cap as i64;
    }

    // Copy element bytes into the data buffer
    unsafe {
        std::ptr::copy_nonoverlapping(elem_ptr, list.data.add(list.len as usize * es), es);
    }
    list.len += 1;
}

/// Extract the `OriList` contents and free the heap wrapper.
///
/// Writes `{len, cap, data}` to `out_ptr` (sret pattern — avoids ABI
/// mismatch for >16 byte struct returns). The data buffer ownership
/// transfers to the caller; only the `OriList` wrapper is freed.
#[no_mangle]
pub extern "C" fn ori_list_take(list: *mut u8, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    if list.is_null() {
        unsafe {
            out_ptr.cast::<i64>().write(0); // len
            out_ptr.cast::<i64>().add(1).write(0); // cap
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut()); // data
        }
        return;
    }
    let boxed = unsafe { Box::from_raw(list.cast::<OriList>()) };
    let len = boxed.len;
    let cap = boxed.cap;
    let data = boxed.data;
    // Box::drop frees the OriList struct; data buffer is NOT freed
    drop(boxed);
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

/// Create a new list with an element appended (functional push).
///
/// Allocates a new data buffer, copies all elements from the original,
/// then copies the new element at the end. Writes the resulting
/// `{len+1, len+1, new_data}` to `out_ptr` (sret pattern).
///
/// The original list data is NOT freed — the caller retains ownership.
#[no_mangle]
pub extern "C" fn ori_list_push_new(
    data: *const u8,
    len: i64,
    elem_ptr: *const u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let old_len = len.max(0) as usize;
    let new_len = old_len + 1;
    let new_total = new_len * es;

    let new_data = crate::ori_alloc(new_total, 8);

    // Copy old elements
    if !data.is_null() && old_len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(data, new_data, old_len * es);
        }
    }

    // Copy new element at the end
    unsafe {
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(old_len * es), es);
    }

    // Write result: {len, cap, data}
    unsafe {
        out_ptr.cast::<i64>().write(new_len as i64);
        out_ptr.cast::<i64>().add(1).write(new_len as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Return the first element of a list, or write a None tag if empty.
///
/// Writes `{tag, value}` to `out_ptr` where tag=1 (Some) with element
/// copied, or tag=0 (None). The value region is `elem_size` bytes.
#[no_mangle]
pub extern "C" fn ori_list_first(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    if data.is_null() || len <= 0 {
        // None: tag = 1 (Ori convention: 0=Some, 1=None)
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
        return;
    }
    // Some: tag = 0, copy first element
    unsafe {
        out_ptr.cast::<i64>().write(0);
        std::ptr::copy_nonoverlapping(data, out_ptr.add(8), es);
    }
}

/// Return the last element of a list, or write a None tag if empty.
///
/// Same layout as `ori_list_first`: `{tag: i64, value: [elem_size]}`.
/// Ori convention: tag 0=Some, 1=None.
#[no_mangle]
pub extern "C" fn ori_list_last(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    if data.is_null() || len <= 0 {
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
        return;
    }
    let last_offset = (len as usize - 1) * es;
    unsafe {
        out_ptr.cast::<i64>().write(0);
        std::ptr::copy_nonoverlapping(data.add(last_offset), out_ptr.add(8), es);
    }
}

/// Check whether a list of i64 elements contains a given value.
///
/// Returns 1 (true) if found, 0 (false) otherwise.
#[no_mangle]
pub extern "C" fn ori_list_contains_int(data: *const u8, len: i64, needle: i64) -> i64 {
    if data.is_null() || len <= 0 {
        return 0;
    }
    let ptr = data.cast::<i64>();
    for i in 0..len as usize {
        if unsafe { *ptr.add(i) } == needle {
            return 1;
        }
    }
    0
}

/// Check whether a list of strings contains a given string.
///
/// Each string element is `{i64 len, ptr data}` (16 bytes).
/// Returns 1 (true) if found, 0 (false) otherwise.
#[no_mangle]
pub extern "C" fn ori_list_contains_str(data: *const u8, len: i64, needle: *const OriStr) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() {
        return 0;
    }
    let needle_str = unsafe { deref_str(needle) };
    let elem_size = std::mem::size_of::<OriStr>();
    for i in 0..len as usize {
        let elem_ptr = unsafe { data.add(i * elem_size).cast::<OriStr>() };
        let elem_str = unsafe { (*elem_ptr).as_str() };
        if elem_str == needle_str {
            return 1;
        }
    }
    0
}

/// Create a reversed copy of a list.
///
/// Allocates new data buffer, copies elements in reverse order.
/// Writes `{len, len, new_data}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_list_reverse(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let n = len.max(0) as usize;

    if data.is_null() || n == 0 {
        unsafe {
            out_ptr.cast::<i64>().write(0);
            out_ptr.cast::<i64>().add(1).write(0);
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut());
        }
        return;
    }

    let total = n * es;
    let new_data = crate::ori_alloc(total, 8);

    for i in 0..n {
        let src_offset = (n - 1 - i) * es;
        let dst_offset = i * es;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_offset), new_data.add(dst_offset), es);
        }
    }

    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Concatenate two lists, returning a new list.
///
/// Allocates new buffer, copies elements from both lists.
/// Writes `{len1+len2, len1+len2, new_data}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_list_concat(
    data1: *const u8,
    len1: i64,
    data2: *const u8,
    len2: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let n1 = len1.max(0) as usize;
    let n2 = len2.max(0) as usize;
    let total_len = n1 + n2;

    if total_len == 0 {
        unsafe {
            out_ptr.cast::<i64>().write(0);
            out_ptr.cast::<i64>().add(1).write(0);
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut());
        }
        return;
    }

    let total_bytes = total_len * es;
    let new_data = crate::ori_alloc(total_bytes, 8);

    unsafe {
        if !data1.is_null() && n1 > 0 {
            std::ptr::copy_nonoverlapping(data1, new_data, n1 * es);
        }
        if !data2.is_null() && n2 > 0 {
            std::ptr::copy_nonoverlapping(data2, new_data.add(n1 * es), n2 * es);
        }
        out_ptr.cast::<i64>().write(total_len as i64);
        out_ptr.cast::<i64>().add(1).write(total_len as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

// -----------------------------------------------------------------------
// Map methods
// -----------------------------------------------------------------------

/// Check if a map contains a string key.
///
/// Map keys are stored as a contiguous array of `OriStr` structs
/// (`{i64 len, ptr data}`). Linear scan with string comparison.
/// Returns 1 if found, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_map_contains_key(keys: *const u8, len: i64, needle: *const OriStr) -> i64 {
    if keys.is_null() || len <= 0 || needle.is_null() {
        return 0;
    }
    let needle_str = unsafe { deref_str(needle) };
    let key_size = std::mem::size_of::<OriStr>();
    for i in 0..len as usize {
        let key_ptr = unsafe { keys.add(i * key_size).cast::<OriStr>() };
        let key_str = unsafe { (*key_ptr).as_str() };
        if key_str == needle_str {
            return 1;
        }
    }
    0
}

/// Extract map keys as a new list.
///
/// Allocates a new buffer and copies all keys. Writes `{len, len, data_ptr}`
/// to `out_ptr` (sret pattern). The resulting list owns its data buffer.
#[no_mangle]
pub extern "C" fn ori_map_keys_to_list(keys: *const u8, len: i64, key_size: i64, out_ptr: *mut u8) {
    write_array_to_list(keys, len, key_size, out_ptr);
}

/// Extract map values as a new list.
///
/// Allocates a new buffer and copies all values. Writes `{len, len, data_ptr}`
/// to `out_ptr` (sret pattern). The resulting list owns its data buffer.
#[no_mangle]
pub extern "C" fn ori_map_values_to_list(
    vals: *const u8,
    len: i64,
    val_size: i64,
    out_ptr: *mut u8,
) {
    write_array_to_list(vals, len, val_size, out_ptr);
}

/// Shared helper: copy a contiguous array into a new list struct via sret.
fn write_array_to_list(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let n = len.max(0) as usize;

    if data.is_null() || n == 0 {
        unsafe {
            out_ptr.cast::<i64>().write(0);
            out_ptr.cast::<i64>().add(1).write(0);
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut());
        }
        return;
    }

    let total = n * es;
    let new_data = crate::ori_alloc(total, 8);
    unsafe {
        std::ptr::copy_nonoverlapping(data, new_data, total);
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Safely dereference an `OriStr` pointer, returning `""` for null.
///
/// # Safety
///
/// If non-null, `ptr` must point to a valid `OriStr`.
unsafe fn deref_str<'a>(ptr: *const OriStr) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        (*ptr).as_str()
    }
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

    OriStr::from_owned(format!("{a_str}{b_str}"))
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

/// Hash a string using FNV-1a (64-bit).
///
/// Returns a deterministic i64 hash of the string's bytes. Uses the same
/// FNV-1a constants as the LLVM derive codegen for struct hashing, so
/// hash values compose correctly via `hash_combine`.
#[no_mangle]
pub extern "C" fn ori_str_hash(s: *const OriStr) -> i64 {
    const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;

    let bytes = if s.is_null() {
        &[]
    } else {
        // SAFETY: Caller ensures s points to a valid OriStr
        let ori_str = unsafe { &*s };
        if ori_str.data.is_null() || ori_str.len <= 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(ori_str.data, ori_str.len as usize) }
        }
    };

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash as i64
}

/// Convert an integer to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_int(n: i64) -> OriStr {
    OriStr::from_owned(n.to_string())
}

/// Create an `OriStr` from a raw pointer + length (e.g., string literals).
///
/// Copies the data into an RC-managed heap allocation so the resulting
/// string can safely participate in ARC `RcInc`/`RcDec` operations.
#[no_mangle]
pub extern "C" fn ori_str_from_raw(src: *const u8, len: i64) -> OriStr {
    if src.is_null() || len <= 0 {
        return OriStr {
            len: 0,
            data: std::ptr::null(),
        };
    }
    let size = len as usize;
    let data = ori_rc_alloc(size, 1);
    if !data.is_null() {
        // SAFETY: src is valid for `size` bytes; data is freshly allocated with at least `size` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(src, data, size);
        }
    }
    OriStr {
        len,
        data: data as *const u8,
    }
}

/// Convert a boolean to a string.
///
/// Returns a heap-allocated `OriStr`, matching the ownership semantics of
/// `ori_str_from_int` and `ori_str_from_float`. All `ori_str_from_*` functions
/// must return owned strings so LLVM-generated cleanup code can uniformly free them.
#[no_mangle]
pub extern "C" fn ori_str_from_bool(b: bool) -> OriStr {
    let result = if b { "true" } else { "false" };
    OriStr::from_owned(result.to_string())
}

/// Convert a float to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_float(f: f64) -> OriStr {
    OriStr::from_owned(f.to_string())
}

// -- String methods --

/// Check if a string contains a substring.
#[no_mangle]
pub extern "C" fn ori_str_contains(s: *const OriStr, needle: *const OriStr) -> bool {
    let (s, needle) = unsafe { (deref_str(s), deref_str(needle)) };
    s.contains(needle)
}

/// Check if a string starts with a prefix.
#[no_mangle]
pub extern "C" fn ori_str_starts_with(s: *const OriStr, prefix: *const OriStr) -> bool {
    let (s, prefix) = unsafe { (deref_str(s), deref_str(prefix)) };
    s.starts_with(prefix)
}

/// Check if a string ends with a suffix.
#[no_mangle]
pub extern "C" fn ori_str_ends_with(s: *const OriStr, suffix: *const OriStr) -> bool {
    let (s, suffix) = unsafe { (deref_str(s), deref_str(suffix)) };
    s.ends_with(suffix)
}

/// Trim whitespace from both ends of a string.
#[no_mangle]
pub extern "C" fn ori_str_trim(s: *const OriStr) -> OriStr {
    let s = unsafe { deref_str(s) };
    OriStr::from_owned(s.trim().to_owned())
}

/// Convert a string to uppercase.
#[no_mangle]
pub extern "C" fn ori_str_to_uppercase(s: *const OriStr) -> OriStr {
    let s = unsafe { deref_str(s) };
    OriStr::from_owned(s.to_uppercase())
}

/// Convert a string to lowercase.
#[no_mangle]
pub extern "C" fn ori_str_to_lowercase(s: *const OriStr) -> OriStr {
    let s = unsafe { deref_str(s) };
    OriStr::from_owned(s.to_lowercase())
}

/// Replace all occurrences of `from` with `to` in a string.
#[no_mangle]
pub extern "C" fn ori_str_replace(
    s: *const OriStr,
    from: *const OriStr,
    to: *const OriStr,
) -> OriStr {
    let (s, from, to) = unsafe { (deref_str(s), deref_str(from), deref_str(to)) };
    OriStr::from_owned(s.replace(from, to))
}

/// Repeat a string `count` times.
#[no_mangle]
pub extern "C" fn ori_str_repeat(s: *const OriStr, count: i64) -> OriStr {
    let s = unsafe { deref_str(s) };
    OriStr::from_owned(s.repeat(count.max(0) as usize))
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
/// Uses `AtomicPtr` with `Relaxed` ordering (matches Swift's panic handler
/// registration pattern). The write happens during single-threaded `main()`
/// init, and reads happen during panic handling which is on a happens-after
/// path. Thread-local `IN_PANIC_HANDLER` provides re-entrancy protection.
static ORI_PANIC_TRAMPOLINE: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

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
    let ptr = ORI_PANIC_TRAMPOLINE.load(Ordering::Relaxed);
    if ptr.is_null() {
        return;
    }
    // SAFETY: Non-null pointer was set by ori_register_panic_handler which
    // transmuted a valid PanicTrampoline function pointer.
    let trampoline: PanicTrampoline = unsafe { std::mem::transmute(ptr) };

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
    ORI_PANIC_TRAMPOLINE.store(handler as *mut (), Ordering::Relaxed);
}

// ── String iteration ─────────────────────────────────────────────────────

/// Result of decoding a single UTF-8 character from a string.
///
/// Returned by `ori_str_next_char`. The codepoint is the Unicode scalar value
/// (as i32/char), and `next_offset` is the byte offset of the next character.
#[repr(C)]
pub struct OriCharResult {
    /// Unicode codepoint (i32). -1 on error or end-of-string.
    pub codepoint: i32,
    /// Byte offset of the next character. Equals `byte_offset + char_width`.
    pub next_offset: i64,
}

/// Decode the next UTF-8 character from a string at the given byte offset.
///
/// Returns the Unicode codepoint and the byte offset of the next character.
/// If `byte_offset >= len` or the byte sequence is invalid, returns
/// codepoint = -1 and `next_offset = len` (termination sentinel).
///
/// # Parameters
/// - `data`: Pointer to the string's UTF-8 byte data
/// - `len`: Total byte length of the string
/// - `byte_offset`: Current byte position to decode from
#[no_mangle]
pub extern "C" fn ori_str_next_char(data: *const u8, len: i64, byte_offset: i64) -> OriCharResult {
    if data.is_null() || byte_offset < 0 || byte_offset >= len {
        return OriCharResult {
            codepoint: -1,
            next_offset: len,
        };
    }

    let offset = byte_offset as usize;
    let length = len as usize;
    let remaining = length - offset;

    // SAFETY: data is valid for `len` bytes, offset < len
    let lead = unsafe { *data.add(offset) };

    // Decode UTF-8 leading byte to determine character width
    let (codepoint, width) = if lead < 0x80 {
        // 1-byte: 0xxxxxxx (ASCII)
        (i32::from(lead), 1)
    } else if lead < 0xC0 {
        // Continuation byte in leading position — invalid
        return OriCharResult {
            codepoint: -1,
            next_offset: byte_offset + 1,
        };
    } else if lead < 0xE0 && remaining >= 2 {
        // 2-byte: 110xxxxx 10xxxxxx
        let b1 = unsafe { *data.add(offset + 1) };
        let cp = (i32::from(lead & 0x1F) << 6) | i32::from(b1 & 0x3F);
        (cp, 2)
    } else if lead < 0xF0 && remaining >= 3 {
        // 3-byte: 1110xxxx 10xxxxxx 10xxxxxx
        let b1 = unsafe { *data.add(offset + 1) };
        let b2 = unsafe { *data.add(offset + 2) };
        let cp =
            (i32::from(lead & 0x0F) << 12) | (i32::from(b1 & 0x3F) << 6) | i32::from(b2 & 0x3F);
        (cp, 3)
    } else if lead < 0xF8 && remaining >= 4 {
        // 4-byte: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
        let b1 = unsafe { *data.add(offset + 1) };
        let b2 = unsafe { *data.add(offset + 2) };
        let b3 = unsafe { *data.add(offset + 3) };
        let cp = (i32::from(lead & 0x07) << 18)
            | (i32::from(b1 & 0x3F) << 12)
            | (i32::from(b2 & 0x3F) << 6)
            | i32::from(b3 & 0x3F);
        (cp, 4)
    } else {
        // Incomplete multi-byte sequence or invalid lead byte
        return OriCharResult {
            codepoint: -1,
            next_offset: byte_offset + 1,
        };
    };

    OriCharResult {
        codepoint,
        next_offset: byte_offset + width,
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
/// Exit codes:
/// - **0**: success
/// - **1**: panic
/// - **2**: RC leak detected (only when `ORI_CHECK_LEAKS=1`)
#[no_mangle]
pub extern "C" fn ori_run_main(main_fn: extern "C" fn()) -> i32 {
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        main_fn();
    }));

    match result {
        Ok(()) => {
            // Check for RC leaks when enabled
            if check_leaks_enabled() {
                let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
                if live != 0 {
                    eprintln!("ori: {live} RC allocation(s) not freed (memory leak)");
                    return 2;
                }
            }
            0
        }
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

/// Check whether ARC leak detection is enabled via environment variable.
///
/// Caches the result in a `OnceLock` to avoid repeated `getenv` syscalls.
/// Enabled by `ORI_CHECK_LEAKS=1` or `ORI_CHECK_LEAKS=true`.
fn check_leaks_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("ORI_CHECK_LEAKS")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests;
