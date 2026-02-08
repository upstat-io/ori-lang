//! Collection lowering for V2 codegen.
//!
//! Handles construction and access for tuples, structs, ranges, lists,
//! maps, sets, and their field/index operations.

use ori_ir::{
    ExprId, ExprRange, FieldInitRange, ListElementRange, MapElementRange, MapEntryRange, Name,
    StructLitFieldRange,
};
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::type_info::TypeInfo;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Tuple
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Tuple(range)` — `(a, b, c)`.
    ///
    /// Compiles each element and builds an LLVM struct.
    pub(crate) fn lower_tuple(&mut self, range: ExprRange, expr_id: ExprId) -> Option<ValueId> {
        let expr_ids = self.arena.get_expr_list(range);
        let mut values = Vec::with_capacity(expr_ids.len());

        for &eid in expr_ids {
            let val = self.lower(eid)?;
            values.push(val);
        }

        let result_type = self.expr_type(expr_id);
        let tuple_ty = self.resolve_type(result_type);
        Some(self.builder.build_struct(tuple_ty, &values, "tuple"))
    }

    // -----------------------------------------------------------------------
    // Struct literal
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Struct { name, fields }` — `Point { x: 1, y: 2 }`.
    ///
    /// Resolves field order from the `TypeInfo` and builds the struct
    /// with fields in declaration order (not source order).
    pub(crate) fn lower_struct(
        &mut self,
        _name: Name,
        fields: FieldInitRange,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let result_type = self.expr_type(expr_id);
        let type_info = self.type_info.get(result_type);

        let field_inits = self.arena.get_field_inits(fields);

        // Get declared field order from TypeInfo
        let declared_fields: Vec<(Name, Idx)> = if let TypeInfo::Struct { fields } = &type_info {
            fields.clone()
        } else {
            // Fall back to source order
            let mut values = Vec::with_capacity(field_inits.len());
            for fi in field_inits {
                if let Some(val_id) = fi.value {
                    let val = self.lower(val_id)?;
                    values.push(val);
                } else {
                    // Shorthand: `{ x }` uses the binding `x`
                    let val = self.lower_ident(fi.name)?;
                    values.push(val);
                }
            }
            let struct_ty = self.resolve_type(result_type);
            return Some(self.builder.build_struct(struct_ty, &values, "struct"));
        };

        // Build values in declaration order
        let mut values = vec![None; declared_fields.len()];
        for fi in field_inits {
            // Find the index of this field in the declaration
            let field_idx = declared_fields
                .iter()
                .position(|(name, _)| *name == fi.name);

            let val = if let Some(val_id) = fi.value {
                self.lower(val_id)?
            } else {
                // Shorthand: `{ x }` uses the binding `x`
                self.lower_ident(fi.name)?
            };

            if let Some(idx) = field_idx {
                values[idx] = Some(val);
            }
        }

        // Fill any missing fields with zero (should not happen with type checking)
        let filled: Vec<ValueId> = values
            .into_iter()
            .map(|v| v.unwrap_or_else(|| self.builder.const_i64(0)))
            .collect();

        let struct_ty = self.resolve_type(result_type);
        Some(self.builder.build_struct(struct_ty, &filled, "struct"))
    }

    /// Lower `ExprKind::StructWithSpread { name, fields }`.
    ///
    /// Struct with spread syntax: `Point { ...base, x: 10 }`.
    #[allow(clippy::unused_self)] // Will use self when spread is implemented
    pub(crate) fn lower_struct_with_spread(
        &mut self,
        _name: Name,
        _fields: StructLitFieldRange,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        tracing::warn!("struct spread syntax not yet implemented in V2 codegen");
        None
    }

    // -----------------------------------------------------------------------
    // Range
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Range { start, end, step, inclusive }`.
    ///
    /// Produces `{i64 start, i64 end, i1 inclusive}`. Step is not stored
    /// in the struct (for-loops default to step=1).
    pub(crate) fn lower_range(
        &mut self,
        start: ExprId,
        end: ExprId,
        _step: ExprId,
        inclusive: bool,
    ) -> Option<ValueId> {
        let start_val = self.lower(start)?;
        let end_val = self.lower(end)?;
        let incl_val = self.builder.const_bool(inclusive);

        // Build range struct type: {i64, i64, i1}
        let range_llvm = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i1().into(),
                    ],
                    false,
                )
                .into(),
        );

        Some(
            self.builder
                .build_struct(range_llvm, &[start_val, end_val, incl_val], "range"),
        )
    }

    // -----------------------------------------------------------------------
    // Field access
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Field { receiver, field }` — `expr.field`.
    ///
    /// For structs: compute field index from `TypeInfo`, emit `extractvalue`.
    /// For tuples: field name is the numeric index (e.g., `.0`, `.1`).
    pub(crate) fn lower_field(&mut self, receiver: ExprId, field: Name) -> Option<ValueId> {
        let recv_val = self.lower(receiver)?;
        let recv_type = self.expr_type(receiver);
        let type_info = self.type_info.get(recv_type);

        match &type_info {
            TypeInfo::Struct { fields } => {
                // Find field index by name
                let field_idx = fields.iter().position(|(name, _)| *name == field);
                if let Some(idx) = field_idx {
                    self.builder.extract_value(recv_val, idx as u32, "field")
                } else {
                    let field_name = self.resolve_name(field);
                    tracing::warn!(field = field_name, "unknown struct field in codegen");
                    None
                }
            }
            TypeInfo::Tuple { .. } => {
                // Tuple field names are numeric: "0", "1", etc.
                let field_name = self.resolve_name(field);
                if let Ok(idx) = field_name.parse::<u32>() {
                    self.builder.extract_value(recv_val, idx, "tuple_field")
                } else {
                    tracing::warn!(field = field_name, "non-numeric tuple field");
                    None
                }
            }
            TypeInfo::Str => {
                // String field access: .len, .data
                let field_name = self.resolve_name(field);
                match field_name {
                    "len" | "length" => self.builder.extract_value(recv_val, 0, "str.len"),
                    _ => {
                        tracing::warn!(field = field_name, "unknown string field");
                        None
                    }
                }
            }
            TypeInfo::List { .. } => {
                let field_name = self.resolve_name(field);
                match field_name {
                    "len" | "length" => self.builder.extract_value(recv_val, 0, "list.len"),
                    "cap" | "capacity" => self.builder.extract_value(recv_val, 1, "list.cap"),
                    _ => {
                        tracing::warn!(field = field_name, "unknown list field");
                        None
                    }
                }
            }
            TypeInfo::Range => {
                let field_name = self.resolve_name(field);
                match field_name {
                    "start" => self.builder.extract_value(recv_val, 0, "range.start"),
                    "end" => self.builder.extract_value(recv_val, 1, "range.end"),
                    "inclusive" => self.builder.extract_value(recv_val, 2, "range.inclusive"),
                    _ => {
                        tracing::warn!(field = field_name, "unknown range field");
                        None
                    }
                }
            }
            _ => {
                let field_name = self.resolve_name(field);
                tracing::warn!(
                    field = field_name,
                    ?recv_type,
                    "field access on unsupported type"
                );
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Index access
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Index { receiver, index }` — `expr[index]`.
    ///
    /// For lists: bounds-check + element pointer access.
    /// For tuples: static index extraction.
    pub(crate) fn lower_index(&mut self, receiver: ExprId, index: ExprId) -> Option<ValueId> {
        let recv_val = self.lower(receiver)?;
        let idx_val = self.lower(index)?;
        let recv_type = self.expr_type(receiver);
        let type_info = self.type_info.get(recv_type);

        match &type_info {
            TypeInfo::List { element } => {
                let elem_idx = *element;
                let elem_llvm_ty = self.resolve_type(elem_idx);

                // Extract data pointer from list struct: field 2
                let data_ptr = self.builder.extract_value(recv_val, 2, "list.data")?;

                // Bounds check
                let len = self.builder.extract_value(recv_val, 0, "list.len")?;
                let in_bounds = self.builder.icmp_slt(idx_val, len, "idx.inbounds");

                let access_bb = self
                    .builder
                    .append_block(self.current_function, "idx.access");
                let panic_bb = self
                    .builder
                    .append_block(self.current_function, "idx.panic");
                let merge_bb = self
                    .builder
                    .append_block(self.current_function, "idx.merge");

                self.builder.cond_br(in_bounds, access_bb, panic_bb);

                // Panic on out-of-bounds
                self.builder.position_at_end(panic_bb);
                self.emit_index_panic();
                self.builder.unreachable();

                // Access element
                self.builder.position_at_end(access_bb);
                let elem_ptr = self
                    .builder
                    .gep(elem_llvm_ty, data_ptr, &[idx_val], "idx.elem_ptr");
                let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "idx.elem");

                if !self.builder.current_block_terminated() {
                    self.builder.br(merge_bb);
                }
                let access_exit = self.builder.current_block()?;

                self.builder.position_at_end(merge_bb);
                self.builder.phi_from_incoming(
                    elem_llvm_ty,
                    &[(elem_val, access_exit)],
                    "idx.result",
                )
            }
            TypeInfo::Str => {
                // String indexing: access byte at index
                let data_ptr = self.builder.extract_value(recv_val, 1, "str.data")?;
                let i8_ty = self.builder.i8_type();
                let byte_ptr = self
                    .builder
                    .gep(i8_ty, data_ptr, &[idx_val], "str.byte_ptr");
                let byte_val = self.builder.load(i8_ty, byte_ptr, "str.byte");
                Some(byte_val)
            }
            TypeInfo::Tuple { elements } => {
                // Static tuple index — the index must be a compile-time constant
                // For now, try to extract from the value directly
                let raw_idx = self.builder.raw_value(idx_val);
                if let Some(const_idx) = raw_idx.into_int_value().get_zero_extended_constant() {
                    let idx = const_idx as u32;
                    if (idx as usize) < elements.len() {
                        self.builder.extract_value(recv_val, idx, "tuple.idx")
                    } else {
                        tracing::warn!(idx, len = elements.len(), "tuple index out of bounds");
                        None
                    }
                } else {
                    tracing::warn!("non-constant tuple index");
                    None
                }
            }
            _ => {
                tracing::warn!(?recv_type, "index access on unsupported type");
                None
            }
        }
    }

    /// Emit a panic for index out-of-bounds.
    fn emit_index_panic(&mut self) {
        let msg = self
            .builder
            .build_global_string_ptr("index out of bounds", "panic.idx_msg");

        // Try to call ori_panic_cstr
        if let Some(panic_fn) = self.builder.scx().llmod.get_function("ori_panic_cstr") {
            let panic_id = self.builder.intern_function(panic_fn);
            self.builder.call(panic_id, &[msg], "");
        }
    }

    // -----------------------------------------------------------------------
    // List literal
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::List(range)` — `[a, b, c]`.
    ///
    /// Allocates a list via `ori_list_new`, then stores each element.
    pub(crate) fn lower_list(&mut self, range: ExprRange, expr_id: ExprId) -> Option<ValueId> {
        let expr_ids = self.arena.get_expr_list(range);
        let count = expr_ids.len();

        let result_type = self.expr_type(expr_id);
        let type_info = self.type_info.get(result_type);
        let elem_idx = match &type_info {
            TypeInfo::List { element } => *element,
            _ => Idx::INT,
        };
        let elem_llvm_ty = self.resolve_type(elem_idx);
        let elem_size = self.type_info.get(elem_idx).size().unwrap_or(8);

        // Allocate list: ori_list_new(capacity, elem_size) -> ptr
        let cap = self.builder.const_i64(count as i64);
        let esize = self.builder.const_i64(elem_size as i64);
        let i64_ty = self.builder.i64_type();
        let i64_ty2 = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let list_new =
            self.builder
                .get_or_declare_function("ori_list_new", &[i64_ty, i64_ty2], ptr_ty);
        let data_ptr = self.builder.call(list_new, &[cap, esize], "list.data")?;

        // Store each element
        let mut compiled_values = Vec::with_capacity(count);
        for &eid in expr_ids {
            let val = self.lower(eid)?;
            compiled_values.push(val);
        }

        for (i, val) in compiled_values.iter().enumerate() {
            let idx = self.builder.const_i64(i as i64);
            let elem_ptr = self
                .builder
                .gep(elem_llvm_ty, data_ptr, &[idx], "list.elem_ptr");
            self.builder.store(*val, elem_ptr);
        }

        // Build list struct: {i64 len, i64 cap, ptr data}
        let len = self.builder.const_i64(count as i64);
        let list_ty = self.resolve_type(result_type);
        Some(
            self.builder
                .build_struct(list_ty, &[len, cap, data_ptr], "list"),
        )
    }

    /// Lower `ExprKind::ListWithSpread(elements)`.
    #[allow(clippy::unused_self)] // Will use self when spread is implemented
    pub(crate) fn lower_list_with_spread(
        &mut self,
        _elements: ListElementRange,
    ) -> Option<ValueId> {
        tracing::warn!("list spread syntax not yet implemented in V2 codegen");
        None
    }

    // -----------------------------------------------------------------------
    // Map literal
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Map(entries)` — `{k: v, ...}`.
    ///
    /// Allocates key and value arrays, stores entries, builds map struct.
    pub(crate) fn lower_map(&mut self, entries: MapEntryRange, expr_id: ExprId) -> Option<ValueId> {
        let map_entries = self.arena.get_map_entries(entries);
        let count = map_entries.len();

        let result_type = self.expr_type(expr_id);
        let type_info = self.type_info.get(result_type);
        let (key_idx, val_idx) = match &type_info {
            TypeInfo::Map { key, value } => (*key, *value),
            _ => (Idx::INT, Idx::INT),
        };
        let key_llvm_ty = self.resolve_type(key_idx);
        let val_llvm_ty = self.resolve_type(val_idx);
        let key_size = self.type_info.get(key_idx).size().unwrap_or(8);
        let val_size = self.type_info.get(val_idx).size().unwrap_or(8);

        // Allocate key and value arrays
        let cap = self.builder.const_i64(count as i64);
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let list_new =
            self.builder
                .get_or_declare_function("ori_list_new", &[i64_ty, i64_ty], ptr_ty);

        let key_elem_sz = self.builder.const_i64(key_size as i64);
        let keys_buf = self
            .builder
            .call(list_new, &[cap, key_elem_sz], "map.keys")?;

        let val_elem_sz = self.builder.const_i64(val_size as i64);
        let vals_buf = self
            .builder
            .call(list_new, &[cap, val_elem_sz], "map.vals")?;

        // Store each entry
        let mut compiled_keys = Vec::with_capacity(count);
        let mut compiled_vals = Vec::with_capacity(count);
        for entry in map_entries {
            let key = self.lower(entry.key)?;
            let val = self.lower(entry.value)?;
            compiled_keys.push(key);
            compiled_vals.push(val);
        }

        for (i, (key, val)) in compiled_keys.iter().zip(compiled_vals.iter()).enumerate() {
            let idx = self.builder.const_i64(i as i64);
            let kp = self
                .builder
                .gep(key_llvm_ty, keys_buf, &[idx], "map.key_ptr");
            self.builder.store(*key, kp);

            let vp = self
                .builder
                .gep(val_llvm_ty, vals_buf, &[idx], "map.val_ptr");
            self.builder.store(*val, vp);
        }

        // Build map struct: {i64 len, i64 cap, ptr keys, ptr vals}
        let len = self.builder.const_i64(count as i64);
        let map_ty = self.resolve_type(result_type);
        Some(
            self.builder
                .build_struct(map_ty, &[len, cap, keys_buf, vals_buf], "map"),
        )
    }

    /// Lower `ExprKind::MapWithSpread(elements)`.
    #[allow(clippy::unused_self)] // Will use self when spread is implemented
    pub(crate) fn lower_map_with_spread(&mut self, _elements: MapElementRange) -> Option<ValueId> {
        tracing::warn!("map spread syntax not yet implemented in V2 codegen");
        None
    }
}
