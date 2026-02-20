//! Collection type method lowering: List, Map, Set dispatch.
//!
//! These are thin wrappers that dispatch to the loop-based implementations
//! in `lower_collection_methods.rs`, which handles the actual element-wise
//! iteration because element count is dynamic.

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // List methods

    pub(super) fn lower_list_method(
        &mut self,
        recv: ValueId,
        element: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "list.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "list.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
            }
            "clone" => Some(recv),
            "compare" | "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                if method == "compare" {
                    self.emit_list_compare(recv, other, element)
                } else {
                    self.emit_list_equals(recv, other, element)
                }
            }
            "hash" => self.emit_list_hash(recv, element),
            _ => None,
        }
    }

    // Map methods

    pub(super) fn lower_map_method(
        &mut self,
        recv: ValueId,
        key: Idx,
        value: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "clone" => Some(recv),
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_map_equals(recv, other, key, value)
            }
            "hash" => self.emit_map_hash(recv, key, value),
            _ => None,
        }
    }

    // Set methods

    pub(super) fn lower_set_method(
        &mut self,
        recv: ValueId,
        element: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_set_equals(recv, other, element)
            }
            "hash" => self.emit_set_hash(recv, element),
            // Into<[T]>: Set and List share layout {i64 len, i64 cap, ptr data}
            "clone" | "into" | "to_list" => Some(recv),
            _ => None,
        }
    }
}
