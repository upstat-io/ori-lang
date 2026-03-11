//! Reuse emission from converged AIMS state map.
//!
//! Detects reuse opportunities where a dying unique value can be recycled
//! for a subsequent same-type allocation, then emits in-place `Set`
//! instructions (static-unique path) or expanded `IsShared`+`Branch` CFG
//! (dynamic path, Stage 2). Replaces `reset_reuse` + `expand_reuse` from
//! the old pipeline.
//!
//! # Algorithm
//!
//! 1. Collect death events (owned, unique/maybe-shared, reusable shape)
//! 2. Collect allocation events (Construct instructions)
//! 3. Match death→alloc pairs (same type, same or dominated block)
//! 4. Emit reuse instructions for matched pairs
//!
//! # Self-set elimination (§09.5)
//!
//! For static-unique reuse, builds a projection map from `Project`
//! instructions to identify which `Construct` args are unchanged from
//! the source. Unchanged fields skip `Set` emission entirely.

mod detect;
#[cfg(test)]
mod tests;

use rustc_hash::FxHashMap;

use ori_types::{Idx, Pool};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{ShapeClass, SizeClass, Uniqueness};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, CtorKind};

/// A matched reuse opportunity: a dying value paired with a compatible allocation.
#[derive(Clone, Debug)]
pub struct ReuseOpportunity {
    /// The variable being consumed (source of the reuse token).
    pub source_var: ArcVarId,
    /// The block where the source dies.
    pub source_block: ArcBlockId,
    /// The instruction index of the `RcDec` for the source.
    pub source_instr: usize,
    /// The block and instruction index of the target `Construct`.
    pub target_instr: (ArcBlockId, usize),
    /// Whether the source is provably unique (skip `IsShared` check).
    pub is_static_unique: bool,
}

/// A death event: a variable transitioning to dead with reusable properties.
#[derive(Clone, Debug)]
pub struct DeathEvent {
    /// The dying variable.
    pub var: ArcVarId,
    /// Block where the death occurs.
    pub block: ArcBlockId,
    /// Instruction index of the `RcDec`.
    pub instr_idx: usize,
    /// Uniqueness at the death point.
    pub uniqueness: Uniqueness,
    /// Type of the dying variable.
    pub ty: Idx,
    /// Shape classification.
    pub shape: ShapeClass,
    /// Allocation size class (Stage 2+ cross-type matching).
    pub size_class: SizeClass,
}

/// An allocation event: a `Construct` instruction creating a new value.
#[derive(Clone, Debug)]
pub struct AllocEvent {
    /// Block containing the `Construct`.
    pub block: ArcBlockId,
    /// Instruction index.
    pub instr_idx: usize,
    /// Destination variable.
    pub dst: ArcVarId,
    /// Type being constructed.
    pub ty: Idx,
    /// Shape classification.
    pub shape: ShapeClass,
    /// Allocation size class (Stage 2+ cross-type matching).
    pub size_class: SizeClass,
}

/// Result of reuse emission.
pub struct EmitReuseResult {
    /// Number of static-unique reuses emitted.
    pub static_reuses: usize,
    /// Number of dynamic (`IsShared`) reuses emitted.
    pub dynamic_reuses: usize,
    /// Number of fields skipped via self-set elimination (§09.5).
    pub fields_skipped: usize,
}

/// Emit reuse operations into the function based on converged AIMS analysis.
///
/// Detects same-block reuse opportunities from the state map, then emits
/// in-place `Set` instructions for static-unique cases with self-set
/// elimination (§09.5). Cross-block reuse and dynamic (`IsShared`)
/// expansion are deferred to Stage 2.
///
/// # Stage 1 scope
///
/// - Same-block, same-type, static-unique reuse only
/// - Self-set elimination: skip `Set` for unchanged fields
/// - Dynamic reuse (`MaybeShared` with `IsShared` check) deferred
/// - Cross-block reuse via `ReusePlanner` deferred
pub fn emit_reuse(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> EmitReuseResult {
    let opportunities = detect::find_reuse_opportunities(func, state_map, pool);

    let mut static_reuses = 0;
    let mut dynamic_reuses = 0;
    let mut fields_skipped = 0;

    for opp in &opportunities {
        if opp.is_static_unique {
            fields_skipped += apply_static_reuse(func, opp);
            static_reuses += 1;
        } else {
            // Stage 1: skip dynamic reuse (MaybeShared with IsShared check).
            // The RcDec remains as-is; the Construct remains as-is.
            dynamic_reuses += 1;
        }
    }

    if static_reuses > 0 || dynamic_reuses > 0 {
        tracing::debug!(
            function = func.name.raw(),
            static_reuses,
            dynamic_reuses,
            fields_skipped,
            "AIMS reuse emission complete"
        );
    }

    EmitReuseResult {
        static_reuses,
        dynamic_reuses,
        fields_skipped,
    }
}

// Projection map for self-set elimination (§09.5)

/// Maps `(base_var, field_index)` → `projected_var` for projections seen
/// before the death site. Used for self-set elimination.
type ProjMap = FxHashMap<(ArcVarId, u32), ArcVarId>;

/// Build a map from `(base_var, field_index)` → `projected_var` for all
/// `Project` instructions in `instrs` that project from `base`.
fn build_proj_map(instrs: &[ArcInstr], base: ArcVarId) -> ProjMap {
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
fn is_self_set(base: ArcVarId, field: u32, value: ArcVarId, proj_map: &ProjMap) -> bool {
    proj_map.get(&(base, field)) == Some(&value)
}

// Static-unique reuse emission

/// Apply static-unique reuse with self-set elimination (§09.5).
///
/// For the static-unique path (source is provably RC == 1):
/// 1. Build projection map from `Project` instructions before the death site
/// 2. For each `Construct` arg, skip `Set` if it's a self-set (unchanged field)
/// 3. Emit `Set` only for changed fields, `SetTag` for enum variant changes
/// 4. Remove `RcDec` (allocation reused, not freed) and `Construct` (in-place)
/// 5. Substitute `Construct.dst` with `source_var` everywhere
///
/// Returns the number of fields skipped via self-set elimination.
fn apply_static_reuse(func: &mut ArcFunction, opp: &ReuseOpportunity) -> usize {
    let (target_block, target_instr) = opp.target_instr;

    // Only same-block reuse in Stage 1.
    debug_assert_eq!(
        opp.source_block, target_block,
        "cross-block reuse not yet supported in Stage 1"
    );

    let block_idx = opp.source_block.index();

    // 1. Build projection map from instructions before the death site.
    let proj_map = build_proj_map(
        &func.blocks[block_idx].body[..opp.source_instr],
        opp.source_var,
    );

    // 2. Extract Construct info before mutation.
    let (dst, ctor, args) = match &func.blocks[block_idx].body[target_instr] {
        ArcInstr::Construct {
            dst, ctor, args, ..
        } => (*dst, *ctor, args.clone()),
        _ => return 0, // Safety: should always be Construct.
    };

    // 3. Build Set instructions for changed fields only (self-set elimination).
    let total_fields = args.len();
    let mut sets: Vec<ArcInstr> = Vec::new();
    for (field_idx, arg) in args.iter().enumerate() {
        let field = u32::try_from(field_idx)
            .unwrap_or_else(|_| panic!("field index {field_idx} exceeds u32::MAX"));
        if !is_self_set(opp.source_var, field, *arg, &proj_map) {
            sets.push(ArcInstr::Set {
                base: opp.source_var,
                field,
                value: *arg,
            });
        }
    }
    let fields_skipped = total_fields - sets.len();

    // SetTag for enum variants (tag may have changed).
    if let CtorKind::EnumVariant { variant, .. } = ctor {
        sets.push(ArcInstr::SetTag {
            base: opp.source_var,
            tag: u64::from(variant),
        });
    }

    let sets_count = sets.len();

    // 4. Rebuild block body: remove RcDec, replace Construct with Sets.
    let old_body = std::mem::take(&mut func.blocks[block_idx].body);
    let mut new_body = Vec::with_capacity(old_body.len().saturating_sub(2) + sets_count);
    let mut set_iter = sets.into_iter();
    for (idx, instr) in old_body.into_iter().enumerate() {
        if idx == opp.source_instr {
            continue; // Remove RcDec — allocation is being reused.
        }
        if idx == target_instr {
            new_body.extend(&mut set_iter); // Replace Construct with Sets.
            continue;
        }
        new_body.push(instr);
    }
    func.blocks[block_idx].body = new_body;

    // Rebuild spans to match new body length.
    if block_idx < func.spans.len() {
        let old_spans = std::mem::take(&mut func.spans[block_idx]);
        let mut new_spans = Vec::with_capacity(old_spans.len().saturating_sub(2) + sets_count);
        for (idx, span) in old_spans.into_iter().enumerate() {
            if idx == opp.source_instr {
                continue;
            }
            if idx == target_instr {
                new_spans.extend(std::iter::repeat_n(None, sets_count));
                continue;
            }
            new_spans.push(span);
        }
        func.spans[block_idx] = new_spans;
    }

    // 5. Substitute Construct's dst with source_var everywhere.
    for block in &mut func.blocks {
        for instr in &mut block.body {
            instr.substitute_var(dst, opp.source_var);
        }
        block.terminator.substitute_var(dst, opp.source_var);
    }

    fields_skipped
}
