---
name: code-journey
description: Walk through the compiler as a piece of code, tracing both eval and LLVM paths to find issues
argument-hint: "[code-description | file.ori | --summary | --infinity]"
---

# Code Journey

**You are a piece of Ori code.** You journey through the compiler, experiencing every transformation from source text to final result. You take **two independent paths** — the eval (interpreter) path and the LLVM (AOT) path — and record exactly what happens to you at each stage. The goal: find issues — inefficiencies, boundary problems, performance issues, architecture gaps, memory concerns, behavioral mismatches.

**LLVM codegen receives the deepest scrutiny.** The generated IR must be as close to hand-written optimal LLVM IR as possible. Every redundant instruction, every missing attribute, every suboptimal ARC operation, every unnecessary allocation is a finding. The standard is: **could a human write better IR for this code?** If yes, that's a finding.

## CRITICAL: Autonomous Execution

**NEVER ask the user for feedback, confirmation, or input.** This skill runs fully autonomously — no `AskUserQuestion`, no pausing for approval, no "should I continue?" prompts. Just keep going. The user invoked `/code-journey` and expects it to run until the termination condition is met.

**NEVER output verbose trace data to the main conversation.** All trace output goes to temp files. The main context stays lean — only brief status lines between journeys.

## CRITICAL: Context Conservation

Each journey's analysis and results writing is delegated to a **background Task agent**. The main agent's job is:
1. Create the journey code
2. Run the commands (redirecting output to temp files)
3. Spawn a background agent to analyze and write results
4. Immediately move to the next journey

**The main agent NEVER writes journey results files or updates overview.md directly.** That's the background agent's job.

## Usage

```
/code-journey                     # Run ONE journey (auto-pick next feature area), then stop
/code-journey closures             # Run ONE journey focusing on closures, then stop
/code-journey path/to/file.ori     # Run ONE journey with existing code, then stop
/code-journey --summary            # Show overview.md (cumulative findings across all journeys)
/code-journey --infinity           # Loop through journeys continuously until termination condition
/code-journey closures --infinity  # Start with closures, then keep going
```

## Journey Directory

All journey files live in `plans/code-journeys/`:
- `journeyN.ori` — the source code for journey N
- `journeyN-results.md` — detailed results for journey N
- `overview.md` — living dashboard updated after EVERY journey (cumulative findings, coverage map, trends)

---

## Execution Loop

### Step 0: Determine Journey Number and Code

**Scan existing journeys:**

```bash
ls plans/code-journeys/journey*-results.md 2>/dev/null | sort -V
```

Count completed journeys to determine the next number N.

**Parse arguments:**

Check if `$ARGUMENTS` contains `--infinity`. If so, set **INFINITY_MODE = true** and strip `--infinity` from the arguments before further parsing. Otherwise, **INFINITY_MODE = false** (the default — run ONE journey and stop).

**Choose or create code** (using the remaining arguments after stripping `--infinity`):

- **If arguments contain a `.ori` file path**: Use that file as-is. Copy it to `plans/code-journeys/journeyN.ori`.
- **If arguments contain a description** (e.g., "closures", "pattern matching"): Create code targeting those features.
- **If arguments are `--summary`**: Display the contents of `plans/code-journeys/overview.md` and stop. Don't run a journey.
- **If no arguments** (or only `--infinity` was provided): Auto-pick by reading `plans/code-journeys/overview.md` (coverage map section) to find untested feature areas.

**Complexity Escalation — Organic, Not Hardcoded:**

Start simple. Each journey adds ONE new language feature category on top of what previous journeys covered. Read `overview.md`'s coverage map to see what's been tested, then pick the next untested feature area. Progression ideas (not a fixed list — adapt based on what you find):

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

**Code design principles:**
- Each journey should produce a deterministic `int` result via `@main () -> int`
- Include a comment with the expected result calculation (e.g., `// = (3+4)*5+1 = 36`)
- Keep code small (< 20 lines) — focused on specific features, not comprehensive
- When a feature area fails, try a DIFFERENT feature area before giving up

Write the code to `plans/code-journeys/journeyN.ori`.

### Step 1: Run Both Paths (Output to Temp Files)

Create a temp directory for this journey's trace data:

```bash
JTMP="/tmp/journey_N"
mkdir -p "$JTMP"
```

**Run both paths, capturing exit codes and output to files (NOT to context):**

```bash
# Eval path
cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/eval_stdout.txt" 2> "$JTMP/eval_stderr.txt"
echo $? > "$JTMP/eval_exit.txt"

# AOT path
rm -rf ~/.cache/ori 2>/dev/null
./target/debug/ori run --compile plans/code-journeys/journeyN.ori > "$JTMP/aot_stdout.txt" 2> "$JTMP/aot_stderr.txt"
echo $? > "$JTMP/aot_exit.txt"
```

**Read ONLY the exit codes and stdout results back into context:**

```bash
cat "$JTMP/eval_exit.txt" "$JTMP/aot_exit.txt" "$JTMP/eval_stdout.txt" "$JTMP/aot_stdout.txt"
```

This gives you just the 4 key values: eval exit code, AOT exit code, eval output, AOT output.

### Step 2: Run All Traces (Output to Temp Files)

**Run ALL trace commands, redirecting everything to temp files. Do NOT read the output into context.**

```bash
# Lexer trace
ORI_LOG=ori_lexer=debug cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/lexer.txt" 2>&1

# Parser trace
ORI_LOG=ori_parse=debug cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/parser.txt" 2>&1

# Type checker trace
ORI_LOG=ori_types=debug cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/typeck.txt" 2>&1

# Canonicalizer trace
ORI_LOG=ori_canon=debug cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/canon.txt" 2>&1

# Eval trace
ORI_LOG=ori_eval=trace cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/eval_trace.txt" 2>&1

# Prelude overhead
ORI_LOG=ori_lexer=debug,ori_parse=debug,ori_types=debug,ori_canon=debug cargo run -- run plans/code-journeys/journeyN.ori > "$JTMP/prelude.txt" 2>&1

# LLVM IR dump (unoptimized — this is what OUR codegen emits)
rm -rf ~/.cache/ori 2>/dev/null
ORI_DEBUG_LLVM=1 ./target/debug/ori run --compile plans/code-journeys/journeyN.ori > "$JTMP/llvm_ir.txt" 2>&1

# LLVM warnings
rm -rf ~/.cache/ori 2>/dev/null
ORI_LOG=warn ./target/debug/ori run --compile plans/code-journeys/journeyN.ori > "$JTMP/llvm_warn.txt" 2>&1

# ARC analysis trace (borrow inference, RC operations)
rm -rf ~/.cache/ori 2>/dev/null
ORI_LOG=ori_llvm=debug ./target/debug/ori run --compile plans/code-journeys/journeyN.ori > "$JTMP/arc_trace.txt" 2>&1

# AOT binary size (if compilation succeeds)
rm -rf ~/.cache/ori 2>/dev/null
./target/debug/ori build plans/code-journeys/journeyN.ori -o "$JTMP/binary" > "$JTMP/build_stdout.txt" 2> "$JTMP/build_stderr.txt" && {
  ls -la "$JTMP/binary" > "$JTMP/binary_size.txt" 2>&1
  # Symbol table — shows all emitted symbols and their sizes
  nm --print-size --size-sort "$JTMP/binary" > "$JTMP/symbols.txt" 2>&1
  # Section sizes — text, data, bss, rodata
  size "$JTMP/binary" > "$JTMP/sections.txt" 2>&1
  size -A "$JTMP/binary" >> "$JTMP/sections.txt" 2>&1
  # Disassembly of user functions (not runtime) for instruction count
  objdump -d "$JTMP/binary" | grep -A 100 '<_ori_' > "$JTMP/disasm.txt" 2>&1
}
```

Run as many of these in parallel as possible (they are independent).

### Step 3: Spawn Background Agent to Write Results

Use the **Task tool** with `run_in_background: true` to spawn a background agent. The agent receives:
- The journey number N
- The journey code (read from the `.ori` file)
- The temp directory path
- The expected result

**The background agent's job:**
1. Read ALL trace files from the temp directory
2. Read existing `overview.md` and previous journey results (for cross-referencing)
3. Perform the **LLVM Deep Scrutiny** analysis (see below)
4. Analyze and categorize all findings (CRITICAL/HIGH/MEDIUM/LOW, NEW/CONFIRMED/REGRESSED/FIXED)
5. Write `plans/code-journeys/journeyN-results.md` with full analysis
6. Update `plans/code-journeys/overview.md` incrementally
7. Clean up the temp directory

**Background agent prompt template** (fill in N, code, expected result, eval exit, aot exit, eval stdout, aot stdout):

```
You are analyzing the results of Code Journey N for the Ori compiler.

Journey code (from plans/code-journeys/journeyN.ori):
[paste the code]

Expected result: [X]
Eval exit code: [E], Eval stdout: [S]
AOT exit code: [A], AOT stdout: [S]

Trace data is in temp files at /tmp/journey_N/:
- lexer.txt, parser.txt, typeck.txt, canon.txt, eval_trace.txt, prelude.txt
- llvm_ir.txt, llvm_warn.txt, arc_trace.txt
- eval_stdout.txt, eval_stderr.txt, aot_stdout.txt, aot_stderr.txt
- binary_size.txt, symbols.txt, sections.txt, disasm.txt (if build succeeded)

Your tasks:
1. Read ALL trace files and analyze the journey through both compiler paths
2. Read plans/code-journeys/overview.md for cross-referencing with previous findings
3. Perform the LLVM Deep Scrutiny analysis (see instructions below)
4. Write plans/code-journeys/journeyN-results.md with full analysis (use the standard format)
5. Update plans/code-journeys/overview.md incrementally — add new journey row, merge findings, update statuses
6. Run: rm -rf /tmp/journey_N

[Include the full results format template, LLVM Deep Scrutiny instructions, and severity/status definitions from below]
```

**Do NOT wait for the background agent to finish.** Immediately proceed to Step 4.

### Step 4: Decide — Continue or Stop

**Print a status line:**

```
Journey N complete: eval=[exit_code] aot=[exit_code]
```

**If INFINITY_MODE = false (the default):** Stop here. The single journey is complete. The background agent will write the results. Print:

```
Journey N complete: eval=[exit_code] aot=[exit_code] — results pending in background
```

**If INFINITY_MODE = true:** Evaluate whether to continue looping:

- **STOP** if and only if: **BOTH the eval path AND the LLVM path completely fail** (both crash, both produce wrong results, or the code can't compile at all). A single path failing is NOT grounds for stopping.
- **If a feature area won't compile**: Try a DIFFERENT feature area. Only stop if you've tried 3+ different feature areas and ALL of them fail on BOTH paths.

**If continuing** (infinity mode, default behavior): Print ONE status line:

```
Journey N complete: eval=[exit_code] aot=[exit_code] — next: [feature area]
```

Then loop back to Step 0 for journey N+1. **Do not elaborate. Do not summarize findings. The background agent handles that.**

**If stopping** (infinity mode, both paths failed): Print a brief termination message and stop.

---

## LLVM Deep Scrutiny (for Background Agent)

**This is the most important analysis.** Every instruction in the generated IR must justify its existence. The standard: **hand-written optimal IR for this program.** Every deviation from that ideal is a finding.

### Scrutiny 1: Instruction-Level Purity

For EACH user function in the emitted IR (ignore runtime declarations):

1. **Count instructions** — total per function. Compare against the theoretical minimum for the computation. A function that adds two ints needs 1 instruction (`add`), not 3 (`alloca`, `store`, `load`, `add`, `ret`). Unnecessary `alloca`/`store`/`load` sequences indicate `mem2reg`-dependent codegen — we should emit SSA directly where possible.

2. **Redundant operations** — Look for:
   - `alloca` + `store` + `load` where a direct SSA value would suffice
   - Back-to-back `store` then `load` to the same address
   - `bitcast` chains that could be collapsed or eliminated
   - `getelementptr` chains that could be merged (multiple GEPs to same struct)
   - `phi` nodes that have identical incoming values from all predecessors (should be the value itself)
   - `select` of identical true/false values
   - Zero-extension or sign-extension of values that are already the target width

3. **Dead code** — Instructions whose results are never used. LLVM will clean these up, but emitting them wastes compile time and bloats pre-optimization IR.

### Scrutiny 2: ARC Purity

**Every `ori_rc_inc`/`ori_rc_dec` call must be necessary.** Analyze:

1. **Balanced pairs** — An `rc_inc` immediately followed by `rc_dec` on the same value (or within the same basic block with no intervening use) is a wasted pair. Count these.

2. **Last-use optimization** — Is `rc_dec` called at the true last use of a value, or at scope end? Late decs keep memory alive longer than necessary.

3. **Borrow elision** — Parameters that are only read (never stored, never passed to functions that take ownership) should be borrowed, not owned. Check if borrow inference is catching all cases.

4. **Move semantics** — When a value is passed to a function and never used after, there should be no `rc_inc` (move, not copy). Check for unnecessary inc before a call followed by dec after.

5. **Drop function quality** — Are drop functions minimally complex? A struct with no RC'd fields should have a trivial drop (just free), not a field-walking drop.

6. **ARC on scalars** — Scalars (int, float, bool, char) should NEVER have RC operations. If you see `rc_inc`/`rc_dec` on an `i64` or `double`, that's CRITICAL.

7. **Count total RC ops per function** — report as `RC ops: N inc + M dec = K total`. Compare against the minimum necessary (one inc per shared reference creation, one dec per scope exit of owned value).

### Scrutiny 3: Attribute Completeness

For EACH function and parameter, check if these attributes are present where applicable:

| Attribute | Should be on | Missing = severity |
|-----------|-------------|-------------------|
| `nounwind` | All functions that cannot throw/unwind | HIGH — prevents LLVM from generating EH tables |
| `noalias` | `sret` params, allocation returns | HIGH — blocks alias analysis optimizations |
| `nonnull` | Pointers known to be non-null (allocated, non-optional) | MEDIUM — missed null-check elimination |
| `dereferenceable(N)` | Pointers to known-size allocations | MEDIUM — enables speculative loads |
| `nocapture` | Params not stored or returned as pointers | MEDIUM — enables stack promotion of caller allocs |
| `readonly` / `readnone` | Pure functions with no side effects | HIGH — blocks CSE and LICM across calls |
| `willreturn` | Functions guaranteed to terminate | MEDIUM — enables dead code elimination after call |
| `mustprogress` | Functions that don't contain infinite loops | MEDIUM — blocks loop optimization |
| `memory(...)` | Functions with restricted memory access | HIGH — blocks alias analysis without it |
| `noundef` | Parameters that cannot be `undef`/`poison` | LOW — enables poison propagation |
| `align N` | Pointer parameters with known alignment | LOW — enables aligned loads/stores |
| `cold` | Error paths, panic handlers, unlikely branches | MEDIUM — displaces cold code from hot path icache |
| `noinline` | Cold functions (pair with `cold`) | LOW — prevents bloating hot callers |

### Scrutiny 4: Calling Convention & ABI Efficiency

1. **fastcc usage** — ALL internal Ori functions must use `fastcc`. Any internal function on `ccc` is a missed optimization (no tail calls, suboptimal register use).

2. **Tail calls** — Recursive functions with `fastcc` should use `musttail` when the recursive call is in tail position. Report missed tail call opportunities.

3. **Small struct passing** — Structs ≤16 bytes should be passed `Direct` (in registers), not `Indirect` (by pointer). Check the ABI decision for every struct parameter.

4. **Return value optimization** — Small structs should be returned directly, not via sret. Only structs >16 bytes need sret.

5. **Unnecessary copies at call boundaries** — Is a value memcpy'd to pass it to a function, then immediately freed? That's a missed move opportunity.

### Scrutiny 5: Constant Folding & Compile-Time Evaluation

1. **Constant expressions** — Any expression composed entirely of literals should be folded at compile time, not emitted as runtime instructions. `3 + 4` should be `7` in the IR, not `add i64 3, 4`.

2. **Constant propagation** — If a `let` binding is assigned a constant and never reassigned, all uses should have the constant inlined.

3. **Dead branches** — `if true { A } else { B }` should emit only `A`. Check for conditional branches on known constants.

4. **Loop-invariant code** — Expressions inside loops that don't depend on the loop variable should be hoisted. (Note: LLVM's LICM handles this, but emitting it pre-hoisted is faster to compile.)

### Scrutiny 6: Memory Layout & Allocation

1. **Stack vs heap** — Values that don't escape the function should be stack-allocated, never heap-allocated. Check for `ori_rc_alloc` calls on values that are provably local.

2. **Struct padding** — Report the LLVM struct layout. Are fields ordered to minimize padding? (e.g., `{i64, i8, i64}` wastes 7 bytes; `{i64, i64, i8}` wastes 0 with trailing padding only.)

3. **Alloca sizing** — Are stack allocations correctly sized? Over-sized allocas waste stack space.

4. **Alignment** — Are struct accesses using correct alignment annotations? Misaligned accesses are slower on x86 and may crash on ARM.

### Scrutiny 7: Control Flow Quality

1. **Empty basic blocks** — Blocks that contain only a `br` to another block should be eliminated (block merging).

2. **Redundant branches** — `br i1 %cond, label %A, label %A` (both targets the same).

3. **Switch optimization** — For pattern matching, check if the emitted `switch` has optimal case density. Sparse switches should use jump tables or binary search, not linear scan.

4. **Phi node quality** — Phi nodes with a single incoming value are just that value. Phi nodes where all incoming values are the same constant should be the constant.

### Scrutiny 8: Optimal IR Comparison

For each user function, write what the **ideal LLVM IR** would look like — the minimal, maximal-attribute, zero-waste IR. Then compare it line-by-line against the actual emitted IR. Report:

- **Instruction overhead ratio**: `actual_instructions / ideal_instructions` (1.0 = perfect, 2.0 = 100% overhead)
- **Missing attributes**: list each missing attribute that the ideal IR would have
- **Unnecessary operations**: list each instruction in actual that doesn't appear in ideal
- **Verdict**: one of `OPTIMAL`, `NEAR-OPTIMAL` (ratio ≤ 1.2), `ACCEPTABLE` (ratio ≤ 1.5), `BLOATED` (ratio ≤ 2.0), `WASTEFUL` (ratio > 2.0)

### Scrutiny 9: Binary Quality (if build succeeded)

1. **Binary size** — Report total size. For a trivial program (just returns an int), the binary should be small. Report `.text`, `.data`, `.bss`, `.rodata` section sizes separately.

2. **Symbol count** — How many symbols are in the binary? Are there symbols that shouldn't be there (e.g., debug symbols in release, unused runtime functions)?

3. **Instruction count** — From the disassembly, count native instructions per user function. Compare against theoretical minimum for the architecture (x86_64).

4. **Runtime overhead** — What percentage of the binary is Ori runtime vs user code? A "hello world" that's 99% runtime has poor dead-code elimination.

---

## Results Format (for Background Agent)

The background agent writes `journeyN-results.md` using this format:

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
[SCC decomposition, borrow inference results — which params are borrowed vs owned]
[RC operation count: N inc + M dec = K total | minimum necessary = J | overhead = K-J]

#### Code Generation
[Declare/define details, monomorphized functions]

#### Generated LLVM IR (User Functions Only)
\```llvm
[ONLY the user functions — strip all runtime declarations, prelude, etc.]
\```

#### LLVM Deep Scrutiny Report

##### Instruction Purity
| Function | Actual Instrs | Ideal Instrs | Overhead Ratio | Verdict |
|----------|--------------|-------------|----------------|---------|
| ... | ... | ... | ... | ... |

[For each function: list every unnecessary instruction and why it's unnecessary]

##### ARC Purity
- **Total RC operations**: N inc + M dec = K total
- **Minimum necessary**: J (explain why)
- **Wasted pairs**: [list any inc/dec pairs that cancel out]
- **Borrow elision misses**: [params that should be borrowed but aren't]
- **Scalar RC violations**: [any RC ops on scalars — CRITICAL]

##### Attribute Audit
| Function | Missing Attributes | Severity |
|----------|-------------------|----------|
| ... | ... | ... |

##### Optimal IR Comparison
For EACH user function:
\```llvm
; === IDEAL IR ===
[what the IR should look like]

; === ACTUAL IR ===
[what was actually emitted]

; === DELTA ===
; + [lines in actual not in ideal — wasteful]
; - [lines in ideal not in actual — missing]
\```

##### Constant Folding Report
[List any constant expressions that were NOT folded at compile time]

##### Binary Analysis (if available)
\```
Total: X bytes | .text: Y | .data: Z | .rodata: W
User functions: N bytes (M instructions) | Runtime: K bytes
Symbols: total T | user S | runtime R
\```

---

## Issues Found

### CRITICAL
[Numbered findings with description, root cause, impact, and recommended fix]

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

## Codegen Quality Score

| Metric | Score | Notes |
|--------|-------|-------|
| Instruction purity | X.Xx | avg overhead ratio across functions |
| ARC purity | X/Y unnecessary ops | wasted RC operations |
| Attribute completeness | N/M present | missing attributes count |
| Constant folding | N missed | compile-time evaluable expressions emitted as runtime |
| Overall verdict | [OPTIMAL / NEAR-OPTIMAL / ACCEPTABLE / BLOATED / WASTEFUL] | |
```

## Severity Definitions (for Background Agent)

| Severity | Criteria |
|----------|----------|
| **CRITICAL** | Wrong output, crashes, data corruption, behavioral mismatch between eval and LLVM, RC ops on scalars, silent miscompilation |
| **HIGH** | Missing `nounwind`/`noalias`/`readonly`/`memory(...)` on applicable functions (blocks major LLVM optimizations), significant unnecessary RC operations (>2 wasted pairs per function), instruction overhead ratio > 2.0 (WASTEFUL), missing `fastcc` on internal functions, missed tail calls |
| **MEDIUM** | Missing `nonnull`/`nocapture`/`dereferenceable`/`willreturn`/`cold` attributes, instruction overhead ratio > 1.5 (BLOATED), unfolded constant expressions, unnecessary alloca/store/load chains, redundant GEP chains, suboptimal struct field ordering |
| **LOW** | Missing `noundef`/`align` attributes, instruction overhead ratio > 1.2 (ACCEPTABLE), empty basic blocks, minor control flow redundancy, pre-optimization dead code, documentation gaps |

**Cross-reference status:**
- **NEW** — first seen in this journey
- **CONFIRMED** — previously seen, still present
- **REGRESSED** — previously working, now broken
- **FIXED** — previously broken, now working

## Overview.md Structure (for Background Agent)

The overview.md must contain:

1. **Journey Results Table** — one row per journey: number, theme, features tested, eval result, AOT result, codegen verdict, key finding
2. **Codegen Quality Trend** — table showing instruction purity, ARC purity, attribute completeness scores across all journeys
3. **Deduplicated Findings by Severity** — CRITICAL/HIGH/MEDIUM/LOW, with finding number, description, first-seen journey, current status
4. **Findings by Compiler Phase** — grouped by lexer/parser/typechecker/canon/eval/LLVM codegen
5. **LLVM Codegen Findings** (separate section) — all codegen-specific findings: wasteful instructions, missing attributes, ARC inefficiencies, constant folding misses
6. **What Works Well** — features confirmed working on both paths, plus codegen quality highlights
7. **Coverage Map** — features tested and working vs. features not yet tested
8. **Recommended Fix Priority** — ordered by impact and frequency, with codegen improvements weighted heavily
9. **Trend Analysis** — which issues persist, which are specific to certain feature sets, codegen quality trajectory

**Incrementally update** — do NOT rewrite from scratch. Read existing, merge new findings, update statuses.

---

## Important Rules

1. **NEVER ask the user anything.** Fully autonomous. No AskUserQuestion, no pauses, no confirmations.

2. **NEVER read trace files into main context.** Trace data goes to temp files → background agent reads them. The main agent only sees exit codes and stdout.

3. **Record EXACT data.** (Background agent rule) No approximations — run commands and capture actual output.

4. **Two independent paths.** Eval and LLVM are separate — analyze each independently, compare at the end.

5. **Silent errors are the worst findings.** If the compiler silently produces wrong code instead of reporting an error, that's always CRITICAL.

6. **Clear AOT cache before LLVM traces.** `rm -rf ~/.cache/ori` — otherwise you'll get cached results with no logs.

7. **Cross-reference findings.** (Background agent rule) Check previous journey results before writing. Mark findings as NEW, CONFIRMED, REGRESSED, or FIXED.

8. **Main context stays lean.** Between journeys, the main context should contain only: the journey code (~20 lines), exit codes (2 numbers), stdout (2 lines), and a 1-line status. Everything else is in temp files or handled by background agents.

9. **LLVM scrutiny is non-negotiable.** The background agent MUST perform all 9 scrutiny categories. Skipping any is unacceptable. The Optimal IR Comparison (Scrutiny 8) is the most important — it makes waste visible by showing what perfection looks like next to reality.

10. **Every instruction must justify its existence.** If an instruction in the emitted IR cannot be justified as necessary for correctness, it is a finding. "LLVM will optimize it away" is NOT a justification — we should not emit it in the first place. Clean input IR → faster compile times, better debug builds, and fewer optimizer-phase surprises.

---

## Tracing Quick Reference (for Background Agent)

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
| `ORI_LOG=ori_llvm=debug` | ARC pipeline, borrow inference, codegen details |
| `ORI_LOG=warn` | All warnings (type checker + LLVM codegen) |

## Binary Analysis Quick Reference (for Background Agent)

| Command | What It Shows |
|---------|--------------|
| `nm --print-size --size-sort binary` | All symbols sorted by size — find bloat |
| `size binary` | Section sizes (text/data/bss) — code vs data split |
| `size -A binary` | Per-section breakdown — rodata, eh_frame, etc. |
| `objdump -d binary \| grep -A 100 '<_ori_'` | Disassembly of user functions — native instruction count |
| `readelf -S binary` | Section headers with flags — identify unexpected sections |
| `strings binary \| wc -l` | String count — detect string table bloat |

## Ori Codegen Architecture Reference (for Background Agent)

Key files in `compiler/ori_llvm/src/` to understand emitted IR:

| File | What It Controls |
|------|-----------------|
| `codegen/function_compiler.rs` | Function declaration (attributes, calling convention, ABI) |
| `codegen/arc_emitter/mod.rs` | ARC IR → LLVM IR emission (RC operations) |
| `codegen/arc_emitter/rc_ops.rs` | RC strategy dispatch (HeapPointer, FatPointer, etc.) |
| `codegen/arc_emitter/drop_gen.rs` | Drop function generation |
| `codegen/ir_builder/calls.rs` | Call emission, attribute application |
| `codegen/ir_builder/memory.rs` | Load/store/alloca/GEP emission |
| `codegen/type_info/info.rs` | Ori type → LLVM type mapping |
| `abi/mod.rs` | ABI computation (Direct/Indirect/Sret thresholds) |
| `aot/passes.rs` | LLVM optimization pass pipeline |
| `codegen/runtime_decl/mod.rs` | Runtime function declarations (attribute source of truth) |

When reporting findings, reference the responsible source file so issues are immediately actionable.
