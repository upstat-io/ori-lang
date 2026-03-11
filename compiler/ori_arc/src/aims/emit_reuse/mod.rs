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
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};

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
/// Detects same-block reuse opportunities from the state map, then emits:
/// - **Static-unique** (`Unique`): in-place `Set` instructions with self-set
///   elimination (§09.5). No runtime check needed.
/// - **Dynamic** (`MaybeShared`): `IsShared` + `Branch` → fast path (in-place
///   `Set`) / slow path (`RcDec` + `Construct`). Emitted directly as expanded
///   CFG — no intermediate `Reset`/`Reuse` instructions.
///
/// Cross-block reuse via `ReusePlanner` is deferred to Stage 2.
///
/// # Processing order
///
/// Opportunities are sorted by (block, reverse instruction index) so that
/// later opportunities in the same block are processed first. This preserves
/// index validity for earlier opportunities — only the tail of the block is
/// modified, leaving prefix indices unchanged.
pub fn emit_reuse(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> EmitReuseResult {
    let mut opportunities = detect::find_reuse_opportunities(func, state_map, pool);

    // Sort by (block, reverse source_instr) so we process later opportunities
    // first within each block, preserving earlier indices.
    opportunities.sort_by(|a, b| {
        a.source_block
            .raw()
            .cmp(&b.source_block.raw())
            .then(b.source_instr.cmp(&a.source_instr))
    });

    let mut static_reuses = 0;
    let mut dynamic_reuses = 0;
    let mut fields_skipped = 0;

    for opp in &opportunities {
        if opp.is_static_unique {
            fields_skipped += apply_static_reuse(func, opp);
            static_reuses += 1;
        } else {
            fields_skipped += apply_dynamic_reuse(func, opp);
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

// Dynamic reuse emission (`MaybeShared` → `IsShared` + `Branch`)

/// Extracted data from a block needed for dynamic reuse expansion.
struct DynamicReuseContext {
    /// `RcDec` strategy from the original instruction.
    rc_strategy: crate::ir::RcStrategy,
    /// Projection map for self-set elimination.
    proj_map: ProjMap,
    /// Construct destination variable.
    dst: ArcVarId,
    /// Construct type.
    ty: Idx,
    /// Constructor kind.
    ctor: CtorKind,
    /// Constructor arguments.
    args: Vec<ArcVarId>,
    /// Instructions between `RcDec` and `Construct` (safe to reorder).
    between: Vec<ArcInstr>,
    /// Spans for between instructions.
    between_spans: Vec<Option<ori_ir::Span>>,
    /// Instructions after the `Construct`.
    suffix: Vec<ArcInstr>,
    /// Original block terminator.
    original_terminator: ArcTerminator,
    /// Whether a merge block is required.
    needs_merge: bool,
}

/// Extract all data needed for dynamic reuse before mutating the function.
///
/// Returns `None` if the block doesn't contain the expected instructions.
fn extract_dynamic_context(
    func: &ArcFunction,
    opp: &ReuseOpportunity,
    target_instr_idx: usize,
) -> Option<DynamicReuseContext> {
    let block_idx = opp.source_block.index();

    let rc_strategy = match &func.blocks[block_idx].body[opp.source_instr] {
        ArcInstr::RcDec { strategy, .. } => *strategy,
        other => {
            tracing::warn!(
                "expected RcDec at source_instr {}, got {:?}",
                opp.source_instr,
                other
            );
            return None;
        }
    };

    let (dst, ty, ctor, args) = match &func.blocks[block_idx].body[target_instr_idx] {
        ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args,
        } => (*dst, *ty, *ctor, args.clone()),
        other => {
            tracing::warn!(
                "expected Construct at target_instr {}, got {:?}",
                target_instr_idx,
                other
            );
            return None;
        }
    };

    let proj_map = build_proj_map(
        &func.blocks[block_idx].body[..opp.source_instr],
        opp.source_var,
    );

    let between = func.blocks[block_idx].body[opp.source_instr + 1..target_instr_idx].to_vec();
    let between_spans = if block_idx < func.spans.len() {
        let start = (opp.source_instr + 1).min(func.spans[block_idx].len());
        let end = target_instr_idx.min(func.spans[block_idx].len());
        func.spans[block_idx][start..end].to_vec()
    } else {
        Vec::new()
    };

    let suffix = func.blocks[block_idx].body[target_instr_idx + 1..].to_vec();
    let original_terminator = func.blocks[block_idx].terminator.clone();
    let needs_merge = !suffix.is_empty() || original_terminator.uses_var(dst);

    Some(DynamicReuseContext {
        rc_strategy,
        proj_map,
        dst,
        ty,
        ctor,
        args,
        between,
        between_spans,
        suffix,
        original_terminator,
        needs_merge,
    })
}

/// Build the fast-path body: `Set` for changed fields + `SetTag` for enums.
///
/// Returns `(body, fields_skipped)`.
fn build_fast_body(
    source_var: ArcVarId,
    args: &[ArcVarId],
    ctor: CtorKind,
    proj_map: &ProjMap,
) -> (Vec<ArcInstr>, usize) {
    let mut body = Vec::new();
    let mut fields_skipped = 0;
    for (field_idx, arg) in args.iter().enumerate() {
        let field = u32::try_from(field_idx)
            .unwrap_or_else(|_| panic!("field index {field_idx} exceeds u32::MAX"));
        if is_self_set(source_var, field, *arg, proj_map) {
            fields_skipped += 1;
            continue;
        }
        body.push(ArcInstr::Set {
            base: source_var,
            field,
            value: *arg,
        });
    }
    if let CtorKind::EnumVariant { variant, .. } = ctor {
        body.push(ArcInstr::SetTag {
            base: source_var,
            tag: u64::from(variant),
        });
    }
    (body, fields_skipped)
}

/// Apply dynamic reuse with `IsShared` conditional branch.
///
/// For the dynamic path (`MaybeShared`, source RC unknown):
/// 1. Move "between" instructions (after `RcDec`, before `Construct`) up
/// 2. Emit `IsShared` check + `Branch` in the original block
/// 3. Fast path (unique): in-place `Set` mutations with self-set elimination
/// 4. Slow path (shared): `RcDec` + fresh `Construct`
/// 5. Merge block if suffix instructions or terminator uses the result
///
/// Returns the number of fields skipped via self-set elimination.
fn apply_dynamic_reuse(func: &mut ArcFunction, opp: &ReuseOpportunity) -> usize {
    let (target_block, target_instr_idx) = opp.target_instr;
    debug_assert_eq!(opp.source_block, target_block, "same-block only");

    let Some(ctx) = extract_dynamic_context(func, opp, target_instr_idx) else {
        return 0;
    };

    let (fast_body, fields_skipped) =
        build_fast_body(opp.source_var, &ctx.args, ctx.ctor, &ctx.proj_map);

    let (fast_block, slow_block, merge_block) = build_dynamic_blocks(func, opp, &ctx, fast_body);

    rewrite_original_block(func, opp, &ctx);

    // Substitute dst → merge_param in existing blocks (before push).
    if let Some(mb) = &merge_block {
        let merge_param = mb.params[0].0;
        for block in &mut func.blocks {
            for instr in &mut block.body {
                instr.substitute_var(ctx.dst, merge_param);
            }
            block.terminator.substitute_var(ctx.dst, merge_param);
        }
    }

    func.push_block(fast_block);
    func.push_block(slow_block);
    if let Some(mb) = merge_block {
        func.push_block(mb);
    }

    fields_skipped
}

/// Build fast, slow, and optional merge blocks for dynamic reuse.
fn build_dynamic_blocks(
    func: &mut ArcFunction,
    opp: &ReuseOpportunity,
    ctx: &DynamicReuseContext,
    fast_body: Vec<ArcInstr>,
) -> (ArcBlock, ArcBlock, Option<ArcBlock>) {
    let fast_id = func.next_block_id();
    let slow_id = ArcBlockId::new(fast_id.raw() + 1);
    let merge_id = if ctx.needs_merge {
        Some(ArcBlockId::new(slow_id.raw() + 1))
    } else {
        None
    };

    // Merge block (built first — needs `func.fresh_var`).
    let merge_block = merge_id.map(|mid| {
        let merge_param = func.fresh_var(ctx.ty);
        let mut merge_body: Vec<ArcInstr> = ctx.suffix.clone();
        for instr in &mut merge_body {
            instr.substitute_var(ctx.dst, merge_param);
        }
        let mut merge_term = ctx.original_terminator.clone();
        merge_term.substitute_var(ctx.dst, merge_param);
        ArcBlock {
            id: mid,
            params: vec![(merge_param, ctx.ty)],
            body: merge_body,
            terminator: merge_term,
        }
    });

    // Fast block.
    let fast_terminator = if let Some(mid) = merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![opp.source_var],
        }
    } else {
        let mut term = ctx.original_terminator.clone();
        term.substitute_var(ctx.dst, opp.source_var);
        term
    };
    let fast_block = ArcBlock {
        id: fast_id,
        params: vec![],
        body: fast_body,
        terminator: fast_terminator,
    };

    // Slow block.
    let mut slow_body = vec![
        ArcInstr::RcDec {
            var: opp.source_var,
            strategy: ctx.rc_strategy,
        },
        ArcInstr::Construct {
            dst: ctx.dst,
            ty: ctx.ty,
            ctor: ctx.ctor,
            args: ctx.args.clone(),
        },
    ];
    let slow_terminator = if let Some(mid) = merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![ctx.dst],
        }
    } else {
        slow_body.extend_from_slice(&ctx.suffix);
        ctx.original_terminator.clone()
    };
    let slow_block = ArcBlock {
        id: slow_id,
        params: vec![],
        body: slow_body,
        terminator: slow_terminator,
    };

    (fast_block, slow_block, merge_block)
}

/// Rewrite the original block: prefix + between + `IsShared` + `Branch`.
fn rewrite_original_block(
    func: &mut ArcFunction,
    opp: &ReuseOpportunity,
    ctx: &DynamicReuseContext,
) {
    let block_idx = opp.source_block.index();
    let num_blocks = func.blocks.len();
    // fast/slow/merge IDs are based on current block count.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "ARC IR block counts fit in u32"
    )]
    let fast_id = ArcBlockId::new(num_blocks as u32);
    let slow_id = ArcBlockId::new(fast_id.raw() + 1);

    let is_shared_var = func.fresh_var(Idx::BOOL);
    let prefix = func.blocks[block_idx].body[..opp.source_instr].to_vec();
    let mut new_body = Vec::with_capacity(prefix.len() + ctx.between.len() + 1);
    new_body.extend(prefix);
    new_body.extend_from_slice(&ctx.between);
    new_body.push(ArcInstr::IsShared {
        dst: is_shared_var,
        var: opp.source_var,
    });
    func.blocks[block_idx].body = new_body;
    func.blocks[block_idx].terminator = ArcTerminator::Branch {
        cond: is_shared_var,
        then_block: slow_id,
        else_block: fast_id,
    };

    // Update spans.
    if block_idx < func.spans.len() {
        let old_spans = &func.spans[block_idx];
        let prefix_end = opp.source_instr.min(old_spans.len());
        let mut new_spans = Vec::with_capacity(prefix_end + ctx.between_spans.len() + 1);
        new_spans.extend_from_slice(&old_spans[..prefix_end]);
        new_spans.extend_from_slice(&ctx.between_spans);
        new_spans.push(None);
        func.spans[block_idx] = new_spans;
    }
}
