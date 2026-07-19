//! Reused collection-buffer construction.

use ori_arc::ir::CtorKind;
use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a `CollectionReuse` instruction.
    ///
    /// Calls `ori_list_reset_buffer` to either reuse the old buffer (if
    /// uniquely owned) or allocate fresh (if shared). Then stores new
    /// elements and builds the result struct.
    pub(super) fn emit_collection_reuse(
        &mut self,
        old_var: ori_arc::ir::ArcVarId,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ori_arc::ir::ArcVarId],
    ) -> ValueId {
        let old_val = self.var(old_var);
        let llvm_ty = self.resolve_type(ty);
        let new_len = args.len();

        // Determine element type from the collection type.
        let type_info = self.type_info.get(ty);
        let elem_idx = match (ctor, &type_info) {
            (CtorKind::ListLiteral, TypeInfo::List { element })
            | (CtorKind::SetLiteral, TypeInfo::Set { element }) => *element,
            _ => unreachable!(
                "collection reuse TypeInfo mismatch: ctor={ctor:?}, info={type_info:?}"
            ),
        };

        // Narrowed element type/size for collection reuse.
        let collection_idx = self.pool.resolve_fully(ty);
        let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_idx);
        let elem_size = self.collection_elem_size(collection_idx, elem_idx);

        // Extract old {len, cap, data} from old_var.
        let Some((old_data, old_len, old_cap)) = self.extract_collection_fields(
            old_val,
            "reuse.old_data",
            "reuse.old_len",
            "reuse.old_cap",
        ) else {
            panic!("CollectionReuse input must have the canonical collection layout");
        };

        // Build call args for ori_list_reset_buffer.
        let new_len_val = self.builder.const_i64(new_len as i64);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_idx);

        // Alloca for out_cap (caller-provided output parameter).
        let i64_ty = self.builder.i64_type();
        let out_cap_alloca = self.builder.alloca(i64_ty, "reuse.out_cap");

        // Call ori_list_reset_buffer.
        let reset_fn = self.builder.runtime_fn("ori_list_reset_buffer");
        let Some(new_data) = self.builder.call(
            reset_fn,
            &[
                old_data,
                old_len,
                old_cap,
                new_len_val,
                elem_size_val,
                elem_dec_fn,
                out_cap_alloca,
            ],
            "reuse.data",
        ) else {
            panic!("ori_list_reset_buffer must return a data pointer");
        };

        // Store each new element into the returned buffer.
        // For narrowed collections, trunc to narrow width before storing.
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        for (i, &val) in arg_vals.iter().enumerate() {
            let idx = self.builder.const_i64(i as i64);
            let elem_ptr = self
                .builder
                .gep(elem_llvm_ty, new_data, &[idx], "reuse.elem_ptr");
            let store_val =
                self.trunc_for_narrowed_collection_element(val, collection_idx, "reuse.elem.trunc");
            self.builder.store(store_val, elem_ptr);
        }

        // Store elem_dec_fn and elem_count in the new buffer's RC header.
        // ori_list_reset_buffer does NOT propagate internally — codegen
        // handles it externally after the reset returns.
        let store_dec_fn = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec_fn, &[new_data, elem_dec_fn], "");
        let store_count_fn = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder
            .call(store_count_fn, &[new_data, new_len_val], "");

        // Load the output capacity.
        let result_cap = self.builder.load(i64_ty, out_cap_alloca, "reuse.cap");

        // Build result struct: {i64 len, i64 cap, ptr data}
        self.builder
            .build_struct(llvm_ty, &[new_len_val, result_cap, new_data], "reuse.list")
    }
}
