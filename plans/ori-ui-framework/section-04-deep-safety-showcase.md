---
section: "04"
title: "Deep Safety Showcase"
status: research-complete
reviewed: true
goal: "Document how the UI framework demonstrates every Deep FFI feature, making it the flagship proof of Ori's safety model"
inspired_by:
  - "plans/deep-safety/ — Boolean effect algebra, parametric FFI, without clause"
  - "Servo (Rust) — framework that validated the language's type system"
  - "Flutter (Dart) — framework that made the language relevant"
depends_on: ["02", "03"]
---

# Section 04: Deep Safety Showcase

**Status:** Research Complete
**Goal:** Document the six safety guarantees that no other UI framework can make, and how the framework's architecture naturally demonstrates them.

**Context:** The UI framework isn't just built with Deep FFI — it IS the Deep FFI demo. Every architectural decision was made to showcase a capability that no other language provides. When someone evaluates Ori, they should see this framework and understand immediately why Ori's approach to native interop is fundamentally different.

---

## 04.1 The Six Guarantees

### Guarantee 1: Layout Engine is Provably Pure

```ori
// The compiler can PROVE this function never calls C code
@compute_layout (box: LayoutBox, constraints: Constraints) -> LayoutNode = {
    // Pure Ori — no FFI capability in scope
    // No uses clause → compiler verifies no transitive FFI calls
    solve_flex(box, constraints)
}
```

**What this means:** The entire layout engine (~2,000 lines) has no `uses` clause. The compiler statically verifies that it never calls any C function, directly or transitively. This isn't a convention — it's a type-level guarantee.

**No other framework can claim this:**
- Flutter's layout engine is in Dart, which is GC-managed and calls Skia for text measurement
- SwiftUI's layout engine is opaque Apple code
- Electron's layout engine IS Chromium

### Guarantee 2: GPU Resources Can't Leak

```ori
extern "c" from "wgpu_native" #free(wgpuBufferRelease) {
    @wgpu_device_create_buffer (
        device: borrowed CPtr,
        desc: borrowed CPtr
    ) -> owned CPtr as "wgpuDeviceCreateBuffer"
}

// Ori auto-generates Drop impl:
// impl Drop for the returned CPtr value:
//   @drop (self) -> void = wgpuBufferRelease(self.handle)

// Usage — resource CANNOT leak:
@create_vertex_buffer (device: GpuDevice, vertices: [Vertex]) -> GpuBuffer
    uses FFI("wgpu") = {
    let buffer = wgpu_device_create_buffer(device.handle, desc)?
    // If this function panics here, buffer is still released via auto-Drop
    upload_data(buffer, vertices)
    GpuBuffer { handle: buffer, size: vertices.len() * vertex_size }
}
```

**What this means:** Every GPU resource (buffers, textures, pipelines, shader modules) has compiler-generated cleanup via `#free`. Forgetting to release a resource is impossible — the compiler inserts the release call.

**Compared to other frameworks:**
- OpenGL: Manual `glDeleteBuffers` — forget and you leak
- Vulkan: Manual `vkDestroyBuffer` — forget and you leak
- Metal: ARC-managed, but Objective-C ARC, not language-level
- Rust/wgpu: Manual `Drop` impl — correct but boilerplate

### Guarantee 3: C Errors Can't Be Silently Ignored

```ori
extern "c" from "freetype" #error(nonzero) {
    @ft_new_face (lib: borrowed CPtr, path: borrowed CPtr, index: c_long,
                  face: out CPtr) -> c_int as "FT_New_Face"
}

// Call site — the error is AUTOMATICALLY wrapped as Result:
@load_font (path: str) -> Result<Font, FfiError>
    uses FFI("freetype") = {
    let face = ft_new_face(lib, path, 0)?  // Auto-checked, auto-wrapped
    // If FreeType returns nonzero, this is Err(FfiError { code, message, source })
    // The ? propagates it. Ignoring the error is a compile error (unused Result).
}
```

**What this means:** The `#error(nonzero)` annotation tells the compiler that nonzero return values are errors. The compiler automatically wraps every call as `Result<T, FfiError>`. You can't accidentally ignore a font loading failure.

**Compared to other frameworks:**
- C: Check return value manually — forget and you get silent corruption
- Rust: Return `Result` manually — correct but boilerplate
- Go: `if err != nil` — easy to forget
- Java: Checked exceptions — verbose, often caught and swallowed

### Guarantee 4: Render Path Can't Allocate (via `without`)

```ori
// The render submission MUST be allocation-free for consistent frame timing
@submit_frame (scene: Scene, device: GpuDevice, queue: GpuQueue) -> void
    uses FFI("wgpu")
    without Allocator = {
    // Calling ANY function that allocates = COMPILE ERROR
    // This is enforced transitively through the entire call chain

    let pipeline = device.cached_pipeline()     // OK — returns cached
    let instances = scene.to_gpu_instances()     // OK — pre-allocated buffer
    queue.submit(commands: [draw_call(pipeline, instances)])  // OK

    // let temp = [1, 2, 3]  ← COMPILE ERROR: Allocator denied in this context
    // format("debug: {scene}")  ← COMPILE ERROR: format allocates
}

// The layout phase CAN allocate — it runs before render
@compute_frame (root: impl Widget, constraints: Constraints) -> Scene
    uses Allocator = {
    let layout = compute_layout(root.layout_box(), constraints)  // Allocates freely
    let scene = Scene.new()
    root.paint(DrawCtx { scene, layout })  // Allocates freely
    scene
}
```

**What this means:** The `without Allocator` clause is a compile-time denial. Any function in the render submission path that transitively calls anything requiring `Allocator` is a type error. This guarantees consistent frame timing — no GC pauses, no allocation jitter, no surprise latency spikes.

**No other framework can make this guarantee:**
- Electron: JavaScript GC can pause at any time during rendering
- Flutter: Dart GC can pause (mitigated but not eliminated)
- SwiftUI: Swift ARC can trigger deallocation cascades during rendering
- Qt: C++ allocation in render path is convention, not enforced

**Based on:** Boolean effect algebra (Lutze et al., ICFP 2023) — proven sound with effect safety theorem guaranteeing excluded effects never execute.

### Guarantee 5: Everything is Testable Without Hardware (via FFI Mocking)

```ori
@test tests @render_button_without_gpu () -> void = {
    // Mock the entire GPU backend — no GPU, no window, no hardware
    with FFI("wgpu") = handler {
        wgpuCreateInstance: (desc) -> mock_instance(),
        wgpuDeviceCreateBuffer: (dev, desc) -> mock_buffer(desc.size),
        wgpuQueueSubmit: (queue, count, cmds) -> record_submission(cmds),
    } in {
        with FFI("SDL3") = handler {
            SDL_CreateWindow: (title, w, h, flags) -> mock_window(),
            SDL_PollEvent: (event) -> 0,  // No events
        } in {
            let scene = button("Click me").paint(mock_draw_ctx())

            // Assert on scene primitives, not pixels
            assert_eq(actual: scene.quads.len(), expected: 1)
            assert_eq(actual: scene.text_runs.len(), expected: 1)
            assert_eq(actual: scene.text_runs[0].text, expected: "Click me")
        }
    }
}

@test tests @layout_is_pure_no_mocking_needed () -> void = {
    // Layout is pure Ori — no FFI, no mocking, no setup
    let box = flex(direction: row, gap: 8) {
        fixed(width: 100, height: 50),
        fixed(width: 200, height: 50),
    }
    let node = compute_layout(box, Constraints.tight(w: 400, h: 100))

    assert_eq(actual: node.children[0].bounds.x, expected: 0.0)
    assert_eq(actual: node.children[1].bounds.x, expected: 108.0)  // 100 + 8 gap
}
```

**What this means:** Every C library dependency can be mocked at the language level using `with FFI("lib") = handler { ... } in { ... }`. You can test the entire rendering pipeline — scene building, GPU submission, event handling — without a GPU, without a window, without any hardware.

**No other framework can do this:**
- Flutter: Can't mock Skia — needs a real rendering backend or golden image testing
- SwiftUI: Can't mock Metal — snapshot testing requires a running app
- Electron: Can't mock Chromium — headless mode still uses real rendering
- Qt: Can't mock OpenGL — needs a display server or virtual framebuffer

### Guarantee 6: Each C Dependency is Isolated (Parametric FFI)

```ori
// The compiler tracks WHICH C libraries each function uses
@create_window (title: str) -> Window
    uses FFI("SDL3") = { ... }                    // Only SDL3

@shape_text (font: Font, text: str) -> ShapedRun
    uses FFI("harfbuzz") = { ... }                // Only HarfBuzz

@render_frame (scene: Scene) -> void
    uses FFI("wgpu") = { ... }                    // Only wgpu

// A function that combines them must declare all:
@draw_text_to_screen (text: str, font: Font) -> void
    uses FFI("harfbuzz"), FFI("freetype"), FFI("wgpu"), FFI("SDL3") = { ... }

// The compiler KNOWS that compute_layout() uses NONE of these:
@compute_layout (box: LayoutBox) -> LayoutNode = { ... }  // No uses = provably pure
```

**What this means:** Each C library is a distinct, typed capability. The compiler's effect system tracks exactly which native libraries every function touches, transitively. You can audit the dependency surface of any function by reading its signature.

**Compared to Rust:** `unsafe` is binary — a function is either safe or unsafe. There's no distinction between "calls SQLite" and "calls OpenGL." In Ori, `uses FFI("sqlite3")` and `uses FFI("wgpu")` are distinct capabilities with independent mocking.

---

## 04.2 The Marketing Story

When someone asks "Why Ori for UI?", the answer is concrete:

> **"Our layout engine is provably pure** — the compiler guarantees it never calls C code.
> **Our GPU resources can't leak** — ownership is tracked across the FFI boundary.
> **Our C errors can't be ignored** — they're automatically wrapped as Results.
> **Our render path can't allocate** — `without Allocator` is a compile-time guarantee.
> **Everything is testable without hardware** — mock any C library at the language level.
> **Each C dependency is isolated** — the compiler tracks which libraries every function uses.
>
> No other framework in any language can make all six of these claims simultaneously."

This isn't a feature list — it's a pitch that makes people switch.

---

## 04.3 The Compiler Stress Test Angle

Building 10,300+ lines of pure Ori that does real work — constraint solving, GPU scene building, event dispatch, animation interpolation — will surface every:

- Type inference edge case
- ARC inefficiency
- COW performance cliff
- Pattern matching codegen bug
- Trait resolution ambiguity
- Closure capture issue

The framework development and the compiler development feed each other. This is exactly how Rust's borrow checker was refined by building Servo, how Dart's GC was optimized by building Flutter, and how Swift's ARC was validated by building SwiftUI.

---

## 04.4 Competitive Positioning

| Feature | Electron | Flutter | SwiftUI | Qt | Ori UI |
|---------|----------|---------|---------|-----|--------|
| CSS layout semantics | Yes | No (own model) | No (own model) | No (own model) | **Yes** |
| GPU-accelerated | Via Chromium | Yes (Impeller) | Yes (Metal) | Optional | **Yes (wgpu)** |
| Binary size | 150MB+ | 10-20MB | N/A (Apple) | 30-50MB | **<10MB** |
| Memory overhead | 200MB+ | 50-100MB | Low | 50-100MB | **Low (ARC)** |
| Provably pure layout | No | No | No | No | **Yes** |
| Resource leak prevention | No | GC | ARC (ObjC) | Manual | **Yes (#free)** |
| Error auto-wrapping | No | No | No | No | **Yes (#error)** |
| Alloc-free render path | No | No | No | No | **Yes (without)** |
| FFI mockability | No | No | No | No | **Yes** |
| Per-library capability tracking | No | No | No | No | **Yes** |
| Cross-platform | Yes | Yes | Apple only | Yes | **Yes (wgpu+SDL3)** |
| Web target | Native | Yes | No | No | **Future (WASM)** |
