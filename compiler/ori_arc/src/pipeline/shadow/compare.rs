//! Shadow comparison logic: AIMS vs legacy per-function analysis.
//!
//! Compares five dimensions: param ownership, return uniqueness, COW
//! annotations, RC operation counts, and per-call-site arg ownership.

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use crate::aims::lattice::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArgOwnership};
use crate::ownership::Ownership;
use crate::pipeline::rc_count::RcOpCount;
use crate::uniqueness::{CowAnnotations, UniquenessSummary};

use super::{AimsSnapshot, DimensionResult, FunctionComparison, ShadowComparisonReport};

/// Compare all functions and build the aggregate report.
pub(super) fn compare_all(
    functions: &[ArcFunction],
    aims_snapshots: &FxHashMap<Name, AimsSnapshot>,
    legacy_rc_counts: &FxHashMap<Name, RcOpCount>,
    old_summaries: &FxHashMap<Name, UniquenessSummary>,
    interner: &StringInterner,
) -> ShadowComparisonReport {
    let mut report = ShadowComparisonReport {
        per_function: Vec::new(),
        total_functions: functions.len(),
        param_matches: 0,
        param_improvements: 0,
        param_regressions: 0,
        return_matches: 0,
        return_improvements: 0,
        return_regressions: 0,
        cow_matches: 0,
        cow_improvements: 0,
        cow_regressions: 0,
        rc_matches: 0,
        rc_improvements: 0,
        rc_regressions: 0,
        arg_ownership_matches: 0,
        arg_ownership_improvements: 0,
        arg_ownership_regressions: 0,
        aims_rc_total: 0,
        legacy_rc_total: 0,
        immortal_skips_total: 0,
    };

    for func in functions {
        let snapshot = aims_snapshots.get(&func.name);
        let old_summary = old_summaries.get(&func.name);
        let legacy_rc = legacy_rc_counts
            .get(&func.name)
            .copied()
            .unwrap_or_default();
        let name_str = interner.lookup(func.name).to_string();

        let comparison =
            compare_function(func, snapshot, old_summary, legacy_rc, name_str, interner);

        tally_dimension(
            &comparison.param_ownership,
            &mut report.param_matches,
            &mut report.param_improvements,
            &mut report.param_regressions,
        );
        tally_dimension(
            &comparison.return_uniqueness,
            &mut report.return_matches,
            &mut report.return_improvements,
            &mut report.return_regressions,
        );
        tally_dimension(
            &comparison.cow_annotations,
            &mut report.cow_matches,
            &mut report.cow_improvements,
            &mut report.cow_regressions,
        );
        tally_dimension(
            &comparison.rc_ops,
            &mut report.rc_matches,
            &mut report.rc_improvements,
            &mut report.rc_regressions,
        );
        tally_dimension(
            &comparison.arg_ownership,
            &mut report.arg_ownership_matches,
            &mut report.arg_ownership_improvements,
            &mut report.arg_ownership_regressions,
        );

        if let Some(snap) = snapshot {
            report.aims_rc_total += snap.rc_ops.total();
        }
        report.legacy_rc_total += legacy_rc.total();
        report.immortal_skips_total += comparison.immortal_skips;

        report.per_function.push(comparison);
    }

    report
}

fn tally_dimension(
    result: &DimensionResult,
    matches: &mut usize,
    improvements: &mut usize,
    regressions: &mut usize,
) {
    match result {
        DimensionResult::Match => *matches += 1,
        DimensionResult::Improvement(_) => *improvements += 1,
        DimensionResult::Regression(_) => *regressions += 1,
        DimensionResult::Skipped(_) => {}
    }
}

pub(super) fn compare_function(
    func: &ArcFunction,
    snapshot: Option<&AimsSnapshot>,
    old_summary: Option<&UniquenessSummary>,
    legacy_rc: RcOpCount,
    name_str: String,
    interner: &StringInterner,
) -> FunctionComparison {
    let Some(snapshot) = snapshot else {
        return FunctionComparison {
            name: func.name,
            name_str,
            param_ownership: DimensionResult::Skipped("no AIMS contract".into()),
            return_uniqueness: DimensionResult::Skipped("no AIMS contract".into()),
            cow_annotations: DimensionResult::Skipped("no AIMS snapshot".into()),
            rc_ops: DimensionResult::Skipped("no AIMS snapshot".into()),
            arg_ownership: DimensionResult::Skipped("no AIMS snapshot".into()),
            immortal_skips: 0,
        };
    };

    let legacy_sites = extract_arg_ownership_sites(func);

    FunctionComparison {
        name: func.name,
        param_ownership: compare_param_ownership(func, snapshot),
        return_uniqueness: compare_return_uniqueness(snapshot, old_summary),
        cow_annotations: compare_cow_annotations(&snapshot.cow_annotations, &func.cow_annotations),
        rc_ops: compare_rc_ops(snapshot.rc_ops, legacy_rc),
        arg_ownership: compare_arg_ownership(
            &snapshot.arg_ownership_sites,
            &legacy_sites,
            interner,
        ),
        immortal_skips: snapshot.immortal_count,
        name_str,
    }
}

/// Compare parameter ownership: AIMS contracts vs legacy borrow inference.
///
/// AIMS `Borrowed` where legacy says `Owned` = improvement (fewer RC ops).
/// AIMS `Owned` where legacy says `Borrowed` = regression (more RC ops).
fn compare_param_ownership(func: &ArcFunction, snapshot: &AimsSnapshot) -> DimensionResult {
    let Some(contract) = &snapshot.contract else {
        return DimensionResult::Skipped("no AIMS contract".into());
    };

    if func.params.len() != contract.params.len() {
        return DimensionResult::Skipped(format!(
            "param count mismatch: legacy={}, AIMS={}",
            func.params.len(),
            contract.params.len()
        ));
    }

    let mut improvements = Vec::new();
    let mut regressions = Vec::new();

    for (i, (param, pc)) in func.params.iter().zip(&contract.params).enumerate() {
        let aims_ownership = match pc.access {
            AccessClass::Borrowed => Ownership::Borrowed,
            AccessClass::Owned => Ownership::Owned,
        };

        if aims_ownership == param.ownership {
            continue;
        }

        if aims_ownership == Ownership::Borrowed {
            improvements.push(format!("param {i}: AIMS=Borrowed, legacy=Owned"));
        } else {
            regressions.push(format!("param {i}: AIMS=Owned, legacy=Borrowed"));
        }
    }

    if !regressions.is_empty() {
        DimensionResult::Regression(regressions.join("; "))
    } else if !improvements.is_empty() {
        DimensionResult::Improvement(improvements.join("; "))
    } else {
        DimensionResult::Match
    }
}

/// Compare return uniqueness: AIMS contract vs legacy `UniquenessSummary`.
///
/// AIMS `Unique` where legacy says `MaybeShared` = improvement.
/// AIMS `MaybeShared` where legacy says `Unique` = regression.
pub(super) fn compare_return_uniqueness(
    snapshot: &AimsSnapshot,
    old_summary: Option<&UniquenessSummary>,
) -> DimensionResult {
    let Some(contract) = &snapshot.contract else {
        return DimensionResult::Skipped("no AIMS contract".into());
    };
    let Some(old) = old_summary else {
        return DimensionResult::Skipped("no legacy summary".into());
    };

    let aims_rank = uniqueness_rank_aims(contract.return_info.uniqueness);
    let legacy_rank = uniqueness_rank_legacy(old.return_val);

    match aims_rank.cmp(&legacy_rank) {
        std::cmp::Ordering::Equal => DimensionResult::Match,
        // Lower rank = tighter (more optimistic) = improvement
        std::cmp::Ordering::Less => DimensionResult::Improvement(format!(
            "AIMS={}, legacy={}",
            aims_uniqueness_name(contract.return_info.uniqueness),
            old.return_val,
        )),
        std::cmp::Ordering::Greater => DimensionResult::Regression(format!(
            "AIMS={}, legacy={}",
            aims_uniqueness_name(contract.return_info.uniqueness),
            old.return_val,
        )),
    }
}

/// Compare COW annotations by counting `StaticUnique` optimizations.
///
/// More `StaticUnique` = better (more COW checks eliminated).
/// Compares counts rather than per-operation matching because the two
/// pipelines may use different block numbering.
pub(super) fn compare_cow_annotations(
    aims_cow: &CowAnnotations,
    legacy_cow: &CowAnnotations,
) -> DimensionResult {
    let aims_su = aims_cow.static_unique_count();
    let legacy_su = legacy_cow.static_unique_count();
    let aims_total = aims_cow.len();
    let legacy_total = legacy_cow.len();

    match aims_su.cmp(&legacy_su) {
        std::cmp::Ordering::Equal => DimensionResult::Match,
        std::cmp::Ordering::Greater => DimensionResult::Improvement(format!(
            "StaticUnique: AIMS={aims_su}, legacy={legacy_su} (total: AIMS={aims_total}, legacy={legacy_total})"
        )),
        std::cmp::Ordering::Less => DimensionResult::Regression(format!(
            "StaticUnique: AIMS={aims_su}, legacy={legacy_su} (total: AIMS={aims_total}, legacy={legacy_total})"
        )),
    }
}

/// Compare RC operation counts between AIMS and legacy pipelines.
///
/// Fewer total RC ops = better (less runtime overhead).
/// AIMS < legacy = improvement; AIMS > legacy = regression.
pub(super) fn compare_rc_ops(aims: RcOpCount, legacy: RcOpCount) -> DimensionResult {
    let aims_total = aims.total();
    let legacy_total = legacy.total();

    match aims_total.cmp(&legacy_total) {
        std::cmp::Ordering::Equal => DimensionResult::Match,
        std::cmp::Ordering::Less => DimensionResult::Improvement(format!(
            "AIMS={aims} ({aims_total}), legacy={legacy} ({legacy_total}), saved {}",
            legacy_total - aims_total,
        )),
        std::cmp::Ordering::Greater => DimensionResult::Regression(format!(
            "AIMS={aims} ({aims_total}), legacy={legacy} ({legacy_total}), excess {}",
            aims_total - legacy_total,
        )),
    }
}

/// Extract per-call-site `arg_ownership` vectors from a function.
///
/// Walks all blocks and instructions, collecting `(call_target, arg_ownership)`
/// for every `Apply` instruction and `Invoke` terminator. The result is ordered
/// by block index then instruction index, providing a deterministic ordering
/// for comparison.
pub(super) fn extract_arg_ownership_sites(
    func: &ArcFunction,
) -> Vec<(ori_ir::Name, Vec<ArgOwnership>)> {
    let mut sites = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                arg_ownership,
                ..
            } = instr
            {
                sites.push((*callee, arg_ownership.clone()));
            }
        }
        if let ArcTerminator::Invoke {
            func: callee,
            arg_ownership,
            ..
        } = &block.terminator
        {
            sites.push((*callee, arg_ownership.clone()));
        }
    }
    sites
}

/// Compare per-call-site `arg_ownership` between AIMS and legacy pipelines.
///
/// Both pipelines populate `arg_ownership` on `Apply`/`Invoke` instructions
/// before any block-altering passes, so call sites should appear in the same
/// order. The comparison walks sites in parallel:
///
/// - AIMS `Borrowed` where legacy says `Owned` = improvement (fewer RC ops)
/// - AIMS `Owned` where legacy says `Borrowed` = regression (more RC ops)
///
/// If the number of call sites differs, the dimension is skipped (indicates
/// a structural divergence between pipelines).
pub(super) fn compare_arg_ownership(
    aims_sites: &[(ori_ir::Name, Vec<ArgOwnership>)],
    legacy_sites: &[(ori_ir::Name, Vec<ArgOwnership>)],
    interner: &StringInterner,
) -> DimensionResult {
    if aims_sites.len() != legacy_sites.len() {
        return DimensionResult::Skipped(format!(
            "call site count mismatch: AIMS={}, legacy={}",
            aims_sites.len(),
            legacy_sites.len()
        ));
    }

    let mut improvements = Vec::new();
    let mut regressions = Vec::new();

    for (site_idx, (aims, legacy)) in aims_sites.iter().zip(legacy_sites).enumerate() {
        let (aims_callee, aims_ownership) = aims;
        let (_legacy_callee, legacy_ownership) = legacy;

        // If arg counts differ at the same call site, skip this site.
        if aims_ownership.len() != legacy_ownership.len() {
            regressions.push(format!(
                "site {site_idx} ({}): arg count mismatch AIMS={}, legacy={}",
                interner.lookup(*aims_callee),
                aims_ownership.len(),
                legacy_ownership.len(),
            ));
            continue;
        }

        for (arg_idx, (a, l)) in aims_ownership.iter().zip(legacy_ownership).enumerate() {
            if a == l {
                continue;
            }

            let callee_name = interner.lookup(*aims_callee);
            if *a == ArgOwnership::Borrowed {
                improvements.push(format!(
                    "site {site_idx} ({callee_name}) arg {arg_idx}: AIMS=Borrowed, legacy=Owned"
                ));
            } else {
                regressions.push(format!(
                    "site {site_idx} ({callee_name}) arg {arg_idx}: AIMS=Owned, legacy=Borrowed"
                ));
            }
        }
    }

    if !regressions.is_empty() {
        DimensionResult::Regression(regressions.join("; "))
    } else if !improvements.is_empty() {
        DimensionResult::Improvement(improvements.join("; "))
    } else {
        DimensionResult::Match
    }
}

// Helpers

fn uniqueness_rank_aims(u: crate::aims::lattice::Uniqueness) -> u8 {
    match u {
        crate::aims::lattice::Uniqueness::Unique => 0,
        crate::aims::lattice::Uniqueness::MaybeShared => 1,
        crate::aims::lattice::Uniqueness::Shared => 2,
    }
}

fn uniqueness_rank_legacy(u: crate::uniqueness::Uniqueness) -> u8 {
    match u {
        crate::uniqueness::Uniqueness::Unique => 0,
        crate::uniqueness::Uniqueness::MaybeShared => 1,
        crate::uniqueness::Uniqueness::Shared => 2,
    }
}

fn aims_uniqueness_name(u: crate::aims::lattice::Uniqueness) -> &'static str {
    match u {
        crate::aims::lattice::Uniqueness::Unique => "Unique",
        crate::aims::lattice::Uniqueness::MaybeShared => "MaybeShared",
        crate::aims::lattice::Uniqueness::Shared => "Shared",
    }
}
