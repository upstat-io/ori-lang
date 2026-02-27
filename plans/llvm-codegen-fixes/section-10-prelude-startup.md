---
section: "10"
title: "Prelude & Startup"
status: not-started
goal: "Reduced prelude overhead through lazy processing"
depends_on: []
sections:
  - id: "10.1"
    title: "Assess M1 — Prelude overhead"
    status: not-started
  - id: "10.2"
    title: "Assess L1/L2 — Canonicalizer expansion"
    status: not-started
  - id: "10.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Prelude & Startup

**Status:** Not Started
**Goal:** Understand and reduce the prelude processing overhead. Assess whether canonicalizer expansion is a concern.

**Context:** Every program processes 10,331 bytes of prelude (31.7x a trivial program's size) — lexing 1,516 tokens, parsing 9 functions + 39 traits, type-checking, and canonicalizing 46 nodes + 4 decision trees. This is the cold-start floor for every compilation. Salsa likely caches this across compilations, so the impact is mainly in first-run and REPL scenarios.

**Note:** This section is assessment-focused. The prelude overhead may be acceptable if Salsa caching works well. The canonicalizer expansion (0-25%) is inherent to desugaring.

---

## 10.1 Assess M1 — Prelude Overhead

**Journey:** J1 (confirmed ALL 12 journeys) | **Severity:** MEDIUM

- [ ] Measure: cold-start time for a trivial program vs prelude-only processing
- [ ] Measure: Salsa cache hit rate for prelude on second compilation
- [ ] Assess: is the 10,331 byte prelude appropriate? Could unused traits be lazily loaded?
- [ ] Decision: if Salsa cache is effective, mark as acceptable overhead
- [ ] If Salsa cache is NOT effective: plan lazy prelude loading

---

## 10.2 Assess L1/L2 — Canonicalizer Expansion

**Journey:** J1, J9 | **Severity:** LOW

Canon expansion ranges from 0% (structs, J4) to 25% (boolean+string, J9). This is inherent to desugaring — `let` bindings expand into pattern+binding nodes, `&&`/`||` desugar into if/else.

Decision trees (L2): 4 prelude decision trees are generated for comparison helpers. These are only needed if the helpers are called.

- [ ] Assess: does canon expansion cause measurable performance impact?
- [ ] Assess: can decision trees be generated lazily (only when the function is called)?
- [ ] If impact is negligible: mark as acceptable

---

## 10.3 Completion Checklist

- [ ] Prelude overhead assessed with measurements
- [ ] Salsa cache effectiveness documented
- [ ] Canonicalizer expansion impact assessed
- [ ] Decision: accept current overhead OR plan lazy loading (with rationale documented)

**Exit Criteria:** Assessment complete with measurements. Decision documented.
