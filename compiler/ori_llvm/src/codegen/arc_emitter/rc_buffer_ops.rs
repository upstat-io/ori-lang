//! Buffer-level RC operations for collections (List, Set, Map).
//!
//! Handles the buffer RC dec/drop for heap-allocated collection data:
//!
//! - **Standard dec**: `ori_buffer_rc_dec`, `ori_set_buffer_rc_dec`, `ori_map_buffer_rc_dec`
//! - **Unique-drop**: `ori_buffer_drop_unique`, `ori_set_buffer_drop_unique`, `ori_map_buffer_drop_unique`
//!
//! Unique-drop functions skip the atomic RC decrement for provably unique
//! collections (refcount known to be 1 via static analysis).
//!
//! Also includes SSO (Small String Optimization) detection for fat pointer
//! RC operations.

use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::Tag;

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // Buffer RC dec helpers

    /// Emit `ori_buffer_rc_dec` for a list or set value.
    ///
    /// Extracts `{len, cap, data}` from the collection value, computes
    /// the element size and element-dec function, and calls the runtime.
    ///
    /// Lists use `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)`.
    /// Sets use `ori_set_buffer_rc_dec(data, cap, len, elem_size, elem_dec_fn)`
    /// (cap and len swapped for hash table layout).
    pub(super) fn emit_buffer_rc_dec_list_or_set(
        &mut self,
        val: super::ValueId,
        resolved: ori_types::Idx,
        tag: Tag,
    ) {
        let Some(data) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") else {
            return;
        };
        let Some(len) = self.builder.extract_value(val, FIELD_LEN, "rc.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, FIELD_CAP, "rc.cap") else {
            return;
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
        // Use narrowed element size for collection buffers.
        let elem_size = self.collection_elem_size(resolved, elem_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_type);

        if tag == Tag::Set {
            // Sets use hash table layout: (data, cap, len, elem_size, elem_dec_fn)
            // (Set element @drop recoverable teardown stays on the runtime path
            // — the set hash-table layout segfaults under codegen teardown.)
            let func_id = self.builder.runtime_fn("ori_set_buffer_rc_dec");
            self.emit_rt_call(func_id, &[data, cap, len, elem_size_val, elem_dec_fn], "");
        } else if self.drop_may_unwind(elem_type)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium
            && self.elem_dec_fn_cache.contains_key(&elem_type)
        {
            // List with a may-unwind element `@drop` (Itanium): atomic
            // dec-to-zero, then on zero run the codegen per-element cleanup loop
            // (drains remaining + frees) instead of the runtime `catch_unwind`
            // cleanup that aborts on the foreign Ori exception. Reached for a
            // may-unwind list nested in a struct/list (its field/element dec is
            // an rc_dec, not a unique-drop).
            let elem_dec_fid = self.elem_dec_fn_cache[&elem_type];
            let cur = self.current_function;
            let dec_fn = self.builder.runtime_fn("ori_rc_dec_to_zero");
            let hit = self
                .builder
                .call(dec_fn, &[data], "ldec.hz")
                .expect("ori_rc_dec_to_zero returns i8");
            let zero8 = self.builder.const_i8(0);
            let is_zero = self.builder.icmp_ne(hit, zero8, "ldec.zero");
            let cleanup_bb = self.builder.append_block(cur, "ldec.cleanup");
            let after_bb = self.builder.append_block(cur, "ldec.after");
            self.builder.cond_br(is_zero, cleanup_bb, after_bb);
            self.builder.position_at_end(cleanup_bb);
            self.emit_codegen_buffer_drop_list_unwind(
                data,
                len,
                cap,
                elem_type,
                elem_dec_fid,
                elem_size,
            );
            self.builder.br(after_bb);
            self.builder.position_at_end(after_bb);
        } else {
            // Lists use packed layout: (data, len, cap, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
            self.emit_rt_call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
        }
    }

    /// Emit `ori_map_buffer_rc_dec` for a map value.
    ///
    /// Maps use an open-addressing hash table `[metadata | keys | values]` with
    /// 1-byte metadata per bucket, one RC header. The runtime function handles
    /// cleaning up both key and value children at their respective offsets.
    pub(super) fn emit_buffer_rc_dec_map(&mut self, val: super::ValueId, resolved: ori_types::Idx) {
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

        // Use narrowed element sizes for map buffers.
        let key_size = self.collection_elem_size(resolved, key_type);
        let val_size = self.collection_elem_size(resolved, val_type);

        let key_may_unwind = self.drop_may_unwind(key_type);
        let val_may_unwind = self.drop_may_unwind(val_type);
        if (key_may_unwind || val_may_unwind)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium
        {
            // Map with a may-unwind key/value `@drop` (Itanium): atomic
            // dec-to-zero, then on zero run the codegen two-channel cleanup loop
            // (its own unwind pads drain remaining buckets + free) instead of
            // the runtime `catch_unwind` cleanup that aborts on the foreign Ori
            // exception. Reached for a may-unwind map nested in a struct/list
            // (its field/element dec is an rc_dec, not a unique-drop).
            let _ = self.get_or_generate_elem_dec_fn(key_type);
            let _ = self.get_or_generate_elem_dec_fn(val_type);
            let key_dec_fid = self.elem_dec_fn_cache.get(&key_type).copied();
            let val_dec_fid = self.elem_dec_fn_cache.get(&val_type).copied();
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
                data,
                cap,
                key_size,
                val_size,
                key_dec_fid,
                val_dec_fid,
                key_may_unwind,
                val_may_unwind,
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

    // Unique-drop handlers (skip atomic RC dec for provably unique collections)

    /// Emit `ori_buffer_drop_unique` / `ori_set_buffer_drop_unique` for a
    /// provably unique list or set.
    ///
    /// Same argument extraction as `emit_buffer_rc_dec_list_or_set`, but calls
    /// the unique-drop function which skips the atomic RC decrement.
    pub(super) fn emit_buffer_drop_unique_list_or_set(
        &mut self,
        val: super::ValueId,
        resolved: ori_types::Idx,
        tag: Tag,
    ) {
        let Some(data) = self
            .builder
            .extract_value(val, FIELD_DATA, "udrop.data_ptr")
        else {
            return;
        };
        let Some(len) = self.builder.extract_value(val, FIELD_LEN, "udrop.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, FIELD_CAP, "udrop.cap") else {
            return;
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
        // Use narrowed element size for collection buffers.
        let elem_size = self.collection_elem_size(resolved, elem_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_type);

        if tag == Tag::Set {
            // Sets use hash table layout: (data, cap, len, elem_size, elem_dec_fn)
            // (Set element @drop recoverable teardown stays on the runtime path
            // — the set hash-table layout segfaults under codegen teardown.)
            let func_id = self.builder.runtime_fn("ori_set_buffer_drop_unique");
            self.emit_rt_call(func_id, &[data, cap, len, elem_size_val, elem_dec_fn], "");
        } else if self.drop_may_unwind(elem_type)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium
        {
            // List with a may-unwind element @drop (Itanium): emit a codegen
            // per-element invoke loop so a panicking element @drop threads
            // through codegen cleanup pads (drain remaining elements + free the
            // buffer + resume) instead of aborting in the runtime catch_unwind.
            if let Some(&elem_dec_fid) = self.elem_dec_fn_cache.get(&elem_type) {
                self.emit_codegen_buffer_drop_list_unwind(
                    data,
                    len,
                    cap,
                    elem_type,
                    elem_dec_fid,
                    elem_size,
                );
                return;
            }
            // No element-dec fn cached (scalar element shouldn't reach here) —
            // fall back to the runtime path.
            let func_id = self.builder.runtime_fn("ori_buffer_drop_unique");
            self.emit_rt_call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
        } else {
            // Lists use packed layout: (data, len, cap, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_buffer_drop_unique");
            self.emit_rt_call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
        }
    }

    /// Codegen-emitted unique-drop element loop for a list whose element type
    /// may unwind (Itanium). Replaces the runtime `ori_buffer_drop_unique` call
    /// so a per-element `@drop` panic threads through codegen cleanup pads
    /// (drain the remaining elements + free the buffer + `resume`) rather than
    /// aborting in the runtime frame's `catch_unwind` (which cannot catch the
    /// foreign Ori exception).
    ///
    /// `elem_size` is the (canonical — may-unwind structs are never narrowed)
    /// element store size; `elem_dec_fid` is the element-dec thunk (itself
    /// may-unwind: it invokes the element's `@drop`). Leaves the builder
    /// positioned at the post-free continuation block for the caller.
    fn emit_codegen_buffer_drop_list_unwind(
        &mut self,
        data: super::ValueId,
        len: super::ValueId,
        cap: super::ValueId,
        elem_type: ori_types::Idx,
        elem_dec_fid: crate::codegen::value_id::FunctionId,
        elem_size: u64,
    ) {
        let cur = self.current_function;
        let i64_ty = self.builder.i64_type();
        let elem_llvm_ty = self.resolve_type(elem_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let one = self.builder.const_i64(1);
        let zero = self.builder.const_i64(0);
        let personality = self.builder.runtime_fn("ori_eh_personality");
        // The enclosing function now contains an invoke + landing pad.
        self.builder.set_personality(cur, personality);
        let free_fn = self.builder.runtime_fn("ori_list_free_data");

        // Loop index in an alloca so the cleanup pad can read it to drain the
        // remaining (post-panic) elements.
        let idx = self.builder.create_entry_alloca(cur, "bdrop.i", i64_ty);
        self.builder.store(zero, idx);

        let hdr = self.builder.append_block(cur, "bdrop.hdr");
        let body = self.builder.append_block(cur, "bdrop.body");
        let cont = self.builder.append_block(cur, "bdrop.cont");
        let cleanup = self.builder.append_block(cur, "bdrop.cleanup");
        let done = self.builder.append_block(cur, "bdrop.done");

        self.builder.br(hdr);

        // hdr: i = *idx; if i >= len -> done else body
        self.builder.position_at_end(hdr);
        let i = self.builder.load(i64_ty, idx, "bdrop.i.v");
        let ge = self.builder.icmp_sge(i, len, "bdrop.ge");
        self.builder.cond_br(ge, done, body);

        // body: invoke elem_dec(data + i) -> cont (normal) / cleanup (unwind)
        self.builder.position_at_end(body);
        let elem_ptr = self.builder.gep(elem_llvm_ty, data, &[i], "bdrop.elem");
        self.builder
            .invoke(elem_dec_fid, &[elem_ptr], cont, cleanup, "");

        // cont: i++ ; -> hdr
        self.builder.position_at_end(cont);
        let i2 = self.builder.load(i64_ty, idx, "bdrop.i2");
        let inc = self.builder.add(i2, one, "bdrop.inc");
        self.builder.store(inc, idx);
        self.builder.br(hdr);

        // cleanup: landingpad; advance past the panicking element (its children
        // were freed by elem_dec's own cleanup pad); drain the rest via plain
        // calls (a nested panic here double-unwinds -> abort); free; resume.
        self.builder.position_at_end(cleanup);
        let lp = self.builder.landingpad(personality, true, "bdrop.lp");
        let cl_enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
        self.builder.call(cl_enter, &[], "");
        let ci = self.builder.load(i64_ty, idx, "bdrop.ci");
        let cnext = self.builder.add(ci, one, "bdrop.cnext");
        self.builder.store(cnext, idx);
        let dhdr = self.builder.append_block(cur, "bdrop.drain.hdr");
        let dbody = self.builder.append_block(cur, "bdrop.drain.body");
        let dfree = self.builder.append_block(cur, "bdrop.drain.free");
        self.builder.br(dhdr);
        self.builder.position_at_end(dhdr);
        let di = self.builder.load(i64_ty, idx, "bdrop.di");
        let dge = self.builder.icmp_sge(di, len, "bdrop.dge");
        self.builder.cond_br(dge, dfree, dbody);
        self.builder.position_at_end(dbody);
        let delem = self.builder.gep(elem_llvm_ty, data, &[di], "bdrop.delem");
        self.builder.call(elem_dec_fid, &[delem], "");
        let dinc = self.builder.add(di, one, "bdrop.dinc");
        self.builder.store(dinc, idx);
        self.builder.br(dhdr);
        self.builder.position_at_end(dfree);
        self.builder.call(free_fn, &[data, cap, elem_size_val], "");
        let cl_exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
        self.builder.call(cl_exit, &[], "");
        self.builder.resume(lp);

        // done: free buffer; leave builder here for the caller's continuation.
        self.builder.position_at_end(done);
        self.builder.call(free_fn, &[data, cap, elem_size_val], "");
    }

    /// Codegen-emitted unique-drop two-channel loop for a map whose key OR
    /// value type may unwind (Itanium). Replaces the runtime
    /// `ori_map_buffer_drop_unique` call so a per-bucket key/value `@drop`
    /// panic threads through codegen cleanup pads (drain the remaining occupied
    /// buckets BOTH channels + free the buffer + `resume`) rather than aborting
    /// in the runtime frame's `catch_unwind` (which cannot catch the foreign
    /// Ori exception).
    ///
    /// Per occupied bucket: dec value, then dec key (mirrors the runtime
    /// `map_buffer_cleanup` order). A may-unwind channel uses `invoke`; a
    /// non-may-unwind channel (e.g. `str` key) uses a plain `call`. The shared
    /// cleanup pad reads a `pending_key_at_cur` flag: a VALUE panic at bucket B
    /// leaves the key at B still to dec (flag = 1); a KEY panic at B means B is
    /// fully done (flag = 0). Both invokes route to one pad which dec's the
    /// pending key, advances past B, drains B+1..cap both channels, frees, and
    /// resumes. `key_size`/`val_size` are the (canonical — may-unwind elements
    /// are never narrowed) store sizes; `*_dec_fid` is `None` for a scalar
    /// channel (no dec needed). Layout offsets come from the
    /// `HashTableLayout::for_map` SSOT accessors. Leaves the builder positioned
    /// at the post-free continuation block for the caller.
    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "Map-buffer drop-on-unwind is one indivisible IR-emission sequence; the 8 params are the buffer-drop descriptor"
    )]
    fn emit_codegen_buffer_drop_map_unwind(
        &mut self,
        data: super::ValueId,
        cap: super::ValueId,
        key_size: u64,
        val_size: u64,
        key_dec_fid: Option<crate::codegen::value_id::FunctionId>,
        val_dec_fid: Option<crate::codegen::value_id::FunctionId>,
        key_may_unwind: bool,
        val_may_unwind: bool,
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

        self.builder.br(hdr);

        // hdr: i = *idx; if i >= cap -> done else body
        self.builder.position_at_end(hdr);
        let i = self.builder.load(i64_ty, idx, "mdrop.i.v");
        let ge = self.builder.icmp_sge(i, cap, "mdrop.ge");
        self.builder.cond_br(ge, done, body);

        // body: occupied(data, i)? -> live : cont
        self.builder.position_at_end(body);
        let i_b = self.builder.load(i64_ty, idx, "mdrop.i.b");
        let occ = self
            .builder
            .call(occ_fn, &[data, i_b], "mdrop.occ")
            .expect("ori_map_bucket_occupied returns i8");
        let is_occ = self.builder.icmp_ne(occ, zero8, "mdrop.is_occ");
        self.builder.cond_br(is_occ, live, cont);

        // live: value channel then key channel (each may invoke or plain-call).
        self.builder.position_at_end(live);
        if let Some(vfid) = val_dec_fid {
            let il = self.builder.load(i64_ty, idx, "mdrop.iv");
            let vm = self.builder.mul(il, val_size_v, "mdrop.vmul");
            let vt = self.builder.add(vals_off, vm, "mdrop.vadd");
            let vp = self.builder.gep(i8_ty, data, &[vt], "mdrop.vptr");
            if val_may_unwind {
                self.builder.store(one8, pk); // pending key at current bucket
                let av = self.builder.append_block(cur, "mdrop.afterval");
                self.builder.invoke(vfid, &[vp], av, cleanup, "");
                self.builder.position_at_end(av);
            } else {
                self.builder.call(vfid, &[vp], "");
            }
        }
        if let Some(kfid) = key_dec_fid {
            let ik = self.builder.load(i64_ty, idx, "mdrop.ik");
            let km = self.builder.mul(ik, key_size_v, "mdrop.kmul");
            let kt = self.builder.add(keys_off, km, "mdrop.kadd");
            let kp = self.builder.gep(i8_ty, data, &[kt], "mdrop.kptr");
            if key_may_unwind {
                self.builder.store(zero8, pk); // current bucket done if key panics
                self.builder.invoke(kfid, &[kp], cont, cleanup, "");
            } else {
                self.builder.call(kfid, &[kp], "");
                self.builder.br(cont);
            }
        } else {
            self.builder.br(cont);
        }

        // cont: i++ ; -> hdr
        self.builder.position_at_end(cont);
        let i2 = self.builder.load(i64_ty, idx, "mdrop.i2");
        let inc = self.builder.add(i2, one, "mdrop.inc");
        self.builder.store(inc, idx);
        self.builder.br(hdr);

        // cleanup: landingpad; if a VALUE invoke panicked (pending_key == 1) the
        // key at the current bucket still needs deccing; then advance past the
        // bucket and drain the rest both channels (nested panic here
        // double-unwinds -> abort via the depth guard); free; resume.
        self.builder.position_at_end(cleanup);
        let lp = self.builder.landingpad(personality, true, "mdrop.lp");
        self.builder.call(cl_enter, &[], "");
        let pk_v = self.builder.load(i8_ty, pk, "mdrop.pk.v");
        let pk_set = self.builder.icmp_ne(pk_v, zero8, "mdrop.pk.set");
        let cl_key = self.builder.append_block(cur, "mdrop.cl.key");
        let cl_adv = self.builder.append_block(cur, "mdrop.cl.adv");
        self.builder.cond_br(pk_set, cl_key, cl_adv);

        self.builder.position_at_end(cl_key);
        if let Some(kfid) = key_dec_fid {
            let ci = self.builder.load(i64_ty, idx, "mdrop.cl.ci");
            let km = self.builder.mul(ci, key_size_v, "mdrop.cl.kmul");
            let kt = self.builder.add(keys_off, km, "mdrop.cl.kadd");
            let kp = self.builder.gep(i8_ty, data, &[kt], "mdrop.cl.kptr");
            self.builder.call(kfid, &[kp], "");
        }
        self.builder.br(cl_adv);

        self.builder.position_at_end(cl_adv);
        let ci2 = self.builder.load(i64_ty, idx, "mdrop.cl.ci2");
        let cnext = self.builder.add(ci2, one, "mdrop.cl.cnext");
        self.builder.store(cnext, idx);
        let dhdr = self.builder.append_block(cur, "mdrop.drain.hdr");
        let dbody = self.builder.append_block(cur, "mdrop.drain.body");
        let dlive = self.builder.append_block(cur, "mdrop.drain.live");
        let dnext = self.builder.append_block(cur, "mdrop.drain.next");
        let dfree = self.builder.append_block(cur, "mdrop.drain.free");
        self.builder.br(dhdr);

        self.builder.position_at_end(dhdr);
        let di = self.builder.load(i64_ty, idx, "mdrop.di");
        let dge = self.builder.icmp_sge(di, cap, "mdrop.dge");
        self.builder.cond_br(dge, dfree, dbody);

        self.builder.position_at_end(dbody);
        let di_b = self.builder.load(i64_ty, idx, "mdrop.di.b");
        let docc = self
            .builder
            .call(occ_fn, &[data, di_b], "mdrop.docc")
            .expect("ori_map_bucket_occupied returns i8");
        let dis = self.builder.icmp_ne(docc, zero8, "mdrop.dis");
        self.builder.cond_br(dis, dlive, dnext);

        self.builder.position_at_end(dlive);
        if let Some(vfid) = val_dec_fid {
            let dii = self.builder.load(i64_ty, idx, "mdrop.dvi");
            let dvm = self.builder.mul(dii, val_size_v, "mdrop.dvmul");
            let dvt = self.builder.add(vals_off, dvm, "mdrop.dvadd");
            let dvp = self.builder.gep(i8_ty, data, &[dvt], "mdrop.dvptr");
            self.builder.call(vfid, &[dvp], "");
        }
        if let Some(kfid) = key_dec_fid {
            let dki = self.builder.load(i64_ty, idx, "mdrop.dki");
            let dkm = self.builder.mul(dki, key_size_v, "mdrop.dkmul");
            let dkt = self.builder.add(keys_off, dkm, "mdrop.dkadd");
            let dkp = self.builder.gep(i8_ty, data, &[dkt], "mdrop.dkptr");
            self.builder.call(kfid, &[dkp], "");
        }
        self.builder.br(dnext);

        self.builder.position_at_end(dnext);
        let din = self.builder.load(i64_ty, idx, "mdrop.din");
        let dinc = self.builder.add(din, one, "mdrop.dinc");
        self.builder.store(dinc, idx);
        self.builder.br(dhdr);

        self.builder.position_at_end(dfree);
        self.builder.call(free_fn, &[data, total, align8], "");
        self.builder.call(cl_exit, &[], "");
        self.builder.resume(lp);

        // done: free buffer; leave builder here for the caller's continuation.
        self.builder.position_at_end(done);
        self.builder.call(free_fn, &[data, total, align8], "");
    }

    /// Emit `ori_map_buffer_drop_unique` for a provably unique map.
    ///
    /// Same argument extraction as `emit_buffer_rc_dec_map`, but calls
    /// the unique-drop function which skips the atomic RC decrement.
    pub(super) fn emit_buffer_drop_unique_map(
        &mut self,
        val: super::ValueId,
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

        // Use narrowed element sizes for map buffers.
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
            self.emit_codegen_buffer_drop_map_unwind(
                data,
                cap,
                key_size,
                val_size,
                key_dec_fid,
                val_dec_fid,
                key_may_unwind,
                val_may_unwind,
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

    // FatPointer handlers (SSO-aware string RC)

    /// Inc a fat value (str = `{i64 len, i64 cap, ptr data}`).
    ///
    /// Calls `ori_str_rc_inc(data, cap)` which handles SSO, heap, and seamless
    /// slices from `str.split()`. The runtime function checks for `SLICE_FLAG`
    /// in cap and finds the original buffer for slices.
    pub(super) fn emit_rc_inc_fat(&mut self, var: ori_arc::ir::ArcVarId, count: u32) {
        let val = self.var(var);
        let Some(data_ptr) = self
            .builder
            .extract_value(val, FIELD_DATA, "rc_inc.fat_data")
        else {
            return;
        };
        let cap = self
            .builder
            .extract_value(val, FIELD_CAP, "rc_inc.fat_cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        self.call_str_rc_inc(data_ptr, cap, count);
    }

    /// Dec a fat value (str = `{i64 len, i64 cap, ptr data}`).
    ///
    /// Calls `ori_str_rc_dec(data, cap, drop_fn)` which handles SSO, heap, and
    /// seamless slices from `str.split()`. The runtime function checks for
    /// `SLICE_FLAG` in cap and finds the original buffer for slices.
    pub(super) fn emit_rc_dec_fat(
        &mut self,
        var: ori_arc::ir::ArcVarId,
        func: &ori_arc::ir::ArcFunction,
    ) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let Some(data_ptr) = self
            .builder
            .extract_value(val, FIELD_DATA, "rc_dec.fat_data")
        else {
            return;
        };
        let cap = self
            .builder
            .extract_value(val, FIELD_CAP, "rc_dec.fat_cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let drop_fn = self.get_or_generate_drop_fn(ty);
        self.call_str_rc_dec(data_ptr, cap, drop_fn);
    }

    /// Check if a string's data pointer field indicates SSO (inline storage).
    ///
    /// SSO strings store inline bytes in the union. The SSO flag is the MSB
    /// of byte 23 (the `flags` field). On little-endian x86-64, this maps to
    /// the MSB of the pointer field. We also treat null pointers as "skip RC"
    /// (empty heap strings have null data).
    pub(super) fn emit_sso_check(
        &mut self,
        data_ptr: super::ValueId,
        prefix: &str,
    ) -> super::ValueId {
        let i64_ty = self.builder.i64_type();
        // Single ptrtoint — reuse for both SSO check and null check.
        let ptr_int = self
            .builder
            .ptr_to_int(data_ptr, i64_ty, &format!("{prefix}.p2i"));
        let sso_mask = self.builder.const_i64(i64::MIN); // 0x8000_0000_0000_0000
        let masked = self
            .builder
            .and(ptr_int, sso_mask, &format!("{prefix}.sso_flag"));
        let zero = self.builder.const_i64(0);
        let is_sso = self
            .builder
            .icmp_ne(masked, zero, &format!("{prefix}.is_sso"));
        // Reuse ptr_int for null check (avoids duplicate ptrtoint)
        let is_null = self
            .builder
            .icmp_eq(ptr_int, zero, &format!("{prefix}.is_null"));
        self.builder
            .or(is_sso, is_null, &format!("{prefix}.skip_rc"))
    }
}
