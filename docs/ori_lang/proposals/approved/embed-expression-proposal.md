# Proposal: Compile-Time File Embedding (`embed`)

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-02-18
**Approved:** 2026-02-18
**Affects:** Compiler (lexer, parser, type checker, evaluator, codegen)

---

## Summary

Add compile-time `embed` and `has_embed` expressions for embedding file contents into Ori programs. The `embed` expression is type-driven: the expected type determines whether the file is embedded as text (`str`) or binary (`[byte]`). The `has_embed` expression provides compile-time file existence checks.

```ori
// Text (UTF-8 validated at compile time)
let readme: str = embed("README.md")

// Binary
let icon: [byte] = embed("assets/icon.png")

// Conditional
let help = if has_embed("help.txt") then embed("help.txt") else "No help available"

// Const path expressions (not limited to literals)
let $LANG = "en"
let $GREETING: str = embed(`i18n/{$LANG}/greeting.txt`)
```

---

## Motivation

### The Problem

Programs regularly need to bundle static assets -- SQL schemas, HTML templates, configuration defaults, license text, shader source, test fixtures. Without compile-time embedding, the options are:

1. **Runtime file I/O** -- Requires the file to exist at the deployment location, adds failure modes, needs the `FileSystem` capability
2. **Manual string literals** -- Error-prone, unreadable for large content, impossible for binary
3. **Build scripts** -- Extra tooling, not portable, not IDE-visible

### Why Compile-Time?

Embedding at compile time gives:
- **Self-contained binaries** -- No external file dependencies at runtime
- **Compile-time validation** -- UTF-8 errors and missing files caught before deployment
- **Zero runtime cost** -- Data lives in the binary's read-only segment
- **No capability needed** -- Unlike `FileSystem.read()`, embedding is a build concern, not a runtime effect

### Why Not a Const Function?

Ori's const functions (`$name`) are pure -- they cannot perform I/O (see const-evaluation-termination-proposal). Reading files is I/O. Rather than weakening the const purity guarantee, `embed` is a dedicated compiler built-in expression, similar to how `compile_error()` is a built-in that operates at compile time without being a const function.

### Prior Art

| Language | Feature | Strengths | Weaknesses |
|----------|---------|-----------|------------|
| Rust | `include_str!`/`include_bytes!` | Clean text/binary split, file tracking | Macro-only, literal paths only, no structured data, slow on large files |
| Go | `//go:embed` + `embed.FS` | Directory embedding, glob patterns, filesystem interface | Package-scope only, directive syntax, no structured data |
| Zig | `@embedFile` | Size in type (`[N:0]u8`), C FFI sentinel | No text/binary distinction, no directory support |
| C23 | `#embed` | `limit`/`prefix`/`suffix`/`if_empty`, `__has_embed` | Preprocessor-level, no type integration |
| D | `import("file")` | First-class expression, works with `mixin` | No typed variants, `-J` flag required |
| Elixir | `@external_resource` + compile-time code | Structured parsing at compile time | Requires explicit dep tracking, not a unified feature |

Ori's design takes the best from each:
- **Expression-based** like D (not a macro -- avoids Rust's literal-only limitation)
- **Type-driven** like Zig's type precision (but using HM inference instead of sentinel types)
- **Conditional** like C23 (`has_embed` for existence check)

---

## Design

### `embed` Expression

```
embed ( path_expr )
```

`embed` is a compiler built-in expression, not a function. It reads the file at `path_expr` during compilation and produces a value whose type is determined by the surrounding context.

#### Path Resolution

- Paths are **relative to the source file** containing the `embed` expression (like Rust, unlike Go)
- Absolute paths are a **compile error** (security: no `embed("/etc/passwd")`)
- Paths cannot escape the project root via `..` (compile error if resolved path is outside project)
- Path separators: always `/` (normalized by the compiler, portable across platforms)
- The path expression may be any const-evaluable `str` expression, not just a literal:

```ori
let $DATA_DIR = "data"
let schema: str = embed(`{$DATA_DIR}/schema.sql`)
```

This avoids Rust's #1 complaint ([issue #53749](https://github.com/rust-lang/rust/issues/53749)) -- the path doesn't need to be a literal.

#### Type-Driven Embedding

The expected type determines how the file is processed:

| Expected Type | Behavior | Compile-Time Validation |
|---------------|----------|------------------------|
| `str` | Read as UTF-8 text | File must be valid UTF-8 |
| `[byte]` | Read as raw bytes | None (any file accepted) |

```ori
// Inferred as str from context
let query: str = embed("queries/users.sql")

// Inferred as [byte] from context
let font: [byte] = embed("assets/font.ttf")
```

When the type cannot be inferred (ambiguous context), the compiler produces an error:

```ori
let x = embed("file.txt")
//      ^^^^^^^^^^^^^^^^^^
// error[E____]: cannot infer embed type
//   help: add a type annotation: `let x: str = embed("file.txt")`
```

### `has_embed` Expression

```
has_embed ( path_expr ) -> bool
```

Compile-time boolean: `true` if the file exists and is readable, `false` otherwise. This enables conditional embedding without compile errors:

```ori
let license = if has_embed("LICENSE") then embed("LICENSE") else "No license file"

// Combine with conditional compilation
let $HAS_MIGRATIONS = has_embed("migrations/")
```

`has_embed` respects the same path restrictions as `embed` (relative, no escape).

---

## File Dependency Tracking

The compiler **must** track all files referenced by `embed` and `has_embed` as build dependencies:

- Modifying an embedded file triggers recompilation of the module containing the `embed`
- A file checked by `has_embed` changing existence triggers recompilation

This avoids the bug Rust had where `include_bytes!` didn't track dependencies ([rust-lang/cargo#1510](https://github.com/rust-lang/cargo/issues/1510)).

### Salsa Integration

In the Salsa-based compiler, `embed` is a tracked query:
- Input: `(source_file_path, embedded_file_path)`
- Output: file contents (or error)
- The embedded file is registered as an external input, invalidated when its mtime/hash changes

---

## Compile-Time Size Limits

To avoid the performance problems Rust experiences with large files ([rust-lang/rust#65818](https://github.com/rust-lang/rust/issues/65818)):

| Limit | Default | Configurable |
|-------|---------|-------------|
| Single file | 10 MB | `#embed_limit(size: 50mb)` or `ori.toml` |

Exceeding the limit produces a clear error with the file size and the limit:

```
error[E____]: embedded file exceeds size limit
  --> src/assets.ori:3:20
   |
3  | let data: [byte] = embed("large_video.mp4")
   |                     ^^^^^^^^^^^^^^^^^^^^^^^^ file is 150 MB, limit is 10 MB
   |
   = help: increase limit with #embed_limit(size: 200mb) or in ori.toml
   = help: consider loading large files at runtime with FileSystem.read()
```

Configuration in `ori.toml`:

```toml
[embed]
max_file_size = "50mb"
```

---

## Examples

### SQL Schema Embedding

```ori
let $CREATE_TABLES: str = embed("sql/create_tables.sql")
let $SEED_DATA: str = embed("sql/seed.sql")

@initialize_db (db: Database) -> Result<void, Error> uses Database =
    run(
        db.execute(query: $CREATE_TABLES)?,
        db.execute(query: $SEED_DATA)?,
    )
```

### CLI Help Text

```ori
let $HELP_TEXT: str = if has_embed("HELP.md") then
    embed("HELP.md")
else
    "Usage: myapp [options]\n  Run with --help for more information."

@main (args: [str]) -> void =
    if args.contains(value: "--help") then
        print(msg: $HELP_TEXT)
    else
        run_app(args: args)
```

### Binary Asset Embedding

```ori
let $FAVICON: [byte] = embed("assets/favicon.ico")
let $DEFAULT_FONT: [byte] = embed("assets/fonts/default.ttf")

@serve_favicon () -> Response =
    Response.new(
        status: 200,
        headers: {"Content-Type": "image/x-icon"},
        body: $FAVICON,
    )
```

### Conditional Platform Assets

```ori
#target(os: "windows")
let $ICON: [byte] = embed("assets/icon.ico")

#target(os: "macos")
let $ICON: [byte] = embed("assets/icon.icns")

#target(os: "linux")
let $ICON: [byte] = embed("assets/icon.png")
```

### Const Path Construction

```ori
let $LANG = "en"
let $GREETING: str = embed(`i18n/{$LANG}/greeting.txt`)

// Or with a const function
$locale_path (lang: str, file: str) -> str = `i18n/{lang}/{file}`
let $WELCOME: str = embed($locale_path(lang: "en", file: "welcome.txt"))
```

### Test Fixture Embedding

```ori
@parse_config (source: str) -> Result<Config, Error>

@test_parse tests @parse_config () -> void =
    run(
        let result = parse_config(source: embed("fixtures/valid_config.json")),
        assert(condition: result.is_ok()),
    )

@test_parse_invalid tests @parse_config () -> void =
    run(
        let result = parse_config(source: embed("fixtures/invalid_config.json")),
        assert(condition: result.is_err()),
    )
```

### Embedded Version String

```ori
let $VERSION: str = embed("VERSION").trim()
let banner = `MyApp v{$VERSION}`
```

---

## Design Rationale

### Why an Expression, Not a Macro or Keyword?

Ori doesn't have Rust-style macros. Making `embed` a built-in expression:
- Integrates naturally with the type system (HM inference determines behavior)
- Accepts const expressions as paths (not limited to literals)
- Works anywhere an expression is valid
- Is IDE-friendly (can show embedded content on hover, navigate to source file)

### Why Type-Driven Instead of Separate Functions?

Rust has `include_str!` vs `include_bytes!` -- two macros that differ only in return type. In Ori, the type system already knows what you want:

```ori
let text: str = embed("file")      // compiler infers: read as UTF-8
let data: [byte] = embed("file")   // compiler infers: read as bytes
```

One expression, two behaviors, zero ambiguity. This is a natural fit for HM inference.

### Why Require Type Annotation When Ambiguous?

Rather than defaulting to `str` (which would silently change behavior if the context changes), requiring an explicit type annotation when inference can't determine the type prevents surprises. Explicit is better than implicit.

### Why Relative Paths Only?

Absolute paths create security risks (any file on the build machine is accessible) and portability problems (paths differ across machines). Relative-to-source-file paths:
- Are portable (project directory structure is consistent)
- Are secure (can't escape the project root)
- Match developer intuition (the embed is "near" the source file)
- Are what Rust, Nim, and Zig do

### Why Size Limits?

Rust's `include_bytes!` with a 256 MB file takes 25 seconds and 10+ GB RAM. Ori should fail fast with a clear message rather than silently degrading the build. The limit is a generous default (10 MB) that can be raised when genuinely needed.

### Why `has_embed` Instead of `embed` Returning `Option`?

If `embed` returned `Option<str>`, every use site would need unwrapping. The common case is "this file must exist" -- `embed` should fail at compile time if it doesn't. `has_embed` handles the rarer case of optional resources, and its boolean result integrates cleanly with `if`/`then`/`else`.

### Why Not a Capability?

File embedding is a **build-time** concern, not a **runtime** effect. The embedded data is frozen into the binary -- there's no I/O at runtime. Requiring `uses FileSystem` would be misleading (the function is pure at runtime) and would prevent embedding in pure functions.

---

## Interaction with Other Features

### Const Evaluation

`embed` results can be bound to const values:

```ori
let $SCHEMA: str = embed("schema.sql")     // const binding
let runtime_data: str = embed("data.txt")  // runtime binding (value still embedded at compile time)
```

Both are embedded at compile time. The difference is that `$SCHEMA` can be used in other const contexts.

### Conditional Compilation

`embed` works naturally with `#target` and `#cfg`:

```ori
#target(os: "linux")
let $INIT_SCRIPT: str = embed("scripts/init_linux.sh")

#target(os: "windows")
let $INIT_SCRIPT: str = embed("scripts/init_windows.bat")
```

The compiler only reads the file for the active target.

### String Interpolation

Embedded strings can be used in interpolation like any other `str`:

```ori
let $VERSION: str = embed("VERSION").trim()
let banner = `MyApp v{$VERSION}`
```

---

## Error Catalog

| Code | Condition | Example |
|------|-----------|---------|
| `E____` | File not found | `embed("nonexistent.txt")` |
| `E____` | Absolute path | `embed("/etc/passwd")` |
| `E____` | Path escapes project | `embed("../../outside.txt")` |
| `E____` | Not valid UTF-8 (when `str` expected) | Binary file embedded as `str` |
| `E____` | Cannot infer embed type | `let x = embed("file")` without annotation |
| `E____` | File exceeds size limit | 150 MB file with 10 MB limit |

All errors include:
- The source location of the `embed` expression
- The resolved file path
- A help suggestion (e.g., "did you mean 'readme.md'?" for close matches)

---

## Spec Changes Required

### New Spec Section: `XX-embed.md`

Document:
1. `embed` expression syntax and semantics
2. `has_embed` expression syntax and semantics
3. Path resolution rules
4. Type-driven behavior table
5. Size limits and configuration
6. Dependency tracking requirements
7. Error codes

### `grammar.ebnf`

Add productions:

```ebnf
embed_expr     = "embed" "(" expression ")" ;
has_embed_expr = "has_embed" "(" expression ")" ;
```

### `12-modules.md` (Prelude)

Add `embed`, `has_embed` to built-in expressions.

### `ori-syntax.md`

Add embed expressions to the quick reference.

---

## Implementation Notes

### Compiler Pipeline

1. **Lexer**: `embed`, `has_embed` are context-sensitive keywords (like `run`, `match`)
2. **Parser**: Parse as built-in expression nodes (`EmbedExpr`, `HasEmbedExpr`)
3. **Type Checker**: Resolve expected type, validate path is const, check file existence
4. **Evaluator**: Read file, convert to value based on type
5. **LLVM Codegen**: Emit as static data in read-only section (`.rodata`)

### Binary Layout

Embedded data should be stored in the binary's read-only data section (`.rodata` on ELF, `__TEXT,__const` on Mach-O, `.rdata` on PE). Multiple references to the same embedded file should deduplicate to a single copy.

### Incremental Compilation

The Salsa query for embed should:
1. Hash the embedded file's contents (not just mtime, for robustness)
2. Store the hash as the query's durability marker
3. Re-read and re-hash on each compilation cycle
4. Only invalidate dependents if the hash changed

---

## Future Extensions

### Structured Embedding (JSON)

When the expected type implements `Json`, the file could be parsed as JSON and validated at compile time:

```ori
#derive(Json, Eq, Debug)
type Config = { host: str, port: int, debug: bool }
let $DEFAULT_CONFIG: Config = embed("default_config.json")
```

This requires either a compiler built-in JSON parser or const-evaluable `Json` trait methods. Deferred until the compile-time evaluation infrastructure matures.

### Directory Embedding (`embed_dir`)

Embed entire directory trees as maps (inspired by Go's `embed.FS`):

```ori
let assets: {str: [byte]} = embed_dir("static/")
let templates: {str: str} = embed_dir("templates/", glob: "*.html")
```

This adds significant compiler complexity (directory traversal, glob matching, recursive enumeration). Deferred to a separate follow-up proposal.

### Encoding Parameter

Non-UTF-8 text file support:

```ori
let legacy: str = embed("data.csv", encoding: "latin1")
```

Requires encoding tables in the compiler. Deferred until demand materializes. Users can embed non-UTF-8 files as `[byte]` and convert at runtime.

### `embed` with `limit`

C23-style byte limiting for large files:

```ori
let preview: str = embed("large_log.txt", limit: 1024)
```

### Custom `Embeddable` Trait

A general trait for compile-time file parsing:

```ori
trait Embeddable {
    $from_file (content: [byte], path: str) -> Result<Self, str>
}
```

This would allow user-defined types to participate in structured embedding without depending on JSON.

### Compile-Time Transforms

Process embedded content at compile time:

```ori
let $MINIFIED_CSS: str = embed("style.css").replace(old: "\n", new: "")
```

This already works if `str.replace` is made const-evaluable -- no special `embed` support needed.

### `embed` with Alignment

For FFI or performance-sensitive binary data:

```ori
#repr("aligned", 16)
let $SIMD_DATA: [byte] = embed("lookup_table.bin")
```

This addresses Rust's alignment issue without needing a separate `embed_aligned` variant.

---

## Alternatives Considered

### 1. Const Function with Special Permission

```ori
$read_file (path: str) -> str  // compiler-magic const function
```

Rejected: Blurs the line between pure const functions and I/O. Every const function would need to ask "can this one do I/O?" The explicit `embed` keyword makes the compile-time file read visible and intentional.

### 2. Attribute-Based (Go-style)

```ori
#embed("data.txt")
let data: str
```

Rejected: Separates the annotation from the expression, doesn't work in expression position, requires special variable declaration semantics.

### 3. Import Syntax

```ori
use "./data.txt" as data
```

Rejected: `use` is for module imports. Conflating file embedding with module resolution creates confusion about what `data` is (a module? a string? a value?).

### 4. Separate `embed_str` / `embed_bytes`

```ori
let text = embed_str("file.txt")
let data = embed_bytes("file.bin")
```

Rejected: Redundant when the type system already distinguishes the two. This is exactly the Rust pattern (`include_str!` vs `include_bytes!`) that type-driven embedding improves upon.

### 5. Runtime Fallback

```ori
let data = embed("file.txt") ?? FileSystem.read("file.txt")
```

Rejected for the `embed` expression itself: embedding is compile-time by definition. Runtime file reading is a separate concern handled by the `FileSystem` capability. However, `has_embed` + `if`/`else` provides conditional embedding, and the runtime branch can use `FileSystem`.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Expressions | `embed(path)`, `has_embed(path)` |
| Kind | Compiler built-in expression (not a function or macro) |
| Type behavior | `str` (UTF-8 validated), `[byte]` (raw binary) |
| Path resolution | Relative to source file, no absolute, no project escape |
| Path expressions | Any const-evaluable `str` (not limited to literals) |
| Existence check | `has_embed(path)` returns compile-time `bool` |
| Size limit | 10 MB per file (configurable via `#embed_limit` or `ori.toml`) |
| Dependency tracking | Mandatory -- file changes trigger recompilation |
| Security | No absolute paths, no project escape, size limits |
| Binary layout | Read-only data section, deduplicated |
| Future | Structured embedding, directory embedding, encoding parameter |
