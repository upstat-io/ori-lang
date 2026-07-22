//! Map and Set formatting contracts for LLVM emission.
//!
//! Maps render as `{key: value}` and Sets as `Set {element}`. Printable map
//! keys remain unquoted; Debug values and Set elements use recursive formatting.
//! Runtime helpers expose temporary contiguous lists for iteration. Their
//! element sizing and widening use the source collection's representation;
//! substituting a canonical element layout corrupts GEP strides.

use super::{super::ArcIrEmitter, RenderStyle};
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};
use crate::codegen::TypeInfo;
use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

#[derive(Clone, Copy)]
struct MapDebugContext {
    map_ty: Idx,
    key_ty: Idx,
    val_ty: Idx,
    str_ty: LLVMTypeId,
    function: FunctionId,
    style: RenderStyle,
}

#[derive(Clone, Copy)]
struct MapEntryLayout {
    key_ty: Idx,
    val_ty: Idx,
    collection_idx: Idx,
    key_llvm_ty: LLVMTypeId,
    val_llvm_ty: LLVMTypeId,
    key_narrowed: bool,
    val_narrowed: bool,
    style: RenderStyle,
}

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
        style: RenderStyle,
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

        self.builder.position_at_end(empty_bb);
        let empty_str = self.emit_literal_ori_str("{}")?;
        let empty_bb_end = self.builder.current_block()?;
        self.builder.br(done_bb);

        self.builder.position_at_end(body_bb);
        let context = MapDebugContext {
            map_ty,
            key_ty,
            val_ty,
            str_ty,
            function: func,
            style,
        };
        let (result, close_bb_end) = self.emit_map_debug_entries(map, zero, context)?;
        self.builder.br(done_bb);

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
    fn emit_map_debug_entries(
        &mut self,
        map: ValueId,
        zero: ValueId,
        context: MapDebugContext,
    ) -> Option<(ValueId, BlockId)> {
        let key_list = self.emit_map_keys(map, context.key_ty, context.map_ty)?;
        let val_list = self.emit_map_values(map, context.key_ty, context.val_ty, context.map_ty)?;

        let key_data = self
            .builder
            .extract_value(key_list, FIELD_DATA, "mdbg.kd")?;

        let val_data = self
            .builder
            .extract_value(val_list, FIELD_DATA, "mdbg.vd")?;
        let entry_count = self.builder.extract_value(key_list, FIELD_LEN, "mdbg.n")?;

        let one = self.builder.const_i64(1);

        let collection_idx = self.pool.resolve_fully(context.map_ty);
        let key_llvm_ty = self.collection_elem_llvm_type(collection_idx, context.key_ty);
        let val_llvm_ty = self.collection_elem_llvm_type(collection_idx, context.val_ty);
        let key_narrowed = self
            .narrowed_collection_element_width(collection_idx)
            .is_some()
            && self.pool.tag(self.pool.resolve_fully(context.key_ty)) == ori_types::Tag::Int;

        let val_narrowed = self
            .narrowed_collection_element_width(collection_idx)
            .is_some()
            && self.pool.tag(self.pool.resolve_fully(context.val_ty)) == ori_types::Tag::Int;

        let layout = MapEntryLayout {
            key_ty: context.key_ty,
            val_ty: context.val_ty,
            collection_idx,
            key_llvm_ty,
            val_llvm_ty,
            key_narrowed,
            val_narrowed,
            style: context.style,
        };

        let open = self.emit_literal_ori_str("{")?;
        let acc_init = self.emit_map_entry_str(key_data, val_data, zero, layout, open)?;

        let needs_loop = self.builder.icmp_sgt(entry_count, one, "mdbg.more");
        let first_bb_end = self.builder.current_block()?;

        let loop_hdr = self.builder.append_block(context.function, "mdbg.hdr");
        let loop_body = self.builder.append_block(context.function, "mdbg.loop");
        let close_bb = self.builder.append_block(context.function, "mdbg.close");

        self.builder.cond_br(needs_loop, loop_hdr, close_bb);

        self.builder.position_at_end(loop_hdr);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "mdbg.idx");
        let acc_phi = self.builder.phi(context.str_ty, "mdbg.acc");
        let has_more = self.builder.icmp_slt(idx_phi, entry_count, "mdbg.cont");
        self.builder.cond_br(has_more, loop_body, close_bb);

        self.builder.position_at_end(loop_body);
        let sep = self.emit_literal_ori_str(", ")?;
        let with_sep = self.emit_str_concat(acc_phi, sep)?;
        self.dec_intermediate_str(acc_phi);
        let new_acc = self.emit_map_entry_str(key_data, val_data, idx_phi, layout, with_sep)?;
        self.dec_intermediate_str(with_sep);
        let next_idx = self.builder.add(idx_phi, one, "mdbg.next");
        let body_end = self.builder.current_block()?;
        self.builder.br(loop_hdr);

        self.builder
            .add_phi_incoming(idx_phi, &[(one, first_bb_end), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_bb_end), (new_acc, body_end)]);

        self.builder.position_at_end(close_bb);
        let close_acc = self.builder.phi(context.str_ty, "mdbg.cl.acc");
        self.builder
            .add_phi_incoming(close_acc, &[(acc_init, first_bb_end), (acc_phi, loop_hdr)]);
        let suffix = self.emit_literal_ori_str("}")?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_bb_end = self.builder.current_block()?;

        self.dec_temporary_list_with_size(key_list, context.key_ty, collection_idx);
        self.dec_temporary_list_with_size(val_list, context.val_ty, collection_idx);

        Some((result, close_bb_end))
    }

    /// Format a single map entry: `prefix + key_to_str + ": " + value_debug`.
    ///
    /// Keys use Printable semantics (strings unquoted), values use Debug.
    /// Returns the concatenated string; caller manages intermediate string RC.
    fn emit_map_entry_str(
        &mut self,
        key_data: ValueId,
        val_data: ValueId,
        idx: ValueId,
        layout: MapEntryLayout,
        prefix: ValueId,
    ) -> Option<ValueId> {
        let key_ptr = self
            .builder
            .gep(layout.key_llvm_ty, key_data, &[idx], "mdbg.kp");
        let key = self.builder.load(layout.key_llvm_ty, key_ptr, "mdbg.k");
        let key = if layout.key_narrowed {
            self.sext_narrowed_int_element(key, layout.collection_idx, layout.key_ty, "mdbg.k.sext")
        } else {
            key
        };
        let val_ptr = self
            .builder
            .gep(layout.val_llvm_ty, val_data, &[idx], "mdbg.vp");
        let val = self.builder.load(layout.val_llvm_ty, val_ptr, "mdbg.v");
        let val = if layout.val_narrowed {
            self.sext_narrowed_int_element(val, layout.collection_idx, layout.val_ty, "mdbg.v.sext")
        } else {
            val
        };

        let key_str = if layout.style.is_debug() {
            self.emit_element_debug(key, layout.key_ty)?
        } else {
            let raw_key_str = self.emit_element_to_str(key, layout.key_ty)?;
            let escaped = self.emit_escape_control(raw_key_str)?;
            if !matches!(self.type_info.get(layout.key_ty), TypeInfo::Str) {
                self.dec_intermediate_str(raw_key_str);
            }
            escaped
        };
        let colon = self.emit_literal_ori_str(": ")?;
        let val_str = if layout.style.is_debug() {
            self.emit_element_debug(val, layout.val_ty)?
        } else {
            self.emit_element_to_str(val, layout.val_ty)?
        };
        let val_is_borrowed_str = layout.style == RenderStyle::Printable
            && matches!(self.type_info.get(layout.val_ty), TypeInfo::Str);

        let tmp1 = self.emit_str_concat(prefix, key_str)?;
        let tmp2 = self.emit_str_concat(tmp1, colon)?;
        self.dec_intermediate_str(tmp1);
        let result = self.emit_str_concat(tmp2, val_str)?;
        self.dec_intermediate_str(tmp2);
        self.dec_intermediate_str(key_str);
        if !val_is_borrowed_str {
            self.dec_intermediate_str(val_str);
        }

        Some(result)
    }

    /// Emit `Set<T>.debug()` — format as `Set {elem, elem2, ...}`.
    ///
    /// Strategy: convert set to list via `ori_set_to_list`, iterate elements,
    /// format each with Debug semantics, then dec temporary list buffer.
    pub(super) fn emit_set_debug(
        &mut self,
        set: ValueId,
        elem_ty: Idx,
        style: RenderStyle,
    ) -> Option<ValueId> {
        // Why: Decrementing a borrowed Printable string would double-free its source.
        let elem_is_borrowed_str =
            style == RenderStyle::Printable && matches!(self.type_info.get(elem_ty), TypeInfo::Str);
        let len = self.builder.extract_value(set, FIELD_LEN, "sdbg.len")?;
        let zero = self.builder.const_i64(0);
        let is_empty = self.builder.icmp_eq(len, zero, "sdbg.empty");

        let func = self.current_function;
        let str_ty = self.resolve_type(Idx::STR);

        let empty_bb = self.builder.append_block(func, "sdbg.empty");
        let body_bb = self.builder.append_block(func, "sdbg.body");
        let done_bb = self.builder.append_block(func, "sdbg.done");

        self.builder.cond_br(is_empty, empty_bb, body_bb);

        self.builder.position_at_end(empty_bb);
        let empty_str = self.emit_literal_ori_str("Set {}")?;
        let empty_bb_end = self.builder.current_block()?;
        self.builder.br(done_bb);

        self.builder.position_at_end(body_bb);
        let elem_list = self.emit_set_to_list(set, elem_ty)?;
        let data = self
            .builder
            .extract_value(elem_list, FIELD_DATA, "sdbg.data")?;
        let entry_count = self.builder.extract_value(elem_list, FIELD_LEN, "sdbg.n")?;

        let one = self.builder.const_i64(1);
        let elem_llvm_ty = self.resolve_type(elem_ty);

        let open = self.emit_literal_ori_str("Set {")?;
        let ptr0 = self.builder.gep(elem_llvm_ty, data, &[zero], "sdbg.ep0");
        let elem0 = self.builder.load(elem_llvm_ty, ptr0, "sdbg.e0");
        let elem0_str = if style.is_debug() {
            self.emit_element_debug(elem0, elem_ty)?
        } else {
            self.emit_element_to_str(elem0, elem_ty)?
        };
        let acc_init = self.emit_str_concat(open, elem0_str)?;
        if !elem_is_borrowed_str {
            self.dec_intermediate_str(elem0_str);
        }

        let needs_loop = self.builder.icmp_sgt(entry_count, one, "sdbg.more");
        let first_bb_end = self.builder.current_block()?;

        let loop_hdr = self.builder.append_block(func, "sdbg.hdr");
        let loop_body = self.builder.append_block(func, "sdbg.loop");
        let close_bb = self.builder.append_block(func, "sdbg.close");

        self.builder.cond_br(needs_loop, loop_hdr, close_bb);

        self.builder.position_at_end(loop_hdr);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "sdbg.idx");
        let acc_phi = self.builder.phi(str_ty, "sdbg.acc");
        let has_more = self.builder.icmp_slt(idx_phi, entry_count, "sdbg.cont");
        self.builder.cond_br(has_more, loop_body, close_bb);

        self.builder.position_at_end(loop_body);
        let sep = self.emit_literal_ori_str(", ")?;
        let with_sep = self.emit_str_concat(acc_phi, sep)?;
        self.dec_intermediate_str(acc_phi);
        let ptr_i = self.builder.gep(elem_llvm_ty, data, &[idx_phi], "sdbg.epi");
        let elem_i = self.builder.load(elem_llvm_ty, ptr_i, "sdbg.ei");
        let elem_i_str = if style.is_debug() {
            self.emit_element_debug(elem_i, elem_ty)?
        } else {
            self.emit_element_to_str(elem_i, elem_ty)?
        };
        let new_acc = self.emit_str_concat(with_sep, elem_i_str)?;
        self.dec_intermediate_str(with_sep);
        if !elem_is_borrowed_str {
            self.dec_intermediate_str(elem_i_str);
        }
        let next_idx = self.builder.add(idx_phi, one, "sdbg.next");
        let body_end = self.builder.current_block()?;
        self.builder.br(loop_hdr);

        self.builder
            .add_phi_incoming(idx_phi, &[(one, first_bb_end), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_bb_end), (new_acc, body_end)]);

        self.builder.position_at_end(close_bb);
        let close_acc = self.builder.phi(str_ty, "sdbg.cl.acc");
        self.builder
            .add_phi_incoming(close_acc, &[(acc_init, first_bb_end), (acc_phi, loop_hdr)]);
        let suffix = self.emit_literal_ori_str("}")?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_bb_end = self.builder.current_block()?;

        self.dec_temporary_list_canonical(elem_list, elem_ty);

        self.builder.br(done_bb);

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
        let elem_size = self.collection_elem_size(collection_idx, elem_ty);
        self.dec_temporary_list(list, elem_ty, elem_size);
    }

    /// Dec a temporary list buffer using canonical element sizes.
    ///
    /// Used for set-to-list conversions where `emit_set_to_list` uses
    /// `element_store_size` (canonical, not narrowed).
    fn dec_temporary_list_canonical(&mut self, list: ValueId, elem_ty: Idx) {
        let elem_size = self.element_store_size(elem_ty);
        self.dec_temporary_list(list, elem_ty, elem_size);
    }

    fn dec_temporary_list(&mut self, list: ValueId, elem_ty: Idx, elem_size: u64) {
        let (Some(data), Some(len), Some(cap)) = (
            self.builder.extract_value(list, FIELD_DATA, "dec.data"),
            self.builder.extract_value(list, FIELD_LEN, "dec.len"),
            self.builder.extract_value(list, FIELD_CAP, "dec.cap"),
        ) else {
            // Why: The temporary list value always uses the runtime fat-list layout.
            unreachable!("temporary list value must contain data, length, and capacity fields");
        };
        let Ok(elem_size) = i64::try_from(elem_size) else {
            // Why: LLVM element layouts always fit the runtime list ABI's i64 size field.
            unreachable!("temporary list element size must fit the runtime i64 ABI field");
        };
        let elem_size = self.builder.const_i64(elem_size);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
        self.emit_rt_call(func_id, &[data, len, cap, elem_size, elem_dec_fn], "");
    }
}
