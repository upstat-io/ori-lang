---
section: "06"
title: "Timeline & Milestones"
status: research-complete
reviewed: true
goal: "Define the phased implementation plan with concrete milestones and success criteria"
depends_on: ["01", "02", "03", "04", "05"]
---

# Section 06: Timeline & Milestones

**Status:** Research Complete
**Goal:** Integrate all dependency chains into a realistic phased plan with measurable milestones.

**Context:** The framework has two parallel development streams (compiler FFI + pure Ori layers) that converge when FFI wrappers connect to pure framework code. The timeline accounts for the current compiler state (audited 2026-03-25) and the dependency chain from Section 05.

---

## 06.1 Phase 0: Compiler Foundation (Month 1-2)

**Focus:** FFI basics land in the compiler. Pure Ori geometry and layout types begin.

### Compiler Track

- [ ] CPtr added to type pool
- [ ] C ABI types registered (c_int, c_long, c_char, c_size, c_float, c_double)
- [ ] Extern blocks type-checked (functions registered as callable)
- [ ] LLVM emits `declare` for extern functions
- [ ] C calling convention implemented (x86_64 SysV at minimum)
- [ ] Linker directives for library linking
- [ ] Runtime CPtr values + string marshalling

**Gate:** Can compile and run:
```ori
extern "c" from "SDL3" {
    @sdl_init (flags: c_int) -> c_int as "SDL_Init"
}
@main () -> void = {
    let result = sdl_init(flags: 0)
    print(msg: `SDL init returned: {result}`)
}
```

### Pure Ori Track (parallel)

- [ ] Geometry types: `Rect`, `Point`, `Size`, `Insets`, `Color`, `Transform2D`
- [ ] Layout descriptor types: `LayoutBox`, `SizeSpec`, `FlexLayout`, `GridLayout`, `LayoutConstraints`, `LayoutNode`
- [ ] Basic flex solver: row/column layout with `Hug`/`Fill`/`Fixed` sizing
- [ ] Easing functions: `Linear`, `EaseIn/Out/InOut`, `CubicBezier`
- [ ] Lerp trait + impls for `float`, `Color`, `Point`, `Size`, `Rect`

**Gate:** Can compute flex layout in pure Ori:
```ori
let box = LayoutBox { width: Fixed(400.0), height: Fixed(100.0),
    flex: Some(FlexLayout { direction: Row, gap: 8.0, ... }),
    children: [child1, child2] }
let node = compute_layout(box, Constraints.tight(w: 400.0, h: 100.0))
assert_eq(actual: node.children.len(), expected: 2)
```

### Milestone 0: "Hello SDL" + "Pure Ori Layout"
**Success criteria:** One Ori program calls a C function via FFI. Another Ori program computes flex layout without any FFI. Both compile and run correctly under AOT.

---

## 06.2 Phase 1: Deep FFI + Widget Framework (Month 2-3)

**Focus:** Deep FFI extensions land. Widget system and event handling take shape in pure Ori.

### Compiler Track

- [ ] `owned`/`borrowed` annotations on extern params
- [ ] `#error()` protocols (errno, nonzero, null, negative)
- [ ] `#free()` annotation with auto-generated Drop
- [ ] `out` parameters
- [ ] Parametric `FFI("lib")` capability
- [ ] FFI mocking (`with FFI("lib") = handler in { ... }`)

**Gate:** Can compile:
```ori
extern "c" from "freetype" #error(nonzero) #free(ft_done_face) {
    @ft_new_face (lib: borrowed CPtr, path: borrowed CPtr,
                  index: c_long, face: out CPtr) -> c_int as "FT_New_Face"
}
```

### Pure Ori Track (parallel)

- [ ] Scene primitives: `Quad`, `TextRun`, `LinePrimitive`, `ContentMask`, `RectStyle`
- [ ] Scene API: `push_rect()`, `push_text()`, `push_clip()`/`pop_clip()`, `push_offset()`
- [ ] Widget trait: `layout()`, `paint()`, `on_event()`, defaults
- [ ] Basic widgets: `text`, `button`, `flex` container, `spacer`, `divider`
- [ ] Event types: `MouseMove`, `MouseDown`, `MouseUp`, `KeyDown`, `KeyUp`, `Scroll`
- [ ] Hit testing: point-in-rect, clip-aware, returns hit path
- [ ] Two-phase event dispatch: Capture → Bubble
- [ ] InteractionState: hot, active, focused (framework-managed)
- [ ] Animation: `AnimProperty`, `VisualStateAnimator`, `AnimationGroup`

**Gate:** Can build a widget tree in pure Ori, compute layout, paint to a Scene, dispatch events, and animate state transitions — all without FFI.

### Milestone 1: "Framework in a Vacuum"
**Success criteria:** A complete button click flow works in pure Ori: create button widget → compute layout → paint to Scene → hit test → dispatch click event → update state → re-paint. All tested without any FFI or GPU.

---

## 06.3 Phase 2: FFI Wrappers + Integration (Month 3-4)

**Focus:** Connect pure Ori framework to native libraries via Deep FFI wrappers.

### FFI Wrapper Track

- [ ] `ori_gpu`: wgpu-native wrapper (instance, device, queue, buffer, pipeline, texture, shader)
- [ ] `ori_text`: HarfBuzz wrapper (buffer, shape, glyph info extraction)
- [ ] `ori_text`: FreeType wrapper (font loading, glyph rasterization)
- [ ] `ori_window`: SDL3 wrapper (window creation, event polling, DPI)
- [ ] `ori_font`: Platform font discovery (fontconfig on Linux first)
- [ ] Glyph atlas: Guillotine allocator + GPU texture management

### Integration Track

- [ ] Connect `ori_window` events to `ori_events` dispatch
- [ ] Connect `ori_scene` output to `ori_gpu` rendering
- [ ] Connect `ori_text` shaping to `ori_scene` text runs
- [ ] Frame loop: poll events → update state → layout → paint → submit

**Gate:** A window opens with a rendered button that responds to clicks.

### Milestone 2: "First Window"
**Success criteria:** An Ori program opens an SDL3 window, renders a "Hello Ori" button using wgpu, and the button responds to mouse clicks with a visual state change (hover highlight, press feedback).

---

## 06.4 Phase 3: Deep Safety Showcase (Month 4-6)

**Focus:** Implement `without` clause and demonstrate all six guarantees.

### Compiler Track

- [ ] Capability propagation completion (transitive checking)
- [ ] `without` clause parsing and type checking
- [ ] Denial propagation through call chains
- [ ] `with X = impl in` blocked inside `without X` contexts
- [ ] Error E1250: "capability X is denied in this context"

### Framework Track

- [ ] Add `without Allocator` to render submission path
- [ ] Verify compile error when allocating in render path
- [ ] Write FFI mock tests for GPU rendering
- [ ] Write FFI mock tests for text shaping
- [ ] Write FFI mock tests for windowing
- [ ] Build demo app showcasing all six guarantees
- [ ] Performance validation: measure frame timing, memory usage

### Polish Track

- [ ] CSS layout gaps: `position`, `flex-wrap`, percentage units
- [ ] Additional widgets: `text_input`, `checkbox`, `toggle`, `dropdown`, `scroll`
- [ ] Reactive state: `state()`, version tracking, change detection
- [ ] Capability-based theming: `with Theme = dark in { ... }`
- [ ] Smooth scroll physics
- [ ] Accessibility foundation (semantic tree)

**Gate:** All six guarantees from Section 04 are demonstrable with compiling Ori code.

### Milestone 3: "The Demo"
**Success criteria:** A non-trivial Ori GUI application (e.g., a counter app, a settings panel, or a simple text editor) that:
1. Compiles to a <10MB binary
2. Opens a window and renders widgets
3. Responds to keyboard and mouse input
4. Has `without Allocator` on the render path (compile-time verified)
5. Has FFI mock tests that run without a GPU
6. Uses per-library FFI capabilities (`FFI("wgpu")`, `FFI("harfbuzz")`, etc.)

---

## 06.5 Phase 4: Production Readiness (Month 6+)

### Framework Maturity

- [ ] Full CSS Flexbox specification
- [ ] CSS Grid (basic track sizing)
- [ ] Proportional text: line breaking (UAX #14), bidirectional (UAX #9)
- [ ] Font fallback chains
- [ ] Image loading and rendering
- [ ] Rich text (inline bold/italic/color)
- [ ] Clipboard integration
- [ ] File dialogs
- [ ] Accessibility (screen reader support)

### oriterm Dogfooding

- [ ] Extract `ori_ui_core` Rust crate from `oriterm_ui`
- [ ] Replace `oriterm_ui` internals with `ori_ui_core`
- [ ] Validate performance parity with original implementation
- [ ] Feed back performance insights to Ori compiler optimization

### Platform Expansion

- [ ] macOS support (Metal via wgpu, CoreText fonts)
- [ ] Windows support (DX12 via wgpu, DirectWrite fonts)
- [ ] WASM target (WebGPU, web fonts) — when Ori's WASM backend lands

---

## 06.6 Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| FFI implementation takes longer than estimated | Medium | High | Pure Ori work proceeds in parallel; FFI basics are the minimum viable gate |
| Proportional text rendering complexity | High | Medium | Use FreeType+HarfBuzz (proven); defer line breaking to Phase 4 |
| Ori compiler bugs surface during framework dev | High | **Positive** | This IS the goal — framework stress-tests the compiler |
| Performance gap vs Rust oriterm_ui | Medium | Low | ARC + COW should be competitive; profile and optimize compiler |
| wgpu-native API instability | Low | Medium | WebGPU spec is finalized; pin to stable release |
| Deep Safety (`without`) implementation delayed | Medium | Medium | Framework works without it; showcase delayed, not blocked |

---

## 06.7 Success Metrics

| Metric | Target | When |
|--------|--------|------|
| Pure Ori layout test passing | Yes | Month 2 |
| First C function called from Ori | Yes | Month 2 |
| Widget framework test suite (no FFI) | >50 tests | Month 3 |
| First window opened from Ori | Yes | Month 4 |
| First rendered button | Yes | Month 4 |
| FFI mock tests (no GPU) | >20 tests | Month 5 |
| `without Allocator` compiles | Yes | Month 5-6 |
| All six guarantees demonstrable | Yes | Month 6 |
| Demo app binary size | <10MB | Month 6 |
| Demo app startup time | <100ms | Month 6 |
| Compiler bugs found by framework dev | Track count | Ongoing |
