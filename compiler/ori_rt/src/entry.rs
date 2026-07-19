//! Process-boundary runtime entry, argument, and exception primitives.

use std::ffi::{c_char, CStr};
use std::mem::{align_of, size_of};
use std::sync::atomic::Ordering;

use crate::{check_leaks_enabled, io, rc, OriList, OriStr, RC_LIVE_COUNT};

#[cfg(not(target_env = "msvc"))]
extern "C" {
    fn ori_eh_personality();
}

/// Return the address of Ori's exception-handling personality.
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

/// Return the current OS thread identifier as a positive integer.
#[no_mangle]
pub extern "C" fn ori_thread_id() -> i64 {
    let id_str = format!("{:?}", std::thread::current().id());
    id_str
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse::<i64>()
        .unwrap_or(1)
}

/// Convert C `argc` and `argv` into the Ori `[str]` main-argument list.
#[no_mangle]
#[expect(
    clippy::similar_names,
    reason = "argc/argv are standard C parameter names"
)]
pub extern "C" fn ori_args_from_argv(argc: i32, argv: *const *const c_char) -> OriList {
    if argc <= 1 || argv.is_null() {
        return OriList {
            len: 0,
            cap: 0,
            data: std::ptr::null_mut(),
        };
    }

    let count = (argc - 1) as usize;
    let total = count * size_of::<OriStr>();
    let data = rc::ori_rc_alloc(total, align_of::<OriStr>());
    if data.is_null() {
        return OriList {
            len: 0,
            cap: 0,
            data: std::ptr::null_mut(),
        };
    }

    let elements = data.cast::<OriStr>();
    for i in 0..count {
        // SAFETY: `argv` has `argc` entries and this loop starts after `argv[0]`.
        let c_str = unsafe { CStr::from_ptr(*argv.add(i + 1)) };
        let element = OriStr::from_bytes(c_str.to_bytes());
        // SAFETY: `elements` has capacity for exactly `count` values.
        unsafe { elements.add(i).write(element) };
    }

    // SAFETY: `data` came from `ori_rc_alloc`, so its header is live and writable.
    unsafe {
        rc::store_elem_count(data, count as i64);
        rc::store_elem_dec_fn(data, Some(rc::ori_str_elem_dec));
    }

    OriList {
        len: count as i64,
        cap: count as i64,
        data,
    }
}

/// Release the `[str]` buffer created by [`ori_args_from_argv`].
#[no_mangle]
pub extern "C" fn ori_args_cleanup(data: *mut u8, len: i64) {
    if data.is_null() || len <= 0 {
        return;
    }

    let count = len as usize;
    let elements = data.cast::<OriStr>();
    for i in 0..count {
        // SAFETY: `elements` has `count` initialized entries.
        let value = unsafe { &*elements.add(i) };
        if !value.is_sso() {
            // SAFETY: The heap union variant is active when `is_sso()` is false.
            let heap = unsafe { value.heap };
            if !heap.data.is_null() {
                rc::ori_rc_free(heap.data, heap.cap as usize, 8);
            }
        }
    }
    rc::ori_rc_free(data, count * size_of::<OriStr>(), align_of::<OriStr>());
}

#[cfg(all(target_os = "windows", target_env = "msvc"))]
extern "C" {
    fn ori_try_call(thunk: unsafe extern "C-unwind" fn(*mut u8), ctx: *mut u8) -> i64;
}

#[cfg(all(target_os = "windows", target_env = "msvc"))]
unsafe extern "C-unwind" fn run_main_thunk(ctx: *mut u8) {
    // SAFETY: `ctx` was created from the `extern "C" fn()` passed to `ori_run_main`.
    let main_fn: extern "C" fn() = unsafe { std::mem::transmute(ctx) };
    main_fn();
}

/// Invoke an AOT `@main` function through the platform panic boundary.
#[no_mangle]
pub extern "C" fn ori_run_main(main_fn: extern "C" fn()) -> i32 {
    io::reset_panic_state();

    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        // SAFETY: `ori_try_call` is the linked C++ SEH boundary.
        let succeeded = unsafe { ori_try_call(run_main_thunk, main_fn as *mut u8) };
        if succeeded == 1 {
            return check_leaks_and_exit();
        }
        io::ori_report_uncaught_panic();
        1
    }

    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    {
        // SAFETY: The JIT recovery boundary owns its stack-local jump buffer.
        if unsafe { io::jit_recovery::jit_run_protected(main_fn) }.is_ok() {
            check_leaks_and_exit()
        } else {
            io::ori_report_uncaught_panic();
            1
        }
    }
}

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

/// Return the AOT process exit code after checking live RC allocations.
#[no_mangle]
pub extern "C" fn ori_check_leaks() -> i32 {
    check_leaks_and_exit()
}
