//! Narrowing codegen helpers for [`ArcIrEmitter`].
//!
//! Handles integer and float narrowing at three boundaries:
//!
//! - **Collection elements** (integer narrowing Phase C): `trunc`/`sext` when storing/loading
//!   narrowed int elements in collection backing buffers.
//! - **Local variables** (integer narrowing Phase B): `trunc+sext` pairs on definition for
//!   variables whose value range fits in a narrower integer type.
//! - **Struct fields** (integer/float narrowing): `trunc`/`sext`/`fptrunc`/`fpext` when
//!   constructing or extracting fields of narrowed structs.

mod loop_exclusions;
mod struct_fields;

#[cfg(test)]
mod tests;

use super::ArcIrEmitter;
use crate::codegen::value_id::{LLVMTypeId, ValueId};
use loop_exclusions::loop_carried_narrowing_exclusions;
use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::{Idx, Pool};

/// SSOT for narrowed collection-element width, keyed on a SPECIFIC collection
/// `Idx`. Resolves `collection_idx` through the `Pool`, looks up that exact
/// type's `ReprPlan` entry, and returns the narrowed element `IntWidth` when
/// the collection's element repr is `Int` narrowed below canonical `i64`.
///
/// Keying on `collection_idx` (not a `ReprPlan`-wide scan) is correct when a
/// program holds two narrowed-int collections of different widths: each
/// collection's stride comes from its own entry, never the first match in an
/// unordered map.
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

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // Collection element narrowing.

    /// Returns the narrowed `IntWidth` if the collection's element repr has been
    /// narrowed below canonical `i64`, or `None` otherwise.
    pub(super) fn narrowed_collection_element_width(
        &self,
        collection_idx: Idx,
    ) -> Option<ori_repr::IntWidth> {
        let plan = self.repr_plan?;
        narrowed_collection_element_width(plan, self.pool, collection_idx)
    }

    /// Compute the element store size for a collection, consulting `ReprPlan`
    /// for narrowed element types before falling back to the canonical size.
    ///
    /// `collection_idx` is the collection type (e.g., `pool.list(elem_ty)`).
    /// If the collection has narrowed int elements, returns the narrowed byte
    /// size (1/2/4). Otherwise, falls back to `element_store_size(elem_ty)`.
    pub(crate) fn collection_elem_size(&self, collection_idx: Idx, elem_ty: Idx) -> u64 {
        if let Some(width) = self.narrowed_collection_element_width(collection_idx) {
            return u64::from(width.size_bytes());
        }
        self.element_store_size(elem_ty)
    }

    /// Get the LLVM type for a collection's element, respecting narrowing.
    ///
    /// If the collection has narrowed int elements, returns the narrowed LLVM
    /// type (i8/i16/i32). Otherwise, returns the canonical LLVM type.
    pub(super) fn collection_elem_llvm_type(
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
    /// Used before storing an element into a narrowed collection's backing buffer.
    pub(super) fn trunc_for_narrowed_collection_element(
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
    /// Used after loading an element from a narrowed collection's backing buffer.
    pub(super) fn sext_narrowed_collection_element(
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

    /// Get the element type for one exact collection representation.
    pub(super) fn int_element_llvm_type(
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
    pub(super) fn sext_narrowed_int_element(
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

    // Local variable narrowing (integer narrowing phase B)

    /// Insert trunc+sext for a narrowed local variable (integer narrowing phase B).
    ///
    /// If `v` is in `narrowed_vars`, truncates `val` from i64 to the narrow
    /// width, then sign-extends back to i64. Returns the sext'd value.
    /// If `v` is not narrowed, returns `val` unchanged.
    pub(super) fn narrow_local_if_needed(&mut self, v: ArcVarId, val: ValueId) -> ValueId {
        let Some(&width) = self.narrowed_vars.get(&v) else {
            return val;
        };

        // Only narrow actual i64 int values — skip non-int, zero-sized, pairs.
        let llvm_val = self.builder.arena.get_value(val);
        if !llvm_val.is_int_value() {
            return val;
        }
        let bits = llvm_val.into_int_value().get_type().get_bit_width();
        if bits <= width.size_bytes() * 8 {
            // Already narrow or narrower — no truncation needed.
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

    /// Compute which local variables should use narrow int types (integer narrowing phase B).
    ///
    /// Scans all variables in the function and checks if their value range
    /// (from the `ReprPlan`) fits in a narrower integer type. Function
    /// parameters are excluded (no visibility info in ARC IR — conservative).
    ///
    /// For variables with no range in the plan (e.g., fresh variables created
    /// by block-merge Select folding), derives ranges locally from defining
    /// instructions: `Let{Literal(Int(n))}` → `[n,n]`, `Let{Var(src)}` →
    /// source range, `Select` → `join(true_val, false_val)`.
    pub(super) fn compute_narrowed_vars(&mut self, func: &ArcFunction) {
        use ori_types::Tag;

        self.narrowed_vars.clear();

        let Some(plan) = self.type_resolver.repr_plan() else {
            return;
        };

        // Collect parameter variable IDs for exclusion.
        let param_vars: rustc_hash::FxHashSet<ArcVarId> =
            func.params.iter().map(|p| p.var).collect();
        let loop_carried = loop_carried_narrowing_exclusions(func);

        // Build defining-instruction map for local range derivation.
        // Block-merge creates fresh variables (Select destinations, renamed
        // arm-local Let bindings) that have no range in the ReprPlan because
        // range analysis ran on pre-merge IR. This map lets us derive ranges
        // from the post-merge defining instructions.
        let mut def_instrs: rustc_hash::FxHashMap<ArcVarId, &ori_arc::ir::ArcInstr> =
            rustc_hash::FxHashMap::default();
        for block in &func.blocks {
            for instr in &block.body {
                if let Some(var) = instr.defined_var() {
                    def_instrs.insert(var, instr);
                }
            }
        }

        // Local range cache — avoids re-deriving the same variable.
        let mut local_ranges: rustc_hash::FxHashMap<ArcVarId, ori_repr::ValueRange> =
            rustc_hash::FxHashMap::default();

        // Check each variable in the function.
        for (raw_idx, &ty_idx) in func.var_types.iter().enumerate() {
            let Ok(raw_idx) = u32::try_from(raw_idx) else {
                self.builder.record_codegen_error_with_msg(
                    "ARC variable table exceeds the supported u32 identity range",
                );
                break;
            };
            let var = ArcVarId::new(raw_idx);

            // Skip function parameters (can't distinguish pub from private).
            if param_vars.contains(&var) || loop_carried.contains(&var) {
                continue;
            }

            // Only narrow int-typed variables.
            let resolved = self.pool.resolve_fully(ty_idx);
            if self.pool.tag(resolved) != Tag::Int {
                continue;
            }

            // Query the range analysis, falling back to local derivation
            // for fresh post-merge variables.
            let range = super::narrowing_local::derive_local_range(
                var,
                func.name,
                plan,
                &def_instrs,
                &mut local_ranges,
            );
            let width = range.min_width();

            // Only record if narrower than canonical i64.
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

    /// Get the LLVM integer type for a given `IntWidth`.
    pub(super) fn llvm_type_for_int_width(&mut self, width: ori_repr::IntWidth) -> LLVMTypeId {
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
