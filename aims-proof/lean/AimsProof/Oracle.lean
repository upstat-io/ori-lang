/-
AIMS VF-3 realized-evidence oracle model. This module connects realized
parameter evidence to the committed T2/T4 ledger and TF-11/IC-3 demand
algebras without consulting the inferred contract being checked.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: VF-3 + PV-4 | spec: annex-e §AIMS §9 + §8 RL-2 |
  .proof: aims-proof/proofs/09-verification/VF-3.proof +
    aims-proof/proofs/12-provenance/T4-contract-boundary-composition.proof |
  map: aims-proof/scripts/proof-lean-map.json.
-/

import AimsProof.ContractBoundary
import AimsProof.Transfer
import AimsProof.Verification

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## VF-3 independent realized parameter evidence -/

/-- One semantic event observed in realized IR for a parameter class.

`localCredit n` is an explicit realized `RcInc(count=n)`. `transfer kind` is
classified through the committed RL-2 terminal-use kind. `release` is a
non-iter ownership discharge such as an explicit decrement. An iter-consuming
handoff is represented once as `transfer ApplyToIterConsumingParam`; its
callee-side release must not also be recorded as a local release. -/
inductive RealizedParamEvent
  | localCredit (count : Nat)
  | transfer (kind : TerminalUse)
  | release
  | borrowedSharingCall
  | read
deriving Repr, DecidableEq

/-- Evidence along one realizable CFG path. Demands are semantic TF-11 demand
tokens, not raw variable-occurrence or block-visit counts. -/
structure RealizedParamPath where
  events : List RealizedParamEvent
  demands : List Consumption
deriving Repr, DecidableEq

/-- The complete realized evidence for one parameter: an explicit set of CFG
paths. It intentionally contains no inferred `ParamContract` fields. -/
structure RealizedParamEvidence where
  paths : List RealizedParamPath
deriving Repr, DecidableEq

/-- Whether an observed terminal-use kind is the independently realized
iter-consuming transfer row of the committed RL-2 table. -/
def RealizedParamEvent.isIterTransfer : RealizedParamEvent → Bool
  | .transfer .ApplyToIterConsumingParam => true
  | _ => false

/-- Convert realized events to the LOCAL funding ledger used to decide Access.
Iter-consuming transfer is caller-funded by its independently observed
terminal-use kind, so it consumes no local credit. Every other RL-2 transfer
and every explicit release must be funded by a preceding realized local
credit. -/
def RealizedParamEvent.localFundingEvents : RealizedParamEvent → List LedgerEvent
  | .localCredit count => List.replicate count .credit
  | .transfer .ApplyToIterConsumingParam => []
  | .transfer kind =>
      if rl2_use_transfers_ownership kind then [.consume] else []
  | .release => [.consume]
  | .borrowedSharingCall => []
  | .read => []

/-- The local funding ledger for a path, starting from zero. -/
def RealizedParamPath.localFundingLedger (path : RealizedParamPath) : List LedgerEvent :=
  path.events.flatMap RealizedParamEvent.localFundingEvents

/-- Borrowed access is sufficient on one path exactly when each non-iter
ownership discharge is funded by a preceding explicit local credit. -/
def RealizedParamPath.locallyFunded (path : RealizedParamPath) : Bool :=
  countNeverNegative 0 path.localFundingLedger

/-- VF-3 realized Access derivation. Borrowed is accepted only when every path
is locally funded. This function cannot self-justify from the inferred subject:
its only input is `RealizedParamEvidence`. -/
def RealizedParamEvidence.access (evidence : RealizedParamEvidence) : AccessClass :=
  if evidence.paths.all RealizedParamPath.locallyFunded then .Borrowed else .Owned

/-- Independently observed iter-transfer evidence, derived solely from realized
terminal-use kinds. -/
def RealizedParamEvidence.iterTransfers (evidence : RealizedParamEvidence) : Bool :=
  evidence.paths.any fun path =>
    path.events.any RealizedParamEvent.isIterTransfer

/-- The independently realized iter-transfer bit must equal the inferred bit;
the inferred bit never creates evidence or funding. -/
def RealizedParamEvidence.iterTransferAgrees
    (evidence : RealizedParamEvidence) (inferredIterTransfers : Bool) : Bool :=
  evidence.iterTransfers == inferredIterTransfers

/-- Independent realized `may_share` evidence is either an explicit positive
local credit or a Borrowed boundary call whose callee parameter may share.
The latter is derived from the realized argument ownership plus the OTHER
callee's summary; it is never read from the subject contract being checked. -/
def RealizedParamEvent.isSharingEvidence : RealizedParamEvent → Bool
  | .localCredit 0 => false
  | .localCredit (_ + 1) => true
  | .borrowedSharingCall => true
  | _ => false

def RealizedParamEvidence.mayShare (evidence : RealizedParamEvidence) : Bool :=
  evidence.paths.any fun path =>
    path.events.any RealizedParamEvent.isSharingEvidence

/-! ## VF-3 consumption via TF-11 sequential composition + IC-3 alternatives -/

/-- Sequential semantic demand on one path uses TF-11 `seq_add`. -/
def realizedSequentialConsumption : List Consumption → Consumption
  | [] => .Dead
  | demand :: rest => Consumption.seqAdd demand (realizedSequentialConsumption rest)

/-- Alternative path demand uses the IC-3 component join, never addition. -/
def realizedAlternativeConsumption : List Consumption → Consumption
  | [] => .Dead
  | demand :: rest => demand.join (realizedAlternativeConsumption rest)

/-- Realized consumption: TF-11 within each path, IC-3 across alternatives. -/
def RealizedParamEvidence.consumption (evidence : RealizedParamEvidence) : Consumption :=
  realizedAlternativeConsumption
    (evidence.paths.map fun path => realizedSequentialConsumption path.demands)

/-! ### Realized endpoint identity before TF-11 composition -/

/-- Fold one realized path after SSA/CFG analysis has assigned semantic
endpoint identities. Repeated tokens for one proven lowering endpoint are
counted once; different endpoint identities compose through TF-11. Source spans
are not endpoint identity: generated/no-span sites and same-span sites remain
different unless a structural lowering rule has already assigned one ID.

Loop recurrence is represented by distinct semantic occurrences (or an
already-widened Unrestricted token) before this fold; it is not mistaken for a
duplicate lowering token. -/
def realizedEndpointConsumptionFrom
    (seen : List Nat) : List Nat → Consumption
  | [] => .Dead
  | endpoint :: rest =>
      if endpoint ∈ seen then
        realizedEndpointConsumptionFrom seen rest
      else
        Consumption.seqAdd .Linear
          (realizedEndpointConsumptionFrom (endpoint :: seen) rest)

/-- Sequential demand after structural endpoint normalization. -/
def realizedEndpointConsumption (endpoints : List Nat) : Consumption :=
  realizedEndpointConsumptionFrom [] endpoints

/-- Alternative endpoint-normalized paths still compose through IC-3 join. -/
def realizedEndpointAlternativeConsumption (paths : List (List Nat)) : Consumption :=
  realizedAlternativeConsumption (paths.map realizedEndpointConsumption)

/-! ## VF-3 comparison against the inferred subject -/

/-- Project subject-independent realized evidence into the existing VF-3
comparison carrier. The inferred subject is deliberately absent from the
projection inputs. -/
def RealizedParamEvidence.contractDims
    (evidence : RealizedParamEvidence) (realizedMayAlloc : Bool) : ContractDims :=
  { access := evidence.access
    consumption := evidence.consumption
    mayAlloc := realizedMayAlloc }

/-- The complete per-parameter oracle verdict: independently realized iter
evidence must agree, and every re-derived contract dimension must refine the
inferred subject. The subject participates only on the right-hand comparison
side. -/
def RealizedParamEvidence.oracleAccepts
    (evidence : RealizedParamEvidence)
    (realizedMayAlloc : Bool)
    (inferred : ContractDims)
    (inferredIterTransfers : Bool) : Bool :=
  evidence.iterTransferAgrees inferredIterTransfers
    && vf3_safe (evidence.contractDims realizedMayAlloc) inferred

/-- §VF-3 primary soundness statement. Acceptance is exactly independent iter
agreement plus componentwise refinement of the evidence-derived access,
consumption, and effect dimensions. -/
theorem VF3_independent_oracle_acceptance_iff
    (evidence : RealizedParamEvidence)
    (realizedMayAlloc : Bool)
    (inferred : ContractDims)
    (inferredIterTransfers : Bool) :
    evidence.oracleAccepts realizedMayAlloc inferred inferredIterTransfers = true ↔
      evidence.iterTransferAgrees inferredIterTransfers = true
        ∧ evidence.access.rank ≤ inferred.access.rank
        ∧ evidence.consumption.rank ≤ inferred.consumption.rank
        ∧ boolRank realizedMayAlloc ≤ boolRank inferred.mayAlloc := by
  simp [RealizedParamEvidence.oracleAccepts,
    RealizedParamEvidence.contractDims, vf3_safe, and_assoc]

/-- The alternative operator is exactly the consumption component of IC-3
ParamContract join. -/
theorem VF3_consumption_alternative_matches_IC3 (left right : ParamContract) :
    (left.join right).consumption = left.consumption.join right.consumption := by
  rfl

/-- The sequential operator is exactly TF-11 `seq_add`. -/
theorem VF3_consumption_sequential_matches_TF11 (left right : Consumption) :
    realizedSequentialConsumption [left, right]
        = Consumption.seqAdd left right := by
  cases left <;> cases right <;> rfl

/-- One source demand lowered to two realized instructions is still one Linear
semantic endpoint after normalization. -/
theorem VF3_duplicate_lowering_endpoint_is_linear :
    realizedEndpointConsumption [7, 7] = .Linear := by
  decide

/-- Sharing a source span does not merge genuinely distinct sequential
endpoints; their TF-11 composition is Unrestricted. -/
theorem VF3_distinct_same_span_endpoints_are_unrestricted :
    realizedEndpointConsumption [7, 11] = .Unrestricted := by
  decide

/-- Without structural identity evidence (including generated/no-span IR),
distinct realized sites remain conservative independent endpoints. -/
theorem VF3_generated_distinct_endpoints_are_unrestricted :
    realizedEndpointConsumption [13, 17] = .Unrestricted := by
  decide

/-- Endpoint normalization is per path; two alternative one-endpoint paths
remain Linear under IC-3 rather than becoming sequential demand. -/
theorem VF3_endpoint_alternatives_remain_linear :
    realizedEndpointAlternativeConsumption [[7], [11]] = .Linear := by
  decide

/-! ## Load-bearing positive and negative witnesses -/

def vf3CreditThenTransfer : RealizedParamEvidence :=
  ⟨[⟨[.localCredit 1, .transfer .ApplyToOwnedParam], []⟩]⟩

def vf3TransferThenCredit : RealizedParamEvidence :=
  ⟨[⟨[.transfer .ApplyToOwnedParam, .localCredit 1], []⟩]⟩

/-- A preceding realized credit funds one non-iter transfer, so incoming Access
may remain Borrowed. -/
theorem VF3_local_credit_funds_borrowed_transfer :
    vf3CreditThenTransfer.access = .Borrowed := by
  decide

/-- Negative self-funding witness: a later credit cannot retroactively fund an
earlier transfer. Starting local funding from an inferred subject field would
wrongly accept this path. -/
theorem VF3_inferred_self_funding_rejected :
    vf3TransferThenCredit.access = .Owned := by
  decide

/-- Borrowed-COW discharge has no exceptional seed: absent an explicit prior
realized credit, a non-iter release requires Owned access. -/
theorem VF3_uncredited_borrowed_cow_release_requires_owned :
    (RealizedParamEvidence.mk [⟨[.release], []⟩]).access = .Owned := by
  decide

/-- Independently realized iter transfer remains Borrowed because the committed
terminal-use kind supplies the inward handoff; it also produces the iter bit
that is compared to the inferred flag. -/
theorem VF3_iter_transfer_is_independent_inward_funding :
    let evidence : RealizedParamEvidence :=
      ⟨[⟨[.transfer .ApplyToIterConsumingParam], [.Linear]⟩]⟩
    evidence.access = .Borrowed
      ∧ evidence.iterTransfers = true
      ∧ evidence.iterTransferAgrees true = true := by
  decide

/-- Negative subject-claim witness: an inferred iter bit without realized
terminal-use evidence is rejected and grants no funding. -/
theorem VF3_inferred_iter_claim_cannot_create_evidence :
    let evidence : RealizedParamEvidence := ⟨[⟨[], []⟩]⟩
    evidence.iterTransferAgrees true = false := by
  decide

/-- Path quantification has teeth: one funded branch cannot mask an unfunded
alternative. -/
theorem VF3_branch_with_unfunded_transfer_requires_owned :
    let evidence : RealizedParamEvidence :=
      ⟨[⟨[.localCredit 1, .transfer .ApplyToOwnedParam], []⟩,
        ⟨[.transfer .ApplyToOwnedParam], []⟩]⟩
    evidence.access = .Owned := by
  decide

/-- Branch multiplicity negative witness: one Linear demand on each of two
alternative paths remains Linear under IC-3 join; it is not Unrestricted.
This rejects raw CFG visit-count addition. -/
theorem VF3_alternative_single_uses_are_not_unrestricted :
    let evidence : RealizedParamEvidence :=
      ⟨[⟨[], [.Linear]⟩, ⟨[], [.Linear]⟩]⟩
    evidence.consumption = .Linear := by
  decide

/-- Two sequential Linear demands on one path become Unrestricted by TF-11. -/
theorem VF3_sequential_single_uses_are_unrestricted :
    let evidence : RealizedParamEvidence :=
      ⟨[⟨[], [.Linear, .Linear]⟩]⟩
    evidence.consumption = .Unrestricted := by
  decide

/-- Explicit local credit independently implies may-share; no inferred effect
field participates in the derivation. -/
theorem VF3_local_credit_implies_may_share :
    vf3CreditThenTransfer.mayShare = true := by
  decide

/-- A Borrowed call propagates another callee parameter's independently known
sharing effect without inventing local ownership credit. -/
theorem VF3_borrowed_boundary_propagates_may_share :
    let evidence : RealizedParamEvidence :=
      ⟨[⟨[.borrowedSharingCall], []⟩]⟩
    evidence.access = .Borrowed
      ∧ evidence.mayShare = true := by
  decide

/-- The may-share alternative composition is the same boolean OR used by IC-3. -/
theorem VF3_may_share_alternative_matches_IC3 (left right : ParamContract) :
    (left.join right).may_share = (left.may_share || right.may_share) := by
  rfl

end AimsProof
