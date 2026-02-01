# Proposal: WASM Playground

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Approved:** 2026-01-31
**Affects:** Tooling, developer experience, website, onboarding

---

## Summary

This proposal formalizes the design and implementation decisions for Ori's browser-based WASM playground, which allows users to write, run, and format Ori code directly in the browser without any local installation.

---

## Problem Statement

Learning a new programming language requires hands-on experimentation. Traditional approaches require users to:

1. Download and install a compiler/runtime
2. Configure an editor or IDE
3. Set up a project structure
4. Write, compile, and run code

This friction creates a barrier to adoption, especially for users who want to quickly evaluate the language or share code snippets with others.

A browser-based playground addresses these issues by providing:

- **Zero installation**: Run Ori code immediately in any browser
- **Shareability**: Generate URLs containing code for easy sharing
- **Accessibility**: Works on any device with a modern browser
- **Onboarding**: Provide guided examples for new users

---

## Implementation Status

| Feature | Status |
|---------|--------|
| WASM crate with run/format/version exports | ✅ Complete |
| Monaco editor with Ori syntax | ✅ Complete |
| URL-based code sharing | ✅ Complete |
| Full-screen playground page | ✅ Complete |
| Embedded playground on landing | ✅ Complete |
| Basic examples (5) | ✅ Complete |
| Expanded examples (10) | 🔶 Pending |
| Stdlib documentation | 🔶 Pending |

---

## Architecture

### Directory Structure

The playground lives at the repo root level:

```
ori_lang/                      # Repository root
├── compiler/                 # Compiler crates
├── playground/               # WASM playground
│   ├── wasm/                # WASM crate (Cargo.toml references ../../compiler/)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs       # WASM bindings
│   ├── pkg/                 # Generated WASM output
│   └── src/                 # Svelte components (symlinked)
├── website/                  # Main Ori website (Astro)
│   ├── src/
│   │   ├── pages/
│   │   │   ├── playground.astro  # Full-screen playground page
│   │   │   └── index.astro       # Landing page with embedded playground
│   │   ├── components/
│   │   │   └── playground/       # Shared Playground components
│   │   │       ├── Playground.svelte
│   │   │       ├── MonacoEditor.svelte
│   │   │       ├── OutputPane.svelte
│   │   │       ├── PlaygroundToolbar.svelte
│   │   │       ├── wasm-runner.ts
│   │   │       ├── examples.ts
│   │   │       └── ori-monarch.ts
│   │   └── wasm/                 # Type definitions
│   └── public/wasm/              # Static WASM assets
└── ...
```

### WASM Crate Design

**Decision**: Create a standalone WASM crate that depends only on portable Ori compiler crates.

**Rationale**:
- The main compiler uses Salsa for incremental compilation — Salsa does not compile to WASM
- LLVM codegen is not needed for interpretation — also does not compile to WASM
- A separate crate selects only the portable subset: `ori_ir`, `ori_lexer`, `ori_parse`, `ori_types`, `ori_typeck`, `ori_eval`, `ori_patterns`, `ori_fmt`

**Crate Type**: `cdylib` (C-compatible dynamic library for WASM)

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
ori_ir = { path = "../../compiler/ori_ir" }
ori_lexer = { path = "../../compiler/ori_lexer" }
ori_parse = { path = "../../compiler/ori_parse" }
ori_types = { path = "../../compiler/ori_types" }
ori_typeck = { path = "../../compiler/ori_typeck" }
ori_eval = { path = "../../compiler/ori_eval" }
ori_patterns = { path = "../../compiler/ori_patterns" }
ori_fmt = { path = "../../compiler/ori_fmt" }
wasm-bindgen = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "s"  # Optimize for size
lto = true       # Link-time optimization
```

### WASM API

Three functions are exported via `wasm-bindgen`:

#### `run_ori(source: &str) -> String`

Executes Ori source code and returns a JSON result:

```typescript
interface RunResult {
    success: boolean;
    output: string;       // Expression result (if successful)
    printed: string;      // Output from print() calls
    error?: string;       // Error message (if failed)
    error_type?: 'parse' | 'type' | 'runtime';
}
```

**Pipeline**:
1. Lex source with `ori_lexer::lex()`
2. Parse with `ori_parse::parse()`
3. Type check with `ori_typeck::type_check()`
4. Create interpreter with `InterpreterBuilder`
5. Register prelude functions and derived traits
6. Execute code:
   - If `@main` function exists: execute it
   - If no `@main` but a single top-level expression: evaluate it
   - Otherwise: show error "No @main function found"
7. Capture print output via buffer handler
8. Return result as JSON

#### `format_ori(source: &str, max_width?: usize) -> String`

Formats Ori source code and returns a JSON result:

```typescript
interface FormatResult {
    success: boolean;
    formatted?: string;   // Formatted code (if successful)
    error?: string;       // Error message (if failed)
}
```

Uses `ori_fmt::format_module_with_comments_and_config()` with comment preservation.

#### `version() -> String`

Returns the Ori version string (e.g., `"Ori 0.1.0-alpha"`).

---

## UI Components

### Playground.svelte

The main container component with the following state:

| State | Type | Purpose |
|-------|------|---------|
| `code` | `string` | Current source code (bindable) |
| `selectedExample` | `string` | Currently selected example |
| `result` | `RunResult \| null` | Latest execution result |
| `status` | `'idle' \| 'running' \| 'success' \| 'error'` | Current execution state |
| `wasmVersion` | `string` | Display version or loading status |
| `elapsed` | `string` | Execution time |

**Configuration Options**:

```typescript
interface PlaygroundConfig {
    showToolbar: boolean;       // Show toolbar buttons
    showOutput: boolean;        // Show output pane
    height: string;             // CSS height
    enableShare: boolean;       // Enable share button
    enableExamples: boolean;    // Enable examples dropdown
    readUrlHash: boolean;       // Load code from URL hash
    initialCode?: string;       // Starting code
    fontSize: number;           // Editor font size
    layout: 'horizontal' | 'vertical';
    maxFormatWidth?: number;    // Max line width for formatter
}
```

### MonacoEditor.svelte

Monaco Editor (VS Code's editor engine) configured for Ori:

- **Syntax highlighting**: Custom Monarch grammar for Ori
- **Keyboard shortcuts**: `Ctrl+Enter` to run
- **Theme**: Ori-dark theme with custom color palette
- **Bracket matching**: Smart pairing and selection
- **Dynamic loading**: Loaded dynamically to avoid SSR issues

**Highlighted constructs**:
- Keywords: `if`, `then`, `else`, `let`, `for`, `match`, `type`, `trait`, `impl`, etc.
- Types: `int`, `float`, `str`, `Option`, `Result`, `Never`, etc.
- Operators: arithmetic, bitwise, logical, comparison
- Function names (prefixed with `@`)
- Constants (prefixed with `$`)

### OutputPane.svelte

Displays execution results:

- Success: Shows expression result and print output
- Error: Shows error message with type (parse/type/runtime)
- Timing: "Ran in Xms" or "Xs, interpreted in WASM"
- Status badges: Color-coded (running/success/error)

### PlaygroundToolbar.svelte

Action buttons:

| Button | Action |
|--------|--------|
| Run | Execute code (disabled while running) |
| Format | Format code with `ori_fmt` |
| Share | Copy shareable URL to clipboard |
| Examples | Dropdown to load sample programs |

---

## Features

### URL-Based Code Sharing

**Decision**: Encode code in URL fragment using base64.

**Format**: `https://ori-lang.com/playground#<base64-url-safe-encoded-code>`

**Rationale**:
- Fragments are not sent to server — no backend needed
- URLs can be shared via any medium
- Browser history works naturally
- Bookmarking preserves code

**Implementation**:
- On load: Decode fragment and populate editor
- On share: Encode code and copy URL to clipboard
- Use URL-safe base64 variant (no `+` or `/`)

### Auto-Format on Run

**Decision**: Format code before execution.

**Rationale**:
- Ensures consistent code style in shared snippets
- Catches formatting issues early
- If format fails, show error instead of running

### Example Programs

Ten built-in examples for onboarding:

**Core (implemented):**
1. **Hello World** — Basic `print()` call
2. **Fibonacci** — Memoized recursion with `recurse()` pattern
3. **Factorial** — Recursive function with guards
4. **List Operations** — `map()`, `filter()`, `fold()` on arrays
5. **Structs and Methods** — Type definition, methods via `impl`

**Extended (pending):**
6. **Sum Types** — `Option`, `Result`, pattern matching with `match()`
7. **Error Handling** — `try()` pattern with `?` propagation
8. **Iterators** — Lazy iteration with `for...yield` comprehensions
9. **Traits** — Defining and implementing traits
10. **Generics** — Generic functions and type constraints

### Development Workflow

**Decision**: Support hot reload of WASM during development.

**Implementation**:
- `window.reloadWasm()` exposed in dev mode
- Cache busting via timestamp query parameter
- Scripts: `rebuild-wasm.sh` rebuilds and copies artifacts

---

## Website Integration

### Full-Screen Playground (`/playground`)

- Astro page with `PlaygroundLayout`
- Vertical layout (larger editor, output below)
- All features enabled
- Custom header with navigation

### Embedded Playground (Landing Page)

- Smaller size (560px height)
- `client:visible` directive (lazy-loads when scrolled into view)
- Share button disabled
- Compact max-width (60 chars)
- Limited example selection

---

## Build & Deployment

### Build Command

```bash
wasm-pack build --target web --release --out-dir ../pkg
```

### Output Files

| File | Purpose |
|------|---------|
| `ori_playground_wasm.js` | JavaScript bindings |
| `ori_playground_wasm_bg.wasm` | WASM binary (~828 KB) |
| `ori_playground_wasm.d.ts` | TypeScript definitions |

### Distribution

```
playground/pkg/          → build output
website/src/wasm/        → type definitions (for IDE)
website/public/wasm/     → static assets (served at /wasm/)
```

### Automation

```json
// website/package.json
{
    "build:wasm": "cd ../playground/wasm && wasm-pack build --target web --release --out-dir ../pkg",
    "prebuild": "bash scripts/copy-wasm.sh"
}
```

The `prebuild` hook ensures WASM is copied before Astro build.

---

## Performance

### Binary Size

**Target**: < 1 MB compressed

**Achieved**: ~828 KB (uncompressed)

**Optimizations**:
- `opt-level = "s"` — optimize for size
- `lto = true` — link-time optimization
- Minimal dependencies — no Salsa, no LLVM

### Execution

**Mode**: Interpreter (no JIT compilation in browser)

**Characteristics**:
- Each run creates fresh interpreter instance
- Print output captured via buffer handler
- No persistent state between runs

### Loading Strategy

**Decision**: Dynamic import with lazy initialization.

```typescript
async function initWasm(): Promise<boolean> {
    const module = await import('/wasm/ori_playground_wasm.js');
    await module.default();
    return true;
}
```

**Rationale**:
- Does not block initial page load
- WASM fetched when playground becomes visible
- Cache busting in dev mode via timestamp

---

## Error Handling

### Error Categories

| Category | Display | Example |
|----------|---------|---------|
| WASM load failure | Setup instructions | Network error, unsupported browser |
| Parse error | File/line + message | Syntax error |
| Type error | Type mismatch details | `int` vs `str` |
| Runtime error | Message + partial output | Division by zero, panic |
| Internal error | JS exception with context | Unexpected state |

### Graceful Degradation

- If WASM fails to load: Show error with manual setup instructions
- If format fails: Show error, do not run
- If execution times out: Allow cancel (future enhancement)

---

## Security Considerations

### Sandboxing

**Decision**: WASM execution is inherently sandboxed.

**Guarantees**:
- No filesystem access
- No network access
- Memory limited by browser
- CPU limited by browser (can freeze tab, not system)

### Code Execution

**Decision**: Only interpret user code; no arbitrary host function calls.

**Restrictions**:
- `print()` captures to buffer, does not access console directly
- No FFI capabilities in WASM build
- No file I/O capabilities

### Malicious Input

**Mitigations**:
- Infinite loops will freeze the tab, not the system
- Memory exhaustion limited by browser
- Users can close/refresh the tab to recover

---

## Decisions Summary

| # | Question | Decision |
|---|----------|----------|
| 1 | Separate WASM crate? | Yes — avoid Salsa/LLVM dependencies |
| 2 | WASM API format? | JSON strings via wasm-bindgen |
| 3 | Code editor? | Monaco Editor (VS Code engine) |
| 4 | Code sharing? | URL fragment with base64 encoding |
| 5 | Auto-format? | Yes, before every run |
| 6 | Static vs bundled WASM? | Static serving from /public/wasm/ |
| 7 | SSR handling? | Dynamic import, client-only components |
| 8 | Dev workflow? | Hot reload via window.reloadWasm() |
| 9 | No @main behavior? | Evaluate single expression; otherwise require @main |
| 10 | Stdlib availability? | Prelude only (no capability-requiring modules) |

---

## Alternatives Considered

### Alternative 1: Server-Side Execution

Run Ori code on a backend server instead of WASM.

**Rejected because**:
- Requires infrastructure (servers, scaling, monitoring)
- Adds latency for each execution
- Security complexity (sandboxing server processes)
- Cost scales with usage

### Alternative 2: Tree-sitter for Highlighting

Use tree-sitter instead of Monaco's Monarch.

**Rejected because**:
- Monaco already includes excellent highlighting infrastructure
- Adding tree-sitter would increase bundle size
- Monaco provides editing features beyond highlighting

### Alternative 3: CodeMirror Instead of Monaco

Use CodeMirror as the editor component.

**Rejected because**:
- Monaco provides VS Code-like experience users expect
- Monaco has better TypeScript integration
- Monaco's highlighting system is well-documented

### Alternative 4: Compile to WebAssembly (Not Interpret)

Generate WASM bytecode from Ori instead of interpreting.

**Rejected because**:
- Significant additional complexity
- Slower feedback loop (compile + link + run vs interpret)
- Interpreter is sufficient for playground use cases

---

## Future Enhancements

### Short-term

- **Execution timeout**: Cancel long-running code
- **Multiple files**: Simulate modules with tabs
- **Keyboard shortcuts**: Format, examples, etc.

### Medium-term

- **Autocomplete**: Type-aware completions
- **Error highlighting**: Inline error markers
- **Saved snippets**: Local storage for drafts

### Long-term

- **Collaborative editing**: Share live sessions
- **Test execution**: Run tests in playground
- **REPL mode**: Incremental evaluation

---

## Standard Library Availability

**Decision**: The playground includes the interpreter's prelude only.

**Available**:
- All prelude types: `Option`, `Result`, `Error`, `Ordering`, `PanicInfo`, etc.
- All prelude traits: `Eq`, `Comparable`, `Clone`, `Debug`, `Iterator`, `Printable`, etc.
- All built-in functions: `print()`, `assert()`, `panic()`, `dbg()`, `todo()`, `unreachable()`, etc.
- Basic collection methods: `map`, `filter`, `fold`, `find`, `collect`, `any`, `all`

**Not Available** (require capabilities or FFI):
- `std.time` — requires Clock capability
- `std.fs` — requires FileSystem capability
- `std.http` — requires Http capability
- `std.crypto` — requires Crypto capability and native FFI
- `std.json` — requires FFI (yyjson/pure Ori)

**Future Enhancement**: Consider adding mockable stdlib modules that work without real capabilities.

---

## Spec Changes Required

None — this proposal covers tooling implementation, not language semantics.

---

## Related Documents

| Document | Relationship |
|----------|--------------|
| `playground/wasm/src/lib.rs` | WASM crate implementation |
| `website/src/components/playground/` | UI component implementation |
| `proposals/approved/lsp-implementation-proposal.md` | Related tooling proposal |

---

## Summary

| Aspect | Decision |
|--------|----------|
| Architecture | Separate WASM crate with portable deps |
| WASM size | ~828 KB (optimized for size) |
| Editor | Monaco with Ori syntax highlighting |
| Sharing | URL fragment with base64 code |
| Deployment | Static WASM serving |
| Security | Browser sandboxing, no FFI |
