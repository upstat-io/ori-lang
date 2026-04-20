---
id: "01"
title: "AIMS FFI Contract Extension"
status: not-started
reviewed: false
goal: "Teach the AIMS lattice analysis to consume FFI ownership contracts as typed interprocedural facts, eliminating conservative RC retention at the FFI boundary while preserving Invariant #5 (unified model)."
success_criteria:
  - "extern declarations with `owned`/`borrowed`/`#error(...)` annotations produce a MemoryContract fragment (ParamContract + ReturnContract + EffectSummary) consumed by aims/interprocedural/extract.rs alongside native-Ori callees."
  - "An Ori program whose body is a chain of `owned CPtr`-returning FFI calls feeding `owned CPtr`-consuming FFI calls generates ZERO spurious RC operations in emitted LLVM IR (verified via FileCheck test matching RcInc/RcDec absence)."
  - "FIP certification (VF-4 / `aims-rules.md §5 IC-6`) extends to functions whose body is entirely FFI calls with `owned`/`borrowed` contracts; such functions can achieve FipContract::Certified."
  - "AIMS Invariant #5 (canon.md §7.1 / CLAUDE.md §AIMS / arc.md §Non-Negotiable Invariants) holds — the extension extends MemoryContract fields ONLY; NO parallel RC emission path, NO shadow escape enum, NO independent uniqueness tracker."
  - "Layered verification (VF-1 structural, VF-2 contract consistency, VF-3 oracle cross-check) passes clean on FFI-touching functions."
  - "Design log `plans/ffi-boundary-safety/section-01-design-log.md` exists and pins down EXACTLY which ParamContract / ReturnContract / EffectSummary fields carry FFI-origin facts, with citations to aims-rules.md rule anchors (IC-2, IC-3, IC-4, IC-5)."
inspired_by:
  - "Lean 4: `ownParamsUsingArgs` in `lean4/src/Lean/Compiler/IR/Borrow.lean` — treats borrow-hint propagation as contract-field updates, not a parallel pipeline."
  - "Swift: `lib/SILOptimizer/ARC/` — bridging-retain/bridging-transfer lowered into the same SIL lattice as native Swift."
  - "Koka: `src/Core/Borrowed.hs` — FFI borrow hints as effect-row annotations."
depends_on: []
third_party_review:
  status: none
  updated: null
---

# §01 — AIMS FFI Contract Extension

## Context

Approved proposal §1 (`docs/ori_lang/proposals/approved/ffi-boundary-safety-proposal.md`) states that AIMS today treats extern calls as opaque — assume-the-worst conservative — because extern declarations do not produce `MemoryContract` fragments. This section makes extern declarations first-class contract providers, so the existing interprocedural SCC fixpoint (IC-1 / IC-2 / IC-3 / IC-4 / IC-5) consumes them alongside native Ori callees.

**Invariant #5 is load-bearing here.** Any implementation approach that introduces a parallel structure (shadow contract table, separate FFI lattice, independent RC annotation pass) is out of scope AND will be rejected at `/tpr-review`. The extension MUST be additive on `MemoryContract` / `ParamContract` / `ReturnContract` / `EffectSummary`.

## §01.1 — Design Log FIRST (gate — do not implement before this)

Before any Rust changes land, write `plans/ffi-boundary-safety/section-01-design-log.md` answering:

- [ ] Which `ParamContract` field(s) carry the FFI-declared ownership (`owned` vs `borrowed`)? — default: reuse `ParamContract.access` (Owned/Borrowed) since that is the existing SSOT per `aims-rules.md §1.1`.
- [ ] Which `ParamContract` field carries `may_share` for FFI parameters? — default: reuse the existing `may_share` field on `ParamContract` per `aims-rules.md §5 IC-3`.
- [ ] Which `ReturnContract` field carries the FFI-declared return ownership + `preserves_freshness`? — default: reuse `ReturnContract.uniqueness` (`Unique` for `owned` returns from fresh C allocations) and `ReturnContract.preserves_freshness` (`true` for `owned` + known-fresh allocator; `false` for `borrowed` returns which copy immediately per approved Deep FFI).
- [ ] Which `EffectSummary` fields cover `#error(errno)` / `#error(nonzero)` / etc.? — default: `may_throw = true` when `#error(...) != none` (error paths produce `Result::Err`, which the caller's control-flow graph must account for); `may_allocate = true` when the callee is known to allocate (e.g., `sqlite3_open` creates a handle); `may_deallocate = true` for `#free(...)` functions.
- [ ] What new field (if ANY) is needed? — if none, note that explicitly. If one is needed, justify why it cannot be expressed on existing fields and verify the addition does not violate L-5 (finite height) or L-10 (monotone join).
- [ ] Cite each field mapping against `aims-rules.md` rule anchor (IC-2, IC-3, IC-4, IC-5) — reviewers will check these verbatim.

**Acceptance.** The design log is complete when it exhaustively lists every FFI-declarable annotation (`owned`, `borrowed`, `owned CPtr` return, `borrowed str` return, `borrowed [byte]` return, `#error(errno)`, `#error(nonzero)`, `#error(null)`, `#error(negative)`, `#error(success: N)`, `#error(none)`, `#free(fn)`, `out` param, `#borrow_from(p)` — the last deferred to §04 but the schema must accommodate it) and maps each to a concrete `MemoryContract` field.

**Failure mode.** If the design log cannot express an annotation using existing contract fields, STOP. Do not add a sibling structure. Either (a) propose a specific field addition with justification, or (b) file a bug per `CLAUDE.md §Bug Handling — ZERO DEFERRAL` and escalate via `AskUserQuestion`.

## §01.2 — ExternDecl → MemoryContract fragment emitter

Once §01.1 is signed off:

- [ ] Add `fn extract_extern_contract(decl: &ExternDecl) -> MemoryContract` in `compiler/ori_arc/src/aims/interprocedural/extract.rs` (NEW function; do NOT modify the existing `extract_function_contract` core logic — share helpers only).
- [ ] Wire into the SCC fixpoint: extern decls appear as nodes in the call graph at IC-1 with their contract pre-populated (no fixpoint iteration needed — externs have no bodies to analyze). Update `compute_aims_contracts()` entry point per `arc.md §Entry Points`.
- [ ] Each extern function's contract is produced ONCE at crate-type-check time and cached in the Salsa query layer (per `compiler.md §Salsa` purity requirements).
- [ ] Update `ori_arc/src/aims/contract/mod.rs` documentation comments to note FFI-origin contracts (reading-only — do not change field definitions unless §01.1 justified a new field).

## §01.3 — Call-site refinement (TF-6 interop)

- [ ] In the intraprocedural analysis (`compiler/ori_arc/src/aims/intraprocedural/mod.rs`), the existing TF-6 (`aims-rules.md §3`) `refine(CONSERVATIVE, callee.return_contract)` already handles any callee with a contract, regardless of origin. Verify with a targeted unit test that an extern callee's contract narrows the call-site `AimsState` correctly.
- [ ] Verify TF-11 argument demand refinement (`aims-rules.md §3 TF-11 Apply` row) fires correctly for FFI arguments: Borrowed `Absent` → zero demand; Owned `Absent` → `(arg, Once, Linear)` for RL-5 dec; locality/uniqueness/may_share propagation applies identically to FFI calls.
- [ ] NO new code path needed here — this subsection validates the existing refinement mechanics work for FFI contracts. If they don't, that's a bug in TF-6/TF-11, not a new "FFI refinement" code path.

## §01.4 — Realization phase validation

- [ ] Verify `realize_rc_reuse` (Step 5) and `realize_annotations` (Step 10) emit correct RC operations for FFI-touching functions. The realization doesn't branch on FFI-origin — it reads the lattice state, which §01.2 has populated via §01.3's refinement.
- [ ] Verify `verify_fip_contract` (Step 5a, `aims-rules.md §5 IC-6`) correctly certifies FFI-only functions: a function whose body is `let x = RSA_new(); sign(x, ...); RSA_free(x)` with all `owned`/`borrowed` contracts provided should reach `FipContract::Certified` (zero unmatched alloc/dealloc in realized IR).
- [ ] NO realization-level code changes expected. If changes ARE needed, flag as a design-log deviation and STOP for review.

## §01.5 — Matrix test coverage (MANDATORY)

Per `CLAUDE.md §Fix Completeness` and `tests.md §Matrix Testing Rule`:

- [ ] **Failing tests FIRST** (TDD order):
  - AOT FileCheck test in `compiler/ori_llvm/tests/aot/ffi_contract.rs` that asserts ZERO `RcInc`/`RcDec` in emitted LLVM IR for the canonical FFI owned-chain program.
  - AIMS snapshot test in `compiler/oric/tests/aims-snapshots/ffi_contract/` showing the contract fragment for an annotated extern decl.
- [ ] **Matrix dimensions** (must test ALL cells):
  - **Ownership × Error protocol:** `owned`/`borrowed` × `#error(errno)`/`#error(nonzero)`/`#error(null)`/`#error(negative)`/`#error(none)` = 10 cells.
  - **Return types:** `c_int`, `owned CPtr`, `borrowed CPtr`, `borrowed str`, `borrowed [byte]`, `Result<..., FfiError>` (auto-wrapped by `#error(...)` != `none`) = 6 cells.
  - **Argument patterns:** `owned CPtr` arg + dead elsewhere; `borrowed CPtr` arg + used after call; `owned CPtr` chained into another FFI call (ownership transfer); `borrowed str` + interior mutation attempt (should stay COW-safe).
- [ ] **Semantic pins:**
  - One Ori program `tests/spec/ffi/aims_zero_rc.ori` that ONLY compiles to zero-RC IR with §01 implementation; a FileCheck assertion (`CHECK-NOT: call void @ori_rc_inc` / `CHECK-NOT: call void @ori_rc_dec`) that fails if §01 is reverted.
  - One negative pin: a program that DOES require an RC op (e.g., an `owned CPtr` returned and assigned to a variable used twice) that MUST emit exactly one RcInc — fails if AIMS over-eliminates.
- [ ] **Verification layer matrix:**
  - VF-1 structural check clean on FFI-touching functions (`cargo test -p ori_arc -- verify::check_function`).
  - VF-2 `AbsentParamHasUses` does not false-positive on FFI absent params.
  - VF-3 oracle cross-check re-derives the FFI contract from realized IR and matches the declared contract (`cargo test -p ori_arc -- oracle`).
  - VF-4 FIP certification matrix: `FipContract::Certified`, `FipContract::Bounded(N)`, `FipContract::Conditional`, `FipContract::Never` each reachable via a test program with appropriate FFI call mix.
- [ ] **Dual-execution parity:** Interpreter and LLVM produce identical observable behavior for all FFI test programs (`ORI_CHECK_LEAKS=1` reports zero leaks).
- [ ] **Debug + release:** both build modes pass all tests.

## §01.6 — AIMS rule additions (documentation)

- [ ] Update `.claude/rules/aims-rules.md §5` to note that `ParamContract`/`ReturnContract`/`EffectSummary` accept FFI-origin contracts identically to native contracts — add a one-paragraph note near IC-2/IC-3 citing this section. Co-commit with §01.2.
- [ ] Update `.claude/rules/arc.md §"What AIMS does TODAY"` bullet list to mention FFI contract participation once §01 ships. Co-commit with §01.2.
- [ ] NO changes to the formal rules themselves — they already admit FFI contracts; this is a documentation sync.

## Completion Checklist

- [ ] §01.1 design log exists, cites aims-rules.md anchors, passes independent reading.
- [ ] §01.2 extract_extern_contract lands; Salsa-cached; unit-tested.
- [ ] §01.3 call-site refinement validated via unit tests.
- [ ] §01.4 realization validated; FipContract::Certified reachable for pure-FFI bodies.
- [ ] §01.5 matrix tests green (all cells), semantic pins green, negative pin green, both debug + release.
- [ ] §01.6 aims-rules.md + arc.md documentation sync committed.
- [ ] `./test-all.sh` green (full suite).
- [ ] `./clippy-all.sh` green (zero warnings introduced).
- [ ] `ORI_CHECK_LEAKS=1 ./binary` reports zero leaks on the FFI test programs.
- [ ] `/tpr-review` run to consensus (clean).
- [ ] `/impl-hygiene-review` run to consensus (clean).
- [ ] `/improve-tooling` section-close sweep run.
- [ ] `/sync-claude` section-close doc sync run.
- [ ] Plan status flipped: `sections[id=01].status: complete`; `reviewed: true`; overview Quick Reference updated.

## §01.R — Third Party Review Findings

- None.
