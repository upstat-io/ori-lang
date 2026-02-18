//! Loop-based method codegen for collection types (list, map, set).
//!
//! Unlike tuple/option/result methods which unroll at compile-time,
//! collection methods require runtime loops with phi-merged accumulators
//! because element count is dynamic. Extracted from `lower_builtin_methods.rs`
//! to keep files under 500 lines.
//!
//! # Supported operations
//!
//! - **List**: `compare`, `hash`, `equals`
//! - **Set**: `equals`, `hash`
//! - **Map**: `equals`, `hash`

use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::value_id::ValueId;

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
