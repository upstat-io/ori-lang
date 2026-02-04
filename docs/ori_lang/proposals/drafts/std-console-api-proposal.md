# Proposal: std.console API Design

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-03
**Affects:** Standard library
**Depends on:** Minimal FFI for Console (`minimal-ffi-console-proposal.md`), Capabilities System (Section 6)

---

## Summary

This proposal defines `std.console`, Ori's first-class console/terminal library. The goal is to provide **the best console support of any programming language** — correct Unicode handling, honest capability detection, guaranteed state cleanup, flicker-free rendering, and excellent developer experience.

---

## Motivation

### Why Console Support Matters

Every CLI tool, REPL, and terminal application needs console support. Current libraries in other languages have well-documented problems:

| Problem | Who Suffers | Our Solution |
|---------|-------------|--------------|
| Unicode width calculation wrong | Everyone | Grapheme-aware width via `uucode`-style tables |
| Terminal capability guessing | Everyone | Honest probing, not env var trust |
| State not restored on crash | TUI apps | Capability system guarantees cleanup |
| Flickering during updates | AI tools, TUIs | Atomic output + line caching |
| Copy/paste includes formatting | Everyone | Clean content tracking |
| No testing infrastructure | Developers | Headless driver + snapshot tests |

### Why Ori Has an Advantage

Ori's capability system is **perfect** for console state management:

```ori
// Terminal state is a capability - cleanup is GUARANTEED
@main () -> void uses Console = {
    Console.enter_raw_mode()
    Console.enter_alternate_screen()

    run_app()  // Even if this panics...

    // ...terminal is ALWAYS restored via capability cleanup
}
```

---

## Design Principles

1. **Correct by Default** — Unicode width, capability detection, state cleanup all work correctly without effort
2. **Honest, Not Hopeful** — Probe terminals for real capabilities; don't trust environment variables that lie
3. **Capability-Tracked** — All console operations require `Console` capability; cleanup is automatic
4. **Graceful Degradation** — Work on all terminals; optimize for capable ones
5. **Testable** — Headless driver for tests; snapshot testing built-in
6. **Accessibility-First** — `NO_COLOR`, screen readers, reduced motion respected from day one

---

## Module Structure

```
std/
  console/
    mod.ori              # Public API re-exports
    capability.ori       # Console capability trait
    capabilities.ori     # Terminal capability detection
    input.ori            # Keyboard/mouse input
    output.ori           # Styled output, buffering
    cursor.ori           # Cursor positioning
    screen.ori           # Alternate screen, clearing
    style.ori            # Colors, text attributes
    unicode.ori          # Grapheme iteration, width calculation
    driver/
      mod.ori            # Driver trait
      unix.ori           # POSIX termios driver
      windows.ori        # Windows Console API driver
      headless.ori       # Test driver (no real terminal)
```

---

## Core Types

### Console Capability

```ori
// The Console capability trait
trait Console {
    // Terminal info
    @size (self) -> (int, int)
    @is_tty (self) -> bool
    @capabilities (self) -> Capabilities

    // Raw mode
    @enter_raw_mode (self) -> void
    @exit_raw_mode (self) -> void
    @is_raw_mode (self) -> bool

    // Alternate screen
    @enter_alternate_screen (self) -> void
    @exit_alternate_screen (self) -> void

    // Input
    @read_event (self) -> Event
    @read_event_timeout (self, timeout: Duration) -> Option<Event>
    @poll_event (self, timeout: Duration) -> bool

    // Output
    @write (self, text: str) -> void
    @write_styled (self, text: str, style: Style) -> void
    @flush (self) -> void

    // Cursor
    @move_to (self, x: int, y: int) -> void
    @move_by (self, dx: int, dy: int) -> void
    @cursor_position (self) -> (int, int)
    @show_cursor (self) -> void
    @hide_cursor (self) -> void

    // Screen
    @clear (self) -> void
    @clear_line (self) -> void
}

// Default implementation uses native driver
pub def impl Console { ... }
```

### Terminal Capabilities

```ori
// Detected terminal capabilities (probed, not guessed)
type Capabilities = {
    color_depth: ColorDepth,
    unicode_version: int,
    has_synchronized_output: bool,  // DEC 2026
    has_bracketed_paste: bool,
    has_focus_events: bool,
    mouse_protocol: MouseProtocol,
    clipboard_support: ClipboardSupport,
    graphics_protocol: GraphicsProtocol,
    keyboard_protocol: KeyboardProtocol,
}

type ColorDepth = Mono | Ansi16 | Ansi256 | TrueColor

type MouseProtocol = None | X10 | Normal | Sgr | Urxvt

type ClipboardSupport = None | WriteOnly | ReadWrite

type GraphicsProtocol = None | Sixel | KittyGraphics | ITermGraphics

type KeyboardProtocol = Legacy | KittyKeyboard

impl Capabilities {
    // Probe the actual terminal (not just env vars)
    @probe () -> Capabilities uses Console

    // Color adaptation
    @adapt_color (self, color: Color) -> Color

    // Feature checks
    @supports_true_color (self) -> bool = self.color_depth == TrueColor
    @supports_unicode (self) -> bool = self.unicode_version >= 9
    @supports_mouse (self) -> bool = self.mouse_protocol != None
}
```

### Events

```ori
type Event =
    | Key(key: Key, modifiers: Modifiers)
    | Mouse(kind: MouseEventKind, x: int, y: int, modifiers: Modifiers)
    | Resize(width: int, height: int)
    | FocusGained
    | FocusLost
    | Paste(text: str)

type Key =
    | Char(c: char)
    | Enter | Escape | Backspace | Tab
    | Left | Right | Up | Down
    | Home | End | PageUp | PageDown
    | Insert | Delete
    | F(n: int)  // F1-F12

type Modifiers = {
    shift: bool,
    ctrl: bool,
    alt: bool,
    super: bool,
}

type MouseEventKind =
    | Press(button: MouseButton)
    | Release(button: MouseButton)
    | Move
    | Scroll(direction: ScrollDirection)

type MouseButton = Left | Right | Middle

type ScrollDirection = Up | Down | Left | Right
```

### Styling

```ori
type Style = {
    foreground: Option<Color>,
    background: Option<Color>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    reverse: bool,
    hidden: bool,
    strikethrough: bool,
}

impl Style {
    @new () -> Style = Style {
        foreground: None, background: None,
        bold: false, dim: false, italic: false, underline: false,
        blink: false, reverse: false, hidden: false, strikethrough: false,
    }

    @foreground (self, color: Color) -> Style = { ...self, foreground: Some(color) }
    @background (self, color: Color) -> Style = { ...self, background: Some(color) }
    @bold (self) -> Style = { ...self, bold: true }
    @dim (self) -> Style = { ...self, dim: true }
    @italic (self) -> Style = { ...self, italic: true }
    @underline (self) -> Style = { ...self, underline: true }
    @reverse (self) -> Style = { ...self, reverse: true }
}

type Color =
    | Rgb(r: int, g: int, b: int)
    | Ansi256(code: int)
    | Black | Red | Green | Yellow | Blue | Magenta | Cyan | White
    | BrightBlack | BrightRed | BrightGreen | BrightYellow
    | BrightBlue | BrightMagenta | BrightCyan | BrightWhite

impl Color {
    @rgb (r: int, g: int, b: int) -> Color = Rgb(r: r, g: g, b: b)
    @hex (code: int) -> Color = Rgb(
        r: (code >> 16) & 0xFF,
        g: (code >> 8) & 0xFF,
        b: code & 0xFF,
    )
}
```

---

## Unicode Support

### Grapheme Iteration

```ori
// Iterate over grapheme clusters (not codepoints!)
@graphemes (text: str) -> impl Iterator where Item == Grapheme

type Grapheme = {
    text: str,
    width: int,  // Display width (0, 1, or 2)
}
```

### Display Width

```ori
// Correct width calculation for terminal display
@display_width (text: str) -> int

// Check if terminal has width for text
@fits_width (text: str, max_width: int) -> bool

// Truncate to display width (not byte length!)
@truncate_to_width (text: str, max_width: int, ellipsis: str = "...") -> str
```

**Implementation note:** Uses grapheme cluster segmentation and Unicode East Asian Width property. Handles:
- Emoji (including ZWJ sequences like 👨‍👩‍👧‍👦)
- Wide characters (CJK)
- Combining marks (é as e + combining acute)
- Variation selectors

---

## Atomic Output

The key to flicker-free rendering (learned from fzf):

```ori
// Buffer output and write atomically
@atomic (operations: () -> void) -> void uses Console

// Usage
Console.atomic(run(
    Console.hide_cursor(),
    Console.move_to(x: 0, y: 0),
    Console.write_styled(text: "Status: ", style: Style.bold()),
    Console.write(text: status),
    Console.show_cursor(),
))  // ALL output written in one syscall
```

### Synchronized Output (DEC 2026)

When terminal supports it:

```ori
// Automatic synchronized output when available
@with_sync (operations: () -> void) -> void uses Console

// Checks capabilities.has_synchronized_output
// Falls back to atomic() on unsupported terminals
```

---

## Line-Level Caching

For efficient updates (learned from fzf):

```ori
type CachedRenderer = { ... }

impl CachedRenderer {
    @new () -> CachedRenderer uses Console

    // Only redraws lines that changed
    @render (self, lines: [str]) -> CachedRenderer uses Console

    // Force full redraw
    @render_all (self, lines: [str]) -> CachedRenderer uses Console

    // Clear cache (e.g., after resize)
    @invalidate (self) -> CachedRenderer
}
```

---

## Accessibility

```ori
// Environment checks
@prefers_no_color () -> bool       // NO_COLOR or CLICOLOR=0
@prefers_reduced_motion () -> bool // REDUCE_MOTION=1
@is_screen_reader () -> bool       // Heuristic detection

// Accessible output
@announce (message: str) -> void uses Console  // Screen reader friendly

// Static output mode (no cursor movement, no clearing)
@static_mode () -> bool uses Console
```

---

## Testing Infrastructure

### Headless Driver

```ori
type TestTerminal = { ... }

impl TestTerminal {
    @new (width: int, height: int) -> TestTerminal

    // Simulate input
    @send_key (self, key: Key) -> TestTerminal
    @send_key_with (self, key: Key, modifiers: Modifiers) -> TestTerminal
    @send_text (self, text: str) -> TestTerminal
    @resize (self, width: int, height: int) -> TestTerminal

    // Read output
    @screen (self) -> [[Cell]]
    @screen_text (self) -> str
    @cursor_position (self) -> (int, int)
    @cursor_visible (self) -> bool

    // Assertions
    @contains (self, text: str) -> bool
    @line (self, row: int) -> str
}

// Use in tests
impl Console for TestTerminal { ... }
```

### Snapshot Testing

```ori
@assert_snapshot (terminal: TestTerminal, name: str) -> void

// Saves/compares terminal state as text file
// Example: tests/snapshots/my_app_initial.snap
```

### Example Test

```ori
@test tests @my_app () -> void = {
    let term = TestTerminal.new(width: 80, height: 24)

    with Console = term in run(
        my_app.init(),
        term.send_key(key: Key.Down),
        term.send_key(key: Key.Down),
        term.send_key(key: Key.Enter),

        assert(term.contains(text: "Selected: Item 2")),
        assert_snapshot(terminal: term, name: "after_selection"),
    )
}
```

---

## High-Level Patterns

### Interactive App Loop

```ori
@run_app<Model, Msg> (
    init: () -> (Model, Cmd<Msg>),
    update: (Model, Msg) -> (Model, Cmd<Msg>),
    view: (Model) -> View,
) -> void uses Console

// Elm-style Model-Update-View architecture
// Framework handles:
// - Event loop
// - Rendering
// - Capability cleanup
```

### Progress Indicators

```ori
type Spinner = { ... }

impl Spinner {
    @new (message: str) -> Spinner
    @tick (self) -> Spinner uses Console
    @finish (self) -> void uses Console
    @finish_with (self, message: str) -> void uses Console
}

type ProgressBar = { ... }

impl ProgressBar {
    @new (total: int) -> ProgressBar
    @set (self, current: int) -> ProgressBar uses Console
    @increment (self) -> ProgressBar uses Console
    @finish (self) -> void uses Console
}
```

### Prompts

```ori
@prompt (message: str) -> str uses Console
@confirm (message: str) -> bool uses Console
@select<T: Printable> (message: str, options: [T]) -> T uses Console
@multi_select<T: Printable> (message: str, options: [T]) -> [T] uses Console
```

---

## Examples

### Basic Styled Output

```ori
use std.console { Style, Color }

@main () -> void uses Console = {
    let header = Style.new().foreground(Color.Cyan).bold()
    let warning = Style.new().foreground(Color.Yellow)

    Console.write_styled(text: "Build Results\n", style: header)
    Console.write_styled(text: "Warning: ", style: warning)
    Console.write(text: "3 deprecation notices\n")
}
```

### Interactive Selection

```ori
use std.console { Event, Key }

@select_item (items: [str]) -> int uses Console = {
    let selected = 0

    Console.enter_raw_mode()
    Console.hide_cursor()

    loop(run(
        render_list(items: items, selected: selected),

        match Console.read_event() {
            Key(Key.Up, _) -> {
                selected = max(0, selected - 1)
                continue
            },
            Key(Key.Down, _) -> {
                selected = min(len(items) - 1, selected + 1)
                continue
            },
            Key(Key.Enter, _) -> break selected,
            Key(Key.Escape, _) -> break -1,
            _ -> continue,
        }
    ))
}

@render_list (items: [str], selected: int) -> void uses Console = {
    Console.atomic(run(
        Console.move_to(x: 0, y: 0),
        for (item, i) in items.enumerate() do
            if i == selected then
                Console.write_styled(text: `> {item}\n`, style: Style.new().reverse())
            else
                Console.write(text: `  {item}\n`),
    ))
}
```

### Streaming Output (for AI tools)

```ori
use std.console { StreamWriter }

@stream_response (response: impl Iterator where Item == str) -> void uses Console = {
    let writer = StreamWriter.new()

    for token in response do {
        writer.write(text: token)
        writer.flush_if_idle(timeout: 16ms)  // Smart buffering
    }

    writer.complete()
}
```

---

## Implementation Phases

### Phase 1: Basic I/O (Weeks 1-2)
- [ ] Console capability trait
- [ ] Unix driver (termios)
- [ ] Basic write/flush
- [ ] Terminal size detection
- [ ] Raw mode enter/exit

### Phase 2: Styled Output (Weeks 3-4)
- [ ] Style and Color types
- [ ] ANSI escape sequence generation
- [ ] Color degradation (TrueColor → 256 → 16)
- [ ] Capability detection (color depth)

### Phase 3: Input Handling (Weeks 5-6)
- [ ] Event type
- [ ] Key parsing (including escape sequences)
- [ ] Mouse support
- [ ] Resize events

### Phase 4: Unicode (Weeks 7-8)
- [ ] Grapheme iteration
- [ ] Display width calculation
- [ ] Width-aware truncation

### Phase 5: Advanced Features (Weeks 9-10)
- [ ] Atomic output
- [ ] Synchronized output (DEC 2026)
- [ ] Line caching
- [ ] Cursor management

### Phase 6: Testing & Polish (Weeks 11-12)
- [ ] Headless driver
- [ ] Snapshot testing
- [ ] Windows driver
- [ ] Documentation

---

## Success Criteria

`std.console` is complete when:

1. **Correct Unicode**: `display_width("👨‍👩‍👧‍👦")` returns 2
2. **Honest capabilities**: `Capabilities.probe()` actually queries terminal
3. **State cleanup**: Raw mode always restored, even on panic
4. **No flicker**: `atomic()` prevents mid-frame rendering
5. **Cross-platform**: Works on Linux, macOS, Windows
6. **Testable**: Headless driver works; snapshot tests pass
7. **Accessible**: `NO_COLOR` respected; screen reader mode works
8. **Documented**: All public API has examples

---

## References

### Research Repositories (~/console_repos/)

| Repository | Insights |
|------------|----------|
| `ghostty` | SIMD Unicode width, DEC 2026, capability reporting |
| `fzf` | Atomic output, line caching, multi-channel I/O |
| `crossterm` | Command pattern, platform abstraction |
| `bubbletea` | Elm architecture for TUIs |
| `termenv` | Capability detection, color adaptation |
| `ratatui` | Diff-based rendering, constraint layout |
| `textual` | CSS-like styling, reactive state |
| `unicode-width` | 3-level lookup tables, grapheme handling |

### Standards

- ECMA-48: Control Functions for Coded Character Sets
- XTerm Control Sequences: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
- DEC Mode 2026: Synchronized Output
- Unicode UAX #29: Grapheme Cluster Boundaries
- Unicode UAX #11: East Asian Width

---

## Future Work (Not in This Proposal)

| Feature | Rationale for Deferral |
|---------|------------------------|
| Async I/O | Requires Section 16 (Async) |
| Multi-channel architecture | Requires Section 17 (Concurrency) |
| Graphics protocols (Sixel, Kitty) | Nice-to-have, not essential |
| Clipboard access | Security concerns, needs design |
| Full TUI framework | Build on top of std.console |
