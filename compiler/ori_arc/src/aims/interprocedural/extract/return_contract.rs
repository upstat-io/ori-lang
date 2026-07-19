//! Return-contract extraction: structural definition tracing for
//! return-value uniqueness and freshness.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::super::super::contract::{MemoryContract, ReturnContract};
use super::super::super::lattice::Uniqueness;

use super::contract::{build_definition_map, build_invoke_def_map};

// Return info extraction

/// Determine return value uniqueness from the function's Return terminators.
///
/// Walks all Return terminators, traces each returned variable to its
/// definition instruction, and determines uniqueness based on how the
/// value was produced. Results from all return paths are joined.
pub(super) fn extract_return_info(
    func: &ArcFunction,
    _classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> ReturnContract {
    // Build definition map: var → defining instruction.
    let def_map = build_definition_map(func);

    // Build Invoke definition map: dst → callee name
    // (Invoke is a terminator, not an instruction).
    let invoke_defs = build_invoke_def_map(func);

    // Collect parameter variables for identity checks.
    let param_vars: rustc_hash::FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();

    // The `for_yield` finalizer name: a Return of `@ori_list_take(scratch)`
    // hands the caller a fresh independently owned buffer moved out of the comprehension
    // scratch — the surface-(a) fresh-self-alloc collection return.
    let list_take_name = interner.intern("ori_list_take");

    let mut return_uniqueness = None::<Uniqueness>;
    let mut all_preserve_freshness = true;
    // Optimistic AND-join across return paths: every path must produce a fresh
    // invocation-owned allocation for the contract to certify it (no path may
    // return existing, param-aliased, consumed-input, or callee-borrowed storage).
    let mut all_return_fresh_self_alloc = true;
    let mut saw_return = false;
    let lineage_trace_disabled = fresh_lineage_return_trace_disabled();

    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            saw_return = true;
            let (uniq, preserves) =
                var_uniqueness(*value, func, &def_map, &invoke_defs, &param_vars, sigs);

            return_uniqueness = Some(match return_uniqueness {
                None => uniq,
                Some(prev) => prev.join(uniq),
            });

            if !preserves {
                all_preserve_freshness = false;
            }

            if !var_is_fresh_self_alloc(*value, func, &def_map, &param_vars, list_take_name, sigs)
                && (lineage_trace_disabled
                    || !fresh_lineage_vars(
                        func,
                        &def_map,
                        &param_vars,
                        list_take_name,
                        sigs,
                        interner,
                    )
                    .contains(value))
            {
                all_return_fresh_self_alloc = false;
            }
        }
    }

    match return_uniqueness {
        Some(uniqueness) => ReturnContract {
            uniqueness,
            preserves_freshness: all_preserve_freshness,
            returns_fresh_self_alloc: saw_return && all_return_fresh_self_alloc,
            ..ReturnContract::CONSERVATIVE
        },
        // No Return terminators (e.g., infinite loop) — conservative.
        None => ReturnContract::CONSERVATIVE,
    }
}

fn report_fresh_lineage_return_trace_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE",
            effect = "decline loop-threaded fresh-lineage return certification",
            "ablation toggle fired"
        );
    }
    disabled
}

fn fresh_lineage_return_trace_disabled() -> bool {
    report_fresh_lineage_return_trace_toggle(
        std::env::var_os("ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE").is_some(),
    )
}

/// Legacy receiver-rooted COW mutators without a registry runtime identity.
/// Runtime-backed persistent List mutations are selected from the registry.
/// Concat is excluded because its result has two source operands.
const LEGACY_COW_RECEIVER_MUTATORS: &[&str] = &["pop", "updated"];

/// The fresh-lineage var set: every var whose value is provably THIS function's
/// own fresh collection allocation (or its COW replacement), never a
/// caller-visible buffer. Greatest fixpoint — start from the optimistic
/// closure, remove any member with counterevidence:
///  - seeds: collection `Construct`/`Reuse`/`CollectionReuse` dsts, the
///    `@ori_list_take` finalizer, callee-certified fresh returns;
///  - `Let { Var }` aliases of members;
///  - receiver-rooted COW mutator results (registry runtime mutations plus
///    [`LEGACY_COW_RECEIVER_MUTATORS`]) whose
///    receiver is a member;
///  - block params ALL of whose incoming `Jump`-arg feeders are members (the
///    loop-threaded rebuild: a self-consistent cycle stays; one non-member
///    feeder evicts — per `AimsProof.Partition::scc_external_source_determines`,
///    the external feeders determine the family's birth site).
fn fresh_lineage_vars(
    func: &ArcFunction,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    list_take_name: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> rustc_hash::FxHashSet<ArcVarId> {
    let cow_mutator = |callee: Name| {
        interner.try_lookup(callee).is_some_and(|name| {
            LEGACY_COW_RECEIVER_MUTATORS.contains(&name)
                || crate::borrow::persistent_list_runtime_methods()
                    .any(|method| method.name == name)
        })
    };
    let callee_certified = |callee: Name| {
        callee == list_take_name
            || sigs
                .get(&callee)
                .is_some_and(|c| c.return_info.returns_fresh_self_alloc)
    };
    // Optimistic start: every var is a candidate. Function params are never
    // fresh (caller-visible); everything else is refuted by its definition
    // shape below.
    let n_vars = func.var_types.len();
    let mut fresh = vec![true; n_vars];
    for p in param_vars {
        fresh[p.index()] = false;
    }
    // Invoke-terminator results: certified callee OR COW mutator over a
    // member receiver.
    let invoke_defs = build_invoke_def_map(func);
    // Block-param incoming feeders: (param -> Vec<feeder arg>).
    let mut param_feeders: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut block_params: rustc_hash::FxHashSet<ArcVarId> = rustc_hash::FxHashSet::default();
    for block in &func.blocks {
        for &(p, _) in &block.params {
            block_params.insert(p);
            param_feeders.entry(p).or_default();
        }
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_params = &func.blocks[target.index()].params;
            for (&arg, &(p, _)) in args.iter().zip(target_params.iter()) {
                param_feeders.entry(p).or_default().push(arg);
            }
        }
    }
    // Invoke receiver map: dst -> first arg (the COW receiver position).
    let mut invoke_receivers: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke { dst, args, .. } = &block.terminator {
            if let Some(&recv) = args.first() {
                invoke_receivers.insert(*dst, recv);
            }
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for raw in 0..n_vars {
            if !fresh[raw] {
                continue;
            }
            let var = ArcVarId::new(u32::try_from(raw).unwrap_or(u32::MAX));
            let ok = if let Some(instr) = def_map.get(&var) {
                match instr {
                    ArcInstr::Construct { ctor, .. } => ctor.is_collection_literal(),
                    ArcInstr::CollectionReuse { .. } | ArcInstr::Reuse { .. } => true,
                    ArcInstr::Apply {
                        func: callee, args, ..
                    } => {
                        callee_certified(*callee)
                            || (cow_mutator(*callee)
                                && args.first().is_some_and(|r| fresh[r.index()]))
                    }
                    ArcInstr::Let {
                        value: crate::ir::ArcValue::Var(source),
                        ..
                    } => fresh[source.index()],
                    _ => false,
                }
            } else if let Some(callee) = invoke_defs.get(&var) {
                callee_certified(*callee)
                    || (cow_mutator(*callee)
                        && invoke_receivers.get(&var).is_some_and(|r| fresh[r.index()]))
            } else if block_params.contains(&var) {
                let feeders = param_feeders.get(&var);
                feeders.is_some_and(|fs| !fs.is_empty() && fs.iter().all(|f| fresh[f.index()]))
            } else {
                false
            };
            if !ok {
                fresh[raw] = false;
                changed = true;
            }
        }
    }
    (0..n_vars)
        .filter(|&raw| fresh[raw])
        .map(|raw| ArcVarId::new(u32::try_from(raw).unwrap_or(u32::MAX)))
        .collect()
}

/// `true` iff `var` is defined (tracing through `Let { Var }` aliases) by a
/// fresh producer whose result the caller receives with one logical owner:
///  - the `for_yield` `@ori_list_take` finalizer (moves a fresh scratch buffer
///    out — surface (a)'s `clone_list` return);
///  - a `Construct` / `Reuse` / `CollectionReuse` of a COLLECTION (the
///    `ListLiteral`/`MapLiteral`/`SetLiteral` builders, `TF-3` fresh owner);
///  - an `Apply`/`Invoke` whose callee's contract ALREADY certifies
///    `returns_fresh_self_alloc` (transitive freshness — a forwarder returning
///    `clone_list(..)` also returns storage with no upstream alias).
///
/// Mirrors the FRESH-site set `fresh_self_alloc_dst` (`emit_unified.rs`) the
/// caller-side burden walk treats as self-allocating, restricted to the
/// COLLECTION shapes the fresh-collection-root admission consumes. A param
/// passthrough / `Project` borrow / contract-less call yields `false` (the
/// returned buffer may alias a caller-visible value — NOT a fresh self-alloc).
fn var_is_fresh_self_alloc(
    var: ArcVarId,
    func: &ArcFunction,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    list_take_name: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> bool {
    if param_vars.contains(&var) {
        return false;
    }
    let Some(instr) = def_map.get(&var) else {
        // Invoke-defined and unknown roots remain conservative because the
        // call site consumes the callee's freshness contract directly.
        return false;
    };
    match instr {
        ArcInstr::Construct { ctor, .. } if ctor.is_collection_literal() => true,
        // TF-3 certifies only self-contained structs/tuples whose references
        // all originate in this function. Parameter, view, enum-payload, and
        // closure-environment lineage remains caller-composed and uncertified.
        ArcInstr::Construct {
            ctor: crate::ir::CtorKind::Struct(_) | crate::ir::CtorKind::Tuple,
            args,
            ..
        } => args.iter().all(|&a| {
            let scalar_producer = match def_map.get(&a) {
                Some(ArcInstr::Let {
                    value: crate::ir::ArcValue::PrimOp { .. },
                    ..
                }) => func.primitive_facts.get(a).is_some_and(|fact| {
                    matches!(
                        fact.descriptor.result,
                        ori_registry::PrimitiveResultOwnership::Scalar
                    )
                }),
                Some(ArcInstr::Let {
                    value: crate::ir::ArcValue::Literal(lit),
                    ..
                }) => !matches!(lit, crate::ir::LitValue::String(_)),
                _ => false,
            };
            scalar_producer
                || var_is_fresh_self_alloc(a, func, def_map, param_vars, list_take_name, sigs)
        }),
        ArcInstr::CollectionReuse { .. } | ArcInstr::Reuse { .. } => true,
        ArcInstr::Apply { func: callee, .. } => {
            *callee == list_take_name
                || sigs
                    .get(callee)
                    .is_some_and(|c| c.return_info.returns_fresh_self_alloc)
        }
        // Trace through transparent `Let { Var }` aliases.
        ArcInstr::Let {
            value: crate::ir::ArcValue::Var(source),
            ..
        } => var_is_fresh_self_alloc(*source, func, def_map, param_vars, list_take_name, sigs),
        _ => false,
    }
}

/// Determine the uniqueness of a variable based on its definition.
///
/// Returns `(Uniqueness, preserves_freshness)`.
fn var_uniqueness(
    var: ArcVarId,
    func: &ArcFunction,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    invoke_defs: &FxHashMap<ArcVarId, Name>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(instr) = def_map.get(&var) {
        match instr {
            // Fresh construction, closure, COW reuse, or scalar check → unique.
            ArcInstr::Construct { .. }
            | ArcInstr::Reuse { .. }
            | ArcInstr::PartialApply { .. }
            | ArcInstr::CollectionReuse { .. }
            | ArcInstr::IsShared { .. } => (Uniqueness::Unique, true),

            // Direct call → use callee's return contract.
            ArcInstr::Apply { func: callee, .. } => callee_return_uniqueness(*callee, sigs),

            // Let binding → trace through to source.
            ArcInstr::Let { value, .. } => match value {
                crate::ir::ArcValue::Var(source) => {
                    if param_vars.contains(source) {
                        // Returning a parameter → preserves freshness
                        // (if caller passes unique, return is unique).
                        (Uniqueness::MaybeShared, true)
                    } else {
                        var_uniqueness(*source, func, def_map, invoke_defs, param_vars, sigs)
                    }
                }
                crate::ir::ArcValue::Literal(_) => (Uniqueness::Unique, true),
                crate::ir::ArcValue::PrimOp { args, .. } => match func
                    .primitive_facts
                    .get(var)
                    .unwrap_or_else(|| {
                        panic!("validated PrimOp v{} is missing its frozen fact", var.raw())
                    })
                    .descriptor
                    .result
                {
                    ori_registry::PrimitiveResultOwnership::Scalar
                    | ori_registry::PrimitiveResultOwnership::IndependentOwned
                    | ori_registry::PrimitiveResultOwnership::OwnedFromConsumedOrIndependent {
                        ..
                    } => (Uniqueness::Unique, true),
                    ori_registry::PrimitiveResultOwnership::Alias { operand } => {
                        let source = args.get(usize::from(operand)).copied().unwrap_or_else(|| {
                            panic!("validated primitive alias operand {operand} is out of bounds")
                        });
                        var_uniqueness(source, func, def_map, invoke_defs, param_vars, sigs)
                    }
                },
            },

            // Indirect calls, projections, selects, and side-effect-only
            // ownership/mutation operations remain conservative.
            ArcInstr::ApplyIndirect { .. }
            | ArcInstr::Project { .. }
            | ArcInstr::Select { .. }
            | ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::RcDecPartial { .. }
            | ArcInstr::RcDecField { .. }
            | ArcInstr::RcDecVariant { .. }
            | ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::BurdenDecPartial { .. }
            | ArcInstr::BurdenDecField { .. }
            | ArcInstr::BurdenDecVariant { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. }
            | ArcInstr::Reset { .. } => (Uniqueness::MaybeShared, false),
        }
    } else if let Some(callee) = invoke_defs.get(&var) {
        // Defined by an Invoke terminator → use callee's return contract.
        callee_return_uniqueness(*callee, sigs)
    } else if param_vars.contains(&var) {
        // Returning a parameter directly → uniqueness depends on caller.
        (Uniqueness::MaybeShared, true)
    } else {
        // Block parameter or unknown definition → conservative.
        (Uniqueness::MaybeShared, false)
    }
}

/// Look up a callee's return uniqueness from its contract.
fn callee_return_uniqueness(
    callee: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(contract) = sigs.get(&callee) {
        (
            contract.return_info.uniqueness,
            contract.return_info.preserves_freshness,
        )
    } else {
        (Uniqueness::MaybeShared, false)
    }
}

#[cfg(test)]
mod toggle_tests {
    crate::test_helpers::ablation_env_event_test!(
        fresh_lineage_return_trace_toggle_reports_effect,
        "ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE",
        "decline loop-threaded fresh-lineage return certification",
        super::fresh_lineage_return_trace_disabled,
    );
}
