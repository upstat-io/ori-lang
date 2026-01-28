//! PHI node construction utilities.

use inkwell::values::BasicValueEnum;
use ori_ir::TypeId;
use tracing::{debug, trace};

use crate::builder::Builder;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Build a phi node for the given incoming values.
    ///
    /// Uses the actual type from the first incoming value rather than
    /// `llvm_type(type_id)` to avoid type mismatches with compound types.
    ///
    /// Returns None if there are no incoming values.
    /// Returns the single value directly (no phi) if there's only one incoming.
    pub(crate) fn build_phi_from_incoming(
        &self,
        _type_id: TypeId,
        incoming: &[(BasicValueEnum<'ll>, inkwell::basic_block::BasicBlock<'ll>)],
    ) -> Option<BasicValueEnum<'ll>> {
        trace!("build_phi_from_incoming: {} incoming values", incoming.len());

        if incoming.is_empty() {
            trace!("  -> empty incoming, returning None");
            return None;
        }

        // Single incoming value - no phi needed, just return the value
        if incoming.len() == 1 {
            trace!("  -> single incoming, returning value directly (no phi)");
            return Some(incoming[0].0);
        }

        // Log types of all incoming values for debugging
        for (i, (val, bb)) in incoming.iter().enumerate() {
            trace!(
                "  incoming[{}]: type={:?}, block={:?}",
                i,
                val.get_type(),
                bb.get_name().to_str().unwrap_or("?")
            );
        }

        // Multiple incoming values - create phi node
        let first_type = incoming[0].0.get_type();
        debug!("Creating phi node with type: {:?}", first_type);
        let phi = self.phi(first_type, "phi");

        // Collect incoming values with type coercion if needed
        for (i, (val, bb)) in incoming.iter().enumerate() {
            let val_type = val.get_type();
            if val_type != first_type {
                debug!(
                    "  incoming[{}]: TYPE MISMATCH! expected {:?}, got {:?}",
                    i, first_type, val_type
                );
            }
            // Ensure type compatibility
            let coerced = self.coerce_value_to_type(*val, first_type);
            phi.add_incoming(&[(&coerced, *bb)]);
        }

        Some(phi.as_basic_value())
    }

    /// Coerce a value to match a target type.
    pub(crate) fn coerce_value_to_type(
        &self,
        val: BasicValueEnum<'ll>,
        target_type: inkwell::types::BasicTypeEnum<'ll>,
    ) -> BasicValueEnum<'ll> {
        let val_type = val.get_type();

        // If types match, no coercion needed
        if val_type == target_type {
            return val;
        }

        // Handle int-to-int coercion
        if let (
            inkwell::types::BasicTypeEnum::IntType(val_int_ty),
            inkwell::types::BasicTypeEnum::IntType(target_int_ty),
        ) = (val_type, target_type)
        {
            let val_int = val.into_int_value();
            let val_width = val_int_ty.get_bit_width();
            let target_width = target_int_ty.get_bit_width();

            return if val_width < target_width {
                self.zext(val_int, target_int_ty, "coerce").into()
            } else if val_width > target_width {
                self.trunc(val_int, target_int_ty, "coerce").into()
            } else {
                val
            };
        }

        // For other type mismatches, use bitcast if sizes match or return as-is
        val
    }
}
