//! Map equality and hashing via runtime loops.

use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Map equals — O(n²) key-value containment check
    // -----------------------------------------------------------------------

    /// Emit `map.equals(other)` → bool.
    ///
    /// Two maps are equal if they have the same length and for every key in A,
    /// the same key exists in B with the same value. If a matching key is found
    /// but the value differs, the maps are definitively not equal (keys are
    /// unique). Uses O(n²) nested loops.
    ///
    /// Layout: Map = `{i64 len, i64 cap, ptr keys, ptr vals}`.
    pub(crate) fn emit_map_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        key_type: Idx,
        val_type: Idx,
    ) -> Option<ValueId> {
        let a_len = self.builder.extract_value(lhs, 0, "meq.a.len")?;
        let b_len = self.builder.extract_value(rhs, 0, "meq.b.len")?;
        let a_keys = self.builder.extract_value(lhs, 2, "meq.a.keys")?;
        let a_vals = self.builder.extract_value(lhs, 3, "meq.a.vals")?;
        let b_keys = self.builder.extract_value(rhs, 2, "meq.b.keys")?;
        let b_vals = self.builder.extract_value(rhs, 3, "meq.b.vals")?;

        let key_llvm_ty = self.resolve_type(key_type);
        let val_llvm_ty = self.resolve_type(val_type);

        let len_eq_bb = self
            .builder
            .append_block(self.current_function, "meq.leneq");
        let outer_hdr = self.builder.append_block(self.current_function, "meq.ohdr");
        let outer_body = self
            .builder
            .append_block(self.current_function, "meq.obody");
        let inner_hdr = self.builder.append_block(self.current_function, "meq.ihdr");
        let inner_body = self
            .builder
            .append_block(self.current_function, "meq.ibody");
        let check_val_bb = self
            .builder
            .append_block(self.current_function, "meq.chkval");
        let inner_latch = self
            .builder
            .append_block(self.current_function, "meq.ilatch");
        let found_bb = self
            .builder
            .append_block(self.current_function, "meq.found");
        let true_bb = self.builder.append_block(self.current_function, "meq.true");
        let false_bb = self
            .builder
            .append_block(self.current_function, "meq.false");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "meq.merge");

        // Entry: length check
        let len_eq = self.builder.icmp_eq(a_len, b_len, "meq.len_eq");
        self.builder.cond_br(len_eq, len_eq_bb, false_bb);

        self.builder.position_at_end(len_eq_bb);
        self.builder.br(outer_hdr);

        // Outer loop: iterate entries of A
        self.builder.position_at_end(outer_hdr);
        let i64_ty = self.builder.i64_type();
        let zero = self.builder.const_i64(0);
        let i = self.builder.phi(i64_ty, "meq.i");
        self.builder.add_phi_incoming(i, &[(zero, len_eq_bb)]);
        let i_ok = self.builder.icmp_slt(i, a_len, "meq.i.ok");
        self.builder.cond_br(i_ok, outer_body, true_bb);

        // Load a.keys[i] and a.vals[i]
        self.builder.position_at_end(outer_body);
        let ak_ptr = self.builder.gep(key_llvm_ty, a_keys, &[i], "meq.ak.ptr");
        let a_key = self.builder.load(key_llvm_ty, ak_ptr, "meq.ak");
        let av_ptr = self.builder.gep(val_llvm_ty, a_vals, &[i], "meq.av.ptr");
        let a_val = self.builder.load(val_llvm_ty, av_ptr, "meq.av");
        self.builder.br(inner_hdr);

        // Inner loop: search B for matching key
        self.builder.position_at_end(inner_hdr);
        let j = self.builder.phi(i64_ty, "meq.j");
        self.builder.add_phi_incoming(j, &[(zero, outer_body)]);
        let j_ok = self.builder.icmp_slt(j, b_len, "meq.j.ok");
        self.builder.cond_br(j_ok, inner_body, false_bb);

        // Compare keys
        self.builder.position_at_end(inner_body);
        let bk_ptr = self.builder.gep(key_llvm_ty, b_keys, &[j], "meq.bk.ptr");
        let b_key = self.builder.load(key_llvm_ty, bk_ptr, "meq.bk");
        let key_eq = self.emit_inner_eq(a_key, b_key, key_type, "meq.keq");
        self.builder.cond_br(key_eq, check_val_bb, inner_latch);

        // Keys match: compare values
        self.builder.position_at_end(check_val_bb);
        let bv_ptr = self.builder.gep(val_llvm_ty, b_vals, &[j], "meq.bv.ptr");
        let b_val = self.builder.load(val_llvm_ty, bv_ptr, "meq.bv");
        let val_eq = self.emit_inner_eq(a_val, b_val, val_type, "meq.veq");
        // Key found but value differs → definitive mismatch (keys are unique)
        self.builder.cond_br(val_eq, found_bb, false_bb);

        // Inner latch: j++
        self.builder.position_at_end(inner_latch);
        let one = self.builder.const_i64(1);
        let next_j = self.builder.add(j, one, "meq.j.next");
        self.builder.add_phi_incoming(j, &[(next_j, inner_latch)]);
        self.builder.br(inner_hdr);

        // Found: key+value matched, advance outer loop
        self.builder.position_at_end(found_bb);
        let next_i = self.builder.add(i, one, "meq.i.next");
        self.builder.add_phi_incoming(i, &[(next_i, found_bb)]);
        self.builder.br(outer_hdr);

        // All entries matched
        self.builder.position_at_end(true_bb);
        let true_cur = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Mismatch
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
            "meq.result",
        )
    }

    // -----------------------------------------------------------------------
    // Map hash — XOR of hash_combine(key, value) pairs (order-independent)
    // -----------------------------------------------------------------------

    /// Emit `map.hash()` → i64.
    ///
    /// For each entry, compute `hash_combine(key.hash(), val.hash())`, then
    /// XOR all pair hashes together. XOR is order-independent, matching the
    /// evaluator's `hash_value` for `Value::Map`.
    ///
    /// Layout: Map = `{i64 len, i64 cap, ptr keys, ptr vals}`.
    pub(crate) fn emit_map_hash(
        &mut self,
        val: ValueId,
        key_type: Idx,
        val_type: Idx,
    ) -> Option<ValueId> {
        let len = self.builder.extract_value(val, 0, "mhash.len")?;
        let keys = self.builder.extract_value(val, 2, "mhash.keys")?;
        let vals = self.builder.extract_value(val, 3, "mhash.vals")?;

        let key_llvm_ty = self.resolve_type(key_type);
        let val_llvm_ty = self.resolve_type(val_type);

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "mhash.hdr");
        let body_bb = self
            .builder
            .append_block(self.current_function, "mhash.body");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "mhash.exit");

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + hash accumulator phi
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx = self.builder.phi(i64_ty, "mhash.idx");
        self.builder.add_phi_incoming(idx, &[(zero, entry_bb)]);

        let acc = self.builder.phi(i64_ty, "mhash.acc");
        self.builder.add_phi_incoming(acc, &[(zero, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(idx, len, "mhash.inbounds");
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: hash key+value pair, XOR into accumulator
        self.builder.position_at_end(body_bb);
        let k_ptr = self.builder.gep(key_llvm_ty, keys, &[idx], "mhash.k.ptr");
        let k_val = self.builder.load(key_llvm_ty, k_ptr, "mhash.k");
        let k_hash = self.emit_inner_hash(k_val, key_type, "mhash.kh");

        let v_ptr = self.builder.gep(val_llvm_ty, vals, &[idx], "mhash.v.ptr");
        let v_val = self.builder.load(val_llvm_ty, v_ptr, "mhash.v");
        let v_hash = self.emit_inner_hash(v_val, val_type, "mhash.vh");

        let pair_hash = self.emit_hash_combine(k_hash, v_hash, "mhash.pair");
        let new_acc = self.builder.xor(acc, pair_hash, "mhash.xor");

        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx, one, "mhash.next");

        // Use current_block because emit_inner_hash may create intermediate blocks
        let body_end = self.builder.current_block()?;
        self.builder.add_phi_incoming(idx, &[(next_idx, body_end)]);
        self.builder.add_phi_incoming(acc, &[(new_acc, body_end)]);
        self.builder.br(header_bb);

        // Exit: return accumulated hash
        self.builder.position_at_end(exit_bb);
        Some(acc)
    }
}
