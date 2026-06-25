//! Map literal construction + hash-table operations.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static MAP: &[RtFn] = &[
    // Map literal construction — hash table allocation + per-entry insert
    RtFn {
        name: "ori_map_literal_alloc",
        // (count, key_size, val_size, out_cap) -> ptr
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_literal_put",
        // (data, cap, key, val, key_size, val_size, key_hash)
        params: &[
            Ty::Ptr,
            Ty::I64,
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
    // Map — hash table layout: data = [metadata|keys|values]
    RtFn {
        name: "ori_map_contains_key",
        // (data, cap, len, needle, key_size, key_eq, key_hash) -> i64
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
        name: "ori_map_keys_to_list",
        // (data, cap, len, key_size, key_dec_fn, key_inc_fn, out_ptr)
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
    RtFn {
        name: "ori_map_values_to_list",
        // (data, cap, len, key_size, val_size, val_dec_fn, val_inc_fn, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
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
    RtFn {
        name: "ori_map_get",
        // (data, cap, len, needle, key_size, val_size, key_eq, key_hash, out_ptr)
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
        ],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_insert_cow",
        // (data, len, cap, key, value, key_size, val_size, key_eq, key_hash, key_inc, val_inc, val_dec, cow_mode, out_ptr)
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
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
        name: "ori_map_updated_cow",
        // (data, len, cap, key, value, key_size, val_size, key_eq, key_hash, key_inc, val_inc, val_dec, cow_mode, out_ptr)
        // Same shape as ori_map_insert_cow; value is MOVED (caller ref released).
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
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
        name: "ori_map_merge_cow",
        // (a_data, a_len, a_cap, b_data, b_len, b_cap, key_size, val_size, key_eq, key_hash, key_inc, val_inc, key_dec, val_dec, cow_mode, out_ptr)
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
        name: "ori_map_remove_cow",
        // (data, len, cap, key, key_size, val_size, key_eq, key_hash, key_inc, val_inc, key_dec, val_dec, cow_mode, out_ptr)
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
        name: "ori_map_buffer_rc_dec",
        // (data, cap, len, key_size, val_size, key_dec_fn, val_dec_fn)
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
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Structural trait codegen targets — Eq + Hashable
    // ori_map_eq(a: ptr, b: ptr, key_size, val_size, key_eq, key_hash, val_eq) -> bool
    RtFn {
        name: "ori_map_eq",
        params: &[
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // ori_map_hash(m: ptr, key_size, val_size, key_hash, val_hash) -> i64
    RtFn {
        name: "ori_map_hash",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
