---
name: design-pattern-review
description: Compare Ori's design against prior art from established compilers and propose best-of-breed designs
allowed-tools: Read, Grep, Glob, Task, Bash, Write
---

# Design Pattern Review

Compare Ori's current implementation in a specific domain against established compilers (Rust, Go, Zig, Gleam, Elm, Roc, Swift, Koka, Lean 4, TypeScript), then propose a best-of-breed design combining the strongest patterns.

**One domain per invocation.** This keeps each run focused and within context limits.

## Invocation

`/design-pattern-review <domain>`

### Domains

| Domain | Ori Crates | Best Reference Repos | Prior Art Section |
|--------|-----------|---------------------|-------------------|
| `error-messages` | ori_diagnostic, ori_types, ori_parse | Elm, Rust, Roc, Gleam | 1 |
| `type-representation` | ori_ir (type_id), ori_types | Rust, Zig, Gleam | 2 |
| `incremental` | ori_types (Salsa), oric queries | Rust, Zig, TypeScript | 3 |
| `code-fixes` | ori_diagnostic (suggestions) | Rust, TypeScript, Gleam | 4 |
| `architecture` | all crate lib.rs files | Rust, Zig, Go, Gleam | 5 |
| `test-infrastructure` | oric test runner, tests/spec/ | Rust, Zig, Gleam, Elm | 6 |
| `arc-optimization` | ori_arc | Swift, Lean 4, Koka | 7 |
| `effect-system` | (design phase — proposals/docs) | Koka | 2 |
| `pattern-matching` | ori_patterns, ori_eval decision_tree | Rust, Gleam, Elm | — |

If `$ARGUMENTS` is empty or unrecognized, use `AskUserQuestion` to present the domain list.

---

## Execution

### Step 1: Resolve Domain

Parse `$ARGUMENTS`. Match against the domain table. Determine:
- Which Ori files Agent A should read (from the file map below)
- Which reference repos Agent B should read (from the file map below)
- Which prior-art-ref.md section to reference

### Step 2: Launch Two Research Agents in Parallel

Send a **single message with 2 Task tool calls**, both with `run_in_background: true`.

#### Agent A: Read Ori's Current Implementation

```
Task(
  subagent_type: "Explore",
  description: "Read Ori: {domain}",
  run_in_background: true,
  prompt: <see Agent A template below, filled with domain-specific file list>
)
```

#### Agent B: Read Reference Compilers

```
Task(
  subagent_type: "Explore",
  description: "Read Refs: {domain}",
  run_in_background: true,
  prompt: <see Agent B template below, filled with domain-specific repos/paths>
)
```

### Step 3: Collect Research Results

Read both agents' output files. Extract their summaries — do NOT re-investigate or expand them.

### Step 4: Launch Synthesis Agent

```
Task(
  subagent_type: "general-purpose",
  description: "Synthesize: {domain}",
  prompt: <see Agent C template below, with both summaries injected>
)
```

This agent writes the proposal to `plans/dpr_{domain}_{MMDDYYYY}.md`.

### Step 5: Report

Tell the user where the proposal was written. Give a 3-5 line summary of the key design insight.

---

## Agent Prompt Templates

### Agent A: Ori Current State

```
You are analyzing Ori's current implementation of {DOMAIN}.

Read these files thoroughly:

{DOMAIN-SPECIFIC ORI FILE LIST — see file map below}

Produce a structured summary covering:

## Ori's Current {Domain} Design

### Architecture
How it's structured: key types, data flow, module organization.

### Strengths
3-5 bullet points on what works well.

### Gaps & Pain Points
3-5 bullet points on what's missing, awkward, or inconsistent.

### Key Types & Interfaces
The most important types/traits/functions with brief descriptions.

RULES:
- Actually READ each file — don't just grep for patterns
- Focus on DESIGN CHOICES, not code style or hygiene
- Keep output under 150 lines
- Be honest about gaps — this is for improvement, not validation
```

### Agent B: Reference Compiler Research

```
You are researching how established compilers handle {DOMAIN}.

STEP 1: Read `.claude/skills/design-pattern-review/prior-art-ref.md` section {N}
for an overview of patterns in this domain.

STEP 2: Dive into these reference repos at ~/projects/reference_repos/lang_repos/:

{DOMAIN-SPECIFIC REPO PATHS — see file map below, 2-3 repos}

For each compiler, read the actual source files listed. Extract the key design
patterns — don't just describe the API surface, understand WHY they made each
design choice.

Produce a structured summary:

## Prior Art: {Domain}

### {Compiler 1} — {Their Key Pattern Name}
**Approach:** 1-2 sentence summary
**Key Design Choice:** The most important decision and why they made it
**Concrete Pattern:** Brief code structure or type layout
**Tradeoff:** What they gain vs what they sacrifice

### {Compiler 2} — {Their Key Pattern Name}
{same format}

### {Compiler 3} — {Their Key Pattern Name}
{same format}

### Cross-Cutting Patterns
Patterns appearing in 2+ compilers — these are likely universal best practices.

RULES:
- READ actual source files, not just file names
- Focus on the 2-3 most relevant repos, not all of them
- Extract DESIGN PATTERNS, not implementation details
- Keep output under 150 lines
- Note disagreements between compilers — where they chose differently, explain why
```

### Agent C: Best-of-Breed Synthesis

```
You are synthesizing a best-of-breed design for Ori's {DOMAIN}.

You have two research summaries. Read them carefully:

--- ORI CURRENT STATE ---
{Agent A output — paste full summary here}

--- PRIOR ART ---
{Agent B output — paste full summary here}

YOUR TASK: Write a design proposal that combines the strongest patterns from
reference compilers, adapted to Ori's unique constraints:
- Expression-based language (no return keyword)
- ARC memory management (no GC, no borrow checker)
- Capability-based effects
- Salsa-based incremental compilation
- Configurable test enforcement (off/warn/error)

Write the proposal to: plans/dpr_{DOMAIN}_{MMDDYYYY}.md

USE THIS EXACT FORMAT:

---
plan: "dpr_{DOMAIN}_{MMDDYYYY}"
title: "Design Pattern Review: {Domain Title}"
status: draft
---

# Design Pattern Review: {Domain Title}

## Ori Today

{2-3 paragraphs: what exists, what works, what's missing. Be specific — cite
types, functions, modules. Don't just say "it works well", say why.}

## Prior Art

### {Compiler 1} — {Key Pattern Name}
{What they do, why it works, 1-2 paragraphs}

### {Compiler 2} — {Key Pattern Name}
{same}

### {Compiler 3} — {Key Pattern Name}
{same}

## Proposed Best-of-Breed Design

### Core Idea
{1-2 paragraphs: the combined design. What are we taking from each compiler
and how do they fit together?}

### Key Design Choices
{Numbered list. Each choice cites which compiler(s) inspired it and explains
why it's the right fit for Ori specifically.}

### What Makes Ori's Approach Unique
{Where Ori's constraints (ARC, effects, expression-based) create opportunities
that none of the reference compilers have. This is the novel contribution.}

### Concrete Types & Interfaces
{Rust pseudocode sketching the key types, traits, or functions. This should be
concrete enough to start implementing from.}

## Implementation Roadmap

### Phase 1: Foundation
- [ ] {task with brief description}

### Phase 2: Core
- [ ] {task}

### Phase 3: Polish
- [ ] {task}

## References
{List each reference repo file that was studied}

RULES:
- Be CONCRETE — pseudocode over prose
- Cite your sources — every design choice should reference which compiler inspired it
- Don't just copy one compiler — combine the best of multiple
- Address Ori's unique constraints explicitly
- The proposal should be implementable, not aspirational
```

---

## Domain-Specific File Maps

Use these to populate Agent A and Agent B prompts.

### error-messages

**Agent A (Ori):**
- `compiler/ori_diagnostic/src/lib.rs` — diagnostic types
- `compiler/ori_diagnostic/src/emitter/terminal.rs` — terminal rendering
- `compiler/ori_diagnostic/src/error_code.rs` — error codes
- `compiler/ori_types/src/output/mod.rs` — type error construction
- `compiler/ori_parse/src/error.rs` — parse error construction
- `compiler/ori_eval/src/errors.rs` — eval error construction

**Agent B (Refs):**
- Elm: `elm/compiler/src/Reporting/Error.hs`, `Reporting/Error/Type.hs`, `Reporting/Doc.hs`, `Reporting/Suggest.hs`
- Rust: `rust/compiler/rustc_errors/src/lib.rs`, `rustc_errors/src/diagnostic.rs`
- Roc: `roc/crates/reporting/src/report.rs`, `crates/reporting/src/error/type.rs`
- Gleam: `gleam/compiler-core/src/error.rs`, `compiler-core/src/diagnostic.rs`
- **Prior Art Section:** 1

### type-representation

**Agent A (Ori):**
- `compiler/ori_ir/src/type_id.rs` — type IDs
- `compiler/ori_ir/src/interner.rs` — interning
- `compiler/ori_ir/src/arena.rs` — arena allocation
- `compiler/ori_types/src/lib.rs` — type checker entry
- `compiler/ori_types/src/check/bodies.rs` — body checking

**Agent B (Refs):**
- Rust: `rust/compiler/rustc_middle/src/ty/mod.rs`, `rustc_middle/src/ty/sty.rs`
- Zig: `zig/src/InternPool.zig`, `zig/src/Type.zig`
- Gleam: `gleam/compiler-core/src/type_.rs`, `gleam/compiler-core/src/analyse.rs`
- **Prior Art Section:** 2

### incremental

**Agent A (Ori):**
- `compiler/ori_types/src/lib.rs` — Salsa db setup
- `compiler/oric/src/query/mod.rs` — query definitions
- `compiler/oric/src/lib.rs` — database configuration

**Agent B (Refs):**
- Rust: `rust/compiler/rustc_middle/src/dep_graph/`, `rustc_query_system/src/`
- Zig: `zig/src/Compilation.zig`
- TypeScript: `typescript/src/compiler/builder.ts`, `typescript/src/compiler/builderState.ts`
- **Prior Art Section:** 3

### code-fixes

**Agent A (Ori):**
- `compiler/ori_diagnostic/src/lib.rs` — suggestion types
- `compiler/ori_diagnostic/src/emitter/terminal.rs` — suggestion rendering
- Any files with `Suggestion` or `CodeFix` types

**Agent B (Refs):**
- Rust: `rust/compiler/rustc_errors/src/diagnostic.rs` (Applicability enum)
- TypeScript: `typescript/src/services/codeFixProvider.ts`, `typescript/src/services/textChanges.ts`
- Gleam: `gleam/compiler-core/src/error.rs` (suggestion patterns)
- **Prior Art Section:** 4

### architecture

**Agent A (Ori):**
- All `compiler/*/src/lib.rs` files (9 crates)
- `compiler/oric/src/commands/` — CLI entry points
- `Cargo.toml` — workspace dependencies

**Agent B (Refs):**
- Rust: `rust/compiler/rustc_driver/src/lib.rs`, `rustc_interface/src/passes.rs`
- Zig: `zig/src/Compilation.zig`, `zig/src/main.zig`
- Go: `golang/src/cmd/compile/internal/gc/main.go`
- Gleam: `gleam/compiler-core/src/build.rs`, `gleam/compiler-core/src/lib.rs`
- **Prior Art Section:** 5

### test-infrastructure

**Agent A (Ori):**
- `compiler/oric/src/test/runner.rs` — test runner
- `compiler/oric/src/testing/harness.rs` — test harness
- `tests/spec/` — spec conformance tests (read directory structure + a few examples)
- `compiler/oric/tests/` — phase tests (read directory structure)

**Agent B (Refs):**
- Rust: `rust/tests/ui/` (structure), `rust/src/tools/compiletest/src/`
- Zig: `zig/test/` (inline test structure)
- Gleam: `gleam/compiler-core/src/*/tests.rs`
- Elm: `elm/tests/`
- **Prior Art Section:** 6

### arc-optimization

**Agent A (Ori):**
- `compiler/ori_arc/src/` — all files in the ARC crate
- `compiler/ori_arc/src/lower/control_flow.rs` — control flow lowering
- `compiler/ori_arc/src/reset_reuse.rs` — reset/reuse optimization
- `compiler/ori_llvm/src/codegen/arc_emitter.rs` — ARC codegen

**Agent B (Refs):**
- Swift: `swift/lib/SILOptimizer/ARC/`, `swift/lib/SIL/`, `swift/include/swift/AST/Ownership.h`
- Lean 4: `lean4/src/Lean/Compiler/IR/RC.lean`, `lean4/src/Lean/Compiler/IR/Borrow.lean`, `lean4/src/Lean/Compiler/IR/ExpandResetReuse.lean`
- Koka: `koka/src/Core/Borrowed.hs`, `koka/src/Core/CheckFBIP.hs`
- **Prior Art Section:** 7

### effect-system

**Agent A (Ori):**
- Any existing capability/effect design docs in `docs/ori_lang/proposals/`
- `compiler/ori_ir/src/` — look for effect-related types
- `compiler/ori_types/src/` — look for effect tracking

**Agent B (Refs):**
- Koka: `koka/src/Type/Infer.hs`, `koka/src/Type/Operations.hs`, `koka/src/Type/Unify.hs`, `koka/src/Compile/`
- **Prior Art Section:** 2 (effects subsection)

### pattern-matching

**Agent A (Ori):**
- `compiler/ori_patterns/src/lib.rs` — pattern system entry
- `compiler/ori_patterns/src/recurse.rs` — recursive patterns
- `compiler/ori_eval/src/exec/decision_tree.rs` — decision tree execution
- `compiler/ori_canon/src/patterns.rs` — pattern canonicalization

**Agent B (Refs):**
- Rust: `rust/compiler/rustc_pattern_analysis/src/`
- Gleam: `gleam/compiler-core/src/exhaustiveness.rs`
- Elm: `elm/compiler/src/Nitpick/PatternMatches.hs`
- **Prior Art Section:** None — read repos directly

---

## Output Location

Proposals are written to: `plans/dpr_{domain}_{MMDDYYYY}.md`

Examples:
- `plans/dpr_error-messages_02112026.md`
- `plans/dpr_arc-optimization_02112026.md`

---

## Orchestrator Discipline

**You are a thin coordinator.** Your jobs:

1. **Resolve** the domain from `$ARGUMENTS`
2. **Launch** Agents A and B in parallel (single message, both `run_in_background: true`)
3. **Collect** their output files when both complete
4. **Launch** Agent C with both summaries injected
5. **Report** the file path and a brief summary to the user

**Rules:**
- **NEVER read source code yourself** — Agents A and B do that
- **NEVER read prior-art-ref.md yourself** — Agent B does that
- **NEVER re-investigate agent findings** — trust their output
- **DO inject Agent A and B output into Agent C's prompt** — this is the one place where you pass content between agents
- **Keep your own context minimal** — you're a dispatcher, not an analyst
