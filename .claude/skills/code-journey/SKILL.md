---
name: code-journey
description: Walk through the compiler as a piece of code, tracing both eval and LLVM paths to find issues
argument-hint: "[code-description | file.ori | --summary]"
---

# Code Journey

**You are a piece of Ori code.** You journey through the compiler, experiencing every transformation from source text to final result. You take **two independent paths** — the eval (interpreter) path and the LLVM (AOT) path — and record exactly what happens to you at each stage. The goal: find issues — inefficiencies, boundary problems, performance issues, architecture gaps, memory concerns, behavioral mismatches.

## Usage

```
/code-journey                     # Auto-pick next journey (increasing complexity)
/code-journey closures             # Create code focusing on closures
/code-journey path/to/file.ori     # Journey with existing code
/code-journey --summary            # Show overview.md (cumulative findings across all journeys)
```

## Journey Directory

All journey files live in `plans/code-journeys/`:
- `journeyN.ori` — the source code for journey N
- `journeyN-results.md` — detailed results for journey N
- `overview.md` — living dashboard updated after EVERY journey (cumulative findings, coverage map, trends)

---

## Execution

### Step 0: Determine Journey Number and Code

**Scan existing journeys:**

```bash
ls plans/code-journeys/journey*-results.md 2>/dev/null | sort -V
```

Count completed journeys to determine the next number N.

**Choose or create code:**

- **If `$ARGUMENTS` is a `.ori` file path**: Use that file as-is. Copy it to `plans/code-journeys/journeyN.ori`.
- **If `$ARGUMENTS` is a description** (e.g., "closures", "pattern matching"): Create code targeting those features.
- **If `$ARGUMENTS` is `--summary`**: Display the contents of `plans/code-journeys/overview.md` and stop. Don't run a journey.
- **If no arguments**: Auto-pick by reading previous journey results and escalating complexity.

**Complexity Escalation — Organic, Not Hardcoded:**

Start simple. Each journey adds ONE new language feature category on top of what previous journeys covered. Read previous `journeyN-results.md` files to see what's been tested, then pick the next untested feature area. Progression ideas (not a fixed list — adapt based on what you find):

- Bare literals, arithmetic, `let` bindings
- Function calls, multiple functions
- Generics, type inference, generic structs
- Closures, lambdas, higher-order functions
- Iterators, `.map()`, `.filter()`, `.collect()`
- Pattern matching, `match`, destructuring
- Custom types, derived traits (`Eq`, `Printable`, etc.)
- Sum types (enums), variant constructors
- `Result`/`Option`, `?` propagation
- Collections: lists, maps, sets, nested structures
- ARC-heavy code: shared references, RC lifecycle
- Modules, `use` imports, cross-file references
- Strings, formatting, interpolation
- Loops, ranges, `for` expressions

**The key rule: each journey should exercise features NOT covered by previous journeys.**

**Termination Condition — Keep Going Forever:**

Journeys run **indefinitely**, escalating complexity after each one. The only reason to stop:

- **STOP** if and only if: **BOTH the eval path AND the LLVM path completely fail** for a journey (both crash, both produce wrong results, or the code can't compile at all). A single path failing is NOT grounds for stopping — record the failure, update `overview.md`, and keep going.

**If one path fails but the other succeeds**: Record as a CRITICAL finding, update `overview.md`, try a DIFFERENT feature area for the next journey. The goal is to exhaustively map what works and what doesn't across the entire language surface.

**If both paths succeed with issues**: Record findings, update `overview.md`, escalate complexity, keep going.

**If a feature area won't compile**: Try a DIFFERENT feature area. Only stop if you've tried 3+ different feature areas and ALL of them fail on BOTH paths — the compiler is too broken to continue.

When stopping (both paths fail), write a final `## Journey Terminated` section in the results explaining why, and update `overview.md` one final time.

**Code design principles:**
- Each journey should produce a deterministic `int` result via `@main () -> int`
- Include a comment with the expected result calculation (e.g., `// = (3+4)*5+1 = 36`)
- Keep code small (< 20 lines) — focused on specific features, not comprehensive
- Each journey should exercise features NOT covered by previous journeys
- When a feature area fails, try a DIFFERENT feature area before giving up — the goal is to map the boundary of what works, not just hit the first wall and stop

Write the code to `plans/code-journeys/journeyN.ori`.

### Step 1: Verify Code Runs

```bash
# Interpreter path (should always work)
cargo run -- run plans/code-journeys/journeyN.ori 2>&1
echo "Eval exit code: $?"

# AOT path (may fail for advanced features)
./target/debug/ori run --compile plans/code-journeys/journeyN.ori 2>&1
echo "AOT exit code: $?"
```

Record both exit codes. If they differ, that's already a **CRITICAL** finding (behavioral mismatch).

If the code doesn't compile at all, fix syntax issues. Check `.claude/rules/ori-syntax.md` for correct Ori syntax. Common mistakes:
- Missing `;` after expression bodies: `= a + b;`
- Missing `use std.testing { assert_eq }` for test functions
- No `return` keyword — last expression is the value

### Step 2: Trace the Eval Path

Run with tracing at each stage. **Record exact numbers, not approximations.**

#### Stage 1-2: Lexer
```bash
ORI_LOG=ori_lexer=debug cargo run -- run plans/code-journeys/journeyN.ori 2>&1 | grep "ori_lexer"
```
Record: source bytes, token count, errors, warnings, bytes/token ratio.

#### Stage 3: Parser
```bash
ORI_LOG=ori_parse=debug cargo run -- run plans/code-journeys/journeyN.ori 2>&1 | grep "parse_module"
```
Record: functions, tests, types, traits, impls, imports, expression count, errors.

#### Stage 4: Type Checker
```bash
ORI_LOG=ori_types=debug cargo run -- run plans/code-journeys/journeyN.ori 2>&1 | grep -E "registration|signature|body checking|mono|callee_var"
```
Record: registration counts, mono instances recorded, any warnings.

#### Stage 5: Canonicalizer
```bash
ORI_LOG=ori_canon=debug cargo run -- run plans/code-journeys/journeyN.ori 2>&1 | grep "canon"
```
Record: source exprs → canon nodes, roots, constants, decision trees.

#### Stage 6a: Interpreter Execution
```bash
ORI_LOG=ori_eval=trace cargo run -- run plans/code-journeys/journeyN.ori 2>&1 | grep -E "eval_can|evaluate_binary|evaluate_unary" | head -50
```
Record: total eval_can calls, binary/unary ops, function calls, struct constructions.

#### Prelude Overhead
```bash
ORI_LOG=ori_lexer=debug,ori_parse=debug,ori_types=debug,ori_canon=debug cargo run -- run plans/code-journeys/journeyN.ori 2>&1 | grep -E "(lexing|parse_module|registration|canon lower)" | head -20
```
Record: prelude lex/parse/typecheck/canon stats. Note if prelude is processed more than once (double processing finding from Journey 1).

### Step 3: Trace the LLVM Path

**Prerequisite**: Ensure LLVM-enabled binary is built: `cargo bl`

#### LLVM IR Dump
```bash
rm -rf ~/.cache/ori 2>/dev/null  # Clear AOT cache
ORI_DEBUG_LLVM=1 ./target/debug/ori run --compile plans/code-journeys/journeyN.ori 2>&1
```

#### LLVM Warnings/Errors
```bash
rm -rf ~/.cache/ori 2>/dev/null
ORI_LOG=warn ./target/debug/ori run --compile plans/code-journeys/journeyN.ori 2>&1
```

**Analyze the LLVM IR for:**
- Function signatures and calling conventions (fastcc vs C)
- `invoke` vs `call` usage (landing pads for non-panicking functions?)
- Return type correctness (struct types materialized correctly?)
- Monomorphized function names and parameter types
- Runtime declaration count (`grep -c "^declare"` after unescaping)
- Dead code (unreachable blocks, unused functions)
- ARC lifecycle (RC inc/dec placement)
- Type layout correctness (struct fields, tuple layout)

#### Runtime Declaration Count
```bash
rm -rf ~/.cache/ori 2>/dev/null
ORI_DEBUG_LLVM=1 ./target/debug/ori run --compile plans/code-journeys/journeyN.ori 2>&1 | sed 's/\\n/\n/g' | grep -c "^declare"
```

### Step 4: Analyze and Categorize Findings

For each finding, assign a severity:

| Severity | Criteria |
|----------|----------|
| **CRITICAL** | Wrong output, crashes, data corruption, behavioral mismatch between eval and LLVM |
| **HIGH** | Significant performance overhead, missing functionality, silent error swallowing |
| **MEDIUM** | Unnecessary work, missing fast paths, excessive allocations, missing tracing |
| **LOW** | Style issues, minor overhead, documentation gaps |

**Cross-reference with previous journeys.** Mark findings as:
- **NEW** — first seen in this journey
- **CONFIRMED** — previously seen, still present
- **REGRESSED** — previously working, now broken
- **FIXED** — previously broken, now working

### Step 5: Write Journey Results

Write results to `plans/code-journeys/journeyN-results.md` using this format:

```markdown
# Journey N: "I am [theme]"

**Code**:
\```ori
[the journey code]
\```
**Source**: X bytes, **Expected Result**: Y (= calculation)
**Actual**: Eval = Y (correct/WRONG), AOT = Z (correct/WRONG)

## Transformation Timeline

### Stage 1-2: Lexer
\```
X bytes → Y tokens (Z errors)
\```
[Observations about token ratio, any lexer issues]

### Stage 3: Parser
\```
Y tokens → N functions, M expressions (Z errors)
\```
[AST structure observations]

### Stage 4: Type Checker
\```
registration: N functions, M tests, K impls
signatures: ...
body checking: ...
\```
[Inference observations, mono instances, warnings]

### Stage 5: Canonicalizer
\```
canon lower_module started (functions=N, source_exprs=M)
canon lower_module complete (canon_nodes=K, roots=R, constants=C, decision_trees=D)
\```

### Stage 6a: Eval Path
\```
[Execution trace — simplified, showing key eval steps]
\```
[Total eval_can calls, function calls, binary ops]

### Stage 6b: LLVM Path

#### ARC/Borrow Analysis
[SCC decomposition, borrow inference details]

#### Code Generation
[Declare/define details, monomorphized functions]

#### Generated LLVM IR
\```llvm
[Key IR sections — not the full 98 declarations, just the user functions]
\```

#### Key Observations
[Numbered list of IR-level observations]

---

## Issues Found

### CRITICAL
[Numbered findings with description, root cause, and impact]

### HIGH
[...]

### MEDIUM
[...]

### LOW
[...]

### CONFIRMED FROM PREVIOUS JOURNEYS
[List previously-found issues that are still present]

---

## Eval vs LLVM Behavioral Mismatch
[Table comparing results if they differ]
```

### Step 6: Update overview.md (MANDATORY — after EVERY journey)

**This happens after every single journey, no exceptions.** Write/update `plans/code-journeys/overview.md` with the cumulative state across ALL journeys completed so far.

The overview.md must contain:

1. **Journey Results Table** — one row per journey: number, theme, features tested, eval result, AOT result, key finding
2. **Deduplicated Findings by Severity** — CRITICAL/HIGH/MEDIUM/LOW, with finding number, description, first-seen journey, current status (NEW/CONFIRMED/FIXED/REGRESSED)
3. **Findings by Compiler Phase** — grouped by lexer/parser/typechecker/canon/eval/LLVM codegen
4. **What Works Well** — features confirmed working on both paths
5. **Coverage Map** — features tested and working vs. features not yet tested
6. **Recommended Fix Priority** — ordered by impact and frequency
7. **Trend Analysis** — which issues persist, which are specific to certain feature sets

**When updating** (not first journey): Read the existing `overview.md`, merge in new findings, update statuses of existing findings (CONFIRMED if still present, FIXED if resolved), add the new journey row to the table. Do NOT rewrite from scratch each time — incrementally update.

**When `--summary` is used**: Just display the contents of `overview.md` (don't run a journey).

### Step 7: Decide — Continue or Stop

After updating `overview.md`, evaluate the termination condition (see Step 0).

- **If continuing** (the default — almost always): Tell the user what the next journey will target and why, then loop back to Step 0. **Do not stop. Keep going.**
- **If stopping** (BOTH paths failed completely): Write a `## Journey Terminated` section in the last results file explaining the termination reason, and add a termination note to `overview.md`.

---

## Important Rules

1. **Record EXACT data.** No approximations, no "about 20 tokens." Run the commands and capture the actual output.

2. **Two independent paths.** Eval and LLVM are separate journeys — analyze each independently, compare at the end.

3. **The tracing is permanent.** The tracing infrastructure in `ori_lexer`, `ori_parse`, `ori_canon`, and `ori_eval` was added as part of the journey system. Use it. If a crate has no tracing (like `ori_llvm/codegen/function_compiler`), note that as a finding.

4. **Silent errors are the worst findings.** If the compiler silently produces wrong code instead of reporting an error, that's always CRITICAL. A compile error is always better than wrong output.

5. **Check both exit codes.** The eval path (`cargo run -- run`) and AOT path (`ori run --compile`) should produce the same exit code. Mismatches are CRITICAL.

6. **Clear AOT cache before LLVM traces.** `rm -rf ~/.cache/ori` — otherwise you'll get cached results with no logs.

7. **Cross-reference findings.** Check previous journey results before writing. Mark findings as NEW, CONFIRMED, REGRESSED, or FIXED.

---

## Tracing Quick Reference

| Variable | What It Shows |
|----------|--------------|
| `ORI_LOG=ori_lexer=debug` | Token counts, byte/token ratio |
| `ORI_LOG=ori_parse=debug` | AST node counts, parse context entry |
| `ORI_LOG=ori_types=debug` | Type checker phases, mono instances |
| `ORI_LOG=ori_types=trace` | Per-expression inference (very verbose) |
| `ORI_LOG=ori_canon=debug` | Canon IR node counts, decision trees |
| `ORI_LOG=ori_eval=trace` | Per-expression evaluation |
| `ORI_LOG=ori_eval=debug` | Prelude registration |
| `ORI_DEBUG_LLVM=1` | Full LLVM IR dump to stderr |
| `ORI_LOG=warn` | All warnings (type checker + LLVM codegen) |

Combine: `ORI_LOG=ori_lexer=debug,ori_parse=debug,ori_types=debug,ori_canon=debug,ori_eval=debug`
