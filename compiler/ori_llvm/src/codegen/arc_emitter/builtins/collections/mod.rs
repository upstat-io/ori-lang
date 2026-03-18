//! Collection type builtin methods.
//!
//! Handles `length`/`len`, `is_empty`, `concat`, `iter` for List, Str, Map, Set, Range.
//!
//! This file is exempt from the 500-line limit (currently ~528 lines): it is a
//! pure `declare_builtins!` macro invocation generating a single `match`
//! dispatch — splitting would fragment the lookup surface. The macro cannot be
//! split across files. Method implementations live in sub-modules.

mod hash_thunks;
mod list_builtins;
mod list_cow;
mod map_builtins;
mod set_builtins;
mod string_builtins;

declare_builtins! { emitter, ctx;
    // str
    ("str", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("str", "length") => emitter.emit_str_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "len") => emitter.emit_str_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "is_empty") => emitter.emit_str_is_empty(ctx.arg_vals[0]),
    ("str", "concat") => {
        if ctx.arg_vals.len() >= 2 {
            Some(emitter.emit_str_runtime_call("ori_str_concat", ctx.arg_vals[0], ctx.arg_vals[1], true))
        } else {
            None
        }
    },
    ("str", "to_str") => Some(ctx.arg_vals[0]),
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
            emitter.emit_str_split(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "iter") => emitter.emit_str_iter(ctx.arg_vals[0]),
    // list
    ("list", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("list", "count") => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "length") => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "len") => emitter.emit_list_length(ctx.arg_vals[0]),
    ("list", "is_empty") => emitter.emit_list_is_empty(ctx.arg_vals[0]),
    ("list", "concat") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_concat_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
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
                let r = emitter.emit_list_concat_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
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
                let r = emitter.emit_list_push_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
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
            emitter.emit_list_first(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("list", "last") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_last(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("list", "pop") => {
        if let TypeInfo::List { element } = ctx.type_info {
            // pop() returns Option<T> in the type checker — COW list mutation
            // for pop requires ARC pipeline cooperation (dual return: element + modified list).
            // For now, return the last element as Option<T>.
            emitter.emit_list_last(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("list", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_contains(ctx.arg_vals[0], ctx.arg_vals[1], *element)
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
            let r = emitter.emit_list_reverse_cow(ctx.arg_vals[0], *element, cm);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "sort") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_sort_cow(ctx.arg_vals[0], *element, cm);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "sort_stable") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_sort_stable_cow(ctx.arg_vals[0], *element, cm);
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
                let r = emitter.emit_list_set_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm);
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
                let r = emitter.emit_list_insert_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm);
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
                let r = emitter.emit_list_remove_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
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
                emitter.emit_list_slice(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element)
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
                emitter.emit_list_take_slice(ctx.arg_vals[0], ctx.arg_vals[1], *element)
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
                emitter.emit_list_drop_slice(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "iter") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_iter(ctx.arg_vals[0], ctx.receiver_ty, *element)
        } else {
            None
        }
    },
    // map
    ("map", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("map", "length") => emitter.emit_map_length(ctx.arg_vals[0]),
    ("map", "len") => emitter.emit_map_length(ctx.arg_vals[0]),
    ("map", "is_empty") => emitter.emit_map_is_empty(ctx.arg_vals[0]),
    ("map", "contains_key") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, .. } = ctx.type_info {
                emitter.emit_map_contains_key(ctx.arg_vals[0], ctx.arg_vals[1], *key)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "keys") => {
        if let TypeInfo::Map { key, .. } = ctx.type_info {
            emitter.emit_map_keys(ctx.arg_vals[0], *key)
        } else {
            None
        }
    },
    ("map", "values") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_values(ctx.arg_vals[0], *key, *value)
        } else {
            None
        }
    },
    ("map", "get") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                emitter.emit_map_get(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value)
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
                let r = emitter.emit_map_insert(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *key, *value, cm);
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
            emitter.emit_map_iter(ctx.arg_vals[0], *key, *value)
        } else {
            None
        }
    },
    ("map", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_remove(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, cm);
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
    ("Set", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("Set", "length") => emitter.emit_set_length(ctx.arg_vals[0]),
    ("Set", "len") => emitter.emit_set_length(ctx.arg_vals[0]),
    ("Set", "is_empty") => emitter.emit_set_is_empty(ctx.arg_vals[0]),
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
            emitter.emit_list_iter(ctx.arg_vals[0], ctx.receiver_ty, *element)
        } else {
            None
        }
    },
    // range
    ("range", "iter") => emitter.emit_range_iter(ctx.arg_vals[0]),
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

    /// Build the LLVM struct type `{i64, i64, ptr}` for list sret returns.
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
