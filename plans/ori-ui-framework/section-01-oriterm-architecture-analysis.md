---
section: "01"
title: "oriterm Architecture Analysis"
status: research-complete
reviewed: true
goal: "Map oriterm_ui's architecture to identify what transfers directly, what needs adaptation, and what must be built new for a general-purpose GPU UI framework"
inspired_by:
  - "Zed GPUI — GPU-accelerated UI framework in Rust"
  - "Flutter BoxConstraints — constraint-based layout model"
  - "React/SwiftUI — declarative component composition"
depends_on: []
---

# Section 01: oriterm Architecture Analysis

**Status:** Research Complete
**Goal:** Comprehensive mapping of oriterm_ui's architecture showing what transfers directly to ori_ui, what needs adaptation, and what gaps must be filled.

**Context:** The `ori_term` project at `/home/eric/projects/ori_term/` contains a production-grade GPU UI framework (`oriterm_ui`) built in Rust. It is already GPU-agnostic — the layout engine, widget system, animation, interaction management, and event dispatch have zero GPU dependencies. The terminal-specific parts (terminal grid rendering, PTY integration) are confined to `oriterm_core` and `oriterm/src/gpu/prepare/`. This means the framework is already general-purpose in architecture — it just doesn't know it yet.

---

## 01.1 Workspace Structure

**Source:** `/home/eric/projects/ori_term/`

The workspace has 5 crates with one-way dependency flow:

| Crate | Purpose | GPU dependency? | Transfers? |
|-------|---------|-----------------|------------|
| `oriterm_core` | Terminal emulation (grid, VTE, selection, search) | None | No — terminal-specific |
| `oriterm_mux` | Multiplexing (split trees, domains, IPC, sessions) | None | No — terminal-specific |
| `oriterm_ui` | GPU-agnostic UI framework (layout, widgets, animation) | **None** | **Yes — 100%** |
| `oriterm` | Main GUI app (GPU rendering, windowing, PTY, fonts) | wgpu | Partially — GPU pipeline |
| `oriterm_tui` | TUI client (terminal-in-terminal) | None | No — terminal-specific |

**Key insight:** `oriterm_ui` is the transfer target. It has zero GPU, zero terminal, zero platform dependencies. Everything in it is pure computation.

---

## 01.2 Layout Engine — 100% Transferable

**Source:** `oriterm_ui/src/layout/`

### Current Capabilities

| Abstraction | CSS Equivalent | File |
|---|---|---|
| `LayoutBox` + `Direction::Row/Column` | `display: flex; flex-direction` | `layout_box.rs` |
| `Align` (Start/Center/End/Stretch) | `align-items` | `flex.rs` |
| `Justify` (Start/Center/End/SpaceBetween/SpaceAround) | `justify-content` | `flex.rs` |
| `SizeSpec::Hug` | `width: fit-content` | `layout_box.rs` |
| `SizeSpec::Fill` | `flex: 1` / `width: 100%` | `layout_box.rs` |
| `SizeSpec::Fixed(f32)` | `width: Npx` | `layout_box.rs` |
| `min_width/max_width/min_height/max_height` | Same CSS properties | `layout_box.rs` |
| `padding/margin` via `Insets` | Same CSS properties | `layout_box.rs` |
| `gap` | `gap` | `flex.rs` |
| `GridColumns::Fixed(n)` / `AutoFill` | `grid-template-columns` | `grid_solver.rs` |
| `clip` / `overflow` | `overflow: hidden` | `layout_box.rs` |
| `LayoutConstraints` | Flutter's `BoxConstraints` | `constraints.rs` |

### Architecture

- **Input:** `LayoutBox` (declarative descriptor tree — sizing, padding, margin, flex/grid mode, children)
- **Solver:** `compute_layout(box, constraints) -> LayoutNode` — pure function, two-pass (measure → distribute)
- **Output:** `LayoutNode` (resolved `Rect` per widget, clip rects, widget IDs)
- **Key rule:** Widgets NEVER call `compute_layout()` — the framework does. Widgets only produce `LayoutBox` descriptors.

### CSS Gaps (must be added for full CSS support)

| Feature | Current | Needed |
|---------|---------|--------|
| `position: absolute/relative/fixed/sticky` | Not implemented | Required for overlays, modals, tooltips |
| `z-index` stacking contexts | Not needed (flat terminal) | Required for layered UI |
| Margin collapsing | Not implemented | CSS spec requires it |
| `flex-wrap` | Not implemented | Required for responsive layouts |
| `flex-grow`/`flex-shrink`/`flex-basis` as separate props | Simplified (`Fill`/`Hug`/`Fixed`) | CSS spec has finer control |
| Percentage units | Not implemented | `width: 50%` |
| `calc()` | Not implemented | `width: calc(100% - 32px)` |
| CSS Grid full track sizing (`fr`, `minmax`, `auto-fill/fit`) | Partial (Fixed/AutoFill only) | Full spec |
| CSS Grid named areas | Not implemented | Nice-to-have |

**Assessment:** The foundation is architecturally correct. Adding CSS features is additive — the solver structure, constraint model, and two-pass algorithm are right. The gaps are features, not redesigns.

---

## 01.3 Scene / Draw System — 100% Transferable

**Source:** `oriterm_ui/src/draw/`

### Scene Structure

```
Scene {
    quads: Vec<Quad>              — filled/bordered/shadowed rectangles
    text_runs: Vec<TextRun>       — pre-shaped glyph runs
    lines: Vec<LinePrimitive>     — line segments
    icons: Vec<IconPrimitive>     — monochrome atlas icons
    images: Vec<ImagePrimitive>   — textured rectangles

    clip_stack: Vec<Rect>         — internal state
    offset_stack: Vec<(f32, f32)> — internal state
    opacity_stack: Vec<f32>       — internal state
    layer_bg_stack: Vec<Color>    — internal state
}
```

**Key design:** Type-separated primitive arrays. Each primitive carries a resolved `ContentMask` (clip + offset + opacity baked in). The GPU consumer just iterates typed arrays — no stack interpretation at render time.

**Drawing API:** `push_rect()`, `push_quad()`, `push_border()`, `push_text()`, `push_line()` + stack commands (`push_clip()`, `push_offset()`, `push_opacity()` and `pop_*()` counterparts).

**Primitives for general UI that need addition:**
- Rounded rectangle (SDF-based corner rounding)
- Box shadow (Gaussian or SDF shadow)
- Gradient fills (linear, radial, conic)
- Blur / backdrop filter (multi-pass)
- Rounded-rect clipping (SDF clip in shader)

---

## 01.4 Widget System — 90% Transferable

**Source:** `oriterm_ui/src/widgets/` (105+ files)

### Widget Trait

```rust
pub trait Widget {
    // Core
    fn id(&self) -> WidgetId;
    fn is_focusable(&self) -> bool;
    fn sense(&self) -> Sense;                    // What interactions matter
    fn hit_test_behavior(&self) -> HitTestBehavior;

    // Lifecycle
    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox;
    fn prepaint(&mut self, ctx: &mut PrepaintCtx<'_>);
    fn paint(&self, ctx: &mut DrawCtx<'_>);
    fn lifecycle(&mut self, event: &LifecycleEvent, ctx: &mut LifecycleCtx<'_>);
    fn anim_frame(&mut self, event: &AnimFrameEvent, ctx: &mut AnimCtx<'_>);

    // Events
    fn controllers(&self) -> &[Box<dyn EventController>];
    fn on_input(&mut self, event: &InputEvent, bounds: Rect) -> OnInputResult;
    fn on_action(&mut self, action: WidgetAction, bounds: Rect) -> Option<WidgetAction>;

    // Composition
    fn for_each_child_mut(&mut self, visitor: &mut dyn FnMut(&mut dyn Widget));
}
```

### Transferable Widgets

| Widget | Transfers? | Notes |
|--------|------------|-------|
| Container (flex/grid) | Direct | Generic layout container |
| Button | Direct | Click handling + visual states |
| Label / RichLabel | Direct | Text display |
| TextInput | Direct | Text editing with cursor/selection |
| Checkbox / Toggle | Direct | Toggleable state |
| Dropdown / Menu | Direct | Overlay-based selection |
| Scrollbar / Scroll | Needs adaptation | Smooth scroll physics needed |
| Dialog | Direct | Modal overlays |
| Panel / SettingsPanel | Direct | Multi-section containers |
| TabBar | Direct | Tab navigation |
| NumberInput | Direct | Spinner control |
| Terminal grid widget | Does not transfer | Terminal-specific |
| PTY panel | Does not transfer | Terminal-specific |

### Event System

**Two-phase dispatch:** Capture (root → leaf) then Bubble (leaf → root).
**Composable controllers:** `ClickController`, `DragController`, `HoverController`, `ScrollController`, `FocusController`, `TextEditController` — attached to widgets, not inherited.
**Framework-managed state:** `InteractionManager` tracks hot/active/focus per widget. Widgets declare what they sense (`Sense::click()`, `Sense::hover()`) but don't manage state.

---

## 01.5 Animation System — 100% Transferable

**Source:** `oriterm_ui/src/animation/`

- **Lerp trait:** Linear interpolation for `f32`, `Point`, `Size`, `Rect`, `Transform2D`, `Insets`, `Color`
- **Easing:** Linear, EaseIn/Out/InOut, CubicBezier with Newton's method solver
- **AnimProperty:** Embeddable in widgets, manages active animation lifecycle
- **VisualStateAnimator:** Couples visual states (Normal/Hovered/Pressed/Disabled) to property animations
- **AnimationGroup:** Multi-property transitions with timing coordination

All pure math. Zero terminal dependency.

---

## 01.6 GPU Pipeline — 80% Transferable

**Source:** `oriterm/src/gpu/`

### Architecture: Extract → Prepare → Render

1. **Extract** (`gpu/extract/`): Lock snapshot, copy frame data, unlock. Zero mutation.
2. **Prepare** (`gpu/prepare/`): Pure CPU — convert terminal grid + UI scene to GPU instances. Testable without GPU.
3. **Render** (`gpu/window_renderer.rs`): wgpu submission.

### Current GPU Pipelines

| Pipeline | Instance Size | Purpose |
|----------|---------------|---------|
| Background | 96 bytes | Solid-color quads |
| Foreground (R8) | 96 bytes | Monochrome atlas glyphs |
| Subpixel FG (RGBA) | 96 bytes | LCD subpixel text (ClearType-style) |
| Color FG (RGBA) | 96 bytes | Emoji/color glyphs |
| UI Rect | 144 bytes | UI widget rectangles |

### Rendering Features Already Built

- Glyph atlas with guillotine packing (multi-page, 2048x2048)
- Subpixel positioning with fractional offset quantization
- sRGB ↔ linear conversion for correct color blending
- Per-row damage tracking, instance caching, skip-present when idle
- Mica/Acrylic transparency (Windows), vibrancy (macOS)
- Embedded WGSL shaders with vertex pulling via `@builtin(vertex_index)`

### Rendering Features Needed for General UI

| Feature | Status | Difficulty |
|---------|--------|------------|
| Rounded corners | Not in terminal (brutalist) | Low — SDF shader, well-documented |
| Box shadows | Not needed | Medium — Gaussian blur pass or SDF |
| Gradients (linear/radial/conic) | Not needed | Low — fragment shader |
| Proportional text | Not in terminal (monospace only) | **High** — HarfBuzz + line breaking |
| Rich inline text | Not needed | Medium — run-level shaping |
| SVG / vector graphics | Not needed | Medium-High — Vello or tessellation |
| Blur / backdrop filter | Not needed | Medium — Kawase or Gaussian multi-pass |
| Rounded-rect clipping | Not needed | Low — SDF clip in shader |

**The biggest gap is proportional text.** Monospace text is trivial (fixed-width grid). Variable-width fonts require: HarfBuzz shaping, line breaking (Unicode UAX #14), bidirectional text (UAX #9), font fallback chains. This alone is 3-4 months of work.

---

## 01.7 Transfer Summary

| Category | Lines (Rust) | Est. Lines (Ori) | Transfer % |
|----------|-------------|-------------------|------------|
| Layout engine | ~2,000 | ~1,500 | 100% (port) |
| Scene/draw system | ~800 | ~600 | 100% (port) |
| Widget framework | ~5,000 | ~4,000 | 90% (port, drop terminal widgets) |
| Animation | ~500 | ~350 | 100% (port) |
| Interaction/events | ~1,200 | ~900 | 100% (port) |
| GPU pipeline arch | ~3,000 | ~1,500 | 80% (adapt for Ori FFI) |
| **Total** | **~12,500** | **~8,850** | |

**New code needed (not in oriterm):**
- CSS layout gaps (position, flex-wrap, fr units, etc.): ~1,500 lines Ori
- Proportional text rendering wrapper: ~800 lines Ori
- Rich scene primitives (rounded rects, shadows, gradients): ~500 lines Ori
- Reactive state management: ~600 lines Ori
- Capability-based theming: ~400 lines Ori

**Total estimated:** ~12,650 lines Ori for the full framework.
