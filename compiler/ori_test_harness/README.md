# ori_test_harness

> **`ori_test_harness` exists to give compiler tests a shared, directive-driven infrastructure** — test infrastructure is first-class infrastructure.

Shared test infrastructure for the Ori compiler's verification tooling. Consumed as a dev-dependency by `ori_arc` (AIMS snapshot tests) and `ori_llvm` (FileCheck IR tests).

## Purpose

AIMS pass-level snapshots and FileCheck IR assertions both need directive parsing, revision expansion, bless mode, artifact diffing, and a test orchestration loop. This crate provides all of that as a single source of truth, preventing the two consumers from independently implementing (and inevitably drifting on) the same multi-step algorithm.

## Design Principle

**This crate knows nothing about the Ori compiler.** It parses text, diffs strings, resolves artifact paths, and orchestrates a test loop via the `TestStrategy` trait. Compiler-specific behavior — compilation, IR capture, flag translation — lives entirely in consumer crates' `TestStrategy` implementations.

## Modules

| Module | Purpose |
|--------|---------|
| `directive` | Line-anchored regex parser for `// @<key>: <value>` and `// CHECK:` directives |
| `artifact` | Generic path resolution for expected baselines and actual output |
| `bless` | Bless mode (`ORI_BLESS=1`) — update baselines instead of comparing |
| `diff` | Unified diff generation via the `similar` crate |
| `revision` | Revision expansion from `// @revisions:` directives |
| `runner` | `TestStrategy` trait and `run_test_directory()` canonical orchestration loop |

## Usage

Consumer crates implement `TestStrategy` and call `run_test_directory()`:

```rust
use ori_test_harness::runner::{run_test_directory, TestStrategy, TestOutput};
use ori_test_harness::directive::DirectiveLine;
use ori_test_harness::revision::RevisionConfig;

struct MyStrategy;

impl TestStrategy for MyStrategy {
    type Error = String;

    fn execute(
        &self,
        test_path: &Path,
        revision: &RevisionConfig,
        directives: &[&DirectiveLine],
    ) -> Result<TestOutput, String> {
        // Compile, capture output, return it
    }

    fn verify(
        &self,
        test_path: &Path,
        revision: &RevisionConfig,
        directives: &[&DirectiveLine],
        output: &TestOutput,
    ) -> Result<(), String> {
        // Compare output against expectations
    }
}

#[test]
fn my_tests() {
    use ori_test_harness::bless;
    let bless = bless::is_bless_enabled();
    let summary = run_test_directory(Path::new("tests/my-suite"), &MyStrategy, bless);
    assert!(summary.is_success());
}
```

## Directives

Test files use comment-based directives:

```ori
// @revisions: debug release
// @[release] compile-flags: --release
// @test-arc-pass: realize_rc_reuse
// CHECK: call void @ori_rc_inc
// CHECK-NOT: call void @ori_rc_dec
// CHECK-LABEL: define void @main
// CHECK-NEXT: ret void
```

- `// @revisions:` and `// @compile-flags:` are built-in (handled by the harness)
- `// CHECK:` variants are built-in (FileCheck-style assertions)
- All other `// @<key>: <value>` directives become `Directive::Custom` — interpreted by the consumer's `TestStrategy`

## Bless Mode

Set `ORI_BLESS=1` to update baselines instead of comparing:

```bash
ORI_BLESS=1 cargo test -p ori_arc -- aims_snapshot
```

Only the value `"1"` enables bless mode. `"0"`, `"false"`, `"true"` are all treated as disabled.

## Dependencies

- `similar` — diff generation
- `regex` — directive parsing (line-anchored, no Ori lexer dependency)
- `walkdir` — recursive test file discovery
- `tempfile` (dev) — test fixtures
