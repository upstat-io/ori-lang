# Spec Depth — Design Decisions Record

All decisions made during gap analysis (2026-03-01). These are settled and shall be reflected in the spec.

---

## Batch 1: Numeric Literals & Special Values

### D01: Leading zeros in decimal literals → **REJECT**
- `007` is a compile-time error. Write `7` instead.
- `0` (single zero) is valid.
- Hex/binary prefixes (`0xFF`, `0b101`) are not leading zeros.
- Prior art: Go, Rust, Python, Zig all reject.

### D02: Inf/NaN access → **Associated constants**
- `float.inf` — positive infinity
- `float.neg_inf` — negative infinity
- `float.nan` — NaN
- `float.max` — largest finite float
- `float.min` — smallest positive normal
- `float.epsilon` — machine epsilon
- Consistent with existing `int.min`, `int.max` pattern.
- Prior art: Rust (`f64::INFINITY`), Swift (`Double.infinity`).

### D03: Octal prefix → **NOT IN v2026**
- No `0o777` syntax.
- Can add in future edition if needed.
- Prior art: Gleam, Elm, Roc also omit octal.

---

## Batch 2: Escape Sequences & Strings

### D04: Extra escape sequences → **NO**
- Keep only 6 escapes: `\\`, `\"`, `\n`, `\t`, `\r`, `\0`.
- No `\a` (bell), `\b` (backspace), `\f` (form feed), `\v` (vertical tab).
- Use `\u{7}` for bell character via unicode escape.
- Prior art: Rust, Zig, Gleam, Swift also omit legacy escapes.

### D05: Unicode escape syntax → **ADD `\u{HHHH}`**
- Rust-style variable-length: 1–6 hex digits in braces.
- `\u{1F600}` for emoji, `\u{0041}` for 'A'.
- Surrogate code points (U+D800–U+DFFF) are an error.
- Valid in both regular strings and template strings.
- Prior art: Rust, Swift, JS (ES6+), Zig all use `\u{...}`.

### D06: Nested template strings → **GUARANTEED**
- `` `outer {`inner {x}`}` `` is valid Ori.
- Interpolation `{...}` can contain any expression including template strings.
- Prior art: Swift, JS, Kotlin all support nested interpolation.

---

## Batch 3: Source Code & Positions

### D07: Control characters → **REJECT EVERYWHERE**
- Control chars U+0001–U+001F are errors in all source positions.
- Exceptions: `\t` (U+0009), `\n` (U+000A), `\r` (U+000D, normalized).
- Use escape sequences (`\u{7}`, `\n`, etc.) to embed in strings.
- Prior art: Go, Rust, Zig all reject raw control chars.

### D08: Column numbers → **BYTE OFFSET, 1-BASED**
- Column = byte offset from start of line, 1-based.
- Fast to compute, matches VS Code, LSP protocol.
- Prior art: Go, Rust, Zig all use byte offset.

### D09: Unicode identifiers → **ASCII-ONLY, RESERVE FOR FUTURE**
- v2026: identifiers are `[a-zA-Z_][a-zA-Z0-9_]*`.
- Spec includes NOTE that future editions may expand to Unicode categories.
- Prior art: Zig, Gleam, Elm are also ASCII-only.

---

## Batch 4: Execution Model & Panics

### D10: Panic model → **ABORT (no unwind)**
- Panic = format PanicInfo → call @panic handler → print to stderr → exit(1).
- Drop impls do NOT run during panic.
- No stack unwinding, no unwind tables.
- Philosophy: panics are bugs, use Result for recoverable errors.
- Prior art: Go, Zig, Swift, Gleam, Koka all abort on panic.

### D11: `int as byte` out of range → **PANIC**
- `256 as byte` panics at runtime.
- Consistent with integer overflow panics.
- Use `as?` for checked conversion: `256 as? byte` → `None`.
- Pattern: `as` = assertive (like `list[i]`), `as?` = cautious.
- Prior art: Zig (`@intCast` panics), Swift (traps on overflow).

### D12: Module-level constant purity → **PURE ONLY**
- Module-level `let $` initializers shall be pure expressions.
- No capabilities (no IO, no Random, no Clock, no FileSystem).
- Ensures deterministic initialization regardless of module load order.
- Prior art: Rust (const must be pure), Zig (comptime is pure), Koka.

---

## Batch 5: Type System & Patterns

### D13: Type narrowing → **NO**
- No implicit type narrowing after conditional checks.
- `if is_some(x) then ...` — x is still `Option<T>` in the then-branch.
- Use `match` for destructuring (idiomatic Ori).
- Keeps type system simpler — variable type never changes within a scope.
- Prior art: Go, Rust, Zig, Gleam all use match/switch instead of narrowing.

### D14: `int ↔ char` conversions → **BOTH DIRECTIONS**
- `char as int` → codepoint value (infallible).
- `int as char` → panic if not valid Unicode scalar (U+0000–U+D7FF, U+E000–U+10FFFF).
- `int as? char` → `None` if invalid.
- Prior art: Zig panics, Rust returns Option, Go silently produces U+FFFD.

### D15: `byte → char` → **ALLOWED, LATIN-1**
- `byte as char` interprets byte as Unicode codepoint U+0000–U+00FF.
- Always infallible (all 256 byte values are valid Unicode scalars).
- Prior art: Go, Rust have same semantics.

---

## Batch 6: Conversion Tables

### D16: Complete `as` (infallible) table → **APPROVED**

| Source | Target | Behavior |
|--------|--------|----------|
| `int` | `float` | exact (within i64 range) |
| `int` | `byte` | panic if outside 0..255 |
| `int` | `char` | panic if not Unicode scalar |
| `byte` | `int` | zero-extend (infallible) |
| `byte` | `char` | Latin-1 (infallible) |
| `char` | `int` | codepoint value (infallible) |
| `char` | `str` | single-char string (infallible) |

Anything not in this table is a compile error for `as`. User types use `As<T>` trait.

### D17: Complete `as?` (fallible) table → **APPROVED**

| Source | Target | Returns |
|--------|--------|---------|
| `str` | `int` | `Some(n)` if valid integer, `None` otherwise |
| `str` | `float` | `Some(f)` if valid float, `None` otherwise |
| `str` | `bool` | `Some(b)` for `"true"`/`"false"`, `None` otherwise |
| `int` | `byte` | `Some(b)` if 0..255, `None` otherwise |
| `int` | `char` | `Some(c)` if valid Unicode scalar, `None` otherwise |
| `float` | `int` | `Some(n)` if whole number, no precision loss, in i64 range |

Parsing rules for `str → int`: strip whitespace, accept leading ±, reject `_`/hex/bin prefix, overflow → None.
Parsing rules for `str → float`: strip whitespace, accept `"inf"`/`"nan"`/scientific notation.
User types use `TryAs<T>` trait.

---

## Batch 7: Misc

### D18: `drop_early` on immutable → **ALLOWED**
- `drop_early(value:)` works on both mutable and `$`-prefixed bindings.
- The binding becomes inaccessible after drop (compile error on use-after-drop).
- This is about ownership/lifetime, not mutability.
- Prior art: Rust `drop(x)` works on any owned value.

### D19: Stack depth → **IMPLEMENTATION-DEFINED, MIN 1000**
- Maximum call stack depth is implementation-defined.
- Implementations shall support at least 1000 frames.
- Stack overflow causes a panic.
- Prior art: Go (dynamic stacks), Rust/Zig/Swift (default 8 MiB).

### D20: Digit separators in duration/size → **ALLOWED**
- `1_000ms`, `10_000kb` are valid.
- Same rules as integer separators (no leading, trailing, or adjacent `_`).

---

## Confirmed Compiler Behaviors (codify as-is)

These are already implemented and just need spec text:

- **C01**: Identifiers are ASCII-only
- **C02**: No octal prefix
- **C03**: No leading-dot floats (`.5`) or trailing-dot floats (`5.`)
- **C04**: Only 6 escape sequences (before D05 adds `\u{...}`)
- **C05**: No `if let` syntax (use `match`)
- **C06**: `let` always requires initializer
- **C07**: `{ }` = empty map literal
- **C08**: No method references without calling
- **C09**: `pub` is type-level, not per-field
- **C10**: `(a,)` = parenthesized expression, not single-element tuple
