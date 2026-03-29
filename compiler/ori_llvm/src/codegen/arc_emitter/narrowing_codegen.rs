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

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // Collection element narrowing (integer narrowing phase C)

    /// Check if a collection type has narrowed int elements in the `ReprPlan`.
    ///
    /// Returns the narrowed `IntWidth` if the collection's element repr has been
    /// narrowed below canonical `i64`, `None` otherwise.
    pub(super) fn narrowed_collection_element_width(
        &self,
        collection_idx: Idx,
    ) -> Option<ori_repr::IntWidth> {
        use ori_repr::{FatRepr, IntWidth, MachineRepr};

        let plan = self.repr_plan?;
        let repr = plan.get_repr(collection_idx)?;
        if let MachineRepr::FatPointer(FatRepr::Collection { ref element_repr }) = repr {
            if let MachineRepr::Int { width, .. } = element_repr.as_ref() {
                if *width != IntWidth::I64 {
                    return Some(*width);
                }
            }
        }
        None
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

    /// Check if int elements in any collection are narrowed.
    ///
    /// Used in iterator paths where the source collection type is not directly
    /// available. Scans the `ReprPlan` for any collection type with narrowed int
    /// elements. Returns the narrowed width if found.
    ///
    /// This is safe because (1) the pool interns types — there is at most one
    /// `List<int>` type, and (2) sets are excluded from Phase C narrowing
    /// (eq/hash thunks are not narrowing-aware), so only list types contribute.
    pub(super) fn narrowed_int_collection_element_width(&self) -> Option<ori_repr::IntWidth> {
        use ori_repr::{FatRepr, IntWidth, MachineRepr};
        let plan = self.repr_plan?;
        for idx in plan.decision_indices() {
            if let Some(MachineRepr::FatPointer(FatRepr::Collection { ref element_repr })) =
                plan.get_repr(idx)
            {
                if let MachineRepr::Int { width, .. } = element_repr.as_ref() {
                    if *width != IntWidth::I64 {
                        return Some(*width);
                    }
                }
            }
        }
        None
    }

    /// Get narrowed element size for int-typed elements, with no collection context.
    ///
    /// Falls back to canonical `element_store_size` for non-narrowed or non-int elements.
    pub(crate) fn int_element_store_size(&self, elem_ty: Idx) -> u64 {
        if self.pool.tag(self.pool.resolve_fully(elem_ty)) == ori_types::Tag::Int {
            if let Some(width) = self.narrowed_int_collection_element_width() {
                return u64::from(width.size_bytes());
            }
        }
        self.element_store_size(elem_ty)
    }

    /// Get narrowed LLVM type for int-typed elements, with no collection context.
    pub(super) fn int_element_llvm_type(&mut self, elem_ty: Idx) -> LLVMTypeId {
        if self.pool.tag(self.pool.resolve_fully(elem_ty)) == ori_types::Tag::Int {
            if let Some(width) = self.narrowed_int_collection_element_width() {
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
        elem_ty: Idx,
        label: &str,
    ) -> ValueId {
        if self.pool.tag(self.pool.resolve_fully(elem_ty)) == ori_types::Tag::Int
            && self.narrowed_int_collection_element_width().is_some()
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
            let var = ArcVarId::new(raw_idx as u32);

            // Skip function parameters (can't distinguish pub from private).
            if param_vars.contains(&var) {
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

    // Struct field narrowing (integer/float narrowing)

    /// Truncate struct field values from canonical width (i64) to narrowed
    /// field width (i8/i16/i32) when constructing a narrowed struct.
    ///
    /// Returns a new `Vec` with truncated values where needed. Values whose
    /// LLVM type already matches the struct field type pass through unchanged.
    ///
    /// Only truncates fields whose pool type is `Tag::Int` (canonical i64) —
    /// naturally narrow types (Byte → i8, Char → i32) are not affected.
    ///
    /// This is the construction-side half of integer narrowing codegen.
    /// The extraction-side half is [`sext_narrowed_field`](Self::sext_narrowed_field).
    pub(super) fn trunc_for_narrowed_struct(
        &mut self,
        struct_ty_id: LLVMTypeId,
        args: &[ValueId],
        ctor_type: Idx,
    ) -> Vec<ValueId> {
        use inkwell::types::BasicTypeEnum;
        use ori_types::Tag;

        let raw_ty = self.builder.arena.get_type(struct_ty_id);
        let BasicTypeEnum::StructType(st) = raw_ty else {
            return args.to_vec();
        };

        // Get the struct/tuple field types from the pool.
        let resolved = self.pool.resolve_fully(ctor_type);
        let pool_tag = self.pool.tag(resolved);
        let field_pool_types: Vec<Idx> = if pool_tag == Tag::Struct {
            self.pool
                .struct_fields(resolved)
                .into_iter()
                .map(|(_, idx)| idx)
                .collect()
        } else if pool_tag == Tag::Tuple {
            self.pool.tuple_elems(resolved)
        } else {
            return args.to_vec();
        };

        args.iter()
            .enumerate()
            .map(|(i, &val)| {
                let field_pool_tag = field_pool_types
                    .get(i)
                    .map(|&idx| self.pool.tag(self.pool.resolve_fully(idx)));

                // Integer narrowing: truncate when pool field is Int (canonical i64)
                // but the struct field is narrower.
                if field_pool_tag == Some(Tag::Int) {
                    let Some(BasicTypeEnum::IntType(field_int)) =
                        st.get_field_type_at_index(i as u32)
                    else {
                        return val;
                    };
                    let field_bits = field_int.get_bit_width();
                    if field_bits >= 64 {
                        return val;
                    }
                    let v = self.builder.arena.get_value(val);
                    if !v.is_int_value() {
                        return val;
                    }
                    let val_bits = v.into_int_value().get_type().get_bit_width();
                    if val_bits > field_bits {
                        let field_ty_id = self.builder.register_type(field_int.into());
                        return self
                            .builder
                            .trunc(val, field_ty_id, &format!("narrow.trunc.{i}"));
                    }
                    return val;
                }

                // Float narrowing: fptrunc when pool field is Float (canonical f64)
                // but the struct field is f32.
                if field_pool_tag == Some(Tag::Float) {
                    let Some(BasicTypeEnum::FloatType(field_float)) =
                        st.get_field_type_at_index(i as u32)
                    else {
                        return val;
                    };
                    let v = self.builder.arena.get_value(val);
                    if !v.is_float_value() {
                        return val;
                    }
                    let val_float_ty = v.into_float_value().get_type();
                    // f32 field but f64 value → fptrunc
                    if val_float_ty != field_float {
                        let field_ty_id = self.builder.register_type(field_float.into());
                        return self.builder.float_trunc(
                            val,
                            field_ty_id,
                            &format!("narrow.fptrunc.{i}"),
                        );
                    }
                }

                val
            })
            .collect()
    }

    /// Widen a narrowed struct field value back to canonical width after
    /// extraction.
    ///
    /// For integer fields: sign-extend (i8/i16/i32 → i64) when the ARC IR
    /// destination type is `Tag::Int`. For float fields: fpext (f32 → f64)
    /// when the ARC IR destination type is `Tag::Float`.
    ///
    /// Naturally narrow types (Byte → i8, Char → i32) are not affected
    /// because their pool tag is `Tag::Byte`/`Tag::Char`, not `Tag::Int`.
    ///
    /// This is the extraction-side half of integer/float narrowing codegen.
    pub(super) fn sext_narrowed_field(
        &mut self,
        extracted: ValueId,
        field_index: u32,
        dst_type: Idx,
    ) -> ValueId {
        use ori_types::Tag;

        let resolved = self.pool.resolve_fully(dst_type);
        let tag = self.pool.tag(resolved);

        // Integer: sext when the destination expects i64 (Tag::Int).
        if tag == Tag::Int {
            let v = self.builder.arena.get_value(extracted);
            if !v.is_int_value() {
                return extracted;
            }
            let bits = v.into_int_value().get_type().get_bit_width();
            if bits >= 64 {
                return extracted;
            }
            let i64_ty = self
                .builder
                .register_type(self.builder.scx.type_i64().into());
            return self
                .builder
                .sext(extracted, i64_ty, &format!("narrow.sext.{field_index}"));
        }

        // Float narrowing: fpext when the destination expects f64 (Tag::Float).
        if tag == Tag::Float {
            let v = self.builder.arena.get_value(extracted);
            if !v.is_float_value() {
                return extracted;
            }
            let val_float_ty = v.into_float_value().get_type();
            let canonical_f64 = self.builder.scx.type_f64();
            if val_float_ty != canonical_f64 {
                let f64_ty_id = self.builder.register_type(canonical_f64.into());
                return self.builder.float_ext(
                    extracted,
                    f64_ty_id,
                    &format!("narrow.fpext.{field_index}"),
                );
            }
        }

        extracted
    }
}
