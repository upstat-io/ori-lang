//! LLVM initialization: fatal error handler.
//!
//! Tracing initialization is the sole responsibility of `oric::tracing_setup`.
//! See BUG-07-007 — this file previously contained a parallel `init_tracing()`
//! that duplicated `tracing_setup.rs` with different behavior (only `RUST_LOG`,
//! no `ORI_LOG`, no tree mode, no default, `.init()` instead of `.try_init()`).
//! It had zero callers and was removed as a SSOT violation.

use std::sync::Once;

static FATAL_HANDLER_INIT: Once = Once::new();

/// Install a custom LLVM fatal error handler that logs instead of aborting.
///
/// By default, LLVM calls `abort()` on fatal errors (e.g., "unable to allocate
/// function return"), which kills the entire process. This replaces that handler
/// with one that logs the error. Note: the handler cannot prevent abort since
/// panicking across `extern "C"` boundaries is not allowed.
///
/// Safe to call multiple times — only the first call takes effect.
pub fn install_fatal_error_handler() {
    FATAL_HANDLER_INIT.call_once(|| {
        // SAFETY: `LLVMInstallFatalErrorHandler` is called once during
        // initialization with a valid function pointer.
        unsafe {
            llvm_sys::error_handling::LLVMInstallFatalErrorHandler(Some(llvm_fatal_error_handler));
        }
    });
}

/// LLVM fatal error callback that logs the error.
///
/// Cannot unwind (extern "C"), so we log and let LLVM abort.
extern "C" fn llvm_fatal_error_handler(reason: *const std::ffi::c_char) {
    let msg = if reason.is_null() {
        "unknown LLVM fatal error".to_string()
    } else {
        // SAFETY: LLVM guarantees a valid C string pointer in the callback.
        unsafe { std::ffi::CStr::from_ptr(reason) }
            .to_string_lossy()
            .into_owned()
    };
    eprintln!("LLVM fatal error (aborting): {msg}");
}
