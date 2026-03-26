---
section: "02"
title: "Pure Ori Layers"
status: research-complete
reviewed: true
goal: "Design the pure Ori framework layers that have zero FFI dependencies — layout, scene, widgets, animation, events, state, and theming"
inspired_by:
  - "oriterm_ui layout/widget/animation architecture"
  - "Flutter BoxConstraints + RenderObject model"
  - "CSS Flexbox + Grid specification"
  - "Elm Architecture (state → view → update)"
  - "SwiftUI declarative composition"
depends_on: []
---

# Section 02: Pure Ori Layers

**Status:** Research Complete
**Goal:** Design the ~10,300 lines of pure Ori code that form the framework's core — layout, scene, widgets, animation, events, state, and theming. These layers have zero capabilities and the compiler can statically prove they never touch C code.

**Context:** The pure Ori layers are the majority of the framework. They port directly from oriterm_ui's Rust architecture but expressed in Ori's type system. Ori's expression-based syntax, pattern matching, traits, and capabilities make several patterns more natural than their Rust equivalents.

---

## 02.1 ori_layout — CSS Layout Engine (~2,000 lines)

**Port from:** `oriterm_ui/src/layout/`
**New additions:** CSS gaps identified in Section 01.2

### Core Types

```ori
// Layout descriptor — widgets produce these, solver consumes them
type LayoutBox = {
    width: SizeSpec,
    height: SizeSpec,
    min_width: Option<float>,
    max_width: Option<float>,
    min_height: Option<float>,
    max_height: Option<float>,
    padding: Insets,
    margin: Insets,
    flex: Option<FlexLayout>,
    grid: Option<GridLayout>,
    position: Position,
    z_index: int,
    clip: bool,
    children: [LayoutBox],
}

type SizeSpec = Hug | Fill | Fixed(value: float) | Percent(value: float)
    | FillRemaining(margin: float) | Calc(expr: SizeExpr)

type Position = Static | Relative(offset: Offset) | Absolute(anchor: Anchor)
    | Fixed(anchor: Anchor) | Sticky(threshold: float)

type FlexLayout = {
    direction: Direction,
    align: Align,
    justify: Justify,
    gap: float,
    wrap: bool,
    grow: float,
    shrink: float,
}

type GridLayout = {
    columns: GridColumns,
    row_gap: float,
    col_gap: float,
}

type GridColumns = Fixed(count: int) | AutoFill(min_width: float)
    | Template(tracks: [TrackSize])

type TrackSize = FixedTrack(value: float) | Fr(value: float)
    | MinMax(min: float, max: float) | Auto

// Constraints passed down from parent to child
type LayoutConstraints = {
    min_width: float,
    max_width: float,
    min_height: float,
    max_height: float,
}

// Resolved layout tree — output of solver
type LayoutNode = {
    bounds: Rect,
    content_bounds: Rect,
    children: [LayoutNode],
    widget_id: Option<WidgetId>,
    clip_rect: Option<Rect>,
    z_index: int,
}
```

### Solver Architecture

```ori
// Pure function — no side effects, no capabilities needed
@compute_layout (box: LayoutBox, constraints: LayoutConstraints) -> LayoutNode = {
    match box {
        { flex: Some(flex), .. } -> solve_flex(box, flex, constraints),
        { grid: Some(grid), .. } -> solve_grid(box, grid, constraints),
        _ -> solve_leaf(box, constraints),
    }
}
```

The solver is a pure function: `(LayoutBox, LayoutConstraints) -> LayoutNode`. Two-pass algorithm: measure children, then distribute space. Follows the CSS Flexbox specification for sizing, alignment, and space distribution.

### What Ori Makes Better Than Rust

- **Pattern matching on layout mode:** `match box { { flex: Some(flex), .. } -> ... }` is cleaner than Rust's nested if-let chains
- **Expression-based builders:** `LayoutBox { ...defaults, width: Fixed(100.0), flex: Some(row_layout) }` with struct spread
- **No mutability concerns:** Pure computation, no `&mut` threading through the solver

---

## 02.2 ori_scene — Scene Primitives (~1,000 lines)

**Port from:** `oriterm_ui/src/draw/scene/`
**New additions:** Rounded rects, gradients, shadows, blur

### Core Types

```ori
type Scene = {
    quads: [Quad],
    text_runs: [TextRun],
    lines: [LinePrimitive],
    icons: [IconPrimitive],
    images: [ImagePrimitive],
    clip_stack: [Rect],
    offset_stack: [(float, float)],
    opacity_stack: [float],
}

type Quad = {
    bounds: Rect,
    style: RectStyle,
    mask: ContentMask,
    widget_id: Option<WidgetId>,
}

type RectStyle = {
    fill: Option<Fill>,
    border: Option<Border>,
    shadow: Option<Shadow>,
    corner_radius: CornerRadius,
}

type Fill = Solid(color: Color) | LinearGradient(start: Point, end: Point, stops: [ColorStop])
    | RadialGradient(center: Point, radius: float, stops: [ColorStop])

type CornerRadius = Uniform(r: float) | PerCorner(tl: float, tr: float, br: float, bl: float)

type Shadow = {
    offset: (float, float),
    blur_radius: float,
    spread: float,
    color: Color,
    inset: bool,
}

type ContentMask = {
    clip: Option<Rect>,
    offset: (float, float),
    opacity: float,
}

type TextRun = {
    position: Point,
    shaped: ShapedText,
    color: Color,
    bg_hint: Option<Color>,
    mask: ContentMask,
    widget_id: Option<WidgetId>,
}
```

### Scene API

```ori
impl Scene {
    @new () -> Scene = Scene {
        quads: [], text_runs: [], lines: [], icons: [], images: [],
        clip_stack: [], offset_stack: [], opacity_stack: [],
    }

    @push_rect (self, bounds: Rect, style: RectStyle) -> void = {
        let mask = self.resolve_content_mask()
        self.quads = [...self.quads, Quad { bounds, style, mask, widget_id: None }]
    }

    @push_clip (self, rect: Rect) -> void = {
        self.clip_stack = [...self.clip_stack, rect]
    }

    @pop_clip (self) -> void = {
        self.clip_stack = self.clip_stack.slice(start: 0, end: self.clip_stack.len() - 1)
    }

    @resolve_content_mask (self) -> ContentMask = {
        let clip = if self.clip_stack.is_empty() then None
                   else Some(self.clip_stack.fold(initial: self.clip_stack[0], op: (a, b) -> a.intersect(b)))
        let offset = self.offset_stack.fold(initial: (0.0, 0.0), op: (a, b) -> (a.0 + b.0, a.1 + b.1))
        let opacity = self.opacity_stack.fold(initial: 1.0, op: (a, b) -> a * b)
        ContentMask { clip, offset, opacity }
    }
}
```

---

## 02.3 ori_widgets — Widget System (~5,000 lines)

**Port from:** `oriterm_ui/src/widgets/`

### Widget Trait

```ori
trait Widget {
    type State

    @layout (self, ctx: LayoutCtx) -> LayoutBox
    @paint (self, ctx: DrawCtx) -> void
    @on_event (self, event: Event, bounds: Rect) -> EventResult

    // Defaults
    @prepaint (self, ctx: PrepaintCtx) -> void = ()
    @is_focusable (self) -> bool = false
    @sense (self) -> Sense = Sense.none()
    @children (self) -> [impl Widget] = []
}
```

### Declarative Composition

```ori
// Expression-based syntax IS the component tree
@counter_app () -> impl Widget = {
    let count = state(0)

    flex(direction: column, align: center, padding: 16, gap: 12) {
        text(`Count: {count.value}`)
            .font_size(24)
            .color(theme.text.primary)

        flex(direction: row, gap: 8) {
            button("Decrement", on_click: () -> count.update(n -> n - 1))
            button("Increment", on_click: () -> count.update(n -> n + 1))
        }
    }
}
```

**Ori advantages over oriterm_ui's Rust:**
- No `Box<dyn Widget>` — Ori uses `impl Widget` existential types
- No `RefCell` for cached layout — Ori's value semantics + COW handle this naturally
- No closure boxing — Ori closures capture by value with ARC sharing
- No lifetime annotations — ARC handles lifetimes automatically
- `if condition then widget_a else widget_b` — conditional composition is just expressions

### Standard Widgets

| Widget | Description | Approximate Lines |
|--------|-------------|-------------------|
| `flex` | Flexbox container | ~200 |
| `grid` | Grid container | ~200 |
| `stack` | Z-stack (overlapping children) | ~100 |
| `text` | Static text display | ~150 |
| `button` | Clickable with visual states | ~200 |
| `text_input` | Editable text with cursor/selection | ~500 |
| `checkbox` / `toggle` | Binary state | ~150 each |
| `dropdown` | Overlay-based selection | ~300 |
| `scroll` | Scrollable container with scrollbar | ~400 |
| `dialog` | Modal overlay | ~200 |
| `divider` | Visual separator | ~50 |
| `spacer` | Flexible space | ~30 |
| `image` | Image display | ~100 |

---

## 02.4 ori_animation — Animation System (~500 lines)

**Port from:** `oriterm_ui/src/animation/`

### Core Design

```ori
trait Lerp {
    @lerp (self, target: Self, t: float) -> Self
}

impl float: Lerp {
    @lerp (self, target: float, t: float) -> float =
        self + (target - self) * t
}

impl Color: Lerp {
    @lerp (self, target: Color, t: float) -> Color =
        Color {
            r: self.r.lerp(target: target.r, t:),
            g: self.g.lerp(target: target.g, t:),
            b: self.b.lerp(target: target.b, t:),
            a: self.a.lerp(target: target.a, t:),
        }
}

type Easing = Linear | EaseIn | EaseOut | EaseInOut
    | CubicBezier(x1: float, y1: float, x2: float, y2: float)

@ease (easing: Easing, t: float) -> float = match easing {
    Linear -> t,
    EaseIn -> t * t,
    EaseOut -> t * (2.0 - t),
    EaseInOut -> if t < 0.5 then 2.0 * t * t else -1.0 + (4.0 - 2.0 * t) * t,
    CubicBezier(x1, y1, x2, y2) -> solve_cubic_bezier(x1, y1, x2, y2, t),
}

// Visual state animation — framework resolves interaction state → property values
type VisualStateAnimator<T: Lerp> = {
    current: T,
    target: T,
    progress: float,
    duration: Duration,
    easing: Easing,
}
```

---

## 02.5 ori_events — Event System (~800 lines)

**Port from:** `oriterm_ui/src/input/`, `oriterm_ui/src/controllers/`

### Hit Testing

```ori
@hit_test (root: LayoutNode, point: Point) -> Option<HitPath> = {
    // Walk layout tree, respect clip rects, find deepest widget containing point
    // Returns path from root to leaf for capture/bubble dispatch
}
```

### Two-Phase Dispatch

```ori
type EventPhase = Capture | Bubble

@dispatch_event (root: impl Widget, event: Event, hit_path: HitPath) -> EventResult = {
    // Phase 1: Capture — deliver root → leaf
    for widget in hit_path.ancestors() do {
        match widget.on_event(event, phase: Capture) {
            Handled -> break,
            Continue -> (),
        }
    }
    // Phase 2: Bubble — deliver leaf → root
    for widget in hit_path.from_leaf() do {
        match widget.on_event(event, phase: Bubble) {
            Handled -> break,
            Continue -> (),
        }
    }
}
```

### Composable Controllers

```ori
trait EventController {
    @on_event (self, event: Event, ctx: ControllerCtx) -> ControllerResult
    @on_lifecycle (self, event: LifecycleEvent, ctx: ControllerCtx) -> void = ()
}

// Attach multiple controllers to a widget
@my_widget () -> impl Widget = {
    button("Click me")
        .controller(on_click(handler: () -> print(msg: "clicked!")))
        .controller(on_hover(enter: () -> ..., leave: () -> ...))
        .controller(on_drag(start: ..., move: ..., end: ...))
}
```

---

## 02.6 ori_state — Reactive State (~600 lines)

### State Management

```ori
// State cell — triggers re-render when updated
type State<T> = { value: T, version: int }

@state<T> (initial: T) -> State<T> = State { value: initial, version: 0 }

impl<T> State<T> {
    @update (self, transform: (T) -> T) -> void = {
        self.value = transform(self.value)
        self.version = self.version + 1
        // Framework detects version change → schedules re-render
    }

    @set (self, value: T) -> void = self.update(transform: _ -> value)
}
```

**Ori's ARC + COW advantage:** When state updates, unchanged widget subtrees share memory via ARC. Only the changed path gets new allocations. This is what React's virtual DOM approximates with diffing — but at the memory level, with zero diffing overhead.

---

## 02.7 ori_theme — Capability-Based Theming (~400 lines)

### Design

```ori
// Theme is a capability — scoped, mockable, type-safe
type Theme = {
    text: TextTheme,
    color: ColorTheme,
    spacing: SpacingTheme,
    typography: TypographyTheme,
    border: BorderTheme,
    shadow: ShadowTheme,
    animation: AnimationTheme,
}

// Provide theme via capabilities — no prop drilling
@app () -> impl Widget uses Render = {
    with Theme = dark_theme() in {
        my_app_content()
    }
}

// Nested theme override — scoped to subtree
@settings_panel () -> impl Widget = {
    with Theme = high_contrast_theme() in {
        // This subtree uses high contrast
        settings_content()
    }
}
```

**No CSS cascade complexity.** Theme flows through capabilities — explicit, scoped, type-checked. No specificity wars, no `!important`, no inheritance surprises.
