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

// Print functions

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

// Panic functions

/// Panic with a message. OWNS the message: the caller transfers its message
/// reference (callers pass a fresh +1 — ARC marks the arg Owned and dup-incs
/// still-live messages; non-ARC emission sites inc explicitly).
///
/// Dispatch order:
/// 1. Store panic state (for JIT test assertions) — copies the message text
/// 2. Release the original message buffer (exactly-once: no caller-side
///    release fires on any panic path; SSO/slice/immortal-aware)
/// 3. If user `@panic` handler registered and not re-entrant: call trampoline
/// 4. Raise C exception (`_Unwind_RaiseException` on Itanium, `RaiseException` on MSVC).
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
        let text = text.to_string();

        // Release the transferred message reference — the text was copied
        // above, and `ori_catch_recover` reads thread-local state, never this
        // buffer. ori_str_rc_dec skips SSO (bit-63 flag survives the union
        // read of `heap.data`), routes slices to the original buffer, and
        // no-ops on immortal (MAX_REFCOUNT) headers.
        // SAFETY: heap union fields are plain i64/ptr reads valid for any
        // 24-byte OriStr; ori_str_rc_dec classifies SSO/heap/slice itself.
        unsafe {
            crate::rc::ori_str_rc_dec(
                ori_str.heap.data,
                ori_str.heap.cap,
                Some(crate::rc::ori_str_drop_buffer),
            );
        }
        text
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
    // Nested drop-panic: a panic raised while a drop-cleanup region is active
    // (a codegen cleanup landing pad running drop work on the unwind path) is
    // the double-panic case — abort instead of unwinding further
    // (drop-trait-proposal.md §Drop and panic).
    if super::rc::drop_cleanup_active() {
        super::rc::ori_drop_double_panic_abort();
    }
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

// Catch/recover

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

// Assert functions

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

/// Traceable `TraceEntry` struct layout representation for FFI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OriTraceEntry {
    pub function: crate::OriStr,
    pub file: crate::OriStr,
    pub line: i64,
    pub column: i64,
}

extern "C" fn trace_entry_inc_fn(ptr: *mut u8) {
    let entry = unsafe { &*(ptr.cast::<OriTraceEntry>()) };
    unsafe {
        let func_data = entry.function.heap.data;
        let func_cap = entry.function.heap.cap;
        crate::rc::ori_str_rc_inc(func_data, func_cap);

        let file_data = entry.file.heap.data;
        let file_cap = entry.file.heap.cap;
        crate::rc::ori_str_rc_inc(file_data, file_cap);
    }
}

extern "C" fn trace_entry_dec_fn(ptr: *mut u8) {
    let entry = unsafe { &*(ptr.cast::<OriTraceEntry>()) };
    unsafe {
        let func_data = entry.function.heap.data;
        let func_cap = entry.function.heap.cap;
        crate::rc::ori_str_rc_dec(func_data, func_cap, Some(crate::rc::ori_str_drop_buffer));

        let file_data = entry.file.heap.data;
        let file_cap = entry.file.heap.cap;
        crate::rc::ori_str_rc_dec(file_data, file_cap, Some(crate::rc::ori_str_drop_buffer));
    }
}

/// Error struct layout representation for FFI.
#[repr(C)]
pub struct OriError {
    pub message: crate::OriStr,
    pub trace: crate::OriList,
}

/// Inject a trace entry into an Error struct.
///
/// `function` and `file` are passed by pointer, not by value: `OriStr` is a
/// 24-byte struct, and the `SysV` AMD64 C ABI classifies a struct larger than
/// 16 bytes as MEMORY, passed indirectly. Passing it as a raw `{i64,i64,ptr}`
/// value across the LLVM->Rust FFI boundary mis-classifies the eightbytes and
/// shifts every following argument. Mirrors `_ori_error_with_trace`, which
/// passes its struct operands by pointer for the same reason.
#[no_mangle]
pub extern "C" fn _ori_inject_trace_entry(
    error_ptr: *mut OriError,
    function: *const crate::OriStr,
    file: *const crate::OriStr,
    line: i64,
    column: i64,
) {
    if error_ptr.is_null() || function.is_null() || file.is_null() {
        return;
    }
    // SAFETY: error_ptr is verified non-null. Caller guarantees it points to a valid OriError.
    let error = unsafe { &mut *error_ptr };
    // SAFETY: function/file are verified non-null; the caller spills each OriStr
    // to an alloca and passes its pointer per the by-pointer ABI above.
    let entry = OriTraceEntry {
        function: unsafe { *function },
        file: unsafe { *file },
        line,
        column,
    };

    let mut out_list = crate::OriList {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    };
    let out_list_ptr = std::ptr::from_mut::<crate::OriList>(&mut out_list).cast::<u8>();
    let entry_bytes = std::ptr::from_ref::<OriTraceEntry>(&entry).cast::<u8>();
    let elem_size = std::mem::size_of::<OriTraceEntry>() as i64;

    // Mutation is COW-aware and consumes the trace reference owned by `error`.
    crate::ori_list_push_cow(
        error.trace.data,
        error.trace.len,
        error.trace.cap,
        entry_bytes,
        elem_size,
        std::mem::align_of::<OriTraceEntry>() as i64,
        Some(trace_entry_inc_fn),
        0, // cow_mode = dynamic (0)
        out_list_ptr,
    );

    if !out_list.data.is_null() {
        unsafe {
            crate::rc::store_elem_dec_fn(out_list.data, Some(trace_entry_dec_fn));
        }
    }
    error.trace = out_list;
}

/// Format the trace of an Error struct as a string.
#[no_mangle]
pub extern "C" fn _ori_format_error_trace(error_ptr: *const OriError) -> crate::OriStr {
    if error_ptr.is_null() {
        return crate::OriStr::from_owned("");
    }
    // SAFETY: error_ptr is verified non-null. Caller guarantees it points to a valid OriError.
    let error = unsafe { &*error_ptr };
    let trace_len = error.trace.len;
    if trace_len <= 0 || error.trace.data.is_null() {
        return crate::OriStr::from_owned("");
    }

    let mut parts = Vec::new();
    let entries = unsafe {
        std::slice::from_raw_parts(error.trace.data.cast::<OriTraceEntry>(), trace_len as usize)
    };
    for entry in entries {
        // SAFETY: entry is a valid OriTraceEntry with properly initialized string fields.
        let func = unsafe { entry.function.as_str() };
        let file = unsafe { entry.file.as_str() };
        let formatted = format!("{} at {}:{}:{}", func, file, entry.line, entry.column);
        parts.push(formatted);
    }
    let joined = parts.join("\n");
    crate::OriStr::from_owned(&joined)
}

/// Create a new Error struct with an additional trace entry appended.
#[no_mangle]
pub extern "C" fn _ori_error_with_trace(
    out_ptr: *mut OriError,
    error_ptr: *const OriError,
    entry_ptr: *const OriTraceEntry,
) {
    if out_ptr.is_null() {
        return;
    }
    if error_ptr.is_null() || entry_ptr.is_null() {
        unsafe {
            std::ptr::write(
                out_ptr,
                OriError {
                    message: crate::OriStr::from_owned(""),
                    trace: crate::OriList {
                        len: 0,
                        cap: 0,
                        data: std::ptr::null_mut(),
                    },
                },
            );
            return;
        }
    }
    // SAFETY: error_ptr and entry_ptr are verified non-null.
    let error = unsafe { &*error_ptr };
    let entry = unsafe { &*entry_ptr };

    let message = unsafe {
        let cap = error.message.heap.cap;
        let data = error.message.heap.data;
        crate::rc::ori_str_rc_inc(data, cap);
        error.message
    };

    // Increment trace's RC to transfer ownership of one reference to `ori_list_push_cow`.
    crate::rc::ori_list_rc_inc(error.trace.data, error.trace.cap);

    let entry_copy = *entry;
    unsafe {
        let func_data = entry_copy.function.heap.data;
        let func_cap = entry_copy.function.heap.cap;
        crate::rc::ori_str_rc_inc(func_data, func_cap);

        let file_data = entry_copy.file.heap.data;
        let file_cap = entry_copy.file.heap.cap;
        crate::rc::ori_str_rc_inc(file_data, file_cap);
    }

    let mut trace = crate::OriList {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    };
    let trace_out_ptr = std::ptr::from_mut::<crate::OriList>(&mut trace).cast::<u8>();
    let entry_bytes = std::ptr::from_ref::<OriTraceEntry>(&entry_copy).cast::<u8>();
    let elem_size = std::mem::size_of::<OriTraceEntry>() as i64;

    crate::ori_list_push_cow(
        error.trace.data,
        error.trace.len,
        error.trace.cap,
        entry_bytes,
        elem_size,
        std::mem::align_of::<OriTraceEntry>() as i64,
        Some(trace_entry_inc_fn),
        0, // cow_mode = dynamic (0)
        trace_out_ptr,
    );

    if !trace.data.is_null() {
        unsafe {
            crate::rc::store_elem_dec_fn(trace.data, Some(trace_entry_dec_fn));
        }
    }

    unsafe {
        std::ptr::write(out_ptr, OriError { message, trace });
    }
}
