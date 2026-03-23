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
        let Some(data) = self.builder.extract_value(val, 2, "rc.data_ptr") else {
            return;
        };
        let Some(len) = self.builder.extract_value(val, 0, "rc.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "rc.cap") else {
            return;
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
        let elem_size = self.element_store_size(elem_type);
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
        let Some(len) = self.builder.extract_value(val, 0, "rc.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "rc.cap") else {
            return;
        };
        let Some(data) = self.builder.extract_value(val, 2, "rc.data_ptr") else {
            return;
        };

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        let key_size = self.element_store_size(key_type);
        let val_size = self.element_store_size(val_type);
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
        let Some(data) = self.builder.extract_value(val, 2, "udrop.data_ptr") else {
            return;
        };
        let Some(len) = self.builder.extract_value(val, 0, "udrop.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "udrop.cap") else {
            return;
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
        let elem_size = self.element_store_size(elem_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_type);

        if tag == Tag::Set {
            // Sets use hash table layout: (data, cap, len, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_set_buffer_drop_unique");
            self.emit_rt_call(func_id, &[data, cap, len, elem_size_val, elem_dec_fn], "");
        } else {
            // Lists use packed layout: (data, len, cap, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_buffer_drop_unique");
            self.emit_rt_call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
        }
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
        let Some(len) = self.builder.extract_value(val, 0, "udrop.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "udrop.cap") else {
            return;
        };
        let Some(data) = self.builder.extract_value(val, 2, "udrop.data_ptr") else {
            return;
        };

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        let key_size = self.element_store_size(key_type);
        let val_size = self.element_store_size(val_type);
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
        let Some(data_ptr) = self.builder.extract_value(val, 2, "rc_inc.fat_data") else {
            return;
        };
        let cap = self
            .builder
            .extract_value(val, 1, "rc_inc.fat_cap")
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
        let Some(data_ptr) = self.builder.extract_value(val, 2, "rc_dec.fat_data") else {
            return;
        };
        let cap = self
            .builder
            .extract_value(val, 1, "rc_dec.fat_cap")
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
