//! Builtin method codegen for the ARC emitter.
//!
//! Intercepts method calls between the `method_functions` lookup and the
//! runtime fallback to generate inline LLVM IR for builtin methods like
//! `clone`, `length`, `iter`, `is_some`, etc.
//!
//! Dispatch uses `(TypeInfo, method_name)` pairs to route to specialized
//! handlers in submodules.

mod collections;
mod compound_traits;
mod iterator;
mod option_result;
mod primitives;
mod traits;
mod trampolines;

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;

use super::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Try to emit inline IR for a builtin method call.
    ///
    /// Returns `Some(result_value)` if the method was handled, `None` if
    /// the caller should fall through to the runtime function lookup.
    pub(super) fn try_emit_builtin_method(
        &mut self,
        callee: Name,
        args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.is_empty() {
            return None;
        }

        let method_name = self.interner.lookup(callee);
        let receiver_ty = arc_func.var_type(args[0]);
        let type_info = self.type_info.get(receiver_ty);
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        match &type_info {
            // Primitives: clone, to_int, byte, f, trait methods (equals, compare, hash, etc.)
            TypeInfo::Int
            | TypeInfo::Float
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Duration
            | TypeInfo::Size => self
                .emit_primitive_method(method_name, &arg_vals, &type_info)
                .or_else(|| self.emit_trait_method(method_name, &arg_vals, &type_info)),

            // Ordering: predicates (is_less, is_equal, is_greater, ...) + reverse + compare
            TypeInfo::Ordering => self
                .emit_primitive_method(method_name, &arg_vals, &type_info)
                .or_else(|| self.emit_ordering_method(method_name, &arg_vals)),

            // Unit: clone only
            TypeInfo::Unit => self.emit_primitive_method(method_name, &arg_vals, &type_info),

            // Strings: clone, length/len, is_empty, concat, to_str, iter, trait methods
            TypeInfo::Str => match method_name {
                "clone" => self.emit_rc_inc_clone(arg_vals[0], receiver_ty),
                "length" | "len" => self.emit_str_length(arg_vals[0]),
                "is_empty" => self.emit_str_is_empty(arg_vals[0]),
                "concat" if arg_vals.len() >= 2 => Some(self.emit_str_runtime_call(
                    "ori_str_concat",
                    arg_vals[0],
                    arg_vals[1],
                    true,
                )),
                "to_str" => Some(arg_vals[0]),
                "iter" => self.emit_str_iter(arg_vals[0]),
                _ => self.emit_str_trait_method(method_name, &arg_vals),
            },

            // Lists: clone, length/len, is_empty, iter, get, push, contains, traits
            TypeInfo::List { element } => {
                let elem = *element;
                match method_name {
                    "clone" => self.emit_rc_inc_clone(arg_vals[0], receiver_ty),
                    "length" | "len" => self.emit_list_length(arg_vals[0]),
                    "is_empty" => self.emit_list_is_empty(arg_vals[0]),
                    "iter" => self.emit_list_iter(arg_vals[0], receiver_ty, elem),
                    "equals" | "is_equal" if arg_vals.len() >= 2 => {
                        self.emit_element_equals(arg_vals[0], arg_vals[1], receiver_ty)
                    }
                    "compare" if arg_vals.len() >= 2 => {
                        self.emit_element_compare(arg_vals[0], arg_vals[1], receiver_ty)
                    }
                    "hash" => self.emit_element_hash(arg_vals[0], receiver_ty),
                    _ => None,
                }
            }

            // Maps: clone, length/len, iter
            TypeInfo::Map { key, value } => {
                let k = *key;
                let v = *value;
                match method_name {
                    "clone" => self.emit_rc_inc_clone(arg_vals[0], receiver_ty),
                    "length" | "len" => self.emit_map_length(arg_vals[0]),
                    "iter" => self.emit_map_iter(arg_vals[0], k, v),
                    _ => None,
                }
            }

            // Sets: clone, length/len, iter
            TypeInfo::Set { element } => {
                let elem = *element;
                match method_name {
                    "clone" => self.emit_rc_inc_clone(arg_vals[0], receiver_ty),
                    "length" | "len" => self.emit_set_length(arg_vals[0]),
                    "iter" => self.emit_list_iter(arg_vals[0], receiver_ty, elem),
                    _ => None,
                }
            }

            // Range: iter
            TypeInfo::Range => match method_name {
                "iter" => self.emit_range_iter(arg_vals[0]),
                _ => None,
            },

            // Option: is_some, is_none, unwrap, traits (equals, compare, hash)
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_method(method_name, &arg_vals, receiver_ty)
                    .or_else(|| match method_name {
                        "equals" | "is_equal" if arg_vals.len() >= 2 => {
                            self.emit_option_equals(arg_vals[0], arg_vals[1], inner)
                        }
                        "compare" if arg_vals.len() >= 2 => {
                            self.emit_option_compare(arg_vals[0], arg_vals[1], inner)
                        }
                        "hash" => self.emit_option_hash(arg_vals[0], inner),
                        _ => None,
                    })
            }

            // Result: is_ok, is_err, unwrap, unwrap_err, traits
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_method(method_name, &arg_vals, receiver_ty)
                    .or_else(|| match method_name {
                        "equals" | "is_equal" if arg_vals.len() >= 2 => {
                            self.emit_result_equals(arg_vals[0], arg_vals[1], ok, err)
                        }
                        "compare" if arg_vals.len() >= 2 => {
                            self.emit_result_compare(arg_vals[0], arg_vals[1], ok, err)
                        }
                        "hash" => self.emit_result_hash(arg_vals[0], ok, err),
                        _ => None,
                    })
            }

            // Iterator: adapters and consumers
            TypeInfo::Iterator { element } => {
                let elem = *element;
                self.emit_iterator_method(method_name, &arg_vals, args, arc_func, elem)
            }

            // Tuples: clone, trait methods (equals, compare, hash)
            TypeInfo::Tuple { elements } => {
                let elems = elements.clone();
                match method_name {
                    "clone" => self.emit_rc_inc_clone(arg_vals[0], receiver_ty),
                    "equals" | "is_equal" if arg_vals.len() >= 2 => {
                        self.emit_tuple_equals(arg_vals[0], arg_vals[1], &elems)
                    }
                    "compare" if arg_vals.len() >= 2 => {
                        self.emit_tuple_compare(arg_vals[0], arg_vals[1], &elems)
                    }
                    "hash" => self.emit_tuple_hash(arg_vals[0], &elems),
                    _ => None,
                }
            }

            // Structs/Enums/Functions: clone (RC inc)
            TypeInfo::Struct { .. } | TypeInfo::Enum { .. } | TypeInfo::Function { .. } => {
                match method_name {
                    "clone" => self.emit_rc_inc_clone(arg_vals[0], receiver_ty),
                    _ => None,
                }
            }

            _ => None,
        }
    }

    /// Emit RC increment + return receiver (clone for heap-backed types).
    fn emit_rc_inc_clone(&mut self, val: ValueId, ty: ori_types::Idx) -> Option<ValueId> {
        if let Some(llvm_func) = self.builder.scx().llmod.get_function("ori_rc_inc") {
            let func_id = self.builder.intern_function(llvm_func);
            let data_ptrs = self.extract_rc_data_ptrs(val, ty);
            for data_ptr in data_ptrs {
                self.builder.call(func_id, &[data_ptr], "");
            }
        }
        Some(val)
    }
}
