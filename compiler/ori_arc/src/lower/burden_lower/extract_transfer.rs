//! Match-handoff extract-transfer attribution (RL-2): the all-arms verdict
//! that cures the sum-payload loop-rebuild double-free.
//!
//! `r = Pair { o: Some(extracted), b: r.b }` where `extracted = match r.o
//! { Some(xs) -> xs, None -> [] }` extracts the sum field's PAYLOAD through
//! the match block-param handoff and re-wraps it into the rebuild construct.
//! The carrier's `BurdenDecPartial` still releases the sum field — freeing a
//! payload whose ownership already transferred (RL-2 `ConstructArg`) into the
//! new loop-carried struct: a double-free.
//!
//! Cure (post-sibling-union): a still-released sum field counts MOVED when
//! EVERY switch arm over the field's tag either (a) transfers the extracted
//! payload into the rebuild construct on an UNCONDITIONAL arm path (owning
//! Construct-arg position, per-edge block-param attribution), or (b) is a
//! payload-less-variant arm (vacuous). ANY arm with a conditional / partial
//! flow DECLINES — the release stays (the dropped-payload path needs it).
//!
//! Spec: Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec`: a transferred
//! payload's obligation moves to the consumer; a dec on it double-releases;
//! `RL2_nontransfer_kinds_dec`: a non-transferred payload keeps its release).

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use ori_types::TypeRegistry;

use crate::aims::intraprocedural::project_aliases::ParamEdgeArg;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::super::burden::{Burden, BurdenRef, TypeRef};
use super::super::burden_lookup::{idx_to_type_ref, lookup_burden};
use super::sibling_union::owned_top_level_fields;

/// `ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER=1` restores the per-field
/// attribution WITHOUT the all-arms extract-transfer verdict: a rebuild
/// carrier's `BurdenDecPartial` keeps releasing a sum field whose payload was
/// extracted through the match block-param handoff and re-wrapped into the
/// rebuild construct (the pre-fix double-free shape). Bisection surface:
/// isolates the sum-payload match-rebuild double-free to this verdict vs the
/// rest of the Phase-5 walk. Default (unset): the all-arms verdict widens the
/// carrier's skip set. Spec: Annex E §AIMS RL-2.
pub(super) fn match_handoff_extract_transfer_disabled() -> bool {
    std::env::var("ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER").as_deref() == Ok("1")
}

/// Apply the all-arms match-handoff extract-transfer verdict, mutating
/// `partial_move_vars` (widening a carrier's `skip_fields` with all-arms-
/// transferred sum fields) and `full_move_vars` (absorbing carriers whose
/// widened skip covers every owned RC field). Runs after
/// `apply_sibling_moved_field_union` and BEFORE `inc_suppressed_vars` is
/// derived from `full_move_vars`.
pub(super) fn apply_match_handoff_extract_transfer(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) {
    apply_match_handoff_extract_transfer_with(
        match_handoff_extract_transfer_disabled(),
        func,
        type_registry,
        owned_vars_needing_rc,
        param_edge_args,
        full_move_vars,
        partial_move_vars,
    );
}

/// Toggle-injected body of [`apply_match_handoff_extract_transfer`]; `disabled`
/// carries the `ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER` verdict so tests
/// exercise the disabled path without mutating process-global env.
pub(super) fn apply_match_handoff_extract_transfer_with(
    disabled: bool,
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) {
    if disabled {
        return;
    }
    let idx = FuncIndex::build(func);

    let carriers: Vec<(ArcVarId, Vec<u32>)> = partial_move_vars
        .iter()
        .map(|(&c, skip)| (c, skip.clone()))
        .collect();
    for (carrier, skip) in carriers {
        if !owned_vars_needing_rc.contains(&carrier) {
            continue;
        }
        // The carrier MUST resolve to a back-edge block-param root (the
        // loop-carried rebuild lineage). Non-loop shapes keep their release.
        let Some(root) = idx.resolve_root(carrier, param_edge_args) else {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                carrier = carrier.index(),
                "match-handoff verdict declined: no loop-carried root"
            );
            continue;
        };
        // The merge block holding the carrier's Let definition — the rebuild
        // + partial-dec site.
        let Some(&merge_block) = idx.def_block.get(&carrier) else {
            continue;
        };
        let owned_fields = owned_top_level_fields(func, carrier, type_registry);
        let mut widened: Vec<u32> = skip.clone();
        for &field in &owned_fields {
            if skip.contains(&field) {
                continue;
            }
            if all_arms_transfer_field(
                func,
                type_registry,
                &idx,
                param_edge_args,
                carrier,
                root,
                merge_block,
                field,
            ) {
                widened.push(field);
                tracing::trace!(
                    target: "ori_arc::aims::realize",
                    fn_name = ?func.name,
                    carrier = carrier.index(),
                    field,
                    "match-handoff extract-transfer verdict: field counts moved (all arms transfer)"
                );
            }
        }
        if widened.len() == skip.len() {
            continue;
        }
        widened.sort_unstable();
        let covers_all = owned_fields.iter().all(|f| widened.contains(f));
        if covers_all {
            partial_move_vars.remove(&carrier);
            full_move_vars.insert(carrier);
        } else {
            partial_move_vars.insert(carrier, widened);
        }
    }
}

/// Structural per-function indexes the verdict consumes.
struct FuncIndex {
    /// `Let { dst, Var(src) }` edges: dst -> src.
    alias_src: FxHashMap<ArcVarId, ArcVarId>,
    /// All block-param vars.
    block_params: FxHashSet<ArcVarId>,
    /// var -> defining block index (instr dsts only; params excluded).
    def_block: FxHashMap<ArcVarId, usize>,
    /// Project dst -> (source value, field).
    project_of: FxHashMap<ArcVarId, (ArcVarId, u32)>,
}

impl FuncIndex {
    fn build(func: &ArcFunction) -> Self {
        let mut alias_src = FxHashMap::default();
        let mut block_params = FxHashSet::default();
        let mut def_block = FxHashMap::default();
        let mut project_of = FxHashMap::default();
        for (block_idx, block) in func.blocks.iter().enumerate() {
            for &(p, _) in &block.params {
                block_params.insert(p);
            }
            for instr in &block.body {
                if let Some(dst) = instr.defined_var() {
                    def_block.insert(dst, block_idx);
                }
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } => {
                        alias_src.insert(*dst, *src);
                    }
                    ArcInstr::Project {
                        dst, value, field, ..
                    } => {
                        project_of.insert(*dst, (*value, *field));
                    }
                    _ => {}
                }
            }
        }
        Self {
            alias_src,
            block_params,
            def_block,
            project_of,
        }
    }

    /// Resolve `start` to its loop-carried chain root: follow `Let { Var }`
    /// edges; a block-param hop passes through when EVERY non-back-edge
    /// incoming edge's arg resolves to the SAME root (per-edge attribution —
    /// all edges agreeing on one allocation is a pure rename, not a true
    /// merge); a back-edge-carrying param IS the root terminus. `None` when
    /// the chain dead-ends or edges disagree. Generalizes the sibling-union
    /// single-pred hop to the all-edges-agree multi-pred case (the match
    /// merge passes the SAME loop struct from every arm). Spec: Annex E
    /// §AIMS RL-1 + RL-2 (per-edge attribution).
    fn resolve_root(
        &self,
        start: ArcVarId,
        param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    ) -> Option<ArcVarId> {
        self.resolve_root_inner(start, param_edge_args, 0)
    }

    fn resolve_root_inner(
        &self,
        start: ArcVarId,
        param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
        depth: usize,
    ) -> Option<ArcVarId> {
        // Bounded recursion: each param hop recurses once per edge; chains are
        // short (the rebuild handoff spans a handful of hops).
        if depth > 32 {
            return None;
        }
        let mut cur = start;
        for _ in 0..=self.alias_src.len().max(self.block_params.len()) {
            if self.block_params.contains(&cur) {
                let edges = param_edge_args.get(&cur)?;
                // Pure rename: EVERY incoming edge passes the SAME arg var —
                // the param IS that allocation on every path, regardless of
                // edge cycle classification (a merge inside the loop body has
                // its edges classified back-edges because the body reaches
                // them around the loop; same-arg edges stay one allocation).
                let first = edges.first()?.arg;
                if first != cur && edges.iter().all(|e| e.arg == first) {
                    cur = first;
                    continue;
                }
                if edges.iter().any(|e| e.is_back_edge) {
                    // Loop-header merge (init edge + iteration back-edge
                    // disagree): the loop-carried root terminus. The
                    // back-edge itself is never traversed.
                    return Some(cur);
                }
                // Forward merge with disagreeing args: every edge's arg must
                // resolve to one root.
                let mut agreed: Option<ArcVarId> = None;
                for e in edges {
                    let r = self.resolve_root_inner(e.arg, param_edge_args, depth + 1)?;
                    match agreed {
                        None => agreed = Some(r),
                        Some(prev) if prev == r => {}
                        Some(_) => return None,
                    }
                }
                return agreed;
            }
            match self.alias_src.get(&cur) {
                Some(&next) => cur = next,
                None => return None,
            }
        }
        None
    }
}

/// The all-arms verdict for one still-released sum field of one carrier:
/// every switch arm over the field's tag either transfers the extracted
/// payload into the rebuild construct (unconditional arm path) or is the
/// payload-less-variant arm. Per-edge identity throughout — NEVER a
/// use-count proxy.
///
/// v1 shape gate: a TWO-variant single-payload sum (the `Option`-family
/// match-handoff). The payload arm is identified STRUCTURALLY (the arm that
/// projects the payload out of the view), never by variant-id ordinal —
/// the burden-row variant ordering and the runtime discriminant tags are
/// independent numbering surfaces, so a counting argument (exactly one
/// payload row, exactly one extracting arm) replaces an ordinal mapping:
/// when the counts match, the non-extracting arm IS the payload-less
/// variant on every execution.
#[expect(
    clippy::too_many_arguments,
    reason = "verdict inputs are the per-carrier structural indexes; grouping \
              would add indirection to a single-call-site helper"
)]
fn all_arms_transfer_field(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    idx: &FuncIndex,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    carrier: ArcVarId,
    root: ArcVarId,
    merge_block: usize,
    field: u32,
) -> bool {
    let decline = |gate: &str| {
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            carrier = carrier.index(),
            field,
            gate,
            "match-handoff verdict declined"
        );
    };
    // Gate 1: the field type is a TWO-variant sum with exactly ONE
    // single-payload variant (the Option-family shape).
    let Some(field_burden) = field_sum_burden(func, type_registry, carrier, field) else {
        decline("field-not-sum");
        return false;
    };
    // Per-variant payload count: the builtin Option/Result templates carry
    // the match-bound payload as a `transfers_on_match` Move rule (empty
    // `retained_owned`); user enums carry BOTH. A variant is payload-carrying
    // when either set is non-empty.
    let rows: Vec<usize> = field_burden
        .variant_burdens()
        .map(|v| v.transfers_on_match.len().max(v.retained_owned.len()))
        .collect();
    let payload_rows = rows.iter().filter(|&&n| n > 0).count();
    if rows.len() != 2 || payload_rows != 1 || rows.iter().any(|&n| n > 1) {
        decline("not-two-variant-single-payload");
        return false;
    }

    // Gate 2: exactly ONE scrutinee view — a Project of `field` off an alias
    // of the SAME root — whose tag projection drives exactly ONE Switch. The
    // view must be an inline Aggregate (a boxed sum's own allocation is NOT
    // discharged by a payload transfer).
    let mut scrutinee: Option<(ArcVarId, usize)> = None; // (view, switch block)
    for (&view, &(src, f)) in &idx.project_of {
        if f != field {
            continue;
        }
        if idx.resolve_root(src, param_edge_args) != Some(root) {
            continue;
        }
        if !matches!(func.var_repr(view), Some(ValueRepr::Aggregate)) {
            decline("view-not-inline-aggregate");
            return false;
        }
        let Some(switch_block) = switch_block_for_view(func, idx, view) else {
            decline("no-single-switch-for-view");
            return false;
        };
        if scrutinee.replace((view, switch_block)).is_some() {
            // Ambiguous: two switches over the same field. Decline.
            decline("ambiguous-scrutinee");
            return false;
        }
    }
    let Some((view, switch_block)) = scrutinee else {
        decline("no-scrutinee-view");
        return false;
    };
    let ArcTerminator::Switch { cases, default, .. } = &func.blocks[switch_block].terminator else {
        decline("scrutinee-block-not-switch");
        return false;
    };

    // Gate 3: one case per variant; the default arm must be vacuous
    // (Unreachable) — an executable default cannot be attributed.
    if cases.len() != rows.len()
        || !matches!(
            func.blocks[default.index()].terminator,
            ArcTerminator::Unreachable
        )
    {
        decline("cases-variants-mismatch-or-live-default");
        return false;
    }

    // Gate 4 (counting argument): exactly ONE arm extracts the payload and
    // unconditionally transfers it into the rebuild; the other arm extracts
    // nothing (the payload-less variant — vacuous). An arm matching the
    // payload variant WITHOUT extracting would leave zero extracting arms —
    // declined by the count.
    let mut extracting_arms = 0usize;
    for &(_, arm_target) in cases {
        if arm_extracts_payload(func, view, arm_target.index()) {
            extracting_arms += 1;
            if !arm_transfers_extract(func, idx, view, carrier, merge_block, arm_target.index()) {
                decline("arm-conditional-or-untransferred");
                return false;
            }
        }
    }
    if extracting_arms != payload_rows {
        decline("extracting-arm-count-mismatch");
        return false;
    }
    true
}

/// True iff `arm_block` contains a payload projection off `view` (any
/// `Project { value: view }` — the tag projection lives in the switch block,
/// never an arm block).
fn arm_extracts_payload(func: &ArcFunction, view: ArcVarId, arm_block: usize) -> bool {
    func.blocks[arm_block]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Project { value, .. } if *value == view))
}

/// Look up the carrier's owned-field entry for `field` and return its sum
/// burden when the field type carries variant burdens. `None` for non-sum
/// fields (struct / collection / scalar) — the verdict never fires on them.
fn field_sum_burden<'a>(
    func: &ArcFunction,
    type_registry: &'a TypeRegistry,
    carrier: ArcVarId,
    field: u32,
) -> Option<BurdenRef<'a>> {
    let carrier_ty: TypeRef = idx_to_type_ref(func.var_type(carrier), type_registry);
    let carrier_burden = lookup_burden(carrier_ty, type_registry)?;
    let field_ty = carrier_burden
        .owned_fields()
        .find(|of| of.field_path.first() == Some(&field))
        .map(|of| of.field_type)?;
    let field_burden = lookup_burden(field_ty, type_registry)?;
    field_burden.variant_burdens().next()?;
    Some(field_burden)
}

/// Find the block whose `Switch` scrutinee is the tag projection of `view`
/// (`Project { dst: tag, value: view, .. }` feeding `Switch { scrutinee: tag }`).
fn switch_block_for_view(func: &ArcFunction, idx: &FuncIndex, view: ArcVarId) -> Option<usize> {
    let mut found: Option<usize> = None;
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Switch { scrutinee, .. } = &block.terminator else {
            continue;
        };
        let Some(&(src, _)) = idx.project_of.get(scrutinee) else {
            continue;
        };
        if src != view {
            continue;
        }
        if found.replace(block_idx).is_some() {
            return None; // two switches over the same view's tag — ambiguous.
        }
    }
    found
}

/// One payload-carrying arm's verdict: the arm extracts the payload from
/// `view`, the arm path to `merge_block` is UNCONDITIONAL (Jump-only chain),
/// and the extract flows — through `Let { Var }` rebinds + the arm's own
/// per-edge block-param hops — into an owning `Construct` arg whose result
/// feeds the rebuild construct (the `Construct` consuming a projection of
/// `carrier`). Any other use of a flow member DECLINES (conservative).
fn arm_transfers_extract(
    func: &ArcFunction,
    idx: &FuncIndex,
    view: ArcVarId,
    carrier: ArcVarId,
    merge_block: usize,
    arm_block: usize,
) -> bool {
    let Some(extract) = sole_arm_extract(func, view, arm_block) else {
        // No extraction (the payload is dropped on this arm — the partial
        // dec IS its release) or an ambiguous double extraction. Decline.
        return false;
    };
    let Some(path_blocks) = jump_only_chain(func, arm_block, merge_block) else {
        // A Branch / Switch / Invoke before the merge = a conditional flow.
        // Decline (the pin-4 precision gate).
        return false;
    };

    // Flow-track the extract along the arm path. The wrap construct (owning
    // arg consume) must exist exactly once; its result must feed the rebuild
    // construct, OR the extract feeds the rebuild construct directly.
    let rebuild = rebuild_construct_in(func, idx, carrier, merge_block);
    let mut flow: FxHashSet<ArcVarId> = FxHashSet::default();
    flow.insert(extract);
    let mut wrap_dst: Option<ArcVarId> = None;
    let mut transferred_into_rebuild = false;

    for (i, &b) in path_blocks.iter().enumerate() {
        for instr in &func.blocks[b].body {
            // Skip instrs before the extract's definition in the arm block.
            if b == arm_block {
                if let ArcInstr::Project { dst, .. } = instr {
                    if *dst == extract {
                        continue;
                    }
                }
            }
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } if flow.contains(src) => {
                    flow.insert(*dst);
                }
                ArcInstr::Construct { dst, args, .. } if args.iter().any(|a| flow.contains(a)) => {
                    // Owning Construct-arg consume (RL-2 ConstructArg).
                    if wrap_dst.replace(*dst).is_some() {
                        return false; // two consuming constructs — ambiguous.
                    }
                    if Some(*dst) == rebuild {
                        transferred_into_rebuild = true;
                    }
                }
                // The wrap result feeding the rebuild construct.
                ArcInstr::Construct { dst, args, .. }
                    if wrap_dst.is_some_and(|w| {
                        args.iter()
                            .any(|a| *a == w || idx.alias_src.get(a) == Some(&w))
                    }) =>
                {
                    if Some(*dst) == rebuild {
                        transferred_into_rebuild = true;
                    }
                }
                other => {
                    // Any other use of a flow member declines (conservative:
                    // the payload escapes the modeled transfer).
                    if other.used_vars().iter().any(|u| flow.contains(u)) {
                        return false;
                    }
                }
            }
        }
        // Block boundary: the arm's own edge carries flow members into the
        // next block's params (per-edge attribution — this walk IS the edge).
        if let Some(&next) = path_blocks.get(i + 1) {
            let ArcTerminator::Jump { target, args } = &func.blocks[b].terminator else {
                return false;
            };
            debug_assert_eq!(target.index(), next);
            for (pos, a) in args.iter().enumerate() {
                if flow.contains(a) {
                    if let Some(&(param, _)) = func.blocks[next].params.get(pos) {
                        flow.insert(param);
                    }
                }
            }
        } else {
            // Terminal (merge) block: the carrier's partial dec lives here;
            // flow members must not escape via the terminator.
            if func.blocks[b]
                .terminator
                .used_vars()
                .iter()
                .any(|u| flow.contains(u))
            {
                return false;
            }
        }
    }

    // The wrap's own result feeding the rebuild may follow the wrap in a
    // later pass over the same block; re-check directly when not yet proven.
    if !transferred_into_rebuild {
        if let (Some(w), Some(r)) = (wrap_dst, rebuild) {
            transferred_into_rebuild = construct_args_contain(func, idx, r, w);
        }
    }
    wrap_dst.is_some() && transferred_into_rebuild
}

/// The arm's payload extraction: exactly ONE non-tag `Project` off `view`
/// defined in the arm block. (The tag projection lives in the switch block,
/// never an arm block, so any `Project` off `view` here is a payload read.)
/// `None` on zero or two extractions.
fn sole_arm_extract(func: &ArcFunction, view: ArcVarId, arm_block: usize) -> Option<ArcVarId> {
    let mut extract: Option<ArcVarId> = None;
    for instr in &func.blocks[arm_block].body {
        if let ArcInstr::Project { dst, value, .. } = instr {
            if *value == view && extract.replace(*dst).is_some() {
                return None; // two payload extractions — ambiguous.
            }
        }
    }
    extract
}

/// The UNCONDITIONAL arm path: the Jump-only block chain from `arm_block` to
/// `merge_block` inclusive. `None` when any block on the way terminates with
/// anything but a `Jump` (a conditional flow) or the walk cycles.
fn jump_only_chain(func: &ArcFunction, arm_block: usize, merge_block: usize) -> Option<Vec<usize>> {
    let mut path_blocks: Vec<usize> = vec![arm_block];
    let mut cur = arm_block;
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    while cur != merge_block {
        if !visited.insert(cur) {
            return None;
        }
        let ArcTerminator::Jump { target, .. } = &func.blocks[cur].terminator else {
            return None;
        };
        cur = target.index();
        path_blocks.push(cur);
    }
    Some(path_blocks)
}

/// The rebuild construct in `merge_block`: the `Construct` consuming a
/// projection of `carrier` (or a `Let { Var }` alias of one). `None` when
/// absent or ambiguous (two candidates).
fn rebuild_construct_in(
    func: &ArcFunction,
    idx: &FuncIndex,
    carrier: ArcVarId,
    merge_block: usize,
) -> Option<ArcVarId> {
    // Projections off the carrier (the rebuild's still-moved fields).
    let mut carrier_projections: FxHashSet<ArcVarId> = FxHashSet::default();
    for (&dst, &(src, _)) in &idx.project_of {
        if src == carrier {
            carrier_projections.insert(dst);
        }
    }
    let mut found: Option<ArcVarId> = None;
    for instr in &func.blocks[merge_block].body {
        let ArcInstr::Construct { dst, args, .. } = instr else {
            continue;
        };
        let consumes_projection = args.iter().any(|a| {
            carrier_projections.contains(a)
                || idx
                    .alias_src
                    .get(a)
                    .is_some_and(|s| carrier_projections.contains(s))
        });
        if consumes_projection && found.replace(*dst).is_some() {
            return None;
        }
    }
    found
}

/// True iff `construct`'s args contain `var` directly or via a one-hop
/// `Let { Var }` alias.
fn construct_args_contain(
    func: &ArcFunction,
    idx: &FuncIndex,
    construct: ArcVarId,
    var: ArcVarId,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, args, .. } = instr {
                if *dst == construct {
                    return args
                        .iter()
                        .any(|a| *a == var || idx.alias_src.get(a) == Some(&var));
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
