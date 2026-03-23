---
plan: "deep-safety"
title: "Deep Safety: Exhaustive Implementation Plan"
status: research
references:
  - "docs/ori_lang/v2026/spec/20-capabilities.md"
  - "docs/ori_lang/v2026/spec/26-ffi.md"
  - "docs/ori_lang/v2026/spec/21-memory-model.md"
  - "docs/ori_lang/proposals/approved/deep-ffi-proposal.md"
  - "docs/ori_lang/proposals/approved/unsafe-semantics-proposal.md"
  - "docs/ori_lang/proposals/drafts/negative-effect-without-proposal.md"
  - "docs/ori_lang/proposals/drafts/capability-propagation-completion-proposal.md"
  - "docs/ori_lang/proposals/drafts/unsafe-operation-gating-proposal.md"
  - "plans/deep-safety/research.md"
  - "plans/deep-safety/negative-effects-research.md"
  - "plans/deep-safety/01-lock-and-zerocopy-research.md"
---

# Deep Safety: Exhaustive Implementation Plan

## Mission

Make Ori the better choice than Rust for low-level development — including kernel dev, embedded, drivers, and systems programming — without sacrificing value semantics or expression-based design, and without forcing developers into binary `unsafe` escape hatches that abandon compiler verification.

## The Problem

Rust's safety model is binary: code is either fully safe (compiler verifies everything) or `unsafe` (compiler verifies nothing inside the block). In kernel development, `unsafe` appears in nearly every function — for FFI, MMIO, DMA, synchronization, callbacks, per-CPU variables, RCU, inline assembly, and more. When `unsafe` is everywhere, it stops being a meaningful safety boundary.

Ori's capability system provides the architectural foundation to replace binary `unsafe` with **graduated, capability-tracked, contract-verified low-level operations** where the compiler never stops helping.

## Core Thesis

> "In Rust, kernel code is mostly `unsafe` with comments explaining why it's correct. In Ori, kernel code uses specific capabilities with compiler-checked contracts. The compiler never stops verifying, even at the hardware level."

## Key Design Pillars

1. **Graduated capabilities, not binary unsafe** — Decompose `Unsafe` into specific low-level capabilities (`VolatileIO`, `RawMemory`, `InlineAsm`, `StaticMut`, `Transmute`, `InterruptCtx`, `DMA`, `Allocator`, etc.). Each is trackable, propagated through call chains, and paired with contracts.

2. **Contracts as proof obligations** — `pre()` and `post()` attach verifiable invariants to capability-gated operations. Runtime-checked today, statically verifiable later. Far beyond Rust's `// SAFETY:` comments.

3. **Type-safe hardware abstractions** — Typed newtypes (`PhysAddr`, `VirtAddr`, `MmioRegion`, `DmaBuffer<T>`, `UserPtr`) replace raw pointers. The compiler enforces bounds, alignment, and type correctness at the hardware boundary.

4. **Value semantics preserved** — The `Value` trait marks types that are inline, zero-ARC, bitwise-copyable. Kernel code operates primarily on `Value` types. ARC is available for complex structures but not required in hot paths.

5. **Capability composition enforces context rules** — `InterruptCtx` prohibits `Allocator` and `Suspend`. `PerCpuAccess` prohibits sleeping. The compiler enforces scheduling constraints that Rust can only check with external tools (Klint).

## Current Status

**Research complete. All major design questions resolved.** See `research.md` (2345 lines, 14 parts) for:
- Comprehensive taxonomy of all 35 `unsafe` categories in Linux kernel Rust code
- Scorecard with evidence column: 17 eliminated, 15 capability-safe, 3 residual
- Empirical data from CVE-2025-68260, USENIX ATC 2024, ACSAC 2024, Rudra, Asterinas
- 12 failed approach case studies with quantitative data and design principles
- Prior art analysis across 15+ languages/systems (Koka, F*, SPARK, Swift, Pony, Lean 4, etc.)
- **Concrete solutions** for all 7 previously open design questions

## Key Design Decisions (Resolved)

1. **Negative effects**: Boolean effect algebra with `without` clause (Lutze et al., ICFP 2023). Proven sound.
2. **Lock management**: Scoped APIs on existing `with()`. Type-level lock ordering via `LockBefore<L>`.
3. **Zero-copy**: Three layers — seamless slices (existing), callback-scoped views, second-class borrows.
4. **Capability granularity**: 14 capabilities in 4 domains, validated against 12 failure studies.
5. **Static verification path**: Liquid Haskell model — refinement types, Z3, 3-year roadmap.
6. **Kernel profile**: Capability-based enforcement via `Value` + `without Allocator`, not a separate mode.
7. **LKMM**: Residual capability — tracks non-standard model usage, no static verification.

## Draft Proposals

Three draft proposals implement the DNA-level architectural decisions from this research:

| Proposal | File | New Syntax? | Depends On |
|----------|------|-------------|------------|
| **Capability Propagation Completion** | `proposals/drafts/capability-propagation-completion-proposal.md` | No — completing existing spec | Nothing (prerequisite) |
| **Unsafe Operation Gating** | `proposals/drafts/unsafe-operation-gating-proposal.md` | No — activating E1250 | Propagation completion |
| **Negative Effects (`without`)** | `proposals/drafts/negative-effect-without-proposal.md` | Yes — `without` clause | Propagation completion |

## Next Steps

1. **Review and approve** the three draft proposals
2. **Phase 0**: Capability propagation completion + unsafe gating (prerequisite work)
3. **Phase 1**: `without` clause implementation (parser + type checker)
4. **Phase 2**: 4 core capabilities (`InterruptCtx`, `VolatileIO`, `DMA`, `Synchronization`)
5. **Phase 3**: VM NIC driver proof of concept
6. **Phase 4**: External design audit (Part 6 of research.md)

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| — | Research (main) | `research.md` | Complete (2345 lines, 14 parts) |
| — | Negative Effects Research | `negative-effects-research.md` | Complete (672 lines) |
| — | Lock & Zero-Copy Research | `01-lock-and-zerocopy-research.md` | Complete (800 lines) |
| — | Static Verification Research | `static-verification-research.md` | Complete (700 lines) |
| — | Failed Approaches Analysis | `failed-approaches.md` | Complete (925 lines) |
| 00 | Overview | `00-overview.md` | Complete |
