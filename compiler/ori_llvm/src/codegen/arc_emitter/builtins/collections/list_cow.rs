//! COW (Copy-on-Write) mutation codegen for lists.
//!
//! All list mutation methods use COW semantics: when the list is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each method returns a `{i64 len, i64 cap, ptr data}` struct.

mod scalar_update;
mod transforms;

use ori_arc::CowMode;
use ori_ir::{FIELD_DATA, FIELD_LEN};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{FunctionId, ValueId};

use self::scalar_update::ScalarUpdatedArgs;
use super::super::super::ArcIrEmitter;

/// Physical storage available to a list mutation receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YieldReceiverStorage {
    /// Runtime-managed heap storage.
    Runtime,
    /// Stack storage retaining a runtime-compatible header.
    ManagedStack,
    /// Compact stack storage without a runtime allocation.
    CompactStack,
}

#[derive(Clone, Copy)]
enum IndexedListCowOperation {
    Set,
    Insert,
}

impl YieldReceiverStorage {
    const fn is_stack(self) -> bool {
        match self {
            Self::Runtime => false,
            Self::ManagedStack | Self::CompactStack => true,
        }
    }
}

impl IndexedListCowOperation {
    const fn runtime_symbol(self) -> &'static str {
        match self {
            Self::Set => "ori_list_set_cow",
            Self::Insert => "ori_list_insert_cow",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Insert => "insert",
        }
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    fn emit_list_cow_call(
        &mut self,
        func_id: FunctionId,
        label: &str,
        mut args: Vec<ValueId>,
    ) -> Option<ValueId> {
        let list_struct_ty = self.list_struct_type();
        let out =
            self.builder
                .create_entry_alloca(self.current_function, "list.cow.out", list_struct_ty);
        args.push(out);
        self.emit_rt_call(func_id, &args, label);
        Some(self.builder.load(list_struct_ty, out, "list.cow.val"))
    }

    /// Emit `list.push(x)` — COW push returning the (possibly mutated) list.
    ///
    /// Fast path (unique + capacity): appends in place, O(1).
    /// Slow path (shared): copies to new buffer, O(n).
    #[must_use = "the absence of a value must be handled"]
    pub(crate) fn emit_list_push_cow(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
        receiver_returned: bool,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_push_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let result = self.emit_list_cow_call(
            func_id,
            "push",
            vec![
                data_ptr,
                len,
                cap,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )?;
        // INVARIANT: Only returned receivers own the element keep-alive credit.
        let header_store_disabled = push_result_elem_header_store_disabled();
        if !receiver_returned || header_store_disabled {
            return Some(result);
        }
        let result_data = self
            .builder
            .extract_value(result, FIELD_DATA, "push.data")?;
        let result_len = self.builder.extract_value(result, FIELD_LEN, "push.len")?;
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec, &[result_data, elem_dec_fn], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder
            .call(store_count, &[result_data, result_len], "");
        Some(result)
    }

    /// Emit `list.set(index, value)` — COW index assignment returning modified list.
    ///
    /// Fast path (unique): overwrites element at index in place.
    /// Slow path (shared): copies buffer, overwrites target index.
    #[must_use = "the absence of a value must be handled"]
    pub(crate) fn emit_list_set_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_indexed_list_cow(
            receiver,
            index,
            elem,
            elem_ty,
            cow_mode,
            list_ty,
            IndexedListCowOperation::Set,
        )
    }

    fn emit_indexed_list_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
        operation: IndexedListCowOperation,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(operation.runtime_symbol());
        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, &format!("{}.elem", operation.label()));
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            operation.label(),
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emits COW replacement for `list.updated(key, value)`. Unique lists update
    /// in place; shared lists copy first. The runtime panics on out-of-bounds keys
    /// and consumes the value's owned reference.
    #[must_use = "the absence of a value must be handled"]
    pub(crate) fn emit_list_updated_cow(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: CowMode,
        list_ty: Idx,
        receiver_storage: YieldReceiverStorage,
        negated_same_index: bool,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_updated_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let bool_elem = self.pool.tag(self.pool.resolve_fully(elem_ty)) == Tag::Bool;
        if bool_elem
            && receiver_storage.is_stack()
            && cow_mode == CowMode::StaticUnique
            && negated_same_index
        {
            let elem_llvm_ty = self
                .builder
                .register_type(self.builder.scx().type_i8().into());

            let elem_ptr =
                self.builder
                    .gep(elem_llvm_ty, data_ptr, &[key], "updated.toggle.elem_ptr");

            let old = self
                .builder
                .load(elem_llvm_ty, elem_ptr, "updated.toggle.old");
            let one = self.builder.const_i8(1);
            let toggled = self.builder.xor(old, one, "updated.toggle.value");
            self.builder.store(toggled, elem_ptr);
            return Some(receiver);
        }
        // Why: Direct scalar overwrite avoids runtime calls; unsafe shapes retain COW checks.
        if self.classifier.is_scalar(elem_ty) {
            return self.emit_scalar_list_updated_cow(&ScalarUpdatedArgs {
                receiver,
                key,
                elem,
                data_ptr,
                len,
                cap,
                elem_ty,
                cow_mode,
                list_ty,
                receiver_storage,
                func_id,
            });
        }

        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "updated.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);
        let dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let cow_mode = self.builder.const_i32(cow_mode_code(cow_mode));

        self.emit_list_cow_call(
            func_id,
            "updated",
            vec![
                data_ptr,
                len,
                cap,
                key,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                dec_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.insert(index, value)` — COW insert returning modified list.
    ///
    /// Fast path (unique + capacity): memmove + write in place.
    /// Slow path (shared): new allocation with element inserted.
    #[must_use = "the absence of a value must be handled"]
    pub(crate) fn emit_list_insert_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_indexed_list_cow(
            receiver,
            index,
            elem,
            elem_ty,
            cow_mode,
            list_ty,
            IndexedListCowOperation::Insert,
        )
    }

    /// Emit `list.prepend(value)` through the canonical index-zero insert path.
    #[must_use = "the absence of a value must be handled"]
    pub(crate) fn emit_list_prepend_cow(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let index = self.builder.const_i64(0);
        self.emit_list_insert_cow(receiver, index, elem, elem_ty, cow_mode, list_ty)
    }

    /// Emit `list.remove(index)` — COW remove returning modified list.
    ///
    /// Fast path (unique): memmove shift left in place.
    /// Slow path (shared): new allocation without the removed element.
    #[must_use = "the absence of a value must be handled"]
    pub(crate) fn emit_list_remove_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_remove_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "remove",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }
}

// Env: ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE — skips result metadata stores, debug-only.
fn push_result_elem_header_store_disabled() -> bool {
    let disabled = std::env::var_os("ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE").is_some();
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE",
            effect = "skip result-buffer element destructor metadata stores",
            "ablation toggle fired"
        );
    }
    disabled
}

const fn cow_mode_code(mode: CowMode) -> i32 {
    match mode {
        CowMode::Dynamic => 0,
        CowMode::StaticUnique => 1,
        CowMode::StaticShared => 2,
    }
}

#[cfg(test)]
mod tests;
