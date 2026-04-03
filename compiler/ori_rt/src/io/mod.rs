//! I/O, panic, and assertion functions.
//!
//! Provides the runtime's interaction with the outside world:
//! - **Print**: `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool`
//! - **Panic**: `ori_panic`, `ori_panic_cstr` (with JIT recovery + user handler dispatch)
//! - **Assert**: `ori_assert`, `ori_assert_eq_*`
//! - **Catch/recover**: `ori_catch_cleanup`, `ori_catch_recover`
//! - **JIT recovery**: LLVM `invoke`/`landingpad` for test wrappers; legacy `setjmp`/`longjmp` fallback in `jit_run_protected` (`jit_recovery`)
//! - **Panic handler**: `ori_register_panic_handler` for user `@panic` functions (`panic_state`)

pub(crate) mod jit_recovery;
mod panic_state;

use std::ffi::{c_char, CStr};

use crate::OriStr;

// Re-export public API from submodules
pub use jit_recovery::{jit_run_protected, JmpBuf};
pub use panic_state::{
    did_panic, get_panic_message, ori_register_panic_handler, reset_panic_state,
    set_panic_state_for_test,
};

#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
pub use jit_recovery::{enter_jit_mode, jit_setjmp, leave_jit_mode};

// `ori_raise_exception` is implemented in platform-specific C/C++:
//   - Itanium: _Unwind_RaiseException with OriException object (eh_personality.c)
//   - MSVC: C++ throw OriPanicException (eh_personality_msvc.cpp)
//
// Both unwind, so `C-unwind` is required — without this, Rust inserts an
// abort guard that kills the process before the exception reaches LLVM
// cleanup pads (Itanium: landingpad, MSVC: cleanuppad) or ori_try_call.
extern "C-unwind" {
    fn ori_raise_exception() -> !;
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
    // SAFETY: OriStr::as_str reads from the inline SSO buffer or heap data pointer.
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
/// 3. Raise C exception (`_Unwind_RaiseException` on Itanium, `RaiseException` on MSVC).
///    `catch(expr:)` landing pads catch the exception; uncaught panics propagate
///    to the JIT test wrapper's catch-all landing pad.
#[no_mangle]
pub extern "C-unwind" fn ori_panic(s: *const OriStr) {
    let msg = if s.is_null() {
        "panic!".to_string()
    } else {
        // SAFETY: Caller ensures s points to a valid OriStr
        let ori_str = unsafe { &*s };
        // SAFETY: OriStr::as_str reads from the inline SSO buffer or heap data pointer.
        let text = unsafe { ori_str.as_str() };
        text.to_string()
    };

    // Store panic state in thread-local storage
    panic_state::store_panic(&msg);

    // Call user @panic handler if registered (AOT only, not re-entrant).
    // If no handler is registered, print the default panic message.
    if !panic_state::call_panic_trampoline(&msg) {
        eprintln!("ori panic: {msg}");
    }

    dispatch_panic(msg);
}

/// Panic with a C string message.
///
/// Same dispatch order as `ori_panic`: user handler → C exception.
#[no_mangle]
pub extern "C-unwind" fn ori_panic_cstr(s: *const c_char) {
    let msg = if s.is_null() {
        "panic!".to_string()
    } else {
        // SAFETY: Caller ensures s points to a valid C string
        let cstr = unsafe { CStr::from_ptr(s) };
        cstr.to_string_lossy().to_string()
    };

    panic_state::store_panic(&msg);

    if !panic_state::call_panic_trampoline(&msg) {
        eprintln!("ori panic: {msg}");
    }

    dispatch_panic(msg);
}

/// Choose the correct panic recovery mechanism.
///
/// - **JIT mode** (Itanium only): `longjmp` back to the Rust caller's `setjmp`
///   save point. Used when `ori_panic` is called from Rust test infrastructure
///   or `ori_run_main`, which cannot catch foreign C exceptions.
/// - **Otherwise**: Raise a C exception via `_Unwind_RaiseException` (Itanium)
///   or `RaiseException` (MSVC). Caught by LLVM-generated `invoke`/`landingpad`
///   in `catch(expr:)` blocks and JIT test wrappers.
#[expect(
    improper_ctypes_definitions,
    reason = "C-unwind ABI is for unwind semantics, not actual C interop — String stays in Rust frames"
)]
extern "C-unwind" fn dispatch_panic(msg: String) -> ! {
    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    {
        if super::jit_recovery::is_jit_mode() {
            let buf = super::jit_recovery::JIT_RECOVERY_BUF.with(std::cell::Cell::get);
            if !buf.is_null() {
                // SAFETY: buf was set by enter_jit_mode and points to a valid JmpBuf
                // on the caller's stack frame. longjmp never returns.
                unsafe { super::jit_recovery::longjmp(buf, 1) };
            }
        }
    }
    aot_raise_exception(msg);
}

/// Raise an exception for AOT panic paths.
///
/// Implemented in platform-specific C/C++:
///   - Itanium (Linux, macOS, MinGW): `_Unwind_RaiseException` (`eh_personality.c`)
///   - Windows MSVC: C++ `throw OriPanicException` (`eh_personality_msvc.cpp`)
///
/// The panic message was already stored in thread-local storage by
/// `ori_panic`/`ori_panic_cstr` before this is called.
#[expect(
    improper_ctypes_definitions,
    reason = "C-unwind ABI is for unwind semantics, not actual C interop — String stays in Rust frames"
)]
extern "C-unwind" fn aot_raise_exception(_msg: String) -> ! {
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
    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    if !exc_ptr.is_null() {
        // SAFETY: exc_ptr is a non-null Itanium exception object from a landingpad.
        unsafe {
            jit_recovery::_Unwind_DeleteException(exc_ptr);
        }
    }
    // On MSVC, SEH handles cleanup — exc_ptr is unused.
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    let _ = exc_ptr;
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
    let msg = panic_state::take_panic_message();
    let text = msg.unwrap_or_else(|| "unknown panic".to_string());
    OriStr::from_owned(&text)
}

// ── Assert functions ─────────────────────────────────────────────────────

/// Assert that a condition is true.
///
/// On failure, routes through `ori_panic_cstr` which unwinds via
/// Ori exception in both JIT and AOT paths.
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
        // SAFETY: Pointer validated non-null; caller guarantees valid OriStr.
        unsafe { (*actual).as_str() }
    };
    let expected_str = if expected.is_null() {
        ""
    } else {
        // SAFETY: Pointer validated non-null; caller guarantees valid OriStr.
        unsafe { (*expected).as_str() }
    };

    if actual_str != expected_str {
        let msg = format!("assertion failed: \"{actual_str}\" != \"{expected_str}\"\0");
        ori_panic_cstr(msg.as_ptr().cast::<c_char>());
    }
}
