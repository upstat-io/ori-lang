#!/usr/bin/env python3
"""Generate proof-lean-map.json from the .proof corpus + AimsProof Lean modules.

Emits one row per .proof under aims-proof/proofs/. Row shape:
  {proof_id, proof_path, kind, lean_module, lean_theorem, parity_tokens, note}

kind values:
  theorem            dischargeable rule theorem; lean_theorem is the PRIMARY
                     Lean decl stating the rule's core obligation; parity_tokens
                     are carrier/predicate identifiers the Lean statement must
                     mention. Parity-enforced by lean-statement-parity.sh.
  removed            rule removed for unsoundness; soundness-of-removal .proof;
                     lean_theorem null; lean_encoding cites the removal note.
  doc_encoded        meta/model-level obligation faithfully encoded as a Lean
                     doc-comment section (L-9 model invariant, L-10 schema),
                     not a standalone theorem; lean_encoding cites the section.
  composition_folded composition/aux obligation discharged by a folding Lean
                     theorem rather than a 1:1 named theorem; lean_theorem cites
                     the folding decl; parity-enforced on that decl.
  definition         definition_only .proof (formal-spec SSOT; check-proofs.sh
                     SKIPs); no Lean theorem.
  foundation         §01 signature/definition skeleton (unimplemented_engine_shape);
                     no rule Lean theorem.
  coverage_shape     §00 engine-coverage smoke shape; no rule Lean theorem.
  bootstrap          §01A checker-bootstrap engine/kernel proof (KERNEL trust set);
                     no rule Lean theorem.

Run: python3 aims-proof/scripts/gen-proof-lean-map.py > aims-proof/scripts/proof-lean-map.json
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
AIMS_PROOF = SCRIPT_DIR.parent
PROOFS = AIMS_PROOF / "proofs"
LEAN = AIMS_PROOF / "lean" / "AimsProof"

THEOREM_HEADER_RE = re.compile(r"^\s*[Tt]heorem\s+([A-Za-z0-9][A-Za-z0-9.\- +]*?)\s*:")
STATUS_RE = re.compile(r"^\s*status:\s*(\S+)\s*$")

# Primary Lean theorem + parity carrier/predicate tokens per dischargeable rule.
# parity_tokens: identifiers the Lean theorem STATEMENT must mention (carrier
# types + the operator/predicate the .proof obligation names). NOT vacuous True.
THEOREM_MAP: dict[str, tuple[str, str, list[str]]] = {
    # L — Lattice.lean
    "L-1": ("Lattice", "L1_join_comm", ["AimsState", "join"]),
    "L-2": ("Lattice", "L2_rawJoin_assoc", ["AimsState", "rawJoin"]),
    "L-3": ("Lattice", "L3_join_idem_of_canonical", ["AimsState", "join"]),
    "L-4": ("Lattice", "L4_le_refl", ["AimsState", "le"]),
    "L-5": ("Lattice", "L5_finite_height", ["height"]),
    "L-7": ("Lattice", "L7_canonicalize_idem", ["AimsState", "canonicalize"]),
    "L-8": ("Lattice", "L8_canonicalize_preserves_join", ["AimsState", "canonicalize", "join"]),
    # L-6 lives in Transfer.lean (the per-TF concrete-monotonicity closure).
    "L-6": ("Transfer", "L6_const_monotone", ["AimsState", "le"]),
    # CN — Canonicalization.lean
    "CN-1": ("Canonicalization", "CN1_dead_absent", ["consumption", "cardinality"]),
    "CN-3": ("Canonicalization", "CN3_shared_blocks_reuse", ["Uniqueness", "Shape"]),
    "CN-6": ("Canonicalization", "CN6_wide_locality_uniqueness_ceiling", ["Locality", "Uniqueness"]),
    "CN-8": ("Canonicalization", "CN8_borrowed_locality_ceiling", ["Borrowed", "Locality"]),
    # TF — Transfer.lean
    "TF-3": ("Transfer", "TF3_construct_fresh", ["Construct", "FRESH"]),
    "TF-4": ("Transfer", "TF4_project_monotone", ["AimsState", "le"]),
    "TF-5": ("Transfer", "TF5_apply_no_contract_conservative", ["CONSERVATIVE"]),
    "TF-6": ("Transfer", "TF6_refine_narrows", ["refine"]),
    "TF-7": ("Transfer", "TF7_partial_apply_fresh", ["FRESH"]),
    "TF-8": ("Transfer", "TF8_uniqueness_downgrade_table", ["Uniqueness", "MaybeShared"]),
    "TF-9": ("Transfer", "TF9_reuse_fresh", ["FRESH"]),
    "TF-9a": ("Transfer", "TF9a_collection_reuse_fresh", ["FRESH", "CollectionBuffer"]),
    "TF-11": ("Transfer", "TF11_seq_add_consumption_comm", ["seq_add", "Consumption"]),
    "TF-13": ("Transfer", "TF13_capture_state_update_monotone", ["AimsState", "le"]),
    "TF-14": ("Transfer", "TF14_propagation_spec", ["cardinality", "locality"]),
    # DP — Decision.lean
    "DP-1": ("Decision", "DP1_is_rc_needed_table", ["is_rc_needed", "Owned"]),
    "DP-2": ("Decision", "DP2_is_rc_dec_unnecessary_table", ["is_rc_dec_unnecessary"]),
    "DP-3": ("Decision", "DP3_is_rc_inc_elidable_table", ["is_rc_inc_elidable"]),
    "DP-4": ("Decision", "DP4_needs_cow_check_table", ["needs_cow_check", "MaybeShared"]),
    "DP-5": ("Decision", "DP5_can_mutate_in_place_table", ["can_mutate_in_place"]),
    "DP-6": ("Decision", "DP6_is_reuse_candidate_table", ["is_reuse_candidate"]),
    "DP-7": ("Decision", "DP7_is_rc_skip_eligible_table", ["is_rc_skip_eligible"]),
    "DP-8": ("Decision", "DP8_is_local_table", ["is_local", "Locality"]),
    "DP-9": ("Decision", "DP9_cow_mode_table", ["cow_mode"]),
    # IC — Interprocedural.lean
    "IC-1": ("Interprocedural", "IC1_callee_processed_before_caller", ["callee", "caller"]),
    "IC-2": ("Interprocedural", "IC2_optimistic_is_bottom", ["ParamContract"]),
    "IC-3": ("Interprocedural", "IC3_paramcontract_join_comm", ["ParamContract", "join"]),
    "IC-4": ("Interprocedural", "IC4_returncontract_join_comm", ["ReturnContract", "join"]),
    "IC-5": ("Interprocedural", "IC5_effectsummary_join_comm", ["EffectSummary", "join"]),
    "IC-6": ("Interprocedural", "IC6_fip_contract_join_lattice", ["FipContract", "join"]),
    "IC-7": ("Interprocedural", "IC7_fixpoint_converges", ["converge"]),
    "IC-8a": ("Interprocedural", "IC8a_conservative_upper_bounds", ["ParamContract"]),
    # PL — Pipeline.lean
    "PL-1": ("Pipeline", "PL1_prefix_before_suffix", ["before"]),
    "PL-1a": ("Pipeline", "PL1a_callee_visited_before_caller", ["callee", "caller"]),
    "PL-2": ("Pipeline", "PL2_analyze_before_realize", ["analyze", "realize", "before"]),
    "PL-3": ("Pipeline", "PL3_realize_before_merge", ["realize", "merge", "before"]),
    "PL-4": ("Pipeline", "PL4_annotations_after_merge", ["merge"]),
    "PL-4a": ("Pipeline", "PL4a_unwind_before_merge", ["unwind", "merge", "before"]),
    "PL-5": ("Pipeline", "PL5_no_stale_summaries", ["stale"]),
    "PL-6": ("Pipeline", "PL6_shift_strict_mono", ["mono"]),
    "PL-7": ("Pipeline", "PL7_contextbehavior_four_fields", ["ContextBehavior"]),
    "PL-8": ("Pipeline", "PL8_candidate_iff_all_three", ["candidate"]),
    "PL-9": ("Pipeline", "PL9_external_arity_preserved", ["arity"]),
    "PL-10": ("Pipeline", "PL10_all_pass_survives", ["pass"]),
    "PL-11": ("Pipeline", "PL11_contextbehavior_join_comm", ["ContextBehavior", "join"]),
    # RL — Realization.lean
    "RL-1": ("Realization", "RL1_emit_iff_not_elidable", ["elidable"]),
    "RL-2": ("Realization", "RL2_dec_at_last_use", ["dec"]),
    "RL-3": ("Realization", "RL3_elide_iff_any_predicate", ["elide"]),
    "RL-4": ("Realization", "RL4_edge_dec_decision", ["edge", "dec"]),
    "RL-5": ("Realization", "RL5_dead_at_entry_cleanup", ["dead"]),
    "RL-6": ("Realization", "RL6_static_unique_in_place", ["Unique"]),
    "RL-7": ("Realization", "RL7_dynamic_cow", ["MaybeShared"]),
    "RL-8": ("Realization", "RL8_static_shared_copy", ["Shared"]),
    "RL-9": ("Realization", "RL9_contraction_equiv", ["contract"]),
    "RL-10": ("Realization", "RL10_disjoint_field_in_place", ["field"]),
    "RL-11": ("Realization", "RL11_same_block_reuse", ["reuse"]),
    "RL-11a": ("Realization", "RL11a_dynamic_unique_arm", ["reuse", "Unique"]),
    "RL-12": ("Realization", "RL12_cross_block_reuse", ["reuse"]),
    "RL-14": ("Realization", "RL14_headerless_stack", ["stack"]),
    "RL-14a": ("Realization", "RL14a_immortal_header_stack", ["stack", "header"]),
    "RL-15": ("Realization", "RL15_bump_alloc", ["alloc"]),
    "RL-15a": ("Realization", "RL15a_cat1_borrowed_unique_headerless", ["Borrowed", "Unique"]),
    "RL-16": ("Realization", "RL16_heap_alloc", ["heap", "alloc"]),
    "RL-17": ("Realization", "RL17_local_unique_none", ["bound"]),
    "RL-18": ("Realization", "RL18_width_holds_bound", ["width", "bound"]),
    "RL-18a": ("Realization", "RL18a_strategy_agrees_escape", ["escape"]),
    "RL-19": ("Realization", "RL19_thread_local_non_atomic", ["thread", "atomic"]),
    "RL-20": ("Realization", "RL20_thread_shared_atomic", ["thread", "atomic"]),
    "RL-21": ("Realization", "RL21_no_thread_boundary_all_non_atomic", ["thread", "atomic"]),
    "RL-22": ("Realization", "RL22_eliminate_iff_known_safe", ["known", "safe"]),
    "RL-23": ("Realization", "RL23_join_all_preds", ["pred"]),
    "RL-24": ("Realization", "RL24_matched_iff_bidirectional", ["bidirectional"]),
    "RL-25": ("Realization", "RL25_eliminable_decision", ["eliminable"]),
    "RL-26": ("Realization", "RL26_barrier_blocks", ["barrier"]),
    "RL-27": ("Realization", "RL27_flush_decision", ["flush"]),
    "RL-28": ("Realization", "RL28_unknown_callee_flushes_all", ["flush"]),
    "RL-29": ("Realization", "RL29_noalias_decision", ["noalias"]),
    "RL-30": ("Realization", "RL30_pure_no_access_none", ["memory"]),
    "RL-31": ("Realization", "RL31_disjoint_borrowed_noalias", ["Borrowed", "noalias"]),
    "RL-32": ("Realization", "RL32_borrowed_default", ["Borrowed"]),
    "RL-33": ("Realization", "RL33_field_owned_promotes_source", ["Owned"]),
    "RL-34": ("Realization", "RL34_owned_transfers", ["Owned"]),
    # VF — Verification.lean
    "VF-1": ("Verification", "VF1_catches_structural", ["structural"]),
    "VF-2": ("Verification", "VF2_catches_contract", ["contract"]),
    "VF-3": ("Verification", "VF3_catches_oracle", ["oracle"]),
    "VF-4": ("Verification", "VF4_catches_fip", ["fip"]),
    "VF-5": ("Verification", "VF5_three_tier_conjunction", ["tier"]),
    "VF-6": ("Verification", "VF6_certified_requires_both", ["certified"]),
    "VF-7": ("Verification", "VF7_three_tier_conjunction", ["tier"]),
    "VF-8": ("Verification", "VF8_covered_iff_not_gap", ["gap"]),
    # RL-DROP — Realization.lean (drop-glue-gated scope-exit user @drop).
    "RL-DROP": ("Realization", "RLDROP_scalar_lifecycle_sound", ["userDrop", "scalar"]),
    # 12-provenance T-theorems — Partition / Ledger / RunningCount /
    # ContractBoundary / FieldDecomposition. Keyed by the .proof header id
    # (family prefix + T-n); emitted rows carry the bare T-n proof_id via
    # display_pid.
    "IA-T1": ("Partition", "samerep_birthsite_sound", ["birthsite", "samerep"]),
    "RL-T2": ("Ledger", "compositional_placement_sound", ["threeclauses", "ledgersafe"]),
    "RL-T3": ("RunningCount", "keep_alive_redundancy_sound_iff_whole_pair", ["runningrc", "neverbelow"]),
    "IC-T4": ("ContractBoundary", "T4_contract_boundary_composition_sound", ["boundaryinstrs", "calleeconforms"]),
    "IA-T5": ("ContractBoundary", "T5_frame_untouched_class_ledger_verbatim", ["mergeclasses", "deriveledger"]),
    "IA-T6": ("FieldDecomposition", "FD_skipset_sound", ["payloadevents", "threeclauses"]),
    "CH-comp-PV": ("ProvenanceComposition", "CHcomp_partition_side_table", ["eliminateatclass", "handshakeaccepts"]),
    # CH — Coexistence.lean
    "CH-1": ("Coexistence", "CH1_lattice_bridge", ["burden", "bridge"]),
    "CH-2": ("Coexistence", "CH2_single_elimination", ["elimination"]),
    "CH-3": ("Coexistence", "CH3_classes_disjoint", ["class"]),
    "CH-4": ("Coexistence", "CH4_state_map_immutable", ["state", "map"]),
    "CH-5": ("Coexistence", "CH5_analyze_before_realize", ["analyze", "realize", "before"]),
    # comp / composition-folded rows (kind=composition_folded; folding decl).
    "PL-comp": ("Pipeline", "PLcomp_pipeline_composes", ["compose"]),
    "VF-comp": ("Verification", "VFcomp_stack_covers_union", ["union"]),
    "CH-comp": ("Coexistence", "CHcomp_handshake_union", ["union"]),
    "TF-Composition": ("Transfer", "L6_const_monotone", ["AimsState", "le"]),
    "RL-comp": ("Realization", "RL3_elision_net_preserving", ["net"]),
    "RL-1-RL-2-composition": ("Realization", "RL1_duplication_balanced", ["balanced"]),
    "IA-5-step-1": ("Transfer", "TF14_propagation_spec", ["cardinality", "locality"]),
    "CN-Ordering": ("Canonicalization", "CN_canonicalize_idempotent", ["canonicalize"]),
    "CN-Fixpoint-OneRound": ("Canonicalization", "CN_canonicalize_idempotent", ["canonicalize"]),
}

# Folded transfer/canonicalization rules whose obligation is discharged by an
# existing Lean theorem (per the Transfer.lean / Canonicalization.lean module
# headers documenting the folding). NOT gaps; NOT stubs — faithful folding cite.
THEOREM_MAP.update({
    # CONSERVATIVE-constant forward rules — Transfer.lean header:
    # "TF-5 / TF-5a / TF-6b / TF-6c are the CONSERVATIVE constant".
    "TF-5a": ("Transfer", "TF5_apply_no_contract_conservative", ["CONSERVATIVE"]),
    "TF-6b": ("Transfer", "TF5_apply_no_contract_conservative", ["CONSERVATIVE"]),
    "TF-6c": ("Transfer", "TF5_apply_no_contract_conservative", ["CONSERVATIVE"]),
    # refine group — Transfer.lean header: "TF-6 / TF-6a are the refine".
    "TF-6a": ("Transfer", "TF6_refine_narrows", ["refine"]),
    # TF-11a terminator demand — TF-11 backward-demand family.
    "TF-11a": ("Transfer", "TF11_seq_add_consumption_comm", ["seq_add", "Consumption"]),
    # CN-2 subsumed by CN-1 (Canonicalization.lean header §CN-2 note).
    "CN-2": ("Canonicalization", "CN1_dead_absent", ["consumption", "cardinality"]),
    # CN-6 composition reconciliation lemma under the CN-6 body.
    "CN-6-Extraction-Reconciliation": ("Canonicalization", "CN6_wide_locality_uniqueness_ceiling", ["Locality", "Uniqueness"]),
    # MF-1 moved-out-fields INTERSECT-fixpoint bounded convergence — the
    # load-bearing no-change-at-derived-cap theorem (reuses IC7 abstract lemma).
    "IA-MF1": ("MovedFields", "MF1_no_change_at_derived_cap", ["iterate", "bound"]),
})

COMPOSITION_FOLDED = {
    "PL-comp", "VF-comp", "CH-comp", "TF-Composition", "RL-comp",
    "RL-1-RL-2-composition", "IA-5-step-1", "CN-Ordering", "CN-Fixpoint-OneRound",
    "TF-5a", "TF-6b", "TF-6c", "TF-6a", "TF-11a", "CN-2",
    "CN-6-Extraction-Reconciliation",
}

# SCALAR-sentinel / no-AimsState-image rules faithfully encoded as Lean
# doc-comments per the Transfer.lean / Canonicalization.lean module headers
# (the model has no SCALAR AimsState inhabitant — L-9; in-place rules emit no
# dst). NOT gaps — the absence of a forward image IS the faithful encoding.
DOC_ENCODED_TF = {
    "TF-1": ("Transfer", "SCALAR-sentinel row (L-9 model exclusion); no AimsState forward image"),
    "TF-2": ("Transfer", "identity → state(v) (Transfer.lean header); transparent alias"),
    "TF-2a": ("Transfer", "SCALAR-sentinel row (L-9 model exclusion); no AimsState forward image"),
    "TF-10": ("Transfer", "SCALAR-sentinel row (L-9 model exclusion); no AimsState forward image"),
    "TF-10a": ("Transfer", "SCALAR-sentinel row (L-9 model exclusion); no AimsState forward image"),
    "TF-12": ("Transfer", "PartialApply emits NO TF-11 demand (Transfer.lean header); no demand image"),
    "TF-15": ("Transfer", "Set in-place, no dst (Transfer.lean header); no forward image"),
    "TF-15a": ("Transfer", "SetTag in-place, no dst (Transfer.lean header); no forward image"),
    "TF-N-A": ("Transfer", "RcInc/RcDec + BurdenInc/BurdenDec side-effect-only, no dst; no forward image"),
}
CN5_DOC = ("Canonicalization", "§CN-5 — Unique+Dead preserves reusable shape (forbids a collapse, adds no theorem)")

# Genuine dual-prover-formalization coverage gaps — substantive rule obligations with NO Lean
# theorem AND not documented as folded/doc-encoded. The Lean proof for these
# rules is MISSING and must be authored. Emitting a stub theorow is BANNED.
GAP_IDS: dict[str, tuple[str, str]] = {}

# doc-comment-encoded meta/model obligations (L-9 model invariant, L-10 schema).
DOC_ENCODED = {
    "L-9": ("Lattice", "§L-9 — SCALAR sentinel exclusion (model-level invariant)"),
    "L-10": ("Lattice", "§L-10 — dimension-extension schema (meta-theorem)"),
}

# REMOVED-rule soundness proofs (lean_theorem null; removal-note encoding).
REMOVED_IDS = {
    "CN-4-REMOVED", "CN-7-REMOVED", "DP-10-REMOVED", "IC-8-REMOVED", "RL-13",
}


def proof_id(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        m = THEOREM_HEADER_RE.match(line)
        if m:
            return m.group(1).strip()
    return path.stem


def proof_status(path: Path) -> str | None:
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        m = STATUS_RE.match(line)
        if m:
            return m.group(1)
    return None


def display_pid(pid: str) -> str:
    """Emitted proof_id: family-12 T-theorem headers (IA-T1, RL-T2, ...) carry
    the bare T-n id in the checked-in rows; every other id passes through."""
    m = re.match(r"^(?:IA|RL|IC)-(T\d+)$", pid)
    return m.group(1) if m else pid


def classify(rel: str, pid: str, status: str | None) -> dict:
    row: dict = {"proof_id": display_pid(pid), "proof_path": rel}
    if status == "definition_only":
        row.update(kind="definition", lean_module=None, lean_theorem=None,
                   parity_tokens=[], note="definition_only SSOT; check-proofs.sh skips; no Lean theorem")
        return row
    if rel.startswith("proofs/00-coverage-corpus/"):
        row.update(kind="coverage_shape", lean_module=None, lean_theorem=None,
                   parity_tokens=[], note="§00 engine-coverage smoke shape; not a rule theorem")
        return row
    if rel.startswith("proofs/00-smoke-test/"):
        row.update(kind="coverage_shape", lean_module=None, lean_theorem=None,
                   parity_tokens=[], note="§00 smoke-test; not a rule theorem")
        return row
    if re.match(r"proofs/01-[a-z]", rel):
        row.update(kind="foundation", lean_module=None, lean_theorem=None,
                   parity_tokens=[], note="§01 signature/definition skeleton (unimplemented_engine_shape)")
        return row
    if rel.startswith("proofs/01A-bootstrap/"):
        row.update(kind="bootstrap", lean_module=None, lean_theorem=None,
                   parity_tokens=[], note="§01A checker-bootstrap KERNEL trust-set proof; not a rule theorem")
        return row
    if pid in REMOVED_IDS:
        row.update(kind="removed", lean_module=None, lean_theorem=None,
                   parity_tokens=[], lean_encoding="rule removed for unsoundness; removal-soundness proof",
                   note="removed rule; no standalone Lean theorem")
        return row
    if pid in DOC_ENCODED:
        mod, section = DOC_ENCODED[pid]
        row.update(kind="doc_encoded", lean_module=f"AimsProof.{mod}", lean_theorem=None,
                   parity_tokens=[], lean_encoding=section,
                   note="meta/model obligation faithfully encoded as Lean doc-comment section")
        return row
    if pid in DOC_ENCODED_TF:
        mod, section = DOC_ENCODED_TF[pid]
        row.update(kind="doc_encoded", lean_module=f"AimsProof.{mod}", lean_theorem=None,
                   parity_tokens=[], lean_encoding=section,
                   note="SCALAR/no-AimsState-image rule faithfully encoded as Lean doc-comment")
        return row
    if pid == "CN-5":
        mod, section = CN5_DOC
        row.update(kind="doc_encoded", lean_module=f"AimsProof.{mod}", lean_theorem=None,
                   parity_tokens=[], lean_encoding=section,
                   note="forbids-a-collapse rule faithfully encoded as Lean doc-comment")
        return row
    if pid in GAP_IDS:
        mod, reason = GAP_IDS[pid]
        row.update(kind="gap", lean_module=f"AimsProof.{mod}", lean_theorem=None,
                   parity_tokens=[], gap_reason=reason,
                   note="dual-prover-formalization COVERAGE GAP — Lean proof missing; must be authored (NOT a stub)")
        return row
    if pid in THEOREM_MAP:
        mod, thm, tokens = THEOREM_MAP[pid]
        kind = "composition_folded" if pid in COMPOSITION_FOLDED else "theorem"
        row.update(kind=kind, lean_module=f"AimsProof.{mod}", lean_theorem=thm,
                   parity_tokens=tokens, note="")
        return row
    raise SystemExit(
        f"UNMAPPED .proof rule id {pid!r} ({rel}) — no Lean theorem mapping. "
        f"Authoring a fake row is BANNED; this is a dual-prover-formalization coverage gap to report."
    )


def main() -> int:
    proofs = sorted(PROOFS.rglob("*.proof"))
    rows = []
    for p in proofs:
        rel = "proofs/" + str(p.relative_to(PROOFS))
        pid = proof_id(p)
        status = proof_status(p)
        rows.append(classify(rel, pid, status))
    doc = {
        "schema": "proof-lean-map/v1",
        "description": "Per-.proof to primary AimsProof Lean theorem map. SSOT for "
                       "the .proof<->Lean mapping AND the statement-parity check "
                       "(lean-statement-parity.sh). kind=theorem|composition_folded "
                       "rows are parity-enforced; other kinds carry faithful "
                       "classification, never a stub theorem.",
        "rows": rows,
    }
    json.dump(doc, sys.stdout, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
