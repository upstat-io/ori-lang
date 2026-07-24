//! LLVM-local callable-name rewrites for lowered lambdas.

use ori_arc::ArcInstr;
use ori_ir::Name;

/// Remap partial-application targets after LLVM assigns callable identities.
pub(super) fn remap_partial_apply_names(func: &mut ori_arc::ArcFunction, renames: &[(Name, Name)]) {
    for block in &mut func.blocks {
        for instr in &mut block.body {
            if let ArcInstr::PartialApply { func, .. } = instr {
                if let Some((_, new)) = renames.iter().find(|(old, _)| *old == *func) {
                    *func = *new;
                }
            }
        }
    }
}
