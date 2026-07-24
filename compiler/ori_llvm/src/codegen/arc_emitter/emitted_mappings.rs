//! ARC variable and block mappings for emitted LLVM values.

use ori_arc::ir::{ArcFunction, ArcValue, ArcVarId};
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::context::EmittedValue;
use super::emitter_utils::required_var_repr;
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Return an ARC variable's raw LLVM value.
    ///
    /// Missing variables record a codegen error and return the builder's poison value.
    pub(super) fn var(&self, v: ArcVarId) -> ValueId {
        self.var_emitted(v).into_raw()
    }

    /// Return addressable field storage for an ARC variable.
    ///
    /// RC pointers are already addressable; by-value representations are spilled
    /// because LLVM field GEPs cannot address aggregate SSA values directly.
    pub(super) fn var_field_base_ptr(&mut self, v: ArcVarId, base_ty: Idx) -> ValueId {
        let emitted = self.var_emitted(v);
        if let EmittedValue::RcPointer(ptr) = emitted {
            return ptr;
        }
        let llvm_ty = self.resolve_type(base_ty);
        let slot = self.builder.alloca(llvm_ty, "burden.spill");
        self.builder.store(emitted.into_raw(), slot);
        slot
    }

    /// Look up the typed emitted value for an ARC variable.
    ///
    /// Returns the full [`EmittedValue`] including representation info.
    /// Prefer this over [`var`](Self::var) when the consumer needs to
    /// distinguish between value kinds (e.g., RC operations).
    pub(super) fn var_emitted(&self, v: ArcVarId) -> EmittedValue {
        debug_assert!(v.is_valid(), "var_emitted called with INVALID ArcVarId");
        if let Some(Some(val)) = self.var_map.get(v.index()) {
            *val
        } else {
            tracing::error!(var = v.raw(), "ArcIrEmitter: variable not yet defined");
            self.builder.record_codegen_error();
            // Why: ValueId::NONE cascades into LLVM crashes before the sticky codegen error is rejected.
            EmittedValue::Immediate(self.builder.poison_value)
        }
    }

    /// Bind an ARC variable to a typed LLVM value.
    pub(super) fn def_var(&mut self, v: ArcVarId, val: EmittedValue) {
        let idx = v.index();
        if idx >= self.var_map.len() {
            self.var_map.resize(idx + 1, None);
        }
        self.var_map[idx] = Some(val);
    }

    /// Bind an ARC variable using its required representation and local narrowing.
    pub(super) fn def_var_repr(&mut self, v: ArcVarId, val: ValueId, func: &ArcFunction) {
        let repr = required_var_repr(v, func);
        let final_val = self.narrow_local_if_needed(v, val);
        self.def_var(v, EmittedValue::from_repr(repr, final_val));
    }

    /// Look up the LLVM block for an ARC block.
    ///
    /// Missing blocks record a codegen error and return the entry block.
    pub(super) fn block(&self, b: ori_arc::ir::ArcBlockId) -> super::BlockId {
        debug_assert!(b.is_valid(), "block() called with INVALID ArcBlockId");
        if let Some(Some(block_id)) = self.block_map.get(b.index()) {
            *block_id
        } else {
            tracing::error!(
                block = b.raw(),
                block_map_len = self.block_map.len(),
                "ArcIrEmitter: block not mapped (out of bounds or dead unwind)"
            );
            self.builder.record_codegen_error();
            // Why: A poison block violates IR reachability checks; the sticky error prevents execution.
            let Some(entry) = self.block_map.first().copied().flatten() else {
                // Why: Function emission installs the entry block before mapping ARC blocks.
                unreachable!("entry block must always exist in block_map")
            };
            entry
        }
    }
}

/// Resolve transitive `Let` aliases to their root variable.
pub(super) fn let_alias_root(function: &ArcFunction, mut value: ArcVarId) -> ArcVarId {
    loop {
        let source = function
            .blocks
            .iter()
            .flat_map(|block| &block.body)
            .find_map(|instruction| match instruction {
                ori_arc::ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(source),
                    ..
                } if *dst == value => Some(*source),
                _ => None,
            });
        match source {
            Some(source) if source != value => value = source,
            _ => return value,
        }
    }
}
