//! Shared helpers for reuse emission: projection maps, self-set detection,
//! Set/SetTag instruction building, and variable substitution.

use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, CtorKind};

// Projection map for self-set elimination

/// Maps `(base_var, field_index)` → `projected_var` for projections seen
/// before the death site. Used for self-set elimination.
pub(super) type ProjMap = FxHashMap<(ArcVarId, u32), ArcVarId>;

/// Build a map from `(base_var, field_index)` → `projected_var` for all
/// `Project` instructions in `instrs` that project from `base`.
pub(super) fn build_proj_map(instrs: &[ArcInstr], base: ArcVarId) -> ProjMap {
    let mut map = ProjMap::default();
    for instr in instrs {
        if let ArcInstr::Project {
            dst, value, field, ..
        } = instr
        {
            if *value == base {
                map.insert((base, *field), *dst);
            }
        }
    }
    map
}

/// Check whether writing `value` to `base.field` is a self-set (no-op).
///
/// A Set is a self-set if `value` was obtained via `Project { value: base, field }`.
pub(super) fn is_self_set(base: ArcVarId, field: u32, value: ArcVarId, proj_map: &ProjMap) -> bool {
    proj_map.get(&(base, field)) == Some(&value)
}

/// Build `Set`/`SetTag` instructions for reuse, with self-set elimination.
///
/// Returns `(instructions, fields_skipped)`.
pub(super) fn build_set_instructions(
    source_var: ArcVarId,
    args: &[ArcVarId],
    ctor: CtorKind,
    proj_map: &ProjMap,
) -> (Vec<ArcInstr>, usize) {
    let total_fields = args.len();
    let mut sets: Vec<ArcInstr> = Vec::new();
    for (field_idx, arg) in args.iter().enumerate() {
        let field = u32::try_from(field_idx)
            .unwrap_or_else(|_| panic!("field index {field_idx} exceeds u32::MAX"));
        if !is_self_set(source_var, field, *arg, proj_map) {
            sets.push(ArcInstr::Set {
                base: source_var,
                field,
                value: *arg,
            });
        }
    }
    let fields_skipped = total_fields - sets.len();

    if let CtorKind::EnumVariant { variant, .. } = ctor {
        sets.push(ArcInstr::SetTag {
            base: source_var,
            tag: u64::from(variant),
        });
    }

    (sets, fields_skipped)
}

/// Extract `(dst, ctor, args)` from a `Construct` instruction.
/// Returns `(None, CtorKind::ListLiteral, vec![])` if the instruction is not a Construct.
pub(super) fn extract_construct_info(
    func: &ArcFunction,
    block_idx: usize,
    instr_idx: usize,
) -> (Option<ArcVarId>, CtorKind, Vec<ArcVarId>) {
    match &func.blocks[block_idx].body[instr_idx] {
        ArcInstr::Construct {
            dst, ctor, args, ..
        } => (Some(*dst), *ctor, args.clone()),
        _ => (None, CtorKind::ListLiteral, Vec::new()),
    }
}

/// Substitute `old_var` with `new_var` in all blocks.
pub(super) fn substitute_var_all(func: &mut ArcFunction, old_var: ArcVarId, new_var: ArcVarId) {
    for block in &mut func.blocks {
        for instr in &mut block.body {
            instr.substitute_var(old_var, new_var);
        }
        block.terminator.substitute_var(old_var, new_var);
    }
}
