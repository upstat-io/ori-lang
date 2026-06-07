//! COW primitives, empty-collection sentinels, args conversion/cleanup.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static COW_ARGS: &[RtFn] = &[
    // COW primitives
    RtFn {
        name: "ori_rc_is_unique",
        params: &[Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_is_unique_or_null",
        params: &[Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_realloc",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_memcpy_elements",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_memmove_elements",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_list_ensure_capacity",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Empty collection sentinels
    RtFn {
        name: "ori_list_empty",
        params: &[],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_str_empty",
        params: &[],
        ret: Some(Ty::Str),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_empty",
        params: &[],
        ret: Some(Ty::Map),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_empty",
        params: &[],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Args conversion — extern "C"
    RtFn {
        name: "ori_args_from_argv",
        params: &[Ty::I32, Ty::Ptr],
        ret: Some(Ty::List),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Args cleanup — free [str] buffer after @main returns
    RtFn {
        name: "ori_args_cleanup",
        params: &[Ty::Ptr, Ty::I64],
        //        data   len
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: false,
    },
];
