//! Collection type builtin methods.
//!
//! Handles `length`/`len`, `is_empty`, `concat`, `iter` for List, Str, Map, Set, Range.
//!
//! This file is exempt from the 500-line limit: it is a pure `declare_builtins!`
//! macro invocation generating a single `match` dispatch — splitting would
//! fragment the lookup surface. The macro cannot be split across files. Method
//! implementations live in sub-modules.

mod hash_thunks;
mod list_builtins;
mod list_cow;
mod map_builtins;
mod set_builtins;
mod string_builtins;

declare_builtins! { emitter, ctx;
    // str
    ("str", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    // `str.into() : Error` constructs the user-facing Error struct.
    ("str", "into") => emitter.emit_str_into_error(ctx.arg_vals[0], ctx.receiver_ty, ctx.dst_ty),
    ("str", "length") => emitter.emit_str_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "len") => emitter.emit_str_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "is_empty") => emitter.emit_str_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "concat") => {
        if ctx.arg_vals.len() >= 2 {
            Some(emitter.emit_str_runtime_call("ori_str_concat", ctx.arg_vals[0], ctx.arg_vals[1], true))
        } else {
            None
        }
    },
    ("str", "to_str") => Some(ctx.arg_vals[0]),
    ("str", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    ("str", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_contains", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "starts_with") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_starts_with", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "ends_with") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_ends_with", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "trim") => emitter.emit_str_unary_call("ori_str_trim", ctx.arg_vals[0]),
    ("str", "substring") => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_substring(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "slice") => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_substring(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "to_uppercase") => emitter.emit_str_unary_call("ori_str_to_uppercase", ctx.arg_vals[0]),
    ("str", "to_lowercase") => emitter.emit_str_unary_call("ori_str_to_lowercase", ctx.arg_vals[0]),
    ("str", "escape") => emitter.emit_str_unary_call("ori_str_escape_control", ctx.arg_vals[0]),
    ("str", "replace") => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_replace(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "repeat") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_repeat(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "chars") => emitter.emit_str_chars(ctx.arg_vals[0]),
    ("str", "split") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_split(ctx.arg_vals[0], ctx.arg_vals[1], ori_types::Idx::STR)
        } else {
            None
        }
    },
    ("str", "iter") => emitter.emit_str_iter(ctx.arg_vals[0]),
    // list
    ("list", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("list", "count") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.len"),
    ("list", "length") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.len"),
    ("list", "len") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.len"),
    ("list", "is_empty") => emitter.emit_collection_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.is_empty"),
    ("list", "concat") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_concat_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "add") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_concat_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "push") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_push_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "first") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_first(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "last") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_last(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "pop") => {
        if let TypeInfo::List { element } = ctx.type_info {
            // Why: pop() is typed Option<T> but the removing dual-return (element +
            // modified list) needs ARC-pipeline cooperation that is not wired, so the
            // emitted form is the non-mutating last-element read.
            emitter.emit_list_last(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_contains(ctx.arg_vals[0], ctx.arg_vals[1], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "reverse") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_reverse_cow(ctx.arg_vals[0], *element, cm, ctx.receiver_ty);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "sort") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_sort_cow(ctx.arg_vals[0], *element, cm, ctx.receiver_ty);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "sort_stable") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_sort_stable_cow(ctx.arg_vals[0], *element, cm, ctx.receiver_ty);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "set") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_set_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "updated") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_updated_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "insert") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_insert_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_remove_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "slice") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_slice(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "take") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_take_slice(ctx.arg_vals[0], ctx.arg_vals[1], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "drop") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_drop_slice(ctx.arg_vals[0], ctx.arg_vals[1], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "debug") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_debug(ctx.arg_vals[0], *element, true)
        } else {
            None
        }
    },
    ("list", "to_str") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_debug(ctx.arg_vals[0], *element, false)
        } else {
            None
        }
    },
    // Fixed-capacity conversions are value-identity at runtime: `[T]` and
    // `[T, max N]` share the `{ len, cap, data }` fat-pointer layout; the
    // capacity constraint is type-level only. Forward the receiver unchanged.
    // Spec: Clause 8.2.2.
    ("list", "to_dynamic") => Some(ctx.arg_vals[0]),
    ("list", "to_fixed") => Some(ctx.arg_vals[0]),
    ("list", "iter") => {
        if let TypeInfo::List { element } = ctx.type_info {
            // owns_data = the .iter() receiver is owned (ARC inc'd it → the iterator
            // holds its own ref → Drop decs). A borrowed-rooted receiver (the flatten
            // inner sub.iter() on a trampoline-closure param) → owns_data = false so the
            // outer container's elem_dec_fn frees the buffer exactly once.
            let owns_data = !emitter.is_var_borrowed_rooted(ctx.arc_args[0]);
            emitter.emit_list_iter(ctx.arg_vals[0], ctx.receiver_ty, *element, owns_data)
        } else {
            None
        }
    },
    // map
    ("map", "debug") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_debug(ctx.arg_vals[0], ctx.receiver_ty, *key, *value, true)
        } else {
            None
        }
    },
    ("map", "to_str") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_debug(ctx.arg_vals[0], ctx.receiver_ty, *key, *value, false)
        } else {
            None
        }
    },
    ("map", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("map", "length") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "map.len"),
    ("map", "len") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "map.len"),
    ("map", "is_empty") => emitter.emit_collection_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "map.is_empty"),
    ("map", "contains_key") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, .. } = ctx.type_info {
                emitter.emit_map_contains_key(ctx.arg_vals[0], ctx.arg_vals[1], *key, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    // `map.contains(key)` is the `Contains`-trait spelling of `contains_key`.
    ("map", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, .. } = ctx.type_info {
                emitter.emit_map_contains_key(ctx.arg_vals[0], ctx.arg_vals[1], *key, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "keys") => {
        if let TypeInfo::Map { key, .. } = ctx.type_info {
            emitter.emit_map_keys(ctx.arg_vals[0], *key, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("map", "values") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_values(ctx.arg_vals[0], *key, *value, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("map", "get") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                emitter.emit_map_get(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, Some(ctx.receiver_ty))
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "insert") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_insert(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "merge") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_merge(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "updated") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_updated(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "iter") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            // owns_data: same gate as the list path — a borrowed-rooted receiver
            // (the flatten inner `m.iter()` on a trampoline-closure param) →
            // owns_data = false so the outer container's elem_dec_fn frees the
            // map buffer exactly once (no double-free).
            let owns_data = !emitter.is_var_borrowed_rooted(ctx.arc_args[0]);
            emitter.emit_map_iter(ctx.arg_vals[0], *key, *value, ctx.receiver_ty, owns_data)
        } else {
            None
        }
    },
    ("map", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_remove(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    // Set
    ("Set", "debug") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_debug(ctx.arg_vals[0], *element, true)
        } else {
            None
        }
    },
    ("Set", "to_str") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_debug(ctx.arg_vals[0], *element, false)
        } else {
            None
        }
    },
    ("Set", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("Set", "length") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "set.len"),
    ("Set", "len") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "set.len"),
    ("Set", "is_empty") => emitter.emit_collection_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "set.is_empty"),
    ("Set", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                emitter.emit_set_contains(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "insert") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_insert(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_remove(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "union") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_union(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "intersection") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_intersection(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "difference") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_difference(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "to_list") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_to_list(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("Set", "into") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_to_list(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("Set", "iter") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_iter(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    // range
    ("range", "iter") => emitter.emit_range_iter(ctx.arg_vals[0]),
    ("range", "len") => emitter.emit_range_len(ctx.arg_vals[0]),
    ("range", "length") => emitter.emit_range_len(ctx.arg_vals[0]),
    ("range", "count") => emitter.emit_range_len(ctx.arg_vals[0]),
    ("range", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_range_contains(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
}

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;

// Shared helpers used by multiple collection type modules

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Alloca+store a string value and return the pointer.
    ///
    /// Runtime string methods take `*const OriStr`, but LLVM values are
    /// `{ i64, i64, ptr }` aggregates. This helper allocates stack space, stores
    /// the aggregate, and returns the pointer for the runtime call.
    pub(crate) fn str_to_ptr(&mut self, val: ValueId, name: &str) -> ValueId {
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, str_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Like [`str_to_ptr`] but with borrowed parameter forwarding.
    ///
    /// If the variable has a known source pointer (from a `Reference`/`Indirect`
    /// parameter), returns it directly instead of creating an alloca+store.
    pub(crate) fn str_to_ptr_forwarded(
        &mut self,
        val: ValueId,
        var: ori_arc::ir::ArcVarId,
        name: &str,
    ) -> ValueId {
        if let Some(&src_ptr) = self.borrowed_param_ptrs.get(&var) {
            return src_ptr;
        }
        self.str_to_ptr(val, name)
    }

    /// Alloca+store an element value and return the pointer.
    ///
    /// List runtime methods take `*const u8` for elements. This helper
    /// allocates stack space for the element, stores the value, and
    /// returns the pointer.
    pub(crate) fn elem_to_ptr(&mut self, val: ValueId, elem_ty: Idx, name: &str) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, llvm_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Emit a collection `len`/`length` field-read with borrowed-parameter
    /// forwarding, mirroring [`Self::emit_str_length_forwarded`]. When the
    /// receiver is a borrowed pointer-only param its LLVM value is a zero
    /// `{i64, i64, ptr}` placeholder (the entry-block struct-value load was
    /// elided — 24-byte collections pass indirectly per the ABI), so read
    /// `FIELD_LEN` directly from the source pointer via GEP + load. This keeps
    /// the param pointer-only (no struct-value materialization, no RC-flow
    /// change). Otherwise the receiver is a loaded struct value and
    /// `extract_value` reads it. Shared by list/map/set — identical
    /// `{i64, i64, ptr}` fat-pointer layout.
    pub(crate) fn emit_collection_length_forwarded(
        &mut self,
        receiver: ValueId,
        var: ori_arc::ir::ArcVarId,
        name: &str,
    ) -> Option<ValueId> {
        if let Some(&src_ptr) = self.borrowed_param_ptrs.get(&var) {
            let struct_ty = self.list_struct_type();
            let len_ptr = self
                .builder
                .struct_gep(struct_ty, src_ptr, ori_ir::FIELD_LEN, name);
            let i64_ty = self
                .builder
                .register_type(self.builder.scx().type_i64().into());
            return Some(self.builder.load(i64_ty, len_ptr, name));
        }
        self.builder
            .extract_value(receiver, ori_ir::FIELD_LEN, name)
    }

    /// Emit a collection `is_empty` (`len == 0`) with the same borrowed-parameter
    /// forwarding as [`Self::emit_collection_length_forwarded`]: read `FIELD_LEN`
    /// via the source pointer when the receiver is a borrowed pointer-only param
    /// (its struct value is a zero placeholder), else `extract_value`. Shared by
    /// list/map/set — identical `{i64, i64, ptr}` fat-pointer layout.
    pub(crate) fn emit_collection_is_empty_forwarded(
        &mut self,
        receiver: ValueId,
        var: ori_arc::ir::ArcVarId,
        name: &str,
    ) -> Option<ValueId> {
        let len = self.emit_collection_length_forwarded(receiver, var, name)?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, name))
    }

    /// Build the LLVM struct type `{i64, i64, ptr}` — the shared list/map/set
    /// fat-pointer layout used for list sret returns AND the borrowed-parameter
    /// `FIELD_LEN` forwarding in [`Self::emit_collection_length_forwarded`].
    pub(crate) fn list_struct_type(&mut self) -> LLVMTypeId {
        self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        )
    }
}
