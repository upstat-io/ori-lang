//! Set operations, literal construction, buffer RC cleanup.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static SET: &[RtFn] = &[
    // Set — all extern "C"
    RtFn {
        name: "ori_set_contains",
        // (data, cap, len, needle, elem_size, elem_eq, elem_hash) -> i64
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_insert_cow",
        // (data, len, cap, elem, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::NoaliasLastParam],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_remove_cow",
        // (data, len, cap, elem, elem_size, elem_align, elem_eq, elem_hash, inc_fn, elem_dec_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::NoaliasLastParam],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_union_cow",
        // (d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::NoaliasLastParam],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_intersection_cow",
        // (d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::NoaliasLastParam],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_difference_cow",
        // (d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I32,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::NoaliasLastParam],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_to_list",
        // (data, cap, len, elem_size, elem_dec_fn, elem_inc_fn, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Set literal construction — hash table allocation + per-entry insert
    RtFn {
        name: "ori_set_literal_alloc",
        // (count, elem_size, out_cap) -> ptr
        params: &[Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_literal_put",
        // (data, cap, elem, elem_size, elem_hash)
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Set buffer RC cleanup (hash table layout — cap and len swapped vs list)
    RtFn {
        name: "ori_set_buffer_rc_dec",
        // (data, cap, len, elem_size, elem_dec_fn)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_buffer_drop_unique",
        // (data, cap, len, elem_size, elem_dec_fn)
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Structural trait codegen targets — Eq + Hashable
    RtFn {
        name: "ori_set_eq",
        // (a, b, elem_size, elem_eq, elem_hash) -> bool
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_set_hash",
        // (set, elem_size, elem_hash) -> i64
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
