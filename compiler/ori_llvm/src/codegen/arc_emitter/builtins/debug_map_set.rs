//! Debug formatting for Map and Set types in LLVM codegen.
//!
//! Map: `{key: value, key2: value2}` — keys use Printable semantics (unquoted
//! strings), values use Debug semantics (quoted strings, recursive formatting).
//! Set: `Set {elem, elem2}` — elements use Debug semantics.
//!
//! Both convert the hash table to temporary contiguous lists via runtime
//! helpers (`ori_map_keys_to_list`, `ori_set_to_list`), iterate to build
//! the formatted string, then dec the temporary list buffers.
//!
//! IMPORTANT: Temporary lists use the map/set's collection-level element
//! sizes (via `collection_elem_size`), NOT the function-level narrowing
//! (via `int_element_llvm_type`). Using the wrong narrowing context causes
//! GEP stride mismatches and value corruption.

use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::value_id::{LLVMTypeId, ValueId};
use crate::codegen::TypeInfo;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `{K: V}.debug()` — format as `{key: value, ...}`.
    ///
    /// Strategy: convert map to key/value lists, iterate both in parallel,
    /// format each `key_to_str + ": " + value_debug`, then dec temporary lists.
    ///
    /// Uses collection-level narrowing (from the map type) for element sizes,
    /// since the temporary lists are created by `emit_map_keys`/`emit_map_values`
    /// which use `collection_elem_size`.
    pub(super) fn emit_map_debug(
        &mut self,
        map: ValueId,
        map_ty: Idx,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let len = self.builder.extract_value(map, FIELD_LEN, "mdbg.len")?;
        let zero = self.builder.const_i64(0);
        let is_empty = self.builder.icmp_eq(len, zero, "mdbg.empty");

        let func = self.current_function;
        let str_ty = self.resolve_type(Idx::STR);

        let empty_bb = self.builder.append_block(func, "mdbg.empty");
        let body_bb = self.builder.append_block(func, "mdbg.body");
        let done_bb = self.builder.append_block(func, "mdbg.done");

        self.builder.cond_br(is_empty, empty_bb, body_bb);

        // Empty: return "{}"
        self.builder.position_at_end(empty_bb);
        let empty_str = self.emit_literal_ori_str("{}")?;
        let empty_bb_end = self.builder.current_block().unwrap();
        self.builder.br(done_bb);

        // Non-empty: format all entries with loop.
        self.builder.position_at_end(body_bb);
        let (result, close_bb_end) =
            self.emit_map_debug_entries(map, map_ty, key_ty, val_ty, str_ty, zero, func)?;
        self.builder.br(done_bb);

        // Done: phi between empty and formatted
        self.builder.position_at_end(done_bb);
        let final_phi = self.builder.phi(str_ty, "mdbg.final");
        self.builder.add_phi_incoming(
            final_phi,
            &[(empty_str, empty_bb_end), (result, close_bb_end)],
        );
        Some(final_phi)
    }

    /// Format all map entries: extract key/value lists, format first entry,
    /// loop remaining, close brace, dec temporary buffers.
    /// Returns `(formatted_string, close_bb_end)` for the phi merge.
    #[expect(
        clippy::too_many_arguments,
        reason = "entry point from emit_map_debug with pre-resolved types and builder state"
    )]
    fn emit_map_debug_entries(
        &mut self,
        map: ValueId,
        map_ty: Idx,
        key_ty: Idx,
        val_ty: Idx,
        str_ty: LLVMTypeId,
        zero: ValueId,
        func: crate::codegen::value_id::FunctionId,
    ) -> Option<(ValueId, crate::codegen::value_id::BlockId)> {
        let key_list = self.emit_map_keys(map, key_ty, map_ty)?;
        let val_list = self.emit_map_values(map, key_ty, val_ty, map_ty)?;

        let key_data = self
            .builder
            .extract_value(key_list, FIELD_DATA, "mdbg.kd")?;
        let val_data = self
            .builder
            .extract_value(val_list, FIELD_DATA, "mdbg.vd")?;
        let entry_count = self.builder.extract_value(key_list, FIELD_LEN, "mdbg.n")?;

        let one = self.builder.const_i64(1);

        let collection_idx = self.pool.resolve_fully(map_ty);
        let key_llvm_ty = self.collection_elem_llvm_type(collection_idx, key_ty);
        let val_llvm_ty = self.collection_elem_llvm_type(collection_idx, val_ty);
        let key_narrowed = self
            .narrowed_collection_element_width(collection_idx)
            .is_some()
            && self.pool.tag(self.pool.resolve_fully(key_ty)) == ori_types::Tag::Int;
        let val_narrowed = self
            .narrowed_collection_element_width(collection_idx)
            .is_some()
            && self.pool.tag(self.pool.resolve_fully(val_ty)) == ori_types::Tag::Int;

        // Format first entry: "{" + key_to_str + ": " + value_debug
        let open = self.emit_literal_ori_str("{")?;
        let acc_init = self.emit_map_entry_str(
            key_data,
            val_data,
            zero,
            key_ty,
            val_ty,
            key_llvm_ty,
            val_llvm_ty,
            key_narrowed,
            val_narrowed,
            open,
        )?;

        let needs_loop = self.builder.icmp_sgt(entry_count, one, "mdbg.more");
        let first_bb_end = self.builder.current_block().unwrap();

        let loop_hdr = self.builder.append_block(func, "mdbg.hdr");
        let loop_body = self.builder.append_block(func, "mdbg.loop");
        let close_bb = self.builder.append_block(func, "mdbg.close");

        self.builder.cond_br(needs_loop, loop_hdr, close_bb);

        // Loop header
        self.builder.position_at_end(loop_hdr);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "mdbg.idx");
        let acc_phi = self.builder.phi(str_ty, "mdbg.acc");
        let has_more = self.builder.icmp_slt(idx_phi, entry_count, "mdbg.cont");
        self.builder.cond_br(has_more, loop_body, close_bb);

        // Loop body: acc = acc + ", " + key_to_str + ": " + value_debug
        self.builder.position_at_end(loop_body);
        let sep = self.emit_literal_ori_str(", ")?;
        let with_sep = self.emit_str_concat(acc_phi, sep)?;
        self.dec_intermediate_str(acc_phi);
        let new_acc = self.emit_map_entry_str(
            key_data,
            val_data,
            idx_phi,
            key_ty,
            val_ty,
            key_llvm_ty,
            val_llvm_ty,
            key_narrowed,
            val_narrowed,
            with_sep,
        )?;
        self.dec_intermediate_str(with_sep);
        let next_idx = self.builder.add(idx_phi, one, "mdbg.next");
        let body_end = self.builder.current_block().unwrap();
        self.builder.br(loop_hdr);

        // Wire loop phis
        self.builder
            .add_phi_incoming(idx_phi, &[(one, first_bb_end), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_bb_end), (new_acc, body_end)]);

        // Close: acc + "}"
        self.builder.position_at_end(close_bb);
        let close_acc = self.builder.phi(str_ty, "mdbg.cl.acc");
        self.builder
            .add_phi_incoming(close_acc, &[(acc_init, first_bb_end), (acc_phi, loop_hdr)]);
        let suffix = self.emit_literal_ori_str("}")?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_bb_end = self.builder.current_block().unwrap();

        // Dec temporary list buffers
        self.dec_temporary_list_with_size(key_list, key_ty, collection_idx);
        self.dec_temporary_list_with_size(val_list, val_ty, collection_idx);

        Some((result, close_bb_end))
    }

    /// Format a single map entry: `prefix + key_to_str + ": " + value_debug`.
    ///
    /// Keys use Printable semantics (strings unquoted), values use Debug.
    /// Returns the concatenated string; caller manages intermediate string RC.
    #[expect(
        clippy::too_many_arguments,
        reason = "carries pre-computed LLVM types and narrowing flags through the loop to avoid recomputing per-iteration"
    )]
    fn emit_map_entry_str(
        &mut self,
        key_data: ValueId,
        val_data: ValueId,
        idx: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        key_llvm_ty: LLVMTypeId,
        val_llvm_ty: LLVMTypeId,
        key_narrowed: bool,
        val_narrowed: bool,
        prefix: ValueId,
    ) -> Option<ValueId> {
        let key_ptr = self.builder.gep(key_llvm_ty, key_data, &[idx], "mdbg.kp");
        let key = self.builder.load(key_llvm_ty, key_ptr, "mdbg.k");
        let key = if key_narrowed {
            self.sext_narrowed_int_element(key, key_ty, "mdbg.k.sext")
        } else {
            key
        };
        let val_ptr = self.builder.gep(val_llvm_ty, val_data, &[idx], "mdbg.vp");
        let val = self.builder.load(val_llvm_ty, val_ptr, "mdbg.v");
        let val = if val_narrowed {
            self.sext_narrowed_int_element(val, val_ty, "mdbg.v.sext")
        } else {
            val
        };

        // Keys: Printable (to_str) with Debug fallback for complex key types
        let raw_key_str = self
            .emit_element_to_str(key, key_ty)
            .or_else(|| self.emit_element_debug(key, key_ty))?;
        // Escape control characters in key strings for readable debug output.
        // Matches the interpreter's `escape_debug_str()` behavior in map debug.
        let key_str = self.emit_escape_control(raw_key_str)?;
        // Only dec raw_key_str when it was freshly allocated by emit_to_str.
        // For Str keys, emit_element_to_str returns the original borrowed value
        // (not a new allocation), so decrementing would double-free.
        let key_type_info = self.type_info.get(key_ty);
        if !matches!(key_type_info, TypeInfo::Str) {
            self.dec_intermediate_str(raw_key_str);
        }
        let colon = self.emit_literal_ori_str(": ")?;
        // Values: Debug semantics
        let val_str = self.emit_element_debug(val, val_ty)?;

        let tmp1 = self.emit_str_concat(prefix, key_str)?;
        let tmp2 = self.emit_str_concat(tmp1, colon)?;
        self.dec_intermediate_str(tmp1);
        let result = self.emit_str_concat(tmp2, val_str)?;
        self.dec_intermediate_str(tmp2);
        self.dec_intermediate_str(key_str);
        self.dec_intermediate_str(val_str);

        Some(result)
    }

    /// Emit `Set<T>.debug()` — format as `Set {elem, elem2, ...}`.
    ///
    /// Strategy: convert set to list via `ori_set_to_list`, iterate elements,
    /// format each with Debug semantics, then dec temporary list buffer.
    pub(super) fn emit_set_debug(&mut self, set: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let len = self.builder.extract_value(set, FIELD_LEN, "sdbg.len")?;
        let zero = self.builder.const_i64(0);
        let is_empty = self.builder.icmp_eq(len, zero, "sdbg.empty");

        let func = self.current_function;
        let str_ty = self.resolve_type(Idx::STR);

        let empty_bb = self.builder.append_block(func, "sdbg.empty");
        let body_bb = self.builder.append_block(func, "sdbg.body");
        let done_bb = self.builder.append_block(func, "sdbg.done");

        self.builder.cond_br(is_empty, empty_bb, body_bb);

        // Empty: "Set {}"
        self.builder.position_at_end(empty_bb);
        let empty_str = self.emit_literal_ori_str("Set {}")?;
        let empty_bb_end = self.builder.current_block().unwrap();
        self.builder.br(done_bb);

        // Non-empty: convert to list.
        // emit_set_to_list uses canonical element sizes, so use resolve_type.
        self.builder.position_at_end(body_bb);
        let elem_list = self.emit_set_to_list(set, elem_ty)?;
        let data = self
            .builder
            .extract_value(elem_list, FIELD_DATA, "sdbg.data")?;
        let entry_count = self.builder.extract_value(elem_list, FIELD_LEN, "sdbg.n")?;

        let one = self.builder.const_i64(1);
        // Use canonical types — set_to_list uses element_store_size, not narrowed.
        let elem_llvm_ty = self.resolve_type(elem_ty);

        // Format first element: "Set {" + elem_debug
        let open = self.emit_literal_ori_str("Set {")?;
        let ptr0 = self.builder.gep(elem_llvm_ty, data, &[zero], "sdbg.ep0");
        let elem0 = self.builder.load(elem_llvm_ty, ptr0, "sdbg.e0");
        let elem0_str = self.emit_element_debug(elem0, elem_ty)?;
        let acc_init = self.emit_str_concat(open, elem0_str)?;
        self.dec_intermediate_str(elem0_str);

        let needs_loop = self.builder.icmp_sgt(entry_count, one, "sdbg.more");
        let first_bb_end = self.builder.current_block().unwrap();

        let loop_hdr = self.builder.append_block(func, "sdbg.hdr");
        let loop_body = self.builder.append_block(func, "sdbg.loop");
        let close_bb = self.builder.append_block(func, "sdbg.close");

        self.builder.cond_br(needs_loop, loop_hdr, close_bb);

        // Loop header
        self.builder.position_at_end(loop_hdr);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "sdbg.idx");
        let acc_phi = self.builder.phi(str_ty, "sdbg.acc");
        let has_more = self.builder.icmp_slt(idx_phi, entry_count, "sdbg.cont");
        self.builder.cond_br(has_more, loop_body, close_bb);

        // Loop body
        self.builder.position_at_end(loop_body);
        let sep = self.emit_literal_ori_str(", ")?;
        let with_sep = self.emit_str_concat(acc_phi, sep)?;
        self.dec_intermediate_str(acc_phi);
        let ptr_i = self.builder.gep(elem_llvm_ty, data, &[idx_phi], "sdbg.epi");
        let elem_i = self.builder.load(elem_llvm_ty, ptr_i, "sdbg.ei");
        let elem_i_str = self.emit_element_debug(elem_i, elem_ty)?;
        let new_acc = self.emit_str_concat(with_sep, elem_i_str)?;
        self.dec_intermediate_str(with_sep);
        self.dec_intermediate_str(elem_i_str);
        let next_idx = self.builder.add(idx_phi, one, "sdbg.next");
        let body_end = self.builder.current_block().unwrap();
        self.builder.br(loop_hdr);

        // Wire phis
        self.builder
            .add_phi_incoming(idx_phi, &[(one, first_bb_end), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_bb_end), (new_acc, body_end)]);

        // Close: acc + "}"
        self.builder.position_at_end(close_bb);
        let close_acc = self.builder.phi(str_ty, "sdbg.cl.acc");
        self.builder
            .add_phi_incoming(close_acc, &[(acc_init, first_bb_end), (acc_phi, loop_hdr)]);
        let suffix = self.emit_literal_ori_str("}")?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_bb_end = self.builder.current_block().unwrap();

        // Dec temporary list buffer
        self.dec_temporary_list_canonical(elem_list, elem_ty);

        self.builder.br(done_bb);

        // Done: phi
        self.builder.position_at_end(done_bb);
        let final_phi = self.builder.phi(str_ty, "sdbg.final");
        self.builder.add_phi_incoming(
            final_phi,
            &[(empty_str, empty_bb_end), (result, close_bb_end)],
        );
        Some(final_phi)
    }

    /// Dec a temporary list buffer using collection-level element sizes.
    ///
    /// Used for map key/value lists where element sizes come from the map's
    /// `collection_elem_size`, not the canonical `element_store_size`.
    fn dec_temporary_list_with_size(&mut self, list: ValueId, elem_ty: Idx, collection_idx: Idx) {
        let Some(data) = self.builder.extract_value(list, FIELD_DATA, "dec.data") else {
            return;
        };
        let Some(len) = self.builder.extract_value(list, FIELD_LEN, "dec.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(list, FIELD_CAP, "dec.cap") else {
            return;
        };
        let elem_size = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
        self.emit_rt_call(func_id, &[data, len, cap, elem_size, elem_dec_fn], "");
    }

    /// Dec a temporary list buffer using canonical element sizes.
    ///
    /// Used for set-to-list conversions where `emit_set_to_list` uses
    /// `element_store_size` (canonical, not narrowed).
    fn dec_temporary_list_canonical(&mut self, list: ValueId, elem_ty: Idx) {
        let Some(data) = self.builder.extract_value(list, FIELD_DATA, "dec.data") else {
            return;
        };
        let Some(len) = self.builder.extract_value(list, FIELD_LEN, "dec.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(list, FIELD_CAP, "dec.cap") else {
            return;
        };
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
        self.emit_rt_call(func_id, &[data, len, cap, elem_size, elem_dec_fn], "");
    }
}
