---
paths:
  - "**/compiler/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

**Expression-based — NO `return`**: Last expression IS the value. Exit via `?`/`break`/`panic`.

# Compiler

## Architecture
- **Deps**: `oric` → `ori_types/eval/patterns` → `ori_parse` → `ori_lexer` → `ori_ir/diagnostic`
- **IO**: only in `oric`; core crates pure
- **No phase bleeding**: parser ≠ type-check, lexer ≠ parse

## Memory
- Arena + ID (`ExprArena`+`ExprId`), not `Box<Expr>`
- Intern identifiers (`Name`), not `String`
- Newtypes for IDs; no `Arc` cloning in hot paths
- `&'a T` for borrowing, `Arc<T>` only for shared ownership

## Dispatch
- Enum for fixed sets (exhaustiveness, static dispatch)
- `dyn Trait` only for user-extensible
- Cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`

## API
- >3 params → config struct
- No boolean flags
- Return iterators, not `Vec`
- RAII guards for context

## Salsa
- Query types: `Clone, Eq, PartialEq, Hash, Debug`
- No `Arc<Mutex<T>>`, fn pointers, or `dyn Trait`
- Deterministic (no random/time/IO)

## Diagnostics
- All errors have spans
- Accumulate, don't bail
- Imperative: "try using X"
- No `panic!` on user errors

## Tracing — ALWAYS USE FOR DEBUGGING

**`ORI_LOG` is your first debugging tool.** Before adding `println!`, before reading code line-by-line, turn on tracing.

### Environment Variables
- **`ORI_LOG`**: Filter string (`RUST_LOG` syntax). Falls back to `RUST_LOG`. Default: `warn`.
- **`ORI_LOG_TREE=1`**: Hierarchical tree output with indentation (uses `tracing-tree`)
- Setup: `compiler/oric/src/tracing_setup.rs`, initialized in `main.rs`

### Quick Reference
```bash
ORI_LOG=debug ori check file.ori                    # All phases at debug level
ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check f.ori  # Type inference call tree
ORI_LOG=ori_eval=debug ori run file.ori             # Evaluator method dispatch
ORI_LOG=oric=debug ori check file.ori               # Salsa query execution
ORI_LOG=ori_types=debug,ori_eval=debug ori run f.ori    # Multiple targets
```

### Phase Dumps (debug builds only)
```bash
ORI_DUMP_AFTER_PARSE=1 ori check file.ori      # AST after parse
ORI_DUMP_AFTER_TYPECK=1 ori check file.ori     # Typed IR after typeck
ORI_DUMP_AFTER_ARC=1 ori build file.ori        # ARC IR with RC strategies
ORI_DUMP_AFTER_LLVM=1 ori build file.ori       # Annotated LLVM IR
```

### Runtime Instrumentation (AOT binaries)
```bash
ORI_TRACE_RC=1 ./binary                         # RC event trace (alloc/inc/dec/free)
ORI_RT_DEBUG=1 ./binary                          # Runtime assertions (header validation)
ORI_CHECK_LEAKS=1 ./binary                       # Leak check with attribution
```

### Tracing Targets (by crate)
| Target | What it shows |
|--------|--------------|
| `oric` | Salsa queries (lexing, parsing, type checking, evaluating), cache hits/misses |
| `ori_types` | Type checking phases, inference, unification, type errors |
| `ori_eval` | Expression evaluation, method dispatch, function calls |
| `ori_llvm` | LLVM codegen, pattern matching, control flow |
| `ori_parse` | Parser (dependency declared, limited instrumentation) |
| `ori_patterns` | Pattern system (dependency declared, limited instrumentation) |

### Levels
- `error`: Should never happen — internal invariant violations
- `warn`: Recoverable issues
- `debug`: Phase boundaries, query execution, function-level events
- `trace`: Per-expression, hot paths — very verbose

### Coding Guidelines
- Use `tracing` crate, never `println!`/`eprintln!` for debug output
- `#[tracing::instrument]` on public API entry points; use `skip_all` or `skip(arena, engine)` for large/non-Debug args
- Salsa `#[tracked]` functions: use manual `tracing::debug!()` events (not `#[instrument]`)

## Diagnostic Scripts — USE THESE

**Before reading code line-by-line, run the diagnostic scripts.** They extract more signal in seconds than manual investigation in minutes.

```bash
# All-in-one (build + run + leak check + RC stats + IR dump)
diagnostics/diagnose-aot.sh file.ori              # Add --valgrind for memory errors

# Behavioral mismatch (interpreter vs AOT comparison)
diagnostics/dual-exec-debug.sh file.ori           # Auto-dumps IR + RC stats on mismatch

# RC correctness
diagnostics/rc-stats.sh file.ori                  # RC balance per function (flags imbalances)
diagnostics/codegen-audit.sh file.ori             # Static RC + COW + ABI analysis (--strict, --function name)

# IR inspection
diagnostics/ir-dump.sh file.ori                   # Annotated LLVM IR (--raw for undecorated)
diagnostics/ir-diff.sh a.ori b.ori                # Side-by-side IR comparison

# Disassembly
diagnostics/disasm-ori.sh file.ori                # Native disassembly with Ori demangling
```

**In-pipeline codegen audit** (Rust-level, runs during compilation):
```bash
ORI_AUDIT_CODEGEN=1 ori build file.ori            # RC balance, COW sequencing, ABI arg counts
ORI_AUDIT_STRICT=1 ORI_AUDIT_CODEGEN=1 ori build file.ori  # Pessimistic mode
ORI_AUDIT_FUNCTION=name ORI_AUDIT_CODEGEN=1 ori build file.ori  # Filter to specific function
```

**Consistency check**: `diagnostics/check-debug-flags.sh` — validates all `ORI_*` flags are defined, used, and documented.

## Bug Debugging Workflow

When you encounter a bug, follow this order — **do not skip steps**:

1. **STOP** — Do not jump to fixing. Resist the urge.
2. **Consult spec** — `docs/ori_lang/0.1-alpha/spec/` for intended behavior
3. **Run diagnostics** — Choose based on symptom:
   - Wrong output → `diagnostics/dual-exec-debug.sh` (compare eval vs AOT)
   - Crash/segfault → `diagnostics/diagnose-aot.sh --valgrind`
   - Memory leak → `ORI_CHECK_LEAKS=1 ./binary` then `diagnostics/rc-stats.sh`
   - RC corruption → `ORI_TRACE_RC=1 ./binary` then `diagnostics/codegen-audit.sh --strict`
   - Type error → `ORI_LOG=ori_types=debug ori check file.ori`
   - Wrong IR → `diagnostics/ir-dump.sh` and compare with `diagnostics/ir-diff.sh`
4. **Write tests** — MULTIPLE: exact case, edges, variations, guards
5. **Verify tests fail** — proves understanding
6. **Fix the code**
7. **Tests pass unchanged**

## Cascading Fixes = Architectural Smell
- When fixing a bug at one callsite moves the failure to the next layer, **STOP**. Do not patch the next callsite. Diagnose the shared assumption that's wrong across the pipeline.
- If the same logical fix must be applied at 3+ independent callsites, it's a missing abstraction or violated boundary contract — fix at the boundary, not at every consumer.
- Present the architectural issue to the user with 2-3 options (boundary fix, abstraction, workaround) and let them choose.

## Style
- Functions < 100 lines (strongly prefer shorter — target < 50)
- No dead code, no `#[allow(clippy)]` without reason
- Use `//!`/`///` docs

## Testing
- TDD for bugs: tests first, verify fail, fix, tests pass unchanged
- Tests live in sibling `tests.rs` files (not inline): `#[cfg(test)] mod tests;` declaration in source, test body in `tests.rs`
  - `foo.rs` → `foo/tests.rs`
  - `mod.rs` in `bar/` → `bar/tests.rs`
  - `lib.rs` / `main.rs` → `tests.rs` in same directory
- `cargo t` (all), `cargo st` (spec), `./test-all.sh` (full)

## Key Patterns

**TypeChecker (V2)**: InferEngine, Pool, Registries, ModuleChecker

**Method Dispatch**: BuiltinMethods → InherentImpl → TraitImpl (via MethodRegistry)

## Crates
- `ori_ir`: AST, spans, TypeId
- `ori_lexer`: Tokenization
- `ori_parse`: Parser
- `ori_types`: Type checking (V2 — Pool, InferEngine, registries)
- `ori_eval`: Interpreter
- `ori_patterns`: Pattern system
- `ori_llvm`: LLVM backend
- `ori_rt`: AOT runtime
- `ori_diagnostic`: Error reporting
- `oric`: CLI, Salsa

## Change Locations
- Expression: `ori_parse/grammar/expr/`, `ori_types/infer/expr/`, `ori_eval/interpreter/`
- Type: `ori_ir/type_id.rs`, `ori_types/pool/`, `ori_types/check/`
- Method: `ori_types/registry/methods/`, `ori_eval/interpreter/method_dispatch/`
- Derive: `ori_ir/derives/mod.rs` (source of truth), `ori_types/check/registration/`, `ori_eval/interpreter/derived_methods.rs`, `ori_llvm/codegen/derive_codegen/`

## Source of Truth
1. `docs/ori_lang/0.1-alpha/spec/` — authoritative
2. `~/projects/reference_repos/lang_repos/` — Rust, Go, TS, Zig, Gleam, Elm, Roc, Swift, Koka, Lean 4
