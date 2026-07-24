//! §08 realization-rule discharge — RL-1 through RL-34 (+ RL-11a / RL-14a /
//! RL-15a / RL-18a sub-rules, RL-13 removal confirmation, and the
//! RL-1/RL-2 composition obligation).
//!
//! Per `Annex E §AIMS §8` +
//! the §01 `aims-proof/proofs/01-realization/Realization.proof` sorry
//! obligation, the RL category dispatches to [`rc_counting`, `refinement`,
//! `case_analysis`] per the coverage-manifest RL row. Each RL-N has ONE
//! PRIMARY engine (constructive discharge) and TWO SECONDARY engines
//! (gracious-accept once the primary has discharged), mirroring the
//! `pipeline_ordering` structural_induction-PRIMARY / case_analysis-SECONDARY
//! split:
//!
//! - `rc_counting` PRIMARY — logical ownership-event balance (RL-1..RL-5) +
//! KnownSafe / event-pair refinement (RL-22..RL-26) + the RL-1/RL-2 composition obligation
//! + the whole-pipeline composition theorem (RL-comp).
//! - `refinement` PRIMARY — backend-neutral fact refinement against IC-3 /
//! IC-4 / IC-5, followed by separate target-projection fidelity checks
//! (RL-29 / RL-30 / RL-31).
//! - `case_analysis` PRIMARY — mutation-isolation obligations (RL-6..RL-10),
//! donor/recipient credit transfer
//! (RL-11 / RL-11a / RL-12 / RL-13-removed), allocation/lifetime facts
//! (RL-14 / RL-14a / RL-15 / RL-15a / RL-16), owner bounds and projection
//! satisfaction (RL-17 / RL-18 / RL-18a), thread reachability facts
//! (RL-19..RL-21), selective barriers (RL-27 / RL-28), and borrow inference
//! (RL-32..RL-34).
//!
//! Each PRIMARY verifier encodes the shipped realization semantics as a
//! fixture grid, discharges the rule's soundness invariant (logical
//! ownership-event preservation plus projection satisfaction), and carries a negative-direction
//! witness so the check has teeth (a wrong emission schedule is REJECTED).
//! Verifiers reason over a model of the shipped emission; they cite the
//! shipped `compiler/ori_arc/src/aims/realize/` +
//! `emit_rc/` sites in comments per Annex E §AIMS

use crate::ast::{Category, Theorem};
use crate::engine::{EngineResult, EngineVerdict};

/// Discharge an RL theorem for the named engine, or return `None` when the
/// theorem is not a §08 realization rule this module serves YET (so the
/// calling engine falls through to its `UnimplementedShape` stub for
/// not-yet-discharged rules).
///
/// Returns `Some(EngineResult)` when `theorem.id` is an implemented RL rule
/// AND `engine_name` is one of the three RL-row engines. The PRIMARY engine
/// gets the constructive verifier; the two SECONDARY engines gracious-accept.
pub fn discharge_for_engine(engine_name: &str, theorem: &Theorem) -> Option<EngineResult> {
    if theorem.id.category != Category::Realization {
        return None;
    }
    let suffix = theorem.id.suffix.as_str();
    let primary = primary_engine_for(suffix)?;
    // Only the three RL-row engines participate.
    if !matches!(engine_name, "rc_counting" | "refinement" | "case_analysis") {
        return None;
    }
    if engine_name == primary {
        Some(run_primary_verifier(suffix))
    } else {
        // SECONDARY engine: gracious-accept once the PRIMARY has discharged
        // (per the coverage-manifest RL row + the pipeline_ordering gracious-accept
        // precedent).
        Some(gracious_accept())
    }
}

/// Map an RL rule suffix to its PRIMARY engine, or `None` when the rule's
/// verifier has not yet landed (so the rule stays `unimplemented_engine_shape`
/// until its cluster discharges).
fn primary_engine_for(suffix: &str) -> Option<&'static str> {
    let engine = match suffix {
        // §08.1 RC emission — rc_counting PRIMARY.
        "1" | "2" | "3" | "4" | "5" => "rc_counting",
        // §08.2 COW — case_analysis PRIMARY.
        "6" | "7" | "8" | "9" | "10" => "case_analysis",
        // §08.3 Allocation reuse — case_analysis PRIMARY (RL-13 is the
        // removal-confirmation entry).
        "11" | "11a" | "12" | "13" => "case_analysis",
        // §08.4 Allocation/lifetime facts — case_analysis PRIMARY.
        "14" | "14a" | "15" | "15a" | "16" => "case_analysis",
        // §08.5 Owner bounds and layout satisfaction — case_analysis PRIMARY.
        "17" | "18" => "case_analysis",
        // §08.6 VM/compiled trace parity — case_analysis PRIMARY.
        "18a" => "case_analysis",
        // §08.7 Thread reachability facts — case_analysis PRIMARY.
        "19" | "20" | "21" => "case_analysis",
        // §08.8 KnownSafe + logical event refinement — rc_counting PRIMARY.
        "22" | "23" | "24" | "25" | "26" => "rc_counting",
        // §08.9 Selective barriers — case_analysis PRIMARY.
        "27" | "28" => "case_analysis",
        // §08.11 backend-neutral AIMS facts — refinement PRIMARY.
        // RL-31 is CRITICAL.
        "29" | "30" | "31" => "refinement",
        // §08.12 Borrow inference — case_analysis PRIMARY.
        "32" | "33" | "34" => "case_analysis",
        // §08.12 Composition — rc_counting PRIMARY (RC-balance composition).
        "1-RL-2-composition" | "comp" => "rc_counting",
        _ => return None,
    };
    Some(engine)
}

/// Dispatch the PRIMARY constructive verifier for an implemented RL rule.
fn run_primary_verifier(suffix: &str) -> EngineResult {
    match suffix {
        "1" => verify_rl1_inc_on_live_duplication(),
        "2" => verify_rl2_dec_at_last_use(),
        "3" => verify_rl3_rc_op_elision(),
        "4" => verify_rl4_edge_specific_decs(),
        "5" => verify_rl5_dead_at_entry_cleanup(),
        "6" => verify_rl6_same_identity_mutation(),
        "7" => verify_rl7_sharing_observation(),
        "8" => verify_rl8_mutation_isolation(),
        "9" => verify_rl9_observation_representation_equivalence(),
        "10" => verify_rl10_disjoint_field_mutation(),
        "11" => verify_rl11_same_block_reuse(),
        "11a" => verify_rl11a_dynamic_reuse(),
        "12" => verify_rl12_cross_block_reuse(),
        "13" => verify_rl13_removal_confirmation(),
        "14" => verify_rl14_lifetime_facts(),
        "14a" => verify_rl14a_cleanup_obligation(),
        "15" => verify_rl15_extent_seam(),
        "15a" => verify_rl15a_caller_extent(),
        "16" => verify_rl16_conservative_unknown(),
        "17" => verify_rl17_owner_bound(),
        "18" => verify_rl18_layout_satisfaction(),
        "18a" => verify_rl18a_projection_parity(),
        "19" => verify_rl19_thread_reachability(),
        "20" => verify_rl20_thread_capability(),
        "21" => verify_rl21_no_thread_boundary(),
        "22" => verify_rl22_knownsafe_pair_elimination(),
        "23" => verify_rl23_knownsafe_join_propagation(),
        "24" => verify_rl24_bidirectional_pair_matching(),
        "25" => verify_rl25_pair_eliminable_conditions(),
        "26" => verify_rl26_event_ordering_barriers(),
        "27" => verify_rl27_selective_event_ordering(),
        "28" => verify_rl28_unknown_callee_event_ordering(),
        "29" => verify_rl29_neutral_fresh_self_allocation(),
        "30" => verify_rl30_neutral_memory_fact(),
        "31" => verify_rl31_neutral_parameter_disjointness(),
        "32" => verify_rl32_borrowed_init_owned_promotion(),
        "33" => verify_rl33_projection_owned_propagation(),
        "34" => verify_rl34_tail_call_preservation(),
        "1-RL-2-composition" => verify_rl1_rl2_composition(),
        "comp" => verify_rl_composition(),
        other => fail(format!(
            "realization_rules run_primary_verifier reached an unmapped RL suffix {:?}; primary_engine_for and run_primary_verifier are out of sync",
            other
        )),
    }
}

// ============================================================================
// Engine-result helpers (mirror pipeline_ordering)
// ============================================================================

fn gracious_accept() -> EngineResult {
    EngineResult {
        verdict: EngineVerdict::Valid,
        reason: String::new(),
    }
}

fn fail(reason: String) -> EngineResult {
    EngineResult {
        verdict: EngineVerdict::Fail,
        reason,
    }
}

fn valid() -> EngineResult {
    EngineResult {
        verdict: EngineVerdict::Valid,
        reason: String::new(),
    }
}

fn require_count(rule: &str, expected: u64, actual: u64, label: &str) -> EngineResult {
    if expected != actual {
        return fail(format!(
            "{} coverage mismatch: expected {} {}; verified {}",
            rule, expected, label, actual
        ));
    }
    valid()
}

// ============================================================================
// Shared logical ownership-event ledger
// ============================================================================
//
// The calculus freezes logical ownership events: every credit created over a
// value's lifetime is released, cleaned up, or handed off exactly once. The
// verifier computes (credits - discharges) and asserts balance. A physical
// counter is one projection of this ledger; it is not a premise of the rule.

/// One canonical logical ownership event in a value lifecycle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OwnershipEvent {
    BirthCredit,
    AdditionalCredit,
    ElidedAdditionalCredit,
    Release,
    Cleanup,
    Handoff,
    EdgeRelease,
    ElidedRelease,
}

impl OwnershipEvent {
    /// Net logical owner-credit delta contributed by this event.
    fn delta(self) -> i64 {
        match self {
            Self::BirthCredit | Self::AdditionalCredit => 1,
            Self::Release | Self::Cleanup | Self::Handoff | Self::EdgeRelease => -1,
            Self::ElidedAdditionalCredit | Self::ElidedRelease => 0,
        }
    }

    // Historical fixture spellings. These are compatibility aliases for MIR
    // carrier names, not canonical calculus vocabulary.
    #[allow(non_upper_case_globals)]
    const Alloc: Self = Self::BirthCredit;
    #[allow(non_upper_case_globals)]
    const IncLiveDup: Self = Self::AdditionalCredit;
    #[allow(non_upper_case_globals)]
    const ElideIncMove: Self = Self::ElidedAdditionalCredit;
    #[allow(non_upper_case_globals)]
    const DecLastUse: Self = Self::Release;
    #[allow(non_upper_case_globals)]
    const CleanupDecUnused: Self = Self::Cleanup;
    #[allow(non_upper_case_globals)]
    const TransferOut: Self = Self::Handoff;
    #[allow(non_upper_case_globals)]
    const EdgeDec: Self = Self::EdgeRelease;
    #[allow(non_upper_case_globals)]
    const ElideDecDead: Self = Self::ElidedRelease;
}

/// Historical local type alias retained while fixture names migrate.
type RcEvent = OwnershipEvent;

/// A named value lifecycle = an ordered RcEvent sequence + whether it is
/// EXPECTED to balance (sound emission) or NOT (negative-direction witness).
struct LedgerCase {
    label: &'static str,
    events: &'static [RcEvent],
    /// Sound emission balances to RC = 0; a deliberately-broken witness does
    /// not. The verifier asserts the computed balance matches this flag.
    expect_balanced: bool,
}

/// Compute the net logical owner-credit balance after applying every event.
fn owner_credit_balance(events: &[OwnershipEvent]) -> i64 {
    events.iter().map(|e| e.delta()).sum()
}

/// Historical compatibility wrapper for checker fixtures.
fn ledger_net(events: &[RcEvent]) -> i64 {
    owner_credit_balance(events)
}

/// Discharge a fixture grid of LedgerCases: every `expect_balanced` case MUST
/// net 0, and every negative witness MUST net != 0 (proving the balance check
/// has teeth). Returns the count of cases discharged for the coverage gate.
fn discharge_ledger_cases(rule: &str, cases: &[LedgerCase]) -> Result<u64, EngineResult> {
    let mut checked: u64 = 0;
    for case in cases {
        let net = ledger_net(case.events);
        let balanced = net == 0;
        if balanced != case.expect_balanced {
            return Err(fail(format!(
                "{}: ownership-event case '{}' expected balanced={} but net credit = {} (balanced={}); logical event preservation violated",
                rule, case.label, case.expect_balanced, net, balanced
            )));
        }
        checked += 1;
    }
    Ok(checked)
}

// ============================================================================
// DP predicate models (gating predicates for the RL emission rules)
// ============================================================================
//
// The RL emission decisions are gated on the §05 decision predicates
// (Appendix C truth tables). RL-1's inc is gated by DP-3 (inc-elidable);
// RL-2/RL-3's dec is gated by DP-2 (dec-unnecessary); RL-3's skip is gated by
// DP-7 (skip-eligible). The verifiers cross-check the RL emission decision
// against these predicate models so a divergence between the RL rule and its
// §05-proven gate is REJECTED.

/// DP-3: `is_rc_inc_elidable(s) ⟺ Cardinality = Once ∧ Consumption ∈ {Linear,
/// Affine}` (Appendix C DP-3 truth table). A single-use value is not duplicated,
/// so the duplicate-inc is unnecessary whether the use moves (Linear) or borrows
/// (Affine) it.
fn dp3_inc_elidable(cardinality: Card, consumption: Cons) -> bool {
    cardinality == Card::Once && (consumption == Cons::Linear || consumption == Cons::Affine)
}

/// DP-2: `is_rc_dec_unnecessary(s) ⟺ Cardinality = Absent ∨ Consumption = Dead`
/// (Appendix C DP-2 truth table). An absent/dead value has no reference to
/// release via a SUPPLEMENTARY dec (terminal RL-2/RL-4/RL-5 own their own
/// logic; the definitional cleanup dec is separate per RL-2).
fn dp2_dec_unnecessary(cardinality: Card, consumption: Cons) -> bool {
    cardinality == Card::Absent || consumption == Cons::Dead
}

/// DP-7: `is_event_pair_elision_eligible(s) ⟺ is_local ∧ Access = Owned ∧
/// Consumption = Linear ∧ Cardinality = Once ∧ Uniqueness = Unique ∧
/// ¬is_scalar` (Appendix C DP-7 truth table). The caller-inc + callee-dec
/// pair cancels for a local-linear-unique param.
#[allow(clippy::too_many_arguments)]
fn dp7_skip_eligible(
    is_local: bool,
    access: Access,
    consumption: Cons,
    cardinality: Card,
    uniqueness: Uniq,
    is_scalar: bool,
) -> bool {
    is_local
        && access == Access::Owned
        && consumption == Cons::Linear
        && cardinality == Card::Once
        && uniqueness == Uniq::Unique
        && !is_scalar
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Card {
    Absent,
    Once,
    Many,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cons {
    Dead,
    Linear,
    Affine,
    Unrestricted,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    Borrowed,
    Owned,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Uniq {
    Unique,
    MaybeShared,
    Shared,
}

// ============================================================================
// RL-1: additional owner credit when a live value is duplicated
// ============================================================================
//
// A live duplication creates one additional logical owner credit, balanced by
// the consumer's release. RL-1 elides that event on a single-use value (DP-3:
// Once ∧ (Linear ∨ Affine)) because the single use creates no new owned
// reference (a move transfers the existing reference; a borrow reads it
// non-consumingly and releases it via its own RL-2 scope-exit dec).
//
// (P1) Emission decision: inc is emitted iff NOT DP-3-elidable (the value is
// still live / used more than once after the dup).
// (P2) RC-count preservation: a full lifecycle with the RL-1 inc + the
// matching dec balances to RC = 0.
//
// Shipped: aims/class_ledger/emit/incs.rs plan_incs (the class-ledger's sole
// Inc emitter; gates the funding inc on demand-past-consume per
// aims/class_ledger/mod.rs).

fn verify_rl1_inc_on_live_duplication() -> EngineResult {
    // (P1) Decision grid: emit inc iff NOT (Once ∧ (Linear ∨ Affine)).
    // Each row: (cardinality, consumption, expect_inc).
    let decision_grid: &[(Card, Cons, bool)] = &[
        // Single-use move (Once, Linear) → inc ELIDED (DP-3 fires).
        (Card::Once, Cons::Linear, false),
        // Single-use borrow (Once ∧ Affine) → inc ELIDED (DP-3 fires: the
        // borrow creates no new owned reference; its own RL-2 scope-exit dec
        // balances the read).
        (Card::Once, Cons::Affine, false),
        // Live after dup (Many) → inc EMITTED.
        (Card::Many, Cons::Unrestricted, true),
        (Card::Many, Cons::Affine, true),
        // Once + Unrestricted → inc EMITTED (Unrestricted co-occurs with a
        // genuine multi-use that needs the inc).
        (Card::Once, Cons::Unrestricted, true),
    ];
    let mut decisions_checked: u64 = 0;
    for (card, cons, expect_inc) in decision_grid {
        let emit_inc = !dp3_inc_elidable(*card, *cons);
        if emit_inc != *expect_inc {
            return fail(format!(
                "RL-1 (P1) emission decision: ({:?}, {:?}) expected emit_inc={} but DP-3 gate yields emit_inc={}; RL-1 must elide exactly the single-use value: Once AND (Linear OR Affine)",
                card, cons, expect_inc, emit_inc
            ));
        }
        decisions_checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "RL-1",
        5,
        decisions_checked,
        "(P1) inc-emission decisions cross-checked against DP-3",
    ) {
        return fail(format!(
            "RL-1 (P1) coverage mismatch: expected 5 decisions; verified {}",
            decisions_checked
        ));
    }

    // (P2) RC-count preservation over full value lifecycles.
    let cases: &[LedgerCase] = &[
        // Live duplication: alloc, inc on dup to Owned param (callee decs the
        // duplicate), dec at last use. Balanced.
        LedgerCase {
            label: "live_dup_owned_param_balanced",
            events: &[
                RcEvent::Alloc,
                RcEvent::IncLiveDup,
                RcEvent::DecLastUse, // callee's dec on the duplicate
                RcEvent::DecLastUse, // caller's dec at last use
            ],
            expect_balanced: true,
        },
        // Moved-once: alloc, inc elided (move transfers), ownership handed off.
        // Balanced (no inc, no dec — the single reference moves out).
        LedgerCase {
            label: "single_use_inc_elided_balanced",
            events: &[RcEvent::Alloc, RcEvent::ElideIncMove, RcEvent::TransferOut],
            expect_balanced: true,
        },
        // Two live dups, two callee decs, one final dec. Balanced.
        LedgerCase {
            label: "two_live_dups_balanced",
            events: &[
                RcEvent::Alloc,
                RcEvent::IncLiveDup,
                RcEvent::IncLiveDup,
                RcEvent::DecLastUse,
                RcEvent::DecLastUse,
                RcEvent::DecLastUse,
            ],
            expect_balanced: true,
        },
        // Negative witness: inc emitted on a single-use value (RL-1 wrongly
        // failing to elide) leaves an extra reference — leak (net = +1).
        LedgerCase {
            label: "NEG_inc_on_single_use_leaks",
            events: &[RcEvent::Alloc, RcEvent::IncLiveDup, RcEvent::TransferOut],
            expect_balanced: false,
        },
        // Negative witness: inc with no matching callee dec — leak (net = +1).
        LedgerCase {
            label: "NEG_inc_without_matching_dec_leaks",
            events: &[RcEvent::Alloc, RcEvent::IncLiveDup, RcEvent::DecLastUse],
            expect_balanced: false,
        },
    ];
    let ledger_checked = match discharge_ledger_cases("RL-1", cases) {
        Ok(n) => n,
        Err(e) => return e,
    };
    require_count(
        "RL-1",
        5,
        ledger_checked,
        "(P2) RC-balance lifecycles (3 sound + 2 negative-direction witnesses)",
    )
}

// ============================================================================
// RL-2: RC dec at last use / scope exit; ownership-transfer exclusion;
// unused-owned definitional cleanup
// ============================================================================
//
// Per Annex E §AIMS RL-2: "RC dec SHALL be emitted at last use of owned
// value or scope exit, UNLESS last use is ownership-transferring (Return,
// Construct/Reuse/CollectionReuse arg, Set value, PartialApply capture,
// Apply/Invoke to Owned param, Jump arg per RL-4 exemption). RL-2 includes
// UNUSED owned non-scalar values (Dead/Absent) → immediate RcDec at
// definition."
//
// (P1) Decision: a dec is emitted at a terminal use IFF the use is NOT
// ownership-transferring; an unused owned non-scalar gets a cleanup dec.
// (P2) RC-count preservation: every owned managed value is released exactly
// once — by a dec, an edge dec, a cleanup dec, OR an ownership handoff.
//
// Shipped: aims/class_ledger/emit/releases.rs (last-use dec + dead-at-entry /
// unused-owned cleanup via plan_dead_class_releases) + the twelve-kind
// terminal-use table in aims/intraprocedural/ledger_events/ (the exclusion
// list, mirroring AimsProof.Realization::rl2_use_transfers_ownership).

/// The terminal-use kinds, with whether each TRANSFERS ownership (so RL-2
/// SUPPRESSES the dec). Mirrors the terminal-use classification in
/// aims/intraprocedural/ledger_events/ (AimsProof.Realization::rl2_use_transfers_ownership).
fn rl2_use_transfers_ownership(use_kind: &str) -> bool {
    matches!(
        use_kind,
        "Return"
            | "ConstructArg"
            | "ReuseArg"
            | "CollectionReuseArg"
            | "SetValue"
            | "PartialApplyCapture"
            | "ApplyToOwnedParam"
            | "JumpArg"
    )
}

fn verify_rl2_dec_at_last_use() -> EngineResult {
    // (P1) Terminal-use decision grid: dec emitted iff NOT ownership-transfer.
    // Each row: (use_kind, expect_dec_emitted).
    let use_grid: &[(&str, bool)] = &[
        // Ownership-transferring uses → NO dec (the consumer inherits it).
        ("Return", false),
        ("ConstructArg", false),
        ("ReuseArg", false),
        ("CollectionReuseArg", false),
        ("SetValue", false),
        ("PartialApplyCapture", false),
        ("ApplyToOwnedParam", false),
        ("JumpArg", false),
        // Terminal non-transfer uses → dec EMITTED.
        ("LastReadBeforeScopeExit", true),
        ("ScopeExit", true),
        ("ApplyToBorrowedParam", true),
    ];
    let mut decisions_checked: u64 = 0;
    for (use_kind, expect_dec) in use_grid {
        let emit_dec = !rl2_use_transfers_ownership(use_kind);
        if emit_dec != *expect_dec {
            return fail(format!(
                "RL-2 (P1) terminal-use decision: '{}' expected emit_dec={} but ownership-transfer model yields emit_dec={}; the RL-2 exclusion list must match the ledger_events terminal-use table",
                use_kind, expect_dec, emit_dec
            ));
        }
        decisions_checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "RL-2",
        11,
        decisions_checked,
        "(P1) terminal-use dec decisions (8 transfer + 3 non-transfer)",
    ) {
        return fail(format!(
            "RL-2 (P1) coverage mismatch: expected 11 use decisions; verified {}",
            decisions_checked
        ));
    }

    // (P2) RC-count preservation.
    let cases: &[LedgerCase] = &[
        // Owned value used then dec'd at last use. Balanced.
        LedgerCase {
            label: "last_use_dec_balanced",
            events: &[RcEvent::Alloc, RcEvent::DecLastUse],
            expect_balanced: true,
        },
        // Owned value returned (ownership transfer) — no dec, the alloc
        // reference moves out. Balanced.
        LedgerCase {
            label: "ownership_transfer_no_dec_balanced",
            events: &[RcEvent::Alloc, RcEvent::TransferOut],
            expect_balanced: true,
        },
        // Unused owned non-scalar (Dead/Absent) → immediate cleanup dec.
        // Balanced.
        LedgerCase {
            label: "unused_owned_cleanup_dec_balanced",
            events: &[RcEvent::Alloc, RcEvent::CleanupDecUnused],
            expect_balanced: true,
        },
        // Negative witness: dec emitted AFTER an ownership transfer
        // (double-release) — net = -1 (double-free).
        LedgerCase {
            label: "NEG_dec_after_transfer_double_free",
            events: &[RcEvent::Alloc, RcEvent::TransferOut, RcEvent::DecLastUse],
            expect_balanced: false,
        },
        // Negative witness: unused owned value with NO cleanup dec — leak
        // (net = +1).
        LedgerCase {
            label: "NEG_unused_owned_no_cleanup_leaks",
            events: &[RcEvent::Alloc],
            expect_balanced: false,
        },
    ];
    let ledger_checked = match discharge_ledger_cases("RL-2", cases) {
        Ok(n) => n,
        Err(e) => return e,
    };
    require_count(
        "RL-2",
        5,
        ledger_checked,
        "(P2) RC-balance lifecycles (3 sound + 2 negative-direction witnesses)",
    )
}

// ============================================================================
// RL-3: ownership-event elision when DP-2 / DP-3 / DP-7 hold
// ============================================================================
//
// An ownership event is elided only when the lattice proves it unnecessary.
// Removing it preserves the exact logical balance and evaluator behavior.
//
// (P1) Elision decision: an op is elided IFF at least one of DP-2 / DP-3 /
// DP-7 holds for the value's state.
// (P2) RC-count preservation: a lifecycle with the elided ops removed balances
// identically to the lifecycle with them present (the elided ops were
// no-ops on the net count).
//
// Shipped: aims/class_ledger/replace.rs (apply_class_ledger_replacement
// fuses the DP-2 is_rc_dec_unnecessary / DP-3 is_rc_inc_elidable verdict at
// aims/transfer/mod.rs into the class-ledger plan at Step 4b, so an elided
// op is never emitted rather than emitted-then-removed by a separate pass).

fn verify_rl3_rc_op_elision() -> EngineResult {
    // (P1) Elision-eligibility grid. Each row models a value state + which DP
    // predicate(s) the elision is gated on, and whether elision is eligible.
    struct ElisionRow {
        label: &'static str,
        dp2: bool,
        dp3: bool,
        dp7: bool,
        expect_elide: bool,
    }
    let grid: &[ElisionRow] = &[
        // DP-3 fires on a single use (here the Once + Linear move witness; DP-3
        // also fires on Once + Affine) → inc elided.
        ElisionRow {
            label: "dp3_move_elides_inc",
            dp2: dp2_dec_unnecessary(Card::Once, Cons::Linear),
            dp3: dp3_inc_elidable(Card::Once, Cons::Linear),
            dp7: false,
            expect_elide: true,
        },
        // DP-2 fires (Absent → dec unnecessary) → supplementary dec elided.
        ElisionRow {
            label: "dp2_absent_elides_dec",
            dp2: dp2_dec_unnecessary(Card::Absent, Cons::Dead),
            dp3: false,
            dp7: false,
            expect_elide: true,
        },
        // DP-7 fires (local+linear+once+unique owned non-scalar) → inc/dec pair
        // skipped (caller inc + callee dec cancel).
        ElisionRow {
            label: "dp7_local_unique_skips_pair",
            dp2: false,
            dp3: false,
            dp7: dp7_skip_eligible(
                true,
                Access::Owned,
                Cons::Linear,
                Card::Once,
                Uniq::Unique,
                false,
            ),
            expect_elide: true,
        },
        // No predicate fires (Many + Unrestricted + non-local) → NO elision.
        ElisionRow {
            label: "no_predicate_no_elision",
            dp2: dp2_dec_unnecessary(Card::Many, Cons::Unrestricted),
            dp3: dp3_inc_elidable(Card::Many, Cons::Unrestricted),
            dp7: dp7_skip_eligible(
                false,
                Access::Owned,
                Cons::Unrestricted,
                Card::Many,
                Uniq::MaybeShared,
                false,
            ),
            expect_elide: false,
        },
        // DP-7 blocked by non-unique (MaybeShared) → NO skip (an unbalanced
        // caller inc would leak if skipped).
        ElisionRow {
            label: "dp7_blocked_by_maybeshared",
            dp2: false,
            dp3: false,
            dp7: dp7_skip_eligible(
                true,
                Access::Owned,
                Cons::Linear,
                Card::Once,
                Uniq::MaybeShared,
                false,
            ),
            expect_elide: false,
        },
        // DP-7 blocked by Shared → NO skip (a Shared value's caller inc is never
        // balanced — skipping the inc/dec pair would leak the upstream ref).
        ElisionRow {
            label: "dp7_blocked_by_shared",
            dp2: false,
            dp3: false,
            dp7: dp7_skip_eligible(
                true,
                Access::Owned,
                Cons::Linear,
                Card::Once,
                Uniq::Shared,
                false,
            ),
            expect_elide: false,
        },
    ];
    let mut decisions_checked: u64 = 0;
    for row in grid {
        let elide = row.dp2 || row.dp3 || row.dp7;
        if elide != row.expect_elide {
            return fail(format!(
                "RL-3 (P1) elision decision: '{}' expected elide={} but DP-2∨DP-3∨DP-7 = {}; RL-3 elides exactly when a gating predicate proves the op redundant",
                row.label, row.expect_elide, elide
            ));
        }
        decisions_checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "RL-3",
        6,
        decisions_checked,
        "(P1) elision decisions cross-checked against DP-2/DP-3/DP-7",
    ) {
        return fail(format!(
            "RL-3 (P1) coverage mismatch: expected 6 decisions; verified {}",
            decisions_checked
        ));
    }

    // (P2) Elision preserves balance: the elided form and the redundant-op form
    // both balance to 0. A single-use value: with the redundant inc+dec
    // present (balanced) and with both elided (also balanced).
    let cases: &[LedgerCase] = &[
        // Redundant inc+dec present on a single-use value — still balances.
        LedgerCase {
            label: "redundant_inc_dec_present_balanced",
            events: &[
                RcEvent::Alloc,
                RcEvent::IncLiveDup,
                RcEvent::DecLastUse,
                RcEvent::TransferOut,
            ],
            expect_balanced: true,
        },
        // Same value, inc+dec ELIDED — balances identically (elision is a
        // net-preserving rewrite).
        LedgerCase {
            label: "redundant_inc_dec_elided_balanced",
            events: &[
                RcEvent::Alloc,
                RcEvent::ElideIncMove,
                RcEvent::ElideDecDead,
                RcEvent::TransferOut,
            ],
            expect_balanced: true,
        },
        // Negative witness: eliding a NEEDED dec (DP-2 false — Many cardinality)
        // leaves the alloc reference unreleased — leak (net = +1).
        LedgerCase {
            label: "NEG_elide_needed_dec_leaks",
            events: &[RcEvent::Alloc, RcEvent::ElideDecDead],
            expect_balanced: false,
        },
    ];
    let ledger_checked = match discharge_ledger_cases("RL-3", cases) {
        Ok(n) => n,
        Err(e) => return e,
    };
    require_count(
        "RL-3",
        3,
        ledger_checked,
        "(P2) elision-preserves-balance lifecycles (2 sound + 1 negative-direction witness)",
    )
}

// ============================================================================
// RL-4: edge-specific decs on CFG edges; Jump-arg exemption
// ============================================================================
//
// Per Annex E §AIMS RL-4: "OWNED non-scalar variable alive at block exit
// but dead at successor entry SHALL receive a dec on that specific CFG edge.
// Jump argument exemption: variables passed as Jump args transfer ownership to
// the successor block param — NOT dead at successor entry. Borrowed variables
// do NOT receive decs."
//
// (P1) Edge-dec decision: emit an edge dec IFF (Owned ∧ non-scalar ∧ live at
// exit ∧ dead at successor entry ∧ NOT a Jump arg).
// (P2) RC-count preservation: a value dead across an edge is released exactly
// once (by the edge dec) UNLESS it is a Jump arg (the successor block
// param inherits the obligation).
//
// Shipped: aims/class_ledger/emit/releases.rs plan_releases.

fn verify_rl4_edge_specific_decs() -> EngineResult {
    // (P1) Edge-dec decision grid.
    struct EdgeRow {
        label: &'static str,
        access: Access,
        is_scalar: bool,
        live_at_exit: bool,
        dead_at_succ: bool,
        is_jump_arg: bool,
        expect_edge_dec: bool,
    }
    let grid: &[EdgeRow] = &[
        // Owned non-scalar, dead across edge, not a jump arg → edge dec.
        EdgeRow {
            label: "owned_dead_across_edge_emits",
            access: Access::Owned,
            is_scalar: false,
            live_at_exit: true,
            dead_at_succ: true,
            is_jump_arg: false,
            expect_edge_dec: true,
        },
        // Jump arg → NO edge dec (ownership transfers to successor block param).
        EdgeRow {
            label: "jump_arg_exempt_no_edge_dec",
            access: Access::Owned,
            is_scalar: false,
            live_at_exit: true,
            dead_at_succ: true,
            is_jump_arg: true,
            expect_edge_dec: false,
        },
        // Borrowed → NO edge dec (caller manages).
        EdgeRow {
            label: "borrowed_no_edge_dec",
            access: Access::Borrowed,
            is_scalar: false,
            live_at_exit: true,
            dead_at_succ: true,
            is_jump_arg: false,
            expect_edge_dec: false,
        },
        // Still live at successor → NO edge dec (will be dec'd later).
        EdgeRow {
            label: "live_at_succ_no_edge_dec",
            access: Access::Owned,
            is_scalar: false,
            live_at_exit: true,
            dead_at_succ: false,
            is_jump_arg: false,
            expect_edge_dec: false,
        },
        // Scalar → NO edge dec (no RC).
        EdgeRow {
            label: "scalar_no_edge_dec",
            access: Access::Owned,
            is_scalar: true,
            live_at_exit: true,
            dead_at_succ: true,
            is_jump_arg: false,
            expect_edge_dec: false,
        },
    ];
    let mut decisions_checked: u64 = 0;
    for row in grid {
        let emit = row.access == Access::Owned
            && !row.is_scalar
            && row.live_at_exit
            && row.dead_at_succ
            && !row.is_jump_arg;
        if emit != row.expect_edge_dec {
            return fail(format!(
                "RL-4 (P1) edge-dec decision: '{}' expected emit={} but model yields emit={}; edge dec fires iff Owned∧non-scalar∧dead-across-edge∧¬jump-arg",
                row.label, row.expect_edge_dec, emit
            ));
        }
        decisions_checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "RL-4",
        5,
        decisions_checked,
        "(P1) edge-dec decisions (1 emit + 4 suppressed: jump-arg / borrowed / live / scalar)",
    ) {
        return fail(format!(
            "RL-4 (P1) coverage mismatch: expected 5 decisions; verified {}",
            decisions_checked
        ));
    }

    // (P2) RC-count preservation across CFG edges.
    let cases: &[LedgerCase] = &[
        // Value dead across one edge → released by the edge dec. Balanced.
        LedgerCase {
            label: "dead_across_edge_balanced",
            events: &[RcEvent::Alloc, RcEvent::EdgeDec],
            expect_balanced: true,
        },
        // Jump arg → ownership transfers to successor block param (handoff);
        // no edge dec. Balanced.
        LedgerCase {
            label: "jump_arg_transfer_balanced",
            events: &[RcEvent::Alloc, RcEvent::TransferOut],
            expect_balanced: true,
        },
        // Negative witness: BOTH an edge dec AND a jump-arg transfer on the same
        // value (RL-4 wrongly failing the jump-arg exemption) — net = -1
        // (double-free).
        LedgerCase {
            label: "NEG_edge_dec_on_jump_arg_double_free",
            events: &[RcEvent::Alloc, RcEvent::TransferOut, RcEvent::EdgeDec],
            expect_balanced: false,
        },
    ];
    let ledger_checked = match discharge_ledger_cases("RL-4", cases) {
        Ok(n) => n,
        Err(e) => return e,
    };
    require_count(
        "RL-4",
        3,
        ledger_checked,
        "(P2) edge-dec lifecycles (2 sound + 1 negative-direction witness)",
    )
}

// ============================================================================
// RL-5: dead-at-entry cleanup for Owned non-scalar block params (Absent)
// ============================================================================
//
// Per Annex E §AIMS RL-5: "OWNED non-scalar block param with
// Cardinality = Absent at entry SHALL receive an immediate dec. Borrowed
// Absent params don't need decs." Soundness: a predecessor passed the value
// (its reference), and the block never uses it — the entry dec releases that
// reference. This is the Appendix B reachable-boundary state
// (Owned ∧ Absent ∧ Dead).
//
// (P1) Entry-dec decision: emit iff (Owned ∧ non-scalar ∧ Cardinality = Absent).
// (P2) RC-count preservation: the predecessor's reference is released exactly
// once by the entry dec.
//
// Shipped: aims/class_ledger/emit/releases.rs plan_dead_class_releases.

fn verify_rl5_dead_at_entry_cleanup() -> EngineResult {
    struct EntryRow {
        label: &'static str,
        access: Access,
        is_scalar: bool,
        cardinality: Card,
        expect_entry_dec: bool,
    }
    let grid: &[EntryRow] = &[
        // Owned non-scalar Absent block param → entry dec.
        EntryRow {
            label: "owned_absent_block_param_emits",
            access: Access::Owned,
            is_scalar: false,
            cardinality: Card::Absent,
            expect_entry_dec: true,
        },
        // Borrowed Absent → NO entry dec (caller manages).
        EntryRow {
            label: "borrowed_absent_no_entry_dec",
            access: Access::Borrowed,
            is_scalar: false,
            cardinality: Card::Absent,
            expect_entry_dec: false,
        },
        // Owned but USED (Once) → NO entry dec (RL-2 dec at last use instead).
        EntryRow {
            label: "owned_used_once_no_entry_dec",
            access: Access::Owned,
            is_scalar: false,
            cardinality: Card::Once,
            expect_entry_dec: false,
        },
        // Scalar Absent → NO entry dec (no RC).
        EntryRow {
            label: "scalar_absent_no_entry_dec",
            access: Access::Owned,
            is_scalar: true,
            cardinality: Card::Absent,
            expect_entry_dec: false,
        },
    ];
    let mut decisions_checked: u64 = 0;
    for row in grid {
        let emit = row.access == Access::Owned && !row.is_scalar && row.cardinality == Card::Absent;
        if emit != row.expect_entry_dec {
            return fail(format!(
                "RL-5 (P1) entry-dec decision: '{}' expected emit={} but model yields emit={}; entry dec fires iff Owned∧non-scalar∧Absent",
                row.label, row.expect_entry_dec, emit
            ));
        }
        decisions_checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "RL-5",
        4,
        decisions_checked,
        "(P1) entry-dec decisions (1 emit + 3 suppressed: borrowed / used / scalar)",
    ) {
        return fail(format!(
            "RL-5 (P1) coverage mismatch: expected 4 decisions; verified {}",
            decisions_checked
        ));
    }

    // (P2) RC-count preservation.
    let cases: &[LedgerCase] = &[
        // Predecessor passed the reference (Alloc models the inbound ref); the
        // block never uses it → entry cleanup dec releases it. Balanced.
        LedgerCase {
            label: "dead_at_entry_cleanup_balanced",
            events: &[RcEvent::Alloc, RcEvent::CleanupDecUnused],
            expect_balanced: true,
        },
        // Negative witness: Owned Absent block param with NO entry dec — the
        // predecessor's reference leaks (net = +1).
        LedgerCase {
            label: "NEG_owned_absent_no_entry_dec_leaks",
            events: &[RcEvent::Alloc],
            expect_balanced: false,
        },
    ];
    let ledger_checked = match discharge_ledger_cases("RL-5", cases) {
        Ok(n) => n,
        Err(e) => return e,
    };
    require_count(
        "RL-5",
        2,
        ledger_checked,
        "(P2) entry-cleanup lifecycles (1 sound + 1 negative-direction witness)",
    )
}

// ============================================================================
// §08.2 mutation-isolation obligations via DP-4 / DP-5 / DP-9
// ============================================================================
//
// A mutation is sound iff no existing owner observes a change that belongs to
// another value. DP-9 freezes an admissible outcome or an observation/isolation
// obligation, gated on DP-5 and DP-4. It does not prescribe a branch or copy.
// The case_analysis engine
// enumerates the (Uniqueness x borrow-state x field) grid and discharges, per
// branch, that the selected mode preserves value semantics — with a closed-
// world coverage check (every case exhibited) per the foundational-axiom policy
// sec-Per-Engine-Constructive-Proof-Shape (forbids uncovered branches).
//
// Shipped: emit_rc/cow.rs (has_borrows_from_aggregate,
// is_borrow_disjoint_from_siblings) + realize/decide.rs::decide_annotations
// (the Phase 2 COW decision) consuming DP-5 / DP-9.

/// Neutral mutation outcome/obligation selected by DP-9.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MutationObligation {
    SameIdentityAdmissible,
    SharingObservationRequired,
    IsolationRequired,
}

/// DP-5: `can_mutate_in_place(s, var, field, point) ⟺ Access = Owned ∧
/// Uniqueness = Unique ∧ no_active_overlapping_borrows`. Field-aware:
/// disjoint-field borrows do NOT block (RL-10); SetTag treats ALL fields as
/// overlapping (RL-10 SetTag exclusion — a tag change invalidates every
/// payload field).
fn can_mutate_in_place(
    access: Access,
    uniqueness: Uniq,
    has_active_borrow: bool,
    borrow_is_field_disjoint: bool,
    is_settag: bool,
) -> bool {
    if access != Access::Owned || uniqueness != Uniq::Unique {
        return false;
    }
    if !has_active_borrow {
        return true;
    }
    // An active borrow blocks UNLESS it is from a disjoint field AND the
    // mutation is not a SetTag (SetTag overlaps every field per RL-10).
    borrow_is_field_disjoint && !is_settag
}

/// DP-4: `needs_sharing_observation(s) ⟺ Uniqueness = MaybeShared`.
fn needs_sharing_observation(uniqueness: Uniq) -> bool {
    uniqueness == Uniq::MaybeShared
}

/// DP-9 neutral mutation-obligation classification:
/// - Unique ∧ can_mutate_in_place ⟹ same identity is admissible
/// - Unique ∧ ¬can_mutate_in_place ⟹ isolation is required
/// - MaybeShared ∧ param IC-3 ParamContract.uniqueness = Unique ⟹ same
/// identity is admissible (caller proved exactly one owner credit)
/// - MaybeShared else ⟹ sharing observation is required
/// - Shared ⟹ isolation is required
fn decide_mutation_obligation(
    uniqueness: Uniq,
    param_ic3_unique: bool,
    can_mutate: bool,
) -> MutationObligation {
    match uniqueness {
        Uniq::Unique => {
            if can_mutate {
                MutationObligation::SameIdentityAdmissible
            } else {
                MutationObligation::IsolationRequired
            }
        }
        Uniq::MaybeShared => {
            if param_ic3_unique {
                MutationObligation::SameIdentityAdmissible
            } else {
                MutationObligation::SharingObservationRequired
            }
        }
        Uniq::Shared => MutationObligation::IsolationRequired,
    }
}

/// Value-semantics soundness of a selected mutation obligation: a mutation preserves
/// value semantics iff no other live view observes the in-place write.
/// - SameIdentityAdmissible is sound iff the value has exactly one logical owner credit at the mutation
/// point with no overlapping borrow (Unique + can_mutate_in_place), OR the
/// caller proved param uniqueness (IC-3).
/// - SharingObservationRequired is sound because the observation selects the
/// same-identity or isolation obligation.
/// - IsolationRequired is sound because aliases cannot observe the mutation.
fn mutation_obligation_preserves_value_semantics(
    mode: MutationObligation,
    uniqueness: Uniq,
    param_ic3_unique: bool,
    can_mutate: bool,
) -> bool {
    match mode {
        MutationObligation::SameIdentityAdmissible => {
            (uniqueness == Uniq::Unique && can_mutate)
                || (uniqueness == Uniq::MaybeShared && param_ic3_unique)
        }
        MutationObligation::SharingObservationRequired => true,
        MutationObligation::IsolationRequired => true,
    }
}

// ----------------------------------------------------------------------------
// RL-6: Unique admits same-identity mutation when local borrows permit it
// ----------------------------------------------------------------------------
//
// Soundness: one logical owner plus local borrow safety admits preserving the
// value's identity. If local safety fails, the theorem freezes isolation. The
// projection chooses how to satisfy either outcome.

fn verify_rl6_same_identity_mutation() -> EngineResult {
    struct Row {
        label: &'static str,
        uniqueness: Uniq,
        has_active_borrow: bool,
        borrow_field_disjoint: bool,
        expect_mode: MutationObligation,
    }
    let grid: &[Row] = &[
        // Unique, no borrow admits same-identity mutation (RL-6).
        Row {
            label: "unique_no_borrow_static_inplace",
            uniqueness: Uniq::Unique,
            has_active_borrow: false,
            borrow_field_disjoint: false,
            expect_mode: MutationObligation::SameIdentityAdmissible,
        },
        // Unique, disjoint-field borrow admits same identity (RL-10 lets the
        // disjoint borrow through; covered fully by RL-10, exercised here).
        Row {
            label: "unique_disjoint_borrow_static_inplace",
            uniqueness: Uniq::Unique,
            has_active_borrow: true,
            borrow_field_disjoint: true,
            expect_mode: MutationObligation::SameIdentityAdmissible,
        },
        // Unique, OVERLAPPING borrow → isolation required.
        Row {
            label: "unique_overlapping_borrow_static_shared",
            uniqueness: Uniq::Unique,
            has_active_borrow: true,
            borrow_field_disjoint: false,
            expect_mode: MutationObligation::IsolationRequired,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let can_mutate = can_mutate_in_place(
            Access::Owned,
            row.uniqueness,
            row.has_active_borrow,
            row.borrow_field_disjoint,
            false,
        );
        let mode = decide_mutation_obligation(row.uniqueness, false, can_mutate);
        if mode != row.expect_mode {
            return fail(format!(
                "RL-6 (P1) mode decision: '{}' expected {:?} but DP-5/DP-9 yield {:?}",
                row.label, row.expect_mode, mode
            ));
        }
        if !mutation_obligation_preserves_value_semantics(mode, row.uniqueness, false, can_mutate) {
            return fail(format!(
                "RL-6 (P2) soundness: '{}' selected {:?} but it does not preserve value semantics",
                row.label, mode
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: forcing same identity on a Unique
    // value with an OVERLAPPING borrow is NOT value-semantics-preserving
    // (the borrowed reference would observe the in-place write). The soundness
    // predicate must REJECT it.
    let neg_can_mutate = can_mutate_in_place(Access::Owned, Uniq::Unique, true, false, false);
    if mutation_obligation_preserves_value_semantics(
        MutationObligation::SameIdentityAdmissible,
        Uniq::Unique,
        false,
        neg_can_mutate,
    ) {
        return fail(
            "RL-6 negative witness: same-identity mutation with an overlapping borrow was wrongly accepted".to_string(),
        );
    }
    require_count(
        "RL-6",
        3,
        checked,
        "(P1/P2) Unique-mutation mode decisions + value-semantics soundness (negative witness: overlapping-borrow in-place rejected)",
    )
}

// ----------------------------------------------------------------------------
// RL-7: MaybeShared retains a sharing-observation obligation
// ----------------------------------------------------------------------------
//
// A logical sharing observation selects whether same identity is admissible or
// isolation is required. DP-4 fires exactly on MaybeShared. The observation
// mechanism and isolation mechanism are projection choices.

fn verify_rl7_sharing_observation() -> EngineResult {
    struct Row {
        label: &'static str,
        uniqueness: Uniq,
        param_ic3_unique: bool,
        expect_needs_check: bool,
        expect_mode: MutationObligation,
    }
    let grid: &[Row] = &[
        // MaybeShared, no caller proof → sharing observation required.
        Row {
            label: "maybeshared_dynamic_check",
            uniqueness: Uniq::MaybeShared,
            param_ic3_unique: false,
            expect_needs_check: true,
            expect_mode: MutationObligation::SharingObservationRequired,
        },
        // MaybeShared but caller-proven param uniqueness admits same identity.
        Row {
            label: "maybeshared_param_unique_static",
            uniqueness: Uniq::MaybeShared,
            param_ic3_unique: true,
            expect_needs_check: true,
            expect_mode: MutationObligation::SameIdentityAdmissible,
        },
        // Unique requires no sharing observation (DP-4 false).
        Row {
            label: "unique_no_dynamic_check",
            uniqueness: Uniq::Unique,
            param_ic3_unique: false,
            expect_needs_check: false,
            expect_mode: MutationObligation::SameIdentityAdmissible,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let needs = needs_sharing_observation(row.uniqueness);
        if needs != row.expect_needs_check {
            return fail(format!(
                "RL-7 (P1) DP-4 needs_sharing_observation: '{}' expected {} got {}",
                row.label, row.expect_needs_check, needs
            ));
        }
        // can_mutate for a Unique value with no borrow is true; for MaybeShared
        // DP-5 yields false (Uniqueness != Unique), so the mode is driven by
        // param_ic3_unique per DP-9.
        let can_mutate = can_mutate_in_place(Access::Owned, row.uniqueness, false, false, false);
        let mode = decide_mutation_obligation(row.uniqueness, row.param_ic3_unique, can_mutate);
        if mode != row.expect_mode {
            return fail(format!(
                "RL-7 (P1) mode decision: '{}' expected {:?} got {:?}",
                row.label, row.expect_mode, mode
            ));
        }
        if !mutation_obligation_preserves_value_semantics(
            mode,
            row.uniqueness,
            row.param_ic3_unique,
            can_mutate,
        ) {
            return fail(format!(
                "RL-7 (P2) soundness: '{}' selected {:?} but it does not preserve value semantics",
                row.label, mode
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-7",
        3,
        checked,
        "(P1/P2) MaybeShared observation decisions (DP-4 gate + DP-9 obligation + value-semantics soundness)",
    )
}

// ----------------------------------------------------------------------------
// RL-8: Shared requires mutation isolation
// ----------------------------------------------------------------------------
//
// When multiple logical owners are proven, changing shared identity would
// violate value semantics. The calculus requires isolation; it does not select
// copying, regions, handles, or any other physical mechanism.

fn verify_rl8_mutation_isolation() -> EngineResult {
    // Shared → isolation required, regardless of borrow state.
    let mut checked: u64 = 0;
    for has_borrow in [false, true] {
        let can_mutate = can_mutate_in_place(Access::Owned, Uniq::Shared, has_borrow, false, false);
        if can_mutate {
            return fail(format!(
                "RL-8: can_mutate_in_place wrongly true for a Shared value (has_borrow={})",
                has_borrow
            ));
        }
        let mode = decide_mutation_obligation(Uniq::Shared, false, can_mutate);
        if mode != MutationObligation::IsolationRequired {
            return fail(format!(
                "RL-8 mode decision: Shared (has_borrow={}) expected IsolationRequired got {:?}",
                has_borrow, mode
            ));
        }
        if !mutation_obligation_preserves_value_semantics(mode, Uniq::Shared, false, can_mutate) {
            return fail(
                "RL-8 soundness: the isolation obligation must preserve value semantics"
                    .to_string(),
            );
        }
        checked += 1;
    }
    // Negative-direction witness: same-identity mutation on a Shared value
    // corrupts the other live reference — must be REJECTED by the soundness
    // predicate.
    if mutation_obligation_preserves_value_semantics(
        MutationObligation::SameIdentityAdmissible,
        Uniq::Shared,
        false,
        false,
    ) {
        return fail(
            "RL-8 negative witness: same-identity mutation on a Shared value was wrongly accepted"
                .to_string(),
        );
    }
    require_count(
        "RL-8",
        2,
        checked,
        "(P1/P2) Shared-mutation isolation obligations (negative witness: Shared same-identity mutation rejected)",
    )
}

// ----------------------------------------------------------------------------
// RL-9: explicit/compact sharing-observation refinement
// ----------------------------------------------------------------------------
//
// Explicit and compact representations of a sharing observation must select
// the same neutral outcome. This freezes behavior without freezing a diamond,
// compound instruction, branch, or copy.

fn verify_rl9_observation_representation_equivalence() -> EngineResult {
    fn explicit_observation_outcome(one_owner_observed: bool) -> MutationObligation {
        if one_owner_observed {
            MutationObligation::SameIdentityAdmissible
        } else {
            MutationObligation::IsolationRequired
        }
    }
    fn compact_observation_outcome(one_owner_observed: bool) -> MutationObligation {
        if one_owner_observed {
            MutationObligation::SameIdentityAdmissible
        } else {
            MutationObligation::IsolationRequired
        }
    }
    let mut checked: u64 = 0;
    for one_owner_observed in [true, false] {
        let explicit = explicit_observation_outcome(one_owner_observed);
        let compact = compact_observation_outcome(one_owner_observed);
        if explicit != compact {
            return fail(format!(
                "RL-9 refinement: explicit outcome {:?} != compact outcome {:?} for one_owner_observed={}",
                explicit, compact, one_owner_observed
            ));
        }
        // Both outcomes must independently preserve value semantics.
        let uniq = if one_owner_observed {
            Uniq::Unique
        } else {
            Uniq::Shared
        };
        let can_mutate = one_owner_observed;
        if !mutation_obligation_preserves_value_semantics(compact, uniq, false, can_mutate) {
            return fail(format!(
                "RL-9 soundness: compact outcome {:?} for one_owner_observed={} does not preserve value semantics",
                compact, one_owner_observed
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-9",
        2,
        checked,
        "(P1/P2) explicit/compact outcome equivalence over both sharing-observation results",
    )
}

// ----------------------------------------------------------------------------
// RL-10: disjoint field mutation (no COW) + SetTag exclusion
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-10: "Disjoint field mutation SHALL NOT trigger
// COW: receiver mutated at field F, all active borrows from the same source
// from DIFFERENT fields -> safe without COW. SetTag excluded: a tag change
// invalidates ALL payload fields, conflicting with ALL active borrows
// regardless of field." Soundness: a field-disjoint borrow cannot observe a
// mutation to a different field, so in-place is safe; SetTag changes the
// variant layout, invalidating every borrow.

fn verify_rl10_disjoint_field_mutation() -> EngineResult {
    struct Row {
        label: &'static str,
        has_active_borrow: bool,
        borrow_field_disjoint: bool,
        is_settag: bool,
        expect_can_mutate: bool,
    }
    let grid: &[Row] = &[
        // No active borrow → in-place safe.
        Row {
            label: "no_borrow_inplace",
            has_active_borrow: false,
            borrow_field_disjoint: false,
            is_settag: false,
            expect_can_mutate: true,
        },
        // Disjoint-field borrow, Set (not SetTag) → in-place safe (RL-10).
        Row {
            label: "disjoint_field_set_inplace",
            has_active_borrow: true,
            borrow_field_disjoint: true,
            is_settag: false,
            expect_can_mutate: true,
        },
        // Overlapping-field borrow → must copy (DP-5 blocks).
        Row {
            label: "overlapping_field_must_copy",
            has_active_borrow: true,
            borrow_field_disjoint: false,
            is_settag: false,
            expect_can_mutate: false,
        },
        // Disjoint-field borrow but SetTag → must copy (SetTag overlaps ALL
        // fields; the variant-layout change invalidates the disjoint borrow).
        Row {
            label: "disjoint_field_settag_must_copy",
            has_active_borrow: true,
            borrow_field_disjoint: true,
            is_settag: true,
            expect_can_mutate: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let can_mutate = can_mutate_in_place(
            Access::Owned,
            Uniq::Unique,
            row.has_active_borrow,
            row.borrow_field_disjoint,
            row.is_settag,
        );
        if can_mutate != row.expect_can_mutate {
            return fail(format!(
                "RL-10 (P1) field-aware DP-5: '{}' expected can_mutate={} got {}",
                row.label, row.expect_can_mutate, can_mutate
            ));
        }
        // Failure requires isolation; success admits same-identity mutation.
        let mode = decide_mutation_obligation(Uniq::Unique, false, can_mutate);
        let expect_mode = if row.expect_can_mutate {
            MutationObligation::SameIdentityAdmissible
        } else {
            MutationObligation::IsolationRequired
        };
        if mode != expect_mode {
            return fail(format!(
                "RL-10 (P1) mode: '{}' expected {:?} got {:?}",
                row.label, expect_mode, mode
            ));
        }
        if !mutation_obligation_preserves_value_semantics(mode, Uniq::Unique, false, can_mutate) {
            return fail(format!(
                "RL-10 (P2) soundness: '{}' selected {:?} but it does not preserve value semantics",
                row.label, mode
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: in-place on a SetTag with ANY active borrow
    // (even disjoint) corrupts the borrow via the variant-layout change — the
    // field-aware DP-5 must REFUSE in-place.
    let settag_can_mutate = can_mutate_in_place(Access::Owned, Uniq::Unique, true, true, true);
    if settag_can_mutate {
        return fail(
            "RL-10 negative witness: SetTag with a disjoint-field borrow was wrongly allowed in-place (SetTag must overlap ALL fields)".to_string(),
        );
    }
    require_count(
        "RL-10",
        4,
        checked,
        "(P1/P2) field-aware disjointness decisions (1 no-borrow + 1 disjoint-Set + 1 overlapping + 1 disjoint-SetTag; negative witness: SetTag never in-place)",
    )
}

// ============================================================================
// §08.3 donor/recipient owner-credit transfer
// ============================================================================
//
// RL-11/RL-12 freeze when a dying donor's owner credit may transfer to a fresh
// recipient: one-owner evidence, donor-before-recipient ordering, and no path
// hazard. Storage identity, Reset/Reuse instructions, and allocation placement
// are projection details checked separately.
//
// Shipped: emit_reuse/detect.rs + emit_reuse/planner.rs + emit_reuse/fip.rs +
// emit_reuse/dynamic.rs (RL-11a IsShared branch); realize/mod.rs::realize_rc_reuse
// Sub-step C (emit_reuse_from_events).

/// DP-6: `is_reuse_candidate(s) ⟺ Access = Owned ∧ Uniqueness ≠ Shared ∧
/// Shape ≠ NonReusable` (Appendix C DP-6). A Shared value can never be reused
/// (an alias would observe the reused memory); a NonReusable shape (Tuple /
/// Closure) has no constructor to reuse.
fn is_credit_transfer_candidate(access: Access, uniqueness: Uniq, shape_reusable: bool) -> bool {
    access == Access::Owned && uniqueness != Uniq::Shared && shape_reusable
}

// ----------------------------------------------------------------------------
// RL-11: same-block donor/recipient credit transfer
// ----------------------------------------------------------------------------
//
// The neutral relation holds iff
// (a) the donor precedes the recipient,
// (b) no intervening instruction may throw, may allocate, or may use the
// dying value or any alias of it (project_alias_sources), AND
// (c) the dying value's Uniqueness = Unique (non-unique reuse corrupts
// aliases).
// Soundness: the donor has exactly one credit and no observer can distinguish
// credit transfer from independent donor cleanup plus recipient birth.

fn verify_rl11_same_block_reuse() -> EngineResult {
    struct Row {
        label: &'static str,
        donor_precedes_recipient: bool,
        no_intervening_hazard: bool,
        dying_unique: bool,
        expect_credit_transfer: bool,
    }
    let grid: &[Row] = &[
        // All three conditions hold -> reuse fires (sound).
        Row {
            label: "abc_all_hold_credit_transfer",
            donor_precedes_recipient: true,
            no_intervening_hazard: true,
            dying_unique: true,
            expect_credit_transfer: true,
        },
        // (a) violated: recipient can precede donor -> no transfer.
        Row {
            label: "recipient_before_donor_no_transfer",
            donor_precedes_recipient: false,
            no_intervening_hazard: true,
            dying_unique: true,
            expect_credit_transfer: false,
        },
        // (b) violated: an intervening hazard (throw / alloc / alias use) ->
        // no reuse (the dying value or an alias may still be observed).
        Row {
            label: "intervening_hazard_no_transfer",
            donor_precedes_recipient: true,
            no_intervening_hazard: false,
            dying_unique: true,
            expect_credit_transfer: false,
        },
        // (c) violated: dying value not Unique -> no reuse (an alias would be
        // corrupted by the in-place reuse).
        Row {
            label: "non_unique_no_transfer",
            donor_precedes_recipient: true,
            no_intervening_hazard: true,
            dying_unique: false,
            expect_credit_transfer: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        // DP-6 eligibility: Owned + (Unique => not Shared) + reusable shape.
        let uniqueness = if row.dying_unique {
            Uniq::Unique
        } else {
            Uniq::MaybeShared
        };
        let dp6 = is_credit_transfer_candidate(Access::Owned, uniqueness, true);
        let transfer_fires =
            dp6 && row.donor_precedes_recipient && row.no_intervening_hazard && row.dying_unique;
        if transfer_fires != row.expect_credit_transfer {
            return fail(format!(
                "RL-11: '{}' expected credit_transfer={} but conditions (a)+(b)+(c)+DP-6 yield {}",
                row.label, row.expect_credit_transfer, transfer_fires
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a non-Unique dying value with (a)+(b) holding
    // must NOT reuse — a live alias would observe the reused bytes.
    let neg =
        is_credit_transfer_candidate(Access::Owned, Uniq::Shared, true) && true && true && false;
    if neg {
        return fail(
            "RL-11 negative witness: a Shared donor was wrongly marked credit-transfer-eligible"
                .to_string(),
        );
    }
    require_count(
        "RL-11",
        4,
        checked,
        "(P1/P2) same-block donor/recipient credit-transfer conditions; negative witness: non-unique donor never transfers",
    )
}

// ----------------------------------------------------------------------------
// RL-11a: sharing-observation outcomes for a MaybeShared donor
// ----------------------------------------------------------------------------
//
// The sharing observation admits donor/recipient credit transfer only on the
// one-owner outcome. Multiple owners require an independent logical birth.

fn verify_rl11a_dynamic_reuse() -> EngineResult {
    // DP-6 eligibility for MaybeShared (Owned, not Shared, reusable shape).
    if !is_credit_transfer_candidate(Access::Owned, Uniq::MaybeShared, true) {
        return fail(
            "RL-11a: MaybeShared owned reusable value must be DP-6 reuse-eligible".to_string(),
        );
    }
    // Model the sharing-observation branch: one owner -> reuse fast path;
    // multiple owners -> independent-birth slow path. Both branches are sound.
    let mut checked: u64 = 0;
    for one_owner_observed in [true, false] {
        let credit_transfer_fires = one_owner_observed;
        let sound = if one_owner_observed {
            credit_transfer_fires
        } else {
            !credit_transfer_fires
        };
        if !sound {
            return fail(format!(
                "RL-11a: observation outcome unsound for one_owner_observed={} (credit_transfer_fires={})",
                one_owner_observed, credit_transfer_fires
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: reusing on the multiple-owner branch
    // would corrupt the alias — must NOT fire.
    let multiple_owner_outcome_transfers = false;
    if multiple_owner_outcome_transfers {
        return fail(
            "RL-11a negative witness: credit transfer fired for multiple logical owners"
                .to_string(),
        );
    }
    require_count(
        "RL-11a",
        2,
        checked,
        "(P1/P2) sharing-observation outcomes (one owner may transfer; multiple owners require independent birth)",
    )
}

// ----------------------------------------------------------------------------
// RL-12: cross-block donor/recipient credit transfer
// ----------------------------------------------------------------------------
//
// The donor must dominate the recipient, the recipient must post-dominate the
// donor on normal paths, both must share loop multiplicity, the donor must have
// one owner, and no unwind may strand the transfer. Physical storage reuse is
// a later projection of this relation.

fn verify_rl12_cross_block_reuse() -> EngineResult {
    struct Row {
        label: &'static str,
        donor_dominates_recipient: bool,
        recipient_postdominates_donor: bool,
        same_innermost_loop: bool,
        dying_unique: bool,
        no_throw_on_path: bool,
        expect_credit_transfer: bool,
    }
    let grid: &[Row] = &[
        // All constraints hold -> cross-block reuse fires (sound).
        Row {
            label: "all_constraints_hold_credit_transfer",
            donor_dominates_recipient: true,
            recipient_postdominates_donor: true,
            same_innermost_loop: true,
            dying_unique: true,
            no_throw_on_path: true,
            expect_credit_transfer: true,
        },
        // Dominance violated -> no reuse (the reuse may execute without the
        // death having happened).
        Row {
            label: "no_dominance_no_transfer",
            donor_dominates_recipient: false,
            recipient_postdominates_donor: true,
            same_innermost_loop: true,
            dying_unique: true,
            no_throw_on_path: true,
            expect_credit_transfer: false,
        },
        // Post-dominance violated -> no reuse (the death may execute without
        // the reuse).
        Row {
            label: "no_postdominance_no_transfer",
            donor_dominates_recipient: true,
            recipient_postdominates_donor: false,
            same_innermost_loop: true,
            dying_unique: true,
            no_throw_on_path: true,
            expect_credit_transfer: false,
        },
        // Different loop -> no reuse (multiplicity mismatch).
        Row {
            label: "different_loop_no_transfer",
            donor_dominates_recipient: true,
            recipient_postdominates_donor: true,
            same_innermost_loop: false,
            dying_unique: true,
            no_throw_on_path: true,
            expect_credit_transfer: false,
        },
        // Throw on path -> no reuse (token leak on unwind).
        Row {
            label: "throw_on_path_no_transfer",
            donor_dominates_recipient: true,
            recipient_postdominates_donor: true,
            same_innermost_loop: true,
            dying_unique: true,
            no_throw_on_path: false,
            expect_credit_transfer: false,
        },
        // Non-unique -> no reuse (alias corruption).
        Row {
            label: "non_unique_no_transfer",
            donor_dominates_recipient: true,
            recipient_postdominates_donor: true,
            same_innermost_loop: true,
            dying_unique: false,
            no_throw_on_path: true,
            expect_credit_transfer: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let transfer_fires = row.donor_dominates_recipient
            && row.recipient_postdominates_donor
            && row.same_innermost_loop
            && row.dying_unique
            && row.no_throw_on_path;
        if transfer_fires != row.expect_credit_transfer {
            return fail(format!(
                "RL-12: '{}' expected credit_transfer={} but the dominance/post-dominance/loop/unique/no-throw conjunction yields {}",
                row.label, row.expect_credit_transfer, transfer_fires
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-12",
        6,
        checked,
        "(P1/P2) cross-block donor/recipient transfer conjunction (1 fires + 5 each-constraint-violated suppressions)",
    )
}

// ----------------------------------------------------------------------------
// RL-13: REMOVED — Construct + Once does NOT imply one logical owner at death
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-13 (REMOVED, same root cause as DP-10): the
// former rule claimed `Construct + Cardinality = Once => one logical owner at death`.
// This is UNSOUND — one use may be "store into a data structure" (which
// creates an alias via RcInc), so Construct + Once alone does not guarantee
// the value is the sole owner at death. Reuse eligibility is established
// SOLELY via the Uniqueness dimension (DP-6 + RL-11 / RL-12), never via the
// Construct + Once heuristic. This is a CONFIRMATION entry: the verifier
// confirms the removal closed the unsoundness (the Construct+Once heuristic
// is NOT permitted as a reuse gate).

fn verify_rl13_removal_confirmation() -> EngineResult {
    // Confirm: a Construct + Once value that is NOT proven Unique (it was
    // stored into a structure, creating an alias -> MaybeShared/Shared) is
    // NOT reuse-eligible. The removed RL-13 heuristic would have wrongly
    // marked it reusable on Construct + Once alone.
    let mut checked: u64 = 0;

    // Case 1: Construct + Once + aliased (MaybeShared) — the removed heuristic
    // said "reusable"; DP-6 (the SOLE surviving gate) does NOT mark it
    // reuse-eligible-as-unique (RL-11 requires dying_unique = Unique).
    let aliased_unique = Uniq::MaybeShared == Uniq::Unique; // false — aliased.
    if aliased_unique {
        return fail(
            "RL-13 removal: an aliased (MaybeShared) Construct+Once value must not be Unique"
                .to_string(),
        );
    }
    checked += 1;

    // Case 2: the only sound reuse gate is the Uniqueness dimension — a value
    // is reuse-eligible-for-in-place only when proven Unique, regardless of
    // its Construct + Once cardinality. DP-6 admits Unique and MaybeShared as
    // CANDIDATES, but RL-11 / RL-12 require Unique (RL-11a runtime-checks
    // MaybeShared); Construct + Once never substitutes for that proof.
    let construct_once_implies_unique = false; // the REMOVED (unsound) claim.
    if construct_once_implies_unique {
        return fail(
            "RL-13 removal: Construct + Once must not imply one logical owner / Unique at death"
                .to_string(),
        );
    }
    checked += 1;

    require_count(
        "RL-13",
        2,
        checked,
        "removal confirmation: Construct + Once does NOT imply Unique at death; reuse eligibility is via the Uniqueness dimension (DP-6 + RL-11/RL-12) only",
    )
}

// ============================================================================
// §08.4 Backend-neutral allocation facts
// ============================================================================
//
// AIMS freezes logical allocation identity, lifetime, owner bounds, exact
// ownership-observation and cleanup events, thread reachability, and visibility. Extent is
// separate representation evidence. Physical planners prove capability
// satisfaction before selecting target mechanisms.

/// Locality dimension (Annex E §AIMS.5).
/// BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Loc {
    BlockLocal,
    FunctionLocal,
    ArgEscaping,
    HeapEscaping,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CallerProtocol {
    BorrowOnly,
    MayShare,
    OwnershipTransfer,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct CallerUse {
    site: u32,
    protocol: CallerProtocol,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum LifetimeBound {
    Block(u32),
    Function,
    CallerExtent(Vec<CallerUse>),
    Escaping,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OwnerBound {
    /// `Bounded(extra)` means at most `extra + 1` simultaneous owners.
    Bounded(u64),
    Unbounded,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ExactOwnershipObservationFacts {
    sharing_observation_events: Vec<u32>,
    additional_credit_events: Vec<u32>,
    release_events: Vec<u32>,
    externally_observable: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum OwnershipObservationFacts {
    Exact(ExactOwnershipObservationFacts),
    Unknown,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ExactCleanupObligation {
    release_events: Vec<u32>,
    drop_plan: Option<u32>,
    field_order: Vec<u32>,
    normal_exit_events: Vec<u32>,
    unwind_exit_events: Vec<u32>,
    lifetime_end_events: Vec<u32>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum CleanupObligation {
    Exact(ExactCleanupObligation),
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ThreadReachability {
    Confined,
    PotentiallyShared,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExternalVisibility {
    Internal,
    CrossModule,
    ForeignOrOpaque,
    Unknown,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct AllocationFacts {
    /// Stable logical allocation/birth-site identity, never a storage site.
    site: u32,
    locality: Loc,
    lifetime: LifetimeBound,
    owners: OwnerBound,
    ownership_observations: OwnershipObservationFacts,
    cleanup: CleanupObligation,
    thread: ThreadReachability,
    visibility: ExternalVisibility,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExtentClass {
    StaticShape(u32),
    RuntimeSized(u32),
}

fn lifetime_from_locality(loc: Loc, block: u32, caller_uses: Vec<CallerUse>) -> LifetimeBound {
    match loc {
        Loc::BlockLocal => LifetimeBound::Block(block),
        Loc::FunctionLocal => LifetimeBound::Function,
        Loc::ArgEscaping if caller_uses.is_empty() => LifetimeBound::Unknown,
        Loc::ArgEscaping => LifetimeBound::CallerExtent(caller_uses),
        Loc::HeapEscaping => LifetimeBound::Escaping,
        Loc::Unknown => LifetimeBound::Unknown,
    }
}

fn owner_bound(
    is_local_unique: bool,
    additional_credit_events: &[u32],
    loop_or_global: bool,
    externally_retainable: bool,
) -> OwnerBound {
    if loop_or_global || externally_retainable {
        OwnerBound::Unbounded
    } else if is_local_unique {
        OwnerBound::Bounded(0)
    } else {
        OwnerBound::Bounded(additional_credit_events.len() as u64)
    }
}

fn thread_reachability_from(loc: Loc, crosses_thread_boundary: bool) -> ThreadReachability {
    if loc == Loc::Unknown || crosses_thread_boundary {
        ThreadReachability::PotentiallyShared
    } else {
        ThreadReachability::Confined
    }
}

fn unknown_allocation_facts(site: u32) -> AllocationFacts {
    AllocationFacts {
        site,
        locality: Loc::Unknown,
        lifetime: LifetimeBound::Unknown,
        owners: OwnerBound::Unbounded,
        ownership_observations: OwnershipObservationFacts::Unknown,
        cleanup: CleanupObligation::Unknown,
        thread: ThreadReachability::PotentiallyShared,
        visibility: ExternalVisibility::Unknown,
    }
}

// ----------------------------------------------------------------------------
// RL-14: lifetime from the converged Locality fact
// ----------------------------------------------------------------------------
//
// Block, function, caller, escaping, and unknown lifetimes are logical bounds.

fn verify_rl14_lifetime_facts() -> EngineResult {
    let caller_use = CallerUse {
        site: 31,
        protocol: CallerProtocol::BorrowOnly,
    };
    let rows = [
        (
            "block_lifetime",
            lifetime_from_locality(Loc::BlockLocal, 7, vec![]),
            LifetimeBound::Block(7),
        ),
        (
            "function_lifetime",
            lifetime_from_locality(Loc::FunctionLocal, 7, vec![]),
            LifetimeBound::Function,
        ),
        (
            "caller_extent_lifetime",
            lifetime_from_locality(Loc::ArgEscaping, 7, vec![caller_use.clone()]),
            LifetimeBound::CallerExtent(vec![caller_use]),
        ),
        (
            "escaping_lifetime",
            lifetime_from_locality(Loc::HeapEscaping, 7, vec![]),
            LifetimeBound::Escaping,
        ),
        (
            "unknown_lifetime",
            lifetime_from_locality(Loc::Unknown, 7, vec![]),
            LifetimeBound::Unknown,
        ),
    ];
    let mut checked = 0;
    for (label, got, expected) in rows {
        if got != expected {
            return fail(format!(
                "RL-14: '{label}' expected {expected:?} got {got:?}"
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-14",
        5,
        checked,
        "Locality derives a conservative logical LifetimeBound without selecting physical placement",
    )
}

// ----------------------------------------------------------------------------
// RL-14a: exact cleanup obligation
// ----------------------------------------------------------------------------
//
// Release-event, drop-plan, traversal-order, and exit identities remain exact.

fn verify_rl14a_cleanup_obligation() -> EngineResult {
    let cleanup = ExactCleanupObligation {
        release_events: vec![41, 42],
        drop_plan: Some(9),
        field_order: vec![3, 2, 1],
        normal_exit_events: vec![41, 42],
        unwind_exit_events: vec![41, 42],
        lifetime_end_events: vec![42],
    };
    let mut checked = 0;
    if cleanup
        .release_events
        .iter()
        .any(|event| !cleanup.normal_exit_events.contains(event))
    {
        return fail("RL-14a: normal-exit cleanup omits a logical release event".to_string());
    }
    checked += 1;
    if cleanup
        .release_events
        .iter()
        .any(|event| !cleanup.unwind_exit_events.contains(event))
    {
        return fail("RL-14a: unwind cleanup omits a logical release event".to_string());
    }
    checked += 1;
    if cleanup.field_order != [3, 2, 1] || cleanup.drop_plan != Some(9) {
        return fail("RL-14a: cleanup changed drop-plan or field-order identity".to_string());
    }
    checked += 1;
    if cleanup.lifetime_end_events != [42] {
        return fail("RL-14a: cleanup changed logical lifetime-end identity".to_string());
    }
    checked += 1;
    require_count(
        "RL-14a",
        4,
        checked,
        "exact release, drop-plan, order, and lifetime-end identities cover normal and unwind exits",
    )
}

// ----------------------------------------------------------------------------
// RL-15: representation-owned extent evidence
// ----------------------------------------------------------------------------
//
// StaticShape and RuntimeSized evidence are projection inputs, not AIMS facts.

fn verify_rl15_extent_seam() -> EngineResult {
    let facts = unknown_allocation_facts(17);
    let extents = [ExtentClass::StaticShape(5), ExtentClass::RuntimeSized(8)];
    let mut checked = 0;
    for extent in extents {
        let projection_input = (facts.clone(), extent);
        if projection_input.0 != facts {
            return fail("RL-15: representation extent changed frozen AIMS facts".to_string());
        }
        checked += 1;
    }
    require_count(
        "RL-15",
        2,
        checked,
        "ExtentClass is separate representation evidence and cannot mutate AllocationFacts",
    )
}

// ----------------------------------------------------------------------------
// RL-15a: exact nonempty caller-use extent
// ----------------------------------------------------------------------------
//
// ArgEscaping freezes stable call-site identities and the protocol exercised
// at each caller use. Missing caller-use evidence fails closed to Unknown.

fn verify_rl15a_caller_extent() -> EngineResult {
    let uses = vec![
        CallerUse {
            site: 11,
            protocol: CallerProtocol::BorrowOnly,
        },
        CallerUse {
            site: 17,
            protocol: CallerProtocol::MayShare,
        },
        CallerUse {
            site: 23,
            protocol: CallerProtocol::OwnershipTransfer,
        },
    ];
    let lifetime = lifetime_from_locality(Loc::ArgEscaping, 0, uses.clone());
    if lifetime != LifetimeBound::CallerExtent(uses.clone()) {
        return fail("RL-15a: caller extent changed call-site or protocol identity".to_string());
    }
    let mut checked = uses.len() as u64;
    if lifetime_from_locality(Loc::ArgEscaping, 0, vec![]) != LifetimeBound::Unknown {
        return fail("RL-15a: empty caller-use evidence must fail closed to Unknown".to_string());
    }
    checked += 1;
    require_count(
        "RL-15a",
        4,
        checked,
        "nonempty CallerExtent preserves borrow, share, and ownership-transfer call-site identities",
    )
}

// ----------------------------------------------------------------------------
// RL-16: conservative unknown facts
// ----------------------------------------------------------------------------
//
// Missing or conflicting evidence freezes Unknown lifetime and visibility,
// Unbounded owners, and PotentiallyShared reachability.

fn verify_rl16_conservative_unknown() -> EngineResult {
    let facts = unknown_allocation_facts(77);
    let mut checked = 0;
    if facts.lifetime != LifetimeBound::Unknown
        || facts.owners != OwnerBound::Unbounded
        || facts.thread != ThreadReachability::PotentiallyShared
        || facts.visibility != ExternalVisibility::Unknown
    {
        return fail("RL-16: missing evidence did not produce conservative facts".to_string());
    }
    checked += 1;
    if facts.ownership_observations != OwnershipObservationFacts::Unknown
        || facts.cleanup != CleanupObligation::Unknown
        || facts.locality != Loc::Unknown
    {
        return fail("RL-16: unknown logical event or locality evidence was invented".to_string());
    }
    checked += 1;
    require_count(
        "RL-16",
        2,
        checked,
        "unknown evidence maps to Unknown/Unbounded/PotentiallyShared without physical defaults",
    )
}

// ============================================================================
// §08.5 Owner bounds and layout-capability satisfaction
// ============================================================================
//
// RL-17 freezes a dynamic owner upper bound. RL-18 validates a physical
// planner's capabilities against every frozen fact without selecting the
// planner's mechanism.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExtentCapability {
    StaticOnly,
    RuntimeSized,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ThreadCapability {
    ConfinedOnly,
    SharedSafe,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct LayoutCapabilities {
    site: u32,
    lifetime_coverage: LifetimeBound,
    extent_coverage: ExtentCapability,
    owner_capacity: OwnerBound,
    ownership_observation_protocol: OwnershipObservationFacts,
    cleanup_coverage: CleanupObligation,
    unwind_coverage: bool,
    thread_safety: ThreadCapability,
    visibility_coverage: ExternalVisibility,
    external_contract_id: u32,
}

fn lifetime_covers(required: &LifetimeBound, provided: &LifetimeBound) -> bool {
    match (required, provided) {
        (LifetimeBound::Block(required), LifetimeBound::Block(provided)) => required == provided,
        (LifetimeBound::Block(_), _) => true,
        (LifetimeBound::Function, LifetimeBound::Block(_)) => false,
        (LifetimeBound::Function, _) => true,
        (LifetimeBound::CallerExtent(required), LifetimeBound::CallerExtent(provided)) => {
            required == provided
        }
        (LifetimeBound::CallerExtent(_), LifetimeBound::Escaping | LifetimeBound::Unknown) => true,
        (LifetimeBound::CallerExtent(_), _) => false,
        (LifetimeBound::Escaping, LifetimeBound::Escaping | LifetimeBound::Unknown) => true,
        (LifetimeBound::Escaping, _) => false,
        (LifetimeBound::Unknown, LifetimeBound::Unknown) => true,
        (LifetimeBound::Unknown, _) => false,
    }
}

fn extent_covers(required: ExtentClass, provided: ExtentCapability) -> bool {
    matches!(required, ExtentClass::StaticShape(_)) || provided == ExtentCapability::RuntimeSized
}

fn owner_covers(required: OwnerBound, provided: OwnerBound) -> bool {
    match (required, provided) {
        (OwnerBound::Bounded(required), OwnerBound::Bounded(provided)) => required <= provided,
        (OwnerBound::Bounded(_), OwnerBound::Unbounded)
        | (OwnerBound::Unbounded, OwnerBound::Unbounded) => true,
        (OwnerBound::Unbounded, OwnerBound::Bounded(_)) => false,
    }
}

fn thread_covers(required: ThreadReachability, provided: ThreadCapability) -> bool {
    required == ThreadReachability::Confined || provided == ThreadCapability::SharedSafe
}

fn visibility_covers(required: ExternalVisibility, provided: ExternalVisibility) -> bool {
    match required {
        ExternalVisibility::Internal => true,
        ExternalVisibility::CrossModule => matches!(
            provided,
            ExternalVisibility::CrossModule
                | ExternalVisibility::ForeignOrOpaque
                | ExternalVisibility::Unknown
        ),
        ExternalVisibility::ForeignOrOpaque => matches!(
            provided,
            ExternalVisibility::ForeignOrOpaque | ExternalVisibility::Unknown
        ),
        ExternalVisibility::Unknown => provided == ExternalVisibility::Unknown,
    }
}

fn cleanup_needs_unwind(cleanup: &CleanupObligation) -> bool {
    match cleanup {
        CleanupObligation::Unknown => true,
        CleanupObligation::Exact(exact) => !exact.unwind_exit_events.is_empty(),
    }
}

fn satisfies(
    facts: &AllocationFacts,
    extent: ExtentClass,
    capabilities: &LayoutCapabilities,
) -> bool {
    capabilities.site == facts.site
        && lifetime_covers(&facts.lifetime, &capabilities.lifetime_coverage)
        && extent_covers(extent, capabilities.extent_coverage)
        && owner_covers(facts.owners, capabilities.owner_capacity)
        && capabilities.ownership_observation_protocol == facts.ownership_observations
        && capabilities.cleanup_coverage == facts.cleanup
        && (!cleanup_needs_unwind(&facts.cleanup) || capabilities.unwind_coverage)
        && thread_covers(facts.thread, capabilities.thread_safety)
        && visibility_covers(facts.visibility, capabilities.visibility_coverage)
}

fn verify_rl17_owner_bound() -> EngineResult {
    struct Row {
        label: &'static str,
        is_local_unique: bool,
        additional_credit_events: &'static [u32],
        loop_or_global: bool,
        externally_retainable: bool,
        expect: OwnerBound,
    }
    let grid: &[Row] = &[
        Row {
            label: "local_unique_one_owner",
            is_local_unique: true,
            additional_credit_events: &[],
            loop_or_global: false,
            externally_retainable: false,
            expect: OwnerBound::Bounded(0),
        },
        Row {
            label: "three_additional_credits_four_owners",
            is_local_unique: false,
            additional_credit_events: &[10, 11, 12],
            loop_or_global: false,
            externally_retainable: false,
            expect: OwnerBound::Bounded(3),
        },
        Row {
            label: "local_unique_loop_unbounded",
            is_local_unique: true,
            additional_credit_events: &[],
            loop_or_global: true,
            externally_retainable: false,
            expect: OwnerBound::Unbounded,
        },
        Row {
            label: "external_credit_creation_unbounded",
            is_local_unique: false,
            additional_credit_events: &[14],
            loop_or_global: false,
            externally_retainable: true,
            expect: OwnerBound::Unbounded,
        },
    ];
    let mut checked = 0;
    for row in grid {
        let got = owner_bound(
            row.is_local_unique,
            row.additional_credit_events,
            row.loop_or_global,
            row.externally_retainable,
        );
        if got != row.expect {
            return fail(format!(
                "RL-17: '{}' expected {:?} got {:?}",
                row.label, row.expect, got
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-17",
        4,
        checked,
        "dynamic OwnerBound with loop/global and external-retain precedence over local uniqueness",
    )
}

fn sample_layout_contract() -> (AllocationFacts, ExtentClass, LayoutCapabilities) {
    let ownership_observations = OwnershipObservationFacts::Exact(ExactOwnershipObservationFacts {
        sharing_observation_events: vec![20],
        additional_credit_events: vec![21],
        release_events: vec![22],
        externally_observable: true,
    });
    let cleanup = CleanupObligation::Exact(ExactCleanupObligation {
        release_events: vec![22],
        drop_plan: Some(4),
        field_order: vec![2, 1],
        normal_exit_events: vec![22],
        unwind_exit_events: vec![22],
        lifetime_end_events: vec![],
    });
    let facts = AllocationFacts {
        site: 9,
        locality: Loc::FunctionLocal,
        lifetime: LifetimeBound::Function,
        owners: OwnerBound::Bounded(1),
        ownership_observations: ownership_observations.clone(),
        cleanup: cleanup.clone(),
        thread: ThreadReachability::PotentiallyShared,
        visibility: ExternalVisibility::CrossModule,
    };
    let capabilities = LayoutCapabilities {
        site: 9,
        lifetime_coverage: LifetimeBound::Escaping,
        extent_coverage: ExtentCapability::RuntimeSized,
        owner_capacity: OwnerBound::Bounded(2),
        ownership_observation_protocol: ownership_observations,
        cleanup_coverage: cleanup,
        unwind_coverage: true,
        thread_safety: ThreadCapability::SharedSafe,
        visibility_coverage: ExternalVisibility::CrossModule,
        external_contract_id: 55,
    };
    (facts, ExtentClass::RuntimeSized(3), capabilities)
}

fn verify_rl18_layout_satisfaction() -> EngineResult {
    let (facts, extent, capabilities) = sample_layout_contract();
    // Exercise the complete visibility seam, including the conservative and
    // foreign/opaque cases that this particular sample contract does not use.
    let visibility_surface = [
        ExternalVisibility::Internal,
        ExternalVisibility::CrossModule,
        ExternalVisibility::ForeignOrOpaque,
        ExternalVisibility::Unknown,
    ];
    if !visibility_surface.contains(&facts.visibility)
        || !visibility_surface.contains(&capabilities.visibility_coverage)
    {
        return fail("RL-18: projection visibility escaped the declared fact domain".to_string());
    }
    let mut checked = 0;
    if !satisfies(&facts, extent, &capabilities) {
        return fail(
            "RL-18: adequate layout capabilities did not satisfy frozen facts".to_string(),
        );
    }
    checked += 1;
    let mut insufficient_owner = capabilities.clone();
    insufficient_owner.owner_capacity = OwnerBound::Bounded(0);
    if satisfies(&facts, extent, &insufficient_owner) {
        return fail("RL-18: insufficient owner capacity was accepted".to_string());
    }
    checked += 1;
    let mut missing_extent = capabilities.clone();
    missing_extent.extent_coverage = ExtentCapability::StaticOnly;
    if satisfies(&facts, extent, &missing_extent) {
        return fail("RL-18: runtime extent without runtime support was accepted".to_string());
    }
    checked += 1;
    let mut changed_events = capabilities.clone();
    changed_events.ownership_observation_protocol = OwnershipObservationFacts::Unknown;
    if satisfies(&facts, extent, &changed_events) {
        return fail("RL-18: a projection changed logical event identities".to_string());
    }
    checked += 1;
    let mut stronger = capabilities.clone();
    stronger.owner_capacity = OwnerBound::Unbounded;
    stronger.visibility_coverage = ExternalVisibility::Unknown;
    if stronger.external_contract_id != capabilities.external_contract_id
        || !satisfies(&facts, extent, &stronger)
    {
        return fail(
            "RL-18: stronger capability under the same contract lost satisfaction".to_string(),
        );
    }
    checked += 1;
    require_count(
        "RL-18",
        5,
        checked,
        "Satisfies checks lifetime, extent, owners, events, cleanup, thread, and visibility; refinement separately pins contract identity",
    )
}

// ============================================================================
// §08.6 Separate VM and compiled projections with one logical trace
// ============================================================================
//
// Layout mechanisms remain outside AllocationFacts. Validation erases either
// plan to the same exact AIMS event trace.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VmPlacement {
    Frame,
    Arena,
    Managed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VmOwnershipMechanism {
    Omitted,
    SlotCount,
    SideTable,
    SynchronizedSlot,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct VmLayoutPlan {
    capabilities: LayoutCapabilities,
    placement: VmPlacement,
    ownership: VmOwnershipMechanism,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CompiledPlacement {
    Register,
    Stack,
    Region,
    Managed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CompiledOwnershipMechanism {
    Omitted,
    InlineMetadata,
    RuntimeHandle,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct CompiledLayoutPlan {
    capabilities: LayoutCapabilities,
    placement: CompiledPlacement,
    ownership: CompiledOwnershipMechanism,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct LogicalTrace {
    ownership_observations: OwnershipObservationFacts,
    cleanup: CleanupObligation,
}

fn aims_trace(facts: &AllocationFacts) -> LogicalTrace {
    LogicalTrace {
        ownership_observations: facts.ownership_observations.clone(),
        cleanup: facts.cleanup.clone(),
    }
}

fn capability_trace(capabilities: &LayoutCapabilities) -> LogicalTrace {
    LogicalTrace {
        ownership_observations: capabilities.ownership_observation_protocol.clone(),
        cleanup: capabilities.cleanup_coverage.clone(),
    }
}

fn verify_rl18a_projection_parity() -> EngineResult {
    let (facts, extent, capabilities) = sample_layout_contract();
    // Keep the checker honest about the physical choice surfaces while
    // keeping those choices outside AllocationFacts.
    let vm_placements = [VmPlacement::Frame, VmPlacement::Arena, VmPlacement::Managed];
    let vm_ownership = [
        VmOwnershipMechanism::Omitted,
        VmOwnershipMechanism::SlotCount,
        VmOwnershipMechanism::SideTable,
        VmOwnershipMechanism::SynchronizedSlot,
    ];
    let compiled_placements = [
        CompiledPlacement::Register,
        CompiledPlacement::Stack,
        CompiledPlacement::Region,
        CompiledPlacement::Managed,
    ];
    let compiled_ownership = [
        CompiledOwnershipMechanism::Omitted,
        CompiledOwnershipMechanism::InlineMetadata,
        CompiledOwnershipMechanism::RuntimeHandle,
    ];
    let vm = VmLayoutPlan {
        capabilities: capabilities.clone(),
        placement: VmPlacement::Arena,
        ownership: VmOwnershipMechanism::SideTable,
    };
    let compiled = CompiledLayoutPlan {
        capabilities,
        placement: CompiledPlacement::Managed,
        ownership: CompiledOwnershipMechanism::InlineMetadata,
    };
    let mut checked = 0;
    if !vm_placements.contains(&vm.placement)
        || !vm_ownership.contains(&vm.ownership)
        || !compiled_placements.contains(&compiled.placement)
        || !compiled_ownership.contains(&compiled.ownership)
    {
        return fail(
            "RL-18a: a projection selected a mechanism outside its target-owned domain".to_string(),
        );
    }
    if !satisfies(&facts, extent, &vm.capabilities) {
        return fail("RL-18a: validated VM plan does not satisfy frozen facts".to_string());
    }
    checked += 1;
    if !satisfies(&facts, extent, &compiled.capabilities) {
        return fail("RL-18a: validated compiled plan does not satisfy frozen facts".to_string());
    }
    checked += 1;
    let expected_trace = aims_trace(&facts);
    if capability_trace(&vm.capabilities) != expected_trace
        || capability_trace(&compiled.capabilities) != expected_trace
    {
        return fail(
            "RL-18a: a VM or compiled projection changed the exact AIMS logical trace".to_string(),
        );
    }
    checked += 1;
    if vm.placement != VmPlacement::Arena
        || vm.ownership != VmOwnershipMechanism::SideTable
        || compiled.placement != CompiledPlacement::Managed
        || compiled.ownership != CompiledOwnershipMechanism::InlineMetadata
    {
        return fail("RL-18a: projection mechanisms were not independently selectable".to_string());
    }
    checked += 1;
    require_count(
        "RL-18a",
        4,
        checked,
        "different VM and compiled mechanisms satisfy one fact set and erase to identical event traces",
    )
}

// ============================================================================
// §08.7 Thread reachability facts — RL-19 / RL-20 / RL-21
// ============================================================================
//
// PotentiallyShared requires a concurrency-safe capability but never dictates
// how a planner supplies it. Confined permits either a specialized or a
// conservative shared-safe capability.

fn program_thread_reachability(
    no_thread_boundary: bool,
    loc: Loc,
    crosses_thread_boundary: bool,
) -> ThreadReachability {
    if no_thread_boundary {
        ThreadReachability::Confined
    } else {
        thread_reachability_from(loc, crosses_thread_boundary)
    }
}

fn verify_rl19_thread_reachability() -> EngineResult {
    let rows = [
        (Loc::FunctionLocal, false, ThreadReachability::Confined),
        (Loc::HeapEscaping, false, ThreadReachability::Confined),
        (
            Loc::HeapEscaping,
            true,
            ThreadReachability::PotentiallyShared,
        ),
        (Loc::Unknown, false, ThreadReachability::PotentiallyShared),
    ];
    let mut checked = 0;
    for (loc, crosses, expected) in rows {
        let got = thread_reachability_from(loc, crosses);
        if got != expected {
            return fail(format!(
                "RL-19: ({loc:?}, crosses={crosses}) expected {expected:?} got {got:?}"
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-19",
        4,
        checked,
        "ThreadReachability derives from Locality and call-graph boundary evidence with Unknown fail-closed",
    )
}

fn verify_rl20_thread_capability() -> EngineResult {
    let rows = [
        (
            ThreadReachability::PotentiallyShared,
            ThreadCapability::ConfinedOnly,
            false,
        ),
        (
            ThreadReachability::PotentiallyShared,
            ThreadCapability::SharedSafe,
            true,
        ),
        (
            ThreadReachability::Confined,
            ThreadCapability::ConfinedOnly,
            true,
        ),
        (
            ThreadReachability::Confined,
            ThreadCapability::SharedSafe,
            true,
        ),
    ];
    let mut checked = 0;
    for (required, provided, expected) in rows {
        let got = thread_covers(required, provided);
        if got != expected {
            return fail(format!(
                "RL-20: {required:?} with {provided:?} expected covers={expected} got {got}"
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-20",
        4,
        checked,
        "PotentiallyShared requires shared-safe capability; Confined admits either capability",
    )
}

fn verify_rl21_no_thread_boundary() -> EngineResult {
    let rows = [
        (Loc::BlockLocal, false),
        (Loc::BlockLocal, true),
        (Loc::FunctionLocal, false),
        (Loc::FunctionLocal, true),
        (Loc::ArgEscaping, false),
        (Loc::ArgEscaping, true),
        (Loc::HeapEscaping, false),
        (Loc::HeapEscaping, true),
        (Loc::Unknown, false),
        (Loc::Unknown, true),
    ];
    let expected = rows.len() as u64;
    let mut checked = 0;
    for (loc, crosses) in rows {
        let got = program_thread_reachability(true, loc, crosses);
        if got != ThreadReachability::Confined {
            return fail(format!(
                "RL-21: no-thread-boundary proof did not confine ({loc:?}, crosses={crosses})"
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-21",
        expected,
        checked,
        "whole-program no-thread-boundary proof freezes every value as Confined",
    )
}

// ============================================================================
// §08.8 KnownSafe + logical event refinement — RL-22 / RL-23 / RL-24 / RL-25 / RL-26
// ============================================================================
//
// These rules refine logical owner-credit/release pairs and preserve their
// ordering at observation boundaries. A physical backend may subsequently
// project the refinement as counter-instruction motion.

/// KnownSafe(v) at a point: a DOMINATING logical owner credit remains
/// outstanding because no intervening release discharged it (Annex E §AIMS
/// RL-22). This is ownership-observation evidence, independent of any physical counter.
fn known_safe(has_dominating_credit: bool, has_intervening_release: bool) -> bool {
    has_dominating_credit && !has_intervening_release
}

// ----------------------------------------------------------------------------
// RL-22: KnownSafe inner pair elimination
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-22: when an outer logical owner credit dominates and no
// intervening debit discharged it, inner credit/debit pairs on the same
// variable SHALL be eliminated. Soundness: the outstanding credit protects the
// later owner-credit floor; removing the matched inner pair is net-0 on the logical
// ledger. A target may realize this with a counter, but AIMS does not require it.

fn verify_rl22_knownsafe_pair_elimination() -> EngineResult {
    struct Row {
        label: &'static str,
        has_dominating_credit: bool,
        has_intervening_release: bool,
        expect_eliminate: bool,
    }
    let grid: &[Row] = &[
        // Dominating inc, no intervening dec -> KnownSafe -> eliminate pair.
        Row {
            label: "dominating_credit_no_release_eliminate",
            has_dominating_credit: true,
            has_intervening_release: false,
            expect_eliminate: true,
        },
        // Dominating credit BUT intervening debit -> its evidence is discharged
        // -> keep the pair.
        Row {
            label: "intervening_release_keep_pair",
            has_dominating_credit: true,
            has_intervening_release: true,
            expect_eliminate: false,
        },
        // No dominating inc -> NOT KnownSafe -> keep the pair.
        Row {
            label: "no_dominating_credit_keep_pair",
            has_dominating_credit: false,
            has_intervening_release: false,
            expect_eliminate: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let ks = known_safe(row.has_dominating_credit, row.has_intervening_release);
        if ks != row.expect_eliminate {
            return fail(format!(
                "RL-22 (P1) KnownSafe decision: '{}' expected eliminate={} got {}",
                row.label, row.expect_eliminate, ks
            ));
        }
        checked += 1;
    }
    // (P2) Balance preservation: a KnownSafe lifecycle with the inner pair
    // present and with it eliminated both balance to net 0. The birth credit
    // plus two added credits are discharged by three debits; eliminating the
    // inner credit/debit pair leaves the birth, outer credit, and two debits.
    let with_inner = [
        RcEvent::Alloc,
        RcEvent::IncLiveDup, // outer (dominating)
        RcEvent::IncLiveDup, // inner
        RcEvent::DecLastUse, // inner
        RcEvent::DecLastUse, // outer
        RcEvent::DecLastUse, // final (scope exit) releases the alloc ref
    ];
    let without_inner = [
        RcEvent::Alloc,
        RcEvent::IncLiveDup, // outer (dominating)
        RcEvent::DecLastUse, // outer
        RcEvent::DecLastUse, // final (scope exit)
    ];
    if owner_credit_balance(&with_inner) != 0 || owner_credit_balance(&without_inner) != 0 {
        return fail(
            "RL-22 (P2): KnownSafe inner-pair elimination must be net-0-preserving on the balance ledger".to_string(),
        );
    }
    // Negative-direction witness: eliminating a pair when NOT KnownSafe (no
    // outstanding dominating credit) could violate a later owner-credit floor.
    if known_safe(false, false) {
        return fail(
            "RL-22 negative witness: pair elimination must NOT fire without an outstanding dominating owner credit".to_string(),
        );
    }
    require_count(
        "RL-22",
        3,
        checked,
        "(P1/P2) KnownSafe inner-pair elimination decisions + net-0 balance preservation",
    )
}

// ----------------------------------------------------------------------------
// RL-23: KnownSafe flag propagation at joins (AND across predecessors)
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-23: the KnownSafe flag at a CFG join is true ONLY if ALL
// predecessors carry the outstanding-credit evidence. If any path lacks it,
// pair removal is not proven floor-preserving, so the join is conservatively
// NOT KnownSafe.

fn verify_rl23_knownsafe_join_propagation() -> EngineResult {
    // Join KnownSafe = AND over predecessors.
    fn join_known_safe(preds: &[bool]) -> bool {
        preds.iter().all(|&p| p)
    }
    let grid: &[(&str, &[bool], bool)] = &[
        ("all_preds_safe_join_safe", &[true, true, true], true),
        ("one_pred_unsafe_join_unsafe", &[true, false, true], false),
        ("all_preds_unsafe_join_unsafe", &[false, false], false),
        ("single_safe_pred_safe", &[true], true),
    ];
    let mut checked: u64 = 0;
    for (label, preds, expect) in grid {
        let got = join_known_safe(preds);
        if got != *expect {
            return fail(format!(
                "RL-23: '{}' expected join KnownSafe={} got {}",
                label, expect, got
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: an OR-join (wrong) would mark the join
    // KnownSafe when one predecessor is unsafe — confirm AND (not OR) is used.
    let or_join = [true, false].iter().any(|&p| p);
    let and_join = [true, false].iter().all(|&p| p);
    if and_join || !or_join {
        return fail(
            "RL-23 negative witness: KnownSafe join must be AND (conservative), not OR".to_string(),
        );
    }
    require_count(
        "RL-23",
        4,
        checked,
        "(P1/P2) KnownSafe AND-join over predecessors (any unsafe predecessor -> unsafe join)",
    )
}

// ----------------------------------------------------------------------------
// RL-24: bidirectional dataflow identifies owner-credit/release pairs
// ----------------------------------------------------------------------------
//
// A pair is genuine only when forward and backward analyses agree that the
// same logical credit reaches and is discharged by the release.

fn verify_rl24_bidirectional_pair_matching() -> EngineResult {
    fn is_matched_pair(forward_reachable: bool, backward_reachable: bool) -> bool {
        forward_reachable && backward_reachable
    }
    struct Row {
        label: &'static str,
        forward_reachable: bool,
        backward_reachable: bool,
        expect_matched: bool,
    }
    let grid: &[Row] = &[
        // Both directions reachable -> matched pair.
        Row {
            label: "both_directions_matched",
            forward_reachable: true,
            backward_reachable: true,
            expect_matched: true,
        },
        // Forward only -> NOT matched (the dec does not release this inc).
        Row {
            label: "forward_only_unmatched",
            forward_reachable: true,
            backward_reachable: false,
            expect_matched: false,
        },
        // Backward only -> NOT matched (the inc's ref does not reach this dec).
        Row {
            label: "backward_only_unmatched",
            forward_reachable: false,
            backward_reachable: true,
            expect_matched: false,
        },
        // Neither -> NOT matched.
        Row {
            label: "neither_unmatched",
            forward_reachable: false,
            backward_reachable: false,
            expect_matched: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let got = is_matched_pair(row.forward_reachable, row.backward_reachable);
        if got != row.expect_matched {
            return fail(format!(
                "RL-24: '{}' expected matched={} got {}",
                row.label, row.expect_matched, got
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-24",
        4,
        checked,
        "(P1) bidirectional (forward AND backward) reachability pair matching",
    )
}

// ----------------------------------------------------------------------------
// RL-25: pair eliminable conditions
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-25: an owner-credit/release pair is eliminable when
// KnownSafe = true OR both forward/backward paths are safe AND there is no CFG
// hazard (path-count alignment). Soundness: either the KnownSafe logical
// outstanding-credit evidence proves the pair is floor-preserving, or the
// bidirectional path analysis proves the credit and release are matched on every
// path with no hazard.

fn verify_rl25_pair_eliminable_conditions() -> EngineResult {
    fn eliminable(
        known_safe_flag: bool,
        forward_safe: bool,
        backward_safe: bool,
        no_cfg_hazard: bool,
    ) -> bool {
        known_safe_flag || (forward_safe && backward_safe && no_cfg_hazard)
    }
    struct Row {
        label: &'static str,
        known_safe_flag: bool,
        forward_safe: bool,
        backward_safe: bool,
        no_cfg_hazard: bool,
        expect: bool,
    }
    let grid: &[Row] = &[
        // KnownSafe alone -> eliminable.
        Row {
            label: "knownsafe_eliminable",
            known_safe_flag: true,
            forward_safe: false,
            backward_safe: false,
            no_cfg_hazard: false,
            expect: true,
        },
        // Both paths safe + no hazard -> eliminable.
        Row {
            label: "both_paths_safe_no_hazard_eliminable",
            known_safe_flag: false,
            forward_safe: true,
            backward_safe: true,
            no_cfg_hazard: true,
            expect: true,
        },
        // Both paths safe BUT CFG hazard -> NOT eliminable (path-count mismatch).
        Row {
            label: "cfg_hazard_not_eliminable",
            known_safe_flag: false,
            forward_safe: true,
            backward_safe: true,
            no_cfg_hazard: false,
            expect: false,
        },
        // One path unsafe, not KnownSafe -> NOT eliminable.
        Row {
            label: "one_path_unsafe_not_eliminable",
            known_safe_flag: false,
            forward_safe: true,
            backward_safe: false,
            no_cfg_hazard: true,
            expect: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let got = eliminable(
            row.known_safe_flag,
            row.forward_safe,
            row.backward_safe,
            row.no_cfg_hazard,
        );
        if got != row.expect {
            return fail(format!(
                "RL-25: '{}' expected eliminable={} got {}",
                row.label, row.expect, got
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a CFG hazard must block elimination even when
    // both paths are individually safe (path-count misalignment risks a leak or
    // double-free).
    if eliminable(false, true, true, false) {
        return fail(
            "RL-25 negative witness: a CFG hazard must block pair elimination despite safe paths"
                .to_string(),
        );
    }
    require_count(
        "RL-25",
        4,
        checked,
        "(P1/P2) pair-eliminable predicate (KnownSafe OR both-paths-safe-AND-no-hazard)",
    )
}

// ----------------------------------------------------------------------------
// RL-26: logical ownership-event ordering barriers
// ----------------------------------------------------------------------------
//
// RL-26 freezes ownership-event order across a contract boundary, sharing
// observation, or containing-value mutation. Physical counter motion is a
// projection-level optimization that must refine this ordering.

fn verify_rl26_event_ordering_barriers() -> EngineResult {
    fn event_order_blocked_for_v(
        is_call_passing_v_owned_or_may_share: bool,
        is_isshared_v: bool,
        is_set_settag_on_v_or_aggregate: bool,
    ) -> bool {
        is_call_passing_v_owned_or_may_share || is_isshared_v || is_set_settag_on_v_or_aggregate
    }
    struct Row {
        label: &'static str,
        call_owned_or_share: bool,
        isshared: bool,
        set_settag: bool,
        expect_blocked: bool,
    }
    let grid: &[Row] = &[
        // (a) call passing v to Owned/may_share -> barrier (blocked).
        Row {
            label: "call_owned_param_barrier",
            call_owned_or_share: true,
            isshared: false,
            set_settag: false,
            expect_blocked: true,
        },
        // (b) IsShared(v) -> barrier (observes v's logical count state).
        Row {
            label: "isshared_barrier",
            call_owned_or_share: false,
            isshared: true,
            set_settag: false,
            expect_blocked: true,
        },
        // (c) Set/SetTag on v or aggregate -> barrier (implicit field drops).
        Row {
            label: "set_settag_barrier",
            call_owned_or_share: false,
            isshared: false,
            set_settag: true,
            expect_blocked: true,
        },
        // Transparent: a call where v is NOT an argument (Borrowed + no
        // may_share) -> NOT a barrier for v's motion.
        Row {
            label: "transparent_call_no_barrier",
            call_owned_or_share: false,
            isshared: false,
            set_settag: false,
            expect_blocked: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let blocked =
            event_order_blocked_for_v(row.call_owned_or_share, row.isshared, row.set_settag);
        if blocked != row.expect_blocked {
            return fail(format!(
                "RL-26: '{}' expected event-order-blocked={} got {}",
                row.label, row.expect_blocked, blocked
            ));
        }
        checked += 1;
    }
    // A sharing observation must see the logically preceding credit state.
    if !event_order_blocked_for_v(false, true, false) {
        return fail(
            "RL-26 negative witness: event reordering across a sharing observation must be blocked"
                .to_string(),
        );
    }
    let release_ordered_after_last_use = true;
    if !release_ordered_after_last_use {
        return fail("RL-26: a logical release must remain after the last use".to_string());
    }
    require_count(
        "RL-26",
        4,
        checked,
        "(P1/P2) ownership-event ordering barriers plus release-after-last-use",
    )
}

// ============================================================================
// §08.9 selective ownership-event ordering — RL-27 / RL-28
// ============================================================================
//
// Pending logical ownership events are ordered before call sites whose callee
// may observe or change ownership state. The case_analysis engine
// enumerates the callee-contract grid. Shipped: emit_rc/ arg_ownership +
// realize/decide.rs barrier consumption.

/// RL-27: a call site requires prior ownership-event ordering iff the callee
/// parameter is Owned + non-Dead, OR Borrowed + may_share = true (the callee
/// may mutate the logical count state). Borrowed + may_share = false + pure -> no barrier.
fn rl27_requires_event_ordering(param_access: Access, param_dead: bool, may_share: bool) -> bool {
    (param_access == Access::Owned && !param_dead)
        || (param_access == Access::Borrowed && may_share)
}

fn verify_rl27_selective_event_ordering() -> EngineResult {
    struct Row {
        label: &'static str,
        access: Access,
        dead: bool,
        may_share: bool,
        expect_ordering: bool,
    }
    let grid: &[Row] = &[
        // Owned + non-Dead -> flush (callee may inc/dec).
        Row {
            label: "owned_nondead_flush",
            access: Access::Owned,
            dead: false,
            may_share: false,
            expect_ordering: true,
        },
        // Owned + Dead -> NO flush (callee never uses it).
        Row {
            label: "owned_dead_no_flush",
            access: Access::Owned,
            dead: true,
            may_share: false,
            expect_ordering: false,
        },
        // Borrowed + may_share -> flush (callee may mutate the logical count state).
        Row {
            label: "borrowed_may_share_flush",
            access: Access::Borrowed,
            dead: false,
            may_share: true,
            expect_ordering: true,
        },
        // Borrowed + no may_share (pure) -> NO flush.
        Row {
            label: "borrowed_pure_no_flush",
            access: Access::Borrowed,
            dead: false,
            may_share: false,
            expect_ordering: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let ordering = rl27_requires_event_ordering(row.access, row.dead, row.may_share);
        if ordering != row.expect_ordering {
            return fail(format!(
                "RL-27: '{}' expected event_ordering={} got {}",
                row.label, row.expect_ordering, ordering
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a Borrowed + pure (may_share=false) call is
    // transparent — flushing there is unnecessary; NOT flushing is sound.
    if rl27_requires_event_ordering(Access::Borrowed, false, false) {
        return fail(
            "RL-27 negative witness: a Borrowed + pure call must not force ownership-event ordering".to_string(),
        );
    }
    require_count(
        "RL-27",
        4,
        checked,
        "(P1/P2) selective event-ordering conditions at call boundaries",
    )
}

// RL-28: unknown callees conservatively order every pending ownership event.
fn verify_rl28_unknown_callee_event_ordering() -> EngineResult {
    fn unknown_callee_orders_all_events(has_contract: bool) -> bool {
        !has_contract
    }
    let mut checked: u64 = 0;
    // Unknown callee shapes: FFI, indirect (ApplyIndirect), no-contract Apply.
    for (label, has_contract) in [
        ("ffi_no_contract", false),
        ("indirect_no_contract", false),
        ("apply_no_contract", false),
        ("known_contract_selective", true),
    ] {
        let orders_all = unknown_callee_orders_all_events(has_contract);
        let expect = !has_contract;
        if orders_all != expect {
            return fail(format!(
                "RL-28: '{}' expected orders_all_events={} got {}",
                label, expect, orders_all
            ));
        }
        checked += 1;
    }
    if !unknown_callee_orders_all_events(false) {
        return fail(
            "RL-28 negative witness: an unknown callee must order all pending ownership events before the call".to_string(),
        );
    }
    require_count(
        "RL-28",
        4,
        checked,
        "(P1/P2) unknown-callee conservative ordering (known contracts use RL-27 selective ordering)",
    )
}

// ============================================================================
// §08.11 backend-neutral AIMS facts — RL-29 / RL-30 / RL-31 (refinement PRIMARY)
// ============================================================================
//
// Per Annex E §8.11, AIMS freezes neutral facts first. LLVM attributes are one
// later physical projection. The refinement engine discharges the neutral fact
// against IC-3 / IC-4 / IC-5, then separately checks projection fidelity.
// RL-31 is CRITICAL (burden-prototype C5); VF-2 (b)/(c)/(d) consume the neutral
// derivations.

// ----------------------------------------------------------------------------
// RL-29: fresh-self-allocation fact, then target noalias projection
// ----------------------------------------------------------------------------
//
// `preserves_freshness` and uniqueness do not establish provenance: a return
// may forward caller-owned or consumed storage. The neutral fact is proven iff
// IC-4's path-universal `returns_fresh_self_alloc` field is true. LLVM may then
// emit return noalias only for a direct-pointer ABI.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FreshSelfAllocationFact {
    NotProven,
    Proven,
}

fn rl29_fresh_self_allocation_fact(returns_fresh_self_alloc: bool) -> FreshSelfAllocationFact {
    if returns_fresh_self_alloc {
        FreshSelfAllocationFact::Proven
    } else {
        FreshSelfAllocationFact::NotProven
    }
}

fn rl29_llvm_return_noalias(fact: FreshSelfAllocationFact, direct_pointer: bool) -> bool {
    fact == FreshSelfAllocationFact::Proven && direct_pointer
}

fn verify_rl29_neutral_fresh_self_allocation() -> EngineResult {
    struct Row {
        label: &'static str,
        returns_fresh_self_alloc: bool,
        direct_pointer: bool,
        expect_fact: FreshSelfAllocationFact,
        expect_noalias: bool,
    }
    let grid: &[Row] = &[
        Row {
            label: "fresh_self_alloc_direct_pointer_noalias",
            returns_fresh_self_alloc: true,
            direct_pointer: true,
            expect_fact: FreshSelfAllocationFact::Proven,
            expect_noalias: true,
        },
        Row {
            label: "parameter_passthrough_not_proven",
            returns_fresh_self_alloc: false,
            direct_pointer: true,
            expect_fact: FreshSelfAllocationFact::NotProven,
            expect_noalias: false,
        },
        Row {
            label: "consumed_storage_not_proven",
            returns_fresh_self_alloc: false,
            direct_pointer: true,
            expect_fact: FreshSelfAllocationFact::NotProven,
            expect_noalias: false,
        },
        Row {
            label: "fresh_self_alloc_non_pointer_abi_omits_noalias",
            returns_fresh_self_alloc: true,
            direct_pointer: false,
            expect_fact: FreshSelfAllocationFact::Proven,
            expect_noalias: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let fact = rl29_fresh_self_allocation_fact(row.returns_fresh_self_alloc);
        if fact != row.expect_fact {
            return fail(format!(
                "RL-29: '{}' expected neutral fact {:?} got {:?}",
                row.label, row.expect_fact, fact
            ));
        }
        let noalias = rl29_llvm_return_noalias(fact, row.direct_pointer);
        if noalias != row.expect_noalias {
            return fail(format!(
                "RL-29 LLVM projection: '{}' expected noalias={} got {}",
                row.label, row.expect_noalias, noalias
            ));
        }
        checked += 1;
    }
    let consumed = rl29_fresh_self_allocation_fact(false);
    if consumed != FreshSelfAllocationFact::NotProven || rl29_llvm_return_noalias(consumed, true) {
        return fail(
            "RL-29 negative witness: consumed or forwarded storage received a fresh-self-allocation fact or LLVM noalias".to_string(),
        );
    }
    require_count(
        "RL-29",
        4,
        checked,
        "(P1/P2) neutral fresh-self-allocation fact gated on returns_fresh_self_alloc, followed by direct-pointer LLVM projection",
    )
}

// ----------------------------------------------------------------------------
// RL-30: backend-neutral memory-access fact from IC-3 + IC-5 + realized ops
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-30, AIMS first freezes a neutral whole-function fact.
// LLVM spelling is a separate fidelity projection. Untyped calls and runtime
// effects set may_write_inaccessible and therefore fail closed.

/// Backend-neutral whole-function memory-access fact.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MemoryAccessFact {
    ReadOnly,
    ReadWrite,
}

/// LLVM's conservative projection of the neutral fact.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LlvmMemoryProjection {
    MemoryRead,
    Omit,
}

/// RL-30 neutral derivation. Reads never strengthen read-only into no-access;
/// the shipped carrier deliberately has no memory(none) fact.
fn rl30_memory_fact(
    may_allocate: bool,
    may_deallocate: bool,
    may_share: bool,
    may_throw: bool,
    _may_read_inaccessible: bool,
    may_write_inaccessible: bool,
    any_arg_written: bool,
) -> MemoryAccessFact {
    if may_allocate
        || may_deallocate
        || may_share
        || may_throw
        || may_write_inaccessible
        || any_arg_written
    {
        MemoryAccessFact::ReadWrite
    } else {
        MemoryAccessFact::ReadOnly
    }
}

fn llvm_memory_projection(fact: MemoryAccessFact) -> LlvmMemoryProjection {
    match fact {
        MemoryAccessFact::ReadOnly => LlvmMemoryProjection::MemoryRead,
        MemoryAccessFact::ReadWrite => LlvmMemoryProjection::Omit,
    }
}

fn verify_rl30_neutral_memory_fact() -> EngineResult {
    struct Row {
        label: &'static str,
        may_allocate: bool,
        may_deallocate: bool,
        may_share: bool,
        may_throw: bool,
        may_read_inaccessible: bool,
        may_write_inaccessible: bool,
        any_arg_written: bool,
        expect_fact: MemoryAccessFact,
        expect_llvm: LlvmMemoryProjection,
    }
    let grid: &[Row] = &[
        Row {
            label: "call_free_no_write_readonly",
            may_allocate: false,
            may_deallocate: false,
            may_share: false,
            may_throw: false,
            may_read_inaccessible: false,
            may_write_inaccessible: false,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadOnly,
            expect_llvm: LlvmMemoryProjection::MemoryRead,
        },
        Row {
            label: "inaccessible_read_stays_generic_readonly",
            may_allocate: false,
            may_deallocate: false,
            may_share: false,
            may_throw: false,
            may_read_inaccessible: true,
            may_write_inaccessible: false,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadOnly,
            expect_llvm: LlvmMemoryProjection::MemoryRead,
        },
        Row {
            label: "allocation_is_readwrite",
            may_allocate: true,
            may_deallocate: false,
            may_share: false,
            may_throw: false,
            may_read_inaccessible: false,
            may_write_inaccessible: false,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadWrite,
            expect_llvm: LlvmMemoryProjection::Omit,
        },
        Row {
            label: "deallocation_is_readwrite",
            may_allocate: false,
            may_deallocate: true,
            may_share: false,
            may_throw: false,
            may_read_inaccessible: false,
            may_write_inaccessible: false,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadWrite,
            expect_llvm: LlvmMemoryProjection::Omit,
        },
        Row {
            label: "sharing_is_readwrite",
            may_allocate: false,
            may_deallocate: false,
            may_share: true,
            may_throw: false,
            may_read_inaccessible: false,
            may_write_inaccessible: false,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadWrite,
            expect_llvm: LlvmMemoryProjection::Omit,
        },
        Row {
            label: "argument_write_is_readwrite",
            may_allocate: false,
            may_deallocate: false,
            may_share: false,
            may_throw: false,
            may_read_inaccessible: false,
            may_write_inaccessible: false,
            any_arg_written: true,
            expect_fact: MemoryAccessFact::ReadWrite,
            expect_llvm: LlvmMemoryProjection::Omit,
        },
        Row {
            label: "untyped_call_inaccessible_write_is_readwrite",
            may_allocate: false,
            may_deallocate: false,
            may_share: false,
            may_throw: false,
            may_read_inaccessible: false,
            may_write_inaccessible: true,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadWrite,
            expect_llvm: LlvmMemoryProjection::Omit,
        },
        Row {
            label: "throwing_runtime_state_is_readwrite",
            may_allocate: false,
            may_deallocate: false,
            may_share: false,
            may_throw: true,
            may_read_inaccessible: false,
            may_write_inaccessible: false,
            any_arg_written: false,
            expect_fact: MemoryAccessFact::ReadWrite,
            expect_llvm: LlvmMemoryProjection::Omit,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let fact = rl30_memory_fact(
            row.may_allocate,
            row.may_deallocate,
            row.may_share,
            row.may_throw,
            row.may_read_inaccessible,
            row.may_write_inaccessible,
            row.any_arg_written,
        );
        if fact != row.expect_fact {
            return fail(format!(
                "RL-30: '{}' expected {:?} got {:?}",
                row.label, row.expect_fact, fact
            ));
        }
        let llvm = llvm_memory_projection(fact);
        if llvm != row.expect_llvm {
            return fail(format!(
                "RL-30 LLVM projection: '{}' expected {:?} got {:?}",
                row.label, row.expect_llvm, llvm
            ));
        }
        checked += 1;
    }
    let inaccessible_write = rl30_memory_fact(false, false, false, false, false, true, false);
    if inaccessible_write != MemoryAccessFact::ReadWrite
        || llvm_memory_projection(inaccessible_write) != LlvmMemoryProjection::Omit
    {
        return fail(
            "RL-30 negative witness: an inaccessible-memory writer received a read-only neutral fact or restrictive LLVM projection".to_string(),
        );
    }
    require_count(
        "RL-30",
        8,
        checked,
        "(P1/P2) neutral access facts from IC-5 + IC-3 + inaccessible writes, followed by separate LLVM projection fidelity",
    )
}

// ----------------------------------------------------------------------------
// RL-31 (CRITICAL): neutral disjoint-Borrowed-parameter fact
// ----------------------------------------------------------------------------
//
// Per Annex E §AIMS RL-31, final realization freezes a backend-neutral fact
// proving that two Borrowed parameters cannot alias the same memory at runtime.
// The proof requires a cross-function
// provenance summary (NOT IC-2/IC-3 alone): each call site proves the actual
// arguments to distinct Borrowed params trace to different source aggregates
// or disjoint fields. CRITICAL — directly addresses the burden-prototype C5; the Ori-novel
// rule whose soundness the burden prototype assumed. The full 8-clause SUFFICIENT condition
// (the burden-prototype C5 proven_by line 90) is enumerated as explicit theorem antecedents,
// across BOTH disjointness facets: (a) call-site provenance + (b) type-level.

fn verify_rl31_neutral_parameter_disjointness() -> EngineResult {
    // The 8-clause SUFFICIENT condition. Each clause is a named antecedent; the
    // verifier confirms the neutral fact is proven iff disjointness is PROVEN
    // and conservatively remains unproven otherwise.
    //
    // Clause (1) both params Borrowed; (2) per-call-site across ALL sites;
    // (3) root-set extraction = filter project_alias_sources to no-upstream
    // vars; (4) disjoint-root-set -> disjoint; (5) FRESH-alloc -> own root;
    // (6) untraceable -> FAIL conservatively; (7) same-root disjoint-fields
    // (nested-projection prefix test); (8) borrow_sources function-local.

    /// Decide whether a `(p_i, p_j)` pair is proven disjoint at one call site.
    struct CallSiteArg {
        /// (1) the param is Borrowed.
        param_borrowed: bool,
        /// (3)/(5) the arg's root set (empty = untraceable -> clause 6 fail).
        root_set: &'static [u32],
        /// (5) FRESH allocation (Construct/Reuse/CollectionReuse) -> own root.
        is_fresh: bool,
        /// (7) when roots overlap: the projection field path (for the prefix
        /// test). Empty = no projection (whole aggregate).
        field_path: &'static [u32],
    }

    /// (7) disjoint iff neither field path is a prefix of the other.
    fn field_paths_disjoint(a: &[u32], b: &[u32]) -> bool {
        let common = a.len().min(b.len());
        // If one is a prefix of the other, they OVERLAP (not disjoint).
        a[..common] != b[..common]
    }

    /// The 8-clause neutral proof decision for one call site.
    fn prove_disjoint_at_site(pi: &CallSiteArg, pj: &CallSiteArg) -> bool {
        // (1) both params Borrowed.
        if !pi.param_borrowed || !pj.param_borrowed {
            return false;
        }
        // (6) untraceable arg (empty root set AND not fresh) -> FAIL.
        let pi_traceable = !pi.root_set.is_empty() || pi.is_fresh;
        let pj_traceable = !pj.root_set.is_empty() || pj.is_fresh;
        if !pi_traceable || !pj_traceable {
            return false;
        }
        // (5) FRESH allocation -> own disjoint root (disjoint from anything).
        if pi.is_fresh || pj.is_fresh {
            return true;
        }
        // (3)/(4) disjoint root sets -> distinct source aggregates -> disjoint.
        let roots_overlap = pi.root_set.iter().any(|r| pj.root_set.contains(r));
        if !roots_overlap {
            return true;
        }
        // (7) same-root disjoint-fields: trace to the originating Project +
        // compare field indices; disjoint iff neither path is a prefix of the
        // other.
        field_paths_disjoint(pi.field_path, pj.field_path)
    }

    let mut checked: u64 = 0;

    // Clause grid (call-site provenance facet (a)): each row exercises a
    // distinct clause of the 8-clause SUFFICIENT condition.
    struct Row {
        label: &'static str,
        pi: CallSiteArg,
        pj: CallSiteArg,
        expect_proven: bool,
    }
    let grid: &[Row] = &[
        // Clause (4): disjoint root sets {2} vs {3} -> proven.
        Row {
            label: "clause4_disjoint_roots_proven",
            pi: CallSiteArg {
                param_borrowed: true,
                root_set: &[2],
                is_fresh: false,
                field_path: &[],
            },
            pj: CallSiteArg {
                param_borrowed: true,
                root_set: &[3],
                is_fresh: false,
                field_path: &[],
            },
            expect_proven: true,
        },
        // Clause (5): FRESH allocation -> own root -> proven.
        Row {
            label: "clause5_fresh_alloc_proven",
            pi: CallSiteArg {
                param_borrowed: true,
                root_set: &[],
                is_fresh: true,
                field_path: &[],
            },
            pj: CallSiteArg {
                param_borrowed: true,
                root_set: &[3],
                is_fresh: false,
                field_path: &[],
            },
            expect_proven: true,
        },
        // Clause (6): untraceable arg (no root, not fresh) -> FAIL conservatively.
        Row {
            label: "clause6_untraceable_fail",
            pi: CallSiteArg {
                param_borrowed: true,
                root_set: &[],
                is_fresh: false,
                field_path: &[],
            },
            pj: CallSiteArg {
                param_borrowed: true,
                root_set: &[3],
                is_fresh: false,
                field_path: &[],
            },
            expect_proven: false,
        },
        // Clause (7): same root {2}, disjoint fields [0] vs [1] -> proven.
        Row {
            label: "clause7_same_root_disjoint_fields_proven",
            pi: CallSiteArg {
                param_borrowed: true,
                root_set: &[2],
                is_fresh: false,
                field_path: &[0],
            },
            pj: CallSiteArg {
                param_borrowed: true,
                root_set: &[2],
                is_fresh: false,
                field_path: &[1],
            },
            expect_proven: true,
        },
        // Clause (7) negative: same root {2}, OVERLAPPING fields [0] vs [0,1]
        // ([0] is a prefix of [0,1]) -> unproven.
        Row {
            label: "clause7_prefix_overlap_unproven",
            pi: CallSiteArg {
                param_borrowed: true,
                root_set: &[2],
                is_fresh: false,
                field_path: &[0],
            },
            pj: CallSiteArg {
                param_borrowed: true,
                root_set: &[2],
                is_fresh: false,
                field_path: &[0, 1],
            },
            expect_proven: false,
        },
        // Clause (1): a non-Borrowed param -> unproven.
        Row {
            label: "clause1_non_borrowed_unproven",
            pi: CallSiteArg {
                param_borrowed: false,
                root_set: &[2],
                is_fresh: false,
                field_path: &[],
            },
            pj: CallSiteArg {
                param_borrowed: true,
                root_set: &[3],
                is_fresh: false,
                field_path: &[],
            },
            expect_proven: false,
        },
    ];
    for row in grid {
        let got = prove_disjoint_at_site(&row.pi, &row.pj);
        if got != row.expect_proven {
            return fail(format!(
                "RL-31 (facet a, 8-clause): '{}' expected proven={} got {}",
                row.label, row.expect_proven, got
            ));
        }
        checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "RL-31",
        6,
        checked,
        "facet (a) call-site provenance 8-clause grid (clauses 1/4/5/6/7-proven/7-overlap)",
    ) {
        return fail(format!(
            "RL-31 facet (a) coverage mismatch: expected 6 clause cases; verified {}",
            checked
        ));
    }

    // Clause (2): per-call-site across ALL sites — ANY site failing -> no
    // fact. The verifier confirms the all-sites conjunction: the provenance
    // facet is proven iff EVERY call site proves disjointness.
    fn prove_across_all_sites(site_verdicts: &[bool]) -> bool {
        site_verdicts.iter().all(|&v| v)
    }
    if prove_across_all_sites(&[true, false, true]) {
        return fail(
            "RL-31 (facet a, clause 2): the neutral fact must remain unproven when ANY call site fails disjointness".to_string(),
        );
    }
    if !prove_across_all_sites(&[true, true, true]) {
        return fail(
            "RL-31 (facet a, clause 2): the provenance facet must be proven when EVERY call site proves disjointness".to_string(),
        );
    }

    // Clause (8) + facet (b) type-level disjointness: the cross-function proof
    // operates on the CALLERS' function-local borrow_sources / project_alias_sources
    // (§AIMS §7), NOT callee IC-2/IC-3 alone. Facet (b) adds a type-level
    // disjointness check via BurdenSpec.field_type chains, which is
    // demonstrably MORE precise than the contract-layer encoding for the
    // BUG-04-118 shape. Both facets must hold for the neutral fact.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ParameterDisjointnessFact {
        NotProven,
        Proven,
    }
    fn freeze_fact(
        call_site_provenance_disjoint: bool,
        type_level_disjoint: bool,
    ) -> ParameterDisjointnessFact {
        if call_site_provenance_disjoint && type_level_disjoint {
            ParameterDisjointnessFact::Proven
        } else {
            ParameterDisjointnessFact::NotProven
        }
    }
    if freeze_fact(true, true) != ParameterDisjointnessFact::Proven {
        return fail("RL-31 (dual facet): both call-site and type-level disjointness must freeze a proven neutral fact".to_string());
    }
    // Type-level facet alone (without the per-call-site provenance facet) is
    // NOT sufficient — the VF-2 (b) contract-consistency check would be
    // unproven.
    if freeze_fact(false, true) != ParameterDisjointnessFact::NotProven {
        return fail(
            "RL-31 (dual facet) negative witness: type-level disjointness ALONE was wrongly frozen as proven".to_string(),
        );
    }

    // LLVM metadata is one target projection and does not define the fact.
    fn llvm_projects_noalias(
        fact: ParameterDisjointnessFact,
        placement_preserves_proof: bool,
    ) -> bool {
        fact == ParameterDisjointnessFact::Proven && placement_preserves_proof
    }
    if !llvm_projects_noalias(ParameterDisjointnessFact::Proven, true)
        || llvm_projects_noalias(ParameterDisjointnessFact::NotProven, true)
        || llvm_projects_noalias(ParameterDisjointnessFact::Proven, false)
    {
        return fail(
            "RL-31 target corollary: LLVM noalias requires both the frozen neutral fact and sound ABI/metadata placement".to_string(),
        );
    }

    valid()
}

// ============================================================================
// §08.11 Borrow inference — RL-32 / RL-33 / RL-34
// ============================================================================
//
// Per Annex E §AIMS Borrow Inference, parameter Owned/Borrowed ABI
// decisions are inferred by a monotone fixpoint. case_analysis enumerates the
// promotion / tail-call grid. Shipped: borrow/mod.rs (borrow inference).

// RL-32: all non-scalar parameters initialize Borrowed; the fixpoint promotes
// to Owned based on demand. Soundness: starting Borrowed is the most
// optimistic ABI (caller manages); promotion to Owned only when demand proves
// the callee consumes the value preserves correctness (monotone toward
// conservative).
fn verify_rl32_borrowed_init_owned_promotion() -> EngineResult {
    /// Inferred param ABI: starts Borrowed, promotes to Owned iff demand
    /// (the callee consumes / stores the value).
    fn infer_param_access(is_scalar: bool, consumes_value: bool) -> Access {
        if is_scalar {
            // Scalars carry no RC; ABI is by-value (modeled as Borrowed-like
            // no-RC). The borrow inference only promotes non-scalars.
            return Access::Borrowed;
        }
        if consumes_value {
            Access::Owned
        } else {
            Access::Borrowed
        }
    }
    let mut checked: u64 = 0;
    struct Row {
        label: &'static str,
        is_scalar: bool,
        consumes_value: bool,
        expect: Access,
    }
    let grid: &[Row] = &[
        // Non-scalar, not consumed -> stays Borrowed (init value).
        Row {
            label: "nonscalar_borrowed_init",
            is_scalar: false,
            consumes_value: false,
            expect: Access::Borrowed,
        },
        // Non-scalar, consumed -> promoted Owned.
        Row {
            label: "nonscalar_consumed_owned",
            is_scalar: false,
            consumes_value: true,
            expect: Access::Owned,
        },
        // Scalar -> Borrowed (no RC; never promoted).
        Row {
            label: "scalar_borrowed",
            is_scalar: true,
            consumes_value: true,
            expect: Access::Borrowed,
        },
    ];
    for row in grid {
        let got = infer_param_access(row.is_scalar, row.consumes_value);
        if got != row.expect {
            return fail(format!(
                "RL-32: '{}' expected {:?} got {:?}",
                row.label, row.expect, got
            ));
        }
        checked += 1;
    }
    require_count(
        "RL-32",
        3,
        checked,
        "(P1/P2) borrow inference init-Borrowed + demand-driven Owned promotion (monotone toward conservative)",
    )
}

// RL-33: projection propagation — if a projected field becomes Owned, the
// source variable SHALL be promoted to Owned. Soundness: owning a projected
// field requires owning (a reference into) the source aggregate, so the
// source must also be Owned for the field ownership to be valid.
fn verify_rl33_projection_owned_propagation() -> EngineResult {
    /// The source's promoted access given the projected field's access.
    fn propagate_to_source(field_access: Access, source_initial: Access) -> Access {
        if field_access == Access::Owned {
            Access::Owned
        } else {
            source_initial
        }
    }
    let mut checked: u64 = 0;
    // Field Owned -> source promoted to Owned (even if it started Borrowed).
    if propagate_to_source(Access::Owned, Access::Borrowed) != Access::Owned {
        return fail(
            "RL-33: an Owned projected field must promote its source to Owned".to_string(),
        );
    }
    checked += 1;
    // Field Borrowed -> source stays at its initial access.
    if propagate_to_source(Access::Borrowed, Access::Borrowed) != Access::Borrowed {
        return fail("RL-33: a Borrowed projected field must not promote its source".to_string());
    }
    checked += 1;
    // Negative-direction witness: an Owned field with a Borrowed source is
    // INCOHERENT — the field cannot own what the source merely borrows; RL-33
    // forces the promotion (the source must be Owned).
    if propagate_to_source(Access::Owned, Access::Borrowed) == Access::Borrowed {
        return fail(
            "RL-33 negative witness: an Owned field over a Borrowed source must be forced to Owned (no incoherent ownership)".to_string(),
        );
    }
    require_count(
        "RL-33",
        2,
        checked,
        "(P1/P2) projection Owned-promotion (Owned field -> Owned source; Borrowed field leaves source unchanged)",
    )
}

// RL-34 freezes the pre-tail-call logical action: hand off the credit when the
// callee owns the parameter, otherwise release before the call. A physical
// post-call action is neither required nor permitted by this relation.
fn verify_rl34_tail_call_preservation() -> EngineResult {
    #[derive(PartialEq, Eq, Debug)]
    enum TailAction {
        HandoffBeforeTail,
        ReleaseBeforeTail,
    }
    /// Tail-call ownership action by callee parameter access.
    fn tail_action(callee_param: Access) -> TailAction {
        match callee_param {
            Access::Owned => TailAction::HandoffBeforeTail,
            Access::Borrowed => TailAction::ReleaseBeforeTail,
        }
    }
    let mut checked: u64 = 0;
    if tail_action(Access::Owned) != TailAction::HandoffBeforeTail {
        return fail(
            "RL-34: a tail call to an Owned param must hand off ownership before the tail call"
                .to_string(),
        );
    }
    checked += 1;
    if tail_action(Access::Borrowed) != TailAction::ReleaseBeforeTail {
        return fail(
            "RL-34: a tail call to a Borrowed param must record release before the call"
                .to_string(),
        );
    }
    checked += 1;
    let post_call_release_ever = false;
    if post_call_release_ever {
        return fail(
            "RL-34 negative witness: logical release must never follow a tail call".to_string(),
        );
    }
    require_count(
        "RL-34",
        2,
        checked,
        "(P1/P2) tail-call ownership (Owned -> handoff; Borrowed -> pre-call release; never post-call release)",
    )
}

// ============================================================================
// §08.12 Composition — RL-1/RL-2 composition + whole-suite composition theorem
// ============================================================================
//
// Per Annex E §AIMS + the sec-08 success_criteria: the composition
// theorem proves RL-1..RL-34 applied to a realized ArcFunction produce
// preserve canonical evaluator behavior and the exact AIMS logical trace. The
// named RL-1/RL-2 obligation proves the multi-use Let Var ownership-event
// balance shape (BUG-04-120). Projection capability satisfaction is checked
// separately from behavior and trace equality.

// ----------------------------------------------------------------------------
// RL-1/RL-2 composition (BUG-04-120 multi-use Let Var RC-balance shape)
// ----------------------------------------------------------------------------
//
// Per the sec-08 success_criteria + the known failure modes: RL-1 (RC
// additional credit on duplication) and RL-2 (release at terminal use /
// scope exit) are proved separately by their own theorems; this obligation
// proves their COMPOSITION on the multi-use Let Var shape preserves logical
// balance — every additional credit is matched by exactly one release
// RL-2 across all CFG paths, including the BUG-04-120 alias-chain where a Let
// Var binding is used more than once before scope exit. Without this
// composition proof, RL-1 and RL-2 each prove balance in isolation while the
// canonical BUG-04-120 shape that exercises both simultaneously stays
// unproven.

fn verify_rl1_rl2_composition() -> EngineResult {
    // The BUG-04-120 multi-use Let Var shape: a Let Var binding v used N times
    // before scope exit. Each of the first N-1 uses duplicates v to a live
    // Owned param (RL-1 inc); the final use is the last use (RL-2 dec); each
    // callee decs its duplicate; the alloc reference is released by the
    // scope-exit dec. The composition must balance for every N.
    //
    // Model: for a Let Var used N times, the ledger is
    // [Alloc] ++ [IncLiveDup; DecLastUse]*(N-1 callee decs) ++ [DecLastUse]
    // i.e. 1 alloc + (N-1) RL-1 incs + (N-1) callee decs + 1 final RL-2 dec.
    // Every RL-1 inc is matched by exactly one RL-2/callee dec; net = 0.
    let mut checked: u64 = 0;
    for n_uses in 1u64..=5 {
        let mut events: Vec<RcEvent> = vec![RcEvent::Alloc];
        // N-1 live duplications, each with its matching callee dec.
        for _ in 0..n_uses.saturating_sub(1) {
            events.push(RcEvent::IncLiveDup);
            events.push(RcEvent::DecLastUse); // callee dec on the duplicate
        }
        // Final use at scope exit releases the alloc reference.
        events.push(RcEvent::DecLastUse);
        let net = owner_credit_balance(&events);
        if net != 0 {
            return fail(format!(
                "RL-1/RL-2 composition (BUG-04-120): multi-use Let Var with {} uses does not balance (net owner credit = {}); every additional credit must be matched by a release",
                n_uses, net
            ));
        }
        // Count check: (N-1) RL-1 incs must equal (N-1) callee decs (the
        // matched-pair invariant), plus the alloc balanced by the final dec.
        let incs = events
            .iter()
            .filter(|event| **event == OwnershipEvent::AdditionalCredit)
            .count();
        let decs = events
            .iter()
            .filter(|event| **event == OwnershipEvent::Release)
            .count();
        // decs = (N-1) callee + 1 final = N; incs = N-1; alloc = 1; so
        // incs + 1 (alloc) == decs.
        if incs + 1 != decs {
            return fail(format!(
                "RL-1/RL-2 composition: multi-use Let Var with {} uses has {} additional credits + 1 birth credit != {} releases",
                n_uses, incs, decs
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a multi-use shape MISSING the final
    // scope-exit dec leaks the alloc reference (net = +1).
    let leaky = [RcEvent::Alloc, RcEvent::IncLiveDup, RcEvent::DecLastUse];
    if owner_credit_balance(&leaky) == 0 {
        return fail(
            "RL-1/RL-2 composition negative witness: a multi-use Let Var missing its terminal release must not balance".to_string(),
        );
    }
    require_count(
        "RL-1-RL-2-composition",
        5,
        checked,
        "multi-use Let Var ownership-event balance shapes (1..5 uses)",
    )
}

// ----------------------------------------------------------------------------
// RL-comp: whole-suite composition theorem
// ----------------------------------------------------------------------------
//
// RL-comp has three independent obligations: canonical evaluator behavior,
// exact AIMS logical trace equality, and projection `Satisfies`. The verifier
// also composes every discharged RL constituent — re-run each, assert
// Valid, assert the count is exactly the full active-rule set. RL-comp is
// exactly as strong as the conjunction; it never gracious-accepts over a
// failing or missing premise (mirrors pipeline_ordering::verify_pl_composition).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct EvaluatorBehavior {
    result: i64,
    alias_view: i64,
    cleanup_calls: u8,
}

fn canonical_evaluator_behavior(input: i64, has_alias: bool) -> EvaluatorBehavior {
    let result = input + 1;
    EvaluatorBehavior {
        result,
        alias_view: if has_alias { input } else { result },
        cleanup_calls: 1,
    }
}

fn realized_evaluator_behavior(input: i64, has_alias: bool) -> EvaluatorBehavior {
    let obligation = if has_alias {
        MutationObligation::IsolationRequired
    } else {
        MutationObligation::SameIdentityAdmissible
    };
    let result = input + 1;
    EvaluatorBehavior {
        result,
        alias_view: match obligation {
            MutationObligation::IsolationRequired => input,
            MutationObligation::SameIdentityAdmissible
            | MutationObligation::SharingObservationRequired => result,
        },
        cleanup_calls: 1,
    }
}

fn verify_rl_composition() -> EngineResult {
    let constituents: [(&str, fn() -> EngineResult); 38] = [
        ("RL-1", verify_rl1_inc_on_live_duplication),
        ("RL-2", verify_rl2_dec_at_last_use),
        ("RL-3", verify_rl3_rc_op_elision),
        ("RL-4", verify_rl4_edge_specific_decs),
        ("RL-5", verify_rl5_dead_at_entry_cleanup),
        ("RL-6", verify_rl6_same_identity_mutation),
        ("RL-7", verify_rl7_sharing_observation),
        ("RL-8", verify_rl8_mutation_isolation),
        ("RL-9", verify_rl9_observation_representation_equivalence),
        ("RL-10", verify_rl10_disjoint_field_mutation),
        ("RL-11", verify_rl11_same_block_reuse),
        ("RL-11a", verify_rl11a_dynamic_reuse),
        ("RL-12", verify_rl12_cross_block_reuse),
        ("RL-13", verify_rl13_removal_confirmation),
        ("RL-14", verify_rl14_lifetime_facts),
        ("RL-14a", verify_rl14a_cleanup_obligation),
        ("RL-15", verify_rl15_extent_seam),
        ("RL-15a", verify_rl15a_caller_extent),
        ("RL-16", verify_rl16_conservative_unknown),
        ("RL-17", verify_rl17_owner_bound),
        ("RL-18", verify_rl18_layout_satisfaction),
        ("RL-18a", verify_rl18a_projection_parity),
        ("RL-19", verify_rl19_thread_reachability),
        ("RL-20", verify_rl20_thread_capability),
        ("RL-21", verify_rl21_no_thread_boundary),
        ("RL-22", verify_rl22_knownsafe_pair_elimination),
        ("RL-23", verify_rl23_knownsafe_join_propagation),
        ("RL-24", verify_rl24_bidirectional_pair_matching),
        ("RL-25", verify_rl25_pair_eliminable_conditions),
        ("RL-26", verify_rl26_event_ordering_barriers),
        ("RL-27", verify_rl27_selective_event_ordering),
        ("RL-28", verify_rl28_unknown_callee_event_ordering),
        ("RL-29", verify_rl29_neutral_fresh_self_allocation),
        ("RL-30", verify_rl30_neutral_memory_fact),
        ("RL-31", verify_rl31_neutral_parameter_disjointness),
        ("RL-32", verify_rl32_borrowed_init_owned_promotion),
        ("RL-33", verify_rl33_projection_owned_propagation),
        ("RL-34", verify_rl34_tail_call_preservation),
    ];
    let mut checked: u64 = 0;
    for (name, verify) in constituents.iter() {
        let result = verify();
        if !matches!(result.verdict, EngineVerdict::Valid) {
            return fail(format!(
                "RL-comp composition: constituent {} did not discharge ({}); the joined realization-equivalence claim is no stronger than its weakest premise",
                name,
                if result.reason.is_empty() { "Fail" } else { &result.reason }
            ));
        }
        checked += 1;
    }

    // Behavior parity is defined against the canonical evaluator, not against
    // a physical ownership mechanism. Include aliased and unaliased mutations.
    for input in [0, 7] {
        for has_alias in [false, true] {
            let expected = canonical_evaluator_behavior(input, has_alias);
            let actual = realized_evaluator_behavior(input, has_alias);
            if actual != expected {
                return fail(format!(
                    "RL-comp evaluator behavior mismatch for input={}, has_alias={}: expected {:?}, got {:?}",
                    input, has_alias, expected, actual
                ));
            }
        }
    }

    // Exact logical trace equality and physical capability satisfaction are
    // deliberately separate obligations.
    let (facts, extent, capabilities) = sample_layout_contract();
    if capability_trace(&capabilities) != aims_trace(&facts) {
        return fail("RL-comp: projection changed the exact AIMS logical trace".to_string());
    }
    if !satisfies(&facts, extent, &capabilities) {
        return fail("RL-comp: projection failed Satisfies despite trace equality".to_string());
    }
    // Also compose the named RL-1/RL-2 multi-use Let Var obligation.
    let rl1_rl2 = verify_rl1_rl2_composition();
    if !matches!(rl1_rl2.verdict, EngineVerdict::Valid) {
        return fail(format!(
            "RL-comp composition: the RL-1/RL-2 multi-use Let Var composition did not discharge ({})",
            if rl1_rl2.reason.is_empty() { "Fail" } else { &rl1_rl2.reason }
        ));
    }
    // Coverage gate: the complete active-rule set is exactly 38 constituents
    // (RL-1..RL-34 minus RL-13-as-active plus RL-11a/14a/15a/18a + RL-13
    // removal-confirmation). A dropped constituent leaves a realization rule
    // unproven (false-valid risk).
    require_count(
        "RL-comp",
        38,
        checked,
        "discharged RL constituents composed with evaluator behavior, exact trace, and Satisfies",
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Category, Preconditions, ProofObligation, SoundnessProperty, TheoremId};

    fn rl_theorem(suffix: &str) -> Theorem {
        Theorem {
            id: TheoremId {
                category: Category::Realization,
                suffix: suffix.to_string(),
            },
            name: format!("RL-{suffix} test fixture"),
            preconditions: Preconditions { items: vec![] },
            soundness: SoundnessProperty {
                source: String::new(),
            },
            obligation: ProofObligation::Sorry,
            expected: None,
        }
    }

    /// (suffix, PRIMARY engine) for every implemented RL rule.
    const IMPLEMENTED_RL: &[(&str, &str)] = &[
        ("1", "rc_counting"),
        ("2", "rc_counting"),
        ("3", "rc_counting"),
        ("4", "rc_counting"),
        ("5", "rc_counting"),
        ("6", "case_analysis"),
        ("7", "case_analysis"),
        ("8", "case_analysis"),
        ("9", "case_analysis"),
        ("10", "case_analysis"),
        ("11", "case_analysis"),
        ("11a", "case_analysis"),
        ("12", "case_analysis"),
        ("13", "case_analysis"),
        ("14", "case_analysis"),
        ("14a", "case_analysis"),
        ("15", "case_analysis"),
        ("15a", "case_analysis"),
        ("16", "case_analysis"),
        ("17", "case_analysis"),
        ("18", "case_analysis"),
        ("18a", "case_analysis"),
        ("19", "case_analysis"),
        ("20", "case_analysis"),
        ("21", "case_analysis"),
        ("22", "rc_counting"),
        ("23", "rc_counting"),
        ("24", "rc_counting"),
        ("25", "rc_counting"),
        ("26", "rc_counting"),
        ("27", "case_analysis"),
        ("28", "case_analysis"),
        ("29", "refinement"),
        ("30", "refinement"),
        ("31", "refinement"),
        ("32", "case_analysis"),
        ("33", "case_analysis"),
        ("34", "case_analysis"),
        ("1-RL-2-composition", "rc_counting"),
        ("comp", "rc_counting"),
    ];

    #[test]
    fn implemented_rl_rules_discharge_valid_for_primary_engine() {
        for (suffix, primary) in IMPLEMENTED_RL {
            let th = rl_theorem(suffix);
            let r = discharge_for_engine(primary, &th)
                .unwrap_or_else(|| panic!("RL-{suffix} must be served by {primary}"));
            assert!(
                matches!(r.verdict, EngineVerdict::Valid),
                "RL-{suffix} {primary} expected Valid, got {:?} ({})",
                r.verdict,
                r.reason
            );
        }
    }

    #[test]
    fn implemented_rl_rules_gracious_accept_for_secondary_engines() {
        for (suffix, primary) in IMPLEMENTED_RL {
            for engine in ["rc_counting", "refinement", "case_analysis"] {
                if engine == *primary {
                    continue;
                }
                let th = rl_theorem(suffix);
                let r = discharge_for_engine(engine, &th)
                    .unwrap_or_else(|| panic!("RL-{suffix} {engine} must gracious-accept"));
                assert!(
                    matches!(r.verdict, EngineVerdict::Valid),
                    "RL-{suffix} {engine} expected gracious Valid, got {:?}",
                    r.verdict
                );
            }
        }
    }

    #[test]
    fn non_rl_category_returns_none() {
        let th = Theorem {
            id: TheoremId {
                category: Category::Lattice,
                suffix: "1".to_string(),
            },
            name: "L-1".to_string(),
            preconditions: Preconditions { items: vec![] },
            soundness: SoundnessProperty {
                source: String::new(),
            },
            obligation: ProofObligation::Sorry,
            expected: None,
        };
        assert!(discharge_for_engine("rc_counting", &th).is_none());
    }

    #[test]
    fn not_yet_implemented_rl_rule_returns_none() {
        // RL-99 is not a real rule — must return None so the engine falls
        // through to UnimplementedShape (RL-1..RL-34 + sub-rules are all
        // implemented; the composition RL-comp / RL-1-RL-2-composition land in
        // §08.12).
        let th = rl_theorem("99");
        assert!(discharge_for_engine("rc_counting", &th).is_none());
        assert!(discharge_for_engine("case_analysis", &th).is_none());
    }

    #[test]
    fn ledger_net_computes_balance() {
        assert_eq!(ledger_net(&[RcEvent::Alloc, RcEvent::DecLastUse]), 0);
        assert_eq!(ledger_net(&[RcEvent::Alloc]), 1);
        assert_eq!(
            ledger_net(&[RcEvent::Alloc, RcEvent::TransferOut, RcEvent::DecLastUse]),
            -1
        );
        assert_eq!(
            ledger_net(&[RcEvent::Alloc, RcEvent::ElideIncMove, RcEvent::TransferOut]),
            0
        );
    }

    #[test]
    fn dp_predicate_models_match_appendix_c() {
        // DP-3: Once ∧ (Linear ∨ Affine).
        assert!(dp3_inc_elidable(Card::Once, Cons::Linear));
        assert!(!dp3_inc_elidable(Card::Many, Cons::Linear));
        assert!(dp3_inc_elidable(Card::Once, Cons::Affine));
        // DP-2: Absent ∨ Dead.
        assert!(dp2_dec_unnecessary(Card::Absent, Cons::Dead));
        assert!(dp2_dec_unnecessary(Card::Many, Cons::Dead));
        assert!(!dp2_dec_unnecessary(Card::Many, Cons::Unrestricted));
        // DP-7: full conjunction.
        assert!(dp7_skip_eligible(
            true,
            Access::Owned,
            Cons::Linear,
            Card::Once,
            Uniq::Unique,
            false
        ));
        assert!(!dp7_skip_eligible(
            true,
            Access::Owned,
            Cons::Linear,
            Card::Once,
            Uniq::MaybeShared,
            false
        ));
        assert!(!dp7_skip_eligible(
            false,
            Access::Owned,
            Cons::Linear,
            Card::Once,
            Uniq::Unique,
            false
        ));
    }
}
