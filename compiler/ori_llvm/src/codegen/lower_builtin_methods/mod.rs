//! Built-in method dispatch for V2 codegen.
//!
//! Handles type-specific method calls that are compiled inline rather than
//! dispatched to user-defined functions. Extracted from `lower_calls.rs` to
//! keep files under 500 lines.
//!
//! # Supported Types
//!
//! - **Primitives**: int, float, bool, byte, char (compare, hash, abs, clone)
//! - **Ordering**: `is_less`, `is_equal`, `is_greater`, `reverse`, `compare`, `equals`, `hash`
//! - **Str**: `len`, `is_empty`, `compare`, `equals`, `hash`, `clone`
//! - **Option**: `is_some`, `is_none`, `unwrap`, `unwrap_or`, `compare`, `equals`, `hash`, `clone`
//! - **Result**: `is_ok`, `is_err`, `unwrap`, `compare`, `equals`, `hash`, `clone`
//! - **Tuple**: `len`, `compare`, `equals`, `hash`, `clone`
//! - **List**: `len`, `is_empty`, `clone`, `compare`, `equals`, `hash`
//! - **Map**: `clone`, `equals`, `hash`
//! - **Set**: `clone`, `equals`, `hash`

mod collections;
mod helpers;
mod inner_dispatch;
mod option;
mod primitives;
mod result;
mod tuple;

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Dispatch built-in methods based on receiver type.
    ///
    /// Returns `None` if the method is not a built-in, allowing fallthrough
    /// to user-defined method lookup.
    pub(crate) fn lower_builtin_method(
        &mut self,
        recv_val: ValueId,
        recv_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match recv_type {
            Idx::INT | Idx::DURATION | Idx::SIZE => self.lower_int_method(recv_val, method, args),
            Idx::FLOAT => self.lower_float_method(recv_val, method, args),
            Idx::BOOL => self.lower_bool_method(recv_val, method, args),
            Idx::ORDERING => self.lower_ordering_method(recv_val, method, args),
            Idx::STR => self.lower_str_method(recv_val, method, args),
            Idx::BYTE => self.lower_byte_method(recv_val, method, args),
            Idx::CHAR => self.lower_char_method(recv_val, method, args),
            _ => match self.type_info.get(recv_type) {
                TypeInfo::Option { inner } => {
                    self.lower_option_method(recv_val, inner, method, args)
                }
                TypeInfo::Result { ok, err } => {
                    self.lower_result_method(recv_val, ok, err, method, args)
                }
                TypeInfo::List { element } => {
                    self.lower_list_method(recv_val, element, method, args)
                }
                TypeInfo::Tuple { elements } => {
                    self.lower_tuple_method(recv_val, &elements, method, args)
                }
                TypeInfo::Map { key, value } => {
                    self.lower_map_method(recv_val, key, value, method, args)
                }
                TypeInfo::Set { element } => self.lower_set_method(recv_val, element, method, args),
                _ => None,
            },
        }
    }
}
