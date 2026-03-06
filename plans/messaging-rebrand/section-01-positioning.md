---
section: "01"
title: "Positioning Strategy"
status: complete
goal: "Finalize Ori's public identity: tagline, feature hierarchy, tone, and narrative arc"
depends_on: []
sections:
  - id: "01.1"
    title: "Feature Hierarchy — What Leads"
    status: not-started
  - id: "01.2"
    title: "Tagline & Hero Text"
    status: not-started
  - id: "01.3"
    title: "Feature Cards & Pillars"
    status: not-started
  - id: "01.4"
    title: "Narrative Arc"
    status: not-started
  - id: "01.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Positioning Strategy

**Status:** Not Started
**Goal:** Decide the exact tagline, feature hierarchy, tone, and narrative arc that will drive all content across README, website, docs, and future marketing.

**Context:** The current messaging leads with "Code That Proves Itself" and positions mandatory testing as the headline. User group feedback showed this was the least compelling feature to potential adopters. The memory model (8-layer ARC pipeline), effect system (capabilities), and trait system resonated strongly. This section decides what leads, what follows, and how the story flows.

---

## 01.1 Feature Hierarchy — What Leads

Ori's genuinely unique features, ranked by competitive differentiation:

### Tier 1: Nobody else has this (the moat)

1. **8-layer ARC optimization pipeline** — Type classification, interprocedural borrow inference, Perceus-style last-use cleanup, cross-block reset/reuse, surgical field updates, RC elimination, COW collections, compile-time uniqueness proofs. Lean 4 has 4 layers. Koka has 3. Swift has 2.

2. **Value semantics + capability-based effects** — Because there's no shared mutable state, the capability system can prove effects are the ONLY observable behavior change. `with Http = mock in { ... }` gives complete environment substitution. No mocking framework needed.

3. **Dependency-aware test execution** — Tests are nodes in the call graph. Change `parse()`, and tests for `compile()` (which calls `parse()`) automatically run. No other language or test framework does this at the compiler level.

### Tier 2: Rare (1-2 languages)

4. ARC without the pain (no GC, no borrow checker, no weak refs, no manual memory)
5. Function clauses with guards (Erlang/Gleam style)
6. Structured concurrency (nurseries with typed cancellation modes)
7. Contracts (`pre()`/`post()`) in a modern HM-inferred language
8. Deep FFI with declarative marshalling and capability gating

### Tier 3: Well-executed common (table stakes)

9. Expression-based, HM inference, algebraic types, pattern matching
10. `Duration`/`Size` literal types
11. Exact-version package management

**Decision:** Tier 1 features lead. Tier 2 features support. Tier 3 features are mentioned but not headlined.

- [x] Confirm feature hierarchy with user — **confirmed**: Tier 1 leads, Tier 2 supports, Tier 3 mentioned
- [x] "No GC, no borrow checker" **promoted to headline position** alongside 8-layer pipeline

---

## 01.2 Tagline & Hero Text

### Tagline Options

The tagline appears in the HTML `<title>`, the hero `<h1>`, and social media cards. It must be 2-7 words.

**Option A: "Write pure. Run fast."** (Recommended)
- Captures value semantics ("pure") and performance ("fast") in 4 words
- Echoes Roc's "fast, friendly, functional" brevity
- Implies: functional code compiles to fast code — the core technical claim
- Risk: "pure" might imply Haskell-level purity (Ori has effects)

**Option B: "Functional code. Imperative speed. Zero compromise."**
- Already the current subtitle — promotion to headline
- More explicit than Option A
- Risk: 6 words is longer; "zero compromise" is marketing-speak

**Option C: "No GC. No borrow checker. No compromise."**
- Directly addresses the known pain points of Go and Rust
- Immediately graspable by any systems programmer
- Risk: defines Ori by what it ISN'T rather than what it IS

**Option D: "Value semantics. Everything follows."**
- Most technically precise — the compounding design IS the story
- Appeals to PL enthusiasts and language nerds
- Risk: "value semantics" is jargon; non-PL developers won't understand

- [x] **Selected**: "Functional Code. Imperative Speed."

### Hero Subtitle Options

The subtitle appears below the tagline in smaller text. 1-2 sentences.

**Current:** "Every function tested. Every change traced. Every effect explicit."

**Proposed A:** "No garbage collector. No borrow checker. Eight compiler optimizations turn functional code into in-place mutations — automatically."

**Proposed B:** "Value semantics that compile to in-place mutations. Effects that make side effects visible. A memory model nobody else has."

**Proposed C:** "Write the code you want to write. The compiler writes the code you need to run."

- [x] **Selected**: "No garbage collector. No borrow checker. Eight compiler optimizations turn functional code into in-place mutations — automatically."

---

## 01.3 Feature Cards & Pillars

The website shows 4 feature cards. Current order: Invisible Memory, Mandatory Verification, Dependency-Aware Testing, Explicit Effects.

### Proposed Feature Cards (new order)

**Card 1: The Memory Model Nobody Else Has**
"No garbage collector. No borrow checker. No manual allocation. Eight stacked optimizations turn functional code into in-place mutations — zero compromise between safety and speed."

**Card 2: Effects You Can See**
"Side effects are explicit. Every function declares its capabilities. Mocking is built-in — just provide a different implementation with `with...in`. No frameworks. No DI containers."

**Card 3: The Design That Compounds**
"Value semantics enables safe memory, which enables complete effects, which enables trivial mocking, which makes testing painless. One decision, compounding returns."

**Card 4: Smart Testing**
"Tests live in the dependency graph. Change a function, and every affected test runs automatically. Your tests even make your binary faster — they're automatic profiling data for the optimizer."

Note: Card 2 renamed from "Explicit Effects" to "Effects You Can See" — more conversational. Card 4 reframed from "Mandatory Verification" to "Smart Testing" — empowering rather than prescriptive.

- [x] **Confirmed**: Memory → Effects → Compounding → Testing (4 cards)
- [x] Keep 4 cards

---

## 01.4 Narrative Arc

The story the README and website tell, in order:

```
1. HOOK:    What if functional code ran as fast as imperative code?
2. PROBLEM: Most languages make you choose — GC pauses, borrow checker, or manual memory.
3. CLAIM:   Ori eliminates all three. Value semantics + ARC + 8 stacked optimizations.
4. PROOF:   The comparison table. The 8-layer pipeline explanation. Code examples.
5. BONUS 1: Because there's no shared state, effects are complete. Capabilities track everything.
6. BONUS 2: Because effects are injectable, mocking is trivial. No frameworks needed.
7. BONUS 3: Testing is smart — dep-graph-aware, capability-mocked, optional but powerful.
8. UNIFY:   One design decision (value semantics). Compounding returns.
9. CTA:     Try it. Install. Read docs.
```

This puts the strongest unique feature first (memory model), explains why it enables the other features (effects, mocking, testing), and ends with the unifying design insight.

- [x] Narrative arc confirmed (as proposed)
- [x] Full 8-layer pipeline shown in README (it's strong content), link to ARC docs for details

---

## 01.5 Completion Checklist

- [x] Tagline selected and documented
- [x] Hero subtitle selected and documented
- [x] Feature card order and copy finalized
- [x] Narrative arc confirmed
- [x] Feature hierarchy confirmed
- [x] All decisions documented in this file for Sections 03-04 to consume

**Exit Criteria:** A single document (this file) with all positioning decisions locked in, ready for Section 03 (README) and Section 04 (Website) to implement without ambiguity.
