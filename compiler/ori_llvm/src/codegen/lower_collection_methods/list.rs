//! List comparison, equality, and hashing via runtime loops.

use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // List compare — lexicographic element-by-element
    // -----------------------------------------------------------------------

    /// Emit `list.compare(other)` → Ordering (i8).
    ///
    /// Lexicographic comparison: compare elements pairwise until a non-equal
    /// result is found. If all shared elements are equal, shorter list is Less.
    ///
    /// Algorithm:
    /// 1. `min_len` = min(a.len, b.len)
    /// 2. for i in `0..min_len`: if a\[i\] != b\[i\], return compare(a\[i\], b\[i\])
    /// 3. return compare(a.len, b.len)
    pub(crate) fn emit_list_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_type: Idx,
    ) -> Option<ValueId> {
        let lhs_len = self.builder.extract_value(lhs, 0, "lcmp.a.len")?;
        let rhs_len = self.builder.extract_value(rhs, 0, "lcmp.b.len")?;
        let lhs_data = self.builder.extract_value(lhs, 2, "lcmp.a.data")?;
        let rhs_data = self.builder.extract_value(rhs, 2, "lcmp.b.data")?;

        let elem_llvm_ty = self.resolve_type(elem_type);

        // min_len = if a.len < b.len then a.len else b.len
        let cmp_lt = self.builder.icmp_slt(lhs_len, rhs_len, "lcmp.lt");
        let min_len = self.builder.select(cmp_lt, lhs_len, rhs_len, "lcmp.min");

        let entry_bb = self.builder.current_block()?;
        let header_bb = self.builder.append_block(self.current_function, "lcmp.hdr");
        let body_bb = self
            .builder
            .append_block(self.current_function, "lcmp.body");
        let early_bb = self
            .builder
            .append_block(self.current_function, "lcmp.early");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "lcmp.latch");
        let len_cmp_bb = self
            .builder
            .append_block(self.current_function, "lcmp.lencmp");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "lcmp.merge");

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx = self.builder.phi(i64_ty, "lcmp.idx");
        self.builder.add_phi_incoming(idx, &[(zero, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(idx, min_len, "lcmp.inbounds");
        self.builder.cond_br(in_bounds, body_bb, len_cmp_bb);

        // Body: compare elements
        self.builder.position_at_end(body_bb);
        let a_ptr = self
            .builder
            .gep(elem_llvm_ty, lhs_data, &[idx], "lcmp.a.ptr");
        let b_ptr = self
            .builder
            .gep(elem_llvm_ty, rhs_data, &[idx], "lcmp.b.ptr");
        let a_elem = self.builder.load(elem_llvm_ty, a_ptr, "lcmp.a.elem");
        let b_elem = self.builder.load(elem_llvm_ty, b_ptr, "lcmp.b.elem");

        let ord = self.emit_inner_compare(a_elem, b_elem, elem_type, "lcmp.elem");
        let equal_val = self.builder.const_i8(1); // Ordering::Equal
        let is_eq = self.builder.icmp_eq(ord, equal_val, "lcmp.is_eq");
        self.builder.cond_br(is_eq, latch_bb, early_bb);

        // Early exit: element comparison was non-equal
        self.builder.position_at_end(early_bb);
        let early_cur = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Latch: increment and loop
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx, one, "lcmp.next");
        self.builder.add_phi_incoming(idx, &[(next_idx, latch_bb)]);
        self.builder.br(header_bb);

        // Length comparison: all shared elements equal, compare lengths
        self.builder.position_at_end(len_cmp_bb);
        let len_ord = self.emit_icmp_ordering(lhs_len, rhs_len, "lcmp.len", true);
        let len_cmp_cur = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Merge
        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder.phi_from_incoming(
            i8_ty,
            &[(ord, early_cur), (len_ord, len_cmp_cur)],
            "lcmp.result",
        )
    }

    // -----------------------------------------------------------------------
    // List hash — fold with hash_combine
    // -----------------------------------------------------------------------

    /// Emit `list.hash()` → i64.
    ///
    /// Folds element hashes with `hash_combine`, starting from seed 0.
    /// Matches the evaluator's `hash_value` for `Value::List`.
    pub(crate) fn emit_list_hash(&mut self, val: ValueId, elem_type: Idx) -> Option<ValueId> {
        let len = self.builder.extract_value(val, 0, "lhash.len")?;
        let data = self.builder.extract_value(val, 2, "lhash.data")?;

        let elem_llvm_ty = self.resolve_type(elem_type);

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "lhash.hdr");
        let body_bb = self
            .builder
            .append_block(self.current_function, "lhash.body");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "lhash.exit");

        let zero_i64 = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + accumulator phi
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx = self.builder.phi(i64_ty, "lhash.idx");
        self.builder.add_phi_incoming(idx, &[(zero_i64, entry_bb)]);

        let seed = self.builder.phi(i64_ty, "lhash.seed");
        let zero_seed = self.builder.const_i64(0);
        self.builder
            .add_phi_incoming(seed, &[(zero_seed, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(idx, len, "lhash.inbounds");
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: hash element, combine with seed
        self.builder.position_at_end(body_bb);
        let elem_ptr = self
            .builder
            .gep(elem_llvm_ty, data, &[idx], "lhash.elem_ptr");
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "lhash.elem");

        let elem_hash = self.emit_inner_hash(elem_val, elem_type, "lhash.eh");
        let combined = self.emit_hash_combine(seed, elem_hash, "lhash");

        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx, one, "lhash.next");

        // Use current_block (not body_bb) because emit_inner_hash may create
        // intermediate blocks for compound element types (Option, Tuple, etc.)
        let body_end = self.builder.current_block()?;
        self.builder.add_phi_incoming(idx, &[(next_idx, body_end)]);
        self.builder.add_phi_incoming(seed, &[(combined, body_end)]);
        self.builder.br(header_bb);

        // Exit: return accumulated hash
        self.builder.position_at_end(exit_bb);
        Some(seed)
    }

    // -----------------------------------------------------------------------
    // List equals — length check + element-wise comparison
    // -----------------------------------------------------------------------

    /// Emit `list.equals(other)` → bool.
    ///
    /// Short-circuits on length mismatch, then compares elements pairwise.
    /// Returns false on first non-equal pair.
    pub(crate) fn emit_list_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_type: Idx,
    ) -> Option<ValueId> {
        let lhs_len = self.builder.extract_value(lhs, 0, "leq.a.len")?;
        let rhs_len = self.builder.extract_value(rhs, 0, "leq.b.len")?;
        let lhs_data = self.builder.extract_value(lhs, 2, "leq.a.data")?;
        let rhs_data = self.builder.extract_value(rhs, 2, "leq.b.data")?;

        let elem_llvm_ty = self.resolve_type(elem_type);

        let len_eq_bb = self
            .builder
            .append_block(self.current_function, "leq.leneq");
        let header_bb = self.builder.append_block(self.current_function, "leq.hdr");
        let body_bb = self.builder.append_block(self.current_function, "leq.body");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "leq.latch");
        let true_bb = self.builder.append_block(self.current_function, "leq.true");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "leq.merge");

        // Length check
        let len_eq = self.builder.icmp_eq(lhs_len, rhs_len, "leq.len_eq");
        self.builder.cond_br(len_eq, len_eq_bb, merge_bb);
        let false_from_entry = self.builder.current_block()?;

        // Lengths match — check elements
        self.builder.position_at_end(len_eq_bb);
        self.builder.br(header_bb);

        // Header: index phi + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let zero = self.builder.const_i64(0);
        let idx = self.builder.phi(i64_ty, "leq.idx");
        self.builder.add_phi_incoming(idx, &[(zero, len_eq_bb)]);

        let in_bounds = self.builder.icmp_slt(idx, lhs_len, "leq.inbounds");
        self.builder.cond_br(in_bounds, body_bb, true_bb);

        // Body: compare elements
        self.builder.position_at_end(body_bb);
        let a_ptr = self
            .builder
            .gep(elem_llvm_ty, lhs_data, &[idx], "leq.a.ptr");
        let b_ptr = self
            .builder
            .gep(elem_llvm_ty, rhs_data, &[idx], "leq.b.ptr");
        let a_elem = self.builder.load(elem_llvm_ty, a_ptr, "leq.a.elem");
        let b_elem = self.builder.load(elem_llvm_ty, b_ptr, "leq.b.elem");

        let eq = self.emit_inner_eq(a_elem, b_elem, elem_type, "leq.elem");
        self.builder.cond_br(eq, latch_bb, merge_bb);
        let false_from_body = self.builder.current_block()?;

        // Latch: increment and loop
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx, one, "leq.next");
        self.builder.add_phi_incoming(idx, &[(next_idx, latch_bb)]);
        self.builder.br(header_bb);

        // All elements equal
        self.builder.position_at_end(true_bb);
        let true_cur = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Merge: phi(false from length/element mismatch, true from loop completion)
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        let false_val = self.builder.const_bool(false);
        let true_val = self.builder.const_bool(true);
        self.builder.phi_from_incoming(
            bool_ty,
            &[
                (false_val, false_from_entry),
                (false_val, false_from_body),
                (true_val, true_cur),
            ],
            "leq.result",
        )
    }
}
