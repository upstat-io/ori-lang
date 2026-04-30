# Proposal: AIMS Spec Promotion

**Status:** Approved
**Author:** Eric (with Claude assistance)
**Created:** 2026-04-30
**Approved:** 2026-04-30
**Affects:** spec (Annex E), tooling (new `/sync-aims-spec` skill), documentation, in-tree rules (`canon.md §6`, `missions.md §AIMS`, `CLAUDE.md §AIMS`)
**Depends On:** *(none)*

---

## Summary

Promote the full AIMS lattice formalism — including target-only rules — into the public language spec under Annex E, giving OSS readers a citable destination for the memory-management design Ori provides. Annex E's INFORMATIVE designation is the framing that absorbs target-only rules: the spec section uses ISO/IEC normative voice (`shall` / `shall not` / `NOTE` / `EXAMPLE`) for clarity, while the annex's informative status keeps unshipped rules from imposing pre-shipping conformance on implementations. `.claude/rules/arc.md` + `.claude/rules/aims-rules.md` become the implementer's working surface that mirrors the spec content with prescriptive editorial style. Add a `/sync-aims-spec` skill — analogous to `/sync-grammar` — that produces a candidate spec section from the rules files; humans review the ISO/IEC voice transformation. The skill is wired into the pre-commit lefthook to fail on drift. The proposal does NOT change any language semantics; it organizes existing implemented and target behavior into a public documentation surface.

---

## Motivation

AIMS is a load-bearing language feature. The mission — "RC operations are rare in emitted code, not RC ops faster" — is the design center that lets Ori give programmers value semantics + ARC + zero ceremony, with hand-coded-C-class performance. AIMS spans seven product-lattice dimensions, interprocedural contracts (`MemoryContract`, `ParamContract`, `ReturnContract`, `EffectSummary`), a layered verification stack, and the FBIP / TRMC / immortal pre-pass / borrow inference subsystems.

### The Problem in Practice

Today the only places AIMS is documented are:

1. **`.claude/rules/arc.md`** (~309 lines) — the shipped surface overview, auto-loaded into Claude's context when ARC code is touched.
2. **`.claude/rules/aims-rules.md`** (~1003 lines) — the formal ruleset including target-only / unshipped subsystems.

Both files live in the private/internal `.claude/` tree. Public OSS readers cannot read them. Contributors landing in `compiler/ori_arc/` see code that enforces invariants (`AIMS Invariant 5`, `CN-3`, `RL-9`) but the invariants themselves are undocumented in any public surface.

The language has a major design feature with no public documentation. This is a coherence gap between the spec ("what Ori is") and the implementation ("what the compiler enforces").

### When This Matters

Three concrete scenarios surface this gap:

1. **Public OSS code review** — a recent external review of `compiler_repo` flagged ~150 references in compiler comments to private rule files (`impl-hygiene.md §X`, `aims-rules.md §Y`, `CLAUDE.md §AIMS Invariant N`). Phase 5/6 of the cleanup pass stripped these citations to remove the leak, but that left the technical claims standalone with no public destination to point at. The strip cleared the symptom; the spec promotion clears the cause.

2. **Contributor onboarding** — a contributor who reads `compiler/ori_arc/src/lib.rs` and sees comments referencing "AIMS Invariant 5" has no way to look up what that invariant says. The intel-graph layer can find related symbols, but the formal rule is private.

3. **Cross-language spec comparisons** — when comparing Ori's memory model to Rust / Swift / Koka / Lean 4, reviewers reach for the spec (which is public). Today the spec's Memory Model section (Clause 21) covers ARC at the syntactic / semantic level but not the AIMS analysis layer that makes ARC competitive. Ori's distinctiveness is invisible from the spec alone.

---

## Design

### Two-Tier Model

The AIMS surface splits along an audience boundary, with both tiers carrying the SAME content (the rules files mirror the spec):

| Audience | Voice | Destination |
|---|---|---|
| **Language users + OSS readers + external reviewers** | ISO/IEC normative (`shall` / `shall not` / `NOTE` / `EXAMPLE`) inside an Annex E section | Spec annex (public) |
| **Compiler implementers** | Prescriptive bullets ("Tools MUST NOT...", imperative form, in-tree commentary) | `.claude/rules/arc.md` + `.claude/rules/aims-rules.md` (working source) |

Both tiers carry the full lattice formalism (TF-1..TF-15, CN-1..CN-8, IC-1..IC-7, PL-1..PL-11, RL-1..RL-34, VF-1..VF-8). Annex E's INFORMATIVE designation handles target-only rules: a target-only rule in spec uses normative `shall` voice without locking the compiler into pre-shipping conformance, because Annex E carries implementation-defined-behavior framing. Implementers reading `aims-rules.md` see the same content with prescriptive editorial style suited to working-document workflow plus extra in-tree commentary that has no place in normative spec text (debugging tips, evolution notes, target-system rationale).

### Why Annex E (Informative)

Annex E is the right home because:

- Annex E is INFORMATIVE — sections in this annex describe implementation considerations, not user-facing requirements. Target-only AIMS rules fit this framing.
- Annex E already covers ARC Runtime, Heap Object Layout, Built-in Type Representations, Representation Optimization. AIMS extends the existing surface — the *intelligence layer* over the runtime substrate.
- AIMS's analog at the implementer-facing layer (`.claude/rules/repr.md`) already mirrors Annex E §Representation Optimization. The proposal extends this exact pattern to AIMS.
- A new top-level Clause 28 would imply normative weight; Annex E's informative status is what makes target-rule inclusion sound.

### Spec Destination

Add `§AIMS — ARC Intelligent Memory System` to `compiler_repo/docs/ori_lang/v2026/spec/annex-e-system-considerations.md`. Section structure:

```
§AIMS — ARC Intelligent Memory System
  §1 Mission and Design Center
  §2 Five Load-Bearing Invariants
  §3 Lattice Dimensions (Access × Consumption × Cardinality × Uniqueness × Locality × Shape × Effect)
  §4 Transfer Functions (TF-1..TF-15)
  §5 Canonicalization Rules (CN-1..CN-8)
  §6 Pipeline Ordering (PL-1..PL-11)
  §7 Interprocedural Contracts (IC-1..IC-7) — MemoryContract / ParamContract / ReturnContract / EffectSummary
  §8 Realization Rules (RL-1..RL-34)
  §9 Verification Layers (VF-1..VF-8) — structural / contract-consistency / oracle / FIP certification
  §10 Active Subsystems (RC elimination, FIP, TRMC, immortal pre-pass, borrow inference)
  §11 Target Subsystems (stack promotion, header compression, non-atomic RC, AIMS→LLVM fact export)
```

A header note inside §AIMS makes the informative-status framing explicit:

> NOTE  Annex E is informative. Rules in this section using `shall` / `shall not` document the AIMS algorithm and its invariants. Target subsystems documented in §11 describe design targets; implementations conforming to a given Ori build need not satisfy target rules until those subsystems ship.

### Sync Mechanism: `/sync-aims-spec` Skill

Following the precedent of `/sync-grammar` (which keeps `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf` in sync with the parser/lexer surface), but with a richer transformation step:

- **Direction**: `.claude/rules/arc.md` + `.claude/rules/aims-rules.md` → `compiler_repo/docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS`. **One-way.** Spec edits are gated by `/create-draft-proposal` → `/review-draft-proposal`; the proposal-gate IS the bidirectionality.
- **Transformation**: voice-rewriting (prescriptive bullets → ISO/IEC normative `shall` form, with `NOTE` / `EXAMPLE` blocks). The skill produces a candidate spec section; humans review and commit the polished version. ISO/IEC voice has subtle conventions (clause numbering, `shall` vs `should` vs `may` distinctions, `NOTE` placement) that templated rewrites tend to miss — human-in-the-loop is load-bearing.
- **Drift detection**: a coherence check verifies the spec content is derivable from the rules files (modulo voice). Drift between rule edits and spec edits is a `DRIFT:aims-spec-drift` finding.
- **Pre-commit gate**: `/sync-aims-spec --check` is wired into `compiler_repo/lefthook.yml`, mirroring the existing `intel-query-ssot` and `spec-proposal-gate` lefthook gates. Drift fails the commit.
- **Substantive changes**: still gated by `/create-draft-proposal` → `/review-draft-proposal`. Sync handles the routine "keep them equal" pass; new AIMS invariants land via proposal, then propagate through this skill.

### Why Sync Over `@`-Include Forwarder

An alternative architecture (Option A) would have `.claude/rules/arc.md` become a one-line `@compiler_repo/.../annex-e-aims.md` forwarder, with the spec section as the SSOT. Rejected because:

- The `@`-include mechanism is documented as working for skills (`.claude/skills/**/*.md`) and commands (`.claude/commands/**/*.md`), but its behavior on `paths:`-glob auto-loaded rule files is not documented or tested. If the harness does not recursively expand `@`-includes when auto-loading rules, Claude silently loses access to AIMS — a degraded-but-not-visibly-broken failure.
- Forced sync is robust against this uncertainty: both files contain the content; Claude reads the rule file as today; the spec mirrors it. Either failure mode (Claude can't read includes; spec drifts from rules) is detectable.
- Cost: dual maintenance of the rule content, mitigated by the sync skill being human-reviewed-but-skill-driven.

### Why Not Strip-Only (Status Quo After Phase 5/6)

Phase 5/6 of the leak-cleanup pass stripped citations to private rule files from compiler source comments. This eliminated the OSS leak (the original reviewer concern) but left:

- **No public destination** for future code citations to AIMS invariants
- **No public documentation** of AIMS guarantees for OSS readers
- **No coherence** between Ori's stated design center and what the public spec describes

The strip was an interim correctness fix; the spec promotion is the durable architectural fix.

---

## Alternatives Considered

### Alternative 1: `@`-include forwarder (Option A)

- `.claude/rules/arc.md` becomes a one-line `@compiler_repo/docs/.../annex-e-aims.md`. Spec is SSOT.
- **Rejected**: unproven harness behavior for `paths:`-auto-loaded rule files. Silent-degradation failure mode is the worst kind.

### Alternative 2: Per-rule `NOTE` wrappers tagging target-only rules inside an otherwise-normative section

- Spec section structured as normative-by-default with explicit `NOTE` blocks marking which rules are not-yet-shipped.
- **Rejected**: more editorial overhead per rule edit; readers must distinguish per-rule status rather than reading the whole annex's informative framing once. Annex E's INFORMATIVE designation already provides the framing — per-rule wrapping duplicates that work.

### Alternative 3: Shipped subset only (original proposal framing)

- Move only the shipped surface (5 invariants + mission + verification stack overview + active-subsystem summary) to spec; keep formal lattice in rules.
- **Rejected**: less public transparency; OSS readers and external reviewers cannot see Ori's roadmap from spec alone. Distinctiveness (immortal pre-pass, FBIP/TRMC progression, borrow inference) becomes invisible. Annex E's informative status is the cover that makes full enumeration sound.

### Alternative 4: Spec-only, no rule-file mirror

- Move all AIMS content to spec; delete `.claude/rules/arc.md` and `.claude/rules/aims-rules.md`.
- **Rejected**: rules files carry implementer-specific commentary (debugging tips, evolution notes, target-system rationale, in-tree review citations) that has no place in normative spec text. Compiler implementers lose the working surface optimized for their workflow.

### Alternative 5: New top-level Clause 28 instead of Annex E section

- Give AIMS its own clause, not an annex section.
- **Rejected**: a new clause implies normative weight on every rule; Annex E's INFORMATIVE designation is what makes target-rule inclusion sound. AIMS fits naturally with Annex E's existing system-level implementation-considerations content (ARC Runtime, Heap Object Layout, Built-in Type Representations, Representation Optimization).

### Alternative 6: Strip-only (status quo after Phase 5/6 cleanup)

- Leave the compiler source citation-free; don't add a public destination.
- **Rejected**: leaves the language without a public AIMS documentation surface. Future contributors and OSS reviewers have nothing to cite. The strip was a correctness fix; this proposal closes the architectural gap the strip exposed.

---

## Purity Analysis

**Can be pure Ori?** N/A — this is a documentation organization proposal. No language semantics, no syntax, no compiler behavior changes.

**Compiler changes required?** None. The compiler already implements AIMS. This proposal documents what it does (and where it's headed).

**Recommendation**: documentation + tooling proposal. Add spec section; create `/sync-aims-spec` skill; wire pre-commit drift gate; propagate to canon.md / missions.md / CLAUDE.md SSOT navigation surfaces. All non-semantic.

---

## Spec & Grammar Impact

### Spec Edits

- **Add**: new `§AIMS — ARC Intelligent Memory System` section to `compiler_repo/docs/ori_lang/v2026/spec/annex-e-system-considerations.md`, structured per §Spec Destination above (§§1–11).
- **Cross-reference**: Clause 21 (Memory Model) gets a forward-pointer to Annex E §AIMS at the ARC introduction.
- **No removals or edits to existing spec content.**

### Grammar Edits

None.

### New Error Codes

None.

### Rules File Updates (in-tree, non-spec — co-committed with spec section landing)

- `.claude/rules/canon.md §6 SSOT table` — split the AIMS row: spec section is SSOT for the algorithmic contract and lattice formalism; rules files are SSOT for implementer commentary and working-document content.
- `.claude/rules/missions.md §AIMS` — add Annex E §AIMS as Tier-A intent source alongside `CLAUDE.md §AIMS` and `canon.md §7.1`.
- `CLAUDE.md §AIMS` — add forward-pointer to Annex E §AIMS for the public-facing surface; preserve the in-tree summary as the dev-facing pointer.

### Pre-Commit Hook

Wire `/sync-aims-spec --check` into `compiler_repo/lefthook.yml`, mirroring the existing `intel-query-ssot` and `spec-proposal-gate` gates. Drift fails the commit.

---

## Roadmap Impact

Estimated 2-3 plan sections:

1. **Spec section authoring** — write the full Annex E §AIMS content per the §§1–11 structure above. Source material exists in `arc.md` + `aims-rules.md`; the work is full enumeration in ISO/IEC normative voice (~70 rules across TF / CN / IC / PL / RL / VF, plus dimension descriptions, plus invariants). 1 plan section.
2. **`/sync-aims-spec` skill + pre-commit gate** — build the voice-transforming sync skill (skill produces draft, human reviews voice transformation), wire drift detection, integrate with `compiler_repo/lefthook.yml`. 1 plan section.
3. **Propagation: `canon.md §6` + `missions.md §AIMS` + `CLAUDE.md §AIMS`** — co-committed with the spec section landing. May fold into §1 as a final subsection or sit as a standalone wrap-up.

---

## Migration / Breaking Changes

None. No existing code, configuration, or workflow changes. The compiler continues to enforce AIMS invariants exactly as today; the rules files continue to be auto-loaded for AI assistance; the only addition is a new public spec section + a sync skill + a lefthook gate.

Compiler source citations to AIMS rules were already cleared by Phase 5/6 of the public-OSS leak cleanup. Future citations can point to Annex E §AIMS once the spec section lands.

---

## Prior Art

The two-tier model — public language spec for users vs. compiler architecture docs for implementers — is the convergent pattern across mature language ecosystems:

- **Rust**: *The Rust Reference* (public language spec) + RFCs (proposal corpus) + *rustc-dev-guide* (compiler architecture / internals). Memory model details split across all three: borrow checker semantics in the Reference, ownership/lifetime design rationales in RFCs, MIR / borrowck implementation in dev-guide.
- **Swift**: *The Swift Programming Language* (book — language spec) + *swift/docs/* (compiler architecture docs including SIL ARC optimization details, ownership SSA design). Swift's ARC optimizer documentation is in `swift/docs/ARCOptimization.md` — the implementer-facing doc — while user-visible ARC behavior is in the book.
- **Lean 4**: language manual + source-level theorems + dev docs. Lean's RC insertion / borrow inference algorithms (the closest cross-language analog to AIMS) are documented in published papers (Ullrich & de Moura, IFL 2019; Reinking et al., PLDI 2021) plus implementer docs, NOT the language manual.
- **Koka**: language reference + Perceus papers (PLDI 2021, etc.) + repo docs. The FBIP analysis algorithm has a public paper but the implementation rules are in `Core/Borrowed.hs` and `Core/CheckFBIP.hs` source comments.

The convergent pattern: **observable contract in spec; formal algorithm in dev-guide / source / papers.** This proposal goes one step further than the convergent pattern by promoting the FORMAL algorithm into spec — leveraging Annex E's INFORMATIVE designation to do so without imposing pre-shipping conformance. The trade-off is in Ori's favor: AIMS is distinctive enough that surfacing the full algorithm publicly aids both contributor onboarding and external technical comparison.

The unique element Ori brings: AIMS is a *unified* framework — product lattice + interprocedural contracts + FBIP + TRMC + immortal pre-pass + borrow inference, all driven from one analysis. None of the reference compilers has this combination at this level of integration. The spec section makes that distinctiveness visible to readers; the formal rules stay co-located in the implementer-facing rules files for working-document workflow.
