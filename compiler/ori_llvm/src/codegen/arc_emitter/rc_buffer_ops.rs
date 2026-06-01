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
            let func_id = self.builder.runtime_fn("ori_set_buffer_rc_dec");
            self.emit_rt_call(func_id, &[data, cap, len, elem_size_val, elem_dec_fn], "");
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
            // (Set element @drop recoverable teardown is BUG-05-006-blocked — the
            // set hash-table layout segfaults; stays on the runtime path.)
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
