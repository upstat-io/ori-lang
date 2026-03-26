---
plan: "ori-ui-framework"
title: "Ori-Native GPU-Accelerated UI Framework: Research & Design"
status: research
references:
  - "plans/deep-safety/"
  - "plans/roadmap/section-11-ffi.md"
  - "/home/eric/projects/ori_term/"
  - "docs/ori_lang/proposals/approved/deep-ffi-proposal.md"
  - "docs/ori_lang/proposals/approved/platform-ffi-proposal.md"
  - "docs/ori_lang/proposals/drafts/negative-effect-without-proposal.md"
---

# Ori-Native GPU-Accelerated UI Framework: Research & Design

## Mission

Build a **100% Ori-native** GPU-accelerated UI framework with CSS layout semantics that serves as the flagship showcase for Ori's Deep FFI capabilities. The framework proves that Ori can build production-grade systems software while demonstrating language features no other framework can match: parametric FFI capability tracking, ownership-annotated C interop, compile-time allocation denial on render paths, and full FFI mockability for testing without hardware.

## Thesis

**CSS layout + GPU rendering + native performance + type-safe DSL + Deep FFI safety = a framework that doesn't exist today.** Electron has CSS layout but ships 150MB of Chromium. Flutter has GPU rendering but its own unfamiliar layout model. SwiftUI is Apple-only. Tauri uses a webview. None of them can mock their rendering backend, prove their render path is allocation-free, or track C library usage as typed capabilities. Ori can do all of this.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              PURE ORI (zero capabilities)                │
│                                                          │
│  ori_layout    — CSS layout engine (flexbox, grid, pos)  │
│  ori_scene     — Scene primitives & building             │
│  ori_widgets   — Widget trait, components, containers    │
│  ori_animation — Lerp, easing, state animation           │
│  ori_events    — Hit testing, dispatch, focus            │
│  ori_state     — Reactive state management               │
│  ori_theme     — Capability-based theming                │
│                                                          │
│  Compiler PROVES these never touch C code                │
├─────────────────────────────────────────────────────────┤
│    SAFE FFI WRAPPERS (uses FFI("lib"), ownership)        │
│                                                          │
│  ori_gpu    uses FFI("wgpu")     #free(wgpuRelease...)   │
│  ori_text   uses FFI("harfbuzz") + FFI("freetype")       │
│  ori_window uses FFI("SDL3")     #error(negative)        │
│  ori_font   uses FFI(platform)   font discovery          │
│                                                          │
│  Ownership tracked (owned/borrowed)                      │
│  Errors auto-wrapped (Result<T, FfiError>)               │
│  Resources auto-freed (#free annotation)                 │
│  Fully mockable (with FFI("lib") = handler in { })       │
├─────────────────────────────────────────────────────────┤
│    RENDER PIPELINE (without Allocator on hot path)       │
│                                                          │
│  Frame: layout(uses Allocator) → paint(pure) →           │
│         submit(uses FFI("wgpu"), without Allocator)      │
│                                                          │
│  Compile-time guarantee: render path is alloc-free       │
└─────────────────────────────────────────────────────────┘
```

## Design Principles

### 1. CSS Semantics, Not CSS Syntax

Developers get familiar layout rules (flexbox, grid, box model) through Ori's type-safe DSL — not string-based stylesheets. Typos are compile errors. Layout properties are typed. The layout engine follows the CSS spec for _behavior_ but exposes it through Ori's expression-based syntax.

### 2. The Framework IS the Deep FFI Demo

Every `extern "c"` block in the FFI wrappers demonstrates a Deep FFI feature: `owned`/`borrowed` for GPU resource management, `#error(errno)` for FreeType calls, `#free(wgpuRelease)` for auto-Drop, `uses FFI("harfbuzz")` for per-library capability tracking, `with FFI("wgpu") = handler in { ... }` for GPU-free testing. The framework doesn't _use_ Deep FFI — it _showcases_ it.

### 3. Pure Core, Thin FFI Shell

The majority of the framework (~75% by lines) is pure Ori with zero capabilities. The compiler can statically prove that layout, scene building, widget logic, animation, event dispatch, and state management never call C code. Only the thin FFI wrappers (~25%) touch native libraries. This isn't a convention — it's a compiler guarantee.

## Section Dependency Graph

```
Section 01: oriterm Analysis ──────────────────────────────┐
                                                            │
Section 05: Compiler Dependencies ─────────────────────────┤
       │                                                    │
       ▼                                                    ▼
Section 02: Pure Ori Layers     Section 03: FFI Wrappers
       │                               │
       └──────────┬────────────────────┘
                  ▼
       Section 04: Deep Safety Showcase
                  │
                  ▼
       Section 06: Timeline & Milestones
```

- **Section 01** (oriterm analysis) is purely informational — it documents what transfers.
- **Section 02** (pure Ori) and **Section 03** (FFI wrappers) are independent design documents.
- **Section 04** (Deep Safety showcase) synthesizes 02 + 03 into the marketing/differentiation story.
- **Section 05** (compiler dependencies) maps what must land in the compiler before framework work can begin.
- **Section 06** (timeline) integrates all dependencies into a phased plan.

## Source Material

### oriterm_ui Codebase (explored 2026-03-25)

The `ori_term` project at `/home/eric/projects/ori_term/` contains a production-grade GPU UI framework (`oriterm_ui`) that is GPU-agnostic and highly transferable:

- **Layout engine** (`oriterm_ui/src/layout/`): Flexbox + grid subset with `LayoutBox` → `LayoutConstraints` → `LayoutNode` two-pass solver. ~2,000 lines.
- **Scene system** (`oriterm_ui/src/draw/scene/`): Type-separated primitive arrays (Quad, TextRun, Line, Icon, Image) with resolved ContentMask per primitive. ~800 lines.
- **Widget system** (`oriterm_ui/src/widgets/`): 105+ widget files, `Widget` trait with `layout()`, `paint()`, `prepaint()`, lifecycle events, composable `EventController`s. ~5,000 lines.
- **Animation** (`oriterm_ui/src/animation/`): Lerp trait, easing curves (CubicBezier with Newton's method), AnimProperty, VisualStateAnimator. ~500 lines.
- **Interaction** (`oriterm_ui/src/interaction/`): Framework-managed hot/active/focus state, VisualStateGroup. ~400 lines.
- **GPU pipeline** (`oriterm/src/gpu/`): Extract → Prepare → Render architecture, wgpu-based, glyph atlas with guillotine packing, subpixel positioning, damage tracking. ~3,000 lines.

### Deep Safety Plan (research complete 2026-03-22)

The `plans/deep-safety/` research plan (5,561 lines across 6 files) provides the theoretical foundation:

- **Parametric FFI**: `uses FFI("library")` — per-C-library capability tracking
- **Ownership annotations**: `owned`/`borrowed` on extern function parameters and returns
- **Error protocols**: `#error(errno|nonzero|null|negative|success:N)` — automatic Result wrapping
- **Auto-free**: `#free(fn)` — compiler-generated Drop impls for C resources
- **Negative effects**: `without Allocator` — compile-time denial of capabilities via Boolean effect algebra (Lutze et al., ICFP 2023)
- **FFI mocking**: `with FFI("lib") = handler { ... } in { ... }` — test without hardware

### Compiler FFI Status (audited 2026-03-25)

| Component | Status | Notes |
|-----------|--------|-------|
| Parser | **Complete** | `extern "c"`, `from`, `as`, variadics, all syntax |
| AST/IR | **Complete** | ExternBlock, ExternItem, ExternParam |
| Type System | **Missing** | No CPtr, no c_int/c_long, no FFI capability |
| Type Checker | **Missing** | Extern blocks parsed but not type-checked |
| Evaluator | **Missing** | Cannot call C functions |
| LLVM Codegen | **Missing** | No `declare`, no C ABI, no linker |
| Runtime | **Missing** | No CPtr values, no marshalling |
| Deep FFI | **Missing** | owned/borrowed, #error, #free — all unimplemented |
| Roadmap | Tier 4 | Section 11, 11 subsections, not-started |

## Estimated Scale

| Layer | Est. Lines (Ori) | Category |
|-------|------------------|----------|
| ori_layout | ~2,000 | Pure Ori |
| ori_scene | ~1,000 | Pure Ori |
| ori_widgets | ~5,000 | Pure Ori |
| ori_animation | ~500 | Pure Ori |
| ori_events | ~800 | Pure Ori |
| ori_state | ~600 | Pure Ori |
| ori_theme | ~400 | Pure Ori |
| **Pure Ori total** | **~10,300** | |
| ori_gpu | ~1,500 | FFI wrapper |
| ori_text | ~800 | FFI wrapper |
| ori_window | ~600 | FFI wrapper |
| ori_font | ~400 | FFI wrapper |
| **FFI wrapper total** | **~3,300** | |
| **Grand total** | **~13,600** | ~75% pure / ~25% FFI |

## C Library Dependencies

| Library | Purpose | API Style | FFI Complexity |
|---------|---------|-----------|----------------|
| wgpu-native | GPU (WebGPU C API) | ~80 functions, handle-based | Medium — many types |
| HarfBuzz | Text shaping | ~30 core functions, buffer-based | Low — clean C API |
| FreeType | Font rasterization | ~20 core functions, face/glyph lifecycle | Low — clean C API |
| SDL3 | Windowing + input | ~50 used functions, event-based | Medium — large event union |
| fontconfig (Linux) | Font discovery | ~15 functions, pattern-based | Low |
| DirectWrite (Windows) | Font discovery | COM-based | High — COM interop |
| CoreText (macOS) | Font discovery | CF-based | Medium — CF conventions |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | oriterm Architecture Analysis | `section-01-oriterm-architecture-analysis.md` | Research Complete |
| 02 | Pure Ori Layers | `section-02-pure-ori-layers.md` | Research Complete |
| 03 | FFI Wrapper Layers | `section-03-ffi-wrapper-layers.md` | Research Complete |
| 04 | Deep Safety Showcase | `section-04-deep-safety-showcase.md` | Research Complete |
| 05 | Compiler Dependencies & Blockers | `section-05-compiler-dependencies.md` | Research Complete |
| 06 | Timeline & Milestones | `section-06-timeline-and-milestones.md` | Research Complete |
