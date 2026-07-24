//! Narrowing codegen implementation for [`ArcIrEmitter`].
//!
//! Handles integer and float narrowing at three boundaries:
//!
//! - **Collection elements** (integer narrowing Phase C): `trunc`/`sext` when storing/loading
//!   narrowed int elements in collection backing buffers.
//! - **Local variables** (integer narrowing Phase B): `trunc+sext` pairs on definition for
//!   variables whose value range fits in a narrower integer type.
//! - **Struct fields** (integer/float narrowing): `trunc`/`sext`/`fptrunc`/`fpext` when
//!   constructing or extracting fields of narrowed structs.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::{Idx, Pool};

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;
use super::loop_exclusions::loop_carried_narrowing_exclusions;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Returns the narrowed `IntWidth` if the collection's element repr has been
    /// narrowed below canonical `i64`, or `None` otherwise.
    pub(in crate::codegen::arc_emitter) fn narrowed_collection_element_width(
        &self,
        collection_idx: Idx,
    ) -> Option<ori_repr::IntWidth> {
        let plan = self.repr_plan?;
        narrowed_collection_element_width(plan, self.pool, collection_idx)
    }

    /// Returns the element's narrowed byte size when `collection_idx` has one,
    /// or the canonical store size otherwise.
    pub(crate) fn collection_elem_size(&self, collection_idx: Idx, elem_ty: Idx) -> u64 {
        if let Some(width) = self.narrowed_collection_element_width(collection_idx) {
            return u64::from(width.size_bytes());
        }
        self.element_store_size(elem_ty)
    }

    /// Returns the collection element's narrowed LLVM type, or its canonical
    /// LLVM type when the representation plan has no narrowing.
    pub(in crate::codegen::arc_emitter) fn collection_elem_llvm_type(
        &mut self,
        collection_idx: Idx,
        elem_ty: Idx,
    ) -> LLVMTypeId {
        if let Some(width) = self.narrowed_collection_element_width(collection_idx) {
            return self.llvm_type_for_int_width(width);
        }
        self.resolve_type(elem_ty)
    }

    /// Truncate a canonical i64 value to the collection's narrowed element width.
    ///
    /// Returns `val` unchanged if the collection has no narrowed elements.
    /// Otherwise the result matches the collection backing buffer's element type.
    pub(in crate::codegen::arc_emitter) fn trunc_for_narrowed_collection_element(
        &mut self,
        val: ValueId,
        collection_idx: Idx,
        label: &str,
    ) -> ValueId {
        let Some(width) = self.narrowed_collection_element_width(collection_idx) else {
            return val;
        };
        let narrow_ty = self.llvm_type_for_int_width(width);
        self.builder.trunc(val, narrow_ty, label)
    }

    /// Sign-extend a narrowed collection element back to canonical i64.
    ///
    /// Returns `val` unchanged if the collection has no narrowed elements.
    /// Otherwise the result has the canonical integer representation.
    pub(in crate::codegen::arc_emitter) fn sext_narrowed_collection_element(
        &mut self,
        val: ValueId,
        collection_idx: Idx,
        label: &str,
    ) -> ValueId {
        let Some(_width) = self.narrowed_collection_element_width(collection_idx) else {
            return val;
        };
        let i64_ty = self
            .builder
            .register_type(self.builder.scx.type_i64().into());
        self.builder.sext(val, i64_ty, label)
    }

    /// Returns the element type for one exact collection representation.
    pub(in crate::codegen::arc_emitter) fn int_element_llvm_type(
        &mut self,
        collection_idx: Idx,
        elem_ty: Idx,
    ) -> LLVMTypeId {
        if self.pool.tag(self.pool.resolve_fully(elem_ty)) == ori_types::Tag::Int {
            if let Some(width) = self.narrowed_collection_element_width(collection_idx) {
                return self.llvm_type_for_int_width(width);
            }
        }
        self.resolve_type(elem_ty)
    }

    /// Sign-extend a potentially narrowed int element to canonical i64.
    ///
    /// No-op if int elements are not narrowed.
    pub(in crate::codegen::arc_emitter) fn sext_narrowed_int_element(
        &mut self,
        val: ValueId,
        collection_idx: Idx,
        elem_ty: Idx,
        label: &str,
    ) -> ValueId {
        if self.pool.tag(self.pool.resolve_fully(elem_ty)) == ori_types::Tag::Int
            && self
                .narrowed_collection_element_width(collection_idx)
                .is_some()
        {
            let i64_ty = self
                .builder
                .register_type(self.builder.scx.type_i64().into());
            return self.builder.sext(val, i64_ty, label);
        }
        val
    }

    /// Applies the narrowed representation selected for `v`, returning `val`
    /// unchanged when the local has no narrowing or is already no wider.
    pub(in crate::codegen::arc_emitter) fn narrow_local_if_needed(
        &mut self,
        v: ArcVarId,
        val: ValueId,
    ) -> ValueId {
        let Some(&width) = self.narrowed_vars.get(&v) else {
            return val;
        };

        let llvm_val = self.builder.arena.get_value(val);
        if !llvm_val.is_int_value() {
            return val;
        }
        let bits = llvm_val.into_int_value().get_type().get_bit_width();
        if bits <= width.size_bytes() * 8 {
            return val;
        }

        let narrow_ty = self.llvm_type_for_int_width(width);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx.type_i64().into());

        let truncated = self
            .builder
            .trunc(val, narrow_ty, &format!("local.trunc.{}", v.raw()));
        self.builder
            .sext(truncated, i64_ty, &format!("local.sext.{}", v.raw()))
    }

    /// Selects narrower integer widths from representation-plan ranges and
    /// locally derived ranges for post-merge definitions. Parameters and
    /// loop-carried variables remain canonical.
    pub(in crate::codegen::arc_emitter) fn compute_narrowed_vars(&mut self, func: &ArcFunction) {
        use ori_types::Tag;

        self.narrowed_vars.clear();

        let Some(plan) = self.type_resolver.repr_plan() else {
            return;
        };

        let param_vars: rustc_hash::FxHashSet<ArcVarId> =
            func.params.iter().map(|p| p.var).collect();
        let loop_carried = loop_carried_narrowing_exclusions(func);

        // INVARIANT: Post-merge definitions supply ranges absent from pre-merge facts.
        let mut def_instrs: rustc_hash::FxHashMap<ArcVarId, &ori_arc::ir::ArcInstr> =
            rustc_hash::FxHashMap::default();
        for block in &func.blocks {
            for instr in &block.body {
                if let Some(var) = instr.defined_var() {
                    def_instrs.insert(var, instr);
                }
            }
        }

        let mut local_ranges: rustc_hash::FxHashMap<ArcVarId, ori_repr::ValueRange> =
            rustc_hash::FxHashMap::default();

        for (raw_idx, &ty_idx) in func.var_types.iter().enumerate() {
            let Ok(raw_idx) = u32::try_from(raw_idx) else {
                self.builder.record_codegen_error_with_msg(
                    "ARC variable table exceeds the supported u32 identity range",
                );
                break;
            };
            let var = ArcVarId::new(raw_idx);

            // Why: ARC IR does not expose whether a parameter is part of a public ABI.
            if param_vars.contains(&var) || loop_carried.contains(&var) {
                continue;
            }

            let resolved = self.pool.resolve_fully(ty_idx);
            if self.pool.tag(resolved) != Tag::Int {
                continue;
            }

            let range = crate::codegen::arc_emitter::narrowing_local::derive_local_range(
                var,
                func.name,
                plan,
                &def_instrs,
                &mut local_ranges,
            );
            let width = range.min_width();

            if width != ori_repr::IntWidth::I64 {
                self.narrowed_vars.insert(var, width);
            }
        }

        if !self.narrowed_vars.is_empty() {
            tracing::debug!(
                func = func.name.raw(),
                count = self.narrowed_vars.len(),
                "integer narrowing phase B: narrowed local variables"
            );
        }
    }

    /// Returns the LLVM integer type for `width`.
    pub(in crate::codegen::arc_emitter) fn llvm_type_for_int_width(
        &mut self,
        width: ori_repr::IntWidth,
    ) -> LLVMTypeId {
        let scx = self.builder.scx();
        let ty = match width {
            ori_repr::IntWidth::I8 => scx.type_i8().into(),
            ori_repr::IntWidth::I16 => scx.type_i16().into(),
            ori_repr::IntWidth::I32 => scx.type_i32().into(),
            ori_repr::IntWidth::I64 => scx.type_i64().into(),
        };
        self.builder.register_type(ty)
    }
}

/// Returns the narrowed element width for this exact collection representation.
/// Exact-key lookup keeps distinct collection widths independent instead of
/// selecting an unrelated entry from the representation plan.
pub(crate) fn narrowed_collection_element_width(
    plan: &ori_repr::ReprPlan,
    pool: &Pool,
    collection_idx: Idx,
) -> Option<ori_repr::IntWidth> {
    use ori_repr::{FatRepr, IntWidth, MachineRepr};

    let resolved = pool.resolve_fully(collection_idx);
    let repr = plan.get_repr(resolved)?;
    if let MachineRepr::FatPointer(FatRepr::Collection { ref element_repr }) = repr {
        if let MachineRepr::Int { width, .. } = element_repr.as_ref() {
            if *width != IntWidth::I64 {
                return Some(*width);
            }
        }
    }
    None
}
