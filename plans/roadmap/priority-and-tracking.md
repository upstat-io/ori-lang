# Priority Order & Tracking

## Current Status

### Tier 1: Foundation

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | Type System Foundation | ✅ Complete | All tests pass |
| 2 | Type Inference | ✅ Complete | All tests pass |
| 3 | Traits | ✅ Complete | All tests pass including map.len(), map.is_empty() |
| 4 | Modules | ✅ Complete | All tests pass; module alias, re-export, qualified access all working |
| 5 | Type Declarations | ✅ Complete | Structs, sum types (multi-field variants), newtypes all work |

### Tier 2: Capabilities & Stdlib

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 6 | Capabilities | ✅ Complete | 27/27 tests pass |
| 7A | Core Built-ins | 🔶 Partial | Type conversions, assertions, I/O |
| 7B | Option & Result | 🔶 Partial | Option/Result methods |
| 7C | Collections | 🔶 Partial | Collection methods done (map, filter, fold, find, collect) |
| 7D | Stdlib Modules | 🔶 Partial | retry, validate pending; time/json/fs pending |

### Tier 3: Core Patterns

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 8 | Pattern Evaluation | 🔶 Partial | Data transformation tests pass; retry pattern pending |
| 9 | Match Expressions | 🔶 Partial | guards/exhaustiveness pending |
| 10 | Control Flow | ⏳ Not started | |

### Tier 4: FFI & Interop

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 11 | FFI | ⏳ Not started | Critical for ecosystem |
| 12 | Variadic Functions | ⏳ Not started | |

### Tier 5: Language Completion

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 13 | Conditional Compilation | ⏳ Not started | |
| 14 | Testing Framework | ⏳ Not started | Gap analysis complete (see 14.9) |
| 15A | Attributes & Comments | ⏳ Not started | Simplified attributes, function_seq/exp, inline comments |
| 15B | Function Syntax | ⏳ Not started | Remove dot prefix, default params, function clauses, positional lambdas |
| 15C | Literals & Operators | ⏳ Not started | String interpolation, spread operator, range step |
| 15D | Bindings & Types | ⏳ Not started | Pre/post checks, as conversion, $ bindings, remove dyn |

### Tier 6: Async & Concurrency

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 16 | Async Support | ⏳ Not started | |
| 17 | Concurrency Extended | ⏳ Not started | Select, cancel, enhanced channels |

### Tier 7: Advanced Type System

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 18 | Const Generics | ⏳ Not started | |
| 19 | Existential Types | ⏳ Not started | |

### Tier 8: Advanced Features

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 20 | Reflection | ⏳ Not started | |
| 21A | LLVM Backend | 🔶 Partial | JIT working; 977/996 tests pass (19 skipped); destructuring support added |
| 21B | AOT Compilation | ⏳ Not started | Object file generation, optimization, linking |
| 22 | Tooling | 🔶 Partial | Formatter CLI complete (440 tests); LSP/WASM pending |

---

## Immediate Priority

**Current Focus: Completing Tier 1-3 partial phases**

### What's Next (Priority Order)

1. **Phase 8 (Patterns)** — cache TTL remaining (NOW UNBLOCKED)
   - All compiler patterns work with stubs
   - Need: cache TTL with Duration, cache capability enforcement

2. **Phase 9 (Match)** — Guards and exhaustiveness
   - Basic pattern matching works
   - Need: `.match(guard)` syntax, exhaustiveness checking

3. **Phase 7 (Stdlib)** — retry, validate
   - Collection methods complete (map, filter, fold, find, collect, any, all)
   - Need: `retry` function, `validate` function

### Recent Completions

**Phase 22.1 (Formatter CLI)** — ✅ COMPLETED 2026-01-30
- `ori fmt` command with full CLI integration
- Width-based breaking, comment preservation, doc comment reordering
- Incremental formatting (30% speedup), parallel processing (2.4x speedup)
- 440 tests (unit, golden, idempotence, property-based, incremental)
- Documentation: user guide, integration guide, troubleshooting, style guide
- See `plans/ori_fmt/` for detailed tracking; LSP/WASM deferred to Phase 22.2

**Phase 4 (Modules)** — ✅ COMPLETED 2026-01-28
- Module alias imports (`use std.http as http`)
- Re-exports (`pub use`)
- Qualified access type checking (`http.get(...)`)
- Type checker support for ModuleNamespace

**Phase 6 (Capabilities)** — ✅ COMPLETED 2026-01-25
- 27/27 capability tests pass

**Phase 5.2 (Sum Types)** — ✅ COMPLETED 2026-01-28
- Unit, single-field, and multi-field variants all work
- Pattern matching on variants works
- LLVM backend support complete

**Phase 5.3 (Newtypes)** — ✅ COMPLETED 2026-01-28
- Nominal type identity (newtype != underlying type)
- Constructor pattern (`UserId("abc")`)
- `unwrap()` method to extract inner value
- Runtime transparent (same as underlying at LLVM level)

**Phase 7 (Stdlib Collection Methods)** — ✅ COMPLETED 2026-01-25
- `map`, `filter`, `fold`, `find` on lists
- `collect`, `map`, `filter`, `fold` on ranges
- `len`, `is_empty` on maps
- `any`, `all` on lists
- Rosetta code examples updated (2026-01-28)

### Approved Proposals

**`as` Conversion Syntax** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/as-conversion-proposal.md`
- Implementation: Phase 15.7
- Adds `As<T>`, `TryAs<T>` traits; removes `int()`, `float()`, `str()`, `byte()` special cases
- Blocked on: Phase 3 (needs `As<T>` and `TryAs<T>` traits in prelude)

**Default Parameter Values** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/default-parameters-proposal.md`
- Implementation: Phase 15.8
- Allows `param: Type = default_expr` syntax; call-time evaluation; works with named args
- Blocked on: None (can be implemented independently)

**Multiple Function Clauses** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/function-clauses-proposal.md`
- Implementation: Phase 15.9
- Pattern matching in function parameters; `if` guards; first-clause establishes signature
- Blocked on: Phase 9 (needs exhaustiveness checking from match)

**Spread Operator** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/spread-operator-proposal.md`
- Implementation: Phase 15.10
- `...` operator for lists (`[...a, ...b]`), maps (`{...a, ...b}`), and structs (`T { ...s, x: v }`)
- Blocked on: None (can be implemented independently)

**Computed Map Keys** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/computed-map-keys-proposal.md`
- Implementation: Phase 15C.4
- Bare identifiers as literal string keys (TypeScript/JS style): `{timeout: 30}` → `{"timeout": 30}`
- Computed keys with brackets: `{[expr]: value}` evaluates `expr`
- Reserved keywords allowed as literal keys: `{if: 1, type: "user"}`
- Blocked on: Spread operator proposal (approved)

**Simplified Bindings with `$` for Immutability** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/simplified-bindings-proposal.md`
- Implementation: Phase 15.11
- `let x` is mutable, `let $x` is immutable; removes `mut` keyword
- Module-level bindings require `$` prefix; `$` is modifier, not part of name
- Blocked on: None (can be implemented independently)

**Remove `dyn` Keyword for Trait Objects** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/remove-dyn-keyword-proposal.md`
- Implementation: Phase 15.12
- Trait names used directly as types mean "any value implementing this trait"
- Removes Rust jargon (`dyn`); follows Go/TypeScript/Java pattern
- Blocked on: None (grammar change only)

**Range with Step** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/range-step-proposal.md`
- Implementation: Phase 15.13
- Adds `by` keyword for range step: `0..10 by 2`, `10..0 by -1`
- Integer-only; zero step panics; mismatched direction produces empty range
- Blocked on: None (can be implemented independently)

**Iterator Traits** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/iterator-traits-proposal.md`
- Implementation: Phase 3.8
- Four core traits: `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`
- Functional `next()` returning `(Option<Item>, Self)`; fused guarantee
- Default methods: map, filter, fold, find, collect, rev, last, cycle, etc.
- Infinite iterators: `repeat(value)` function, `Iterator.cycle()` method
- Added to prelude: `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`, `repeat`
- Blocked on: None (builds on existing Phase 3 infrastructure)

**Clone Trait** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/clone-trait-proposal.md`
- Implementation: Phase 3.7
- Formal definition for `Clone` trait with `@clone (self) -> Self` method
- Implementations for all primitives including Duration and Size
- Element-wise recursive cloning for collections
- Channel integration deferred to parallel-concurrency-proposal
- Blocked on: None (fills spec gap)

**Debug Trait** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/debug-trait-proposal.md`
- Implementation: Phase 3.9
- Separate `Debug` trait for developer-facing structural representation
- Derivable for any type whose fields implement Debug
- Shows escaped strings (`"\"hello\""`) for clarity
- Standard implementations for all primitives, collections, Option, Result
- Blocked on: `as` conversion syntax, `str.escape()`, `Iterator.join()`

**Pre/Post Checks** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/checks-proposal.md`
- Implementation: Phase 15.5
- Extends `run` with `pre_check:` and `post_check:` properties for contract-style checks
- Multiple conditions via multiple properties (not list syntax)
- Custom messages with `| "message"` syntax
- Scope: pre_check can only access outer scope; post_check can access body bindings
- Compile error if post_check used with void body
- Check modes (enforce/observe/ignore) deferred to future proposal
- Blocked on: None (can be implemented independently)

**Error Return Traces** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/error-return-traces-proposal.md`
- Implementation: Phase 10.4
- Automatic trace collection at `?` propagation points (Zig-inspired)
- New prelude types: `TraceEntry` (function, file, line, column)
- New prelude traits: `Into<T>` (conversion), `Traceable` (optional for custom errors)
- Error methods: `.trace()`, `.trace_entries()`, `.has_trace()`
- Result method: `.context(msg)` for adding context while preserving trace
- Traces always collected (all builds), not build-mode dependent
- Blocked on: None (can be implemented independently)

**No Circular Imports** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/no-circular-imports-proposal.md`
- Implementation: Phase 4.4, 4.7
- Circular import dependencies are compile-time errors (E5003)
- Core cycle detection already implemented
- Remaining: enhanced error messages, report all cycles, CLI tooling (`ori check --cycles`, `ori graph --imports`)
- Blocked on: None (core detection complete)

**Incremental Test Execution** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/incremental-test-execution-proposal.md`
- Implementation: Phase 14.2, 14.11
- Explicit floating tests with `tests _` syntax (no more naming convention)
- Attached tests auto-run during `ori check` when targets change
- Non-blocking by default; `--strict` for CI
- Cache in `.ori/cache/test/`; threshold configurable via `ori.toml`
- Blocked on: None (can be implemented independently)

**Integer Overflow Behavior** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/overflow-behavior-proposal.md`
- Implementation: Phase 7.11
- Integer arithmetic panics on overflow by default (safe, catches bugs)
- Explicit alternatives in `std.math`: `saturating_*`, `wrapping_*`, `checked_*`
- Type bounds constants: `int.min`, `int.max`, `byte.min`, `byte.max`
- Consistent behavior in debug and release builds (unlike Rust)
- Blocked on: None (can be implemented independently)

**Sendable Trait and Role-Based Channels** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/sendable-channels-proposal.md`
- Implementation: Phase 17.1-17.5
- `Sendable` auto-implemented marker trait for types safe to send across task boundaries
- Role-based channels: `Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>`
- Channel constructors: `channel()`, `channel_in()`, `channel_out()`, `channel_all()`
- Ownership transfer on send (value consumed, prevents data races)
- `nursery` pattern for structured concurrency with guaranteed task completion
- Blocked on: Phase 16 (Async Support)

**String Interpolation** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/string-interpolation-proposal.md`
- Implementation: Phase 15.6
- Two string types: `"..."` (regular, no interpolation), `` `...` `` (template, with `{expr}` interpolation)
- Type-safe via `Printable` trait; expressions inside `{}` must implement `Printable`
- Format specifiers: `{value:.2}`, `{count:05}`, `{hex:X}`, alignment (`<`, `>`, `^`)
- `Formattable` trait in prelude with blanket impl from `Printable`
- Escapes: `{{`/`}}` for literal braces in template strings; `` \` `` for literal backtick
- Multi-line template strings preserve whitespace exactly (no auto-dedent)
- Blocked on: None (can be implemented independently)

**Remove Dot Prefix from Named Arguments** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/remove-dot-prefix-proposal.md`
- Implementation: Phase 15.3
- Dot removal done; named argument enforcement for built-ins pending
- All functions require named arguments — no exceptions
- Built-ins need update: `print(msg:)`, `len(collection:)`, `assert(condition:)`, etc.
- Type conversions use `as` syntax (see `as-conversion-proposal.md`)
- Blocked on: None (can be implemented independently)

**Structured Diagnostics and Auto-Fix** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/structured-diagnostics-autofix.md`
- Implementation: Phase 22.7
- JSON output mode (`--json`) for AI agents and IDE integrations
- Auto-fix capability (`--fix`, `--fix=all`) with applicability levels
- Improved human-readable output with Rust-style source snippets
- Core types (`Applicability`, `Suggestion`, `Substitution`) already exist; enhances JSON emitter
- Blocked on: None (can be implemented independently)

**Causality Tracking (`ori impact`, `ori why`)** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/why-command-proposal.md`
- Implementation: Phase 22.6
- `ori impact @target` shows blast radius before changing code
- `ori why @target` traces causality chain after something breaks
- Exposes Salsa's dependency tracking to users for debugging
- Supports `--verbose`, `--diff`, `--graph` output modes
- Blocked on: None (Salsa infrastructure already exists)

**Simplified Attribute Syntax** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/simplified-attributes-proposal.md`
- Implementation: Phase 15.1
- Changes `#[name(...)]` to `#name(...)` — removes bracket noise
- Generalized attributes: any attribute can appear before any declaration
- Compiler validates which attributes valid for which declarations
- Blocked on: None (lexer/parser change only)

**Positional Lambdas for Single-Parameter Functions** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/single-lambda-positional-proposal.md`
- Implementation: Phase 15.14
- Allow omitting parameter names when calling single-param functions with inline lambdas
- `items.map(x -> x * 2)` instead of `items.map(transform: x -> x * 2)`
- Only for lambda literals, not function references; `self` excluded from param count
- Complementary to Anonymous Parameters proposal (both can coexist)
- Blocked on: None (type checker change only)

**Default Implementations (`def impl`)** — ✅ APPROVED 2026-01-29
- Proposal: `proposals/approved/default-impl-proposal.md`
- Implementation: Phase 6.10
- `def impl Trait { ... }` syntax for default trait implementations
- Importing a trait automatically binds the default (no `with...in` for common case)
- Stateless methods (no `self`); use module-level `$` bindings for configuration
- Override with `with Trait = other in ...` for testing/custom config
- Blocked on: None (can be implemented independently)

**Test Execution Model** — ✅ APPROVED 2026-01-29
- Proposal: `proposals/approved/test-execution-model-proposal.md`
- Implementation: Phase 14.12
- Consolidates dependency-aware and incremental test execution into unified implementation spec
- Defines `TestRegistry` data structure, content hashing (whitespace/comment normalization), cache format
- Cache maintenance: prune deleted functions on successful build; automatic invalidation via inputs_hash
- Clarifies `--clean` excludes floating tests (they always require `ori test`)
- Blocked on: None (implementation reference for Phase 14)

**Task and Async Context Definitions** — ✅ APPROVED 2026-01-29
- Proposal: `proposals/approved/task-async-context-proposal.md`
- Implementation: Phase 17.0
- Formal definitions for tasks, async contexts, and suspension points
- `@main` must declare `uses Async` for concurrency patterns
- Capture-by-value with ownership transfer for task closures
- Atomic reference counting for cross-task values
- Blocked on: Phase 16 (Async Support)

**Closure Capture Semantics** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/closure-capture-semantics-proposal.md`
- Implementation: Phase 17.0
- Formalizes capture-by-value semantics and capture timing
- Captured bindings are immutable within closures
- Task closures require Sendable captures with move semantics
- Closure types inferred, coerce to function types
- Blocked on: None (formalizes existing spec)

**Sendable Trait and Interior Mutability Definition** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/sendable-interior-mutability-proposal.md`
- Implementation: Phase 17.1
- Clarifies interior mutability: does not exist in user code, only runtime resources
- Channel types (Producer, Consumer) ARE Sendable (fixes spec ambiguity)
- Manual Sendable impl forbidden
- Closure Sendability via capture analysis at task boundaries
- Blocked on: None (clarification of existing spec)

**Capability Composition Rules** — ✅ APPROVED 2026-01-29
- Proposal: `proposals/approved/capability-composition-proposal.md`
- Implementation: Phase 6.11
- Multi-binding `with` syntax: `with Http = a, Cache = b in expr`
- Capability variance: more caps can call fewer (not reverse)
- Resolution priority: inner with → outer with → imported def impl → local def impl → error
- Explicit declaration requirement (no inference)
- Async binding prohibition (E1203)
- Blocked on: Phase 6.10 (def impl)

**Default Implementation Resolution** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/default-impl-resolution-proposal.md`
- Implementation: Phase 6.12
- `without def` import syntax: `use "module" { Trait without def }`
- Conflict detection: one `def impl` per trait per scope (E1000, E1001)
- Resolution order: with...in > imported def > module-local def
- Re-export stripping: `without def` permanently strips default from export path
- Blocked on: Phase 6.10 (def impl)

**Trait Resolution and Conflict Handling** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/trait-resolution-conflicts-proposal.md`
- Implementation: Phase 3.10
- Diamond problem: single impl satisfies all inheritance paths
- Coherence/orphan rules: trait or type must be local
- Method resolution: Inherent > Trait > Extension
- Super trait calls: unified `Trait.method(self)` syntax
- Associated type disambiguation: `Type::Trait::AssocType`
- Extension conflict detection (including re-exports)
- Blocked on: None (builds on existing Phase 3 infrastructure)

**Object Safety Rules** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/object-safety-rules-proposal.md`
- Implementation: Phase 3.11
- Three rules: No Self return, No Self param (except receiver), No generic methods
- Error codes: E0800 (Self return), E0801 (Self param), E0802 (generic method)
- Object-safe wrapper patterns using `Arc<Trait>`
- All bounds in trait object composition must be object-safe
- Blocked on: None (builds on existing Phase 3 infrastructure)

**Custom Subscripting (Index Trait)** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/custom-subscripting-proposal.md`
- Implementation: Phase 3.12
- Read-only `Index<Key, Value>` trait for custom `[]` syntax
- Desugars `x[k]` to `x.index(key: k)`
- Multiple index types per type (e.g., JsonValue with str and int)
- Flexible return types: `T`, `Option<T>`, or `Result<T, E>`
- No IndexMut (Ori has no `mut`); mutation via explicit methods
- Added to prelude
- Blocked on: None (builds on existing Phase 3 infrastructure)

**Const Evaluation Termination** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/const-evaluation-termination-proposal.md`
- Implementation: Phase 18.0
- Resource limits: 1M steps, 1000 depth, 100MB memory, 10s time (all configurable)
- Partial evaluation: required compiler behavior
- Local mutation in const functions: allowed
- Loop expressions in const functions: allowed
- Caching: by function + args hash
- Blocked on: None (can be implemented independently)

**Parallel Execution Guarantees** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/parallel-execution-guarantees-proposal.md`
- Implementation: Phase 17.6
- Execution order: tasks start in list order, may complete in any order
- Result order: always matches original task list order
- Parameters: `max_concurrent: Option<int> = None`, `timeout: Option<Duration> = None`
- Resource exhaustion: per-task `CancellationError { reason: ResourceExhausted }`
- No early termination: use `nursery` with `FailFast` for that
- Cooperative cancellation: `is_cancelled()` available in tasks
- Blocked on: Phase 16 (Async Support), nursery-cancellation-proposal

**Nursery Cancellation Semantics** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/nursery-cancellation-proposal.md`
- Implementation: Phase 17.7
- Model: cooperative cancellation (not preemptive)
- Checkpoints: suspension points, loop iterations, pattern entry
- Error modes: FailFast (cancel all), CancelRemaining (cancel pending), CollectAll (no cancel)
- Types: `CancellationError`, `CancellationReason` (Timeout, SiblingFailed, NurseryExited, ExplicitCancel, ResourceExhausted)
- API: `is_cancelled()` built-in function, automatic loop checking
- Cleanup: guaranteed destructor execution during cancellation
- Blocked on: Phase 16 (Async Support)

**std.time API** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-time-api-proposal.md`
- Implementation: Phase 7.18
- Core types: `Instant` (UTC timestamp), `DateTime` (with timezone), `Date`, `Time`, `Timezone`, `Weekday`
- Duration extension methods (require import from std.time)
- Formatting with pattern specifiers, ISO 8601 support
- Parsing with explicit timezone parameter (`tz: Timezone = Timezone.utc()`)
- Clock capability: `now() -> Instant`, `local_timezone() -> Timezone`
- MockClock with interior mutability for testing
- Blocked on: None (can be implemented independently)

**std.json API** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-json-api-proposal.md`
- Implementation: Phase 7.19
- Core types: `JsonValue` (sum type), `JsonError`, `Json` trait
- Parsing: `parse()` returns `JsonValue`, `parse_as<T>()` for typed deserialization
- Serialization: `stringify()`, `stringify_pretty()`, `to_json_string()`, `to_json_string_pretty()`
- `#derive(Json)` with field attributes (`#json(rename:, skip, default:, flatten)`)
- Flatten conflicts are compile errors
- `as_int()` returns `None` for non-integers (no truncation)
- Streaming API: `JsonParser` implements `Iterator` with `JsonEvent` items
- Built-in extensions: `Duration` (ISO 8601), `Size` (bytes)
- Precision note: integers >2^53 may lose precision
- Blocked on: None (can be implemented independently)

**std.json FFI** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-json-api-ffi-revision.md`
- Implementation: Phase 7D.7.8
- Backend: yyjson (native), JavaScript JSON API (WASM), pure Ori (fallback)
- Native: ~10x faster parsing/serialization via yyjson C library
- WASM: uses browser's JSON.parse/stringify, converts eagerly to JsonValue tree
- Pure Ori: complete recursive descent parser for restricted environments
- Streaming: yyjson tree walking (native), pure Ori (WASM)
- Design decisions: eager JS→JsonValue conversion for cross-platform consistency
- Blocked on: Computed Map Keys proposal (for `{[key]: value}` syntax)

**std.fs API** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-fs-api-proposal.md`
- Implementation: Phase 7.20
- Core types: `Path`, `FileInfo`, `FileError`/`FileErrorKind`, `WriteMode`, `Permissions`
- Reading: `read()`, `read_bytes()`, `read_lines()`, `FileReader` for streaming
- Writing: `write()`, `write_bytes()`, `write_with()`, `FileWriter` for streaming
- Directories: `list_dir()`, `walk_dir()`, `create_dir()`, `remove_dir()`
- Operations: `copy()`, `move()`, `rename()`, `remove()`
- Info: `info()`, `exists()`, `is_file()`, `is_dir()` (bool returns for simpler API)
- Glob: `glob()` with `*`, `**`, `?`, `[abc]`, `{a,b}` patterns
- Temp files: `create_temp_file()`, `with_temp_file()` (scoped auto-cleanup)
- Uses `Instant` from `std.time` for file timestamps
- `WriteMode.Create` errors if file exists; `Truncate` overwrites
- Blocked on: std.time (for Instant type)

**std.fs FFI (POSIX)** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-fs-api-ffi-revision.md`
- Implementation: Phase 7D.8 (part of std.fs)
- Backend: POSIX/libc for Unix platforms (Linux, macOS, BSD)
- Windows support deferred to separate proposal
- Platform-specific errno access (`__errno_location` on Linux, `__error` on macOS/BSD)
- Uses `.zeroed()` method for FFI struct initialization
- Blocked on: std.fs API proposal, Fixed-capacity lists proposal

**std.crypto API** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-crypto-api-proposal.md`
- Implementation: Phase 7D.9
- Type-safe asymmetric keys: `SigningKeyPair`, `EncryptionKeyPair`, `KeyExchangeKeyPair` (prevent misuse at compile time)
- Password hashing: `hash_password()`, `verify_password()` with Argon2id
- Symmetric encryption: `generate_key()`, `encrypt()`, `decrypt()` with XSalsa20-Poly1305
- Digital signatures: `sign()`, `verify_signature()` with Ed25519/RSA
- Key exchange: `derive_shared_secret()` with X25519 (Diffie-Hellman)
- Secure random: `random_bytes()`, `random_int()`, `random_uuid()` using CSPRNG
- Memory safety: automatic key zeroization on drop (`#zeroize` attribute)
- Algorithm deprecation: compiler warnings with `#allow(deprecated_algorithm)` suppression
- Crypto capability: non-suspending, added to standard capabilities
- Hash algorithms: `Sha256 | Sha512 | Blake2b` (SHA-384 and BLAKE3 removed — not in libsodium)
- Blocked on: None (can be implemented independently)

**std.crypto FFI (Native)** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/stdlib-crypto-ffi-native-proposal.md`
- Implementation: Phase 7D.9 (part of std.crypto)
- Backend: libsodium (modern algorithms) + OpenSSL (RSA only)
- Language extensions: `[value; count]` array syntax, `#zeroize` attribute
- Scope: Native platforms only; WASM support deferred to separate proposal
- Blocked on: std.crypto API proposal

**Conditional Compilation** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/conditional-compilation-proposal.md`
- Implementation: Phase 13
- Separate `#target(...)` and `#cfg(...)` attributes (platform vs. feature flags)
- Target families: `family: "unix"/"windows"/"wasm"`
- OR conditions: `any_os`, `any_arch`, `any_feature`
- Negation: `not_os`, `not_arch`, `not_debug`, `not_feature` (unified prefix style)
- File-level conditions: `#!target(os: "linux")` at file start
- Compile-time constants: `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release`
- Dead code elimination: false branches not type-checked
- Feature names must be valid identifiers
- Blocked on: None (can be implemented independently)

**Error Trace Async Semantics** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/error-trace-async-semantics-proposal.md`
- Implementation: Phase 7B.3, Phase 16.4
- Trace ordering: most recent first (like stack traces)
- Function names in traces include `@` prefix
- Panic message format: `<message> at <file>:<line>:<column>`
- Result trace methods: `.trace()`, `.trace_entries()`, `.has_trace()`
- Task boundary marker: `<task boundary>` pseudo-entry for async traces
- Context storage: separate from traces, most recent first
- Blocked on: None (Phase 7B.3); Phase 16 for async-specific features (Phase 16.4)

**Developer Convenience Functions** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/developer-functions-proposal.md`
- Implementation: Phase 7A.5
- `todo()`, `todo(reason:)` → `Never` — marks unfinished code with location info
- `unreachable()`, `unreachable(reason:)` → `Never` — marks impossible code paths
- `dbg(value:)`, `dbg(value:, label:)` → `T` — debug print returning value unchanged
- `dbg` uses Print capability (mockable in tests)
- Blocked on: None (can be implemented independently)

**Platform FFI (Native & WASM)** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/platform-ffi-proposal.md`
- Implementation: Phase 11
- Unified FFI for native (C ABI) and WASM (JavaScript interop)
- Syntax: `extern "c" from "lib" { }` and `extern "js" { }`
- Single `FFI` capability for all foreign calls
- `CPtr` opaque pointer type for C, `JsValue` handles for JS objects
- `JsPromise<T>` with implicit resolution (no `await` keyword)
- `unsafe { }` blocks for operations Ori cannot verify
- `#repr("c")` attribute for C-compatible struct layout
- `compile_error("msg")` built-in for platform availability errors
- Blocked on: None (can be implemented independently)

**Fixed-Capacity Lists** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/fixed-capacity-list-proposal.md`
- Implementation: Phase 18.2
- Type syntax: `[T, max N]` for inline-allocated lists with compile-time maximum capacity
- Dynamic length (0 to N elements at runtime), subtype of `[T]`
- Const-generic syntax: `$N: int` for compile-time integer parameters
- Methods: `.capacity()`, `.is_full()`, `.remaining()`, `.try_push()`, `.push_or_oldest()`
- Conversion: `.to_fixed<$N: int>()`, `.try_to_fixed<$N: int>()`
- Trait implementations same as `[T]` with same constraints
- Blocked on: None (part of const-generics infrastructure)

**Test Terminology** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/test-terminology-proposal.md`
- Implementation: Documentation-only (CLI/error message changes tracked in Phase 14)
- Standardizes terminology: "attached tests" (was "targeted"/"bound") and "floating tests" (was "free-floating")
- Updates spec, proposals, and CLAUDE.md
- Blocked on: None (documentation-only)

**Simplified Doc Comment Syntax** — ✅ APPROVED 2026-01-30
- Proposal: `proposals/approved/simplified-doc-comments-proposal.md`
- Implementation: Phase 15A.4
- Removes `#` marker for descriptions (unmarked comments are docs)
- Replaces `@param`/`@field` with `* name:` (markdown-like, context determines meaning)
- Canonical spacing: `// * name: description`
- Blocked on: None (can be implemented independently)

---

## Milestones

### M1: Bootstrapped (Tier 1) — ✅ COMPLETE

- [x] Type system foundation
- [x] Type inference
- [x] **Traits** — complete including map methods
- [x] **Modules** — complete including qualified access
- [x] Type declarations (structs, sum types, newtypes)

**Exit criteria**: Can write programs using traits and modules ✅

### M2: Capabilities & Stdlib (Tier 2) — 🔶 NEAR COMPLETE

- [x] Capabilities ✅ — 27/27 tests pass
- [x] Collection methods ✅ — map, filter, fold, find, collect, any, all
- [ ] Resilience utilities — retry, validate pending

**Exit criteria**: Capabilities working ✅, stdlib collection methods ✅, resilience utilities ❌

### M3: Core Patterns (Tier 3) — 🔶 PARTIAL

- [x] Pattern evaluation — run, try, recurse, parallel, spawn, timeout, with, for all work
- [ ] Cache pattern — TTL support pending
- [ ] Match expressions — basic works, guards/exhaustiveness pending
- [ ] Control flow — for loops work, labeled loops pending

**Exit criteria**: All pattern and control flow constructs working

### M4: FFI & Interop (Tier 4)

- [ ] FFI
- [ ] Variadic functions

**Exit criteria**: Can call C libraries

### M5: Language Complete (Tier 5)

- [ ] Conditional compilation
- [ ] Testing framework
- [ ] Syntax proposals

**Exit criteria**: All core language features complete, testing enforced (Phases 1-15 = "core complete")

### M6: Production Async (Tier 6)

- [ ] Async support
- [ ] Concurrency (select, cancel)

**Exit criteria**: Can write server with graceful shutdown

### M7: Advanced Types (Tier 7)

- [ ] Const generics
- [ ] Existential types

**Exit criteria**: Can write matrix library with compile-time checking

### M8: Full Featured (Tier 8)

- [ ] Reflection
- [ ] Code generation
- [x] Formatter (CLI complete, 440 tests)
- [ ] LSP server
- [ ] WASM playground

**Exit criteria**: Full IDE support, generic serialization

---

## Complexity Estimates

| Phase | Name | Complexity | Compiler Areas |
|-------|------|------------|----------------|
| 1-2 | Types/Inference | Low | Types |
| 3 | Traits | **High** | Parser, type checker, evaluator |
| 4-5 | Modules/Type Decls | Medium | Parser, type checker |
| 6 | Capabilities | Medium | Type checker, evaluator |
| 7 | Stdlib | Medium | Stdlib files |
| 8-10 | Patterns/Match/Control | Medium | Evaluator |
| 11 | FFI | **High** | Lexer, parser, type checker, codegen, runtime |
| 12 | Variadics | Medium | Parser, type checker |
| 13 | Conditional Compilation | Medium | Parser, build system |
| 14 | Testing | Medium | Test runner |
| 15 | Syntax Proposals | Low-Medium | Parser, lexer |
| 16 | Async | Medium | Evaluator, runtime |
| 17 | Concurrency | Medium | Evaluator, runtime |
| 18 | Const Generics | **High** | Type checker, const eval |
| 19 | Existential Types | Medium | Type checker, inference |
| 20 | Reflection | **High** | Codegen, runtime, stdlib |
| 21 | Codegen | **High** | Codegen (LLVM/Cranelift) |
| 22 | Tooling | Medium | External tools |

---

## Compiler Infrastructure Work

These are improvements to the compiler itself (Rust code) that don't fit into language phases but improve maintainability and test coverage.

### Technical Debt Cleanup (Completed 2026-01-25)

Consolidated duplicate code across crates:

- **Lexer**: Single source of truth in `ori_lexer` crate
- **Value types**: Single source in `ori_patterns::value`
- **Error messages**: Wrapper pattern for consistent errors
- **Type inference fixes**: Range types, await errors, type aliases, Self type, config lookup

See `.claude/plans/witty-nibbling-falcon.md` for details.

### Test Consolidation (Completed 2026-01-25)

Reorganized compiler tests following Go-style test scenarios:

**Files created:**
- `oric/src/eval/tests/environment_tests.rs` — 25 tests for scope/binding/capture
- `oric/src/eval/tests/unary_operators_tests.rs` — 40 tests for negation, not, bitwise, try
- `oric/src/eval/tests/expr_tests.rs` — 55 tests for literals, indexing, collection ops
- `oric/src/eval/tests/control_tests.rs` — 28 tests for if/else, pattern binding, loops
- `oric/src/eval/tests/call_tests.rs` — 20 tests for function calls, parameter binding

**Results:**
- Test count increased from 653 to 813 (+160 tests)
- Comprehensive edge case coverage (unicode, boundaries, special float values)
- Tests organized by domain, not by implementation file

**Guidelines:**
- See `docs/compiler/design/appendices/E-coding-guidelines.md` for test organization standards
- Inline tests for small utilities, separate files for comprehensive suites

---

## Test Commands

```bash
# Run ALL tests (Rust + Ori interpreter + LLVM backend)
./test-all

# Individual test suites
cargo t                               # Rust unit tests
cargo st                              # Ori language tests (interpreter)
./llvm-test                           # LLVM Rust unit tests
./docker/llvm/run.sh ori test tests/  # Ori language tests (LLVM)

# By category
cargo st tests/spec/types/        # Tier 1
cargo st tests/spec/traits/       # Tier 1
cargo st tests/spec/capabilities/ # Tier 2
cargo st tests/spec/patterns/     # Tier 3
```

---

## Current Test Results (2026-01-28)

**Rust unit tests:** 1286 passed, 0 failed

**Ori spec tests:** 920 passed, 0 failed, 19 skipped (939 total)

| Category | Passed | Skipped | Notes |
|----------|--------|---------|-------|
| Types | 70/70 | 0 | ✅ Complete (sum types, multi-field variants, newtypes) |
| Expressions | 17/17 | 0 | ✅ Complete |
| Inference | 28/28 | 0 | ✅ Complete |
| Modules | 40/40 | 0 | ✅ Complete (includes qualified access) |
| Declarations | 8/8 | 0 | ✅ Complete |
| Extensions | 4/4 | 0 | ✅ Complete |
| Capabilities | 27/27 | 0 | ✅ Complete |
| Traits | 119/119 | 0 | ✅ Complete (including map methods) |
| Patterns | 71/76 | 5 | Collection methods done; retry, validate blocked |
| Rosetta | 96/110 | 14 | Stack/queue/string-slice blocked |

**Skipped tests (19 remaining):**
- `retry` function (1): in std.resilience, not implemented
- `validate` function (4): in std.validate, not implemented
- Stack methods (6): push, pop, peek, is_empty, size, clear
- Queue methods (6): enqueue, dequeue, peek, is_empty, size, clear
- String slice (2): string slicing syntax not implemented

**Known issue:** Parallel test runner has a thread-safety panic (index out of bounds on ExprArena). Individual test directories pass; full test run may show panic but still completes.

---

## Draft Proposals Pending Review

No proposals currently pending.

**Recently Approved:**
- Platform FFI (Native & WASM) — approved 2026-01-30
- Developer Convenience Functions — approved 2026-01-30
- Debug Trait — approved 2026-01-28
- `as` Conversion Syntax — approved 2026-01-28
- Iterator Traits — approved 2026-01-28

**Decisions made (no proposal needed):**
- NaN comparisons panic (Phase 7.18)
- Skip `AsRef`/`AsMut` — Ori's value semantics don't need them
- Skip `debug_assert*` — same behavior in all builds

---

## LLVM Backend Status (Phase 21) — Updated 2026-01-28

### Test Results

| Test Suite | Passed | Failed | Skipped | Total |
|------------|--------|--------|---------|-------|
| All Ori tests | 977 | 0 | 19 | 996 |
| Rust unit tests | 204 | 0 | 1 | 205 |

### Architecture (Reorganized)

The LLVM backend follows Rust's `rustc_codegen_llvm` patterns:

| Component | File | Status |
|-----------|------|--------|
| SimpleCx | `context.rs` | ✅ Complete |
| CodegenCx | `context.rs` | ✅ Complete |
| Builder | `builder.rs` | ✅ Complete |
| TypeCache | `context.rs` | ✅ Complete |
| Two-phase codegen | `module.rs` | ✅ Complete |
| Trait abstraction | `traits.rs` | ✅ Complete |

### Completed Features

- [x] Context hierarchy (SimpleCx → CodegenCx)
- [x] Separate Builder type for instruction generation
- [x] Two-phase codegen (declare then define)
- [x] Type caching (scalars + complex types)
- [x] Trait-based abstraction (BackendTypes, BuilderMethods)
- [x] JIT execution via inkwell
- [x] Runtime functions (print, panic, assert, collections)
- [x] Expression codegen (literals, binary ops, unary ops)
- [x] Function codegen (signatures, locals, returns)
- [x] Control flow (if/else, loops, break/continue)
- [x] Pattern matching (match expressions, guards)
- [x] Collections (lists, tuples, structs, Option, Result)
- [x] Generic function support (type variable resolution)

### Pending Features

- [ ] AOT compilation (object file generation)
- [ ] Optimization passes (O1, O2, O3)
- [ ] Debug info (DWARF)
- [ ] Executable linking

### Running LLVM Tests

```bash
# All Ori tests via LLVM backend
./docker/llvm/run.sh ori test

# Spec tests only
./docker/llvm/run.sh ori test tests/spec

# Rust unit tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib
```

**Note:** LLVM development requires Docker. See `.claude/rules/llvm.md` for details.
