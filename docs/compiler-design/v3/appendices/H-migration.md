# H: Migration Strategy

This document specifies the migration path from V1 to V2 compiler.

---

## Migration Phases

### Phase 1: Parallel Development

```
Timeline: Weeks 1-16

V1 compiler remains default
V2 compiler developed in separate crate (sigilc-v2)
Both compilers run against same test suite
```

**Directory structure:**
```
compiler/
├── sigilc/           # V1 compiler (current)
└── sigilc-v2/        # V2 compiler (new)
```

### Phase 2: Feature Parity

```
Timeline: Weeks 17-20

V2 achieves feature parity with V1
All V1 tests pass on V2
V2 available as opt-in: `sigil2 run`
```

### Phase 3: V2 Default

```
Timeline: Weeks 21-24

V2 becomes default compiler
V1 available via flag: `sigil --v1 run`
Deprecation warnings for V1
```

### Phase 4: V1 Removal

```
Timeline: After 3 months of V2 default

V1 code removed from repository
Only V2 remains
```

---

## Compatibility Testing

### Test Compatibility Framework

```rust
/// Compare V1 and V2 output for compatibility
pub fn verify_compatibility(test_file: &Path) -> CompatibilityResult {
    // Run V1
    let v1_result = v1::run_file(test_file);

    // Run V2
    let v2_result = v2::run_file(test_file);

    // Compare outputs
    match (v1_result, v2_result) {
        (Ok(v1_out), Ok(v2_out)) => {
            if v1_out == v2_out {
                CompatibilityResult::Match
            } else {
                CompatibilityResult::OutputMismatch {
                    v1: v1_out,
                    v2: v2_out,
                }
            }
        }
        (Err(v1_err), Err(v2_err)) => {
            // Both fail - check error is similar
            if errors_equivalent(&v1_err, &v2_err) {
                CompatibilityResult::Match
            } else {
                CompatibilityResult::ErrorMismatch {
                    v1: v1_err,
                    v2: v2_err,
                }
            }
        }
        (Ok(v1), Err(v2)) => {
            CompatibilityResult::V2Regression {
                v1_success: v1,
                v2_error: v2,
            }
        }
        (Err(v1), Ok(v2)) => {
            CompatibilityResult::V2Improvement {
                v1_error: v1,
                v2_success: v2,
            }
        }
    }
}

pub enum CompatibilityResult {
    Match,
    OutputMismatch { v1: String, v2: String },
    ErrorMismatch { v1: Error, v2: Error },
    V2Regression { v1_success: String, v2_error: Error },
    V2Improvement { v1_error: Error, v2_success: String },
}
```

### Compatibility CI

```yaml
# .github/workflows/compatibility.yml
name: V1/V2 Compatibility

on:
  push:
    paths:
      - 'compiler/sigilc-v2/**'
      - 'tests/**'

jobs:
  compatibility:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Build both compilers
        run: |
          cargo build --release -p sigilc
          cargo build --release -p sigilc-v2

      - name: Run compatibility tests
        run: |
          cargo run --release -p test-runner -- \
            --v1 target/release/sigilc \
            --v2 target/release/sigilc-v2 \
            tests/

      - name: Upload compatibility report
        uses: actions/upload-artifact@v3
        with:
          name: compatibility-report
          path: compatibility-report.json
```

### Full Compatibility Check

```rust
/// Run compatibility check on entire test suite
pub fn full_compatibility_check() -> CompatibilityReport {
    let test_files: Vec<_> = glob("tests/**/*.si")
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    let results: Vec<_> = test_files
        .par_iter()
        .map(|file| {
            let result = verify_compatibility(file);
            (file.clone(), result)
        })
        .collect();

    let mut report = CompatibilityReport::new();

    for (file, result) in results {
        match result {
            CompatibilityResult::Match => report.passed.push(file),
            CompatibilityResult::OutputMismatch { .. } => report.output_mismatch.push(file),
            CompatibilityResult::ErrorMismatch { .. } => report.error_mismatch.push(file),
            CompatibilityResult::V2Regression { .. } => report.regressions.push(file),
            CompatibilityResult::V2Improvement { .. } => report.improvements.push(file),
        }
    }

    report
}
```

---

## Feature Flags

### Runtime Feature Flags

```rust
/// Compiler feature flags for gradual rollout
pub struct CompilerConfig {
    // Phase 1 features
    pub use_interning: bool,
    pub use_flat_ast: bool,
    pub use_salsa: bool,

    // Phase 2 features
    pub use_type_interning: bool,
    pub use_bidirectional_inference: bool,

    // Phase 3 features
    pub use_pattern_templates: bool,
    pub use_pattern_fusion: bool,

    // Phase 4 features
    pub use_parallel_parse: bool,
    pub use_parallel_check: bool,
    pub use_parallel_codegen: bool,

    // Phase 5 features
    pub use_test_gating: bool,
    pub use_lazy_parsing: bool,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            // Enable features as they're implemented
            use_interning: true,
            use_flat_ast: true,
            use_salsa: true,

            use_type_interning: true,
            use_bidirectional_inference: true,

            use_pattern_templates: false,  // Not yet implemented
            use_pattern_fusion: false,

            use_parallel_parse: false,
            use_parallel_check: false,
            use_parallel_codegen: false,

            use_test_gating: false,
            use_lazy_parsing: false,
        }
    }
}

impl CompilerConfig {
    /// All features enabled (for testing)
    pub fn all_enabled() -> Self {
        Self {
            use_interning: true,
            use_flat_ast: true,
            use_salsa: true,
            use_type_interning: true,
            use_bidirectional_inference: true,
            use_pattern_templates: true,
            use_pattern_fusion: true,
            use_parallel_parse: true,
            use_parallel_check: true,
            use_parallel_codegen: true,
            use_test_gating: true,
            use_lazy_parsing: true,
        }
    }
}
```

### Environment Variable Control

```rust
impl CompilerConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if env::var("SIGIL_V2_PARALLEL").is_ok() {
            config.use_parallel_parse = true;
            config.use_parallel_check = true;
            config.use_parallel_codegen = true;
        }

        if env::var("SIGIL_V2_FUSION").is_ok() {
            config.use_pattern_fusion = true;
        }

        if env::var("SIGIL_V2_TEST_GATING").is_ok() {
            config.use_test_gating = true;
        }

        config
    }
}
```

---

## User Migration Guide

### For End Users

```markdown
## Migrating to Sigil Compiler V2

### Automatic (Recommended)

Sigil V2 is now the default compiler. Your existing code should
work without changes.

### If You Encounter Issues

1. Try V1 compatibility mode:
   ```bash
   sigil --v1 run myfile.si
   ```

2. Report the issue:
   ```bash
   sigil report --file myfile.si
   ```

### New Features in V2

- **10x faster compilation** - See the improvement with:
  ```bash
  sigil build --timings
  ```

- **Sub-100ms incremental** - Edit and re-run instantly

- **Better error messages** - More context and suggestions

- **LSP support** - Full IDE integration
```

### For Library Authors

```markdown
## Updating Libraries for V2

### Compatibility

V2 is fully backward compatible. Existing libraries work unchanged.

### Taking Advantage of V2

1. **Pattern fusion** - Chain map/filter/fold for automatic optimization
   ```sigil
   // V2 automatically fuses this into a single pass
   items
       |> map(.transform: process)
       |> filter(.predicate: valid)
       |> fold(.init: 0, .op: sum)
   ```

2. **Parallelism** - Multi-file libraries compile in parallel automatically

3. **Test-gated invalidation** - Comprehensive tests speed up dependent builds
```

---

## Rollback Plan

### Emergency Rollback

```bash
# If V2 has critical issues, users can rollback:
sigil config set compiler.version v1

# Or per-invocation:
sigil --v1 run myfile.si
```

### Project-Level Override

```toml
# sigil.toml
[compiler]
version = "v1"  # Force V1 for this project
```

### CI Integration

```yaml
# Test both compilers in CI during transition
jobs:
  test-v1:
    runs-on: ubuntu-latest
    steps:
      - run: sigil --v1 test

  test-v2:
    runs-on: ubuntu-latest
    steps:
      - run: sigil test  # V2 is default
```

---

## Success Criteria

### Phase 2 Exit Criteria

- [ ] 100% of V1 tests pass on V2
- [ ] No regressions in error messages
- [ ] Performance meets targets (see [G: Benchmarks](G-benchmarks.md))
- [ ] Memory usage within targets

### Phase 3 Exit Criteria

- [ ] V2 default for 2 weeks without critical issues
- [ ] <1% of users request V1 compatibility
- [ ] All known issues documented

### Phase 4 Exit Criteria

- [ ] V2 default for 3 months
- [ ] No outstanding V1 compatibility requests
- [ ] V1 code cleanly removable
