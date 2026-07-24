//! Shared control-flow emission for delimited aggregate formatting.

use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit the remaining-item loop and closing delimiter for a non-empty sequence.
    pub(super) fn emit_debug_sequence_tail<F>(
        &mut self,
        entry_count: ValueId,
        first_index: ValueId,
        acc_init: ValueId,
        first_block_end: BlockId,
        str_ty: LLVMTypeId,
        function: FunctionId,
        closing_delimiter: &'static str,
        mut append_item: F,
    ) -> Option<(ValueId, BlockId)>
    where
        F: FnMut(&mut Self, ValueId, ValueId) -> Option<ValueId>,
    {
        let loop_header = self.builder.append_block(function, "dbg.seq.hdr");
        let loop_body = self.builder.append_block(function, "dbg.seq.body");
        let close_block = self.builder.append_block(function, "dbg.seq.close");

        let needs_loop = self
            .builder
            .icmp_sgt(entry_count, first_index, "dbg.seq.has_tail");
        self.builder.cond_br(needs_loop, loop_header, close_block);

        self.builder.position_at_end(loop_header);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let index_phi = self.builder.phi(i64_ty, "dbg.seq.index");
        let acc_phi = self.builder.phi(str_ty, "dbg.seq.acc");
        let has_more = self
            .builder
            .icmp_slt(index_phi, entry_count, "dbg.seq.has_more");
        self.builder.cond_br(has_more, loop_body, close_block);

        self.builder.position_at_end(loop_body);
        let new_acc = append_item(self, index_phi, acc_phi)?;
        let next_index = self.builder.add(index_phi, first_index, "dbg.seq.next");
        let body_end = self.builder.current_block()?;
        self.builder.br(loop_header);

        self.builder.add_phi_incoming(
            index_phi,
            &[(first_index, first_block_end), (next_index, body_end)],
        );
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_block_end), (new_acc, body_end)]);

        self.builder.position_at_end(close_block);
        let close_acc = self.builder.phi(str_ty, "dbg.seq.close.acc");
        self.builder.add_phi_incoming(
            close_acc,
            &[(acc_init, first_block_end), (acc_phi, loop_header)],
        );
        let suffix = self.emit_literal_ori_str(closing_delimiter)?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_block_end = self.builder.current_block()?;

        Some((result, close_block_end))
    }
}
