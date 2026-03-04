//! I/O, panic, and assertion functions.
//!
//! Provides the runtime's interaction with the outside world:
//! - **Print**: `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool`
//! - **Panic**: `ori_panic`, `ori_panic_cstr` (with JIT recovery + user handler dispatch)
//! - **Assert**: `ori_assert`, `ori_assert_eq_*`
//! - **Catch/recover**: `ori_catch_cleanup`, `ori_catch_recover`
//! - **JIT recovery**: `setjmp`/`longjmp`-based error recovery for test runners
//! - **Panic handler**: `ori_register_panic_handler` for user `@panic` functions

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::OriStr;

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

    /// Free a caught exception object (Itanium ABI).
    ///
    /// Calls the exception's `exception_cleanup` callback (which is
    /// `ori_exception_cleanup` for Ori exceptions — frees the malloc'd
    /// `OriException` object).
    ///
    /// Only used on Itanium targets where `catch(expr:)` landing pads
    /// receive the raw exception pointer. On MSVC, SEH handles cleanup
    /// via __except — no explicit delete needed.
    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    fn _Unwind_DeleteException(exception: *mut u8);
}

// `ori_raise_exception` is implemented in C (`eh_personality.c`) on ALL platforms:
//   - Itanium: _Unwind_RaiseException with OriException object
//   - MSVC: Win32 RaiseException with custom SEH exception code
//
// On Itanium, `C-unwind` tells Rust the function may unwind — without this,
// Rust inserts an abort guard that kills the process before the exception
// reaches LLVM landing pads.
//
// On MSVC, RaiseException is non-returning (EXCEPTION_NONCONTINUABLE),
// so `extern "C"` is sufficient — no Rust unwind tables needed.
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
extern "C-unwind" {
    fn ori_raise_exception() -> !;
}

#[cfg(all(target_os = "windows", target_env = "msvc"))]
extern "C" {
    fn ori_raise_exception() -> !;
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

// ── Print functions ──────────────────────────────────────────────────────

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

// ── Panic functions ──────────────────────────────────────────────────────

/// Panic with a message.
///
/// Dispatch order:
/// 1. Store panic state (for JIT test assertions)
/// 2. If user `@panic` handler registered and not re-entrant: call trampoline
/// 3. If JIT mode: `longjmp` back to test runner
/// 4. AOT default: raise C exception via `_Unwind_RaiseException`
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

    // AOT path: unwind via exception raising.
    eprintln!("ori panic: {msg}");
    aot_raise_exception(msg);
}

/// Panic with a C string message.
///
/// Same dispatch order as `ori_panic`: user handler → JIT longjmp → unwind.
#[no_mangle]
pub extern "C-unwind" fn ori_panic_cstr(s: *const c_char) {
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

    // AOT path: unwind via exception raising.
    eprintln!("ori panic: {msg}");
    aot_raise_exception(msg);
}

/// Raise an exception for AOT panic paths.
///
/// Implemented in C (`eh_personality.c`) on all platforms:
///   - Itanium (Linux, macOS, MinGW): `_Unwind_RaiseException` with `OriException`
///   - Windows MSVC: `RaiseException` with custom SEH exception code
///
/// The panic message was already stored in thread-local storage by
/// `ori_panic`/`ori_panic_cstr` before this is called.
fn aot_raise_exception(_msg: String) -> ! {
    // SAFETY: ori_raise_exception is implemented in eh_personality.c,
    // compiled and linked via build.rs. It never returns.
    unsafe { ori_raise_exception() }
}

// ── Catch/recover ────────────────────────────────────────────────────────

/// Free a caught exception from a `catch(expr:)` landing pad.
///
/// Called from LLVM-generated catch-all landing pads after extracting the
/// exception pointer from the `landingpad catch null` result.
///
/// On Itanium targets, calls `_Unwind_DeleteException` which invokes the
/// exception's cleanup callback (`ori_exception_cleanup` → `free()`).
///
/// On MSVC targets, `catch(expr:)` uses `ori_try_call` + `catch_unwind`
/// instead of landing pads, so this function is a no-op there.
///
/// The panic message is recovered via thread-local storage in
/// [`ori_catch_recover`], not from the exception payload.
#[no_mangle]
pub extern "C" fn ori_catch_cleanup(exc_ptr: *mut u8) {
    if exc_ptr.is_null() {
        return;
    }
    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    unsafe {
        _Unwind_DeleteException(exc_ptr);
    }
}

// ori_try_call is implemented in C (eh_personality.c) on MSVC using
// __try/__except to catch Ori's custom SEH exception. On Itanium targets,
// catch(expr:) uses LLVM invoke/landingpad instead — ori_try_call is not
// called but is still linked (the symbol exists in the C object file only
// on MSVC via #ifdef _MSC_VER).

/// Recover from a caught panic — reads the panic message from thread-local storage.
///
/// Called from `catch(expr:)` after `ori_catch_cleanup` has freed the
/// exception object. Returns the panic message as an `OriStr`. The message
/// was stored in thread-local storage by `ori_panic`/`ori_panic_cstr`
/// before unwinding. Clears the panic state so subsequent catches work correctly.
#[no_mangle]
pub extern "C" fn ori_catch_recover() -> OriStr {
    let msg = PANIC_MESSAGE.with(|m| m.borrow_mut().take());
    PANIC_OCCURRED.with(|p| *p.borrow_mut() = false);
    let text = msg.unwrap_or_else(|| "unknown panic".to_string());
    OriStr::from_owned(&text)
}

// ── Assert functions ─────────────────────────────────────────────────────

/// Assert that a condition is true.
///
/// On failure, routes through `ori_panic_cstr` which handles JIT
/// (longjmp to test runner) and AOT (unwind via Ori exception) paths.
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
        ori_panic_cstr(msg.as_ptr().cast::<c_char>());
    }
}

/// Assert that two booleans are equal.
///
/// On failure, formats a message and routes through `ori_panic_cstr`.
#[no_mangle]
pub extern "C-unwind" fn ori_assert_eq_bool(actual: bool, expected: bool) {
    if actual != expected {
        let msg = format!("assertion failed: {actual} != {expected}\0");
        ori_panic_cstr(msg.as_ptr().cast::<c_char>());
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
        ori_panic_cstr(msg.as_ptr().cast::<c_char>());
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
        ori_panic_cstr(msg.as_ptr().cast::<c_char>());
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
