//! JIT panic recovery via `setjmp`/`longjmp` (Itanium) or `__try`/`__except` (MSVC).
//!
//! Provides platform-abstracted JIT execution protection:
//! - **`jit_run_protected`**: Run a JIT-compiled function, catching panics.
//! - **`JmpBuf`**: Oversized buffer for all platform `jmp_buf` layouts.
//! - **JIT mode**: Thread-local state that redirects panics to `longjmp` (Itanium only).
//!
//! On MSVC, `_setjmp` is a compiler intrinsic with a hidden frame pointer argument —
//! it cannot be called via Rust FFI. Instead, `ori_try_call` uses `__try`/`__except`.

use super::panic_state::{did_panic, get_panic_message, reset_panic_state};

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

// On MSVC, _setjmp is a compiler intrinsic that expects a hidden frame pointer
// argument — it cannot be called via Rust FFI. Use ori_try_call (__try/__except)
// for panic recovery instead. See `jit_run_protected`.
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
extern "C" {
    /// Save the current execution state. Returns 0 on direct call,
    /// non-zero when returning via `longjmp`.
    ///
    /// Uses `_setjmp` (POSIX): does NOT save the signal mask, which is faster
    /// and sufficient for JIT error recovery.
    #[link_name = "_setjmp"]
    pub(super) fn c_setjmp(buf: *mut JmpBuf) -> i32;

    /// Restore execution state saved by `setjmp`. Never returns to caller.
    pub(super) fn longjmp(buf: *mut JmpBuf, val: i32) -> !;

    /// Free a caught exception object (Itanium ABI).
    ///
    /// Calls the exception's `exception_cleanup` callback (which is
    /// `ori_exception_cleanup` for Ori exceptions — frees the malloc'd
    /// `OriException` object).
    ///
    /// Only used on Itanium targets where `catch(expr:)` landing pads
    /// receive the raw exception pointer. On MSVC, SEH handles cleanup
    /// via __except — no explicit delete needed.
    pub(super) fn _Unwind_DeleteException(exception: *mut u8);
}

// JIT mode (setjmp/longjmp) is only used on Itanium targets.
// On MSVC, jit_run_protected uses ori_try_call (__try/__except) instead.
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
use std::cell::Cell;

#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
thread_local! {
    /// Whether the current thread is running JIT-compiled code.
    /// When true, `ori_panic`/`ori_panic_cstr` will `longjmp` instead of `exit(1)`.
    static JIT_MODE: Cell<bool> = const { Cell::new(false) };

    /// Pointer to the active `JmpBuf` for JIT recovery.
    /// Only valid when `JIT_MODE` is true.
    pub(super) static JIT_RECOVERY_BUF: Cell<*mut JmpBuf> = const { Cell::new(std::ptr::null_mut()) };
}

/// Enter JIT mode: panics will `longjmp` to `buf` instead of terminating.
///
/// # Safety
///
/// `buf` must point to a valid `JmpBuf` that outlives the JIT execution.
/// The caller must call `leave_jit_mode()` when done (even on `longjmp` return).
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
pub fn enter_jit_mode(buf: *mut JmpBuf) {
    JIT_MODE.with(|m| m.set(true));
    JIT_RECOVERY_BUF.with(|b| b.set(buf));
}

/// Leave JIT mode: panics will `exit(1)` again (AOT behavior).
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
pub fn leave_jit_mode() {
    JIT_MODE.with(|m| m.set(false));
    JIT_RECOVERY_BUF.with(|b| b.set(std::ptr::null_mut()));
}

/// Check if we're currently in JIT mode.
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
pub(super) fn is_jit_mode() -> bool {
    JIT_MODE.with(std::cell::Cell::get)
}

/// Call `setjmp` on a `JmpBuf`. Returns 0 on direct call, non-zero on `longjmp`.
///
/// # Safety
///
/// `buf` must point to a valid, properly aligned `JmpBuf`.
///
/// **WARNING**: On Windows MSVC, `_setjmp` is a compiler intrinsic that expects
/// a hidden frame pointer argument. This FFI call works on Itanium targets
/// (Linux, macOS) but **corrupts the stack on MSVC**. Use [`jit_run_protected`]
/// instead for cross-platform JIT panic recovery.
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
pub unsafe fn jit_setjmp(buf: *mut JmpBuf) -> i32 {
    c_setjmp(buf)
}

// ── JIT panic recovery (platform-abstracted) ─────────────────────────────

/// Run a JIT-compiled function with panic recovery.
///
/// Returns `Ok(())` on success, `Err(message)` if the function panicked.
///
/// # Platform behavior
///
/// - **Itanium** (Linux, macOS): Uses `setjmp`/`longjmp` via JIT mode.
///   Panics trigger `longjmp` back to this function's `setjmp` save point.
///
/// - **MSVC** (Windows): Uses `ori_try_call` (`__try`/`__except` in C).
///   Panics trigger `RaiseException` which is caught by the SEH handler.
///   `_setjmp` cannot be called via Rust FFI on MSVC because it's a
///   compiler intrinsic that expects a hidden frame pointer argument.
///
/// # Safety
///
/// `func` must be a valid JIT-compiled function pointer with `() -> void`
/// signature.
#[allow(unsafe_code, reason = "JIT execution requires unsafe FFI")]
pub unsafe fn jit_run_protected(func: unsafe extern "C" fn()) -> Result<(), String> {
    reset_panic_state();

    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        // On MSVC, use ori_try_call (C++ try/catch) to catch OriPanicException.
        // Do NOT enter JIT mode — let ori_panic fall through to
        // ori_raise_exception (C++ throw), which ori_try_call catches.
        // Thunk is `C-unwind` so the C++ exception can propagate through it.
        extern "C" {
            fn ori_try_call(thunk: unsafe extern "C-unwind" fn(*mut u8), ctx: *mut u8) -> i64;
        }

        unsafe extern "C-unwind" fn jit_thunk(ctx: *mut u8) {
            let f: unsafe extern "C" fn() = unsafe { std::mem::transmute(ctx) };
            unsafe { f() };
        }

        let succeeded = unsafe { ori_try_call(jit_thunk, func as *mut u8) };
        if succeeded == 1 {
            // Also check panic state as a safety net
            if did_panic() {
                let msg = get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
                return Err(msg);
            }
            return Ok(());
        }
        // Panic was caught — recover message from thread-local storage
        let msg = get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
        Err(msg)
    }

    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    {
        // On Itanium, use setjmp/longjmp via JIT mode.
        let mut jmp_buf = JmpBuf::new();
        let buf_ptr: *mut JmpBuf = &raw mut jmp_buf;
        enter_jit_mode(buf_ptr);

        let longjmp_fired = unsafe { c_setjmp(buf_ptr) } != 0;

        if longjmp_fired {
            leave_jit_mode();
            let msg = get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            return Err(msg);
        }

        unsafe { func() };

        leave_jit_mode();

        // Safety net — check panic state even if longjmp didn't fire
        if did_panic() {
            let msg = get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            return Err(msg);
        }

        Ok(())
    }
}
