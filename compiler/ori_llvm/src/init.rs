//! LLVM initialization: fatal error handler and tracing setup.

use std::sync::Once;

static TRACING_INIT: Once = Once::new();
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

/// Initialize tracing for debug output.
///
/// Call this once at startup. Safe to call multiple times.
/// Enable with `RUST_LOG=ori_llvm=debug` or `RUST_LOG=ori_llvm=trace`.
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};

        // Only initialize if RUST_LOG is set
        if std::env::var("RUST_LOG").is_ok() {
            let filter = EnvFilter::from_default_env();
            tracing_subscriber::registry()
                .with(fmt::layer().with_target(true).with_level(true))
                .with(filter)
                .init();
        }
    });
}
