//! Verdict gates for the match-handoff extract-transfer attribution: the
//! all-arms field verdict ([`all_arms_transfer_field`]) and its structural
//! sub-gates (scrutinee-view discovery, arm extraction, unconditional-path
//! `jump_only_chain`, rebuild-construct identification, transfer flow-track).
//! Spec: Annex E §AIMS RL-2.

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use ori_types::TypeRegistry;

use crate::aims::intraprocedural::project_aliases::ParamEdgeArg;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::super::super::burden::{Burden, BurdenRef, TypeRef};
use super::super::super::burden_lookup::{idx_to_type_ref, lookup_burden};
use super::FuncIndex;

/// The all-arms verdict for one still-released sum field of one carrier
/// (per-edge identity, NEVER a use-count proxy): every switch arm over the
/// field's tag either transfers the extracted payload into the rebuild
/// construct (unconditional arm path) or is the payload-less-variant arm.
/// v1 shape gate: a TWO-variant single-payload sum (the `Option`-family
/// match-handoff), with the payload arm identified by the counting argument
/// (exactly one payload row, exactly one extracting arm) — never by
/// variant-id ordinal (burden-row and runtime-tag numbering are independent).
#[expect(
    clippy::too_many_arguments,
    reason = "verdict inputs are the per-carrier structural indexes; grouping \
              would add indirection to a single-call-site helper"
)]
pub(super) fn all_arms_transfer_field(
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
                            .any(|a| *a == w || idx.chain.alias_src.get(a) == Some(&w))
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
                    .chain
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
                        .any(|a| *a == var || idx.chain.alias_src.get(a) == Some(&var));
                }
            }
        }
    }
    false
}
