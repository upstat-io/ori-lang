//! Buffer reference-count operations for lists, sets, maps, and string fat pointers.
//!
//! Unique-drop entry points skip the atomic decrement when static analysis has
//! proved the collection uniquely owned.

use ori_types::Tag;

use super::ArcIrEmitter;

mod map;

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
        let Some((data, len, cap)) =
            self.extract_collection_fields(val, "rc.data_ptr", "rc.len", "rc.cap")
        else {
            // Why: ARC routes only canonical list/set values to buffer RC emission.
            unreachable!("list/set RC input must have the canonical collection layout")
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
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
            // Itanium uses the codegen cleanup loop because runtime catch_unwind
            // aborts on foreign Ori exceptions. The dec-to-zero gate handles
            // nested collections whose field/element teardown is an rc_dec.
            let elem_dec_fid = self.elem_dec_fn_cache[&elem_type];
            let cur = self.current_function;
            let dec_fn = self.builder.runtime_fn("ori_rc_dec_to_zero");
            let Some(hit) = self.builder.call(dec_fn, &[data], "ldec.hz") else {
                // Why: The registered ori_rc_dec_to_zero ABI returns an i8 flag.
                unreachable!("ori_rc_dec_to_zero must produce its registered return value")
            };
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
        let Some((data, len, cap)) =
            self.extract_collection_fields(val, "udrop.data_ptr", "udrop.len", "udrop.cap")
        else {
            // Why: ARC routes only canonical list/set values to unique-drop emission.
            unreachable!("list/set unique-drop input must have the canonical collection layout")
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
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

    /// Emit an Itanium element-drop loop with cleanup pads for a list.
    ///
    /// A panicking `@drop` drains the remaining elements, frees the buffer, and
    /// resumes the exception. `elem_size` is the canonical store size and
    /// `elem_dec_fid` is the may-unwind element-dec thunk. The builder finishes
    /// at the post-free continuation block.
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

        self.builder.position_at_end(hdr);
        let i = self.builder.load(i64_ty, idx, "bdrop.i.v");
        let ge = self.builder.icmp_sge(i, len, "bdrop.ge");
        self.builder.cond_br(ge, done, body);

        self.builder.position_at_end(body);
        let elem_ptr = self.builder.gep(elem_llvm_ty, data, &[i], "bdrop.elem");
        self.builder
            .invoke(elem_dec_fid, &[elem_ptr], cont, cleanup, "");

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

        self.builder.position_at_end(done);
        self.builder.call(free_fn, &[data, cap, elem_size_val], "");
    }

    // FatPointer handlers (SSO-aware string RC)

    /// Inc a fat value (str = `{i64 len, i64 cap, ptr data}`).
    ///
    /// Calls `ori_str_rc_inc(data, cap)` which handles SSO, heap, and shared-buffer
    /// slices from `str.split()`. The runtime function checks for `SLICE_FLAG`
    /// in cap and finds the original buffer for slices.
    pub(super) fn emit_rc_inc_fat(&mut self, var: ori_arc::ir::ArcVarId, count: u32) {
        let val = self.var(var);
        let Some((data_ptr, _, cap)) = self.extract_collection_fields(
            val,
            "rc_inc.fat_data",
            "rc_inc.fat_len",
            "rc_inc.fat_cap",
        ) else {
            // Why: ARC routes only canonical Str values to fat-pointer RC emission.
            unreachable!("string RC input must have the canonical fat-pointer layout")
        };
        self.call_str_rc_inc(data_ptr, cap, count);
    }

    /// Dec a fat value (str = `{i64 len, i64 cap, ptr data}`).
    ///
    /// Calls `ori_str_rc_dec(data, cap, drop_fn)` which handles SSO, heap, and
    /// shared-buffer slices from `str.split()`. The runtime function checks for
    /// `SLICE_FLAG` in cap and finds the original buffer for slices.
    pub(super) fn emit_rc_dec_fat(
        &mut self,
        var: ori_arc::ir::ArcVarId,
        func: &ori_arc::ir::ArcFunction,
    ) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let Some((data_ptr, _, cap)) = self.extract_collection_fields(
            val,
            "rc_dec.fat_data",
            "rc_dec.fat_len",
            "rc_dec.fat_cap",
        ) else {
            // Why: ARC routes only canonical Str values to fat-pointer RC emission.
            unreachable!("string RC input must have the canonical fat-pointer layout")
        };
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
        let ptr_int = self
            .builder
            .ptr_to_int(data_ptr, i64_ty, &format!("{prefix}.p2i"));
        let sso_mask = self.builder.const_i64(i64::MIN);
        let masked = self
            .builder
            .and(ptr_int, sso_mask, &format!("{prefix}.sso_flag"));
        let zero = self.builder.const_i64(0);
        let is_sso = self
            .builder
            .icmp_ne(masked, zero, &format!("{prefix}.is_sso"));
        let is_null = self
            .builder
            .icmp_eq(ptr_int, zero, &format!("{prefix}.is_null"));
        self.builder
            .or(is_sso, is_null, &format!("{prefix}.skip_rc"))
    }
}
