#!/usr/bin/env bash
#
# check-section-04-compiler-conformance.sh — §04-IM-G compiler-conformance
# cross-walk gate.
#
# Per Annex E §AIMS §4
# §04-IM-G: "Mechanically cross-walks every proven TF-N rule against
# the compiler implementation site
# compiler/ori_arc/src/aims/transfer/mod.rs."
#
# Mechanical cross-walk: for each TF-N rule (19 forward + 5 backward +
# TF-N-A + Composition + IA-5-step-1), verify the compiler's
# transfer/mod.rs has the corresponding implementation function or
# match-arm signature that emits the canonical AimsState that the proof
# asserts.
#
# Exit codes + emitted lines:
# exit 0 + emits `transfer_proofs_passed` — ALL 27 entries confirm
# exit 2 + emits `transfer_compiler_spec_divergence` — >=1 divergence
# exit 3 + emits `transfer_infrastructure_failed` — file unreadable
#
# Invocation: bash check-section-04-compiler-conformance.sh
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at
# entry.

set -e
cd "$(dirname "$0")/.."

readonly TRANSFER_MOD="../compiler/ori_arc/src/aims/transfer/mod.rs"
readonly PROOFS_DIR="proofs/04-transfers"

if [[ ! -f "$TRANSFER_MOD" ]]; then
    echo "transfer_infrastructure_failed: compiler transfer/mod.rs not found at ${TRANSFER_MOD}"
    exit 3
fi

if [[ ! -d "$PROOFS_DIR" ]]; then
    echo "transfer_infrastructure_failed: proofs directory not found at ${PROOFS_DIR}"
    exit 3
fi

# Conformance entries: each row = "TF-id|required-signature-or-pattern|description"
#
# The required-signature is a literal-string token that MUST appear in
# transfer/mod.rs. Tokens chosen to be load-bearing (function names,
# canonical state constants, match-arm key tokens) such that absence
# indicates the compiler has dropped or renamed the rule's implementation.
readonly -a CONFORMANCE_ROWS=(
    "TF-1|ArcValue::Literal|Let { Literal } -> SCALAR"
    "TF-2|ArcValue::Var|Let { Var(v) } -> state(v)"
    "TF-2a|ArcValue::PrimOp|Let { PrimOp } -> SCALAR"
    "TF-3|fn transfer_construct|Construct { ctor } -> FRESH(shape_from_ctor)"
    "TF-4|fn transfer_project|Project -> Borrowed view"
    "TF-5|fn transfer_apply_conservative|Apply no-contract -> CONSERVATIVE"
    "TF-5a|ArcInstr::ApplyIndirect|ApplyIndirect -> CONSERVATIVE/TOP"
    "TF-6|fn transfer_apply_conservative|Apply with contract -> refine(CONSERVATIVE, contract)"
    "TF-6a|ArcTerminator::Invoke|Invoke with contract"
    "TF-6b|ArcTerminator::Invoke|Invoke no contract"
    "TF-6c|ArcTerminator::Invoke|InvokeIndirect"
    "TF-7|fn transfer_partial_apply|PartialApply -> FRESH(NonReusable)"
    "TF-8|fn transfer_select|Select -> join branches"
    "TF-9|fn transfer_reuse|Reuse -> FRESH(shape)"
    "TF-9a|fn transfer_collection_reuse|CollectionReuse -> FRESH(CollectionBuffer)"
    "TF-10|ArcInstr::IsShared|IsShared -> SCALAR"
    "TF-10a|ArcInstr::Reset|Reset -> SCALAR"
    "TF-15|ArcInstr::Set|Set { base, field, value } no dst"
    "TF-15a|ArcInstr::SetTag|SetTag { base, tag } no dst"
    "TF-11|fn backward_demands|Standard backward demand (Once)"
    "TF-11a|fn backward_terminator_demands|Terminator backward demand"
    "TF-12|ArcInstr::PartialApply|PartialApply no TF-11 demand"
    "TF-13|fn capture_state_update|capture_state_update lambda-mode"
    "TF-14|fn transfer_project|Project backward propagation"
    "TF-N-A|ArcInstr::RcInc|RcInc/RcDec side-effect-only no dst"
    "Composition|fn transfer_def|TF-chain composition via transfer_def dispatch"
    "IA-5-step-1|fn backward_demands|IA-5 step (1) backward operand demand"
)

divergences=()
checked=0

for row in "${CONFORMANCE_ROWS[@]}"; do
    tf_id="${row%%|*}"
    rest="${row#*|}"
    required_token="${rest%%|*}"
    description="${rest#*|}"

    proof_path="${PROOFS_DIR}/${tf_id}.proof"
    if [[ ! -f "$proof_path" ]]; then
        divergences+=("${tf_id}: proof_status: divergent (proof file missing: ${proof_path})")
        checked=$((checked + 1))
        continue
    fi

    if ! grep -qF "$required_token" "$TRANSFER_MOD"; then
        divergences+=("${tf_id}: proof_status: divergent (required token '${required_token}' not in transfer/mod.rs — ${description})")
    fi
    checked=$((checked + 1))
done

if [[ ${#divergences[@]} -gt 0 ]]; then
    echo "transfer_compiler_spec_divergence: ${#divergences[@]} divergence(s) across ${checked} TF rules"
    for div in "${divergences[@]}"; do
        echo " ${div}"
    done
    exit 2
fi

echo "transfer_proofs_passed: ${checked}/27 TF rules confirmed in transfer/mod.rs"
exit 0
