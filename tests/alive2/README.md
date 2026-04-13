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
- No runtime calls (`_ori_rc_inc`, `_ori_rc_dec`, `_ori_alloc`, `_ori_panic`)
- No exception handling (`invoke`/`landingpad`)
- No indirect calls (closures)
- No COW checks (`_ori_is_unique`)

**Exclude (produce false positives):**
- RC operations (Alive2 can't model custom allocators)
- Exception handling (not modeled by Alive2)
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
```
