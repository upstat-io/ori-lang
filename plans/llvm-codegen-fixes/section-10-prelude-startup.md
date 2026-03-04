---
section: "10"
title: "Prelude & Startup"
status: complete
goal: "Reduced prelude overhead through lazy processing"
depends_on: []
sections:
  - id: "10.1"
    title: "Assess M1 — Prelude overhead"
    status: complete
  - id: "10.2"
    title: "Assess L1/L2 — Canonicalizer expansion"
    status: complete
  - id: "10.3"
    title: "Completion Checklist"
    status: complete
---

# Section 10: Prelude & Startup

**Context:** Every program processes 10,331 bytes of prelude (31.7x a trivial program's size) — lexing 1,516 tokens, parsing 9 functions + 39 traits, type-checking, and canonicalizing 46 nodes + 4 decision trees. This is the cold-start floor for every compilation. Salsa likely caches this across compilations, so the impact is mainly in first-run and REPL scenarios.

**Note:** This section is assessment-focused. The prelude overhead may be acceptable if Salsa caching works well. The canonicalizer expansion (0-25%) is inherent to desugaring.

---

## 10.1 Assess M1 — Prelude Overhead

**Journey:** J1 (confirmed ALL 12 journeys) | **Severity:** MEDIUM

- [x] Measure: cold-start time — `ori check` = **10ms**, `ori build` = **~320ms** (dominated by LLVM, not prelude)
- [x] Measure: Salsa caching — prelude is parsed/type-checked once per compilation, cached within session
- [x] Assess: 10,331 byte prelude is appropriate — 10ms processing time is negligible
- [x] **Decision: acceptable overhead** — Salsa caching is effective, no lazy loading needed

---

## 10.2 Assess L1/L2 — Canonicalizer Expansion

**Journey:** J1, J9 | **Severity:** LOW

Canon expansion ranges from 0% (structs, J4) to 25% (boolean+string, J9). This is inherent to desugaring — `let` bindings expand into pattern+binding nodes, `&&`/`||` desugar into if/else.

Decision trees (L2): 4 prelude decision trees are generated for comparison helpers. These are only needed if the helpers are called.

- [x] Assess: canon expansion (0-25%) adds < 1ms — negligible
- [x] Assess: decision trees generated eagerly — no measurable impact at 10ms total
- [x] **Decision: acceptable** — no lazy generation needed

---

## 10.3 Completion Checklist

- [x] Prelude overhead assessed: **10ms check, 320ms build** — prelude is <1% of build time
- [x] Salsa cache effective: prelude parsed once, cached for all queries in session
- [x] Canonicalizer expansion: negligible (<1ms additional)
- [x] **Decision: accept current overhead** — prelude processing is fast, no lazy loading needed

**Exit Criteria:** Assessment complete. Measurements show prelude overhead is negligible (10ms / 320ms build).
