---
section: "05"
title: "Compiler Dependencies & Blockers"
status: research-complete
reviewed: true
goal: "Map the exact compiler features needed before each framework layer can be built, with current implementation status"
inspired_by:
  - "plans/roadmap/section-11-ffi.md — FFI roadmap"
  - "plans/deep-safety/ — Deep FFI research"
depends_on: []
---

# Section 05: Compiler Dependencies & Blockers

**Status:** Research Complete (audited 2026-03-25)
**Goal:** Map the precise dependency chain from current compiler state to framework viability, identifying what blocks what and what can proceed in parallel.

**Context:** The framework has two distinct dependency chains: the pure Ori layers need core language features (traits, modules, generics), while the FFI wrappers need the FFI pipeline. These chains are partially independent — pure Ori work can begin before FFI lands.

---

## 05.1 Current Compiler Feature Status (Audited 2026-03-25)

### Features Needed for Pure Ori Layers

| Feature | Required For | Roadmap Section | Status |
|---------|-------------|-----------------|--------|
| Struct types | All types | Section 5 | **In Progress** |
| Sum types (enums) | SizeSpec, Position, Event, etc. | Section 5 | **In Progress** |
| Traits | Widget, Lerp, EventController | Section 3 | **In Progress** |
| Trait impls | impl Widget for Button, etc. | Section 3 | **In Progress** |
| Generics | State<T>, Option<T>, containers | Section 2 | **Complete** |
| Pattern matching | Event dispatch, layout solver | Section 9 | **Not Started** |
| Closures | Event handlers, state updates | Section 5 | **In Progress** |
| Methods (inherent) | Scene.push_rect(), etc. | Section 3 | **In Progress** |
| Method chaining | .font_size(24).color(red) | Section 3 | **In Progress** |
| `impl Trait` return | @counter() -> impl Widget | Section 19 | **Not Started** |
| Default trait methods | Widget defaults | Section 3 | **In Progress** |
| Associated types | Widget::State | Section 3 | **In Progress** |
| Struct spread | { ...defaults, width: 100 } | Section 5 | **In Progress** |
| For-yield | Collection transforms | Section 10 | **Not Started** |
| Capabilities | Theme, Render | Section 6 | **In Progress (14%)** |
| `with...in` | Theme scoping | Section 6 | **In Progress** |

### Features Needed for FFI Wrappers

| Feature | Required For | Roadmap Section | Status |
|---------|-------------|-----------------|--------|
| `extern "c"` type checking | All FFI | Section 11 | **Not Started** |
| CPtr type | All FFI | Section 11 | **Not Started** |
| C ABI types (c_int, etc.) | All FFI | Section 11 | **Not Started** |
| FFI capability trait | `uses FFI("lib")` | Section 11 | **Not Started** |
| LLVM `declare` emission | Calling C functions | Section 11 | **Not Started** |
| C calling convention | ABI compatibility | Section 11 | **Not Started** |
| Linker integration | Linking C libraries | Section 11 | **Not Started** |
| `owned`/`borrowed` | Deep FFI ownership | Section 11.11 | **Not Started** |
| `#error()` | Deep FFI error wrapping | Section 11.11 | **Not Started** |
| `#free()` | Deep FFI auto-Drop | Section 11.11 | **Not Started** |
| `out` parameters | Deep FFI | Section 11.11 | **Not Started** |
| Parametric `FFI("lib")` | Per-library capability | Section 11.11 | **Not Started** |

### Features Needed for Deep Safety Showcase

| Feature | Required For | Location | Status |
|---------|-------------|----------|--------|
| `without` clause | Alloc-free render | Deep Safety plan | **Research Only** |
| Capability propagation | Transitive checking | Section 6 | **Partial** |
| FFI mocking | `with FFI = handler in` | Section 11.11 + Deep Safety | **Not Started** |
| Boolean effect algebra | Negative effects | Deep Safety plan | **Research Only** |

---

## 05.2 Dependency Chain Visualization

```
PURE ORI LAYERS                          FFI LAYERS
═══════════════                          ══════════

Section 3 (Traits) ──┐
Section 5 (Types)  ──┤
Section 2 (Inference)─┤                  Section 6 (Capabilities) ──┐
                      │                          │                   │
                      ▼                          ▼                   │
               Core Language              Capability System          │
              (structs, enums,            (uses, with...in)          │
               traits, closures,                 │                   │
               pattern matching)                 ▼                   │
                      │                  Section 11 (FFI) ──────────┘
                      │                  CPtr, c_int, extern blocks
                      │                  LLVM declare, C ABI, linker
                      │                          │
                      ▼                          ▼
              ori_layout (pure)          Section 11.11 (Deep FFI)
              ori_scene (pure)           owned/borrowed, #error, #free
              ori_widgets (pure)         parametric FFI("lib")
              ori_animation (pure)               │
              ori_events (pure)                  ▼
              ori_state (pure)           ori_gpu, ori_text, ori_window
              ori_theme (pure)           ori_font
                      │                          │
                      └──────────┬───────────────┘
                                 ▼
                          INTEGRATION
                    (connect pure layers to FFI)
                                 │
                                 ▼
                     Deep Safety plan (without)
                     Alloc-free render guarantee
                     FFI mocking for tests
```

---

## 05.3 What Can Start NOW vs What's Blocked

### Can Start Now (Core language features sufficient)

**Pure computation that needs only structs, enums, basic traits, and functions:**

- **Geometry types** (`Rect`, `Point`, `Size`, `Insets`, `Color`): These are simple struct types with arithmetic methods. The current compiler handles these.
- **Layout descriptor types** (`LayoutBox`, `SizeSpec`, `FlexLayout`, `LayoutConstraints`): Structs and enums. Work today.
- **Basic layout solver** (`compute_layout`): Pattern matching on structs, recursive function calls, arithmetic. Needs pattern matching on structs to be solid (Section 9 progress).
- **Easing functions**: Pure math (`float -> float`). Works today.
- **Lerp for primitives**: `float.lerp()`, `Color.lerp()`. Needs trait impls to work.

### Blocked by Core Language Features (Sections 3-6, 9-10)

**Needs traits, generics, closures, pattern matching to be solid:**

- **Widget trait**: Needs traits with associated types, default methods, and `impl Trait` returns (Section 3 + 19)
- **Event dispatch**: Needs pattern matching on sum types (Section 9)
- **Reactive state**: Needs generics + closures working together (Sections 2 + 5)
- **Capability-based theming**: Needs `with...in` to work (Section 6)
- **Builder pattern / method chaining**: Needs method dispatch to be reliable (Section 3)
- **For-yield transforms**: Needs for-yield (Section 10)

### Blocked by FFI (Section 11)

- **All FFI wrappers**: ori_gpu, ori_text, ori_window, ori_font
- **Any test that opens a window or renders pixels**
- **Glyph atlas**: Needs GPU texture creation
- **Text measurement**: Needs HarfBuzz calls for proportional text width

### Blocked by Deep Safety (Section 11.11 + Deep Safety plan)

- **`without Allocator` on render path**: Needs `without` clause implemented
- **FFI mocking in tests**: Needs `with FFI("lib") = handler in { ... }`
- **Parametric capability tracking**: Needs `uses FFI("harfbuzz")` distinct from `uses FFI("wgpu")`
- **Ownership annotations**: `owned`/`borrowed` on extern functions

---

## 05.4 Compiler Work Needed (Phased)

### Phase A: FFI Basics (~3-4 weeks)

Unblocks: Basic C function calls from Ori code.

1. **Add CPtr to type pool** (~2-3 days)
   - Add `CPtr` variant to type system in `ori_types`
   - Register in builtin types
   - No layout/size — opaque pointer

2. **Add C ABI types** (~2-3 days)
   - `c_int`, `c_long`, `c_longlong`, `c_short`, `c_char`, `c_float`, `c_double`, `c_size`
   - Type aliases mapping to Ori primitives (e.g., `c_int` = `int` on most platforms)
   - Registered in prelude or `std.ffi`

3. **Type-check extern blocks** (~1 week)
   - Register extern functions as callable in `ModuleChecker`
   - Validate parameter and return types are FFI-safe
   - Wire up name resolution for extern function calls

4. **LLVM `declare` + C ABI** (~1 week)
   - Emit LLVM `declare` for extern functions
   - Implement C calling convention (x86_64 SysV, Windows x64)
   - Add linker directives for library linking (`-lSDL3`, `-lharfbuzz`, etc.)

5. **Runtime CPtr support** (~3-5 days)
   - CPtr value representation in `ori_rt`
   - String marshalling (Ori `str` ↔ C null-terminated `char*`)
   - Basic errno reading infrastructure

**Gate:** Can compile and run `extern "c" from "SDL3" { @sdl_init (flags: c_int) -> c_int }` and call it from Ori code.

### Phase B: Deep FFI (~4-6 weeks)

Unblocks: Safe, showcase-quality FFI wrappers.

1. **`owned`/`borrowed` annotations** (~1 week)
   - Parser already handles attributes; extend to extern params
   - Type checker validates ownership annotations
   - Codegen emits appropriate cleanup

2. **`#error()` protocols** (~1 week)
   - Parse `#error(errno|nonzero|null|negative|success:N)` on extern blocks
   - Type checker wraps return types as `Result<T, FfiError>`
   - Codegen emits error checking + wrapping

3. **`#free()` annotation** (~1 week)
   - Parse `#free(fn)` on extern blocks
   - Compiler generates `Drop` impl calling the specified free function
   - Wire up ARC pipeline to call auto-generated Drop

4. **`out` parameters** (~3-5 days)
   - Parse `out` keyword on extern params
   - Type checker folds out params into return type
   - Codegen allocates stack space, passes pointer

5. **Parametric `FFI("lib")` capability** (~1-2 weeks)
   - Define `FFI` as a parameterized capability (not a fixed trait)
   - Each `from "lib"` generates a distinct capability
   - Type checker tracks capabilities per extern block

6. **FFI mocking** (~1-2 weeks)
   - Extend `with...in` to handle `FFI("lib")` capabilities
   - Handler-based mocking for extern function calls
   - Evaluator routes calls to mock handlers when provided

**Gate:** Can compile the full `ori_gpu` wrapper with `owned`/`borrowed`, `#error`, `#free`, and test it via `with FFI("wgpu") = handler { ... } in { ... }`.

### Phase C: Deep Safety (~4-6 weeks, partially parallel with B)

Unblocks: The six guarantees from Section 04.

1. **Capability propagation completion** (~2 weeks)
   - Verify transitive capability requirements
   - Functions calling FFI functions must declare the same capability

2. **`without` clause** (~2-3 weeks)
   - Parse `without Capability` in function signatures
   - Track denied set through call chains
   - Deny `with X = impl in` inside `without X` contexts
   - Error E1250: "capability X is denied in this context"

3. **Boolean unification for polymorphic denial** (~2-3 weeks)
   - Effect variable inference with denial constraints
   - Algorithm W + Boolean unification for principal types

**Gate:** Can compile `@submit_frame (...) uses FFI("wgpu") without Allocator = { ... }` and get a compile error when calling an allocating function.

---

## 05.5 Parallel Work Streams

```
Timeline:
═════════

Month 1     Month 2     Month 3     Month 4     Month 5     Month 6
│           │           │           │           │           │
├─── FFI Basics (Phase A) ──┤
│                            │
├─── Pure Ori: geometry, layout types, easing ──────────────────────┤
│                            │
│           ├─── Deep FFI (Phase B) ─────────────┤
│           │                                     │
│           ├─── Pure Ori: widgets, events, state ──────────────────┤
│           │                                     │
│           │               ├─── Deep Safety (Phase C) ─────────────┤
│           │               │                     │
│           │               │    ├─── FFI Wrappers (ori_gpu, etc.) ─┤
│           │               │    │                │
│           │               │    │    ├─── Integration ─────────────┤
│           │               │    │    │           │
│           │               │    │    │    ├─── Deep Safety Showcase ┤
```

**Key insight:** The pure Ori layers can proceed during compiler FFI development. They share a dependency on core language features (Sections 3-6) but not on FFI. The two streams converge at Month 4-5 when integration begins.
