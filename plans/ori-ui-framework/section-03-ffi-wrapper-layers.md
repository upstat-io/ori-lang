---
section: "03"
title: "FFI Wrapper Layers"
status: research-complete
reviewed: true
goal: "Design the thin Ori FFI wrappers around C libraries (wgpu-native, HarfBuzz, FreeType, SDL3) using Deep FFI features"
inspired_by:
  - "oriterm GPU pipeline (extract → prepare → render)"
  - "wgpu-native WebGPU C API (webgpu.h)"
  - "HarfBuzz C API (hb_buffer, hb_shape)"
  - "FreeType C API (FT_Face, FT_Load_Glyph)"
  - "SDL3 C API (SDL_CreateWindow, SDL_PollEvent)"
depends_on: []
---

# Section 03: FFI Wrapper Layers

**Status:** Research Complete
**Goal:** Design the ~3,300 lines of Ori FFI wrapper code that provides safe, capability-tracked access to C libraries for GPU rendering, text shaping, font rasterization, and windowing.

**Context:** These wrappers are thin — they exist to provide safe Ori types over unsafe C APIs. Every wrapper uses Deep FFI features: `owned`/`borrowed` for ownership tracking, `#error()` for automatic error wrapping, `#free()` for resource cleanup, and `uses FFI("lib")` for per-library capability isolation. Users never see the `extern "c"` blocks — they interact with safe Ori types.

---

## 03.1 ori_gpu — wgpu-native Wrapper (~1,500 lines)

**C library:** wgpu-native (WebGPU C API via `webgpu.h`)
**FFI capability:** `uses FFI("wgpu")`

### Why wgpu-native Over Raw Vulkan/Metal/DX12

wgpu-native exposes the WebGPU C API (`webgpu.h`) — the same API browsers ship. Benefits:
- **One API, all platforms:** Vulkan (Linux/Windows/Android), Metal (macOS/iOS), DX12 (Windows), WebGPU (browsers)
- **Clean C API:** ~80 handle-based functions, no COM, no Objective-C
- **WebGPU is the future:** Same code targets native and web (when Ori adds WASM)
- **Validation layer:** Debug builds get GPU validation for free

### Deep FFI Design

```ori
extern "c" from "wgpu_native" #free(wgpuInstanceRelease) {
    @wgpu_create_instance (desc: borrowed CPtr) -> owned CPtr
        as "wgpuCreateInstance"
}

extern "c" from "wgpu_native" #free(wgpuDeviceRelease) {
    @wgpu_adapter_request_device (
        adapter: borrowed CPtr,
        desc: borrowed CPtr,
        callback: (c_int, CPtr, CPtr, CPtr) -> void,
        userdata: CPtr
    ) -> void as "wgpuAdapterRequestDevice"
}

extern "c" from "wgpu_native" #free(wgpuBufferRelease) {
    @wgpu_device_create_buffer (
        device: borrowed CPtr,
        desc: borrowed CPtr
    ) -> owned CPtr as "wgpuDeviceCreateBuffer"
}

extern "c" from "wgpu_native" {
    @wgpu_queue_submit (
        queue: borrowed CPtr,
        count: c_size,
        commands: borrowed CPtr
    ) -> void as "wgpuQueueSubmit"
}
```

### Safe Ori Types

```ori
type GpuInstance = { handle: CPtr }
type GpuAdapter = { handle: CPtr }
type GpuDevice = { handle: CPtr }
type GpuQueue = { handle: CPtr }
type GpuBuffer = { handle: CPtr, size: int, usage: BufferUsage }
type RenderPipeline = { handle: CPtr }
type ShaderModule = { handle: CPtr }
type Texture = { handle: CPtr, format: TextureFormat, size: (int, int) }

// Ownership tracked — compiler auto-generates Drop via #free
impl GpuDevice {
    @create_buffer (self, size: int, usage: BufferUsage) -> Result<GpuBuffer, GpuError>
        uses FFI("wgpu") = { ... }

    @create_render_pipeline (self, desc: PipelineDesc) -> Result<RenderPipeline, GpuError>
        uses FFI("wgpu") = { ... }

    @create_shader_module (self, source: str) -> Result<ShaderModule, GpuError>
        uses FFI("wgpu") = { ... }
}

impl GpuQueue {
    @submit (self, commands: [CommandBuffer]) -> void
        uses FFI("wgpu") = { ... }

    @write_buffer (self, buffer: GpuBuffer, offset: int, data: [byte]) -> void
        uses FFI("wgpu") = { ... }
}
```

### Glyph Atlas Manager

Port from `oriterm/src/gpu/atlas/`. Manages GPU texture pages for rendered glyphs:

```ori
type GlyphAtlas = {
    pages: [AtlasPage],
    cache: {GlyphKey: AtlasEntry},
    page_size: int,
}

type AtlasPage = {
    texture: Texture,
    allocator: GuillotineAllocator,
    format: AtlasFormat,
}

type AtlasFormat = R8 | Rgba8  // Monochrome vs color (emoji)

type GlyphKey = { font_id: int, glyph_id: int, size_px: int, subpixel_offset: int }
type AtlasEntry = { page: int, uv: Rect }
```

---

## 03.2 ori_text — HarfBuzz + FreeType Wrapper (~800 lines)

### Why Not swash

swash is ~15k lines of pure Rust with no C API, heavy use of Rust-specific patterns (`&[u8]` slicing, `bytemuck`, zero-copy parsing). Porting it to Ori would be premature — the byte-level font table parsing is not where Ori shines yet.

**HarfBuzz + FreeType** is the industry standard stack used by Chrome, Firefox, Android, GNOME, and LibreOffice. Clean C APIs, battle-tested, widely available.

### HarfBuzz — Text Shaping

**FFI capability:** `uses FFI("harfbuzz")`

```ori
extern "c" from "harfbuzz" #free(hb_buffer_destroy) {
    @hb_buffer_create () -> owned CPtr as "hb_buffer_create"
}

extern "c" from "harfbuzz" {
    @hb_buffer_add_utf8 (
        buf: borrowed CPtr,
        text: borrowed CPtr,
        text_length: c_int,
        item_offset: c_int,
        item_length: c_int
    ) -> void as "hb_buffer_add_utf8"

    @hb_buffer_set_direction (buf: borrowed CPtr, dir: c_int) -> void
        as "hb_buffer_set_direction"

    @hb_buffer_set_script (buf: borrowed CPtr, script: c_int) -> void
        as "hb_buffer_set_script"

    @hb_buffer_set_language (buf: borrowed CPtr, lang: CPtr) -> void
        as "hb_buffer_set_language"

    @hb_shape (
        font: borrowed CPtr,
        buffer: borrowed CPtr,
        features: borrowed CPtr,
        num_features: c_int
    ) -> void as "hb_shape"

    @hb_buffer_get_glyph_infos (buf: borrowed CPtr, length: out c_int) -> borrowed CPtr
        as "hb_buffer_get_glyph_infos"

    @hb_buffer_get_glyph_positions (buf: borrowed CPtr, length: out c_int) -> borrowed CPtr
        as "hb_buffer_get_glyph_positions"
}
```

### Safe Ori Shaping API

```ori
type ShapedRun = { glyphs: [ShapedGlyph], width: float }

type ShapedGlyph = {
    glyph_id: int,
    cluster: int,
    x_advance: float,
    y_advance: float,
    x_offset: float,
    y_offset: float,
}

@shape_text (font: Font, text: str, direction: TextDirection) -> ShapedRun
    uses FFI("harfbuzz") = {
    // Create buffer, add UTF-8, set direction/script/language, shape, extract results
    // All CPtr handles auto-freed via #free
}
```

### FreeType — Font Rasterization

**FFI capability:** `uses FFI("freetype")`

```ori
extern "c" from "freetype" #error(nonzero) {
    @ft_init_freetype (lib: out CPtr) -> c_int as "FT_Init_FreeType"
    @ft_new_face (lib: borrowed CPtr, path: borrowed CPtr, index: c_long,
                  face: out CPtr) -> c_int as "FT_New_Face"
    @ft_set_pixel_sizes (face: borrowed CPtr, w: c_int, h: c_int) -> c_int
        as "FT_Set_Pixel_Sizes"
    @ft_load_glyph (face: borrowed CPtr, index: c_int, flags: c_int) -> c_int
        as "FT_Load_Glyph"
    @ft_render_glyph (slot: borrowed CPtr, mode: c_int) -> c_int
        as "FT_Render_Glyph"
}
```

### Safe Ori Font API

```ori
type Font = { face: CPtr, metrics: FontMetrics }
type FontMetrics = { ascender: float, descender: float, line_height: float, units_per_em: int }

type GlyphBitmap = {
    pixels: [byte],
    width: int,
    height: int,
    bearing_x: float,
    bearing_y: float,
    advance: float,
}

@load_font (path: str, size_px: int) -> Result<Font, FfiError>
    uses FFI("freetype") = {
    let lib = ft_init_freetype()?
    let face = ft_new_face(lib, path, 0)?
    ft_set_pixel_sizes(face, size_px, size_px)?
    // Extract metrics, return safe Font type
}

@rasterize_glyph (font: Font, glyph_id: int) -> Result<GlyphBitmap, FfiError>
    uses FFI("freetype") = {
    ft_load_glyph(font.face, glyph_id, 0)?  // FT_LOAD_DEFAULT
    ft_render_glyph(font.face.glyph, 0)?     // FT_RENDER_MODE_NORMAL
    // Extract bitmap data into safe Ori types
}
```

---

## 03.3 ori_window — SDL3 Wrapper (~600 lines)

**C library:** SDL3
**FFI capability:** `uses FFI("SDL3")`

### Why SDL3 Over GLFW

| Feature | SDL3 | GLFW |
|---------|------|------|
| Windowing | Yes | Yes |
| Input (keyboard, mouse) | Yes | Yes |
| Gamepad | Yes | No |
| Audio | Yes | No |
| Clipboard | Yes | Limited |
| File dialogs | Yes | No |
| DPI handling | Yes | Yes |
| Wayland | Full | Partial |
| Mobile (iOS/Android) | Yes | No |
| License | zlib | zlib |

SDL3 is the better choice for a comprehensive framework — it handles platform differences and provides clipboard, DPI, file dialogs out of the box.

### Deep FFI Design

```ori
extern "c" from "SDL3" #error(negative) {
    @sdl_init (flags: c_int) -> c_int as "SDL_Init"
}

extern "c" from "SDL3" #error(null) #free(sdl_destroy_window) {
    @sdl_create_window (
        title: borrowed CPtr,
        w: c_int,
        h: c_int,
        flags: c_int
    ) -> owned CPtr as "SDL_CreateWindow"
}

extern "c" from "SDL3" {
    @sdl_poll_event (event: out SdlEvent) -> c_int as "SDL_PollEvent"
    @sdl_get_window_size (win: borrowed CPtr, w: out c_int, h: out c_int) -> void
        as "SDL_GetWindowSize"
    @sdl_get_window_surface (win: borrowed CPtr) -> borrowed CPtr
        as "SDL_GetWindowSurface"
}
```

### Safe Ori Window API

```ori
type Window = { handle: CPtr, width: int, height: int, scale: float }

type WindowEvent = Resize(w: int, h: int) | Close | Focus | Blur
    | KeyDown(key: Key, modifiers: Modifiers)
    | KeyUp(key: Key, modifiers: Modifiers)
    | MouseMove(x: float, y: float)
    | MouseDown(button: MouseButton, x: float, y: float)
    | MouseUp(button: MouseButton, x: float, y: float)
    | Scroll(dx: float, dy: float)
    | TextInput(text: str)
    | DpiChange(scale: float)

@create_window (title: str, width: int, height: int) -> Result<Window, FfiError>
    uses FFI("SDL3") = { ... }

@poll_events () -> [WindowEvent]
    uses FFI("SDL3") = { ... }
```

---

## 03.4 ori_font — Platform Font Discovery (~400 lines)

Platform-specific font discovery. Each platform has a different C API:

| Platform | Library | API Style |
|----------|---------|-----------|
| Linux | fontconfig | Pattern matching (`FcPatternCreate`, `FcFontMatch`) |
| macOS | CoreText | CF-based (`CTFontCreateWithName`) |
| Windows | DirectWrite | COM-based (`IDWriteFactory::CreateTextFormat`) |

```ori
// Unified Ori API — platform differences hidden behind FFI
type FontDescriptor = {
    family: str,
    weight: FontWeight,
    style: FontStyle,
    stretch: FontStretch,
}

type FontWeight = Thin | ExtraLight | Light | Regular | Medium | SemiBold | Bold | ExtraBold | Black

@find_font (desc: FontDescriptor) -> Result<str, FfiError>
    uses FFI(platform_font_lib) = {
    // Returns path to font file
    // Platform-specific implementation via conditional compilation
}

@find_system_font (family: str) -> Result<str, FfiError>
    uses FFI(platform_font_lib) = {
    find_font(FontDescriptor { family, weight: Regular, style: Normal, stretch: Normal })
}
```

---

## 03.5 FFI Layer Summary

| Layer | C Library | FFI Capability | Deep FFI Features Used |
|-------|-----------|----------------|----------------------|
| ori_gpu | wgpu-native | `FFI("wgpu")` | `owned`/`borrowed`, `#free`, callbacks |
| ori_text | HarfBuzz | `FFI("harfbuzz")` | `borrowed` (buffers), `out` (glyph info) |
| ori_text | FreeType | `FFI("freetype")` | `#error(nonzero)`, `out` (init), `borrowed` |
| ori_window | SDL3 | `FFI("SDL3")` | `#error(negative/null)`, `#free`, `out` (events) |
| ori_font | fontconfig/DW/CT | `FFI(platform)` | Platform-conditional, `#error` |

**Total extern functions:** ~80 (wgpu) + ~15 (HarfBuzz) + ~10 (FreeType) + ~20 (SDL3) + ~15 (font) = **~140 C functions wrapped in safe Ori types.**
