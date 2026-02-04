# Proposal: AOT Test Backend

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-02-01
**Approved:** 2026-02-01
**Affects:** `compiler/oric/`, `compiler/ori_llvm/`, `compiler/ori_rt/`, test infrastructure

## Summary

Add a new `Backend::AOT` test execution mode that compiles `.ori` tests through the full Ahead-of-Time compilation pipeline: LLVM IR generation → object file emission → linking → binary execution. This validates the complete production compilation path, catching bugs that JIT-only testing would miss.

## Motivation

The LLVM/AOT layer is one of the most critical parts of the compiler pipeline. It's where:

1. **Abstract semantics meet concrete machine instructions** — A bug here silently corrupts the meaning of correct programs
2. **Debugging is hardest** — Bugs in generated code are notoriously difficult to trace back to their source
3. **Impact is multiplicative** — Every compiled program inherits any codegen bugs

### Current State

The test runner has two backends:

| Backend | Flow | What It Tests |
|---------|------|---------------|
| `Interpreter` | AST → tree-walking evaluation | Semantic correctness |
| `LLVM` (JIT) | AST → LLVM IR → JIT execution | IR generation, runtime calls |

**The gap:** Neither backend tests object emission, linking, or actual binary execution.

### The Problem

JIT execution skips critical production code paths:

```
Current LLVM Backend (JIT):
  Parse → TypeCheck → LLVM IR → [JIT Execute in-process]
                                      ↑
                              Skips these entirely:
                              - ObjectEmitter
                              - LinkerDriver
                              - Runtime linking
                              - Binary execution
```

Bugs that could hide:
- Incorrect object file format or section alignment
- Missing or mislinked runtime symbols
- Platform-specific linker flag issues
- ABI mismatches between generated code and runtime
- Binary execution semantics differing from JIT

### Why This Matters

Production Ori programs will **always** go through AOT compilation. Users will:
```bash
ori build src/main.ori -o myapp
./myapp
```

If we only test via JIT, we're not testing what users actually run.

## Design

### Scope

**In scope (this proposal):**
- Native target AOT testing (Linux, macOS, Windows)
- `--backend=aot` CLI flag
- `AotTestExecutor` infrastructure
- Runtime library panic detection API

**Out of scope (future work):**
- WASM AOT testing via wasmtime/wasmer
- Cross-compilation testing via QEMU
- Batch compilation optimization

### New Backend Variant

```rust
// compiler/oric/src/test/runner.rs

pub enum Backend {
    #[default]
    Interpreter,
    LLVM,      // JIT execution (existing)
    AOT,       // Full compilation pipeline (new)
}
```

### Runtime Library Requirements

The AOT test executor requires these runtime functions in `ori_rt`:

| Function | Signature | Purpose |
|----------|-----------|---------|
| `ori_rt_had_panic` | `() -> bool` | Returns true if a panic occurred during execution |
| `ori_rt_reset_panic` | `() -> void` | Resets panic state before test execution |

These must be added to `ori_rt` as part of Phase 1 implementation.

### AOT Test Execution Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                      AOT Test Execution                              │
│                                                                      │
│  .ori test file                                                      │
│       │                                                              │
│       ▼                                                              │
│  ┌─────────────┐                                                     │
│  │   Parse     │                                                     │
│  └──────┬──────┘                                                     │
│         ▼                                                            │
│  ┌─────────────┐                                                     │
│  │ Type Check  │                                                     │
│  └──────┬──────┘                                                     │
│         ▼                                                            │
│  ┌─────────────┐                                                     │
│  │ModuleCompile│  ← Compile all functions + test wrapper             │
│  │  (LLVM IR)  │                                                     │
│  └──────┬──────┘                                                     │
│         ▼                                                            │
│  ┌─────────────┐                                                     │
│  │ObjectEmitter│  ← emit_object() to temp .o file                    │
│  └──────┬──────┘                                                     │
│         ▼                                                            │
│  ┌─────────────┐                                                     │
│  │LinkerDriver │  ← Link with ori_rt runtime library                 │
│  └──────┬──────┘                                                     │
│         ▼                                                            │
│  ┌─────────────┐                                                     │
│  │ Execute Bin │  ← Run binary, capture exit code + output           │
│  └──────┬──────┘                                                     │
│         ▼                                                            │
│     Pass/Fail                                                        │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### AotTestExecutor

New module in `compiler/ori_llvm/src/aot/`:

```rust
// compiler/ori_llvm/src/aot/test_executor.rs

pub struct AotTestExecutor {
    target: TargetConfig,
    runtime_path: Option<PathBuf>,
    temp_dir: PathBuf,
}

pub struct AotTestResult {
    pub passed: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

impl AotTestExecutor {
    /// Create executor for native target
    pub fn native() -> Result<Self, AotTestError>;

    /// Execute a single test through full AOT pipeline
    pub fn execute_test(
        &self,
        test_name: Name,
        test_body: ExprId,
        module: &Module,
        arena: &ExprArena,
        interner: &StringInterner,
        expr_types: &[TypeId],
        function_sigs: &[FunctionSig],
    ) -> Result<AotTestResult, AotTestError>;
}
```

### Test Wrapper Generation

Each test is wrapped in a `main` function:

```llvm
; Generated wrapper for @test_foo
define i32 @main() {
entry:
    call void @ori_rt_reset_panic()  ; Reset panic state
    call void @__test_test_foo()     ; Run the actual test
    %panic = call i1 @ori_rt_had_panic()
    br i1 %panic, label %fail, label %pass

pass:
    ret i32 0

fail:
    ret i32 1
}
```

Exit codes:
- `0` = test passed
- `1` = test failed (assertion/panic)
- Other = unexpected error

### Runtime Linking

The AOT executor must link against the Ori runtime library (`ori_rt`):

```rust
impl AotTestExecutor {
    fn link_test_binary(&self, object_path: &Path) -> Result<PathBuf, AotTestError> {
        let output_path = self.temp_dir.join("test_binary");

        let driver = LinkerDriver::new(&self.target);
        driver.link(&LinkInput {
            objects: vec![object_path.to_path_buf()],
            output: output_path.clone(),
            output_kind: LinkOutput::Executable,
            libraries: vec![
                LinkLibrary::new("ori_rt"),
                // Platform libraries (libc, etc.)
            ],
            library_paths: vec![
                self.runtime_path.clone().unwrap_or_default(),
            ],
            ..Default::default()
        })?;

        Ok(output_path)
    }
}
```

### Test Runner Integration

```rust
// compiler/oric/src/test/runner.rs

impl TestRunner {
    fn run_file(&self, path: &Path) -> FileSummary {
        // ... existing code ...

        match self.config.backend {
            Backend::Interpreter => { /* existing */ }
            Backend::LLVM => { /* existing JIT */ }
            Backend::AOT => {
                self.run_file_aot(&mut summary, &parse_result, &typed_module, interner);
            }
        }
    }

    #[cfg(feature = "llvm")]
    fn run_file_aot(
        &self,
        summary: &mut FileSummary,
        parse_result: &ParseOutput,
        typed_module: &TypedModule,
        interner: &StringInterner,
    ) {
        let executor = match AotTestExecutor::native() {
            Ok(e) => e,
            Err(e) => {
                summary.add_error(format!("AOT executor init failed: {e}"));
                return;
            }
        };

        for test in &parse_result.module.tests {
            if test.is_compile_fail() {
                // compile_fail tests don't need AOT execution
                continue;
            }

            let result = executor.execute_test(
                test.name,
                test.body,
                &parse_result.module,
                &parse_result.arena,
                interner,
                &typed_module.expr_types,
                &function_sigs,
            );

            match result {
                Ok(aot_result) => {
                    if aot_result.passed {
                        summary.add_result(TestResult::passed(...));
                    } else {
                        summary.add_result(TestResult::failed(
                            ...,
                            aot_result.stderr,
                            ...
                        ));
                    }
                }
                Err(e) => {
                    summary.add_result(TestResult::failed(..., e.to_string(), ...));
                }
            }
        }
    }
}
```

### CLI Interface

```bash
# Run tests with AOT backend
ori test tests/spec/ --backend=aot

# Run specific test file
ori test tests/spec/types/int.ori --backend=aot

# With filter
ori test --backend=aot --filter=arithmetic
```

### Error Handling

```rust
pub enum AotTestError {
    /// Failed to create target configuration
    TargetConfig(TargetError),

    /// Failed to compile to LLVM IR
    Compilation(String),

    /// Failed to emit object file
    ObjectEmission(EmitError),

    /// Failed to link executable
    Linking(LinkerError),

    /// Linker not found on system
    LinkerNotFound { message: String },

    /// Runtime library not found
    RuntimeNotFound { searched: Vec<PathBuf> },

    /// Failed to execute test binary
    Execution { exit_code: Option<i32>, stderr: String },

    /// Test timed out
    Timeout { duration: Duration },
}
```

### Temporary File Management

Each test execution creates temporary files:
- `test_{name}.o` — Object file
- `test_{name}` — Linked binary

Files are cleaned up after execution:

```rust
impl AotTestExecutor {
    fn execute_test(&self, ...) -> Result<AotTestResult, AotTestError> {
        let temp_dir = tempfile::tempdir()?;
        let object_path = temp_dir.path().join("test.o");
        let binary_path = temp_dir.path().join("test");

        // ... compile, link, execute ...

        // temp_dir dropped here, cleaning up files
    }
}
```

### Performance Considerations

| Aspect | JIT Backend | AOT Backend |
|--------|------------|-------------|
| Per-test overhead | ~1-5ms | ~50-200ms |
| First test | Fast (no startup) | Slow (linker startup) |
| Subsequent tests | Fast | Moderate (reuse executor) |
| Total for 100 tests | ~500ms | ~10-20s |

Mitigations:
1. **Parallel execution** — Run multiple test compilations concurrently
2. **Batch linking** — Compile multiple tests into one binary where possible
3. **Incremental use** — Only use AOT for CI; use JIT for development

### Configuration

```toml
# ori.toml
[testing]
aot_timeout = "30s"           # Per-test timeout
aot_parallel = true           # Parallel test compilation
aot_runtime_path = "path/to"  # Override runtime location
```

## Implementation Plan

### Phase 1: Runtime API & AotTestExecutor Core
- [ ] Add `ori_rt_had_panic()` and `ori_rt_reset_panic()` to `ori_rt`
- [ ] Create `compiler/ori_llvm/src/aot/test_executor.rs`
- [ ] Implement `AotTestExecutor::native()`
- [ ] Implement test wrapper generation (main function)
- [ ] Implement `execute_test()` basic flow

### Phase 2: Object Emission Integration
- [ ] Wire up `ModuleCompiler` to generate complete test module
- [ ] Add entry point (`main`) to compiled module
- [ ] Emit object file via `ObjectEmitter`

### Phase 3: Linker Integration
- [ ] Find/configure runtime library path
- [ ] Link test binary via `LinkerDriver`
- [ ] Handle platform-specific linker requirements

### Phase 4: Binary Execution
- [ ] Execute compiled binary
- [ ] Capture stdout/stderr
- [ ] Interpret exit code as pass/fail
- [ ] Implement timeout handling

### Phase 5: Test Runner Integration
- [ ] Add `Backend::AOT` enum variant
- [ ] Add `--backend=aot` CLI flag
- [ ] Wire up `run_file_aot()` in test runner

### Phase 6: Error Handling & Polish
- [ ] Implement all `AotTestError` variants
- [ ] Add helpful error messages (linker not found, etc.)
- [ ] Temp file cleanup
- [ ] Progress reporting

### Phase 7: Testing the Tests
- [ ] Add integration tests for AOT executor itself
- [ ] Run spec tests through AOT to validate
- [ ] Compare results: Interpreter vs JIT vs AOT

## Testing the Implementation

Validate with:

1. **Unit tests** for `AotTestExecutor`
2. **Integration tests** comparing JIT vs AOT results
3. **Run full spec suite** through AOT backend:
   ```bash
   ./llvm-test.sh --backend=aot tests/spec/
   ```

Success criteria:
- All spec tests that pass with JIT also pass with AOT
- AOT catches bugs that JIT misses (if any exist)
- Clear error messages when linker/runtime missing

## Alternatives Considered

### 1. Only Test via JIT

Rejected: JIT skips critical production code paths (object emission, linking). Users run AOT-compiled binaries, so we should test what they use.

### 2. Compile All Tests into One Binary

Considered: Would be faster but:
- Test isolation is lost (one panic affects all)
- Harder to identify which test failed
- More complex implementation

May revisit as optimization later.

### 3. Use External Test Harness

Rejected: Would require maintaining separate infrastructure. Better to integrate into existing test runner with backend abstraction.

### 4. WASM AOT Only

Rejected: WASM is important but native AOT is what most users will use. Test both via target selection.

## Future Extensions

1. **Cross-compilation testing** — Run AOT tests for different targets via QEMU
2. **WASM AOT testing** — Compile to WASM, run via wasmtime/wasmer
3. **Batch compilation** — Multiple tests in one binary for speed
4. **Coverage integration** — Generate coverage data from AOT runs

## Summary

This proposal adds `Backend::AOT` to the test runner:

1. **Full pipeline coverage** — Tests object emission, linking, and binary execution
2. **Production parity** — Tests what users actually run
3. **Minimal disruption** — Integrates with existing backend abstraction
4. **Clear tradeoffs** — Slower than JIT, but more comprehensive

Combined with existing JIT testing, this creates a robust validation layer for the most critical part of the compiler.
