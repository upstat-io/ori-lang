//! List structural trait codegen (equals, compare, hash).
//!
//! Implements element-wise loop comparison and hashing for `List<T>`.
//! Each method generates multi-block LLVM IR with phi nodes for the
//! loop index and accumulator.
//!
//! ## Layout
//!
//! `List<T>`: `{i64 len, i64 cap, ptr data}` — element-wise iteration
//! via GEP into the data pointer.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// `List<T>.equals(other) -> bool`
    ///
    /// Equal if same length AND all elements are equal.
    /// Layout: `{i64 len, i64 cap, ptr data}`.
    pub(in crate::codegen::arc_emitter) fn emit_list_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_len = self.builder.extract_value(lhs, 0, "list.lhs.len")?;
        let rhs_len = self.builder.extract_value(rhs, 0, "list.rhs.len")?;
        let lens_eq = self.builder.icmp_eq(lhs_len, rhs_len, "lens_eq");

        let lhs_data = self.builder.extract_value(lhs, 2, "list.lhs.data")?;
        let rhs_data = self.builder.extract_value(rhs, 2, "list.rhs.data")?;

        let func = self.current_function;
        let pre_header = self.builder.current_block().expect("current block");
        let header = self.builder.append_block(func, "leq.hdr");
        let body = self.builder.append_block(func, "leq.body");
        let exit_true = self.builder.append_block(func, "leq.true");
        let exit_false = self.builder.append_block(func, "leq.false");
        let merge = self.builder.append_block(func, "leq.merge");

        // Pre-header: lengths differ → false.
        self.builder.cond_br(lens_eq, header, exit_false);

        // Header: check index < len.
        self.builder.position_at_end(header);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "idx");
        let has_more = self.builder.icmp_slt(idx_phi, lhs_len, "has_more");
        self.builder.cond_br(has_more, body, exit_true);

        // Body: compare elements[idx].
        // Use narrowed element type for GEP/load, sext after load.
        self.builder.position_at_end(body);
        let elem_ty_id = self.int_element_llvm_type(elem_ty);
        let lhs_ptr = self.builder.gep(elem_ty_id, lhs_data, &[idx_phi], "lhs.ep");
        let rhs_ptr = self.builder.gep(elem_ty_id, rhs_data, &[idx_phi], "rhs.ep");
        let lhs_elem = self.builder.load(elem_ty_id, lhs_ptr, "lhs.e");
        let lhs_elem = self.sext_narrowed_int_element(lhs_elem, elem_ty, "lhs.sext");
        let rhs_elem = self.builder.load(elem_ty_id, rhs_ptr, "rhs.e");
        let rhs_elem = self.sext_narrowed_int_element(rhs_elem, elem_ty, "rhs.sext");
        let elem_eq = self.emit_element_equals(lhs_elem, rhs_elem, elem_ty)?;
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, "next_idx");
        let body_end = self.builder.current_block().expect("body block");
        self.builder.cond_br(elem_eq, header, exit_false);

        // Wire phi: idx starts at 0 (pre_header), increments (body).
        let zero = self.builder.const_i64(0);
        self.builder
            .add_phi_incoming(idx_phi, &[(zero, pre_header), (next_idx, body_end)]);

        // Merge.
        self.builder.position_at_end(exit_true);
        self.builder.br(merge);
        self.builder.position_at_end(exit_false);
        self.builder.br(merge);

        self.builder.position_at_end(merge);
        let bool_ty = self
            .builder
            .register_type(self.builder.scx().type_i1().into());
        let result = self.builder.phi(bool_ty, "leq.res");
        let true_val = self.builder.const_bool(true);
        let false_val = self.builder.const_bool(false);
        self.builder
            .add_phi_incoming(result, &[(true_val, exit_true), (false_val, exit_false)]);

        Some(result)
    }

    /// `List<T>.compare(other) -> Ordering`
    ///
    /// Lexicographic: compare element-wise, shorter list is Less.
    pub(in crate::codegen::arc_emitter) fn emit_list_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_len = self.builder.extract_value(lhs, 0, "list.lhs.len")?;
        let rhs_len = self.builder.extract_value(rhs, 0, "list.rhs.len")?;
        let lhs_data = self.builder.extract_value(lhs, 2, "list.lhs.data")?;
        let rhs_data = self.builder.extract_value(rhs, 2, "list.rhs.data")?;

        // min_len = min(lhs_len, rhs_len)
        let lhs_shorter = self.builder.icmp_slt(lhs_len, rhs_len, "lhs_shorter");
        let min_len = self
            .builder
            .select(lhs_shorter, lhs_len, rhs_len, "min_len");

        let func = self.current_function;
        let pre_header = self.builder.current_block().expect("current block");
        let header = self.builder.append_block(func, "lcmp.hdr");
        let body = self.builder.append_block(func, "lcmp.body");
        let diff = self.builder.append_block(func, "lcmp.diff");
        let len_cmp_block = self.builder.append_block(func, "lcmp.len");
        let merge = self.builder.append_block(func, "lcmp.merge");

        self.builder.br(header);

        // Header: check index < min_len.
        self.builder.position_at_end(header);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "idx");
        let has_more = self.builder.icmp_slt(idx_phi, min_len, "has_more");
        self.builder.cond_br(has_more, body, len_cmp_block);

        // Body: compare elements[idx].
        // Use narrowed element type for GEP/load, sext after load.
        self.builder.position_at_end(body);
        let elem_ty_id = self.int_element_llvm_type(elem_ty);
        let lhs_ptr = self.builder.gep(elem_ty_id, lhs_data, &[idx_phi], "lhs.ep");
        let rhs_ptr = self.builder.gep(elem_ty_id, rhs_data, &[idx_phi], "rhs.ep");
        let lhs_elem = self.builder.load(elem_ty_id, lhs_ptr, "lhs.e");
        let lhs_elem = self.sext_narrowed_int_element(lhs_elem, elem_ty, "lhs.sext");
        let rhs_elem = self.builder.load(elem_ty_id, rhs_ptr, "rhs.e");
        let rhs_elem = self.sext_narrowed_int_element(rhs_elem, elem_ty, "rhs.sext");
        let elem_cmp = self.emit_element_compare(lhs_elem, rhs_elem, elem_ty)?;
        let equal_ord = self.builder.const_i8(1);
        let is_eq = self.builder.icmp_eq(elem_cmp, equal_ord, "is_eq");
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, "next_idx");
        let body_end = self.builder.current_block().expect("body block");
        self.builder.cond_br(is_eq, header, diff);

        // Wire phi.
        let zero = self.builder.const_i64(0);
        self.builder
            .add_phi_incoming(idx_phi, &[(zero, pre_header), (next_idx, body_end)]);

        // Diff: element comparison gave non-Equal result.
        self.builder.position_at_end(diff);
        self.builder.br(merge);

        // Len compare: all shared elements equal, decide by length.
        self.builder.position_at_end(len_cmp_block);
        let len_ord = self
            .builder
            .emit_icmp_ordering(lhs_len, rhs_len, "len_cmp", true);
        self.builder.br(merge);

        // Merge: result from diff or len_cmp.
        self.builder.position_at_end(merge);
        let i8_ty = self
            .builder
            .register_type(self.builder.scx().type_i8().into());
        let result = self.builder.phi(i8_ty, "lcmp.res");
        self.builder
            .add_phi_incoming(result, &[(elem_cmp, diff), (len_ord, len_cmp_block)]);

        Some(result)
    }

    /// `List<T>.hash() -> int`
    ///
    /// Fold `hash_combine` over element hashes, starting from 0.
    pub(in crate::codegen::arc_emitter) fn emit_list_hash(
        &mut self,
        val: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let len = self.builder.extract_value(val, 0, "list.len")?;
        let data = self.builder.extract_value(val, 2, "list.data")?;

        let func = self.current_function;
        let pre_header = self.builder.current_block().expect("current block");
        let header = self.builder.append_block(func, "lhash.hdr");
        let body = self.builder.append_block(func, "lhash.body");
        let exit = self.builder.append_block(func, "lhash.exit");

        self.builder.br(header);

        // Header: check index < len.
        self.builder.position_at_end(header);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "idx");
        let hash_phi = self.builder.phi(i64_ty, "hash");
        let has_more = self.builder.icmp_slt(idx_phi, len, "has_more");
        self.builder.cond_br(has_more, body, exit);

        // Body: hash current element, combine.
        // Use narrowed element type for GEP/load, sext after load.
        self.builder.position_at_end(body);
        let elem_ty_id = self.int_element_llvm_type(elem_ty);
        let elem_ptr = self.builder.gep(elem_ty_id, data, &[idx_phi], "elem.ptr");
        let elem = self.builder.load(elem_ty_id, elem_ptr, "elem");
        let elem = self.sext_narrowed_int_element(elem, elem_ty, "elem.sext");
        let elem_hash = self.emit_element_hash(elem, elem_ty)?;
        let new_hash = self.emit_hash_combine(hash_phi, elem_hash);
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, "next_idx");
        let body_end = self.builder.current_block().expect("body block");
        self.builder.br(header);

        // Wire phis.
        let zero = self.builder.const_i64(0);
        self.builder
            .add_phi_incoming(idx_phi, &[(zero, pre_header), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(hash_phi, &[(zero, pre_header), (new_hash, body_end)]);

        // Exit: return accumulated hash.
        self.builder.position_at_end(exit);
        Some(hash_phi)
    }
}
