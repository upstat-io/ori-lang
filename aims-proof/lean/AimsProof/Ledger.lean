/-
AIMS CFG-ledger module — kernel-checked Lean proofs of the compositional
placement soundness theorem T2 over the CFG-ledger model: blocks, directed
edges with distinguished kinds (normal / back-edge / unwind), a TRMC-rewritten
tail-call region marker, fuel-bounded walks, and per-partition-class event
ledgers derived from instruction-shaped input by a computed classification.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: T2 (compositional placement soundness over the CFG ledger) |
  spec: annex-e §AIMS §8 (the consumed committed family: the RL-2 twelve-kind
    terminal-use table, the RL-4 jump-arg exemption, the RL-7 dynamic-COW
    emission, the RL-34 no-post-tail-call-dec law) |
  .proof: aims-proof/proofs/12-provenance/T2-compositional-placement.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: T2 governs the placement of per-class releases (`BurdenDec`)
over the realized CFG (compiler/ori_arc/src/lower/burden_lower/ Phase-5
emission + compiler/ori_arc/src/aims/realize/ Phase-6/7 placement): where a
release may land relative to reads, mutations, merges, back-edges, unwind
cleanups, and TRMC-rewritten tail calls. The partition classes are the T1
union-find representatives (AimsProof.Partition `PartitionUF` /
`buildPartitionUF` / `PartitionUF.find`) — the engine takes the partition AS GIVEN
input and admits no phi edge of its own. Events are DERIVED, never free input:
the classification function maps each instruction to its per-class ledger
event through the committed RL-2 ownership-transfer table
(`rl2_use_transfers_ownership`, AimsProof.Realization) and the partition —
BIRTH (fresh allocation), CREDIT (RL-1 duplication inc / cross-class jump-arg
handoff in), CONSUME (placed release / ownership-transferring terminal use /
cross-class jump-arg handoff out), READ (borrow-view read / non-transferring
terminal read), MUTATE (RL-7 dynamic-COW mutation carrying its live-sibling
floor computed from the path suffix). The model is the calculus authority for
the placement invariant; conformance of the shipped emitter to the model is
established empirically on the compiler's ARC-verified test surface, not here.

Structure:
  Part A — the CFG engine: blocks, `CfgEdge` with `EdgeKind`
    (normal / backEdge / unwind), the TRMC region marker, the declarative walk
    predicate `isWalk`, and the fuel-bounded walk enumeration `walksFrom`
    (fuel-bounded recursion; no termination measure needed).
  Part B — the derived per-class ledger: `LedgerInstr` (instruction-shaped
    input), the computed classification `deriveLedger`, the three-clause
    invariant `threeClauses` — (1) net = 0 per class per path; (2) running
    count >= 1 at every READ; (3) running count >= 1 + live-siblings at every
    dynamic-COW MUTATE — and the operational machine `runLedger` with
    use-after-free and COW-hazard flags.
  Part C — the load-bearing equivalence `three_clauses_iff_ledger_safe` and
    THE compositional placement theorem `compositional_placement_sound`:
    a placement satisfying the three clauses per class per path is
    balanced-and-safe on EVERY path (no leak, no use-after-free, no COW
    corruption); enumeration soundness makes the fuel-bounded walk set honest
    (every enumerated walk is a genuine CFG walk).
  Part D — classification bridges: the derived terminal-use event matches the
    committed RL-2 table verdict for all twelve kinds; jump-arg routing
    consumes the partition (same-class exemption / cross-class handoff);
    class equality IS T1 `PartitionUF.sameRep`.
  Part E — kill-criterion K1: a multi-predecessor merge with one predecessor
    arriving via an UNWIND edge — the class member dead on the unwind path
    (cleanup released it), live through the normal path. The path-local
    placement satisfies the invariant on both paths; relocating the release
    PAST the merge double-frees the unwind path and is provably rejected.
  Part F — kill-criterion K2: a back-edge loop, TRMC-marked (the rewritten
    self-recursive tail call), proven for EVERY iteration count n — the walk
    crossing the back-edge n times is a genuine CFG walk and every touched
    class satisfies all three clauses; a release placed after the tail-call
    transfer inside the TRMC region is provably rejected (the RL-34 law at
    the ledger level).
  Part G — negative witnesses: a release placed before a later READ on the
    same class/path violates clause 2 (use-after-free observed); an early
    sibling release before a dynamic-COW MUTATE violates clause 3 while
    clause 2 alone would pass (the COW-corruption surface only clause 3 sees).
-/

import AimsProof.Partition
import AimsProof.Realization

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §T2 Part A — the CFG engine

    Blocks are `Nat` ids. Edges are directed and carry a kind: `normal`
    forward flow, `backEdge` (loop latch to header), `unwind` (a throwing
    block to its cleanup landing block). `trmcBlocks` marks the
    TRMC-rewritten tail-call region (the loop a self-recursive tail call
    became). Walks are fuel-bounded edge-following block sequences from the
    entry to an exit — fuel-bounded recursion, so a back-edge CFG needs no
    termination argument. -/

/-- §T2 the edge kinds the placement invariant must survive. -/
inductive EdgeKind
  | normal    -- forward control flow
  | backEdge  -- loop latch -> header (the walk may cross it any number of times)
  | unwind    -- throwing block -> unwind-cleanup landing block
deriving Repr, DecidableEq

/-- §T2 a directed CFG edge. -/
structure CfgEdge where
  src : Nat
  dst : Nat
  kind : EdgeKind
deriving Repr, DecidableEq

/-- §T2 instruction-shaped input — what the realization walk reads per block.
    Values are SSA node ids; the partition maps each value to its
    same-allocation class (T1). `burdenDec` is the PLACED release — the
    placement decision under verification. `escapeUse` is an RL-2 terminal
    use whose consumer is outside the function (the twelve-kind table);
    `jumpArg` is the in-function `TerminalUse.JumpArg` form carrying its
    receiving block-param. -/
inductive LedgerInstr
  | construct (v : Nat)                     -- fresh allocation into value v
  | dup (v : Nat)                           -- RL-1 duplication inc of value v
  | projRead (v : Nat)                      -- Project borrow-view read of v
  | cowMutate (v : Nat)                     -- RL-7 dynamic-COW mutation through v
  | escapeUse (v : Nat) (u : TerminalUse)   -- RL-2 terminal use of v (12-kind table)
  | jumpArg (v : Nat) (p : Nat)             -- jump-arg edge transfer v -> block-param p
  | burdenDec (v : Nat)                     -- a PLACED release on v (the placement)
  | holeFill (v : Nat) (hole : Nat)         -- TRMC ContextHole fill-at-recursive-call:
                                            -- v's ref transfers INTO aggregate `hole`'s
                                            -- interior (consume); the hole write carries
                                            -- the clause-3 floor on `hole`'s class (K3)
deriving Repr, DecidableEq

/-- §T2 a CFG with per-block instruction lists and the TRMC region marker. -/
structure LedgerCfg where
  entry : Nat
  exits : List Nat
  edges : List CfgEdge
  blockInstrs : Nat → List LedgerInstr
  trmcBlocks : List Nat

/-- §T2 an edge from `a` to `b` exists (any kind). -/
def hasEdge (g : LedgerCfg) (a b : Nat) : Bool :=
  g.edges.any (fun e => e.src == a && e.dst == b)

/-- §T2 the successor blocks of `b`. -/
def cfgSuccessors (g : LedgerCfg) (b : Nat) : List Nat :=
  (g.edges.filter (fun e => e.src == b)).map (fun e => e.dst)

/-- §T2 the predecessor blocks of `b` (the multi-pred-merge witness reads it). -/
def cfgPreds (g : LedgerCfg) (b : Nat) : List Nat :=
  (g.edges.filter (fun e => e.dst == b)).map (fun e => e.src)

/-- §T2 the declarative walk-suffix predicate: consecutive blocks are
    edge-connected and the last block is an exit. -/
def isWalkSuffix (g : LedgerCfg) : List Nat → Bool
  | [] => false
  | [b] => g.exits.contains b
  | a :: b :: rest => hasEdge g a b && isWalkSuffix g (b :: rest)

/-- §T2 the declarative walk predicate: starts at the entry, edge-connected,
    ends at an exit. Covers EVERY walk — including one crossing a back-edge
    arbitrarily many times. -/
def isWalk (g : LedgerCfg) (w : List Nat) : Bool :=
  match w with
  | [] => false
  | b :: _ => b == g.entry && isWalkSuffix g w

/-- §T2 fuel-bounded walk enumeration from block `b`: every edge-following
    block sequence of at most `fuel` edges ending at an exit. Fuel-bounded
    recursion — the back-edge makes the unbounded walk set infinite; the
    enumeration is the finite fuel-indexed approximation, proven sound
    against `isWalkSuffix` below. -/
def walksFrom (g : LedgerCfg) : Nat → Nat → List (List Nat)
  | 0, b => if g.exits.contains b then [[b]] else []
  | fuel+1, b =>
      if g.exits.contains b then
        [b] :: (cfgSuccessors g b).flatMap
          (fun s => (walksFrom g fuel s).map (fun w => b :: w))
      else
        (cfgSuccessors g b).flatMap
          (fun s => (walksFrom g fuel s).map (fun w => b :: w))

/-- §T2 the fuel-bounded walk set of the CFG. -/
def cfgWalks (g : LedgerCfg) (fuel : Nat) : List (List Nat) :=
  walksFrom g fuel g.entry

/-- §T2 the instruction stream of a walk (per-block lists concatenated). -/
def walkInstrs (g : LedgerCfg) (w : List Nat) : List LedgerInstr :=
  w.flatMap g.blockInstrs

/-! ## §T2 Part B — the derived per-class ledger + the three-clause invariant

    The event vocabulary over one partition class: BIRTH / CREDIT (+1),
    CONSUME (-1), READ (floor 1), MUTATE (floor 1 + live siblings). Events
    are COMPUTED from the instruction stream by `deriveLedger` — the
    classification consumes the committed RL-2 twelve-kind table
    (`rl2_use_transfers_ownership`) and the partition-class map; a model whose
    events were unconstrained free input would prove nothing about the
    emitter. -/

/-- §T2 a per-class ledger event. `mutate` carries the live-sibling count the
    dynamic-COW floor demands — computed at derivation time from the path
    suffix, never free input. -/
inductive LedgerEvent
  | birth              -- the class's allocation funds the ledger: +1
  | credit             -- RL-1 duplication inc / cross-class handoff in: +1
  | consume            -- placed release / ownership handoff out: -1
  | read               -- borrow-view or terminal read: running count >= 1
  | mutate (liveSibs : Nat)  -- dynamic-COW mutate: count >= 1 + liveSibs
deriving Repr, DecidableEq

/-- §T2 the value an instruction READS, per the RL-2 verdict: a borrow-view
    `projRead` reads; a NON-transferring terminal use is the terminal read
    (RL-2 emits the dec at/after it); a transferring use hands the reference
    off and reads nothing the sibling floor must protect. -/
def LedgerInstr.readsValue : LedgerInstr → Option Nat
  | .projRead v => some v
  | .escapeUse v u => if rl2_use_transfers_ownership u then none else some v
  | _ => none

/-- §T2 the live-sibling count at a mutate through `v`: the number of DISTINCT
    other values of the same class still read in the path suffix. A sibling
    read after the mutate means its data is still demanded — an in-place
    mutation while the placed count undercounts it corrupts that read. -/
def sibReadCount (classOf : Nat → Nat) (c v : Nat) (rest : List LedgerInstr) : Nat :=
  (((rest.filterMap LedgerInstr.readsValue).filter
      (fun w => !(w == v) && classOf w == c)).eraseDups).length

/-- §T2 THE computed classification: derive class `c`'s ledger from the
    instruction stream. Per instruction:
    * `construct` births its value's class.
    * `dup` credits its value's class (RL-1 duplication inc).
    * `projRead` reads its value's class.
    * `cowMutate` mutates, carrying the live-sibling floor from the suffix.
    * `escapeUse` consults the COMMITTED RL-2 table: an ownership-transferring
      kind (9 of 12) consumes — the obligation hands off; a non-transferring
      kind (3 of 12) is the terminal READ the placed dec must follow.
    * `jumpArg` consults the PARTITION: a same-class receiving param is the
      RL-4 jump-arg exemption (the reference persists in the class — no
      event); a cross-class param hands the obligation off (consume from the
      source class, credit into the param's class).
    * `burdenDec` — the PLACED release — consumes. -/
def deriveLedger (classOf : Nat → Nat) (c : Nat) : List LedgerInstr → List LedgerEvent
  | [] => []
  | .construct v :: rest =>
      if classOf v == c then .birth :: deriveLedger classOf c rest
      else deriveLedger classOf c rest
  | .dup v :: rest =>
      if classOf v == c then .credit :: deriveLedger classOf c rest
      else deriveLedger classOf c rest
  | .projRead v :: rest =>
      if classOf v == c then .read :: deriveLedger classOf c rest
      else deriveLedger classOf c rest
  | .cowMutate v :: rest =>
      if classOf v == c then
        .mutate (sibReadCount classOf c v rest) :: deriveLedger classOf c rest
      else deriveLedger classOf c rest
  | .escapeUse v u :: rest =>
      if classOf v == c then
        (if rl2_use_transfers_ownership u then .consume else .read)
          :: deriveLedger classOf c rest
      else deriveLedger classOf c rest
  | .jumpArg v p :: rest =>
      if classOf v == c then
        (if classOf p == c then deriveLedger classOf c rest
         else .consume :: deriveLedger classOf c rest)
      else
        if classOf p == c then .credit :: deriveLedger classOf c rest
        else deriveLedger classOf c rest
  | .burdenDec v :: rest =>
      if classOf v == c then .consume :: deriveLedger classOf c rest
      else deriveLedger classOf c rest
  | .holeFill v hole :: rest =>
      if classOf hole == c then
        (if classOf v == c then
          .mutate (sibReadCount classOf c hole rest) :: .consume
            :: deriveLedger classOf c rest
         else
          .mutate (sibReadCount classOf c hole rest) :: deriveLedger classOf c rest)
      else
        if classOf v == c then .consume :: deriveLedger classOf c rest
        else deriveLedger classOf c rest

/-- §T2 the running-count delta of one event. -/
def eventDelta : LedgerEvent → Int
  | .birth => 1
  | .credit => 1
  | .consume => -1
  | .read => 0
  | .mutate _ => 0

/-- §T2 the floor an event demands of the running count BEFORE it fires:
    a READ demands count >= 1 (clause 2); a MUTATE demands
    count >= 1 + live-siblings (clause 3); credit-bearing events demand none. -/
def eventFloor (n : Int) : LedgerEvent → Bool
  | .read => decide (1 ≤ n)
  | .mutate sibs => decide (1 + (sibs : Int) ≤ n)
  | _ => true

/-- §T2 clause 1's measure: the class's net ledger balance over the path. -/
def ledgerNet (es : List LedgerEvent) : Int :=
  (es.map eventDelta).foldr (· + ·) 0

/-- §T2 clause 1: the class's ledger nets zero over the path. -/
def clauseNetZero (es : List LedgerEvent) : Bool := ledgerNet es == 0

/-- §T2 clauses 2 + 3 checked along the running count from `n`. -/
def clauseFloors : Int → List LedgerEvent → Bool
  | _, [] => true
  | n, e :: rest => eventFloor n e && clauseFloors (n + eventDelta e) rest

/-- §T2 THE three-clause placement invariant on one class's per-path ledger:
    (1) net = 0; (2) count >= 1 at every READ; (3) count >= 1 + live-siblings
    at every dynamic-COW MUTATE. -/
def threeClauses (es : List LedgerEvent) : Bool :=
  clauseNetZero es && clauseFloors 0 es

/-- §T2 the operational machine state: the running placed count plus the two
    hazard flags — `uaf` (a READ observed a freed value) and `cowHazard`
    (a MUTATE observed a count below the sibling floor: the runtime
    uniqueness check would mutate in place under a live sibling). -/
structure LedgerRun where
  count : Int
  uaf : Bool
  cowHazard : Bool
deriving Repr, DecidableEq

/-- §T2 one operational step. A flag, once set, is never cleared. -/
def ledgerStep (st : LedgerRun) : LedgerEvent → LedgerRun
  | .birth => { st with count := st.count + 1 }
  | .credit => { st with count := st.count + 1 }
  | .consume => { st with count := st.count - 1 }
  | .read => { st with uaf := st.uaf || !eventFloor st.count .read }
  | .mutate sibs =>
      { st with cowHazard := st.cowHazard || !eventFloor st.count (.mutate sibs) }

/-- §T2 run a class's per-path ledger from count 0 with clean flags. -/
def runLedger (es : List LedgerEvent) : LedgerRun :=
  es.foldl ledgerStep ⟨0, false, false⟩

/-- §T2 balanced-and-safe: no use-after-free observed, no COW hazard observed,
    and the class nets to zero (no leak, no double free). -/
def ledgerSafe (es : List LedgerEvent) : Prop :=
  (runLedger es).uaf = false ∧ (runLedger es).cowHazard = false
    ∧ (runLedger es).count = 0

/-! ## §T2 Part C — clauses <-> safety, and the compositional theorem -/

/-- §T2 the operational count is the start count plus the net ledger. -/
theorem foldl_ledgerStep_count :
    ∀ (es : List LedgerEvent) (st : LedgerRun),
      (es.foldl ledgerStep st).count = st.count + ledgerNet es := by
  intro es
  induction es with
  | nil =>
      intro st
      show st.count = st.count + (0 : Int)
      omega
  | cons e rest ih =>
      intro st
      rw [List.foldl_cons, ih]
      have hnet : ledgerNet (e :: rest) = eventDelta e + ledgerNet rest := by
        show ((e :: rest).map eventDelta).foldr (· + ·) 0 = _
        rw [List.map_cons, List.foldr_cons]
        rfl
      rw [hnet]
      cases e with
      | birth => show st.count + 1 + ledgerNet rest = st.count + ((1 : Int) + ledgerNet rest); omega
      | credit => show st.count + 1 + ledgerNet rest = st.count + ((1 : Int) + ledgerNet rest); omega
      | consume => show st.count - 1 + ledgerNet rest = st.count + ((-1 : Int) + ledgerNet rest); omega
      | read => show st.count + ledgerNet rest = st.count + ((0 : Int) + ledgerNet rest); omega
      | mutate sibs => show st.count + ledgerNet rest = st.count + ((0 : Int) + ledgerNet rest); omega

/-- §T2 the use-after-free flag is sticky: once set it survives every step. -/
theorem foldl_ledgerStep_uaf_sticky :
    ∀ (es : List LedgerEvent) (st : LedgerRun), st.uaf = true →
      (es.foldl ledgerStep st).uaf = true := by
  intro es
  induction es with
  | nil => intro st h; exact h
  | cons e rest ih =>
      intro st h
      rw [List.foldl_cons]
      apply ih
      cases e <;> simp [ledgerStep, h]

/-- §T2 the COW-hazard flag is sticky: once set it survives every step. -/
theorem foldl_ledgerStep_cow_sticky :
    ∀ (es : List LedgerEvent) (st : LedgerRun), st.cowHazard = true →
      (es.foldl ledgerStep st).cowHazard = true := by
  intro es
  induction es with
  | nil => intro st h; exact h
  | cons e rest ih =>
      intro st h
      rw [List.foldl_cons]
      apply ih
      cases e <;> simp [ledgerStep, h]

/-- §T2 the floor clauses hold from count `n` EXACTLY when the run from
    count `n` with clean flags raises neither hazard flag — clause 2 is the
    use-after-free surface and clause 3 is the COW-corruption surface, by
    induction on the event list. -/
theorem clauseFloors_iff_flags_clean :
    ∀ (es : List LedgerEvent) (n : Int),
      clauseFloors n es = true
        ↔ ((es.foldl ledgerStep ⟨n, false, false⟩).uaf = false
            ∧ (es.foldl ledgerStep ⟨n, false, false⟩).cowHazard = false) := by
  intro es
  induction es with
  | nil =>
      intro n
      constructor
      · intro _; exact ⟨rfl, rfl⟩
      · intro _; rfl
  | cons e rest ih =>
      intro n
      rw [List.foldl_cons]
      cases e with
      | birth =>
          have hstep : ledgerStep ⟨n, false, false⟩ .birth = ⟨n + 1, false, false⟩ := rfl
          have hfl : clauseFloors n (.birth :: rest) = clauseFloors (n + 1) rest := by
            show (true && clauseFloors (n + eventDelta .birth) rest) = _
            rw [Bool.true_and]
            have harith : n + eventDelta .birth = n + 1 := by
              show n + (1 : Int) = n + 1; omega
            rw [harith]
          rw [hstep, hfl]
          exact ih (n + 1)
      | credit =>
          have hstep : ledgerStep ⟨n, false, false⟩ .credit = ⟨n + 1, false, false⟩ := rfl
          have hfl : clauseFloors n (.credit :: rest) = clauseFloors (n + 1) rest := by
            show (true && clauseFloors (n + eventDelta .credit) rest) = _
            rw [Bool.true_and]
            have harith : n + eventDelta .credit = n + 1 := by
              show n + (1 : Int) = n + 1; omega
            rw [harith]
          rw [hstep, hfl]
          exact ih (n + 1)
      | consume =>
          have hstep : ledgerStep ⟨n, false, false⟩ .consume = ⟨n - 1, false, false⟩ := rfl
          have hfl : clauseFloors n (.consume :: rest) = clauseFloors (n - 1) rest := by
            show (true && clauseFloors (n + eventDelta .consume) rest) = _
            rw [Bool.true_and]
            have harith : n + eventDelta .consume = n - 1 := by
              show n + (-1 : Int) = n - 1; omega
            rw [harith]
          rw [hstep, hfl]
          exact ih (n - 1)
      | read =>
          have hstep : ledgerStep ⟨n, false, false⟩ .read
              = ⟨n, false || !eventFloor n .read, false⟩ := rfl
          have hfl : clauseFloors n (.read :: rest)
              = (eventFloor n .read && clauseFloors (n + eventDelta .read) rest) := rfl
          have harith : n + eventDelta .read = n := by
            show n + (0 : Int) = n; omega
          rw [hstep, hfl, harith]
          cases hf : eventFloor n .read with
          | true =>
              rw [show (false || !true) = false from rfl, Bool.true_and]
              exact ih n
          | false =>
              rw [show (false || !false) = true from rfl, Bool.false_and]
              have hsticky := foldl_ledgerStep_uaf_sticky rest ⟨n, true, false⟩ rfl
              constructor
              · intro hcontra; exact absurd hcontra (by decide)
              · rintro ⟨h1, _⟩
                rw [hsticky] at h1
                exact absurd h1 (by decide)
      | mutate sibs =>
          have hstep : ledgerStep ⟨n, false, false⟩ (.mutate sibs)
              = ⟨n, false, false || !eventFloor n (.mutate sibs)⟩ := rfl
          have hfl : clauseFloors n (.mutate sibs :: rest)
              = (eventFloor n (.mutate sibs)
                  && clauseFloors (n + eventDelta (.mutate sibs)) rest) := rfl
          have harith : n + eventDelta (.mutate sibs) = n := by
            show n + (0 : Int) = n; omega
          rw [hstep, hfl, harith]
          cases hf : eventFloor n (.mutate sibs) with
          | true =>
              rw [show (false || !true) = false from rfl, Bool.true_and]
              exact ih n
          | false =>
              rw [show (false || !false) = true from rfl, Bool.false_and]
              have hsticky := foldl_ledgerStep_cow_sticky rest ⟨n, false, true⟩ rfl
              constructor
              · intro hcontra; exact absurd hcontra (by decide)
              · rintro ⟨_, h2⟩
                rw [hsticky] at h2
                exact absurd h2 (by decide)

/-- §T2 (P2) THE clauses-safety equivalence. A class's per-path ledger
    satisfies the three clauses EXACTLY when its operational run is
    balanced-and-safe: clause 1 is the no-leak/no-double-free surface
    (final count 0), clause 2 the no-use-after-free surface, clause 3 the
    no-COW-corruption surface. Both directions — the declarative check
    neither under- nor over-approximates the machine. -/
theorem three_clauses_iff_ledger_safe (es : List LedgerEvent) :
    threeClauses es = true ↔ ledgerSafe es := by
  have hcount := foldl_ledgerStep_count es ⟨0, false, false⟩
  constructor
  · intro h
    have h' : (clauseNetZero es && clauseFloors 0 es) = true := h
    rw [Bool.and_eq_true] at h'
    obtain ⟨hnet, hfloors⟩ := h'
    have hnetv : (ledgerNet es == 0) = true := hnet
    have hnet' : ledgerNet es = 0 := beq_iff_eq.mp hnetv
    have hflags := (clauseFloors_iff_flags_clean es 0).mp hfloors
    refine ⟨hflags.1, hflags.2, ?_⟩
    show (es.foldl ledgerStep ⟨0, false, false⟩).count = 0
    rw [hcount, hnet']
    show (0 : Int) + 0 = 0
    omega
  · rintro ⟨huaf, hcow, hcnt⟩
    show (clauseNetZero es && clauseFloors 0 es) = true
    rw [Bool.and_eq_true]
    constructor
    · have h0 : (es.foldl ledgerStep ⟨0, false, false⟩).count = 0 := hcnt
      rw [hcount] at h0
      have h1 : (0 : Int) + ledgerNet es = 0 := h0
      show (ledgerNet es == 0) = true
      exact beq_iff_eq.mpr (by omega)
    · exact (clauseFloors_iff_flags_clean es 0).mpr ⟨huaf, hcow⟩

/-- §T2 (P3) THE compositional placement soundness theorem. For EVERY CFG
    (back-edges, unwind edges, TRMC regions included), EVERY partition-class
    map (the T1 partition, taken as given input), a placement whose derived
    per-class ledger satisfies the three clauses on every walk is
    balanced-and-safe on EVERY walk: no leak (net 0), no use-after-free
    (count floor at every READ), no COW corruption (sibling floor at every
    MUTATE). The `threeClauses` hypothesis is the per-class per-path
    invariant; `ledgerSafe` is the operational guarantee. -/
theorem compositional_placement_sound
    (g : LedgerCfg) (classOf : Nat → Nat)
    (hplacement : ∀ w, isWalk g w = true → ∀ c : Nat,
        threeClauses (deriveLedger classOf c (walkInstrs g w)) = true) :
    ∀ w, isWalk g w = true → ∀ c : Nat,
        ledgerSafe (deriveLedger classOf c (walkInstrs g w)) := by
  intro w hw c
  exact (three_clauses_iff_ledger_safe _).mp (hplacement w hw c)

/-- §T2 (P3) the fuel-bounded enumeration corollary: the same guarantee over
    the enumerated walk set. -/
theorem compositional_placement_sound_enumerated
    (g : LedgerCfg) (classOf : Nat → Nat) (fuel : Nat)
    (hplacement : ∀ w ∈ cfgWalks g fuel, ∀ c : Nat,
        threeClauses (deriveLedger classOf c (walkInstrs g w)) = true) :
    ∀ w ∈ cfgWalks g fuel, ∀ c : Nat,
        ledgerSafe (deriveLedger classOf c (walkInstrs g w)) := by
  intro w hw c
  exact (three_clauses_iff_ledger_safe _).mp (hplacement w hw c)

/-! ## §T2 Part C.1 — enumeration soundness (the fuel-bounded walk set is
    honest: every enumerated walk is a genuine CFG walk) -/

/-- §T2 a successor produced by `cfgSuccessors` is edge-connected. -/
theorem cfgSuccessor_hasEdge (g : LedgerCfg) (b s : Nat)
    (h : s ∈ cfgSuccessors g b) : hasEdge g b s = true := by
  unfold cfgSuccessors at h
  obtain ⟨e, he, hdst⟩ := List.mem_map.mp h
  obtain ⟨hmem, hsrc⟩ := List.mem_filter.mp he
  unfold hasEdge
  apply List.any_eq_true.mpr
  refine ⟨e, hmem, ?_⟩
  rw [Bool.and_eq_true]
  exact ⟨hsrc, beq_iff_eq.mpr hdst⟩

/-- §T2 (P3) enumeration soundness: every walk `walksFrom` enumerates starts
    at its block and is a genuine edge-connected exit-terminated suffix. -/
theorem walksFrom_sound (g : LedgerCfg) :
    ∀ (fuel b : Nat) (w : List Nat), w ∈ walksFrom g fuel b →
      w.head? = some b ∧ isWalkSuffix g w = true := by
  intro fuel
  induction fuel with
  | zero =>
      intro b w hw
      simp only [walksFrom] at hw
      cases hb : g.exits.contains b with
      | true =>
          rw [hb] at hw
          simp at hw
          subst hw
          refine ⟨rfl, ?_⟩
          show g.exits.contains b = true
          exact hb
      | false =>
          rw [hb] at hw
          simp at hw
  | succ f ih =>
      intro b w hw
      simp only [walksFrom] at hw
      have onward_sound :
          w ∈ (cfgSuccessors g b).flatMap
              (fun s => (walksFrom g f s).map (fun w' => b :: w')) →
          w.head? = some b ∧ isWalkSuffix g w = true := by
        intro hmem
        obtain ⟨s, hs, hw'⟩ := List.mem_flatMap.mp hmem
        obtain ⟨w', hw'', heq⟩ := List.mem_map.mp hw'
        obtain ⟨hhead, hsuffix⟩ := ih s w' hw''
        subst heq
        cases w' with
        | nil => cases hhead
        | cons s' rest =>
            have hs' : s' = s := by
              have hsome : some s' = some s := hhead
              exact Option.some.inj hsome
            refine ⟨rfl, ?_⟩
            show (hasEdge g b s' && isWalkSuffix g (s' :: rest)) = true
            rw [Bool.and_eq_true]
            refine ⟨?_, hsuffix⟩
            rw [hs']
            exact cfgSuccessor_hasEdge g b s hs
      cases hb : g.exits.contains b with
      | true =>
          rw [hb] at hw
          simp only [reduceIte] at hw
          rcases List.mem_cons.mp hw with hcase | honward
          · subst hcase
            refine ⟨rfl, ?_⟩
            show g.exits.contains b = true
            exact hb
          · exact onward_sound honward
      | false =>
          rw [hb] at hw
          rw [if_neg (by decide : ¬(false = true))] at hw
          exact onward_sound hw

/-- §T2 (P3) every enumerated CFG walk is a genuine walk: entry-anchored,
    edge-connected, exit-terminated. -/
theorem cfgWalks_isWalk (g : LedgerCfg) (fuel : Nat) :
    ∀ w ∈ cfgWalks g fuel, isWalk g w = true := by
  intro w hw
  obtain ⟨hhead, hsuffix⟩ := walksFrom_sound g fuel g.entry w hw
  cases w with
  | nil => cases hhead
  | cons b rest =>
      have hb : b = g.entry := by
        have hsome : some b = some g.entry := hhead
        exact Option.some.inj hsome
      show (b == g.entry && isWalkSuffix g (b :: rest)) = true
      rw [Bool.and_eq_true]
      exact ⟨beq_iff_eq.mpr hb, hsuffix⟩

/-! ## §T2 Part C.2 — untouched classes and ledger decomposition lemmas -/

/-- §T2 whether an instruction touches class `c` at all. -/
def LedgerInstr.touchesClass (classOf : Nat → Nat) (c : Nat) : LedgerInstr → Bool
  | .construct v => classOf v == c
  | .dup v => classOf v == c
  | .projRead v => classOf v == c
  | .cowMutate v => classOf v == c
  | .escapeUse v _ => classOf v == c
  | .jumpArg v p => classOf v == c || classOf p == c
  | .burdenDec v => classOf v == c
  | .holeFill v hole => classOf v == c || classOf hole == c

/-- §T2 a class no instruction touches derives the empty ledger. -/
theorem deriveLedger_untouched (classOf : Nat → Nat) (c : Nat) :
    ∀ instrs : List LedgerInstr,
      instrs.all (fun i => !(LedgerInstr.touchesClass classOf c i)) = true →
      deriveLedger classOf c instrs = [] := by
  intro instrs
  induction instrs with
  | nil => intro _; rfl
  | cons i rest ih =>
      intro h
      rw [List.all_cons, Bool.and_eq_true] at h
      obtain ⟨hi, hrest⟩ := h
      have htail := ih hrest
      cases i <;> simp_all [deriveLedger, LedgerInstr.touchesClass]

/-- §T2 the empty ledger satisfies the three clauses (an untouched class is
    trivially balanced-and-safe). -/
theorem threeClauses_untouched (classOf : Nat → Nat) (c : Nat)
    (instrs : List LedgerInstr)
    (h : instrs.all (fun i => !(LedgerInstr.touchesClass classOf c i)) = true) :
    threeClauses (deriveLedger classOf c instrs) = true := by
  rw [deriveLedger_untouched classOf c instrs h]
  decide

/-- §T2 `all` distributes over a walk's per-block concatenation. -/
theorem all_flatMap {α β : Type} (p : β → Bool) (f : α → List β) :
    ∀ l : List α, (∀ a ∈ l, (f a).all p = true) → (l.flatMap f).all p = true := by
  intro l
  induction l with
  | nil => intro _; rfl
  | cons a rest ih =>
      intro h
      rw [List.flatMap_cons, List.all_append]
      rw [h a (List.mem_cons_self ..), ih (fun x hx => h x (List.mem_cons_of_mem _ hx))]
      rfl

/-- §T2 whether an instruction is a dynamic-COW mutate (the one classification
    case whose event reads the path suffix). -/
def LedgerInstr.isMutate : LedgerInstr → Bool
  | .cowMutate _ => true
  | .holeFill _ _ => true
  | _ => false

/-- §T2 derivation distributes over concatenation when the left segment holds
    no mutate (only the mutate event reads the suffix). -/
theorem deriveLedger_append_mutate_free (classOf : Nat → Nat) (c : Nat) :
    ∀ (xs ys : List LedgerInstr),
      xs.all (fun i => !(LedgerInstr.isMutate i)) = true →
      deriveLedger classOf c (xs ++ ys)
        = deriveLedger classOf c xs ++ deriveLedger classOf c ys := by
  intro xs ys
  induction xs with
  | nil => intro _; rfl
  | cons i rest ih =>
      intro h
      rw [List.all_cons, Bool.and_eq_true] at h
      obtain ⟨hi, hrest⟩ := h
      have htail := ih hrest
      cases i with
      | cowMutate v => simp [LedgerInstr.isMutate] at hi
      | holeFill v hole => simp [LedgerInstr.isMutate] at hi
      | construct v =>
          show deriveLedger classOf c (.construct v :: (rest ++ ys)) = _
          cases hv : classOf v == c <;> simp [deriveLedger, hv, htail]
      | dup v =>
          show deriveLedger classOf c (.dup v :: (rest ++ ys)) = _
          cases hv : classOf v == c <;> simp [deriveLedger, hv, htail]
      | projRead v =>
          show deriveLedger classOf c (.projRead v :: (rest ++ ys)) = _
          cases hv : classOf v == c <;> simp [deriveLedger, hv, htail]
      | escapeUse v u =>
          show deriveLedger classOf c (.escapeUse v u :: (rest ++ ys)) = _
          cases hv : classOf v == c <;> simp [deriveLedger, hv, htail]
      | jumpArg v p =>
          show deriveLedger classOf c (.jumpArg v p :: (rest ++ ys)) = _
          cases hv : classOf v == c <;> cases hp : classOf p == c <;>
            simp [deriveLedger, hv, hp, htail]
      | burdenDec v =>
          show deriveLedger classOf c (.burdenDec v :: (rest ++ ys)) = _
          cases hv : classOf v == c <;> simp [deriveLedger, hv, htail]

/-- §T2 the net ledger is additive over concatenation. -/
theorem ledgerNet_append (xs ys : List LedgerEvent) :
    ledgerNet (xs ++ ys) = ledgerNet xs + ledgerNet ys := by
  unfold ledgerNet
  induction xs with
  | nil => simp
  | cons e rest ih =>
      simp only [List.map_cons, List.cons_append, List.foldr_cons]
      rw [ih]
      omega

/-- §T2 the floor clauses decompose over concatenation: the right segment is
    checked from the running count the left segment ends at. -/
theorem clauseFloors_append (xs ys : List LedgerEvent) :
    ∀ n : Int, clauseFloors n (xs ++ ys)
      = (clauseFloors n xs && clauseFloors (n + ledgerNet xs) ys) := by
  induction xs with
  | nil =>
      intro n
      show clauseFloors n ys = (true && clauseFloors (n + ledgerNet []) ys)
      rw [Bool.true_and]
      have harith : n + ledgerNet [] = n := by
        show n + (0 : Int) = n; omega
      rw [harith]
  | cons e rest ih =>
      intro n
      have hnetcons : ledgerNet (e :: rest) = eventDelta e + ledgerNet rest := by
        show ((e :: rest).map eventDelta).foldr (· + ·) 0 = _
        rw [List.map_cons, List.foldr_cons]
        rfl
      calc clauseFloors n (e :: (rest ++ ys))
          = (eventFloor n e && clauseFloors (n + eventDelta e) (rest ++ ys)) := rfl
        _ = (eventFloor n e && (clauseFloors (n + eventDelta e) rest
              && clauseFloors ((n + eventDelta e) + ledgerNet rest) ys)) := by
              rw [ih (n + eventDelta e)]
        _ = ((eventFloor n e && clauseFloors (n + eventDelta e) rest)
              && clauseFloors (n + ledgerNet (e :: rest)) ys) := by
              rw [← Bool.and_assoc]
              have harith : (n + eventDelta e) + ledgerNet rest
                  = n + ledgerNet (e :: rest) := by
                rw [hnetcons]; omega
              rw [harith]
        _ = (clauseFloors n (e :: rest) && clauseFloors (n + ledgerNet (e :: rest)) ys) := rfl

/-- §T2 an n-fold repeated segment (the loop-body iteration builder). -/
def segRepeat {α : Type} (l : List α) : Nat → List α
  | 0 => []
  | n+1 => l ++ segRepeat l n

/-- §T2 repeating the empty segment yields the empty list. -/
theorem segRepeat_nil {α : Type} (n : Nat) : segRepeat ([] : List α) n = [] := by
  induction n with
  | zero => rfl
  | succ k ih => show [] ++ segRepeat [] k = []; rw [ih]; rfl

/-- §T2 `all` survives segment repetition. -/
theorem all_segRepeat {α : Type} (p : α → Bool) (l : List α)
    (h : l.all p = true) (n : Nat) : (segRepeat l n).all p = true := by
  induction n with
  | zero => rfl
  | succ k ih =>
      show (l ++ segRepeat l k).all p = true
      rw [List.all_append, h, ih]
      rfl

/-- §T2 a net-zero segment repeats to a net-zero ledger. -/
theorem ledgerNet_segRepeat (seg : List LedgerEvent) (h : ledgerNet seg = 0)
    (n : Nat) : ledgerNet (segRepeat seg n) = 0 := by
  induction n with
  | zero => rfl
  | succ k ih =>
      show ledgerNet (seg ++ segRepeat seg k) = 0
      rw [ledgerNet_append, h, ih]
      omega

/-- §T2 the loop-invariant floor argument: a segment whose floors hold from
    count `m` and whose net is zero repeats safely from `m` for EVERY
    repetition count — the running count re-enters each iteration at `m`.
    This is the back-edge clause proof with no termination hand-waving. -/
theorem clauseFloors_segRepeat (seg : List LedgerEvent) (m : Int) (n : Nat)
    (hfloors : clauseFloors m seg = true) (hnet : ledgerNet seg = 0) :
    clauseFloors m (segRepeat seg n) = true := by
  induction n with
  | zero => rfl
  | succ k ih =>
      show clauseFloors m (seg ++ segRepeat seg k) = true
      rw [clauseFloors_append, hfloors, hnet]
      have harith : m + (0 : Int) = m := by omega
      rw [harith, ih]
      rfl

/-- §T2 derivation commutes with segment repetition (mutate-free segments). -/
theorem deriveLedger_segRepeat (classOf : Nat → Nat) (c : Nat)
    (seg : List LedgerInstr) (h : seg.all (fun i => !(LedgerInstr.isMutate i)) = true)
    (n : Nat) :
    deriveLedger classOf c (segRepeat seg n)
      = segRepeat (deriveLedger classOf c seg) n := by
  induction n with
  | zero => rfl
  | succ k ih =>
      show deriveLedger classOf c (seg ++ segRepeat seg k) = _
      rw [deriveLedger_append_mutate_free classOf c seg (segRepeat seg k) h, ih]
      rfl

/-- §T2 the loop workhorse: a prefix + n-fold net-zero cycle + suffix ledger
    satisfies all three clauses for EVERY n, given the prefix floors from 0,
    the cycle floors from the prefix's net (re-entered every iteration), the
    suffix floors from the same count, and prefix + suffix netting zero. -/
theorem threeClauses_prefix_cycle_suffix
    (pre seg suf : List LedgerEvent)
    (hpre : clauseFloors 0 pre = true)
    (hseg : clauseFloors (ledgerNet pre) seg = true)
    (hsegnet : ledgerNet seg = 0)
    (hsuf : clauseFloors (ledgerNet pre) suf = true)
    (hnet : ledgerNet pre + ledgerNet suf = 0)
    (n : Nat) :
    threeClauses (pre ++ (segRepeat seg n ++ suf)) = true := by
  show (clauseNetZero (pre ++ (segRepeat seg n ++ suf))
      && clauseFloors 0 (pre ++ (segRepeat seg n ++ suf))) = true
  rw [Bool.and_eq_true]
  constructor
  · show (ledgerNet (pre ++ (segRepeat seg n ++ suf)) == 0) = true
    rw [ledgerNet_append, ledgerNet_append, ledgerNet_segRepeat seg hsegnet n]
    exact beq_iff_eq.mpr (by omega)
  · rw [clauseFloors_append, hpre, Bool.true_and]
    have h0 : (0 : Int) + ledgerNet pre = ledgerNet pre := by omega
    rw [h0, clauseFloors_append, ledgerNet_segRepeat seg hsegnet n,
        clauseFloors_segRepeat seg (ledgerNet pre) n hseg hsegnet, Bool.true_and]
    have h1 : ledgerNet pre + (0 : Int) = ledgerNet pre := by omega
    rw [h1, hsuf]

/-! ## §T2 Part D — classification bridges: the RL-2 table, the jump-arg
    partition routing, and the T1 partition identity -/

/-- §T2 (P1) the derived terminal-use event matches the COMMITTED RL-2
    twelve-kind table verdict: CONSUME exactly on the 9 ownership-transfer
    kinds (the consumer inherits the release obligation), READ on the 3
    non-transfer kinds (the terminal read the placed dec must follow) — for
    every terminal-use kind. -/
theorem terminal_event_matches_rl2 (classOf : Nat → Nat) (c v : Nat)
    (h : (classOf v == c) = true) (u : TerminalUse) :
    deriveLedger classOf c [.escapeUse v u]
      = [if rl2_use_transfers_ownership u then .consume else .read] := by
  show (if classOf v == c then
          (if rl2_use_transfers_ownership u then LedgerEvent.consume else .read)
            :: deriveLedger classOf c []
        else deriveLedger classOf c []) = _
  rw [h]
  rfl

/-- §T2 (P1) a transfer-kind terminal use consumes (mirrors
    `RL2_transfer_kinds_no_dec`: the caller emits no dec — the obligation
    hands off, so the class ledger records the handoff as its consume). -/
theorem transfer_use_consumes (classOf : Nat → Nat) (c v : Nat)
    (h : (classOf v == c) = true) (u : TerminalUse)
    (hu : rl2_use_transfers_ownership u = true) :
    deriveLedger classOf c [.escapeUse v u] = [.consume] := by
  rw [terminal_event_matches_rl2 classOf c v h u, hu]
  rfl

/-- §T2 (P1) a non-transfer terminal use reads (mirrors
    `RL2_nontransfer_kinds_dec`: the value must be live at its terminal read;
    the placed dec — clause 1's consume — follows it). -/
theorem borrow_read_use_reads (classOf : Nat → Nat) (c v : Nat)
    (h : (classOf v == c) = true) (u : TerminalUse)
    (hu : rl2_use_transfers_ownership u = false) :
    deriveLedger classOf c [.escapeUse v u] = [.read] := by
  rw [terminal_event_matches_rl2 classOf c v h u, hu]
  rfl

/-- §T2 (P1) the RL-4 jump-arg exemption at the ledger level: a jump arg
    whose receiving block-param is in the SAME class (the T1
    singleton-witness-admitted merge) is no ledger event — the reference
    persists in the class across the edge, so no edge dec is owed. -/
theorem jump_arg_same_class_exempt (classOf : Nat → Nat) (c v p : Nat)
    (hv : (classOf v == c) = true) (hp : (classOf p == c) = true) :
    deriveLedger classOf c [.jumpArg v p] = [] := by
  show (if classOf v == c then
          (if classOf p == c then deriveLedger classOf c []
           else .consume :: deriveLedger classOf c [])
        else
          if classOf p == c then LedgerEvent.credit :: deriveLedger classOf c []
          else deriveLedger classOf c []) = []
  rw [hv, hp]
  rfl

/-- §T2 (P1) the cross-class jump-arg handoff: when the receiving block-param
    is a DIFFERENT class (the non-admitted merge — distinct birth-sites per
    T1's kill-criterion), the source class consumes and the param class
    credits — the obligation moves between class ledgers, each path
    accounting it exactly once. -/
theorem jump_arg_cross_class_handoff (classOf : Nat → Nat) (c c' v p : Nat)
    (hv : (classOf v == c) = true) (hp : (classOf p == c) = false)
    (hv' : (classOf v == c') = false) (hp' : (classOf p == c') = true) :
    deriveLedger classOf c [.jumpArg v p] = [.consume]
    ∧ deriveLedger classOf c' [.jumpArg v p] = [.credit] := by
  constructor
  · show (if classOf v == c then
            (if classOf p == c then deriveLedger classOf c []
             else .consume :: deriveLedger classOf c [])
          else
            if classOf p == c then LedgerEvent.credit :: deriveLedger classOf c []
            else deriveLedger classOf c []) = [.consume]
    rw [hv, hp]
    rfl
  · show (if classOf v == c' then
            (if classOf p == c' then deriveLedger classOf c' []
             else .consume :: deriveLedger classOf c' [])
          else
            if classOf p == c' then LedgerEvent.credit :: deriveLedger classOf c' []
            else deriveLedger classOf c' []) = [.credit]
    rw [hv', hp']
    rfl

/-- §T2 (P1) partition identity: two values share a T2 ledger class exactly
    when the T1 union-find computes one representative — the classes ARE the
    T1 `PartitionUF` representatives, never a parallel partition notion. -/
theorem class_eq_iff_sameRep (uf : PartitionUF) (u v : Nat) :
    uf.find u = uf.find v ↔ uf.sameRep u v = true := by
  unfold PartitionUF.sameRep
  exact beq_iff_eq.symm

/-! ## §T2 Part E — kill-criterion K1: unwind-fed multi-predecessor merge

    The shape that breaks naive placement: block 4 is a multi-pred merge; one
    predecessor (block 2) arrives by normal flow with the class LIVE (read
    through block 2, released at its last read pre-merge); the other
    predecessor (block 3, the unwind-cleanup landing block) arrives via an
    UNWIND edge with the class DEAD (the cleanup released it — unwind cleanup
    is not removable). Values: 100 is the allocation, 101 its Project
    borrow-view — ONE class via the T1 tier-1 view edge, computed by the real
    union-find.

        b0 (construct 100)
         |            normal
        b1 (borrowed-call read of 100; may throw)
         |  \
  normal |   \ UNWIND
         |    \
        b2     b3 (cleanup: burdenDec 100 — class DEAD from here)
   (read 101,  |
    dec 101)   | normal
         \     |
          \    |
           b4 (MERGE: 2 preds, one unwind-fed)
            |  normal
           b5 (exit)

    The CURED placement is path-local: each path releases the class on its
    own side of the merge; block 4 touches it not at all. The REJECTED
    placement relocates the normal-path release PAST the merge into block 4:
    the normal path alone still balances — the unwind path double-frees
    (cleanup dec + merge dec), and clause 1 catches it per path. -/

def k1PartitionEdges : List (Nat × Nat) := [(101, 100)]

/-- §T2 K1's partition — the REAL T1 union-find over the tier-1 view edge. -/
def k1UF : PartitionUF := buildPartitionUF k1PartitionEdges

def k1ClassOf : Nat → Nat := fun v => k1UF.find v

/-- §T2 K1's single allocation class (the computed representative). -/
def k1Class : Nat := k1ClassOf 100

def k1CuredInstrs : Nat → List LedgerInstr
  | 0 => [.construct 100]
  | 1 => [.escapeUse 100 .ApplyToBorrowedParam]
  | 2 => [.projRead 101, .burdenDec 101]
  | 3 => [.burdenDec 100]
  | _ => []

def k1CfgEdges : List CfgEdge :=
  [ ⟨0, 1, .normal⟩, ⟨1, 2, .normal⟩, ⟨1, 3, .unwind⟩,
    ⟨2, 4, .normal⟩, ⟨3, 4, .normal⟩, ⟨4, 5, .normal⟩ ]

/-- §T2 K1 the cured (path-local) placement over the unwind-merge CFG. -/
def k1Cured : LedgerCfg :=
  { entry := 0, exits := [5], edges := k1CfgEdges,
    blockInstrs := k1CuredInstrs, trmcBlocks := [] }

/-- §T2 K1 the rejected placement: the normal-path release relocated PAST the
    multi-pred merge into block 4 (the unwind cleanup stays — it is not
    removable). -/
def k1PastMergeInstrs : Nat → List LedgerInstr
  | 0 => [.construct 100]
  | 1 => [.escapeUse 100 .ApplyToBorrowedParam]
  | 2 => [.projRead 101]
  | 3 => [.burdenDec 100]
  | 4 => [.burdenDec 100]
  | _ => []

def k1PastMerge : LedgerCfg := { k1Cured with blockInstrs := k1PastMergeInstrs }

def k1NormalWalk : List Nat := [0, 1, 2, 4, 5]
def k1UnwindWalk : List Nat := [0, 1, 3, 4, 5]

/-- §T2 (P4) K1's shape is genuine: the merge has two predecessors, one fed
    by an UNWIND edge, and the fuel-bounded enumeration finds exactly the
    normal walk and the unwind walk. -/
theorem T2_K1_shape :
    (CfgEdge.mk 1 3 EdgeKind.unwind) ∈ k1Cured.edges
    ∧ (cfgPreds k1Cured 4).length = 2
    ∧ cfgWalks k1Cured 6 = [k1NormalWalk, k1UnwindWalk] := by
  decide

/-- §T2 (P4) the cured path-local placement satisfies all three clauses on
    BOTH merge-predecessor paths — the class live through the normal path
    (read at count 1, released at its last read), dead on the unwind path
    (cleanup released it); each path nets zero. COMPUTED over the real
    union-find classes. -/
theorem T2_K1_cured_clauses :
    threeClauses (deriveLedger k1ClassOf k1Class (walkInstrs k1Cured k1NormalWalk)) = true
    ∧ threeClauses (deriveLedger k1ClassOf k1Class (walkInstrs k1Cured k1UnwindWalk)) = true := by
  decide

/-- §T2 (P4) K1 cured placement: the three-clause invariant holds on every
    enumerated walk for EVERY class — the touched class by computation, every
    untouched class by the empty-ledger lemma. -/
theorem T2_K1_cured_placement_clauses :
    ∀ w ∈ cfgWalks k1Cured 6, ∀ c : Nat,
      threeClauses (deriveLedger k1ClassOf c (walkInstrs k1Cured w)) = true := by
  intro w hw c
  have hwalks : cfgWalks k1Cured 6 = [k1NormalWalk, k1UnwindWalk] := by decide
  rw [hwalks] at hw
  by_cases hc : c = k1Class
  · subst hc
    rcases List.mem_cons.mp hw with h1 | hw'
    · subst h1; exact T2_K1_cured_clauses.1
    · rcases List.mem_cons.mp hw' with h2 | hnil
      · subst h2; exact T2_K1_cured_clauses.2
      · cases hnil
  · have hbeq : (k1Class == c) = false := by
      cases hb : k1Class == c
      · rfl
      · exact absurd (beq_iff_eq.mp hb).symm hc
    have h100 : (k1ClassOf 100 == c) = false := hbeq
    have h101 : (k1ClassOf 101 == c) = false := by
      have he : k1ClassOf 101 = k1Class := by decide
      rw [he]; exact hbeq
    apply threeClauses_untouched
    show (w.flatMap k1Cured.blockInstrs).all
        (fun i => !(LedgerInstr.touchesClass k1ClassOf c i)) = true
    apply all_flatMap
    intro b _
    match b with
    | 0 => simp [k1Cured, k1CuredInstrs, LedgerInstr.touchesClass, h100]
    | 1 => simp [k1Cured, k1CuredInstrs, LedgerInstr.touchesClass, h100]
    | 2 => simp [k1Cured, k1CuredInstrs, LedgerInstr.touchesClass, h101]
    | 3 => simp [k1Cured, k1CuredInstrs, LedgerInstr.touchesClass, h100]
    | (_+4) => rfl

/-- §T2 (P4) K1 cured placement is balanced-and-safe on every enumerated walk
    for every class — THE compositional theorem applied end-to-end to the
    unwind-merge instance. -/
theorem T2_K1_cured_safe_every_class :
    ∀ w ∈ cfgWalks k1Cured 6, ∀ c : Nat,
      ledgerSafe (deriveLedger k1ClassOf c (walkInstrs k1Cured w)) :=
  compositional_placement_sound_enumerated k1Cured k1ClassOf 6
    T2_K1_cured_placement_clauses

/-- §T2 (P4 + P6) the past-merge relocation is REJECTED: with the class dead
    on the unwind predecessor (cleanup released it) and live on the normal
    predecessor, a release relocated past the merge double-frees the unwind
    path — clause 1 computes false there and the operational count lands at
    -1. The trap is visible in the third conjunct: the normal path ALONE
    still balances; only the per-class PER-PATH invariant catches the
    relocation. Releases are path-local, never past a merge. -/
theorem T2_K1_past_merge_relocation_rejected :
    threeClauses (deriveLedger k1ClassOf k1Class (walkInstrs k1PastMerge k1UnwindWalk)) = false
    ∧ (runLedger (deriveLedger k1ClassOf k1Class (walkInstrs k1PastMerge k1UnwindWalk))).count = -1
    ∧ threeClauses (deriveLedger k1ClassOf k1Class (walkInstrs k1PastMerge k1NormalWalk)) = true := by
  decide

/-! ## §T2 Part F — kill-criterion K2: TRMC-marked back-edge loop, every
    iteration count

    The TRMC-rewritten self-recursive tail call is a loop: blocks 1 (header)
    and 2 (body) form the marked region; edge 2 -> 1 is the BACK-EDGE. Two
    allocation families thread it:

      invariant collection (class I = {10, 11}, tier-1 view edge, T1-admitted):
        allocated once at entry, read via its view EVERY iteration, released
        once at exit — its count must hold 1 across every back-edge crossing.
      loop-carried accumulator: the header block-param 21 is its OWN class
        (the T1 kill-criterion: its feeding allocations 20 — entry — and 22 —
        per-iteration — carry distinct birth-sites, so no phi admission).
        Each iteration reads the current accumulator, releases it, constructs
        a fresh one (class {22}), and transfers it into the param via the
        jump arg — the TRMC tail-call ownership transfer (RL-34
        transferOwnership; never a post-call dec). The exit returns the
        accumulator (an RL-2 Return transfer — the caller inherits it).

        b0 (construct 10, construct 20, jumpArg 20 -> 21)
         |  normal
        b1 (projRead 11)  <────────────┐
         |  normal                     │ BACK-EDGE
        b2 (projRead 21, burdenDec 21, │
            construct 22,              │
            jumpArg 22 -> 21) ─────────┘
        b1 -> b3 (projRead 21, burdenDec 11, escapeUse 21 Return)   [exit]

    The clause proofs are parametric in the iteration count n — the walk
    crossing the back-edge n times is proven a genuine CFG walk, and every
    touched class satisfies all three clauses for EVERY n, by the
    prefix/cycle/suffix decomposition (no termination hand-waving). -/

def k2PartitionEdges : List (Nat × Nat) := [(11, 10)]

/-- §T2 K2's partition — the REAL T1 union-find: the view edge unifies
    {10, 11}; the header param 21 and the accumulator allocations 20 / 22
    keep their own representatives (distinct birth-sites — no phi edge). -/
def k2UF : PartitionUF := buildPartitionUF k2PartitionEdges

def k2ClassOf : Nat → Nat := fun v => k2UF.find v

def k2ClassI : Nat := k2ClassOf 10     -- invariant collection class {10, 11}
def k2ClassHdr : Nat := k2ClassOf 21   -- loop-header accumulator block-param
def k2ClassAcc0 : Nat := k2ClassOf 20  -- entry accumulator allocation
def k2ClassAcc1 : Nat := k2ClassOf 22  -- per-iteration accumulator allocation

def k2B0 : List LedgerInstr := [.construct 10, .construct 20, .jumpArg 20 21]
def k2B1 : List LedgerInstr := [.projRead 11]
def k2B2 : List LedgerInstr := [.projRead 21, .burdenDec 21, .construct 22, .jumpArg 22 21]
def k2B3 : List LedgerInstr := [.projRead 21, .burdenDec 11, .escapeUse 21 .Return]

def k2Instrs : Nat → List LedgerInstr
  | 0 => k2B0
  | 1 => k2B1
  | 2 => k2B2
  | 3 => k2B3
  | _ => []

def k2CfgEdges : List CfgEdge :=
  [ ⟨0, 1, .normal⟩, ⟨1, 2, .normal⟩, ⟨2, 1, .backEdge⟩, ⟨1, 3, .normal⟩ ]

/-- §T2 K2 the TRMC-rewritten tail-call loop CFG (region marker on the
    header + body). -/
def k2Cfg : LedgerCfg :=
  { entry := 0, exits := [3], edges := k2CfgEdges,
    blockInstrs := k2Instrs, trmcBlocks := [1, 2] }

/-- §T2 the walk tail visiting the loop body `n` times then exiting. -/
def k2Seg : Nat → List Nat
  | 0 => [3]
  | n+1 => 2 :: 1 :: k2Seg n

/-- §T2 the full walk with `n` back-edge crossings. -/
def k2Walk (n : Nat) : List Nat := 0 :: 1 :: k2Seg n

/-- §T2 (P5) K2's shape is genuine: the back-edge and the TRMC region marker
    are present; the T1 partition unifies the view pair and keeps the header
    param distinct from BOTH accumulator allocations (the kill-criterion
    split, computed by the real union-find). -/
theorem T2_K2_shape :
    (CfgEdge.mk 2 1 EdgeKind.backEdge) ∈ k2Cfg.edges
    ∧ k2Cfg.trmcBlocks = [1, 2]
    ∧ k2UF.sameRep 11 10 = true
    ∧ k2UF.sameRep 21 20 = false
    ∧ k2UF.sameRep 21 22 = false
    ∧ k2UF.sameRep 20 22 = false := by
  decide

/-- §T2 (P5) the loop-tail suffix is edge-connected and exit-terminated for
    every iteration count (crossing the back-edge each round). -/
theorem k2_seg_suffix_valid (n : Nat) :
    isWalkSuffix k2Cfg (1 :: k2Seg n) = true := by
  induction n with
  | zero => decide
  | succ k ih =>
      show (hasEdge k2Cfg 1 2
          && (hasEdge k2Cfg 2 1 && isWalkSuffix k2Cfg (1 :: k2Seg k))) = true
      rw [show hasEdge k2Cfg 1 2 = true from by decide,
          show hasEdge k2Cfg 2 1 = true from by decide, ih]
      rfl

/-- §T2 (P5) the n-iteration walk is a genuine CFG walk for EVERY n — the
    declarative walk predicate holds across any number of back-edge
    crossings. -/
theorem T2_K2_walk_valid (n : Nat) : isWalk k2Cfg (k2Walk n) = true := by
  show ((0 == k2Cfg.entry)
      && (hasEdge k2Cfg 0 1 && isWalkSuffix k2Cfg (1 :: k2Seg n))) = true
  rw [show (0 == k2Cfg.entry) = true from by decide,
      show hasEdge k2Cfg 0 1 = true from by decide, k2_seg_suffix_valid n]
  rfl

/-- §T2 counts adjacent `(a, b)` block pairs in a walk (back-edge crossings). -/
def countAdjPair (a b : Nat) : List Nat → Nat
  | x :: y :: rest => (if x == a && y == b then 1 else 0) + countAdjPair a b (y :: rest)
  | _ => 0

theorem k2_seg_backedge_count (n : Nat) : countAdjPair 2 1 (1 :: k2Seg n) = n := by
  induction n with
  | zero => rfl
  | succ k ih =>
      show (0 + (1 + countAdjPair 2 1 (1 :: k2Seg k)) : Nat) = k + 1
      rw [ih]
      omega

/-- §T2 (P5) the n-iteration walk genuinely crosses the back-edge exactly
    `n` times — the clause proofs below really do range over every loop
    unrolling. -/
theorem T2_K2_backedge_crossings (n : Nat) : countAdjPair 2 1 (k2Walk n) = n := by
  show (0 + countAdjPair 2 1 (1 :: k2Seg n) : Nat) = n
  rw [k2_seg_backedge_count]
  omega

/-- §T2 the walk's instruction stream decomposes as prefix + n-fold loop-body
    cycle + exit suffix. -/
theorem k2_seg_instrs (n : Nat) :
    (k2Seg n).flatMap k2Cfg.blockInstrs = segRepeat (k2B2 ++ k2B1) n ++ k2B3 := by
  induction n with
  | zero => decide
  | succ k ih =>
      show k2B2 ++ (k2B1 ++ (k2Seg k).flatMap k2Cfg.blockInstrs)
          = ((k2B2 ++ k2B1) ++ segRepeat (k2B2 ++ k2B1) k) ++ k2B3
      rw [ih]
      simp [List.append_assoc]

theorem k2_walk_instrs (n : Nat) :
    walkInstrs k2Cfg (k2Walk n)
      = (k2B0 ++ k2B1) ++ (segRepeat (k2B2 ++ k2B1) n ++ k2B3) := by
  show k2B0 ++ (k2B1 ++ (k2Seg n).flatMap k2Cfg.blockInstrs) = _
  rw [k2_seg_instrs]
  simp [List.append_assoc]

/-- §T2 (P5) the invariant-collection ledger in closed form: born at entry,
    read once before the loop and once per iteration (via the T1-unified
    view), released once at exit. -/
theorem T2_K2_ledger_I (n : Nat) :
    deriveLedger k2ClassOf k2ClassI (walkInstrs k2Cfg (k2Walk n))
      = [LedgerEvent.birth, .read]
          ++ (segRepeat [LedgerEvent.read] n ++ [LedgerEvent.consume]) := by
  rw [k2_walk_instrs n,
      deriveLedger_append_mutate_free k2ClassOf k2ClassI (k2B0 ++ k2B1)
        (segRepeat (k2B2 ++ k2B1) n ++ k2B3) (by decide),
      deriveLedger_append_mutate_free k2ClassOf k2ClassI (segRepeat (k2B2 ++ k2B1) n)
        k2B3 (all_segRepeat _ (k2B2 ++ k2B1) (by decide) n),
      deriveLedger_segRepeat k2ClassOf k2ClassI (k2B2 ++ k2B1) (by decide) n,
      show deriveLedger k2ClassOf k2ClassI (k2B0 ++ k2B1)
          = [LedgerEvent.birth, .read] from by decide,
      show deriveLedger k2ClassOf k2ClassI (k2B2 ++ k2B1)
          = [LedgerEvent.read] from by decide,
      show deriveLedger k2ClassOf k2ClassI k2B3
          = [LedgerEvent.consume] from by decide]

/-- §T2 (P5) the loop-header accumulator ledger in closed form: credited by
    the entry jump arg, then per iteration read at count 1, released, and
    re-credited by the tail-call transfer; read and handed off through the
    Return at exit. -/
theorem T2_K2_ledger_Hdr (n : Nat) :
    deriveLedger k2ClassOf k2ClassHdr (walkInstrs k2Cfg (k2Walk n))
      = [LedgerEvent.credit]
          ++ (segRepeat [LedgerEvent.read, .consume, .credit] n
              ++ [LedgerEvent.read, .consume]) := by
  rw [k2_walk_instrs n,
      deriveLedger_append_mutate_free k2ClassOf k2ClassHdr (k2B0 ++ k2B1)
        (segRepeat (k2B2 ++ k2B1) n ++ k2B3) (by decide),
      deriveLedger_append_mutate_free k2ClassOf k2ClassHdr (segRepeat (k2B2 ++ k2B1) n)
        k2B3 (all_segRepeat _ (k2B2 ++ k2B1) (by decide) n),
      deriveLedger_segRepeat k2ClassOf k2ClassHdr (k2B2 ++ k2B1) (by decide) n,
      show deriveLedger k2ClassOf k2ClassHdr (k2B0 ++ k2B1)
          = [LedgerEvent.credit] from by decide,
      show deriveLedger k2ClassOf k2ClassHdr (k2B2 ++ k2B1)
          = [LedgerEvent.read, .consume, .credit] from by decide,
      show deriveLedger k2ClassOf k2ClassHdr k2B3
          = [LedgerEvent.read, .consume] from by decide]

/-- §T2 (P5) the entry accumulator ledger: born at entry and immediately
    handed into the header class by the jump arg — balanced before the loop
    begins; no iteration touches it. -/
theorem T2_K2_ledger_Acc0 (n : Nat) :
    deriveLedger k2ClassOf k2ClassAcc0 (walkInstrs k2Cfg (k2Walk n))
      = [LedgerEvent.birth, .consume] ++ (segRepeat [] n ++ []) := by
  rw [k2_walk_instrs n,
      deriveLedger_append_mutate_free k2ClassOf k2ClassAcc0 (k2B0 ++ k2B1)
        (segRepeat (k2B2 ++ k2B1) n ++ k2B3) (by decide),
      deriveLedger_append_mutate_free k2ClassOf k2ClassAcc0 (segRepeat (k2B2 ++ k2B1) n)
        k2B3 (all_segRepeat _ (k2B2 ++ k2B1) (by decide) n),
      deriveLedger_segRepeat k2ClassOf k2ClassAcc0 (k2B2 ++ k2B1) (by decide) n,
      show deriveLedger k2ClassOf k2ClassAcc0 (k2B0 ++ k2B1)
          = [LedgerEvent.birth, .consume] from by decide,
      show deriveLedger k2ClassOf k2ClassAcc0 (k2B2 ++ k2B1)
          = ([] : List LedgerEvent) from by decide,
      show deriveLedger k2ClassOf k2ClassAcc0 k2B3
          = ([] : List LedgerEvent) from by decide]

/-- §T2 (P5) the per-iteration accumulator ledger: each iteration births a
    fresh allocation and immediately transfers it into the header class via
    the TRMC tail-call jump arg — net zero every iteration. -/
theorem T2_K2_ledger_Acc1 (n : Nat) :
    deriveLedger k2ClassOf k2ClassAcc1 (walkInstrs k2Cfg (k2Walk n))
      = [] ++ (segRepeat [LedgerEvent.birth, .consume] n ++ []) := by
  rw [k2_walk_instrs n,
      deriveLedger_append_mutate_free k2ClassOf k2ClassAcc1 (k2B0 ++ k2B1)
        (segRepeat (k2B2 ++ k2B1) n ++ k2B3) (by decide),
      deriveLedger_append_mutate_free k2ClassOf k2ClassAcc1 (segRepeat (k2B2 ++ k2B1) n)
        k2B3 (all_segRepeat _ (k2B2 ++ k2B1) (by decide) n),
      deriveLedger_segRepeat k2ClassOf k2ClassAcc1 (k2B2 ++ k2B1) (by decide) n,
      show deriveLedger k2ClassOf k2ClassAcc1 (k2B0 ++ k2B1)
          = ([] : List LedgerEvent) from by decide,
      show deriveLedger k2ClassOf k2ClassAcc1 (k2B2 ++ k2B1)
          = [LedgerEvent.birth, .consume] from by decide,
      show deriveLedger k2ClassOf k2ClassAcc1 k2B3
          = ([] : List LedgerEvent) from by decide]

/-- §T2 (P5) K2 THE back-edge kill-criterion: every touched class satisfies
    all three clauses for EVERY iteration count n — the invariant collection
    holds count 1 across every back-edge crossing (clause 2 at each
    iteration's read), the loop-carried accumulator classes re-enter each
    iteration balanced, and every class nets zero. Proven by the
    prefix/cycle/suffix loop-invariant decomposition — no termination
    hand-waving, no unrolled approximation. -/
theorem T2_K2_loop_invariant_all_iterations (n : Nat) :
    threeClauses (deriveLedger k2ClassOf k2ClassI (walkInstrs k2Cfg (k2Walk n))) = true
    ∧ threeClauses (deriveLedger k2ClassOf k2ClassHdr (walkInstrs k2Cfg (k2Walk n))) = true
    ∧ threeClauses (deriveLedger k2ClassOf k2ClassAcc0 (walkInstrs k2Cfg (k2Walk n))) = true
    ∧ threeClauses (deriveLedger k2ClassOf k2ClassAcc1 (walkInstrs k2Cfg (k2Walk n))) = true := by
  refine ⟨?_, ?_, ?_, ?_⟩
  · rw [T2_K2_ledger_I n]
    exact threeClauses_prefix_cycle_suffix _ _ _
      (by decide) (by decide) (by decide) (by decide) (by decide) n
  · rw [T2_K2_ledger_Hdr n]
    exact threeClauses_prefix_cycle_suffix _ _ _
      (by decide) (by decide) (by decide) (by decide) (by decide) n
  · rw [T2_K2_ledger_Acc0 n]
    exact threeClauses_prefix_cycle_suffix _ _ _
      (by decide) (by decide) (by decide) (by decide) (by decide) n
  · rw [T2_K2_ledger_Acc1 n]
    exact threeClauses_prefix_cycle_suffix _ _ _
      (by decide) (by decide) (by decide) (by decide) (by decide) n

/-- §T2 (P5) K2 balanced-and-safe for every iteration count — the clauses
    convert to the operational guarantee through the equivalence. -/
theorem T2_K2_loop_safe_all_iterations (n : Nat) :
    ledgerSafe (deriveLedger k2ClassOf k2ClassI (walkInstrs k2Cfg (k2Walk n)))
    ∧ ledgerSafe (deriveLedger k2ClassOf k2ClassHdr (walkInstrs k2Cfg (k2Walk n)))
    ∧ ledgerSafe (deriveLedger k2ClassOf k2ClassAcc0 (walkInstrs k2Cfg (k2Walk n)))
    ∧ ledgerSafe (deriveLedger k2ClassOf k2ClassAcc1 (walkInstrs k2Cfg (k2Walk n))) := by
  obtain ⟨h1, h2, h3, h4⟩ := T2_K2_loop_invariant_all_iterations n
  exact ⟨(three_clauses_iff_ledger_safe _).mp h1,
         (three_clauses_iff_ledger_safe _).mp h2,
         (three_clauses_iff_ledger_safe _).mp h3,
         (three_clauses_iff_ledger_safe _).mp h4⟩

/-- §T2 (P5 + P6) the RL-34 law at the ledger level: a release placed AFTER
    the tail-call ownership transfer inside the TRMC region double-frees the
    per-iteration accumulator (the transfer already handed the reference to
    the header class) — clause 1 computes false and the count lands at -1.
    Never a post-tail-call dec. -/
def k2PostCallInstrs : Nat → List LedgerInstr
  | 0 => k2B0
  | 1 => k2B1
  | 2 => k2B2 ++ [.burdenDec 22]
  | 3 => k2B3
  | _ => []

def k2PostCall : LedgerCfg := { k2Cfg with blockInstrs := k2PostCallInstrs }

theorem T2_K2_trmc_post_call_dec_rejected :
    k2PostCall.trmcBlocks.contains 2 = true
    ∧ threeClauses (deriveLedger k2ClassOf k2ClassAcc1
        (walkInstrs k2PostCall (k2Walk 1))) = false
    ∧ (runLedger (deriveLedger k2ClassOf k2ClassAcc1
        (walkInstrs k2PostCall (k2Walk 1)))).count = -1 := by
  decide

/-! ## §T2 Part G — negative witnesses: pre-read release; COW sibling floor -/

/-- §T2 the pre-read-release witness CFG: one block constructs, releases,
    then reads through the T1-unified view — the release placed BEFORE a
    later READ on the same class/path. -/
def preReadUF : PartitionUF := buildPartitionUF [(301, 300)]
def preReadClassOf : Nat → Nat := fun v => preReadUF.find v
def preReadClass : Nat := preReadClassOf 300

def preReadInstrs : Nat → List LedgerInstr
  | 0 => [.construct 300, .burdenDec 300, .projRead 301]
  | _ => []

def preReadCfg : LedgerCfg :=
  { entry := 0, exits := [0], edges := [],
    blockInstrs := preReadInstrs, trmcBlocks := [] }

def preReadWalk : List Nat := [0]

/-- §T2 (P6) the PRE-READ RELEASE is REJECTED: the ledger nets zero (clause 1
    alone would pass!) but the READ fires at count 0 — clause 2 computes
    false and the operational machine observes the use-after-free. -/
theorem T2_pre_read_release_rejected :
    isWalk preReadCfg preReadWalk = true
    ∧ threeClauses (deriveLedger preReadClassOf preReadClass
        (walkInstrs preReadCfg preReadWalk)) = false
    ∧ (runLedger (deriveLedger preReadClassOf preReadClass
        (walkInstrs preReadCfg preReadWalk))).uaf = true
    ∧ clauseNetZero (deriveLedger preReadClassOf preReadClass
        (walkInstrs preReadCfg preReadWalk)) = true := by
  decide

/-- §T2 the COW witness pair: value 400 is the allocation, 401 its dup alias
    (one class via the T1 tier-1 edge — a genuine live sibling). The cured
    placement keeps the sibling's credit live across the dynamic-COW mutate
    (count 2 >= 1 + 1 sibling); the rejected placement releases the sibling
    BEFORE the mutate — the runtime uniqueness check would see count 1,
    mutate in place, and corrupt the sibling's later read. -/
def cowUF : PartitionUF := buildPartitionUF [(401, 400)]
def cowClassOf : Nat → Nat := fun v => cowUF.find v
def cowClass : Nat := cowClassOf 400

def cowCuredInstrs : Nat → List LedgerInstr
  | 0 => [.construct 400, .dup 401, .cowMutate 400,
          .projRead 401, .burdenDec 401, .burdenDec 400]
  | _ => []

def cowBadInstrs : Nat → List LedgerInstr
  | 0 => [.construct 400, .dup 401, .burdenDec 401,
          .cowMutate 400, .projRead 401, .burdenDec 400]
  | _ => []

def cowCured : LedgerCfg :=
  { entry := 0, exits := [0], edges := [],
    blockInstrs := cowCuredInstrs, trmcBlocks := [] }

def cowBad : LedgerCfg := { cowCured with blockInstrs := cowBadInstrs }

def cowWalk : List Nat := [0]

/-- §T2 (P6) the sibling floor is satisfiable and load-bearing: the cured
    placement carries count 2 into the mutate against a live-sibling floor of
    1 + 1 (the sibling's later read was counted from the suffix at derivation
    time) — all three clauses hold. -/
theorem T2_cow_sibling_floor_enforced :
    threeClauses (deriveLedger cowClassOf cowClass
        (walkInstrs cowCured cowWalk)) = true
    ∧ (runLedger (deriveLedger cowClassOf cowClass
        (walkInstrs cowCured cowWalk))).cowHazard = false := by
  decide

/-- §T2 (P6) the EARLY SIBLING RELEASE before a dynamic-COW MUTATE is
    REJECTED — and clause 3 is the ONLY clause that sees it: the ledger nets
    zero (clause 1 passes) and the sibling's later read still sees count 1
    (clause 2 passes — no use-after-free), but the mutate fires at count 1
    against a floor of 2, so the in-place mutation would corrupt the live
    sibling. The COW-corruption surface is exactly the sibling floor. -/
theorem T2_cow_early_sibling_release_rejected :
    threeClauses (deriveLedger cowClassOf cowClass
        (walkInstrs cowBad cowWalk)) = false
    ∧ (runLedger (deriveLedger cowClassOf cowClass
        (walkInstrs cowBad cowWalk))).cowHazard = true
    ∧ (runLedger (deriveLedger cowClassOf cowClass
        (walkInstrs cowBad cowWalk))).uaf = false
    ∧ clauseNetZero (deriveLedger cowClassOf cowClass
        (walkInstrs cowBad cowWalk)) = true := by
  decide

/-! ## §T2 conclusion bundle -/

/-- §T2 the compositional-placement bundle: the clauses-safety equivalence,
    the K1 unwind-merge instance safe for every class on every enumerated
    walk, the K2 back-edge walk valid + safe for every iteration count, and
    the four rejections (past-merge relocation, TRMC post-call dec, pre-read
    release, early sibling release before a dynamic-COW mutate). -/
theorem T2_compositional_placement_bundle :
    (∀ es : List LedgerEvent, threeClauses es = true ↔ ledgerSafe es)
    ∧ (∀ w ∈ cfgWalks k1Cured 6, ∀ c : Nat,
        ledgerSafe (deriveLedger k1ClassOf c (walkInstrs k1Cured w)))
    ∧ (∀ n : Nat, isWalk k2Cfg (k2Walk n) = true)
    ∧ (∀ n : Nat,
        ledgerSafe (deriveLedger k2ClassOf k2ClassI (walkInstrs k2Cfg (k2Walk n)))
        ∧ ledgerSafe (deriveLedger k2ClassOf k2ClassHdr (walkInstrs k2Cfg (k2Walk n))))
    ∧ threeClauses (deriveLedger k1ClassOf k1Class
        (walkInstrs k1PastMerge k1UnwindWalk)) = false
    ∧ threeClauses (deriveLedger k2ClassOf k2ClassAcc1
        (walkInstrs k2PostCall (k2Walk 1))) = false
    ∧ threeClauses (deriveLedger preReadClassOf preReadClass
        (walkInstrs preReadCfg preReadWalk)) = false
    ∧ threeClauses (deriveLedger cowClassOf cowClass
        (walkInstrs cowBad cowWalk)) = false :=
  ⟨three_clauses_iff_ledger_safe,
   T2_K1_cured_safe_every_class,
   T2_K2_walk_valid,
   fun n => ⟨(T2_K2_loop_safe_all_iterations n).1, (T2_K2_loop_safe_all_iterations n).2.1⟩,
   T2_K1_past_merge_relocation_rejected.1,
   T2_K2_trmc_post_call_dec_rejected.2.1,
   T2_pre_read_release_rejected.2.1,
   T2_cow_early_sibling_release_rejected.1⟩

/-! ## §T2 Part E — the TRMC ContextHole hole-fill obligation (K3)

    `holeFill v hole` models the TRMC fill-at-recursive-call: `v`'s reference
    transfers INTO aggregate `hole`'s interior (aggregate-owned thereafter —
    the ConstructArg analogy; interior accounting is PV-6's concern), and the
    unconditional in-place hole write carries the clause-3 live-sibling floor
    on `hole`'s class. The kill criterion K3: a placed release of `v` AFTER
    its hole-fill double-releases — clause 1 nets negative. The witnesses
    below pin both directions on the COMPUTED classification. -/

/-- §T2 (K3) the release-after-fill kill criterion: `construct v; holeFill
    v hole; burdenDec v` derives [birth, consume, consume] for v's class —
    the three clauses REJECT (net -1, the double free). -/
theorem holeFill_release_after_fill_rejected :
    threeClauses
      (deriveLedger (fun v => v) 0
        [.construct 0, .holeFill 0 1, .burdenDec 0]) = false := by
  decide

/-- §T2 (K3 dual) the fill IS the filled value's release: `construct v;
    holeFill v hole` nets zero for v's class — the CURED placement plans NO
    dec after the fill. -/
theorem holeFill_is_the_release :
    threeClauses
      (deriveLedger (fun v => v) 0
        [.construct 0, .holeFill 0 1]) = true := by
  decide

/-- §T2 the context class's ledger across a fill: birth, the floored hole
    write, the onward transfer — balanced with the clause-3 floor satisfied
    (TRMC unique-context makes the floor trivially satisfiable; the model
    still checks it). -/
theorem holeFill_context_write_floored :
    threeClauses
      (deriveLedger (fun v => v) 1
        [.construct 1, .holeFill 0 1, .escapeUse 1 .Return]) = true := by
  decide

/-- §T2 the same-class self-referential fill (the linked-node chain — one
    static birth site per PV-1): two births, the fill moves the new node into
    the prior node's hole (mutate floor + consume), the chain head transfers
    out carrying the interior reference. Balanced. -/
theorem holeFill_same_class_chain_balanced :
    threeClauses
      (deriveLedger (fun _ => 0) 0
        [.construct 0, .construct 1, .holeFill 1 0, .escapeUse 0 .Return]) = true := by
  decide

/-- §T2 (K3, same-class) releasing the filled node after the same-class fill
    still double-frees. -/
theorem holeFill_same_class_release_after_fill_rejected :
    threeClauses
      (deriveLedger (fun _ => 0) 0
        [.construct 0, .construct 1, .holeFill 1 0, .burdenDec 1,
         .escapeUse 0 .Return]) = false := by
  decide

/-- §T2 clause 2 stays live across a fill: a non-transferring terminal read
    of the filled value AFTER its fill observes a drained count — rejected. -/
theorem holeFill_read_after_fill_rejected :
    threeClauses
      (deriveLedger (fun v => v) 0
        [.construct 0, .holeFill 0 1,
         .escapeUse 0 .LastReadBeforeScopeExit]) = false := by
  decide

/-- §T2 clause 3 stays live for the fill write: a hole-fill into a context
    whose class carries a live sibling read after the fill demands
    count >= 2 — a single birth fails the floor. -/
theorem holeFill_sibling_floor_enforced :
    threeClauses
      (deriveLedger (fun _ => 0) 0
        [.construct 0, .holeFill 1 0,
         .escapeUse 0 .LastReadBeforeScopeExit]) = false := by
  decide

end AimsProof
