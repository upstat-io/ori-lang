//! Formattable-trait runtime support.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static FORMAT: &[RtFn] = &[
    // Format (section 3.16 Formattable trait) — all extern "C"
    RtFn {
        name: "ori_format_int",
        params: &[Ty::I64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_float",
        params: &[Ty::F64, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_str",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_bool",
        params: &[Ty::Bool, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_format_char",
        params: &[Ty::I32, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
