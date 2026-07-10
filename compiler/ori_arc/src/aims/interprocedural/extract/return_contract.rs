//! Return-contract extraction: structural definition tracing for
//! return-value uniqueness and freshness.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::super::super::contract::{MemoryContract, ReturnContract};
use super::super::super::lattice::Uniqueness;

use super::{build_definition_map, build_invoke_def_map};

// Return info extraction

/// Determine return value uniqueness from the function's Return terminators.
///
/// Walks all Return terminators, traces each returned variable to its
/// definition instruction, and determines uniqueness based on how the
/// value was produced. Results from all return paths are joined.
pub(super) fn extract_return_info(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
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
    // hands the caller a FRESH rc=1 buffer moved out of the comprehension
    // scratch — the surface-(a) fresh-self-alloc collection return.
    let list_take_name = interner.intern("ori_list_take");

    let mut return_uniqueness = None::<Uniqueness>;
    let mut all_preserve_freshness = true;
    // Optimistic AND-join across return paths: every path must produce a fresh
    // self-alloc for the contract to certify it (no path may return an existing
    // / param-aliased / callee-borrowed buffer).
    let mut all_return_fresh_self_alloc = true;
    let mut saw_return = false;

    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            saw_return = true;
            let (uniq, preserves) = var_uniqueness(
                *value,
                &def_map,
                &invoke_defs,
                &param_vars,
                classifier,
                sigs,
            );

            return_uniqueness = Some(match return_uniqueness {
                None => uniq,
                Some(prev) => prev.join(uniq),
            });

            if !preserves {
                all_preserve_freshness = false;
            }

            // Env: ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE — declines the
            // loop-threaded fresh-lineage return certification for bisection,
            // debug-only
            let lineage_trace_disabled =
                std::env::var_os("ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE").is_some();
            if !var_is_fresh_self_alloc(*value, &def_map, &param_vars, list_take_name, sigs)
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

/// Receiver-rooted COW list mutators whose result is the receiver's own logical
/// buffer (in-place at rc=1, or a fresh COW replacement at rc=1) — freshness of
/// the receiver lineage carries to the result. Concat is excluded (two-operand).
const COW_RECEIVER_MUTATORS: &[&str] = &["push", "pop", "set", "insert", "remove", "updated"];

/// The fresh-lineage var set: every var whose value is provably THIS function's
/// own fresh collection allocation (or its COW replacement), never a
/// caller-visible buffer. Greatest fixpoint — start from the optimistic
/// closure, remove any member with counterevidence:
///  - seeds: collection `Construct`/`Reuse`/`CollectionReuse` dsts, the
///    `@ori_list_take` finalizer, callee-certified fresh returns;
///  - `Let { Var }` aliases of members;
///  - receiver-rooted COW mutator results ([`COW_RECEIVER_MUTATORS`]) whose
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
        interner
            .try_lookup(callee)
            .is_some_and(|n| COW_RECEIVER_MUTATORS.contains(&n))
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
/// FRESH self-allocating instruction whose result the caller receives at rc=1:
///  - the `for_yield` `@ori_list_take` finalizer (moves a fresh scratch buffer
///    out — surface (a)'s `clone_list` return);
///  - a `Construct` / `Reuse` / `CollectionReuse` of a COLLECTION (the
///    `ListLiteral`/`MapLiteral`/`SetLiteral` builders, `TF-3` FRESH rc=1);
///  - an `Apply`/`Invoke` whose callee's contract ALREADY certifies
///    `returns_fresh_self_alloc` (transitive freshness — a forwarder returning
///    `clone_list(..)` is itself fresh).
///
/// Mirrors the FRESH-site set `fresh_self_alloc_dst` (`emit_unified.rs`) the
/// caller-side burden walk treats as self-allocating, restricted to the
/// COLLECTION shapes the fresh-collection-root admission consumes. A param
/// passthrough / `Project` borrow / contract-less call yields `false` (the
/// returned buffer may alias a caller-visible value — NOT a fresh self-alloc).
fn var_is_fresh_self_alloc(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    list_take_name: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> bool {
    if param_vars.contains(&var) {
        return false;
    }
    let Some(instr) = def_map.get(&var) else {
        // Block-param / Invoke-defined / unknown → conservative (not certified
        // fresh). The Invoke-result case is deliberately NOT certified here: a
        // function returning `@clone_list(..)` directly is rare; the surface-(a)
        // cure consumes the CALLEE's contract at the call site, not the
        // forwarder's own return.
        return false;
    };
    match instr {
        ArcInstr::Construct { ctor, .. } if ctor.is_collection_literal() => true,
        // A SELF-CONTAINED named-struct / tuple `Construct` — every arg is
        // itself a fresh self-alloc or a scalar producer (`PrimOp` /
        // non-string literal) — is a fresh whole-var unit: its ref-bundle
        // consumes only refs this function birthed. A construct threading a
        // param / alias / extracted view (`Wrapper { inner: p }` — the
        // aggregate-transfer-forwarder shape) stays uncertified: its
        // whole-var accounting composes with the caller's transfer machinery.
        // An `EnumVariant` (niche-family sum: the payload's OWN allocation)
        // and a `Closure` stay uncertified — their whole-var accounting is
        // shared with the payload / env lineage. Spec: Annex E §AIMS TF-3 +
        // §1.9.
        ArcInstr::Construct {
            ctor: crate::ir::CtorKind::Struct(_) | crate::ir::CtorKind::Tuple,
            args,
            ..
        } => args.iter().all(|&a| {
            let scalar_producer = match def_map.get(&a) {
                Some(ArcInstr::Let {
                    value: crate::ir::ArcValue::PrimOp { .. },
                    ..
                }) => true,
                Some(ArcInstr::Let {
                    value: crate::ir::ArcValue::Literal(lit),
                    ..
                }) => !matches!(lit, crate::ir::LitValue::String(_)),
                _ => false,
            };
            scalar_producer || var_is_fresh_self_alloc(a, def_map, param_vars, list_take_name, sigs)
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
        } => var_is_fresh_self_alloc(*source, def_map, param_vars, list_take_name, sigs),
        _ => false,
    }
}

/// Determine the uniqueness of a variable based on its definition.
///
/// Returns `(Uniqueness, preserves_freshness)`.
fn var_uniqueness(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    invoke_defs: &FxHashMap<ArcVarId, Name>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    classifier: &dyn ArcClassification,
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
            ArcInstr::Let { value, ty, .. } => match value {
                crate::ir::ArcValue::Var(source) => {
                    if param_vars.contains(source) {
                        // Returning a parameter → preserves freshness
                        // (if caller passes unique, return is unique).
                        (Uniqueness::MaybeShared, true)
                    } else {
                        var_uniqueness(*source, def_map, invoke_defs, param_vars, classifier, sigs)
                    }
                }
                crate::ir::ArcValue::Literal(_) => (Uniqueness::Unique, true),
                crate::ir::ArcValue::PrimOp { .. } => {
                    if classifier.is_scalar(*ty) {
                        (Uniqueness::Unique, true)
                    } else {
                        (Uniqueness::MaybeShared, false)
                    }
                }
            },

            // Indirect call, projection, select, RC/mutation ops → conservative.
            // BurdenInc/BurdenDec are side-effect-only annotations (no dst);
            // they cannot appear in def_map but the exhaustive match must
            // cover them — group with the other side-effect-only ops.
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
