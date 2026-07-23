# Diagnostic Fixtures — SSOT

This file is the **single source of truth** for fixture categorization, coverage, and self-test contracts.
`self-test.sh` references this file for the canonical fixture list.

## Categories

- **pass** — Exit 0, balanced RC. Basic code patterns exercising core compiler paths.
- **aims-heavy** — Exit 0, exercises AIMS-specific paths (COW, reuse, `?` unwinding, recursion, monomorphization). Feature-specific IR assertions required.
- **expected-fail** — Exit non-zero. Validates that diagnostic scripts correctly detect failures (leaks, mismatches, build errors).
- **infra** — Supporting infrastructure (wrappers, scripts). Not run as standalone fixtures.
- **seam-only** — Compiled for a build-time diagnostic report only; the emitted binary is never executed, so no exit-code or leak contract applies. Excluded from `PASS_FIXTURES` / `AIMS_HEAVY_FIXTURES` / `EXPECTED_FAIL_FIXTURES`.

## Fixture Matrix

| Fixture | Category | Pattern | Key ARC/AIMS Paths | Expected Exit | bisect-passes? |
|---------|----------|---------|-------------------|---------------|----------------|
| `simple.ori` | pass | No collections, no RC | Baseline (no RC ops) | 0 | Yes |
| `clean.ori` | pass | Collections + balanced RC | RC alloc/dec, list ops | 0 | Yes |
| `chain.ori` | pass | Chained COW ops | COW clone path, sequential mutation | 0 | Yes |
| `closure.ori` | pass | Closure capture + call | PartialApply, closure env RC | 0 | Yes |
| `closure_escape.ori` | pass | Escaping closures | Closure lifetime beyond scope | 0 | Yes |
| `iterator_break.ori` | pass | Iterator early exit | Iterator drop, elem cleanup | 0 | Yes |
| `iterator_complex.ori` | pass | Nested/yield/guard iteration | Nested loop RC, partial collect | 0 | Yes |
| `nested_list.ori` | pass | Nested collections | elem_dec_fn propagation | 0 | Yes |
| `trait_dispatch.ori` | pass | Trait method dispatch | Trait vtable codegen, method RC | 0 | Yes |
| `pattern_match.ori` | pass | Sum type mixed variants | Decision tree, per-variant drop | 0 | Yes |
| `map_iteration.ori` | pass | Map create + iterate | Map RC, iterator cleanup | 0 | Yes |
| `question_mark.ori` | aims-heavy | `?` with fat values | Early-exit unwinding, drop all live | 0 | Yes |
| `recursive_tree.ori` | aims-heavy | Recursive fat pointer passing | Stack-frame RC across depth | 0 | Yes |
| `generic_mono.ori` | aims-heavy | Multi-type generic instantiation | Monomorphization RC correctness | 0 | Yes |
| `large_aggregate.ori` | aims-heavy | >16B struct pass/return | ABI compliance, large aggregate load | 0 | Yes |
| `cow_sharing.ori` | aims-heavy | COW sharing/fork | is_unique, COW clone barrier | 0 | Yes |
| `leak.ori` | expected-fail | Panic with fat values | Leak detection path (best-effort) | non-zero | Yes (expect exit 1) |
| `mismatch.ori` | expected-fail | Interpreter vs AOT mismatch | Mismatch detection path | non-zero | No |
| `build-fail-parse.ori` | expected-fail | Parse error (syntax) | Build failure detection | non-zero | No |
| `mismatch-wrapper.sh` | infra | ORI_BIN wrapper for mismatch | Injects deterministic divergence | N/A | N/A |
| `entry_args_read.ori` | seam-only | `@main` argv borrow-read | Entry-point ownership seam, CONSISTENT arm | N/A (not executed) | No |
| `entry_args_consumed.ori` | seam-only | `@main` argv iter-consumed | Entry-point ownership seam, DIVERGENT arm | N/A (not executed) | No |

## Self-Test Contract by Category

### pass
- `ir-dump.sh` produces non-empty LLVM IR
- `arc-dump.sh` produces non-empty ARC IR
- `diagnose-aot.sh` exits 0 (all checks pass)
- `dual-exec-debug.sh` shows MATCH (interpreter == AOT)
- `rc-stats.sh` produces output containing "Function"
- `bisect-passes.sh --rc-only` produces phase table containing "Phase"; output contains "Leak check: clean"

### aims-heavy
Same as **pass**, PLUS:
- `bisect-passes.sh --rc-only` shows non-zero RC operations (not `inc:0 dec:0`)
- Feature-specific IR marker assertions (see §06.5 in plan)

### expected-fail
- `leak.ori`: `diagnose-aot.sh` exits non-zero; output contains "imbalance" or "FAIL"
- `leak.ori`: `bisect-passes.sh --rc-only` output contains "exited with code 1" (panic bypasses runtime leak checker, so "Leak check: clean" is still present — the runtime failure indicator is the key assertion)
- `mismatch.ori` (via wrapper): `dual-exec-debug.sh` exits non-zero; output contains "MISMATCH"
- `build-fail-parse.ori`: build step fails (exit non-zero)

### seam-only
- `entry-ownership.sh` renders a seam block containing `param#0 name=args`
- `entry_args_read.ori`: report contains `seam: CONSISTENT` and `callee_owner_demand = Borrow`
- `entry_args_consumed.ori`: report contains `seam: DIVERGENT` and `callee_owner_demand = WholeValue`
- `entry-ownership.sh --compare` over the pair reports differing semantic fields while `wrapper_owns_on_normal` is absent from the diff
