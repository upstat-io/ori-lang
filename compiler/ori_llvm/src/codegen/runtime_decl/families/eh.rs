//! Panic-handler registration, catch recovery, SEH trampoline, EH personalities.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static EH: &[RtFn] = &[
    // Panic handler registration — extern "C" (stores function pointer, no panic)
    RtFn {
        name: "ori_register_panic_handler",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Catch recovery — extern "C" (called after unwinding completes, no panic)
    RtFn {
        name: "ori_catch_cleanup",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_catch_recover",
        params: &[],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // SEH/MSVC catch trampoline — wraps a function call in __try/__except.
    // extern "C" (catches exceptions internally, returns status code)
    RtFn {
        name: "ori_try_call",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // EH personality (Itanium ABI — required by invoke/landingpad)
    // Implemented in ori_rt/src/eh_personality.c
    RtFn {
        name: "ori_eh_personality",
        params: &[Ty::I32],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // EH personality (Windows SEH — required by invoke/catchswitch/catchpad)
    // AOT-only: JIT uses Itanium EH with ori_eh_personality.
    RtFn {
        name: "__CxxFrameHandler3",
        params: &[],
        ret: Some(Ty::I32),
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
    // Traceable trace injection and formatting
    // `function` and `file` are `*const OriStr` (by pointer): `OriStr` is 24 bytes,
    // and the SysV AMD64 C ABI classifies a struct larger than 16 bytes as MEMORY,
    // passed indirectly. Declaring the params as a by-value `{i64,i64,ptr}` would
    // shift the register-assigned eightbytes and corrupt the trailing scalar args
    // plus the caller frame. Mirrors `_ori_error_with_trace`, which passes its
    // struct operands by pointer for the same reason.
    RtFn {
        name: "_ori_inject_trace_entry",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "_ori_format_error_trace",
        params: &[Ty::Ptr],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "_ori_error_with_trace",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
