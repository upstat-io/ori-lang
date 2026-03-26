---
reroute: false
name: "Ori UI Framework"
full_name: "Ori-Native GPU-Accelerated UI Framework — Research & Design"
status: research
---

# Ori UI Framework Index

> **Status:** Research phase — design exploration and feasibility analysis.
> **Thesis:** An Ori-native GPU UI framework with CSS layout semantics that showcases Deep FFI as its killer differentiator.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: oriterm Architecture Analysis
**File:** `section-01-oriterm-architecture-analysis.md` | **Status:** Research Complete

```
oriterm_ui, oriterm, oriterm_core, oriterm_mux, oriterm_tui
layout_box, SizeSpec, Hug, Fill, Fixed, FillRemaining
LayoutConstraints, LayoutNode, flex_solver, grid_solver
Scene, Quad, TextRun, LinePrimitive, IconPrimitive, ImagePrimitive
ContentMask, DrawCtx, PrepaintCtx, RectStyle
Widget trait, WidgetId, paint, layout, prepaint, lifecycle
InteractionManager, InteractionState, VisualStateAnimator
EventController, ClickController, DragController, HoverController
hit_test, dispatch, capture, bubble, Sense
Lerp, Easing, AnimProperty, AnimationGroup
wgpu, glyph_atlas, instance_buffer, damage_tracking
extract, prepare, render, pipeline
```

---

### Section 02: Pure Ori Layers
**File:** `section-02-pure-ori-layers.md` | **Status:** Research Complete

```
ori_layout, ori_scene, ori_widgets, ori_animation, ori_events, ori_state, ori_theme
flexbox, grid, CSS layout, BoxConstraints, constraint solver
position absolute, position relative, flex-wrap, flex-grow, flex-shrink
fr units, minmax, auto-fill, auto-fit, gap, margin collapse
Scene primitives, Quad, TextRun, content mask, clip stack
Widget trait, layout(), paint(), on_event(), prepaint()
Lerp trait, easing curves, CubicBezier, AnimProperty
hit testing, event dispatch, capture phase, bubble phase
reactive state, COW state, ARC reactivity
capability theming, with Theme, type-safe DSL
expression-based composition, declarative UI
```

---

### Section 03: FFI Wrapper Layers
**File:** `section-03-ffi-wrapper-layers.md` | **Status:** Research Complete

```
ori_gpu, ori_text, ori_window, ori_font
wgpu-native, WebGPU, webgpu.h, wgpuCreateInstance
HarfBuzz, hb_shape, hb_buffer, text shaping, glyph
FreeType, FT_Init_FreeType, FT_New_Face, font rasterization
SDL3, SDL_Init, SDL_CreateWindow, SDL_PollEvent
fontconfig, DirectWrite, CoreText, font discovery
swash, font table parsing, NOT recommended for port
extern "c", from "lib", CPtr, c_int, c_long
owned, borrowed, out params, #error, #free
glyph atlas, subpixel positioning, sRGB, damage tracking
SDF rounded corners, box shadows, gradients, blur
proportional text, line breaking, bidirectional, UAX #14
```

---

### Section 04: Deep Safety Showcase
**File:** `section-04-deep-safety-showcase.md` | **Status:** Research Complete

```
Deep FFI, parametric FFI, uses FFI("lib"), capability tracking
owned, borrowed, ownership annotation, #free, auto-Drop
#error(errno), #error(nonzero), #error(null), FfiError, Result wrapping
without clause, without Allocator, negative effects, denial
Boolean effect algebra, ICFP 2023, Lutze, effect exclusion
FFI mocking, with FFI("wgpu") = handler, testability
render thread alloc-free, static guarantee, compile-time
per-library capability isolation, provably pure layout
resource leak prevention, ownership-tracked GPU resources
capability-based theming, with Theme = dark in
```

---

### Section 05: Compiler Dependencies & Blockers
**File:** `section-05-compiler-dependencies.md` | **Status:** Research Complete

```
CPtr, c_int, c_long, c_size, c_char, C ABI types
type pool, TypeTag, FFI capability trait, prelude
extern block type checking, ModuleChecker, registration
LLVM declare, C calling convention, linker directives
ori_rt CPtr, string marshalling, callback trampolines
Section 6 capabilities, Section 11 FFI, roadmap tier 4
capability propagation, has_capability, uses clause
evaluator extern dispatch, Value enum, CPtr variant
```

---

### Section 06: Timeline & Milestones
**File:** `section-06-timeline-and-milestones.md` | **Status:** Research Complete

```
timeline, milestones, phases, dependency chain
Phase 0 FFI basics, Phase 1 Deep FFI, Phase 2 pure Ori
Phase 3 GPU wrappers, Phase 4 integration, Phase 5 showcase
parallel work streams, pure Ori layers, FFI wrappers
stress test, compiler validation, dogfooding
oriterm dogfooding, ori_ui_core, virtuous cycle
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | oriterm Architecture Analysis | `section-01-oriterm-architecture-analysis.md` |
| 02 | Pure Ori Layers | `section-02-pure-ori-layers.md` |
| 03 | FFI Wrapper Layers | `section-03-ffi-wrapper-layers.md` |
| 04 | Deep Safety Showcase | `section-04-deep-safety-showcase.md` |
| 05 | Compiler Dependencies & Blockers | `section-05-compiler-dependencies.md` |
| 06 | Timeline & Milestones | `section-06-timeline-and-milestones.md` |
