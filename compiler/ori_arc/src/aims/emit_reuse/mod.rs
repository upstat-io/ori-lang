//! Reuse emission from converged AIMS state map.
//!
//! Detects reuse opportunities where a dying unique value can be recycled
//! for a subsequent same-type allocation, then emits in-place `Set`
//! instructions (static-unique path) or expanded `IsShared`+`Branch` CFG
//! (dynamic path). Replaces `reset_reuse` + `expand_reuse` from the old
//! pipeline.
//!
//! # Algorithm
//!
//! 1. Collect death events (owned, unique/maybe-shared, reusable shape)
//! 2. Collect allocation events (Construct instructions)
//! 3. Match death→alloc pairs:
//!    - Same-block: nearest subsequent allocation, no intervening uses
//!    - Cross-block: via [`ReusePlanner`] with dominator/post-dominator
//!      validation (Stage 1: static-unique only)
//! 4. Emit reuse instructions for matched pairs
//!
//! # Self-set elimination
//!
//! For static-unique reuse, builds a projection map from `Project`
//! instructions to identify which `Construct` args are unchanged from
//! the source. Unchanged fields skip `Set` emission entirely.

mod detect;
mod dynamic;
pub(crate) mod fip;
pub(crate) mod planner;
mod set_ops;
#[cfg(test)]
mod tests;

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use crate::aims::contract::{FipContract, MemoryContract};

pub use fip::{FipGateDecision, FipGateRecord};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{Cardinality, ShapeClass, SizeClass, Uniqueness};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};

use set_ops::{build_proj_map, build_set_instructions, extract_construct_info, substitute_var_all};

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
    /// Cardinality (backward demand) — used for cross-dimensional
    /// uniqueness proof: `Once + ReusableCtor → static reuse`.
    /// Section 09.2 Shape Activation.
    pub cardinality: Cardinality,
    /// Type of the dying variable.
    pub ty: Idx,
    /// Shape classification (from per-variable shape map, not block state).
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
    /// Number of same-block static-unique reuses emitted.
    pub static_reuses: usize,
    /// Number of same-block dynamic (`IsShared`) reuses emitted.
    pub dynamic_reuses: usize,
    /// Number of cross-block reuses emitted (static-unique only in Stage 1).
    pub cross_block_reuses: usize,
    /// Number of fields skipped via self-set elimination.
    pub fields_skipped: usize,
    /// FIP gate records: reuse decisions influenced by FIP certification.
    /// Consumed by verification (Section 08).
    pub fip_gates: Vec<FipGateRecord>,
    /// Death events with no compatible allocation found.
    /// Used for FBIP enrichment diagnostics.
    pub missed_reuses: usize,
}

/// Emit reuse operations into the function based on converged AIMS analysis.
///
/// Detects reuse opportunities from the state map, then emits:
/// - **Static-unique** (`Unique`): in-place `Set` instructions with self-set
///   elimination. No runtime check needed.
/// - **Dynamic** (`MaybeShared`): `IsShared` + `Branch` → fast path (in-place
///   `Set`) / slow path (`RcDec` + `Construct`). Emitted directly as expanded
///   CFG — no intermediate `Reset`/`Reuse` instructions.
/// - **Cross-block** (via `ReusePlanner`): dominator/post-dominator validated
///   reuse across basic block boundaries. Stage 1: static-unique only.
///
/// # Processing order
///
/// Same-block opportunities are sorted by (block, reverse instruction index)
/// so that later opportunities in the same block are processed first, preserving
/// index validity. Cross-block opportunities are processed separately, sorted
/// by `(source_block desc, source_instr desc)` then `(target_block desc,
/// target_instr desc)` for index safety.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn emit_reuse(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> EmitReuseResult {
    let (raw_opportunities, total_deaths) = detect::find_reuse_opportunities(func, state_map, pool);

    // Consult FIP contract for this function (Section 05.4).
    let fip = contracts.get(&func.name).map(|c| &c.fip);
    let (opportunities, fip_gates) = fip::apply_fip_upgrades(raw_opportunities, fip);

    // Partition into same-block and cross-block.
    let (mut same_block, mut cross_block): (Vec<_>, Vec<_>) = opportunities
        .into_iter()
        .partition(|opp| opp.source_block == opp.target_instr.0);

    // Sort same-block: (source_block, reverse source_instr).
    same_block.sort_by(|a, b| {
        a.source_block
            .raw()
            .cmp(&b.source_block.raw())
            .then(b.source_instr.cmp(&a.source_instr))
    });

    // Sort cross-block: process source blocks from high to low index,
    // within same source block from high to low instr. This ensures
    // removing an RcDec at index i doesn't affect indices < i.
    cross_block.sort_by(|a, b| {
        b.source_block
            .raw()
            .cmp(&a.source_block.raw())
            .then(b.source_instr.cmp(&a.source_instr))
            .then(b.target_instr.0.raw().cmp(&a.target_instr.0.raw()))
            .then(b.target_instr.1.cmp(&a.target_instr.1))
    });

    let mut static_reuses = 0;
    let mut dynamic_reuses = 0;
    let mut cross_block_reuses = 0;
    let mut fields_skipped = 0;

    // Phase 1: same-block opportunities.
    for opp in &same_block {
        if opp.is_static_unique {
            fields_skipped += apply_static_reuse_same_block(func, opp);
            static_reuses += 1;
        } else {
            fields_skipped += dynamic::apply_dynamic_reuse(func, opp);
            dynamic_reuses += 1;
        }
    }

    // Phase 2: cross-block opportunities (static-unique only in Stage 1).
    for opp in &cross_block {
        fields_skipped += apply_static_reuse_cross_block(func, opp);
        cross_block_reuses += 1;
    }

    let matched = same_block.len() + cross_block.len();
    let missed_reuses = total_deaths.saturating_sub(matched);

    if static_reuses > 0 || dynamic_reuses > 0 || cross_block_reuses > 0 {
        tracing::debug!(
            function = func.name.raw(),
            static_reuses,
            dynamic_reuses,
            cross_block_reuses,
            fields_skipped,
            fip_gates = fip_gates.len(),
            missed_reuses,
            "AIMS reuse emission complete"
        );
    }

    // FBIP enrichment: warn if FIP-certified function has unmatched deaths.
    if missed_reuses > 0 && matches!(fip, Some(FipContract::Certified)) {
        tracing::warn!(
            function = func.name.raw(),
            missed_reuses,
            "FIP-certified function has unmatched death events"
        );
    }

    EmitReuseResult {
        static_reuses,
        dynamic_reuses,
        cross_block_reuses,
        fields_skipped,
        fip_gates,
        missed_reuses,
    }
}

// Static-unique reuse emission

/// Apply same-block static-unique reuse with self-set elimination.
///
/// For the static-unique path (source is provably RC == 1):
/// 1. Build projection map from `Project` instructions before the death site
/// 2. For each `Construct` arg, skip `Set` if it's a self-set (unchanged field)
/// 3. Emit `Set` only for changed fields, `SetTag` for enum variant changes
/// 4. Remove `RcDec` (allocation reused, not freed) and `Construct` (in-place)
/// 5. Substitute `Construct.dst` with `source_var` everywhere
///
/// Returns the number of fields skipped via self-set elimination.
fn apply_static_reuse_same_block(func: &mut ArcFunction, opp: &ReuseOpportunity) -> usize {
    let (target_block, target_instr) = opp.target_instr;

    debug_assert_eq!(
        opp.source_block, target_block,
        "apply_static_reuse_same_block called with cross-block opportunity"
    );

    let block_idx = opp.source_block.index();

    // 1. Build projection map from instructions before the death site.
    let proj_map = build_proj_map(
        &func.blocks[block_idx].body[..opp.source_instr],
        opp.source_var,
    );

    // 2. Extract Construct info before mutation.
    let (dst, ctor, args) = extract_construct_info(func, block_idx, target_instr);
    let Some((dst, ctor, args)) = dst.map(|d| (d, ctor, args)) else {
        return 0;
    };

    // 3. Build Set instructions for changed fields only (self-set elimination).
    let (sets, fields_skipped) = build_set_instructions(opp.source_var, &args, ctor, &proj_map);
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
    substitute_var_all(func, dst, opp.source_var);

    fields_skipped
}

/// Apply cross-block static-unique reuse with self-set elimination.
///
/// For cross-block reuse (source and target in different blocks):
/// 1. Build projection map from source block (before death site)
/// 2. Extract Construct info from target block
/// 3. Build Set instructions with self-set elimination
/// 4. Remove `RcDec` from source block
/// 5. Replace `Construct` in target block with Set instructions
/// 6. Substitute `Construct.dst` with `source_var` everywhere
///
/// Returns the number of fields skipped via self-set elimination.
fn apply_static_reuse_cross_block(func: &mut ArcFunction, opp: &ReuseOpportunity) -> usize {
    let (target_block_id, target_instr) = opp.target_instr;
    let source_block_idx = opp.source_block.index();
    let target_block_idx = target_block_id.index();

    debug_assert_ne!(
        opp.source_block, target_block_id,
        "apply_static_reuse_cross_block called with same-block opportunity"
    );

    // Defensive: verify expected instructions are still in place.
    if !matches!(
        func.blocks[source_block_idx].body.get(opp.source_instr),
        Some(ArcInstr::RcDec { .. })
    ) {
        tracing::warn!(
            "cross-block reuse: expected RcDec at source block {} instr {}",
            source_block_idx,
            opp.source_instr
        );
        return 0;
    }
    if !matches!(
        func.blocks[target_block_idx].body.get(target_instr),
        Some(ArcInstr::Construct { .. })
    ) {
        tracing::warn!(
            "cross-block reuse: expected Construct at target block {} instr {}",
            target_block_idx,
            target_instr
        );
        return 0;
    }

    // 1. Build projection map from source block instructions before death.
    let proj_map = build_proj_map(
        &func.blocks[source_block_idx].body[..opp.source_instr],
        opp.source_var,
    );

    // 2. Extract Construct info from target block.
    let (dst, ctor, args) = extract_construct_info(func, target_block_idx, target_instr);
    let Some((dst, ctor, args)) = dst.map(|d| (d, ctor, args)) else {
        return 0;
    };

    // 3. Build Set instructions with self-set elimination.
    let (sets, fields_skipped) = build_set_instructions(opp.source_var, &args, ctor, &proj_map);
    let sets_count = sets.len();

    // 4. Remove RcDec from source block.
    let old_source_body = std::mem::take(&mut func.blocks[source_block_idx].body);
    let mut new_source_body = Vec::with_capacity(old_source_body.len().saturating_sub(1));
    for (idx, instr) in old_source_body.into_iter().enumerate() {
        if idx == opp.source_instr {
            continue; // Remove RcDec — allocation is being reused.
        }
        new_source_body.push(instr);
    }
    func.blocks[source_block_idx].body = new_source_body;

    // Update source block spans.
    if source_block_idx < func.spans.len() {
        let old_spans = std::mem::take(&mut func.spans[source_block_idx]);
        let mut new_spans = Vec::with_capacity(old_spans.len().saturating_sub(1));
        for (idx, span) in old_spans.into_iter().enumerate() {
            if idx == opp.source_instr {
                continue;
            }
            new_spans.push(span);
        }
        func.spans[source_block_idx] = new_spans;
    }

    // 5. Replace Construct in target block with Set instructions.
    let old_target_body = std::mem::take(&mut func.blocks[target_block_idx].body);
    let mut new_target_body =
        Vec::with_capacity(old_target_body.len().saturating_sub(1) + sets_count);
    let mut set_iter = sets.into_iter();
    for (idx, instr) in old_target_body.into_iter().enumerate() {
        if idx == target_instr {
            new_target_body.extend(&mut set_iter); // Replace Construct with Sets.
            continue;
        }
        new_target_body.push(instr);
    }
    func.blocks[target_block_idx].body = new_target_body;

    // Update target block spans.
    if target_block_idx < func.spans.len() {
        let old_spans = std::mem::take(&mut func.spans[target_block_idx]);
        let mut new_spans = Vec::with_capacity(old_spans.len().saturating_sub(1) + sets_count);
        for (idx, span) in old_spans.into_iter().enumerate() {
            if idx == target_instr {
                new_spans.extend(std::iter::repeat_n(None, sets_count));
                continue;
            }
            new_spans.push(span);
        }
        func.spans[target_block_idx] = new_spans;
    }

    // 6. Substitute Construct's dst with source_var everywhere.
    substitute_var_all(func, dst, opp.source_var);

    fields_skipped
}
