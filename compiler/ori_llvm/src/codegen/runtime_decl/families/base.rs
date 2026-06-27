//! I/O, panic, entry-point wrapper, leak check, element-header helpers, assertions.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static BASE: &[RtFn] = &[
    // I/O — extern "C" (panics abort at ABI boundary, never unwind)
    RtFn {
        name: "ori_print",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_print_int",
        params: &[Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_print_float",
        params: &[Ty::F64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_print_bool",
        params: &[Ty::Bool],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Prelude builtin `thread_id() -> int`: reads a thread-local, never
    // unwinds (extern "C" -> Nounwind per RT-1); JIT-callable.
    RtFn {
        name: "ori_thread_id",
        params: &[],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Panic (cold + noreturn, NOT nounwind — must unwind for RC cleanup)
    RtFn {
        name: "ori_panic",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Cold, Attr::Noreturn],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_panic_cstr",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Cold, Attr::Noreturn],
        jit_allowed: true,
    },
    // Entry point wrapper (AOT-only — JIT runs functions directly).
    // Nounwind: catches all panics internally via catch_unwind (Itanium) or
    // __try/__except SEH (MSVC). Never unwinds to its caller.
    RtFn {
        name: "ori_run_main",
        params: &[Ty::Ptr],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // Leak check for AOT main wrapper. Reads ORI_CHECK_LEAKS env var,
    // returns 0 if clean or disabled, 2 if leaks detected.
    RtFn {
        name: "ori_check_leaks",
        params: &[],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // Element header helpers — store elem_dec_fn and elem_count in RC header
    // at collection construction time.
    RtFn {
        name: "ori_buffer_store_elem_dec",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_buffer_store_elem_count",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Assertions — extern "C-unwind" (call ori_panic on failure, must unwind)
    RtFn {
        name: "ori_assert",
        params: &[Ty::Bool],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_int",
        params: &[Ty::I64, Ty::I64],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_bool",
        params: &[Ty::Bool, Ty::Bool],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_float",
        params: &[Ty::F64, Ty::F64],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_assert_eq_str",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
];
