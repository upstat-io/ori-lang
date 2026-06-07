//! Reference counting, drop functions, buffer RC, map drop-loop accessors, buffer reuse.

use super::super::types::{Attr, RtFn, Ty};

pub(in crate::codegen::runtime_decl) static RC: &[RtFn] = &[
    // Reference counting (ARC-safe attributes are CRITICAL — see section 11.3)
    RtFn {
        name: "ori_rc_alloc",
        params: &[Ty::I64, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind, Attr::NoaliasReturn],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_inc",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_dec",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // May-unwind RC dec: drop fn called directly (no catch_unwind), so a
    // foreign Ori exception from a user @drop unwinds through to the caller's
    // cleanup pad. extern "C-unwind" → NO Nounwind (per RT-1 + runtime.md
    // §Unwinding ABI). Routed via `invoke` at may-unwind RcDec sites only.
    RtFn {
        name: "ori_rc_dec_unwind",
        params: &[Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Drop-cleanup-depth enter/exit: bracket the drop work inside a codegen
    // cleanup landing pad so a nested panic (depth > 0) aborts via
    // ori_drop_double_panic_abort. Mutate a thread-local counter only —
    // nounwind, no argument memory.
    RtFn {
        name: "ori_drop_cleanup_enter",
        params: &[],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_drop_cleanup_exit",
        params: &[],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Slice-aware string RC inc: handles SSO, heap, and seamless slices.
    RtFn {
        name: "ori_str_rc_inc",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Drop function for plain string buffers (no nested elements).
    // Reads data_size from the RC header and calls ori_rc_free.
    // Used as drop_fn argument to ori_str_rc_dec in derive format codegen.
    RtFn {
        name: "ori_str_drop_buffer",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Slice-aware string RC dec: handles SSO, heap, and seamless slices.
    RtFn {
        name: "ori_str_rc_dec",
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_free",
        params: &[Ty::Ptr, Ty::I64, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Atomic RC dec-and-test (no drop): 1 if RC hit zero, else 0. Codegen
    // branches on this on the may-unwind collection rc_dec path so its own
    // cleanup loop (with an unwind landing pad) runs instead of the runtime
    // catch_unwind cleanup.
    RtFn {
        name: "ori_rc_dec_to_zero",
        params: &[Ty::Ptr],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Slice-aware RC inc for list/set data buffers.
    // If cap has slice flag, finds original buffer and inc's that.
    RtFn {
        name: "ori_list_rc_inc",
        params: &[Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Buffer-aware RC dec for collection data buffers.
    // (data, len, cap, elem_size, elem_dec_fn)
    RtFn {
        name: "ori_buffer_rc_dec",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Unique-drop for list/set buffers: skip atomic RC dec, directly
    // clean elements + free. Same signature as ori_buffer_rc_dec.
    RtFn {
        name: "ori_buffer_drop_unique",
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind, Attr::MemArgmemRW],
        jit_allowed: true,
    },
    // Unique-drop for map buffers: skip atomic RC dec, directly
    // clean keys + values + free.
    // (data, cap, len, key_size, val_size, key_dec_fn, val_dec_fn)
    RtFn {
        name: "ori_map_buffer_drop_unique",
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
    // Codegen map two-channel drop-loop layout accessors (HashTableLayout SSOT).
    // (cap, key_size, val_size) -> byte offset / total size.
    RtFn {
        name: "ori_map_keys_offset",
        params: &[Ty::I64, Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_vals_offset",
        params: &[Ty::I64, Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_map_total_size",
        params: &[Ty::I64, Ty::I64, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // (data, bucket) -> 1 if OCCUPIED else 0.
    RtFn {
        name: "ori_map_bucket_occupied",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Collection buffer reuse: reset a list/set buffer for new elements.
    // Checks uniqueness internally — if unique, reuses buffer; if shared,
    // decs old and allocates fresh.
    // (old_data, old_len, old_cap, new_len, elem_size, elem_dec_fn, out_cap) -> ptr
    RtFn {
        name: "ori_list_reset_buffer",
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_live_count",
        params: &[],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_rc_reset_live_count",
        params: &[],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
