//! Map literal construction + hash-table operations.

use super::super::types::{Attr, RtFn, Ty};

/// Runtime declarations for map construction, access, mutation, and traits.
pub(in crate::codegen::runtime_decl) static MAP: &[RtFn] = &[
    // Map literal construction.
    RtFn {
        name: "ori_map_literal_alloc",
        // INVARIANT: Runtime parameter order is count, key_size, val_size, out_cap.
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_literal_put",
        // INVARIANT: The ABI orders storage, key/value data, then comparison and drop hooks.
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // INVARIANT: Map data lays out metadata before keys and values.
    RtFn {
        name: "ori_map_contains_key",
        // INVARIANT: Runtime parameter order ends with key_size, key_eq, and key_hash.
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
        // INVARIANT: Key drop and retain hooks precede the output pointer.
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
        // INVARIANT: Value drop and retain hooks precede the output pointer.
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
        // INVARIANT: Lookup layout and comparison arguments precede the output pointer.
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
        // INVARIANT: The ABI starts with data, len, cap, key, and value.
        // INVARIANT: Layout and equality/hash arguments precede ownership hooks.
        // INVARIANT: Ownership hooks end with cow_mode and out_ptr.
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
        // INVARIANT: The ABI starts with data, len, cap, key, and value.
        // INVARIANT: Layout and equality/hash arguments precede ownership hooks.
        // INVARIANT: The value moves into the call; cow_mode and out_ptr finish the ABI.
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
        // INVARIANT: Both map triples precede layout and equality/hash arguments.
        // INVARIANT: Ownership hooks follow the comparison hooks.
        // INVARIANT: cow_mode and out_ptr finish the ABI.
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
        // INVARIANT: Storage and key arguments precede layout and comparison hooks.
        // INVARIANT: Ownership hooks follow the comparison hooks.
        // INVARIANT: cow_mode and out_ptr finish the ABI.
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
        // INVARIANT: Buffer layout arguments precede the key and value drop hooks.
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
    // Structural equality and hashing.
    RtFn {
        name: "ori_map_eq",
        // INVARIANT: Both maps precede layout, hash, and equality hooks.
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
    RtFn {
        name: "ori_map_hash",
        // INVARIANT: Map layout precedes the key and value hash hooks.
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
