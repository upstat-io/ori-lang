//! Iterator constructors, next, adapters, consumers, cleanup.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static ITERATOR: &[RtFn] = &[
    // Iterator constructors — all extern "C"
    RtFn {
        name: "ori_iter_from_list",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64],
        //        data   len   cap   elem_size
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_range",
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Bool],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_str",
        params: &[Ty::Ptr],
        //        *const OriStr (SSO-safe)
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_map",
        // (data, cap, len, key_size, val_size, owns_data, key_dec_fn, val_dec_fn)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Bool,
            Ty::Ptr,
            Ty::Ptr,
        ],
        //        data   cap    len   ks     vs     owns_data  k_dec  v_dec
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_option",
        // (is_some, payload_ptr, elem_size, elem_dec_fn)
        params: &[Ty::Bool, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator next — extern "C" (callbacks called inside, panics abort at boundary)
    RtFn {
        name: "ori_iter_next",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator adapters — extern "C" (store callback, don't call it)
    RtFn {
        name: "ori_iter_map",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_filter",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_take",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_skip",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_enumerate",
        params: &[Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_zip",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_chain",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_flatten",
        // (iter, inner_elem_size)
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_cycle",
        // (iter, elem_size)
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_rev",
        // (iter, elem_size)
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator consumers — extern "C" (call callbacks internally, panics abort at boundary)
    RtFn {
        name: "ori_iter_collect",
        // (iter, elem_size, elem_inc_fn, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_collect_set",
        // (iter, elem_size, elem_eq, elem_hash, elem_inc_fn, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_count",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_any",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_all",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_find",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_for_each",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_fold",
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_last",
        // (iter, elem_size, out_ptr)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_rfind",
        // (iter, pred_fn, pred_env, elem_size, out_ptr)
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_rfold",
        // (iter, init_ptr, fold_fn, fold_env, elem_size, acc_size, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_join",
        // (iter, sep_field0, sep_field1, sep_field2, to_str_fn, to_str_env, elem_size, out_ptr)
        // sep_field0-2: raw fields of OriStr {i64, i64, ptr} — SSO-safe reconstruction in runtime
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator cleanup — extern "C"
    RtFn {
        name: "ori_iter_drop",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
