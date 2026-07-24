# Proposal: AIMS Spec Reframing — "Ori's AIMS Memory Model" Positioning

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-06-01
**Approved:** 2026-06-01
**Affects:** spec (Annex E §AIMS, informative)
**Related:** `aims-spec-promotion-proposal.md` (approved — established the `§AIMS` section in Annex E, including the `§1 Mission and Design Center` block this proposal reframes; provenance anchor, not a blocker)

---

## Summary

Reframe the public spec face — `compiler_repo/docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS` — from its current implementation-neutral framing to "Ori's AIMS memory model" positioning, with prior compilers (Lean 4, Koka, Swift, GHC, OxCaml, Clang, Racordon) cited as **historical influences** whose argument-shapes Ori adapted, NOT as architectural dependencies. This is the public-spec half of the AIMS proofing suite's global-reframing close-out: with all 173 AIMS theorems machine-checked and dual-discharged against Lean 4's independent kernel, the spec's §AIMS Mission and Design Center should reflect that AIMS is Ori's own, proven memory model. No normative rule changes — only the §AIMS mission/positioning prose in an informative annex.

---

## Motivation

The AIMS memory calculus (the 7-dimension product lattice + interprocedural contracts + transfer/decision/realization rules + verification layers) is now machine-checked sound: every one of its 173 theorems is proven by Ori's own proof checker AND independently re-proven by Lean 4's trusted kernel, with the two verdicts required to agree per-proof (dual-discharge gate). The calculus, the proofs, and the proof checker are all Ori's own contribution.

The implementer-facing documentation describing AIMS has already been reframed to reflect this ("Ori's AIMS memory model" with cited historical influences). The public spec face (`annex-e §AIMS`) is the remaining surface still framed implementation-neutrally — it does not assert Ori's ownership of the proven model, nor does it position the prior compilers it drew on as historical influences rather than architectural dependencies.

### The Problem in Practice

External readers of the spec encounter `§AIMS §1 Mission and Design Center` as a neutral description of "the compile-time intelligence layer," with no statement that the calculus is Ori's own proven contribution and no academic-honesty positioning of its design ancestry. Consumers cannot tell, from the spec alone, that AIMS is a machine-checked memory model Ori owns rather than an assembly of borrowed techniques.

### When This Matters

The spec is the public, external-facing artifact. Positioning AIMS correctly there — as Ori's proven memory model with cited historical influences — is the external-credibility surface for Ori's core "memory safety and C-level performance, the compiler's problem, provably" promise.

---

## Design

Editorial reframe of `annex-e §AIMS`, informative annex, no normative `shall`/`shall not` rule changes.

### Scope of edits

- `§AIMS §1 Mission and Design Center` opening: lead with "AIMS is Ori's AIMS memory model"; state that the calculus, soundness proofs, and proof checker are Ori's own contribution; cite prior compilers (Lean 4, Koka, Swift, GHC, OxCaml, Clang, Racordon) as historical influences whose argument-shapes Ori adapted, not as architectural dependencies.
- Preserve every normative rule (`§1`-`§9` dimension definitions, invariants, transfer functions, canonicalization, pipeline, realization, verification) verbatim — those are the technical content and are unchanged.
- Preserve academic-honesty attribution: historical influences are repositioned from architectural-dependency framing to design-ancestry framing, never erased.

### Semantics

No runtime, type-system, or compiler behavior change. The annex is informative; the edit changes positioning prose only.

### Error Handling

N/A — documentation positioning change.

---

## Alternatives Considered

### Alternative 1: Leave the public spec face unchanged

Rejected: leaves the highest-stakes external surface framed implementation-neutrally while every internal surface asserts Ori's proven ownership — an inconsistency that understates what is verified.

### Alternative 2: Reframe implementer docs only, never touch the spec

Rejected: the spec is the public, external-facing artifact; the internal-only reframe does not reach external readers, who consult the spec.

---

## Purity Analysis

**Can be pure Ori?** N/A — this is a documentation/positioning change to an informative spec annex, not a language or library feature.
**If not, why:** No code change of any kind. No compiler support, no stdlib addition.
**Missing features that would enable purity:** N/A.
**Recommendation:** Proceed as a spec-documentation change gated by the proposal process, because ALL edits under `compiler_repo/docs/ori_lang/v2026/spec/` — informative or normative — require an approved proposal.

---

## Spec & Grammar Impact

- **Affected:** `compiler_repo/docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §1 Mission and Design Center` (informative annex).
- **Grammar:** none.
- **Normative rules:** none changed; all `§AIMS` `shall`/`shall not` rules preserved verbatim.

---

## Prior Art

The reframe itself cites the historical design influences AIMS drew on, per academic-honesty convention (repositioned, never erased):

- **Lean 4 — Counting Immutable Beans** (Ullrich & de Moura, IFL 2019): binary borrow inference + Reset/reuse SHAPE.
- **Koka Perceus** (Reinking, Lorenzen, Leijen & de Moura, PLDI 2021): garbage-free RC with reuse + FBIP certification + TRMC SHAPE.
- **Racordon 𝒜-calculus** (Anzen / Hylo / mutable value semantics): multi-state reference lattice + transient ownership SHAPE.
- **Swift ARC optimizer**: KnownSafe + bidirectional dataflow SHAPE.
- **GHC Demand Analysis** (POPL 2014): Cardinality semiring SHAPE.
- **OxCaml Locality Modes** (ICFP 2024): Locality dimension SHAPE.
- **Clang/LLVM ObjC ARC**: PRE-style RC motion + COW contraction SHAPE.

Ori composes these shapes in a way no single prior system has (the 7-dimension product lattice + coexistence handshake), and the composed calculus is now machine-checked sound and dual-discharged against Lean 4's independent kernel.
