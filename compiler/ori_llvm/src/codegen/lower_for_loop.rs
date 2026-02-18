//! For-loop lowering for V2 codegen.
//!
//! Handles for-loops over all iterable types: Range, List, Str, Option, Set, Map.
//! Supports both `for x in iter do body` (side effects) and
//! `for x in iter yield body` (list collection).
//!
//! Extracted from `lower_control_flow.rs` to keep files under the 500-line limit.

use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_types::Idx;

use super::expr_lowerer::{ExprLowerer, LoopContext};
use super::type_info::TypeInfo;
use super::value_id::{LLVMTypeId, ValueId};

/// Temporary state for for-yield list construction.
pub(crate) struct YieldContext {
    /// Pointer to the allocated data buffer.
    data_ptr: ValueId,
    /// Alloca holding the current write index (mutable counter).
    write_idx: ValueId,
    /// Allocated capacity of the buffer.
    cap: ValueId,
    /// LLVM type of each element (for GEP sizing).
    elem_llvm_ty: LLVMTypeId,
}

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // For-loop dispatch
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::For { binding, iter, guard, body, is_yield }`.
    pub(crate) fn lower_for(
        &mut self,
        binding: Name,
        iter: CanId,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        let iter_val = self.lower(iter)?;
        let iter_type = self.expr_type(iter);
        let type_info = self.type_info.get(iter_type);

        match type_info {
            TypeInfo::Range => {
                self.lower_for_range(binding, iter_val, guard, body, is_yield, expr_id)
            }
            TypeInfo::List { element } => self.lower_for_data_array(
                binding, iter_val, iter_type, element, guard, body, is_yield, expr_id, "forlist",
            ),
            TypeInfo::Str => self.lower_for_str(binding, iter_val, guard, body, is_yield, expr_id),
            TypeInfo::Option { inner } => {
                self.lower_for_option(binding, iter_val, inner, guard, body, is_yield, expr_id)
            }
            TypeInfo::Set { element } => self.lower_for_data_array(
                binding, iter_val, iter_type, element, guard, body, is_yield, expr_id, "forset",
            ),
            TypeInfo::Map { key, value } => self.lower_for_map(
                binding, iter_val, key, value, guard, body, is_yield, expr_id,
            ),
            _ => {
                tracing::warn!(?iter_type, ?type_info, "for-loop over unsupported type");
                self.builder.record_codegen_error();
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // For-loop over Range
    // -----------------------------------------------------------------------

    /// For-loop over a range: `{i64 start, i64 end, i1 inclusive}`.
    fn lower_for_range(
        &mut self,
        binding: Name,
        range_val: ValueId,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        let start = self.builder.extract_value(range_val, 0, "range.start")?;
        let end = self.builder.extract_value(range_val, 1, "range.end")?;
        let inclusive = self.builder.extract_value(range_val, 2, "range.incl")?;

        let yield_ctx = if is_yield {
            Some(self.setup_yield_context_from_range(start, end, inclusive, expr_id)?)
        } else {
            None
        };

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "for.header");
        let body_bb = self.builder.append_block(self.current_function, "for.body");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "for.latch");
        let exit_bb = self.builder.append_block(self.current_function, "for.exit");

        self.builder.br(header_bb);

        // Header: phi for induction variable + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let i_phi = self.builder.phi(i64_ty, "for.i");
        self.builder.add_phi_incoming(i_phi, &[(start, entry_bb)]);

        // Bounds check: i < end (or i <= end if inclusive)
        let cmp_lt = self.builder.icmp_slt(i_phi, end, "for.lt");
        let cmp_eq = self.builder.icmp_eq(i_phi, end, "for.eq");
        let incl_ok = self.builder.and(inclusive, cmp_eq, "for.incl_ok");
        let in_bounds = self.builder.or(cmp_lt, incl_ok, "for.inbounds");

        // Apply guard if present
        if guard.is_valid() {
            self.builder.cond_br(in_bounds, body_bb, exit_bb);
            self.builder.position_at_end(body_bb);

            self.scope.bind_immutable(binding, i_phi);
            let guard_val = self.lower(guard)?;

            let guarded_body_bb = self
                .builder
                .append_block(self.current_function, "for.guarded");
            self.builder.cond_br(guard_val, guarded_body_bb, latch_bb);
            self.builder.position_at_end(guarded_body_bb);
        } else {
            self.builder.cond_br(in_bounds, body_bb, exit_bb);
            self.builder.position_at_end(body_bb);

            self.scope.bind_immutable(binding, i_phi);
        }

        // Save/set loop context — continue goes to latch
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: latch_bb,
            break_values: Vec::new(),
        });

        // Body
        let body_val = self.lower(body);

        // Yield: store body value into the output list
        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

        if !self.builder.current_block_terminated() {
            self.builder.br(latch_bb);
        }

        // Latch: increment and back-edge
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next = self.builder.add(i_phi, one, "for.next");
        self.builder.add_phi_incoming(i_phi, &[(next, latch_bb)]);
        self.builder.br(header_bb);

        // Restore loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Exit
        self.builder.position_at_end(exit_bb);

        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

        self.build_for_result(&loop_ctx, "for.result")
    }

    // -----------------------------------------------------------------------
    // For-loop over data-array types (List, Set)
    // -----------------------------------------------------------------------

    /// For-loop over a data-array collection: `{i64 len, i64 cap, ptr data}`.
    ///
    /// Used for both List and Set, which share the same memory layout.
    /// The `label` parameter gives unique LLVM block names per type.
    #[expect(
        clippy::too_many_arguments,
        reason = "for-loop lowering needs all loop components + type info"
    )]
    fn lower_for_data_array(
        &mut self,
        binding: Name,
        collection_val: ValueId,
        collection_type: Idx,
        elem_idx: Idx,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
        label: &str,
    ) -> Option<ValueId> {
        // {i64 len, i64 cap, ptr data}
        let len = self
            .builder
            .extract_value(collection_val, 0, &format!("{label}.len"))?;
        let data_ptr = self
            .builder
            .extract_value(collection_val, 2, &format!("{label}.data"))?;

        // Resolve element type — fall back to collection's type info if needed
        let resolved_elem = if elem_idx == Idx::NONE {
            match self.type_info.get(collection_type) {
                TypeInfo::List { element } | TypeInfo::Set { element } => element,
                _ => Idx::INT,
            }
        } else {
            elem_idx
        };
        let elem_llvm_ty = self.resolve_type(resolved_elem);

        let yield_ctx = if is_yield {
            Some(self.setup_yield_context_with_capacity(len, expr_id)?)
        } else {
            None
        };

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.header"));
        let body_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.body"));
        let latch_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.latch"));
        let exit_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.exit"));

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx_phi = self.builder.phi(i64_ty, &format!("{label}.idx"));
        self.builder.add_phi_incoming(idx_phi, &[(zero, entry_bb)]);

        let in_bounds = self
            .builder
            .icmp_slt(idx_phi, len, &format!("{label}.inbounds"));
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: load element, bind, execute body
        self.builder.position_at_end(body_bb);
        let elem_ptr = self.builder.gep(
            elem_llvm_ty,
            data_ptr,
            &[idx_phi],
            &format!("{label}.elem_ptr"),
        );
        let elem_val = self
            .builder
            .load(elem_llvm_ty, elem_ptr, &format!("{label}.elem"));

        // Handle guard
        if guard.is_valid() {
            self.scope.bind_immutable(binding, elem_val);
            let guard_val = self.lower(guard);
            if let Some(gv) = guard_val {
                let guarded_bb = self
                    .builder
                    .append_block(self.current_function, &format!("{label}.guarded"));
                self.builder.cond_br(gv, guarded_bb, latch_bb);
                self.builder.position_at_end(guarded_bb);
            }
        } else {
            self.scope.bind_immutable(binding, elem_val);
        }

        // Save/set loop context
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: latch_bb,
            break_values: Vec::new(),
        });

        let body_val = self.lower(body);

        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

        if !self.builder.current_block_terminated() {
            self.builder.br(latch_bb);
        }

        // Latch
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, &format!("{label}.next"));
        self.builder
            .add_phi_incoming(idx_phi, &[(next_idx, latch_bb)]);
        self.builder.br(header_bb);

        // Restore loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Exit
        self.builder.position_at_end(exit_bb);

        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

        self.build_for_result(&loop_ctx, &format!("{label}.result"))
    }

    // -----------------------------------------------------------------------
    // For-loop over Str (UTF-8 character iteration)
    // -----------------------------------------------------------------------

    /// For-loop over a string: iterates UTF-8 characters via `ori_str_next_char`.
    ///
    /// String layout: `{i64 len, ptr data}`. Each iteration decodes the next
    /// UTF-8 codepoint and advances the byte offset.
    fn lower_for_str(
        &mut self,
        binding: Name,
        str_val: ValueId,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        // Extract string components: {i64 len, ptr data}
        let len = self.builder.extract_value(str_val, 0, "forstr.len")?;
        let data_ptr = self.builder.extract_value(str_val, 1, "forstr.data")?;

        // Yield setup: byte length is an upper bound on char count
        let yield_ctx = if is_yield {
            Some(self.setup_yield_context_with_capacity(len, expr_id)?)
        } else {
            None
        };

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "forstr.header");
        let body_bb = self
            .builder
            .append_block(self.current_function, "forstr.body");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "forstr.latch");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "forstr.exit");

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: byte_offset phi + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let offset_phi = self.builder.phi(i64_ty, "forstr.offset");
        self.builder
            .add_phi_incoming(offset_phi, &[(zero, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(offset_phi, len, "forstr.inbounds");
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: call ori_str_next_char to decode next character
        self.builder.position_at_end(body_bb);

        // Declare/get ori_str_next_char(data: ptr, len: i64, offset: i64) -> {i32, i64}
        let ptr_ty = self.builder.ptr_type();
        let ret_ty = {
            let i32_basic = self.builder.scx().type_i32();
            let i64_basic = self.builder.scx().type_i64();
            let struct_ty = self
                .builder
                .scx()
                .type_struct(&[i32_basic.into(), i64_basic.into()], false);
            self.builder.register_type(struct_ty.into())
        };
        let next_char_fn = self.builder.get_or_declare_function(
            "ori_str_next_char",
            &[ptr_ty, i64_ty, i64_ty],
            ret_ty,
        );
        let char_result = self.builder.call(
            next_char_fn,
            &[data_ptr, len, offset_phi],
            "forstr.char_result",
        )?;

        // Extract codepoint (i32) and next_offset (i64)
        let codepoint = self
            .builder
            .extract_value(char_result, 0, "forstr.codepoint")?;
        let next_offset = self
            .builder
            .extract_value(char_result, 1, "forstr.next_offset")?;

        // Handle guard
        if guard.is_valid() {
            self.scope.bind_immutable(binding, codepoint);
            let guard_val = self.lower(guard);
            if let Some(gv) = guard_val {
                let guarded_bb = self
                    .builder
                    .append_block(self.current_function, "forstr.guarded");
                self.builder.cond_br(gv, guarded_bb, latch_bb);
                self.builder.position_at_end(guarded_bb);
            }
        } else {
            self.scope.bind_immutable(binding, codepoint);
        }

        // Loop context
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: latch_bb,
            break_values: Vec::new(),
        });

        let body_val = self.lower(body);

        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

        if !self.builder.current_block_terminated() {
            self.builder.br(latch_bb);
        }

        // Latch: advance byte offset
        self.builder.position_at_end(latch_bb);
        self.builder
            .add_phi_incoming(offset_phi, &[(next_offset, latch_bb)]);
        self.builder.br(header_bb);

        // Restore loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Exit
        self.builder.position_at_end(exit_bb);

        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

        self.build_for_result(&loop_ctx, "forstr.result")
    }

    // -----------------------------------------------------------------------
    // For-loop over Option (0-or-1 element)
    // -----------------------------------------------------------------------

    /// For-loop over an Option: executes body once if Some, skips if None.
    ///
    /// Option layout: `{i8 tag, T payload}`. Tag 0 = None, 1 = Some.
    /// This is not a real loop — it's conditional execution.
    #[expect(
        clippy::too_many_arguments,
        reason = "for-loop lowering needs all loop components + type info"
    )]
    fn lower_for_option(
        &mut self,
        binding: Name,
        option_val: ValueId,
        inner: Idx,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        // Extract tag and payload: {i8 tag, resolve(T) payload}
        // TypeLayoutResolver resolves Option to {i8, resolve(T)}, so the
        // extracted payload already has the correct inner type.
        let tag = self.builder.extract_value(option_val, 0, "foropt.tag")?;
        let elem_val = self
            .builder
            .extract_value(option_val, 1, "foropt.payload")?;
        let _ = inner; // Used by dispatch, payload type already correct

        // Check if Some (tag != 0)
        let zero_tag = self.builder.const_i8(0);
        let is_some = self.builder.icmp_ne(tag, zero_tag, "foropt.is_some");

        // Yield setup: capacity is 0 or 1
        let yield_ctx = if is_yield {
            let one = self.builder.const_i64(1);
            let zero = self.builder.const_i64(0);
            let cap = self.builder.select(is_some, one, zero, "foropt.cap");
            Some(self.setup_yield_context_with_capacity(cap, expr_id)?)
        } else {
            None
        };

        let body_bb = self
            .builder
            .append_block(self.current_function, "foropt.body");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "foropt.exit");

        self.builder.cond_br(is_some, body_bb, exit_bb);

        // Body: bind payload and execute
        self.builder.position_at_end(body_bb);

        // Handle guard
        if guard.is_valid() {
            self.scope.bind_immutable(binding, elem_val);
            let guard_val = self.lower(guard);
            if let Some(gv) = guard_val {
                let guarded_bb = self
                    .builder
                    .append_block(self.current_function, "foropt.guarded");
                self.builder.cond_br(gv, guarded_bb, exit_bb);
                self.builder.position_at_end(guarded_bb);
            }
        } else {
            self.scope.bind_immutable(binding, elem_val);
        }

        // No loop context needed — Option isn't a real loop, break/continue
        // would be caught by the type checker

        let body_val = self.lower(body);

        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

        if !self.builder.current_block_terminated() {
            self.builder.br(exit_bb);
        }

        // Exit
        self.builder.position_at_end(exit_bb);

        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

        Some(self.builder.const_i64(0))
    }

    // -----------------------------------------------------------------------
    // For-loop over Map
    // -----------------------------------------------------------------------

    /// For-loop over a map: iterates `(key, value)` tuples.
    ///
    /// Map layout: `{i64 len, i64 cap, ptr keys, ptr vals}`.
    /// Each iteration loads key[i] and val[i], builds a tuple, and binds it.
    #[expect(
        clippy::too_many_arguments,
        reason = "for-loop lowering needs all loop components + type info"
    )]
    fn lower_for_map(
        &mut self,
        binding: Name,
        map_val: ValueId,
        key_idx: Idx,
        value_idx: Idx,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        // Map = {i64 len, i64 cap, ptr keys, ptr vals}
        let len = self.builder.extract_value(map_val, 0, "formap.len")?;
        let keys_ptr = self.builder.extract_value(map_val, 2, "formap.keys")?;
        let vals_ptr = self.builder.extract_value(map_val, 3, "formap.vals")?;

        let key_llvm_ty = self.resolve_type(key_idx);
        let val_llvm_ty = self.resolve_type(value_idx);

        // Build the tuple type for (key, value)
        let tuple_ty = {
            let key_basic = self.type_resolver.resolve(key_idx);
            let val_basic = self.type_resolver.resolve(value_idx);
            let struct_ty = self
                .builder
                .scx()
                .type_struct(&[key_basic, val_basic], false);
            self.builder.register_type(struct_ty.into())
        };

        let yield_ctx = if is_yield {
            Some(self.setup_yield_context_with_capacity(len, expr_id)?)
        } else {
            None
        };

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "formap.header");
        let body_bb = self
            .builder
            .append_block(self.current_function, "formap.body");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "formap.latch");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "formap.exit");

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx_phi = self.builder.phi(i64_ty, "formap.idx");
        self.builder.add_phi_incoming(idx_phi, &[(zero, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(idx_phi, len, "formap.inbounds");
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: load key[i] and val[i], build tuple
        self.builder.position_at_end(body_bb);

        let key_ptr = self
            .builder
            .gep(key_llvm_ty, keys_ptr, &[idx_phi], "formap.key_ptr");
        let key_val = self.builder.load(key_llvm_ty, key_ptr, "formap.key");

        let val_ptr = self
            .builder
            .gep(val_llvm_ty, vals_ptr, &[idx_phi], "formap.val_ptr");
        let val_val = self.builder.load(val_llvm_ty, val_ptr, "formap.val");

        // Build (key, value) tuple struct
        let tuple_val = self
            .builder
            .build_struct(tuple_ty, &[key_val, val_val], "formap.entry");

        // Handle guard
        if guard.is_valid() {
            self.scope.bind_immutable(binding, tuple_val);
            let guard_val = self.lower(guard);
            if let Some(gv) = guard_val {
                let guarded_bb = self
                    .builder
                    .append_block(self.current_function, "formap.guarded");
                self.builder.cond_br(gv, guarded_bb, latch_bb);
                self.builder.position_at_end(guarded_bb);
            }
        } else {
            self.scope.bind_immutable(binding, tuple_val);
        }

        // Loop context
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: latch_bb,
            break_values: Vec::new(),
        });

        let body_val = self.lower(body);

        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

        if !self.builder.current_block_terminated() {
            self.builder.br(latch_bb);
        }

        // Latch
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, "formap.next");
        self.builder
            .add_phi_incoming(idx_phi, &[(next_idx, latch_bb)]);
        self.builder.br(header_bb);

        // Restore loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Exit
        self.builder.position_at_end(exit_bb);

        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

        self.build_for_result(&loop_ctx, "formap.result")
    }

    // -----------------------------------------------------------------------
    // For-yield helpers
    // -----------------------------------------------------------------------

    /// Compute yield capacity from range bounds.
    fn setup_yield_context_from_range(
        &mut self,
        start: ValueId,
        end: ValueId,
        inclusive: ValueId,
        expr_id: CanId,
    ) -> Option<YieldContext> {
        // Capacity: end - start + (inclusive ? 1 : 0), clamped to 0
        let diff = self.builder.sub(end, start, "yield.diff");
        let one = self.builder.const_i64(1);
        let zero = self.builder.const_i64(0);
        let incl_extra = self
            .builder
            .select(inclusive, one, zero, "yield.incl_extra");
        let raw_cap = self.builder.add(diff, incl_extra, "yield.raw_cap");
        let is_neg = self.builder.icmp_slt(raw_cap, zero, "yield.neg");
        let cap = self.builder.select(is_neg, zero, raw_cap, "yield.cap");

        self.setup_yield_context_with_capacity(cap, expr_id)
    }

    /// Setup yield context with a pre-computed capacity value.
    fn setup_yield_context_with_capacity(
        &mut self,
        cap: ValueId,
        expr_id: CanId,
    ) -> Option<YieldContext> {
        let result_type = self.expr_type(expr_id);
        let type_info = self.type_info.get(result_type);
        let elem_idx = match &type_info {
            TypeInfo::List { element } => *element,
            _ => ori_types::Idx::INT,
        };
        let elem_llvm_ty = self.resolve_type(elem_idx);
        let elem_size = self.type_info.get(elem_idx).size().unwrap_or(8);

        // Allocate raw data buffer: ori_list_alloc_data(capacity, elem_size)
        let esize = self.builder.const_i64(elem_size as i64);
        let i64_ty = self.builder.i64_type();
        let i64_ty2 = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let alloc_data =
            self.builder
                .get_or_declare_function("ori_list_alloc_data", &[i64_ty, i64_ty2], ptr_ty);
        let data_ptr = self.builder.call(alloc_data, &[cap, esize], "yield.data")?;

        // Write index alloca at function entry
        let i64_llvm = self.builder.i64_type();
        let write_idx =
            self.builder
                .create_entry_alloca(self.current_function, "yield.widx", i64_llvm);
        let zero = self.builder.const_i64(0);
        self.builder.store(zero, write_idx);

        Some(YieldContext {
            data_ptr,
            write_idx,
            cap,
            elem_llvm_ty,
        })
    }

    /// Store a body value into the yield output list and increment write index.
    fn emit_yield_store(&mut self, yc: &YieldContext, body_val: ValueId) {
        let i64_ty = self.builder.i64_type();
        let widx = self.builder.load(i64_ty, yc.write_idx, "yield.widx_cur");

        let elem_ptr = self
            .builder
            .gep(yc.elem_llvm_ty, yc.data_ptr, &[widx], "yield.elem_ptr");
        self.builder.store(body_val, elem_ptr);

        let one = self.builder.const_i64(1);
        let next_widx = self.builder.add(widx, one, "yield.widx_next");
        self.builder.store(next_widx, yc.write_idx);
    }

    /// Build the final list struct from yield context after the loop completes.
    fn finish_yield_list(&mut self, yc: &YieldContext, expr_id: CanId) -> Option<ValueId> {
        let i64_ty = self.builder.i64_type();
        let final_len = self.builder.load(i64_ty, yc.write_idx, "yield.final_len");
        let result_type = self.expr_type(expr_id);
        let list_ty = self.resolve_type(result_type);
        Some(
            self.builder
                .build_struct(list_ty, &[final_len, yc.cap, yc.data_ptr], "yield.list"),
        )
    }

    /// Build the result value for a completed for-loop (non-yield).
    fn build_for_result(&mut self, loop_ctx: &LoopContext, label: &str) -> Option<ValueId> {
        if loop_ctx.break_values.is_empty() {
            Some(self.builder.const_i64(0))
        } else {
            let unit_ty = self.builder.unit_type();
            self.builder
                .phi_from_incoming(unit_ty, &loop_ctx.break_values, label)
        }
    }
}
