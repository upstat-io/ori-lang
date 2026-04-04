//! Dead unwind block detection for the ARC IR emitter.
//!
//! With nounwind analysis, `Invoke` terminators calling known-nounwind functions
//! are downgraded to `call` + `br`, making their unwind blocks unreachable.
//! This module detects those dead blocks before LLVM block pre-creation so they
//! can be skipped entirely.

use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_arc::RcStrategy;
use rustc_hash::FxHashSet;

use super::ArcIrEmitter;

/// Result of dead unwind detection: which blocks are dead (skip entirely)
/// and which are live (need landing pad / cleanup pad).
pub(super) struct DeadUnwindResult {
    /// Blocks that are only reachable via nounwind invokes — no LLVM block needed.
    pub dead: FxHashSet<usize>,
    /// Blocks that have real cleanup instructions — need EH prelude.
    pub live: FxHashSet<usize>,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Detect dead and live unwind blocks in the given function.
    ///
    /// An unwind block is "live" only when:
    /// - The callee is not proven nounwind
    /// - The callee is not intercepted by a builtin handler (format calls,
    ///   prelude builtins, builtin methods all emit `call` regardless of mode)
    /// - The unwind block has actual cleanup instructions (`RcDec` etc.)
    ///
    /// All other invoke unwind targets are dead — no LLVM block is created.
    pub(super) fn detect_dead_unwind_blocks(&self, func: &ArcFunction) -> DeadUnwindResult {
        let mut all_invoke_unwind = FxHashSet::default();
        let mut live = FxHashSet::default();

        for block in &func.blocks {
            match &block.terminator {
                ArcTerminator::Invoke {
                    unwind,
                    func: callee,
                    args,
                    ..
                } => {
                    all_invoke_unwind.insert(unwind.index());

                    let ub = &func.blocks[unwind.index()];
                    let has_cleanup = has_effective_cleanup(ub, func);
                    let callee_uses_call = self.ctx.nounwind_functions.contains(callee)
                        || self.callee_will_be_intercepted(*callee, args, func);
                    if !callee_uses_call && has_cleanup {
                        live.insert(unwind.index());
                    }
                }
                ArcTerminator::InvokeIndirect { unwind, .. } => {
                    all_invoke_unwind.insert(unwind.index());

                    let ub = &func.blocks[unwind.index()];
                    let has_cleanup = has_effective_cleanup(ub, func);
                    // Indirect calls can't be statically resolved —
                    // conservatively assume they may unwind.
                    if has_cleanup {
                        live.insert(unwind.index());
                    }
                }
                _ => {}
            }
        }

        let dead = all_invoke_unwind.difference(&live).copied().collect();

        DeadUnwindResult { dead, live }
    }
}

/// Assert that dead unwind blocks are not reachable via non-Invoke edges.
///
/// If a Jump/Branch/Switch targets a dead block, the detection is broken.
pub(super) fn debug_assert_dead_unwind_unreachable(func: &ArcFunction, dead: &FxHashSet<usize>) {
    debug_assert!(
        {
            let mut ok = true;
            for block in &func.blocks {
                let non_invoke_targets: Vec<usize> = match &block.terminator {
                    ArcTerminator::Jump { target, .. } => vec![target.index()],
                    ArcTerminator::Branch {
                        then_block,
                        else_block,
                        ..
                    } => {
                        vec![then_block.index(), else_block.index()]
                    }
                    ArcTerminator::Switch { cases, default, .. } => {
                        let mut t: Vec<usize> = cases.iter().map(|(_, b)| b.index()).collect();
                        t.push(default.index());
                        t
                    }
                    ArcTerminator::Invoke { normal, .. }
                    | ArcTerminator::InvokeIndirect { normal, .. } => vec![normal.index()],
                    _ => vec![],
                };
                for target in non_invoke_targets {
                    if dead.contains(&target) {
                        ok = false;
                    }
                }
            }
            ok
        },
        "dead unwind block is reachable via non-Invoke terminator — \
         dead_unwind detection invariant violated"
    );
}

/// Check if an unwind block has cleanup that produces actual LLVM code.
///
/// Returns `false` when all cleanup is dead — e.g., `RcDec` on
/// non-capturing closures (null env pointer → `RcDec` is a no-op).
/// Used by the dead-block pre-scan and by `emit_invoke`'s mode decision.
pub(super) fn has_effective_cleanup(block: &ArcBlock, func: &ArcFunction) -> bool {
    if !matches!(block.terminator, ArcTerminator::Resume) {
        return true;
    }
    if block.body.is_empty() {
        return false;
    }
    // All instructions must be no-op RcDecs for the block to be dead.
    !block.body.iter().all(|instr| {
        if let ArcInstr::RcDec {
            var,
            strategy: RcStrategy::Closure,
        } = instr
        {
            is_non_capturing_closure(func, *var)
        } else {
            false
        }
    })
}

/// Check if a variable is a non-capturing closure (null env pointer).
///
/// Traces through `Let { Var(src) }` copies to find the defining
/// `PartialApply`. Non-capturing closures have empty `args` — their
/// env pointer is null, making `RcDec` a no-op.
fn is_non_capturing_closure(func: &ArcFunction, var: ArcVarId) -> bool {
    let mut current = var;
    // Depth limit to prevent infinite loops on malformed IR.
    for _ in 0..8 {
        let Some(instr) = find_definition(func, current) else {
            return false;
        };
        match instr {
            ArcInstr::PartialApply { args, .. } => return args.is_empty(),
            ArcInstr::Let {
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } => {
                current = *src;
            }
            _ => return false,
        }
    }
    false
}

/// Find the instruction that defines a variable.
fn find_definition(func: &ArcFunction, var: ArcVarId) -> Option<&ArcInstr> {
    for block in &func.blocks {
        for instr in &block.body {
            if instr.defined_var() == Some(var) {
                return Some(instr);
            }
        }
    }
    None
}
