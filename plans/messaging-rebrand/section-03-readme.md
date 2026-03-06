---
section: "03"
title: "README Rewrite"
status: complete
goal: "Rewrite README.md to lead with the memory model, reframe testing as smart opt-in"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "New README Structure"
    status: not-started
  - id: "03.2"
    title: "Section-by-Section Content"
    status: not-started
  - id: "03.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: README Rewrite

**Status:** Not Started
**Goal:** Rewrite `README.md` to lead with the memory model as the headline differentiator, position effects as the second pillar, and reframe testing as a powerful opt-in feature.

**Context:** The current README leads with "Code That Proves Itself" and puts mandatory testing before the memory model. The new README inverts this hierarchy based on user group feedback and competitive analysis.

**Depends on:** Section 01 (Positioning Strategy) — tagline, feature hierarchy, and narrative arc must be finalized first.

---

## 03.1 New README Structure

### Current Structure (what changes)

```
1. Hero: "Code That Proves Itself" (centered div)
2. "Why Ori?" → "Two Problems, One Design" (testing + memory framing)
3. "The Memory Model Nobody Else Has" — comparison table + 8-layer pipeline
4. "Mandatory Testing" — test syntax examples, "No exceptions. No skipping."
5. "Dependency-Aware Test Execution" — dep-graph explanation
6. "Explicit Effects & Trivial Mocking" — capability system
7. "Contracts" — pre()/post()
8. "The Design That Compounds" — virtuous cycle tree diagram
9. "Quick Start" — installation + first example
10. "Usage" — CLI commands
11. "Installation" — prerequisites + build
12. "Documentation" — links
13. "Design Philosophy" — comparison table ("Tests are optional | Tests are mandatory")
14. "Getting Help" / "Contributing" / "License"
```

### Proposed Structure

```
1. Hero: [Selected tagline from Section 01]
2. "The Memory Model Nobody Else Has" — PROMOTED TO LEAD
   - Comparison table (keep — it's strong)
   - "What You Get" (keep)
   - "How the Compiler Makes It Fast" — 8-layer pipeline (keep)
   - "How Ori Compares" table (keep)
3. "Effects You Can See" — capability system
   - Code example with with...in mocking
   - No framework, no DI container
4. "Smart Testing" — REFRAMED from mandatory
   - Dependency-aware execution (unique infrastructure)
   - Configurable enforcement (off/warn/error)
   - Test-driven optimization (PGO teaser)
5. "Contracts" (keep, minor)
6. "The Design That Compounds" — virtuous cycle (keep — it's strong)
7. "Design Philosophy" — REWRITE (currently references "mandatory testing")
8. Quick Start / Usage / Installation / Docs / Getting Help / Contributing / License (keep)
```

### Key Changes

1. **Hero text** — new tagline and subtitle from Section 01
2. **Memory model promoted** — moves from section 3 to section 2 (first content section)
3. **Testing reframed** — "Mandatory Testing" AND "Dependency-Aware Test Execution" merge into single "Smart Testing" section; examples show opt-in enforcement
4. **Effects promoted** — moves before testing
5. **"Two Problems, One Solution"** — removed (framing was testing-centric)
6. **Hero intro line** — `"...with mandatory testing, explicit effects, and a memory model..."` (line 9) — rewrite to lead with memory model
7. **Design Philosophy table** — row `"Tests are optional | Tests are mandatory"` changed to `"Tests are optional | Tests are smart"` or similar

- [ ] Draft new README following this structure
- [ ] Keep the comparison table and 8-layer pipeline (strongest content)
- [ ] Rewrite testing section with opt-in framing
- [ ] Update hero text based on Section 01 decisions

---

## 03.2 Section-by-Section Content

### Hero Section

Replace:
```markdown
**Code That Proves Itself**
Functional semantics. Imperative performance. Zero compromise.
A statically-typed, expression-based language with mandatory testing...
```

With (example using Option A tagline):
```markdown
**Write Pure. Run Fast.**
A statically-typed language with value semantics that compile to in-place mutations.
No garbage collector. No borrow checker. Eight compiler optimizations. Zero compromise.
```

### "Why Ori?" → Removed or Rewritten

The current "Two Problems, One Solution" framing centers testing. Replace with a direct lead into the memory model, or remove the intermediary section entirely and go straight to "The Memory Model Nobody Else Has."

### Memory Model Section — Keep, Promote

This section is already strong. Changes:
- Move to be the FIRST content section after hero
- Keep the comparison table
- Keep the 8-layer pipeline explanation
- Keep the "How Ori Compares" table
- Minor copy edits only

### Effects Section — Keep, Rename, Promote

Rename from "Explicit Effects & Trivial Mocking" to "Effects You Can See" or "Explicit Effects".
Move before testing.

### Testing Section — Reframe

Replace:
```markdown
## Mandatory Testing
Every function requires tests. No exceptions. No skipping. No "I'll add tests later."
```

With:
```markdown
## Smart Testing
Ori's testing infrastructure is built into the compiler — not bolted on as an afterthought.

### Dependency-Aware Execution
Tests live in the dependency graph. Change `@parse`, and tests for `@compile`
(which calls `@parse`) run automatically.

### Configurable Enforcement
Choose your policy per project:
- `test-enforcement = "off"` — tests optional (default)
- `test-enforcement = "warn"` — warnings for missing tests
- `test-enforcement = "error"` — full enforcement for production codebases
```

### Design That Compounds — Keep

The virtuous cycle section is strong. Only change: update the tree diagram to not mention "mandatory testing" — change to "smart testing" or "testing infrastructure."

### Design Philosophy — Rewrite

The current Design Philosophy section (line ~260 of README.md) contains:
- "Code that proves itself. Every function tested." — needs reframing
- A comparison table with "Tests are optional | Tests are mandatory" — needs "mandatory" removed
- "Ori makes verification and performance automatic" — can be kept with edits

This section should align with the new messaging: memory model + effects + smart testing.

- [ ] Write hero section content (tagline + subtitle from Section 01)
- [ ] Write "Effects You Can See" section with `with...in` mocking example
- [ ] Write "Smart Testing" section with dep-graph explanation and `test-enforcement` config example
- [ ] Rewrite "Design Philosophy" comparison table (remove "Tests are mandatory" row)
- [ ] Update "The Design That Compounds" tree diagram text (replace "mandatory" with "smart")
- [ ] Verify all code examples are valid Ori syntax (`ori check` on extracted snippets)
- [ ] Cross-reference ARC pipeline claims against `compiler/ori_arc/` implementation

---

## 03.3 Completion Checklist

- [ ] README.md rewritten with new structure
- [ ] Hero text matches Section 01 decisions
- [ ] Hero intro line (line 9) rewritten to lead with memory model, not testing
- [ ] Memory model is first content section
- [ ] "Mandatory Testing" and "Dependency-Aware Test Execution" merged into "Smart Testing"
- [ ] Testing reframed as "Smart Testing" with opt-in enforcement
- [ ] "Design Philosophy" comparison table updated (no "mandatory" row)
- [ ] "The Design That Compounds" tree diagram text updated
- [ ] All code examples valid Ori syntax
- [ ] All technical claims accurate
- [ ] Links (website, playground, spec, etc.) still work
- [ ] Comparison tables updated if needed
- [ ] No mention of "mandatory testing" as a requirement (mention as an option)
- [ ] `grep -n "mandatory" README.md` returns 0 results (or only in opt-in context)

**Exit Criteria:** `README.md` leads with the memory model, positions effects second, frames testing as smart and optional, and contains zero mentions of testing as a compiler requirement without the opt-in context.
