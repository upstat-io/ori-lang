//! Set equality and hashing via runtime loops.

use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Set equals — O(n²) element-wise containment check
    // -----------------------------------------------------------------------

    /// Emit `set.equals(other)` → bool.
    ///
    /// Two sets are equal if they have the same length and every element in A
    /// exists in B. Uses O(n²) nested loops (correct; runtime hash-lookup
    /// optimization deferred until Map/Set runtime functions exist).
    pub(crate) fn emit_set_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_type: Idx,
    ) -> Option<ValueId> {
        let a_len = self.builder.extract_value(lhs, 0, "seq.a.len")?;
        let b_len = self.builder.extract_value(rhs, 0, "seq.b.len")?;
        let a_data = self.builder.extract_value(lhs, 2, "seq.a.data")?;
        let b_data = self.builder.extract_value(rhs, 2, "seq.b.data")?;

        let elem_llvm_ty = self.resolve_type(elem_type);

        let len_eq_bb = self
            .builder
            .append_block(self.current_function, "seq.leneq");
        let outer_hdr = self.builder.append_block(self.current_function, "seq.ohdr");
        let outer_body = self
            .builder
            .append_block(self.current_function, "seq.obody");
        let inner_hdr = self.builder.append_block(self.current_function, "seq.ihdr");
        let inner_body = self
            .builder
            .append_block(self.current_function, "seq.ibody");
        let inner_latch = self
            .builder
            .append_block(self.current_function, "seq.ilatch");
        let found_bb = self
            .builder
            .append_block(self.current_function, "seq.found");
        let true_bb = self.builder.append_block(self.current_function, "seq.true");
        let false_bb = self
            .builder
            .append_block(self.current_function, "seq.false");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "seq.merge");

        // Entry: length check
        let len_eq = self.builder.icmp_eq(a_len, b_len, "seq.len_eq");
        self.builder.cond_br(len_eq, len_eq_bb, false_bb);

        self.builder.position_at_end(len_eq_bb);
        self.builder.br(outer_hdr);

        // Outer loop: iterate elements of A
        self.builder.position_at_end(outer_hdr);
        let i64_ty = self.builder.i64_type();
        let zero = self.builder.const_i64(0);
        let i = self.builder.phi(i64_ty, "seq.i");
        self.builder.add_phi_incoming(i, &[(zero, len_eq_bb)]);
        let i_ok = self.builder.icmp_slt(i, a_len, "seq.i.ok");
        self.builder.cond_br(i_ok, outer_body, true_bb);

        // Outer body: load a[i], start inner search
        self.builder.position_at_end(outer_body);
        let a_ptr = self.builder.gep(elem_llvm_ty, a_data, &[i], "seq.a.ptr");
        let a_elem = self.builder.load(elem_llvm_ty, a_ptr, "seq.a.elem");
        self.builder.br(inner_hdr);

        // Inner loop: search for a[i] in B
        self.builder.position_at_end(inner_hdr);
        let j = self.builder.phi(i64_ty, "seq.j");
        self.builder.add_phi_incoming(j, &[(zero, outer_body)]);
        let j_ok = self.builder.icmp_slt(j, b_len, "seq.j.ok");
        self.builder.cond_br(j_ok, inner_body, false_bb);

        // Inner body: compare a[i] with b[j]
        self.builder.position_at_end(inner_body);
        let b_ptr = self.builder.gep(elem_llvm_ty, b_data, &[j], "seq.b.ptr");
        let b_elem = self.builder.load(elem_llvm_ty, b_ptr, "seq.b.elem");
        let eq = self.emit_inner_eq(a_elem, b_elem, elem_type, "seq.cmp");
        self.builder.cond_br(eq, found_bb, inner_latch);

        // Inner latch: j++
        self.builder.position_at_end(inner_latch);
        let one = self.builder.const_i64(1);
        let next_j = self.builder.add(j, one, "seq.j.next");
        self.builder.add_phi_incoming(j, &[(next_j, inner_latch)]);
        self.builder.br(inner_hdr);

        // Found: element matched, advance outer loop
        self.builder.position_at_end(found_bb);
        let next_i = self.builder.add(i, one, "seq.i.next");
        self.builder.add_phi_incoming(i, &[(next_i, found_bb)]);
        self.builder.br(outer_hdr);

        // All elements found
        self.builder.position_at_end(true_bb);
        let true_cur = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Mismatch (length or element not found)
        self.builder.position_at_end(false_bb);
        let false_cur = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Merge
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        let true_val = self.builder.const_bool(true);
        let false_val = self.builder.const_bool(false);
        self.builder.phi_from_incoming(
            bool_ty,
            &[(true_val, true_cur), (false_val, false_cur)],
            "seq.result",
        )
    }

    // -----------------------------------------------------------------------
    // Set hash — XOR of element hashes (order-independent)
    // -----------------------------------------------------------------------

    /// Emit `set.hash()` → i64.
    ///
    /// XOR of all element hashes. XOR is commutative and associative,
    /// producing the same result regardless of iteration order — essential
    /// for an unordered container. Matches the evaluator's `hash_value`
    /// for `Value::Set`.
    pub(crate) fn emit_set_hash(&mut self, val: ValueId, elem_type: Idx) -> Option<ValueId> {
        let len = self.builder.extract_value(val, 0, "shash.len")?;
        let data = self.builder.extract_value(val, 2, "shash.data")?;

        let elem_llvm_ty = self.resolve_type(elem_type);

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "shash.hdr");
        let body_bb = self
            .builder
            .append_block(self.current_function, "shash.body");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "shash.exit");

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + hash accumulator phi
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx = self.builder.phi(i64_ty, "shash.idx");
        self.builder.add_phi_incoming(idx, &[(zero, entry_bb)]);

        let acc = self.builder.phi(i64_ty, "shash.acc");
        self.builder.add_phi_incoming(acc, &[(zero, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(idx, len, "shash.inbounds");
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: hash element, XOR into accumulator
        self.builder.position_at_end(body_bb);
        let elem_ptr = self.builder.gep(elem_llvm_ty, data, &[idx], "shash.ptr");
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "shash.elem");

        let elem_hash = self.emit_inner_hash(elem_val, elem_type, "shash.eh");
        let new_acc = self.builder.xor(acc, elem_hash, "shash.xor");

        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx, one, "shash.next");

        let body_end = self.builder.current_block()?;
        self.builder.add_phi_incoming(idx, &[(next_idx, body_end)]);
        self.builder.add_phi_incoming(acc, &[(new_acc, body_end)]);
        self.builder.br(header_bb);

        // Exit: return accumulated hash
        self.builder.position_at_end(exit_bb);
        Some(acc)
    }
}
