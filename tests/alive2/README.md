# Alive2 Translation Validation Corpus

Curated Ori functions verified by [Alive2](https://github.com/AliveToolkit/alive2) `alive-tv` to prove that LLVM optimization passes preserve program semantics.

## How It Works

1. Ori compiles a `.ori` file with `ORI_ALIVE2_CAPTURE=1`, capturing pre-optimization and post-optimization LLVM IR
2. `alive-tv` uses Z3 (SMT solver) to mathematically prove that the post-opt IR is a valid *refinement* of the pre-opt IR
3. Unlike testing (which checks specific inputs), Alive2 proves correctness for **all possible inputs**

## Corpus Selection Criteria

**Include:**
- Pure arithmetic functions (int, float operations)
- Simple control flow (if/else, small-bound recursion)
- No runtime calls (`ori_rc_inc`, `ori_rc_dec`, `ori_alloc`, `ori_panic`)
- No exception handling (`invoke`/`landingpad`)
- No indirect calls (closures)
- No COW checks (`ori_rc_is_unique`)

**Exclude (produce false positives):**
- RC operations (Alive2 can't model custom allocators) — incl. `ori_rc_alloc`/`ori_rc_free`/`ori_*alloc*`/`ori_*free*`/`ori_*drop*`/`ori_*rc_inc`/`ori_*rc_dec`/`ori_*buffer*`/`ori_*elem_dec`/`ori_panic`
- Exception handling (not modeled by Alive2)
- COW uniqueness checks (`ori_rc_is_unique`)
- Large loop nests (>256 iterations, Z3 timeout)
- Variadic functions
- Checked-overflow intrinsics that branch to panic blocks

## Survival Requirement

Each corpus entry must survive `-O2` optimization — i.e., the function must appear in both pre-opt and post-opt IR. Pure functions are often fully inlined at `-O2`, causing alive-tv to silently skip them. Verify with:

```bash
diagnostics/alive2-verify.sh --check-survival tests/alive2/pure_arithmetic.ori
```

If a function is inlined, replace it with one that survives. Do **not** add `noinline` attributes.

## Adding New Entries

1. Write a pure `.ori` function matching the selection criteria
2. Build: `ORI_ALIVE2_CAPTURE=1 ori build <file> --opt=2`
3. Verify survival: check that the function appears in both `.preopt.ll` and `.postopt.ll`
4. Run alive-tv: `diagnostics/alive2-verify.sh <file> --function <name>`
5. Add to `curated-corpus.txt`: `<file_path> <function_name>`

## Running

```bash
# Single file
diagnostics/alive2-verify.sh tests/alive2/pure_arithmetic.ori

# Curated corpus
diagnostics/alive2-verify.sh --corpus

# All codegen tests (weekly CI sweep)
diagnostics/alive2-verify.sh --all-codegen

# With JSON output (for CI artifacts)
diagnostics/alive2-verify.sh --corpus --json
```

## Machine-Readable Output Contract (v1)

When `--json` is passed, `alive2-verify.sh` writes structured results to `build/alive2-results/results.json` conforming to `tests/alive2/results-schema.json` (JSON Schema draft-07).

### Directory Layout

```
build/alive2-results/
  results.json           # Structured verification results
  *.preopt.ll            # Pre-optimization LLVM IR (per-file)
  *.postopt.ll           # Post-optimization LLVM IR (per-file)
```

### Schema Version

- **Version 1** (current): flat function array with status enum
- Schema changes bump the `version` integer — consumers check version before parsing

### Consumers

- **CI integration**: uploads `build/alive2-results/` as a CI artifact, compares nightly/weekly runs
- **Regression dashboard**: tracks verification trends across runs, detects new failures
