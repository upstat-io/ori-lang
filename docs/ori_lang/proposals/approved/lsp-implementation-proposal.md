# Proposal: LSP Implementation

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-31
**Affects:** Tooling, developer experience, compiler infrastructure

---

## Summary

This proposal formalizes the design and implementation decisions for Ori's Language Server Protocol (LSP) implementation, addressing architecture, integration, and design questions not covered in the existing implementation plans.

---

## Problem Statement

The LSP implementation plans (`plans/ori_lsp/`) provide detailed task breakdowns but leave several architectural and design questions unanswered:

1. **Architecture**: How does the LSP crate integrate with existing compiler infrastructure?
2. **Dependencies**: Which external libraries and what async runtime?
3. **Editor Support**: What editors are supported and how are extensions structured?
4. **Test Protocol**: Which test explorer protocol for IDE integration?
5. **Debug Integration**: How does debugging work through the LSP?
6. **Configuration**: How do workspace, project, and user settings interact?
7. **Incremental Analysis**: What is the exact incremental compilation strategy?
8. **Distribution**: How is the LSP server distributed to users?

---

## Architecture

### Crate Structure

```
compiler/
├── ori_lsp/                    # LSP server crate
│   ├── src/
│   │   ├── lib.rs             # Public API for embedding
│   │   ├── server.rs          # Server lifecycle management
│   │   ├── state.rs           # Global state management
│   │   ├── capabilities.rs    # LSP capability negotiation
│   │   ├── config.rs          # Configuration handling
│   │   ├── handlers/          # LSP request/notification handlers
│   │   ├── analysis/          # Semantic analysis infrastructure
│   │   ├── diagnostics/       # Diagnostic generation
│   │   └── test_integration/  # Test status and execution
│   └── Cargo.toml
├── oric/
│   └── src/commands/
│       └── lsp.rs             # `ori lsp` subcommand
```

### Binary Distribution

**Decision**: The LSP server is accessed exclusively via the `ori lsp` subcommand.

**Rationale**:
- Single binary ensures version consistency between compiler and LSP
- Editors call `ori lsp` to start the server — no separate installation
- Simplifies distribution: one package, one binary, one version
- Users who have `ori` have the LSP; no additional installation step

### Compiler Integration

The LSP crate depends on:
- `oric`: Salsa database (`CompilerDb`), queries, orchestration
- `ori_parse`: Parsing and AST
- `ori_typeck`: Type checking
- `ori_diagnostic`: Structured diagnostics
- `ori_fmt`: Code formatting

**Note**: Whether to extract `ori_typeck` to a separate crate will be evaluated during implementation based on actual dependency needs.

---

## Incremental Computation

**Decision**: Use the existing `CompilerDb` Salsa database (`oric/src/db.rs`).

**Rationale**:
- Ori already uses Salsa for incremental compilation
- The LSP reuses existing queries (parsing, type checking) rather than duplicating
- `CompilerDb::load_file` already handles file caching with proper invalidation
- Adding LSP-specific queries (e.g., hover, completions) extends the existing database

The LSP adds queries for:
- Document symbols
- Reference graph
- Test status tracking

Salsa handles cache invalidation automatically when source files change.

---

## External Dependencies

### Core Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `tower-lsp` | ^0.20 | LSP protocol implementation |
| `lsp-types` | ^0.95 | LSP type definitions |
| `tokio` | ^1.0 | Async runtime |
| `dashmap` | ^5.0 | Concurrent hash maps for live document state |

### Async Runtime

**Decision**: Use `tokio` as the async runtime.

**Rationale**:
- Standard choice for Rust async
- `tower-lsp` is built on `tower` which uses `tokio`
- Good debugging and profiling support

---

## Editor Support

### Supported Editors

| Editor | Priority | Extension Type |
|--------|----------|----------------|
| VS Code | P0 | Native extension |
| Neovim | P1 | LSP configuration |
| Emacs | P2 | `lsp-mode` / `eglot` |
| Zed | P2 | Native LSP |
| Helix | P2 | Native LSP |
| JetBrains | P3 | Plugin |

### VS Code Extension Architecture

```
editors/
├── vscode/
│   ├── src/
│   │   ├── extension.ts       # Extension entry point
│   │   ├── client.ts          # LSP client setup
│   │   ├── test-explorer.ts   # Test Explorer API adapter
│   │   ├── debug-adapter.ts   # Debug Adapter Protocol bridge
│   │   └── commands.ts        # Custom commands
│   ├── syntaxes/
│   │   └── ori.tmLanguage.json  # TextMate grammar for basic highlighting
│   ├── package.json
│   └── tsconfig.json
```

### LSP Binary Resolution

**Decision**: Auto-download on first use with fallback to PATH lookup.

The VS Code extension:
1. Checks if `ori` is in PATH
2. If not found, offers to download the Ori toolchain
3. Uses `ori lsp` to start the language server

### TextMate Grammar vs Semantic Tokens

The extension provides:
1. **TextMate grammar** for immediate syntax highlighting before LSP connects
2. **Semantic tokens** for enhanced, context-aware highlighting once LSP is ready

The semantic tokens ADD to TextMate, not replace entirely, ensuring degraded-but-functional highlighting without LSP.

---

## Test Explorer Protocol

### Protocol Choice

**Decision**: Use VS Code's native Test Explorer API (TestController).

**Rationale**:
- Native VS Code integration
- No additional extension dependencies
- Works with Test Explorer UI extension

### Test Discovery

```typescript
// Test tree structure
workspace/
├── src/
│   └── _test/
│       ├── api.test.ori
│       │   ├── @test_fetch_success
│       │   ├── @test_fetch_error
│       │   └── @test_fetch_retry
│       └── math.test.ori
│           ├── @test_add
│           └── @test_divide
```

### Custom LSP Methods

**Decision**: Use custom JSON-RPC methods (not a separate test server).

For test integration beyond standard LSP, we define custom request/notification methods:

```typescript
// Request: Get tests targeting a function
interface OriTestsForFunctionRequest {
  method: 'ori/testsForFunction';
  params: { uri: string; functionName: string };
}

// Response
interface OriTestsForFunctionResponse {
  tests: Array<{
    name: string;
    uri: string;
    range: Range;
    status: 'passing' | 'failing' | 'unknown';
  }>;
}

// Notification: Test results updated
interface OriTestResultsNotification {
  method: 'ori/testResults';
  params: {
    uri: string;
    results: Array<{
      testName: string;
      status: 'passed' | 'failed' | 'skipped';
      message?: string;
      duration?: number;
    }>;
  };
}
```

---

## Debug Adapter Protocol (DAP)

### Integration Strategy

**Decision**: DAP support is Phase 2 (post-MVP).

For MVP:
- Test execution only (no debugging)
- Printf-style debugging via `dbg(value:)`

For Phase 2:
- Separate DAP server (`ori debug-adapter`)
- VS Code extension bridges LSP and DAP

### DAP Features (Phase 2)

| Feature | Priority | Notes |
|---------|----------|-------|
| Breakpoints | P0 | Line and function breakpoints |
| Step execution | P0 | Step in/over/out |
| Variable inspection | P0 | Local and captured variables |
| Watch expressions | P1 | Evaluate arbitrary expressions |
| Conditional breakpoints | P2 | Break on condition |

**Decision**: DAP server owns breakpoints, LSP provides symbol resolution assistance.

---

## Configuration Management

### Configuration Sources

```
Precedence (highest to lowest):
1. VS Code workspace settings (.vscode/settings.json)
2. Project settings (ori.toml)
3. User settings (~/.config/ori/config.toml)
4. Defaults
```

### ori.toml LSP Section

```toml
[lsp]
# Inlay hints
inlay_hints.types = true
inlay_hints.captures = true
inlay_hints.defaults = false
inlay_hints.parameter_names = true

# Diagnostics
diagnostics.delay_ms = 50
diagnostics.show_untested_warning = true

# Testing
testing.run_on_save = false
testing.show_inline_status = true

# Formatting
formatting.format_on_save = true
```

### Live Configuration Reload

Configuration changes apply without LSP restart:
- `workspace/didChangeConfiguration` notification triggers reload
- Most settings apply immediately
- Some settings (like `diagnostics.delay_ms`) may require pending request cancellation

**Decision**: LSP provides formatting capability; editor controls when to invoke (format-on-save is an editor setting).

---

## Cache Architecture

The LSP maintains layered caches, all managed by Salsa:

```
┌─────────────────┐
│  Document Store │  (file contents, versions)
└────────┬────────┘
         │
┌────────▼────────┐
│   Parse Cache   │  (AST per file, keyed by content hash)
└────────┬────────┘
         │
┌────────▼────────┐
│   Type Cache    │  (types per file, invalidated by dependencies)
└────────┬────────┘
         │
┌────────▼────────┐
│ Reference Graph │  (function→callers, type→usages)
└────────┬────────┘
         │
┌────────▼────────┐
│  Test Results   │  (pass/fail per test, invalidated by source)
└─────────────────┘
```

### Invalidation Rules

Salsa handles invalidation automatically. The logical rules are:

| Cache | Invalidated By |
|-------|----------------|
| Parse cache | File content change |
| Type cache | Parse change in file or dependencies |
| Reference graph | Type change anywhere in project |
| Test results | Source change in test or target |

### Memory Management

**Decision**: Configurable memory limits with sensible defaults.

| Context | Default Budget |
|---------|----------------|
| Small project (<100 files) | < 100MB |
| Medium project (100-1000 files) | < 500MB |
| Large project (1000+ files) | < 2GB |

Configuration via `ori.toml`:
```toml
[lsp]
memory.max_mb = 500
```

---

## Error Recovery

### Partial Analysis Boundaries

```ori
// File with errors - what can LSP provide?

use std.math { sqrt }    // ✓ Import resolution works

type User = {            // ✓ Type definition parsed
    name: str,
    age: int,
}

@process (u: User) -> int =
    u.name + 42          // ✗ Type error here
                         // But LSP still provides:
                         // - Navigation to User type
                         // - Hover on u shows User type
                         // - Completions for u.name, u.age

@other (x: int) -> int = // ✓ This function is fully analyzed
    x * 2
```

### Recovery Strategies

| Error Type | Recovery | Features Available |
|------------|----------|-------------------|
| Lexical error | Stop at error token | Basic highlighting only |
| Parse error | Error node in AST | Highlighting, navigation to valid parts |
| Name error | Mark as unknown | Navigation, some completions |
| Type error | Mark expression as error type | Full navigation, completions |
| Import error | Skip module | Within-file features |

### Error Boundaries

Functions are error boundaries — an error in one function does not prevent analysis of others in the same file.

---

## Semantic Token Types

### Standard Token Types

Using LSP standard types where applicable:

| Ori Construct | LSP Token Type | Modifiers |
|---------------|----------------|-----------|
| `@function` definition | `function` | `declaration` |
| `@function` call | `function` | - |
| `$constant` | `variable` | `readonly`, `static` |
| `type Name` | `type` | `declaration` |
| Type reference | `type` | - |
| Trait name | `interface` | - |
| Sum type variant | `enumMember` | - |
| Parameter | `parameter` | - |
| Local variable | `variable` | - |
| Captured variable | `variable` | `modification` (custom) |

### Custom Token Types

We register additional token types for Ori-specific constructs:

```typescript
const customTokenTypes = [
  'patternKeyword',     // run, try, match, etc.
  'patternProperty',    // .op:, .attempts:, etc.
  'capability',         // Http, Async, etc.
  'testAnnotation',     // @test, tests
];

const customTokenModifiers = [
  'captured',           // Variable captured by closure
  'tested',             // Function has passing tests
  'untested',           // Function has no tests
  'failing',            // Function has failing tests
];
```

**Decision**: Use semantic token modifiers for subtle test status indication, code lens for actions (Run/Debug).

---

## Distribution

### Installation Methods

| Method | Command | Notes |
|--------|---------|-------|
| Cargo | `cargo install ori` | Source build, includes LSP |
| VS Code | Extension prompts to install | Downloads `ori` toolchain |
| Homebrew | `brew install ori` | Includes LSP |
| Apt/Dnf | `apt install ori` | Includes LSP |
| Direct download | GitHub releases | Pre-built binaries |

### Version Compatibility

The LSP is part of the `ori` binary — version always matches.

### Update Strategy

**Decision**: The VS Code extension checks for updates on startup and prompts user.

---

## Performance Benchmarks

### Target Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Cold start | < 500ms | Time to first diagnostic |
| Warm hover | < 20ms | Cached type lookup |
| Cold hover | < 100ms | Parse + type check |
| Completion | < 100ms | From keystroke to result |
| Diagnostics | < 50ms | After edit stabilizes |
| Go-to-definition | < 50ms | Including cross-file |
| Find references | < 200ms | Workspace-wide |
| Format document | < 100ms | 1000-line file |

### Benchmarking Infrastructure

```rust
#[bench]
fn bench_hover_cold(b: &mut Bencher) {
    let server = TestServer::new();
    b.iter(|| {
        server.clear_cache();
        server.hover(test_position());
    });
}
```

**Decision**: Add benchmark suite to CI, track metrics over time, alert on regression (>20% slower).

---

## Implementation Phases

See `plans/ori_lsp/` for detailed phase breakdowns:
- `phase-01-foundation.md` — Server infrastructure, document sync, caching
- `phase-02-navigation.md` — Go-to-definition, find references, symbols
- `phase-03-information.md` — Hover, inlay hints
- `phase-04-editing.md` — Diagnostics, completions, quick fixes
- `phase-05-code-actions.md` — Refactoring actions
- `phase-06-semantic.md` — Semantic highlighting, outline
- `phase-07-test-integration.md` — Test status, code lens, explorer
- `phase-08-workspace.md` — Multi-root, project detection

---

## Decisions Summary

| # | Question | Decision |
|---|----------|----------|
| 1 | Extract `ori_typeck` to separate crate? | Evaluate during implementation |
| 2 | Incremental computation strategy? | Use existing Salsa `CompilerDb` |
| 3 | VS Code extension bundle strategy? | Auto-download on first use |
| 4 | Custom LSP methods vs separate test server? | Custom LSP methods |
| 5 | Breakpoint storage (LSP vs DAP)? | DAP owns breakpoints |
| 6 | Format-on-save control (editor vs LSP)? | Editor controls |
| 7 | Memory budget for LSP? | Configurable (100MB-2GB) |
| 8 | Test status in semantic tokens? | Semantic tokens + code lens |
| 9 | Performance tracking strategy? | CI benchmarks with alerts |

---

## Alternatives Considered

### Alternative 1: Use rust-analyzer as Foundation

Fork or wrap rust-analyzer for Ori.

**Rejected because**:
- Rust-specific assumptions throughout
- More work to adapt than build from scratch
- Different semantic model (Ori's ARC vs Rust's ownership)

### Alternative 2: Tree-sitter Only

Use tree-sitter for parsing without full type checking in LSP.

**Rejected because**:
- Cannot provide accurate type information
- Cannot provide test status
- Limited completion quality

### Alternative 3: Language Server Index Format (LSIF)

Pre-compute index for read-only queries.

**Rejected because**:
- Ori LSP needs live analysis for test status
- LSIF is for code browsing, not active development
- Would add complexity without benefit

---

## Security Considerations

### Code Execution

The LSP server MUST NOT execute arbitrary Ori code except:
- Test execution (user-initiated)
- Const evaluation (compile-time only)

### File Access

The LSP server should only access:
- Files in the workspace
- Standard library files
- Explicit external dependencies

### Untrusted Workspaces

VS Code's workspace trust should be respected:
- In untrusted workspaces, disable auto-test-run
- Warn before executing any code

---

## Spec Changes Required

None — this proposal covers tooling implementation, not language semantics.

---

## Related Documents

| Document | Relationship |
|----------|--------------|
| `plans/ori_lsp/` | Detailed implementation tasks |
| `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` | Original design vision |
| `docs/ori_lang/0.1-alpha/archived-design/12-tooling/01-semantic-addressing.md` | Navigation targets |
| `docs/ori_lang/0.1-alpha/archived-design/12-tooling/03-structured-errors.md` | Diagnostic format |

---

## Summary

| Aspect | Decision |
|--------|----------|
| Crate location | `compiler/ori_lsp/` |
| Binary distribution | `ori lsp` subcommand only |
| Async runtime | `tokio` |
| LSP library | `tower-lsp` |
| Incremental strategy | Existing Salsa `CompilerDb` |
| Primary editor | VS Code (native extension) |
| Test protocol | VS Code TestController + custom LSP methods |
| Debug protocol | DAP (Phase 2) |
| Memory budget | 100MB-2GB configurable |
| Performance tracking | CI benchmarks |
