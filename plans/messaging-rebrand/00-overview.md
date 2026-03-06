---
plan: "messaging-rebrand"
title: "Messaging Rebrand & Testing Policy: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "README.md"
  - "website/src/components/landing/Hero.astro"
  - "website/src/components/landing/Features.astro"
  - "website/src/components/landing/VirtuousCycle.astro"
  - "website/src/pages/index.astro"
  - "website/src/layouts/BaseLayout.astro"
  - "website/public/og-image.svg"
  - "docs/ori_lang/v2026/spec/19-testing.md"
  - "docs/guide/01-getting-started.md"
  - "docs/guide/12-testing.md"
  - "blog/building-ori-from-scratch.md"
  - "compiler/oric/src/problem/semantic/mod.rs"
  - "compiler/oric/src/commands/check.rs"
  - "compiler/oric/src/commands/watch.rs"
  - "compiler/ori_diagnostic/src/errors/E3001.md"
  - "plans/roadmap/section-14-testing.md"
  - "plans/roadmap/00-overview.md"
  - "plans/roadmap/section-22-tooling.md"
---

# Messaging Rebrand & Testing Policy: Exhaustive Implementation Plan

## Mission

Reposition Ori's public-facing identity from "mandatory testing language" to "value semantics with unprecedented compiler optimization" — making the memory model the headline, effects the second pillar, and testing a powerful opt-in feature rather than a mandate. Simultaneously, make the test enforcement policy configurable so the compiler infrastructure (dependency-aware testing, capability-based mocking) stays intact while the mandate becomes project-level choice.

## Context: Why This Plan Exists

User group feedback on the published compiler was clear: people liked the effect system, the trait system, and the memory model, but found mandatory testing prescriptive and off-putting. The current messaging leads with "Code That Proves Itself" and positions mandatory testing as the headline feature. This is backwards — the memory model (8-layer ARC optimization pipeline) is Ori's strongest technical differentiator, and it's currently buried below the testing section in the README.

## Competitive Landscape Analysis

Every successful language leads with empowerment, not prescription:

| Language | Tagline | Leads with | Tone |
|----------|---------|-----------|------|
| **Rust** | "Empowering everyone to build reliable and efficient software" | Performance, Reliability, Productivity | Empowering |
| **Go** | "Build simple, secure, scalable systems with Go" | Simplicity, teams, built-in concurrency | Pragmatic |
| **Zig** | "Maintaining robust, optimal and reusable software" | Simplicity, no hidden control flow, comptime | Technical-honest |
| **Gleam** | "A friendly language for building type-safe systems that scale" | Reliability, tooling, helpfulness | Friendly |
| **Roc** | "A fast, friendly, functional language" | Fast, Friendly, Functional (3 words) | Approachable |
| **Swift** | "Fast. Expressive. Safe." | Performance, expressiveness, safety | Confident |
| **Kotlin** | "Concise. Multiplatform. Fun." | Developer joy, cross-platform | Professional-fun |
| **Elixir** | "Dynamic, functional language for scalable and maintainable apps" | Scalability, fault tolerance | Practical |
| **Vale** | "A fast, safe, and easy programming language" | No GC, no borrow checker | Positioning against Rust |
| **Koka** | "Functional language with effect types and handlers" | Effect system, Perceus RC | Academic-practical |

**Pattern**: Every language that succeeds leads with what it gives you, not what it demands of you. The closest comp to Ori's current messaging would be if Rust's tagline were "A language that forces you to handle all errors" — technically true but wrong framing.

## Design Principles

### 1. Lead with Empowerment, Not Prescription

Every messaging choice asks: "Does this tell developers what the compiler does FOR them, or what it requires OF them?" The memory model eliminates GC, borrow checker, and manual memory management — that's empowerment. Mandatory testing tells developers what they must do — that's prescription.

### 2. The Strongest Unique Feature Leads

The 8-layer ARC optimization pipeline has no equivalent in any shipping language. Lean 4 has 4 layers. Koka has 3. Swift has 2. This is the technical moat and should be the first thing developers encounter.

### 3. Infrastructure Stays, Policy Becomes Optional

The testing infrastructure (dependency-aware execution, capability-based mocking, test-driven PGO) is genuinely innovative and stays exactly as-is. Only the enforcement policy changes from "compiler error" to "configurable lint/warning."

## Section Overview

| Section | Focus | Dependencies |
|---------|-------|-------------|
| 01 | Competitive Analysis & Positioning Strategy | None |
| 02 | Testing Policy — Configurable Enforcement | None (but requires new config infrastructure; `oripk.toml` does not exist yet) |
| 03 | README Rewrite | Section 01 |
| 04 | Website Landing Page | Section 01, 03 |
| 05 | Spec & Documentation Updates | Section 01 (messaging alignment), Section 02 (policy changes) |
| 06 | Verification | All |

## Section Dependency Graph

```
Section 01 (Positioning) ──┬──→ Section 03 (README)
                           └──→ Section 04 (Website)

Section 01 (Positioning) ──────→ Section 05 (Spec/Docs — messaging alignment)
Section 02 (Testing Policy) ──→ Section 05 (Spec/Docs — policy changes)

Section 03 + 04 + 05 ──→ Section 06 (Verification)
```

- Sections 01 and 02 are independent (parallelizable)
- Sections 03 and 04 depend on 01 (positioning decisions must be made first)
- Section 05 depends on 01 AND 02 (spec needs policy; guide/blog/design docs need messaging)
- Section 06 depends on all (verification is last)

## Implementation Sequence

```
Phase 0 — Decision
  └─ Section 01: Finalize tagline, hero text, feature hierarchy, and tone
  └─ Section 02: Finalize testing policy (config format, error→warning behavior)

Phase 1 — Content & Compiler Prep (parallel, after Phase 0)
  ├─ Section 03: Rewrite README.md
  ├─ Section 04: Update website landing page components
  └─ Section 02 compiler prep: semantic/mod.rs split (BLOAT fix), E3001 collision fix

Phase 2 — Compiler & Docs (after Phase 1)
  └─ Section 02 implementation: severity switch, config threading (depends on Phase 0 + Phase 1 prep)
  └─ Section 05: Update spec, CLAUDE.md, guide, blog, design docs, proposals, skills, compiler comments

Phase 3 — Verification
  └─ Section 06: Review all changes, verify consistency, test build
```

**Why this order:**
- Phase 0 is pure decisions — no code changes
- Phase 1 parallelizes content work (README, website) with compiler prep (file split, error code fix) — these are independent
- Phase 2 requires Phase 0 decisions (to know what severity switch defaults to) and Phase 1 prep (split file, new error codes)
- Phase 3 gates release — nothing ships without verification

> **WARNING: Complexity risk.** The `oripk.toml` config system does not exist. Building it
> from scratch is a significant piece of infrastructure work (TOML parsing, config discovery,
> CLI flag merging, default resolution) that is likely to be the bottleneck. Consider
> implementing the CLI flag (`--test-enforcement`) first as a standalone change, then adding
> `oripk.toml` support later. This decomposes the risk.

## Estimated Effort

| Section | Est. Scope | Complexity |
|---------|-----------|------------|
| 01 Positioning Strategy | Document | Low |
| 02 Testing Policy | semantic/mod.rs split (prerequisite) + ~200 lines severity switch + config system (oripk.toml infra from scratch) + E3001 collision fix + tests | **High** |
| 03 README Rewrite | ~300 lines markdown | Low |
| 04 Website Landing Page | ~200 lines Astro/HTML + BaseLayout defaults + OG image SVG | Low-Medium |
| 05 Spec & Docs | ~300 lines across 20+ files (spec, guide, blog, design docs, proposals, skills, compiler comments) | Medium |
| 06 Verification | Review + testing + expanded grep scope | Low-Medium |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Positioning Strategy | `section-01-positioning.md` | Not Started |
| 02 | Testing Policy | `section-02-testing-policy.md` | Not Started |
| 03 | README Rewrite | `section-03-readme.md` | Not Started |
| 04 | Website Landing Page | `section-04-website.md` | Not Started |
| 05 | Spec & Documentation | `section-05-spec-docs.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |
