# Proposal: Reconcile Clause 23.3.2 Panic-Termination Model to the Unwind/Recoverable Drop Semantics

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-01
**Affects:** spec (Clause 23.3.2 Panic termination, Clause 23.4 Panic handler)
**Depends On:** drop-trait-proposal.md (approved 2026-01-30)

---

## Summary

Spec Clause 23.3.2 (`23-program-execution.md`) currently states an **abort model** for panic termination: "Drop implementations do not run during panic. Ori uses an abort model: panics terminate immediately without unwinding the stack." This directly contradicts the **unwind model** already established by the approved `drop-trait-proposal.md` and Clause 21.3.4 / 21.3.6 of the memory model, under which the stack unwinds on panic, `Drop` implementations run for live values during unwinding, and only a nested panic during unwinding (double panic) aborts. This proposal reconciles Clause 23.3.2 + 23.4 to the unwind model so the v2026 spec is internally consistent on the core Drop-during-panic execution model. No new semantic decision is introduced — the unwind model was already ratified; Clause 23.3.2 is stale text that was never synced.

---

## Motivation

The v2026 spec contradicts itself on what happens to destructors when a program panics. Two normative clauses give opposite answers:

- **Clause 23.3.2** (`23-program-execution.md:94`) — abort model:
  > `Drop` implementations do _not_ run during panic. Ori uses an abort model: panics terminate immediately without unwinding the stack.

  and `23-program-execution.md:117` (Clause 23.4):
  > A panic during a `Drop` implementation also causes immediate termination.

- **Clause 21.3.4** (`21-memory-model.md:243-251`) — unwind model:
  > If a destructor panics during normal execution (not already unwinding): 1. That panic propagates normally 2. Other values in scope still have their destructors run 3. Each destructor runs in isolation. If a destructor panics while already unwinding from another panic (double panic): 1. The program **aborts** immediately ...

- **Clause 21.3.6** (`21-memory-model.md:277`):
  > When a task is cancelled, destructors still run during unwinding.

The contradiction is decided by a higher authority: the **approved** `drop-trait-proposal.md` (2026-01-30) mandates the unwind model:

- `drop-trait-proposal.md:139` — "Drop runs synchronously during stack unwinding"
- `drop-trait-proposal.md:154-165` (§Must Not Panic During Unwind) — a drop-panic during unwinding (double panic) aborts; a single panic unwinds normally
- `drop-trait-proposal.md:391` summary table — "Panic in drop | Abort if during unwind"

An approved proposal that post-dates the spec text is the governing authority for resolving a spec-internal contradiction. Two of three spec surfaces (Clause 21.3.4 + 21.3.6) plus the approved proposal all agree on the unwind model; Clause 23.3.2's abort text is the lone outlier.

### Root cause (why the drift exists)

The approved `drop-trait-proposal.md` §"Spec Changes Required" enumerated edits to `07-properties-of-types.md` and `15-memory-model.md` (since reorganized into Clause 21) — but **omitted `23-program-execution.md`**. So when the Drop proposal landed, Clause 23.3.2's pre-Drop abort-model text was never updated. This proposal closes that gap.

### When This Matters

This blocks recoverable drop-panic-unwinding implementation work (the compiler cannot be made to honor a self-contradicting spec): a single `@drop` panic must run the remaining field-walk + free the allocation via a cleanup landing pad, then propagate — but Clause 23.3.2 forbids unwinding entirely. The compiler implementation, the AIMS drop pipeline, and the conformance test corpus all target the unwind model; only Clause 23.3.2 disagrees.

---

## Design

Rewrite Clause 23.3.2's panic-termination sequence + closing paragraph, and Clause 23.4's double-panic sentence, to the unwind model. The change is normative-text-only; no grammar, no operator semantics.

### Proposed Clause 23.3.2 (replacement)

Replace the current step list + abort paragraph with:

> On panic, the runtime executes the following sequence:
>
> 1. Construct a `PanicInfo` value with message, location, and stack trace
> 2. If an `@panic` handler is defined, call it (see 23.4)
> 3. Print the error message to stderr
> 4. Print the stack trace to stderr
> 5. Unwind the stack, running `Drop` implementations for all live values in reverse declaration order (LIFO), as specified in Clause 21.3.4
> 6. Exit with code 1
>
> On an unhandled panic the stack shall unwind and `Drop` implementations shall run for all live values during unwinding, as specified in Clause 21.3.4. A panic is not recoverable at the language level — Ori provides no catch mechanism (use `Result` for recoverable errors per Clause 17) — but destructors shall run during the unwind so that resources are released. A panic that occurs during a `Drop` implementation while the stack is already unwinding from a prior panic (a _double panic_) shall cause immediate termination with no further destructors run, per Clause 21.3.4.

The `NOTE` referencing Clause 17 (error-handling model) is retained.

### Proposed Clause 23.4 (double-panic sentence reconciliation)

Replace `23-program-execution.md:117`:

> A panic inside the `@panic` handler (a _double panic_) causes immediate termination with no further processing. A panic during a `Drop` implementation also causes immediate termination.

with:

> A panic inside the `@panic` handler causes immediate termination with no further processing. A panic during a `Drop` implementation while the stack is already unwinding from a prior panic (a _double panic_) shall cause immediate termination with no further destructors run, per Clause 21.3.4.

This narrows the over-broad "a panic during a `Drop` implementation also causes immediate termination" to the double-panic case the unwind model defines, keeping it consistent with Clause 21.3.4 (a single destructor-panic during normal execution propagates and sibling destructors still run; only a panic *during unwinding* aborts).

### `@panic`-handler ordering (review point)

The current sequence runs the `@panic` handler (step 2) before unwinding. This proposal preserves that ordering. Whether the handler should run before or after the unwind is a refinement reviewers may settle; the abort→unwind reconciliation does not depend on it.

---

## Alternatives Considered

### Alternative 1: Edit Clause 23.3.2 directly without a proposal

Rejected: all spec changes — including consistency fixes and typos — require an approved proposal before the normative text may be edited. This proposal is that required vehicle.

### Alternative 2: Change Clause 21.3.4 to the abort model instead

Rejected: the approved `drop-trait-proposal.md` ratified the unwind model; reversing it would be a new semantic decision contradicting an approved proposal, not a reconciliation. Two of three spec surfaces plus the proposal already agree on unwind.

---

## Purity Analysis

**Can be pure Ori?** N/A — this is a spec-document reconciliation, not a language feature. No compiler code, no stdlib, no grammar.
**Recommendation:** Proceed as a spec-text-only proposal. On approval, edit Clause 23.3.2 + 23.4 of `23-program-execution.md` to the proposed text with commit `Proposal: drop-panic-unwind-clause-reconciliation-proposal.md`.

---

## Spec & Grammar Impact

- `compiler_repo/docs/ori_lang/v2026/spec/23-program-execution.md` — Clause 23.3.2 step list + abort paragraph; Clause 23.4 double-panic sentence.
- No grammar change (`grammar.ebnf` untouched).
- No operator-rules change.
- No other clause changes: Clause 21.3.4 / 21.3.6 already state the target model and are the authority cited.

---

## Prior Art

Rust is the canonical reference for the two panic strategies. Rust's default is `panic = "unwind"`: a panic unwinds the stack, runs `Drop` for live values, then (for an unhandled panic in the main thread) terminates; a panic during unwinding triggers an immediate abort (`rust-lang/rust` panic-runtime; `std::panic`). Rust also offers an opt-in `panic = "abort"` mode that skips unwinding — but that is a build-profile choice, not the language's default destructor contract. Ori's approved `drop-trait-proposal.md` adopts the unwind-default + double-panic-abort semantics directly (proposal §Must Not Panic During Unwind mirrors Rust's double-panic abort). Swift runs `deinit` deterministically at last release but treats `fatalError`/trap as immediate abort with no unwinding — the opposite end; Ori chose Rust's unwind model in the approved proposal, so this reconciliation aligns the spec text with that already-made choice. (Prior-art entries to be verified against reference sources during `/review-draft-proposal`; intelligence graph was unavailable this session.)
