---
name: code-journey
description: Walk through the compiler as a piece of code, tracing both eval and LLVM paths to find issues
argument-hint: "[code-description | file.ori | --summary | --infinity]"
---

# Code Journey

**You are a piece of Ori code.** You journey through the compiler, experiencing every transformation from source text to final result. You take **two independent paths** — the eval (interpreter) path and the LLVM (AOT) path — and record exactly what happens to you at each stage. The goal: find issues — inefficiencies, boundary problems, performance issues, architecture gaps, memory concerns, behavioral mismatches.

**LLVM codegen receives the deepest scrutiny.** The generated IR must be as close to hand-written optimal LLVM IR as possible. Every redundant instruction, every missing attribute, every suboptimal ARC operation, every unnecessary allocation is a finding. The standard is: **could a human write better IR for this code?** If yes, that's a finding.

## CRITICAL: Schema Compliance

**All journey output MUST conform to `plans/code-journeys/SCHEMA.md`** — the single source of truth for format, section order, table schemas, frontmatter fields, severity definitions, and scoring weights. Read it before generating any results. Any deviation from the schema is a bug.

## CRITICAL: Autonomous Execution

**NEVER ask the user for feedback, confirmation, or input.** This skill runs fully autonomously — no `AskUserQuestion`, no pausing for approval, no "should I continue?" prompts. Just keep going. The user invoked `/code-journey` and expects it to run until the termination condition is met.

**NEVER output verbose trace data to the main conversation.** All trace output goes to temp files. The main context stays lean — only brief status lines between journeys.

## CRITICAL: Context Conservation

Each journey's analysis and results writing is delegated to a **background Task agent**. The main agent's job is:
1. Create the journey code
2. Run the commands (redirecting output to temp files)
3. Spawn a background agent to analyze and write results
4. Immediately move to the next journey

**The main agent NEVER writes journey results files directly.** That's the background agent's job.

## Usage

```
/code-journey                     # Run ONE journey (auto-pick next feature area), then stop
/code-journey closures             # Run ONE journey focusing on closures, then stop
/code-journey path/to/file.ori     # Run ONE journey with existing code, then stop
/code-journey --summary            # Show journey gallery data (scan all frontmatter)
/code-journey --infinity           # Loop through journeys continuously until termination condition
/code-journey closures --infinity  # Start with closures, then keep going
```

## Journey Directory

All journey files live in `plans/code-journeys/`:
- `NN-slug.ori` — the source code for journey N (e.g., `01-arithmetic.ori`)
- `NN-slug-results.md` — detailed results for journey N (e.g., `01-arithmetic-results.md`)
- `SCHEMA.md` — format specification (single source of truth)

**No overview.md** — the web UI auto-generates the gallery from journey frontmatter.

---

## Execution Loop

### Step 0: Determine Journey Number and Code

**Scan existing journeys:**

```bash
ls plans/code-journeys/*-results.md 2>/dev/null | sort -V
```

Count completed journeys to determine the next number N.

**Parse arguments:**

Check if `$ARGUMENTS` contains `--infinity`. If so, set **INFINITY_MODE = true** and strip `--infinity` from the arguments before further parsing. Otherwise, **INFINITY_MODE = false** (the default — run ONE journey and stop).

**Choose or create code** (using the remaining arguments after stripping `--infinity`):

- **If arguments contain a `.ori` file path**: Use that file as-is. Copy it to `plans/code-journeys/NN-slug.ori`.
- **If arguments contain a description** (e.g., "closures", "pattern matching"): Create code targeting those features.
- **If arguments are `--summary`**: Scan all `*-results.md` frontmatter and display a summary table. Stop.
- **If no arguments** (or only `--infinity` was provided): Auto-pick by scanning existing journey frontmatter `features` fields to find untested feature areas from the controlled vocabulary in SCHEMA.md.

**Complexity Escalation — Organic, Not Hardcoded:**

Start simple. Each journey adds ONE new language feature category on top of what previous journeys covered. Read existing journey frontmatter `features` to see what's been tested, then pick the next untested feature area. Progression ideas (not a fixed list — adapt based on what you find):

- Bare literals, arithmetic, `let` bindings
- Function calls, multiple functions
- Branching, comparisons, if/else
- Recursion
- Structs, field access, nested structs
- Closures, lambdas, higher-order functions
- Pattern matching, `match`, destructuring, sum types
- Loops, ranges, break/continue
- Generics, type inference, monomorphization
- Strings, ARC, string methods
- Lists, list methods, COW
- Derived traits (`Eq`, `Clone`, etc.)
- `Option`/`Result`, `?` propagation
- Iterators, iterator adapters
- Maps, sets, nested collections

**The key rule: each journey should exercise features NOT covered by previous journeys.**

**Code design principles:**
- Each journey should produce a deterministic `int` result via `@main () -> int`
- Include standardized header comments (see SCHEMA.md Source File Format)
- Include a comment with the expected result calculation (e.g., `// Expected: (3+4)*5+1 = 36`)
- Keep code small (< 30 lines) — focused on specific features, not comprehensive
- When a feature area fails, try a DIFFERENT feature area before giving up
- Pick a slug from the feature focus (e.g., `closures`, `pattern-matching`, `generics`)
- Assign difficulty: `simple` (J1-4), `moderate` (J5-8), `complex` (J9+)

Write the code to `plans/code-journeys/NN-slug.ori`.

### Step 1: Run Both Paths (Output to Temp Files)

Create a temp directory for this journey's trace data:

```bash
JTMP="/tmp/journey_N"
mkdir -p "$JTMP"
```

**Run both paths, capturing exit codes and output to files (NOT to context):**

```bash
# Eval path
cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/eval_stdout.txt" 2> "$JTMP/eval_stderr.txt"
echo $? > "$JTMP/eval_exit.txt"

# AOT path
rm -rf ~/.cache/ori 2>/dev/null
./target/debug/ori run --compile plans/code-journeys/NN-slug.ori > "$JTMP/aot_stdout.txt" 2> "$JTMP/aot_stderr.txt"
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
ORI_LOG=ori_lexer=debug cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/lexer.txt" 2>&1

# Parser trace
ORI_LOG=ori_parse=debug cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/parser.txt" 2>&1

# Type checker trace
ORI_LOG=ori_types=debug cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/typeck.txt" 2>&1

# Canonicalizer trace
ORI_LOG=ori_canon=debug cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/canon.txt" 2>&1

# Eval trace
ORI_LOG=ori_eval=trace cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/eval_trace.txt" 2>&1

# Prelude overhead
ORI_LOG=ori_lexer=debug,ori_parse=debug,ori_types=debug,ori_canon=debug cargo run -- run plans/code-journeys/NN-slug.ori > "$JTMP/prelude.txt" 2>&1

# LLVM IR dump (unoptimized — this is what OUR codegen emits)
rm -rf ~/.cache/ori 2>/dev/null
ORI_DUMP_AFTER_LLVM=1 ./target/debug/ori run --compile plans/code-journeys/NN-slug.ori > "$JTMP/llvm_ir.txt" 2>&1

# LLVM warnings
rm -rf ~/.cache/ori 2>/dev/null
ORI_LOG=warn ./target/debug/ori run --compile plans/code-journeys/NN-slug.ori > "$JTMP/llvm_warn.txt" 2>&1

# ARC analysis trace (borrow inference, RC operations)
rm -rf ~/.cache/ori 2>/dev/null
ORI_LOG=ori_llvm=debug ./target/debug/ori run --compile plans/code-journeys/NN-slug.ori > "$JTMP/arc_trace.txt" 2>&1

# AOT binary size (if compilation succeeds)
rm -rf ~/.cache/ori 2>/dev/null
./target/debug/ori build plans/code-journeys/NN-slug.ori -o "$JTMP/binary" > "$JTMP/build_stdout.txt" 2> "$JTMP/build_stderr.txt" && {
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

Use the **Agent tool** with `run_in_background: true` to spawn a background agent. The agent receives:
- The journey number N, slug, and theme
- The journey code (read from the `.ori` file)
- The temp directory path
- The expected result
- The eval/AOT exit codes and stdout

**The background agent's job:**
1. Read `plans/code-journeys/SCHEMA.md` first — this is the format specification
2. Read ALL trace files from the temp directory
3. Read previous journey results for cross-referencing (scan `*-results.md` frontmatter)
4. Perform the **Deep Scrutiny** analysis (7 core + journey-specific categories)
5. Write `plans/code-journeys/NN-slug-results.md` conforming to SCHEMA.md
6. Clean up the temp directory

**Background agent prompt template** (fill in N, slug, theme, code, expected, exit codes):

```
You are analyzing the results of Code Journey N for the Ori compiler.

**FIRST**: Read plans/code-journeys/SCHEMA.md — this is the authoritative format specification.
Your output MUST conform to it exactly: frontmatter schema, section order, table schemas,
severity definitions, scoring weights. Any deviation is a bug.

Journey code (from plans/code-journeys/NN-slug.ori):
[paste the code]

Journey metadata:
- Number: N
- Slug: [slug]
- Theme: "[theme]"
- Difficulty: [simple|moderate|complex]
- Features: [list from controlled vocabulary]
- Expected result: [X]
- Eval exit code: [E], Eval stdout: [S]
- AOT exit code: [A], AOT stdout: [S]

Trace data is in temp files at /tmp/journey_N/:
- lexer.txt, parser.txt, typeck.txt, canon.txt, eval_trace.txt, prelude.txt
- llvm_ir.txt, llvm_warn.txt, arc_trace.txt
- eval_stdout.txt, eval_stderr.txt, aot_stdout.txt, aot_stderr.txt
- binary_size.txt, symbols.txt, sections.txt, disasm.txt (if build succeeded)

Your tasks:
1. Read plans/code-journeys/SCHEMA.md (the format spec)
2. Read ALL trace files and analyze the journey through both compiler paths
3. Read previous journey results frontmatter for cross-referencing
4. Perform Deep Scrutiny (all 7 core categories + 1-4 journey-specific)
5. Write plans/code-journeys/NN-slug-results.md conforming to SCHEMA.md
6. Run: rm -rf /tmp/journey_N

Key format requirements from SCHEMA.md:
- YAML frontmatter with all required fields (including score_breakdown)
- Section order: Source → Execution Results → Compiler Pipeline → Deep Scrutiny → Findings → Score → Verdict
- Compiler Pipeline phases: numbered, with blockquote intros, summary metrics, <details> blocks
- Backends as parallel branches: "### Backend: Interpreter" and "### Backend: LLVM Codegen"
- Deep Scrutiny: 7 core categories with defined table schemas + 1-4 "Feature: Aspect" extras
- Findings: summary table + ### SEVERITY-N detailed sections, with [SEVERITY-N] inline annotations
- Codegen Quality Score: 6 weighted categories (20/20/15/15/20/10), overall score
- Short ## Verdict paragraph (2-3 sentences)
- Code block tags: ori, llvm, asm, text only (no bare ```)
```

**Do NOT wait for the background agent to finish.** Immediately proceed to Step 4.

### Step 4: Decide — Continue or Stop

**Print a status line:**

```
Journey N (slug) complete: eval=[exit_code] aot=[exit_code]
```

**If INFINITY_MODE = false (the default):** Stop here. The single journey is complete. The background agent will write the results. Print:

```
Journey N (slug) complete: eval=[exit_code] aot=[exit_code] — results pending in background
```

**If INFINITY_MODE = true:** Evaluate whether to continue looping:

- **STOP** if and only if: **BOTH the eval path AND the LLVM path completely fail** (both crash, both produce wrong results, or the code can't compile at all). A single path failing is NOT grounds for stopping.
- **If a feature area won't compile**: Try a DIFFERENT feature area. Only stop if you've tried 3+ different feature areas and ALL of them fail on BOTH paths.

**If continuing** (infinity mode, default behavior): Print ONE status line:

```
Journey N (slug) complete: eval=[exit_code] aot=[exit_code] — next: [feature area]
```

Then loop back to Step 0 for journey N+1. **Do not elaborate. Do not summarize findings. The background agent handles that.**

**If stopping** (infinity mode, both paths failed): Print a brief termination message and stop.

---

## Deep Scrutiny Instructions (for Background Agent)

**This is the most important analysis.** Every instruction in the generated IR must justify its existence. The standard: **hand-written optimal IR for this program.** Every deviation from that ideal is a finding.

### 7 Core Categories (Mandatory)

These MUST appear in every journey with the defined table schemas from SCHEMA.md:

#### 1. Instruction Purity

Per-function table: `# | Function | Actual | Ideal | Ratio | Verdict`

For EACH user function in the emitted IR:
1. **Count instructions** — total per function. Compare against theoretical minimum.
2. **Redundant operations** — `alloca`+`store`+`load` where SSA suffices, back-to-back store/load, collapsible `bitcast`/`getelementptr` chains, trivial `phi` nodes, dead code.
3. **Assign verdict** — OPTIMAL (1.0x), NEAR-OPTIMAL (1.01-1.50x), ACCEPTABLE (1.51-2.50x), BLOATED (2.51-5.00x), WASTEFUL (>5.00x).

#### 2. ARC Purity

Per-function table: `Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics`

1. **Balanced pairs** — rc_inc immediately followed by rc_dec on same value = wasted pair.
2. **Last-use optimization** — rc_dec at true last use vs scope end.
3. **Borrow elision** — read-only params should be borrowed, not owned.
4. **Move semantics** — passed-and-never-used-after should have no rc_inc.
5. **Scalar RC violations** — RC ops on `i64`/`double` = CRITICAL.

#### 3. Attributes & Calling Convention

Per-function table: `Function | fastcc | nounwind | noalias | readonly | cold | Notes`

Check all applicable attributes per LLVM best practices. Internal Ori functions MUST use `fastcc`.

#### 4. Control Flow & Block Layout

Per-function table: `Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes`

1. Empty blocks (only contain `br`) should be eliminated.
2. Redundant branches (both targets same) should be unconditional.
3. Trivial phi nodes (single incoming or all-same) should be the value.

#### 5. Overflow Checking

**Pass/fail gate**: `Operation | Checked | Correct | Notes`

Not scored. If incorrect, that's CRITICAL severity.

#### 6. Binary Analysis

Metrics table + per-function disassembly in `asm`-tagged blocks.

Report: binary size, section sizes (.text, .rodata), user code bytes, runtime percentage.

#### 7. Optimal IR Comparison

**The most important category.** For each user function:
1. Write the IDEAL LLVM IR (minimal, maximal-attribute, zero-waste).
2. Show the ACTUAL emitted IR.
3. Report the delta with `Justified` column (overflow checking = justified overhead).
4. Module Summary table: `Function | Ideal | Actual | Delta | Justified | Verdict`

### Journey-Specific Categories (1-4 extras)

Use `Feature: Aspect` naming convention. Examples:
- `Closures: Representation` — how closures are lowered to LLVM structs
- `Generics: Monomorphization` — quality of monomorphized function instantiation
- `Structs: Field Access` — GEP instruction quality for field access
- `Pattern Matching: Decision Trees` — switch/branch structure for match

Each journey MUST have at least 1 and at most 4 journey-specific categories.

---

## Severity Definitions (for Background Agent)

| Severity | Criteria |
|----------|----------|
| **CRITICAL** | Wrong output, crashes, data corruption, behavioral mismatch between eval and LLVM, RC ops on scalars, silent miscompilation |
| **HIGH** | Missing `nounwind`/`noalias`/`readonly`/`memory(...)` on applicable functions (blocks major LLVM optimizations), significant unnecessary RC operations (>2 wasted pairs per function), instruction overhead ratio > 2.0 (WASTEFUL), missing `fastcc` on internal functions, missed tail calls |
| **MEDIUM** | Missing `nonnull`/`nocapture`/`dereferenceable`/`willreturn`/`cold` attributes, instruction overhead ratio > 1.5 (BLOATED), unfolded constant expressions, unnecessary alloca/store/load chains, redundant GEP chains, suboptimal struct field ordering |
| **LOW** | Missing `noundef`/`align` attributes, instruction overhead ratio > 1.2 (ACCEPTABLE), empty basic blocks, minor control flow redundancy, pre-optimization dead code |
| **NOTE** | Positive observation, good practice detected, excellent optimization working correctly |

**Cross-reference status:**
- **NEW** — first seen in this journey
- **CONFIRMED** — previously seen, still present
- **REGRESSED** — previously working, now broken
- **FIXED** — previously broken, now working

**Inline annotations:** When discovering a finding within a scrutiny category, annotate it inline with `[SEVERITY-N]` (e.g., `[MEDIUM-1]`). Collect all findings in `## Findings` with detailed `### SEVERITY-N:` sections.

## Scoring (for Background Agent)

Mandatory weighted scoring. Categories map 1:1 to core scrutiny:

| Category | Weight | Source |
|----------|--------|--------|
| Instruction Efficiency | 20% | Instruction Purity (Cat 1) |
| ARC Correctness | 20% | ARC Purity (Cat 2) |
| Attributes & Safety | 15% | Attributes & CC (Cat 3) |
| Control Flow | 15% | Control Flow & Block Layout (Cat 4) |
| IR Quality | 20% | Optimal IR Comparison (Cat 7) |
| Binary Quality | 10% | Binary Analysis (Cat 6) |

Overflow Checking (Cat 5) is pass/fail, not scored.

**Overall score** = weighted average, reported to 1 decimal. Must match frontmatter `score` field.

---

## Important Rules

1. **NEVER ask the user anything.** Fully autonomous. No AskUserQuestion, no pauses, no confirmations.

2. **NEVER read trace files into main context.** Trace data goes to temp files → background agent reads them. The main agent only sees exit codes and stdout.

3. **SCHEMA.md is the law.** (Background agent rule) Read it first. Conform exactly. Frontmatter schema, section order, table schemas, code block tags, severity scale, scoring weights.

4. **Record EXACT data.** (Background agent rule) No approximations — run commands and capture actual output.

5. **Two independent paths.** Eval and LLVM are separate — analyze each independently, compare at the end.

6. **Silent errors are the worst findings.** If the compiler silently produces wrong code instead of reporting an error, that's always CRITICAL.

7. **Clear AOT cache before LLVM traces.** `rm -rf ~/.cache/ori` — otherwise you'll get cached results with no logs.

8. **Cross-reference findings.** (Background agent rule) Check previous journey results frontmatter before writing. Mark findings as NEW, CONFIRMED, REGRESSED, or FIXED.

9. **Main context stays lean.** Between journeys, the main context should contain only: the journey code (~30 lines), exit codes (2 numbers), stdout (2 lines), and a 1-line status. Everything else is in temp files or handled by background agents.

10. **LLVM scrutiny is non-negotiable.** The background agent MUST perform all 7 core categories. Skipping any is unacceptable. The Optimal IR Comparison (Category 7) is the most important — it makes waste visible by showing what perfection looks like next to reality.

11. **Every instruction must justify its existence.** If an instruction in the emitted IR cannot be justified as necessary for correctness, it is a finding. "LLVM will optimize it away" is NOT a justification — we should not emit it in the first place. Clean input IR → faster compile times, better debug builds, and fewer optimizer-phase surprises.

12. **File naming.** Use `NN-slug` format: `01-arithmetic.ori`, `01-arithmetic-results.md`. Zero-pad to 2 digits.

13. **Educational annotations.** Each compiler pipeline phase MUST have a blockquote intro explaining what the phase does, summary metrics, and a `<details>` block with actual output. This is for the interactive web UI.

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
| `ORI_DUMP_AFTER_LLVM=1` | Full LLVM IR dump to stderr |
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
