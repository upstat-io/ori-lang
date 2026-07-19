//! The shared panic-block carrier for every checked-op panic site.

use inkwell::basic_block::BasicBlock;

use crate::codegen::ir_builder::IrBuilder;

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Emit a panic block: panic via `ori_panic_cstr(msg)`, then terminate.
    ///
    /// THE single `ori_panic_cstr` carrier for every checked-op panic site.
    /// Positions the builder at `block`, emits the panic, and adds a
    /// terminator. Does NOT reposition the builder after — caller positions at
    /// the next block.
    ///
    /// When a same-frame catch landing pad is in scope
    /// (`self.builder.catch_unwind_target` is `Some`), the panic is emitted as
    /// `invoke @ori_panic_cstr → [normal: a fresh unreachable block, unwind:
    /// catch landing pad]` so the in-frame `catch(expr:)` recovers the panic
    /// instead of aborting. Otherwise it stays `call` + `unreachable` — the
    /// uncaught path that bubbles to the runtime top-level handler.
    pub(super) fn emit_panic_block(&mut self, block: BasicBlock<'ctx>, msg: &str, label: &str) {
        self.builder.position_at_end(block);
        let msg_id = self.build_global_string_ptr(msg, label);
        let panic_fn_id = self.runtime_fn("ori_panic_cstr");

        if let Some(catch_target) = self.catch_unwind_target {
            // Caught path: invoke so the unwind reaches the catch landing pad.
            // The normal dest is a fresh unreachable block — ori_panic_cstr
            // never returns, but `invoke` requires a normal successor.
            let func_id = self.current_function.expect("no current function");
            let func_llvm = self.arena.get_function(func_id);
            let normal_bb = self
                .scx
                .llcx
                .append_basic_block(func_llvm, &format!("{label}.unreachable"));
            let normal_id = self.arena.push_block(normal_bb);
            // `invoke` builds at the current insertion point (already positioned
            // at `block`). Returns None for void `ori_panic_cstr` — ignored.
            self.invoke(panic_fn_id, &[msg_id], normal_id, catch_target, "");
            self.builder.position_at_end(normal_bb);
            self.builder.build_unreachable().expect("unreachable");
        } else {
            // Uncaught path: plain call + unreachable (bubbles to the runtime
            // top-level handler / abort).
            self.call(panic_fn_id, &[msg_id], "");
            self.builder.build_unreachable().expect("unreachable");
        }
    }
}
