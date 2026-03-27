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
//! # Modules
//!
//! - **rc**: Reference counting (`ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, etc.)
//! - **string**: String types and operations (`OriStr`, `ori_str_concat`, etc.)
//! - **list**: List types and operations (`OriList`, `ori_list_push_cow`, etc.)
//! - **map**: Map types and operations (`OriMap`, `ori_map_insert`, etc.)
//! - **set**: Set operations (`ori_set_insert`, `ori_set_union`, etc.)
//! - **io**: I/O, panic, assert, JIT recovery (`ori_print`, `ori_panic`, etc.)
//! - **format**: Template string interpolation (`ori_format_int`, etc.)
//! - **iterator**: Iterator runtime (`ori_iter_from_list`, `ori_iter_next`, etc.)
//!
//! # COW Architecture
//!
//! Every collection mutation follows the COW (Copy-on-Write) protocol:
//! 1. Check uniqueness: `ori_rc_is_unique(data)` — is RC == 1?
//! 2. If unique (fast path): mutate in place, O(1) amortized
//! 3. If shared (slow path): allocate new buffer, copy, mutate, dec old
//!
//! The static uniqueness analysis (`ori_arc`) can eliminate the runtime
//! check when the value is provably unique at compile time (`cow_mode=1`
//! for static unique, `cow_mode=0` for dynamic check).
//!
//! **Seamless slices** (list `take`/`skip`/`slice`, string `substring`/`trim`)
//! create zero-copy views by encoding a byte offset in the cap field's sign
//! bit (`SLICE_FLAG`). COW mutations on slices materialize a standalone copy.
//! See `slice_encoding` and `list::slice` modules.
//!
//! **SSO** (Small String Optimization): Strings ≤23 bytes are stored inline
//! in the `OriStr` struct — no heap allocation or RC management needed.
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
pub mod io;
pub mod iterator;
pub mod list;
pub mod map;
pub mod rc;
pub mod set;
pub(crate) mod slice_encoding;
pub mod string;

// Re-export all public items from submodules.
//
// Existing code (iterator, format, tests) uses `crate::ori_rc_alloc`,
// `crate::OriStr`, etc. These glob re-exports maintain backward compatibility
// so `crate::item_name` paths continue to work.
//
// Note: `map` and `set` are not glob-reexported because both contain a
// `pub mod cow` submodule which would create an ambiguous `cow` name.
pub use io::*;
pub use list::*;
pub use rc::*;
pub use string::*;

// Re-export pub(crate) items used by ori_run_main.
pub(crate) use rc::{check_leaks_enabled, RC_LIVE_COUNT};

// Re-export pub(crate) items used only by tests.
#[cfg(all(test, debug_assertions))]
pub(crate) use rc::{freed_set, rt_debug_check_not_freed};
#[cfg(test)]
pub(crate) use rc::{rc_trace_enabled, MAX_REFCOUNT};
#[cfg(all(test, debug_assertions))]
pub(crate) use rc::{rt_debug_validate_rc, RT_DEBUG_FORCE};

use std::ffi::{c_char, CStr};
use std::sync::atomic::Ordering;

// ── Exception handling personality ──────────────────────────────────────
//
// All EH is implemented in C (`eh_personality.c`), zero Rust panic dependency:
//   - Itanium (Linux, macOS, MinGW): ori_eh_personality + ori_raise_exception
//   - MSVC (Windows): ori_raise_exception (RaiseException) + ori_try_call (__try/__except)

#[cfg(not(target_env = "msvc"))]
extern "C" {
    /// Ori's Itanium EH ABI personality function (implemented in `eh_personality.c`).
    ///
    /// Required by any LLVM function containing `invoke`/`landingpad`.
    /// Compiled into this library via `build.rs` + `cc` crate.
    fn ori_eh_personality();
}

/// Get the address of `ori_eh_personality` for JIT symbol mapping.
///
/// The personality function is implemented in C (`src/eh_personality.c`) and
/// compiled into this library by the `build.rs` script. This function provides
/// its address so the LLVM MCJIT engine can resolve the symbol at runtime.
///
/// On MSVC, returns 0 — SEH is used instead of the Itanium personality.
#[must_use]
pub fn ori_eh_personality_addr() -> usize {
    #[cfg(not(target_env = "msvc"))]
    {
        ori_eh_personality as *const () as usize
    }
    #[cfg(target_env = "msvc")]
    {
        0
    }
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

// ── Collection growth strategy ──────────────────────────────────────────

/// Minimum collection capacity (list, set, map, string).
///
/// Value of 4 avoids excessive reallocations for small collections while
/// not wasting memory for single-element lists. Matches Rust's `Vec`
/// behavior (which uses 4 for small element types).
const MIN_COLLECTION_CAPACITY: usize = 4;

/// Compute the next capacity for a collection that needs to hold at least
/// `required` elements.
///
/// Returns `max(required, current * 2, MIN_COLLECTION_CAPACITY)`.
///
/// Uses 2x doubling (matches Rust `Vec`, Swift `Array`, Java `ArrayList`):
/// - Amortized O(1) append
/// - At most 50% wasted capacity
/// - Simple and well-understood
#[inline]
fn next_capacity(current: usize, required: usize) -> usize {
    let doubled = current.saturating_mul(2);
    doubled.max(required).max(MIN_COLLECTION_CAPACITY)
}

// ── Memory allocation ───────────────────────────────────────────────────

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

// ── Comparison utilities ────────────────────────────────────────────────

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

// ── Args & entry point ──────────────────────────────────────────────────

/// Convert C `argc`/`argv` to an Ori `[str]` list.
///
/// Skips `argv[0]` (program name) per the Ori spec: `@main(args)` receives
/// only user-supplied arguments. Returns `OriList { len, cap, data }` by value.
///
/// Each element is an `OriStr` (24 bytes). Short arguments use SSO.
/// String data is copied to owned allocations so the caller doesn't depend
/// on the lifetime of the original `argv` strings.
#[no_mangle]
#[allow(
    clippy::similar_names,
    reason = "argc/argv are standard C parameter names"
)]
pub extern "C" fn ori_args_from_argv(argc: i32, argv: *const *const c_char) -> OriList {
    // Empty list if no user args or null argv
    if argc <= 1 || argv.is_null() {
        return OriList {
            len: 0,
            cap: 0,
            data: std::ptr::null_mut(),
        };
    }

    let count = (argc - 1) as usize; // skip argv[0]
    let total = count * std::mem::size_of::<OriStr>();
    let data = ori_rc_alloc(total, std::mem::align_of::<OriStr>());
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
        let element = OriStr::from_bytes(c_str.to_bytes());
        // SAFETY: elements[i] is within the allocated array
        unsafe { elements.add(i).write(element) };
    }

    // Store elem_count in the RC header so slice-based cleanup knows the
    // element count. elem_dec_fn is deferred — the LLVM-generated str thunk
    // will be stored by the first ori_buffer_rc_dec via store_elem_dec_fn_once.
    // SAFETY: data was just returned by ori_rc_alloc — header offsets are valid.
    unsafe { rc::store_elem_count(data, count as i64) };

    OriList {
        len: count as i64,
        cap: count as i64,
        data: data.cast::<u8>(),
    }
}

/// Clean up the `[str]` buffer created by `ori_args_from_argv`.
///
/// Frees each heap string's data buffer, then frees the list buffer.
/// Called by the main wrapper after `_ori_main` returns. The strings
/// have refcount 1 (unique — created by `ori_args_from_argv`, passed
/// by reference to `_ori_main` which does not increment).
#[no_mangle]
pub extern "C" fn ori_args_cleanup(data: *mut u8, len: i64) {
    if data.is_null() || len <= 0 {
        return;
    }
    let count = len as usize;
    let elements = data.cast::<string::OriStr>();
    for i in 0..count {
        // SAFETY: elements[i] is within the allocated array (len <= capacity)
        let s = unsafe { &*elements.add(i) };
        if !s.is_sso() {
            let heap = unsafe { s.heap };
            if !heap.data.is_null() {
                rc::ori_rc_free(heap.data, heap.cap as usize, 8);
            }
        }
    }
    let alloc_size = count * std::mem::size_of::<string::OriStr>();
    rc::ori_rc_free(data, alloc_size, std::mem::align_of::<string::OriStr>());
}

// ── ori_try_call (C++ implementation, MSVC only) ────────────────────────
//
// On MSVC, ori_try_call is implemented in eh_personality_msvc.cpp using
// C++ try/catch(OriPanicException&). Thunks are `C-unwind` because the
// C++ exception from ori_raise_exception must propagate through them.
// On Itanium, catch(expr:) uses LLVM invoke/landingpad directly.

#[cfg(all(target_os = "windows", target_env = "msvc"))]
extern "C" {
    fn ori_try_call(thunk: unsafe extern "C-unwind" fn(*mut u8), ctx: *mut u8) -> i64;
}

/// Thunk adapter for `ori_run_main` → `ori_try_call`.
///
/// Casts the context pointer back to a function pointer and calls it.
/// `C-unwind` allows the C++ exception from `ori_raise_exception` to
/// propagate through this thunk into `ori_try_call`'s catch handler.
#[cfg(all(target_os = "windows", target_env = "msvc"))]
unsafe extern "C-unwind" fn run_main_thunk(ctx: *mut u8) {
    let main_fn: extern "C" fn() = unsafe { std::mem::transmute(ctx) };
    main_fn();
}

/// Wrap an AOT `@main` call to handle Ori panics.
///
/// On Itanium targets (Linux, macOS, MinGW): panics are handled by
/// LLVM-generated `invoke`/`landingpad` pairs with `ori_eh_personality`.
/// `ori_run_main` is not used on Itanium — the LLVM-generated `main()`
/// wrapper calls `@main` directly.
///
/// On Windows MSVC: delegates to the C++ `ori_try_call` which uses
/// `try`/`catch(OriPanicException&)` to catch Ori panics.
///
/// Exit codes:
/// - **0**: success
/// - **1**: panic
/// - **2**: RC leak detected (only when `ORI_CHECK_LEAKS=1`)
#[no_mangle]
pub extern "C" fn ori_run_main(main_fn: extern "C" fn()) -> i32 {
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        let succeeded = unsafe { ori_try_call(run_main_thunk, main_fn as *mut u8) };
        if succeeded == 1 {
            return check_leaks_and_exit();
        }
        // Panic was caught — message already printed by ori_panic/ori_panic_cstr
        1
    }

    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    {
        // On Itanium, ori_run_main is typically not called (LLVM main wrapper
        // invokes @main directly with landingpad-based EH). But if called
        // (e.g. test infrastructure), use catch_unwind as a safety net.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            main_fn();
        }));
        match result {
            Ok(()) => check_leaks_and_exit(),
            Err(_) => 1,
        }
    }
}

/// Check for RC leaks and return the appropriate exit code.
fn check_leaks_and_exit() -> i32 {
    if check_leaks_enabled() {
        let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
        if live != 0 {
            eprintln!("ori: {live} RC allocation(s) not freed (memory leak)");
            #[cfg(debug_assertions)]
            rc::alloc_registry_report();
            return 2;
        }
    }
    0
}

/// AOT-callable leak check — called from the LLVM-generated `main()` wrapper.
///
/// Returns 0 if no leaks (or `ORI_CHECK_LEAKS` not set), 2 if leaks detected.
/// The `main` wrapper uses this to override the exit code when leaks are found.
#[no_mangle]
pub extern "C" fn ori_check_leaks() -> i32 {
    check_leaks_and_exit()
}

#[cfg(test)]
pub(crate) mod test_helpers;

#[cfg(test)]
mod tests;

/// Forced-unwind tests for `ori_eh_personality`.
///
/// Must be in-crate (not integration test) because the C/assembly test harness
/// symbols are linked via the build script's `cc::Build::compile()` — static
/// archive symbols are only available to the library's own test binary, not to
/// separate integration test crates.
#[cfg(test)]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod forced_unwind_tests {
    extern "C" {
        fn test_forced_unwind_skips_catch() -> i32;
        fn test_forced_unwind_runs_cleanup() -> i32;
    }

    /// Single test to avoid data races on shared C globals
    /// (`catch_handler_entered`, `cleanup_handler_entered`).
    #[test]
    fn forced_unwind_personality_behavior() {
        // Catch-all pads must NOT be installed during forced unwind
        let result = unsafe { test_forced_unwind_skips_catch() };
        assert_eq!(
            result, 0,
            "catch-all handler should not run during forced unwind"
        );

        // Cleanup pads MUST still run during forced unwind
        let result = unsafe { test_forced_unwind_runs_cleanup() };
        assert_eq!(
            result, 0,
            "cleanup pads should still run during forced unwind"
        );
    }
}
