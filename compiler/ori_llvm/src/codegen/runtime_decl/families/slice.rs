//! Slice operations.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static SLICE: &[RtFn] = &[
    // Slice operations
    RtFn {
        name: "ori_list_slice",
        // data, len, cap, start, end, elem_size, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_slice_take",
        // data, len, cap, n, elem_size, out_ptr
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_slice_drop",
        // data, len, cap, n, elem_size, out_ptr
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_materialize_slice",
        // data, len, cap, elem_size, elem_align, inc_fn, out_ptr
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_substring",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
