//! Iterator constructors, next, adapters, consumers, cleanup.

use super::super::types::{Attr, RtFn, Ty};

/// Runtime declarations for iterator construction, adaptation, consumption, and cleanup.
pub(in crate::codegen::runtime_decl) static ITERATOR: &[RtFn] = &[
    // Iterator constructors use the C ABI.
    RtFn {
        name: "ori_iter_from_list",
        // INVARIANT: Runtime parameter order is data, len, cap, elem_size, owns_data.
        params: &[Ty::Ptr, Ty::I64, Ty::I64, Ty::I64, Ty::Bool],
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
        name: "ori_range_len",
        // INVARIANT: Runtime parameter order is start, end, step, inclusive.
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Bool],
        ret: Some(Ty::I64),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_range_contains",
        // INVARIANT: Runtime parameter order ends with the tested value.
        params: &[Ty::I64, Ty::I64, Ty::I64, Ty::Bool, Ty::I64],
        ret: Some(Ty::Bool),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_str",
        // INVARIANT: The pointer addresses an SSO-safe `OriStr` value.
        params: &[Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_map",
        // INVARIANT: Map storage and layout precede ownership and drop hooks.
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
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_from_option",
        // INVARIANT: Presence and payload precede element layout and its drop hook.
        params: &[Ty::Bool, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_repeat",
        // INVARIANT: Element layout and its drop hook follow the repeated value.
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    // Iterator advancement may unwind through adapter callbacks.
    RtFn {
        name: "ori_iter_next",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_next_back",
        params: &[Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
        jit_allowed: true,
    },
    // Iterator adapters store callbacks without invoking them.
    RtFn {
        name: "ori_iter_map",
        // INVARIANT: Transform code and environment precede input layout and output cleanup.
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
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
        // INVARIANT: The inner element size follows the source iterator.
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_cycle",
        // INVARIANT: Element layout precedes its retain and drop hooks.
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_rev",
        // INVARIANT: Element layout precedes its retain and drop hooks.
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr],
        ret: Some(Ty::Ptr),
        attrs: &[],
        jit_allowed: true,
    },
    // Iterator consumers may unwind through user callbacks.
    RtFn {
        name: "ori_iter_collect",
        // INVARIANT: Element layout and ownership hooks precede the output pointer.
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr, Ty::Ptr, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_collect_set",
        // INVARIANT: Equality/hash and ownership hooks precede the output pointer.
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_count",
        params: &[Ty::Ptr, Ty::I64],
        ret: Some(Ty::I64),
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_any",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_all",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: Some(Ty::I8),
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_find",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_for_each",
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64],
        ret: None,
        attrs: &[],
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
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_last",
        // INVARIANT: Element layout precedes the output pointer.
        params: &[Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_rfind",
        // INVARIANT: Predicate code and environment precede layout and output.
        params: &[Ty::Ptr, Ty::Ptr, Ty::Ptr, Ty::I64, Ty::Ptr],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_rfold",
        // INVARIANT: Fold state and callback precede element/accumulator layout and output.
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
        attrs: &[],
        jit_allowed: true,
    },
    RtFn {
        name: "ori_iter_join",
        // INVARIANT: The separator crosses as three raw, SSO-safe `OriStr` fields.
        // INVARIANT: Conversion code and environment precede element layout and output.
        // INVARIANT: The drop hook is non-null only for proven adapter-produced elements.
        params: &[
            Ty::Ptr,
            Ty::I64,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
            Ty::Ptr,
            Ty::I64,
            Ty::Ptr,
            Ty::Ptr,
        ],
        ret: None,
        attrs: &[],
        jit_allowed: true,
    },
    // Iterator cleanup uses the C ABI.
    RtFn {
        name: "ori_iter_drop",
        params: &[Ty::Ptr],
        ret: None,
        attrs: &[Attr::Nounwind],
        jit_allowed: true,
    },
];
