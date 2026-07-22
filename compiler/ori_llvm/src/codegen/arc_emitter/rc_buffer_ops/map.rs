//! Map buffer reference-count operations and unwind-safe element teardown.

use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};

use super::super::ArcIrEmitter;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

struct MapDropState {
    data: ValueId,
    cap: ValueId,
    key_size: ValueId,
    value_size: ValueId,
    key_drop: DropChannel,
    value_drop: DropChannel,
    function: FunctionId,
    i64_type: LLVMTypeId,
    i8_type: LLVMTypeId,
    one: ValueId,
    zero8: ValueId,
    one8: ValueId,
    align8: ValueId,
    personality: FunctionId,
    occupied_fn: FunctionId,
    free_fn: FunctionId,
    cleanup_enter_fn: FunctionId,
    cleanup_exit_fn: FunctionId,
    keys_offset: ValueId,
    values_offset: ValueId,
    total_size: ValueId,
    index: ValueId,
    pending_key: ValueId,
    header: BlockId,
    body: BlockId,
    live: BlockId,
    continuation: BlockId,
    cleanup: BlockId,
    done: BlockId,
}

#[derive(Clone, Copy)]
enum DropChannel {
    None,
    Plain(FunctionId),
    MayUnwind(FunctionId),
}

impl DropChannel {
    fn from_facts(function: Option<FunctionId>, may_unwind: bool) -> Self {
        match (function, may_unwind) {
            (None, false) => Self::None,
            (Some(function), false) => Self::Plain(function),
            (Some(function), true) => Self::MayUnwind(function),
            (None, true) => {
                unreachable!("a may-unwind drop channel must have a generated drop function")
            }
        }
    }

    const fn may_unwind(self) -> bool {
        matches!(self, Self::MayUnwind(_))
    }

    const fn function(self) -> Option<FunctionId> {
        match self {
            Self::None => None,
            Self::Plain(function) | Self::MayUnwind(function) => Some(function),
        }
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `ori_map_buffer_rc_dec` for a map value.
    ///
    /// Maps use an open-addressing hash table `[metadata | keys | values]` with
    /// 1-byte metadata per bucket, one RC header. The runtime function handles
    /// cleaning up both key and value children at their respective offsets.
    pub(in crate::codegen::arc_emitter) fn emit_buffer_rc_dec_map(
        &mut self,
        val: ValueId,
        resolved: ori_types::Idx,
    ) {
        let Some(len) = self.builder.extract_value(val, FIELD_LEN, "rc.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, FIELD_CAP, "rc.cap") else {
            return;
        };
        let Some(data) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") else {
            return;
        };

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        let key_size = self.collection_elem_size(resolved, key_type);
        let val_size = self.collection_elem_size(resolved, val_type);

        let key_may_unwind = self.drop_may_unwind(key_type);
        let val_may_unwind = self.drop_may_unwind(val_type);
        if (key_may_unwind || val_may_unwind)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium
        {
            // Itanium uses the codegen cleanup loop because runtime catch_unwind
            // aborts on foreign Ori exceptions. The dec-to-zero gate handles
            // nested maps whose field/element teardown is an rc_dec.
            let _ = self.get_or_generate_elem_dec_fn(key_type);
            let _ = self.get_or_generate_elem_dec_fn(val_type);
            let key_dec_fid = self.elem_dec_fn_cache.get(&key_type).copied();
            let val_dec_fid = self.elem_dec_fn_cache.get(&val_type).copied();
            let key_drop = DropChannel::from_facts(key_dec_fid, key_may_unwind);
            let value_drop = DropChannel::from_facts(val_dec_fid, val_may_unwind);
            let cur = self.current_function;
            let dec_fn = self.builder.runtime_fn("ori_rc_dec_to_zero");
            let hit = self
                .builder
                .call(dec_fn, &[data], "mdec.hz")
                .expect("ori_rc_dec_to_zero returns i8");
            let zero8 = self.builder.const_i8(0);
            let is_zero = self.builder.icmp_ne(hit, zero8, "mdec.zero");
            let cleanup_bb = self.builder.append_block(cur, "mdec.cleanup");
            let after_bb = self.builder.append_block(cur, "mdec.after");
            self.builder.cond_br(is_zero, cleanup_bb, after_bb);
            self.builder.position_at_end(cleanup_bb);
            self.emit_codegen_buffer_drop_map_unwind(
                data, cap, key_size, val_size, key_drop, value_drop,
            );
            // emit_codegen_buffer_drop_map_unwind leaves the builder at its
            // post-free `done` block; rejoin the non-zero path.
            self.builder.br(after_bb);
            self.builder.position_at_end(after_bb);
            return;
        }

        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_type);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_type);

        let func_id = self.builder.runtime_fn("ori_map_buffer_rc_dec");
        self.emit_rt_call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                val_size_val,
                key_dec_fn,
                val_dec_fn,
            ],
            "",
        );
    }

    /// Emit an Itanium two-channel drop loop with cleanup pads for a map.
    ///
    /// Each occupied bucket drops its value before its key. A panic drains any
    /// pending key and subsequent occupied buckets, frees the buffer, and resumes
    /// the exception. Scalar channels are omitted. The builder finishes at the
    /// post-free continuation block.
    fn emit_codegen_buffer_drop_map_unwind(
        &mut self,
        data: ValueId,
        cap: ValueId,
        key_size: u64,
        val_size: u64,
        key_drop: DropChannel,
        value_drop: DropChannel,
    ) {
        let cur = self.current_function;
        let i64_ty = self.builder.i64_type();
        let i8_ty = self.builder.i8_type();
        let one = self.builder.const_i64(1);
        let zero = self.builder.const_i64(0);
        let zero8 = self.builder.const_i8(0);
        let one8 = self.builder.const_i8(1);
        let align8 = self.builder.const_i64(8);
        let key_size_v = self.builder.const_i64(key_size as i64);
        let val_size_v = self.builder.const_i64(val_size as i64);
        let personality = self.builder.runtime_fn("ori_eh_personality");
        self.builder.set_personality(cur, personality);

        let keys_off_fn = self.builder.runtime_fn("ori_map_keys_offset");
        let vals_off_fn = self.builder.runtime_fn("ori_map_vals_offset");
        let total_fn = self.builder.runtime_fn("ori_map_total_size");
        let occ_fn = self.builder.runtime_fn("ori_map_bucket_occupied");
        let free_fn = self.builder.runtime_fn("ori_rc_free");
        let cl_enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
        let cl_exit = self.builder.runtime_fn("ori_drop_cleanup_exit");

        let keys_off = self
            .builder
            .call(keys_off_fn, &[cap, key_size_v, val_size_v], "mdrop.koff")
            .expect("ori_map_keys_offset returns i64");
        let vals_off = self
            .builder
            .call(vals_off_fn, &[cap, key_size_v, val_size_v], "mdrop.voff")
            .expect("ori_map_vals_offset returns i64");
        let total = self
            .builder
            .call(total_fn, &[cap, key_size_v, val_size_v], "mdrop.total")
            .expect("ori_map_total_size returns i64");

        // Loop index + pending-key flag in allocas so the cleanup pad can read
        // them to drain the remaining (post-panic) buckets.
        let idx = self.builder.create_entry_alloca(cur, "mdrop.i", i64_ty);
        self.builder.store(zero, idx);
        let pk = self.builder.create_entry_alloca(cur, "mdrop.pk", i8_ty);
        self.builder.store(zero8, pk);

        let hdr = self.builder.append_block(cur, "mdrop.hdr");
        let body = self.builder.append_block(cur, "mdrop.body");
        let live = self.builder.append_block(cur, "mdrop.live");
        let cont = self.builder.append_block(cur, "mdrop.cont");
        let cleanup = self.builder.append_block(cur, "mdrop.cleanup");
        let done = self.builder.append_block(cur, "mdrop.done");

        let state = MapDropState {
            data,
            cap,
            key_size: key_size_v,
            value_size: val_size_v,
            key_drop,
            value_drop,
            function: cur,
            i64_type: i64_ty,
            i8_type: i8_ty,
            one,
            zero8,
            one8,
            align8,
            personality,
            occupied_fn: occ_fn,
            free_fn,
            cleanup_enter_fn: cl_enter,
            cleanup_exit_fn: cl_exit,
            keys_offset: keys_off,
            values_offset: vals_off,
            total_size: total,
            index: idx,
            pending_key: pk,
            header: hdr,
            body,
            live,
            continuation: cont,
            cleanup,
            done,
        };

        self.emit_map_drop_primary_loop(&state);

        self.emit_map_drop_cleanup(&state);

        self.builder.position_at_end(done);
        self.builder.call(free_fn, &[data, total, align8], "");
    }

    fn emit_map_drop_primary_loop(&mut self, state: &MapDropState) {
        self.builder.br(state.header);

        self.builder.position_at_end(state.header);
        let index = self.builder.load(state.i64_type, state.index, "mdrop.i.v");
        let finished = self.builder.icmp_sge(index, state.cap, "mdrop.ge");
        self.builder.cond_br(finished, state.done, state.body);

        self.builder.position_at_end(state.body);
        let index = self.builder.load(state.i64_type, state.index, "mdrop.i.b");
        let occupied = self
            .builder
            .call(state.occupied_fn, &[state.data, index], "mdrop.occ")
            .expect("ori_map_bucket_occupied returns i8");
        let is_occupied = self.builder.icmp_ne(occupied, state.zero8, "mdrop.is_occ");
        self.builder
            .cond_br(is_occupied, state.live, state.continuation);

        self.builder.position_at_end(state.live);
        if let Some(value_dec) = state.value_drop.function() {
            let index = self.builder.load(state.i64_type, state.index, "mdrop.iv");
            let scaled = self.builder.mul(index, state.value_size, "mdrop.vmul");
            let offset = self.builder.add(state.values_offset, scaled, "mdrop.vadd");
            let value_ptr = self
                .builder
                .gep(state.i8_type, state.data, &[offset], "mdrop.vptr");
            if state.value_drop.may_unwind() {
                self.builder.store(state.one8, state.pending_key);
                let after_value = self.builder.append_block(state.function, "mdrop.afterval");
                self.builder
                    .invoke(value_dec, &[value_ptr], after_value, state.cleanup, "");
                self.builder.position_at_end(after_value);
            } else {
                self.builder.call(value_dec, &[value_ptr], "");
            }
        }

        if let Some(key_dec) = state.key_drop.function() {
            let index = self.builder.load(state.i64_type, state.index, "mdrop.ik");
            let scaled = self.builder.mul(index, state.key_size, "mdrop.kmul");
            let offset = self.builder.add(state.keys_offset, scaled, "mdrop.kadd");
            let key_ptr = self
                .builder
                .gep(state.i8_type, state.data, &[offset], "mdrop.kptr");
            if state.key_drop.may_unwind() {
                self.builder.store(state.zero8, state.pending_key);
                self.builder
                    .invoke(key_dec, &[key_ptr], state.continuation, state.cleanup, "");
            } else {
                self.builder.call(key_dec, &[key_ptr], "");
                self.builder.br(state.continuation);
            }
        } else {
            self.builder.br(state.continuation);
        }

        self.builder.position_at_end(state.continuation);
        let index = self.builder.load(state.i64_type, state.index, "mdrop.i2");
        let next = self.builder.add(index, state.one, "mdrop.inc");
        self.builder.store(next, state.index);
        self.builder.br(state.header);
    }

    fn emit_map_drop_cleanup(&mut self, state: &MapDropState) {
        self.builder.position_at_end(state.cleanup);
        let landing_pad = self.builder.landingpad(state.personality, true, "mdrop.lp");
        self.builder.call(state.cleanup_enter_fn, &[], "");
        let pending_key = self
            .builder
            .load(state.i8_type, state.pending_key, "mdrop.pk.v");
        let key_pending = self
            .builder
            .icmp_ne(pending_key, state.zero8, "mdrop.pk.set");
        let clean_key = self.builder.append_block(state.function, "mdrop.cl.key");
        let advance = self.builder.append_block(state.function, "mdrop.cl.adv");
        self.builder.cond_br(key_pending, clean_key, advance);

        self.builder.position_at_end(clean_key);
        if let Some(key_dec) = state.key_drop.function() {
            let index = self
                .builder
                .load(state.i64_type, state.index, "mdrop.cl.ci");
            let scaled = self.builder.mul(index, state.key_size, "mdrop.cl.kmul");
            let offset = self.builder.add(state.keys_offset, scaled, "mdrop.cl.kadd");
            let key_ptr = self
                .builder
                .gep(state.i8_type, state.data, &[offset], "mdrop.cl.kptr");
            self.builder.call(key_dec, &[key_ptr], "");
        }
        self.builder.br(advance);

        self.builder.position_at_end(advance);
        let index = self
            .builder
            .load(state.i64_type, state.index, "mdrop.cl.ci2");
        let next = self.builder.add(index, state.one, "mdrop.cl.cnext");
        self.builder.store(next, state.index);
        let drain_header = self.builder.append_block(state.function, "mdrop.drain.hdr");
        let drain_body = self
            .builder
            .append_block(state.function, "mdrop.drain.body");
        let drain_live = self
            .builder
            .append_block(state.function, "mdrop.drain.live");
        let drain_next = self
            .builder
            .append_block(state.function, "mdrop.drain.next");
        let drain_free = self
            .builder
            .append_block(state.function, "mdrop.drain.free");
        self.builder.br(drain_header);

        self.builder.position_at_end(drain_header);
        let index = self.builder.load(state.i64_type, state.index, "mdrop.di");
        let finished = self.builder.icmp_sge(index, state.cap, "mdrop.dge");
        self.builder.cond_br(finished, drain_free, drain_body);

        self.builder.position_at_end(drain_body);
        let index = self.builder.load(state.i64_type, state.index, "mdrop.di.b");
        let occupied = self
            .builder
            .call(state.occupied_fn, &[state.data, index], "mdrop.docc")
            .expect("ori_map_bucket_occupied returns i8");
        let is_occupied = self.builder.icmp_ne(occupied, state.zero8, "mdrop.dis");
        self.builder.cond_br(is_occupied, drain_live, drain_next);

        self.builder.position_at_end(drain_live);
        self.emit_map_drop_drain_bucket(state);
        self.builder.br(drain_next);

        self.builder.position_at_end(drain_next);
        let index = self.builder.load(state.i64_type, state.index, "mdrop.din");
        let next = self.builder.add(index, state.one, "mdrop.dinc");
        self.builder.store(next, state.index);
        self.builder.br(drain_header);

        self.builder.position_at_end(drain_free);
        self.builder.call(
            state.free_fn,
            &[state.data, state.total_size, state.align8],
            "",
        );
        self.builder.call(state.cleanup_exit_fn, &[], "");
        self.builder.resume(landing_pad);
    }

    fn emit_map_drop_drain_bucket(&mut self, state: &MapDropState) {
        if let Some(value_dec) = state.value_drop.function() {
            let index = self.builder.load(state.i64_type, state.index, "mdrop.dvi");
            let scaled = self.builder.mul(index, state.value_size, "mdrop.dvmul");
            let offset = self.builder.add(state.values_offset, scaled, "mdrop.dvadd");
            let value_ptr = self
                .builder
                .gep(state.i8_type, state.data, &[offset], "mdrop.dvptr");
            self.builder.call(value_dec, &[value_ptr], "");
        }
        if let Some(key_dec) = state.key_drop.function() {
            let index = self.builder.load(state.i64_type, state.index, "mdrop.dki");
            let scaled = self.builder.mul(index, state.key_size, "mdrop.dkmul");
            let offset = self.builder.add(state.keys_offset, scaled, "mdrop.dkadd");
            let key_ptr = self
                .builder
                .gep(state.i8_type, state.data, &[offset], "mdrop.dkptr");
            self.builder.call(key_dec, &[key_ptr], "");
        }
    }

    /// Emit `ori_map_buffer_drop_unique` for a provably unique map.
    ///
    /// Same argument extraction as `emit_buffer_rc_dec_map`, but calls
    /// the unique-drop function which skips the atomic RC decrement.
    pub(in crate::codegen::arc_emitter) fn emit_buffer_drop_unique_map(
        &mut self,
        val: ValueId,
        resolved: ori_types::Idx,
    ) {
        let Some(len) = self.builder.extract_value(val, FIELD_LEN, "udrop.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, FIELD_CAP, "udrop.cap") else {
            return;
        };
        let Some(data) = self
            .builder
            .extract_value(val, FIELD_DATA, "udrop.data_ptr")
        else {
            return;
        };

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        let key_size = self.collection_elem_size(resolved, key_type);
        let val_size = self.collection_elem_size(resolved, val_type);

        let key_may_unwind = self.drop_may_unwind(key_type);
        let val_may_unwind = self.drop_may_unwind(val_type);
        if (key_may_unwind || val_may_unwind)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium
        {
            // Map with a may-unwind key/value `@drop` (Itanium): emit a codegen
            // two-channel invoke loop so a panicking `@drop` threads through
            // codegen cleanup pads instead of aborting in the runtime
            // catch_unwind. Generate + cache both dec thunks; a scalar channel
            // has no cached FunctionId (None -> channel skipped).
            let _ = self.get_or_generate_elem_dec_fn(key_type);
            let _ = self.get_or_generate_elem_dec_fn(val_type);
            let key_dec_fid = self.elem_dec_fn_cache.get(&key_type).copied();
            let val_dec_fid = self.elem_dec_fn_cache.get(&val_type).copied();
            let key_drop = DropChannel::from_facts(key_dec_fid, key_may_unwind);
            let value_drop = DropChannel::from_facts(val_dec_fid, val_may_unwind);
            self.emit_codegen_buffer_drop_map_unwind(
                data, cap, key_size, val_size, key_drop, value_drop,
            );
            return;
        }

        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_type);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_type);

        let func_id = self.builder.runtime_fn("ori_map_buffer_drop_unique");
        self.emit_rt_call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                val_size_val,
                key_dec_fn,
                val_dec_fn,
            ],
            "",
        );
    }
}
