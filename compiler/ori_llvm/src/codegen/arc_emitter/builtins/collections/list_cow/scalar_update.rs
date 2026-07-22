//! In-place list updates fall back to the runtime when bounds or uniqueness are unproven.

use ori_arc::CowMode;
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

use super::{ArcIrEmitter, YieldReceiverStorage};

/// Inputs shared by the direct and runtime-fallback scalar update paths.
#[derive(Debug)]
pub(super) struct ScalarUpdatedArgs {
    /// Original list value returned by an in-place update.
    pub(super) receiver: ValueId,
    /// Element index to update.
    pub(super) key: ValueId,
    /// Replacement element value.
    pub(super) elem: ValueId,
    /// List element-storage pointer.
    pub(super) data_ptr: ValueId,
    /// Current logical element count.
    pub(super) len: ValueId,
    /// Current physical element capacity.
    pub(super) cap: ValueId,
    /// Ori type of the replacement element.
    pub(super) elem_ty: Idx,
    /// Proven or runtime-tested COW ownership mode.
    pub(super) cow_mode: CowMode,
    /// Ori type of the receiver list.
    pub(super) list_ty: Idx,
    /// Selected receiver storage mechanism.
    pub(super) receiver_storage: YieldReceiverStorage,
    /// Runtime fallback function.
    pub(super) func_id: FunctionId,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Select in-place unique storage or the runtime fallback for a scalar update.
    pub(super) fn emit_scalar_list_updated_cow(
        &mut self,
        args: &ScalarUpdatedArgs,
    ) -> Option<ValueId> {
        let list_struct_ty = self.list_struct_type();
        let out = self.builder.create_entry_alloca(
            self.current_function,
            "list.updated.fallback",
            list_struct_ty,
        );

        let check_unique = self
            .builder
            .append_block(self.current_function, "updated.check_unique");
        let direct = self
            .builder
            .append_block(self.current_function, "updated.direct");
        let fallback = self
            .builder
            .append_block(self.current_function, "updated.fallback");
        let merge = self
            .builder
            .append_block(self.current_function, "updated.merge");

        let zero = self.builder.const_i64(0);
        let in_bounds = self
            .builder
            .icmp_ult(args.key, args.len, "updated.in_bounds");
        let may_update =
            if args.receiver_storage.is_stack() && args.cow_mode == CowMode::StaticUnique {
                in_bounds
            } else {
                let regular = self.builder.icmp_sge(args.cap, zero, "updated.regular");
                self.builder.and(regular, in_bounds, "updated.may_update")
            };
        self.builder.cond_br(may_update, check_unique, fallback);

        self.emit_scalar_updated_uniqueness_branch(args, check_unique, direct, fallback);

        self.builder.position_at_end(direct);
        let collection_idx = self.pool.resolve_fully(args.list_ty);
        let bool_elem = self.pool.tag(self.pool.resolve_fully(args.elem_ty)) == Tag::Bool;
        let elem_llvm_ty = if bool_elem {
            self.builder
                .register_type(self.builder.scx().type_i8().into())
        } else {
            self.collection_elem_llvm_type(collection_idx, args.elem_ty)
        };
        let stored = if bool_elem {
            self.builder
                .zext(args.elem, elem_llvm_ty, "updated.elem.bool")
        } else {
            self.trunc_for_narrowed_collection_element(
                args.elem,
                collection_idx,
                "updated.elem.trunc",
            )
        };
        let dst = self
            .builder
            .gep(elem_llvm_ty, args.data_ptr, &[args.key], "updated.elem_ptr");
        self.builder.store(stored, dst);
        let direct_end = self.builder.current_block().expect("direct update block");
        self.builder.br(merge);

        self.builder.position_at_end(fallback);
        let fallback_value = self.emit_scalar_updated_fallback(args, out, list_struct_ty);
        let fallback_end = self.builder.current_block().expect("fallback update block");
        self.builder.br(merge);

        self.builder.position_at_end(merge);
        let result = self.builder.phi(list_struct_ty, "updated.value");
        self.builder.add_phi_incoming(
            result,
            &[(args.receiver, direct_end), (fallback_value, fallback_end)],
        );
        Some(result)
    }

    fn emit_scalar_updated_uniqueness_branch(
        &mut self,
        args: &ScalarUpdatedArgs,
        check_unique: BlockId,
        direct: BlockId,
        fallback: BlockId,
    ) {
        self.builder.position_at_end(check_unique);
        match args.cow_mode {
            CowMode::StaticUnique => self.builder.br(direct),
            CowMode::StaticShared => self.builder.br(fallback),
            CowMode::Dynamic => {
                let i8_ty = self.builder.i8_type();
                let rc_offset = self.builder.const_i64(-8);
                let rc_ptr = self
                    .builder
                    .gep(i8_ty, args.data_ptr, &[rc_offset], "updated.rc_ptr");
                let i64_ty = self.builder.i64_type();
                let rc = self.builder.load(i64_ty, rc_ptr, "updated.rc");
                let one = self.builder.const_i64(1);
                let unique = self.builder.icmp_eq(rc, one, "updated.unique");
                self.builder.cond_br(unique, direct, fallback);
            }
        }
    }

    fn emit_scalar_updated_fallback(
        &mut self,
        args: &ScalarUpdatedArgs,
        out: ValueId,
        list_struct_ty: LLVMTypeId,
    ) -> ValueId {
        if args.receiver_storage == YieldReceiverStorage::CompactStack {
            let panic_fn = self.builder.runtime_fn("ori_panic_index_out_of_bounds");
            self.emit_rt_call(panic_fn, &[args.key, args.len], "updated.oob");
        } else {
            let elem_ptr = self.elem_to_ptr(args.elem, args.elem_ty, "updated.elem");
            let (elem_size_val, elem_align_val) =
                self.elem_size_and_align(args.elem_ty, Some(args.list_ty));
            let inc_fn = self.get_or_generate_elem_inc_fn(args.elem_ty);
            let dec_fn = self.get_or_generate_elem_dec_fn(args.elem_ty);
            let cow_mode = self.builder.const_i32(args.cow_mode as i32);
            self.emit_rt_call(
                args.func_id,
                &[
                    args.data_ptr,
                    args.len,
                    args.cap,
                    args.key,
                    elem_ptr,
                    elem_size_val,
                    elem_align_val,
                    inc_fn,
                    dec_fn,
                    cow_mode,
                    out,
                ],
                "updated",
            );
        }
        if args.receiver_storage.is_stack() && args.cow_mode == CowMode::StaticUnique {
            args.receiver
        } else {
            self.builder
                .load(list_struct_ty, out, "updated.fallback_value")
        }
    }
}
