# Code Journey Schema v1.0

> **This is the single source of truth** for the formalized code journey markdown format.
> All journey result files MUST conform to this schema. The `/code-journey` skill
> references this document when generating new journeys.

## File Naming

```
plans/code-journeys/
  NN-slug.ori                 # Source file (e.g., 01-arithmetic.ori)
  NN-slug-results.md          # Results file (e.g., 01-arithmetic-results.md)
  SCHEMA.md                   # This file
```

- `NN` = zero-padded journey number (01, 02, ... 99)
- `slug` = URL-friendly feature name (lowercase, hyphens)

## Source File Format (.ori)

Standardized header comments followed by the journey code:

```ori
// Journey 4: "I am a struct"
// Slug: structs
// Difficulty: moderate
// Features: struct_construction, field_access, nested_structs
// Expected: 34 + 23 = 57

type Point = { x: int, y: int }

@distance (p: Point) -> int = p.x + p.y

@main () -> int = {
    let $p = Point { x: 34, y: 23 }
    distance(p:)
}
```

**Rules:**
- `@main () -> int` — deterministic int result via exit code
- `// Expected:` includes the calculation (e.g., `(3+4)*5+1 = 36`)
- Keep code small (< 30 lines) — focused on specific features
- Each journey exercises features NOT covered by previous journeys

## YAML Frontmatter Schema

Every results file begins with this frontmatter:

```yaml
---
journey: 4                          # int — sequential number
slug: structs                       # string — URL-friendly identifier
theme: "I am a struct"             # string — the journey's narrative name
date: 2026-03-03                   # date — when the journey was run
status: PASS                       # enum: PASS | FAIL_EVAL | FAIL_AOT | FAIL_BOTH
expected: 57                       # int — expected exit code
eval_result: 57                    # int — actual eval exit code
aot_result: 57                     # int — actual AOT exit code

# Educational metadata
difficulty: moderate                # enum: simple | moderate | complex
prerequisites:                     # list of strings
  - "Basic programming knowledge"
  - "Understanding of structs/records"
learning_objectives:                # list of strings (3-5 items)
  - "See how structs are lowered to LLVM struct types"
  - "Understand field access via GEP instructions"
  - "Compare ideal vs actual codegen for struct operations"

# Feature classification
features:                           # list — from controlled vocabulary (see below)
  - struct_construction
  - field_access
  - nested_structs
feature_description: "Struct construction, field access, and nested struct operations"

# Scoring (mandatory)
score: 8.5                         # float — overall weighted score (0-10)
score_breakdown:                   # per-category scores (0-10)
  instruction_efficiency: 8
  arc_correctness: 10
  attributes_safety: 7
  control_flow: 9
  ir_quality: 7
  binary_quality: 8
overflow_check: PASS               # enum: PASS | FAIL

# Bug history (optional — only if bugs were found/fixed)
bugs_found:                        # list of objects
  - id: C3
    severity: CRITICAL
    description: "payload sum type $eq not generated"
    status: FIXED                  # enum: OPEN | FIXED
    found_in: journey11
    fixed_in: commit_abc123

# Cross-journey references (optional)
related_journeys:                  # list of objects
  - journey: 1
    relationship: "Same redundant branch pattern"
  - journey: 5
    relationship: "Both test ARC with closures"
---
```

### Controlled Feature Vocabulary

Use these standardized tags in the `features` list:

| Tag | Description |
|-----|-------------|
| `arithmetic` | Integer/float operations, operators |
| `let_bindings` | `let` / `let $` bindings |
| `int_literals` | Integer literal values |
| `float_literals` | Float literal values |
| `function_calls` | Calling user-defined functions |
| `multiple_functions` | Programs with >1 user function |
| `branching` | `if/then/else` conditionals |
| `comparison` | Comparison operators (`<`, `>`, `==`, etc.) |
| `recursion` | Recursive function calls |
| `struct_construction` | Creating struct values |
| `field_access` | Accessing struct fields |
| `nested_structs` | Structs containing structs |
| `struct_update` | Struct update syntax `{ ...s, field: v }` |
| `closures` | Lambda/closure values |
| `higher_order` | Functions taking/returning functions |
| `capture` | Closure variable capture |
| `pattern_matching` | `match` expressions |
| `sum_types` | Sum type (enum) definitions and construction |
| `destructuring` | Pattern destructuring in `let`/`match` |
| `exhaustiveness` | Exhaustive pattern matching |
| `loops` | `for`/`loop` expressions |
| `ranges` | Range expressions (`0..10`, `0..=10`) |
| `break_continue` | `break`/`continue` in loops |
| `generics` | Generic type parameters |
| `monomorphization` | Generic instantiation |
| `generic_structs` | Structs with type parameters |
| `strings` | String values and operations |
| `string_methods` | String method calls |
| `arc` | ARC/reference counting behavior |
| `lists` | List creation and operations |
| `list_methods` | List method calls |
| `maps` | Map creation and operations |
| `derived_traits` | `#derive(Eq)`, `#derive(Clone)`, etc. |
| `trait_methods` | Calling trait methods |
| `option_type` | `Option<T>`, `Some`/`None` |
| `result_type` | `Result<T, E>`, `Ok`/`Err` |
| `error_propagation` | `?` operator |
| `iterators` | Iterator creation and consumption |
| `iterator_adapters` | `.map()`, `.filter()`, etc. |
| `cow` | Copy-on-write behavior |
| `modules` | `use` imports |
| `type_inference` | HM type inference exercised |
| `newtypes` | Newtype definitions |
| `extensions` | `extend Type { ... }` |

### Difficulty Levels

| Level | Journeys | Compiler Concepts |
|-------|----------|-------------------|
| `simple` | J1-J4 | Lexing, parsing, basic codegen, simple types |
| `moderate` | J5-J8 | Closures, pattern matching, loops, generics |
| `complex` | J9-J12+ | ARC/memory management, collections, traits, error handling |

## Section Order (Mandatory)

Every results file MUST contain these sections in this exact order:

```
# Journey N: "theme"

## Source
## Execution Results
## Compiler Pipeline
  ### 1. Lexer
  ### 2. Parser
  ### 3. Type Checker
  ### 4. Canonicalization
  ### 5. ARC Pipeline
  ### Backend: Interpreter
  ### Backend: LLVM Codegen
    #### ARC Pipeline
    #### Generated LLVM IR
    #### Disassembly
## Deep Scrutiny
  ### 1. Instruction Purity          (core — mandatory)
  ### 2. ARC Purity                  (core — mandatory)
  ### 3. Attributes & Calling Convention  (core — mandatory)
  ### 4. Control Flow & Block Layout     (core — mandatory)
  ### 5. Overflow Checking               (core — mandatory, pass/fail)
  ### 6. Binary Analysis                 (core — mandatory)
  ### 7. Optimal IR Comparison           (core — mandatory)
  ### 8+. Feature: Aspect              (journey-specific, 1-4 extras)
## Findings
## Codegen Quality Score
## Verdict
## Cross-Journey Observations       (optional)
```

**No horizontal rules (`---`)** between sections. `##` headings provide sufficient separation.

## Section Specifications

### `# Journey N: "theme"`

Top-level heading. No "Results" suffix — the filename already indicates it's a results file.

```markdown
# Journey 4: "I am a struct"
```

### `## Source`

The journey source code, embedded in an `ori`-tagged code block:

```markdown
## Source

```ori
// Journey 4: "I am a struct"
// ...
@main () -> int = { ... }
```​
```

### `## Execution Results`

6-column table showing both backend results:

```markdown
## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 57        | 57       | (none) | (none) | PASS   |
| AOT     | 57        | 57       | (none) | (none) | PASS   |
```

### `## Compiler Pipeline`

Numbered phases in the actual compiler order. Each phase has:
1. **Intro paragraph** (blockquote) — 1-2 sentences explaining what the phase does
2. **Summary metrics** — phase-specific key stats on one line
3. **Expandable detail** — `<details>` block with actual compiler output

#### Phase Format Template

```markdown
### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens — the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 42 | **Keywords**: 8 | **Identifiers**: 12 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(add) LParen Ident(a) Colon Ident(int)
Comma Ident(b) Colon Ident(int) RParen Arrow
Ident(int) Eq Ident(a) Plus Ident(b) Semi
```​

</details>
```

#### Phase-Specific Metrics

| Phase | Required Metrics |
|-------|-----------------|
| **1. Lexer** | Tokens, Keywords, Identifiers, Errors |
| **2. Parser** | Nodes, Max depth, Functions, Errors |
| **3. Type Checker** | Constraints, Types inferred, Unifications, Errors |
| **4. Canonicalization** | Transforms, Desugared, Errors |
| **5. ARC Pipeline** | RC ops inserted, Elided, Net ops |

#### Phase-Specific Detail Content

| Phase | `<details>` Content |
|-------|---------------------|
| **1. Lexer** | Token stream (first 20-30 tokens) |
| **2. Parser** | Simplified AST tree (indented text with `├─`/`└─` connectors) |
| **3. Type Checker** | Annotated source with inferred types as comments |
| **4. Canonicalization** | Key transformations applied (e.g., desugaring, lowering) |
| **5. ARC Pipeline** | Per-function RC annotations |

#### AST Visualization Format (Parser)

```text
Module
├─ FnDecl @add
│  ├─ Params: (a: int, b: int)
│  ├─ Return: int
│  └─ Body: BinOp(+)
│       ├─ Ident(a)
│       └─ Ident(b)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Call(@add)
        ├─ a: Lit(10)
        └─ b: Lit(23)
```

#### Type Annotation Format (Type Checker)

```ori
// All types resolved:
@add (a: int, b: int) -> int = a + b
//                               ^ int (from Add<int, int> -> int)

@main () -> int = {
  let $x: int = add(a: 10, b: 23)  // inferred: int
  x  // -> int (matches return type)
}
```

#### Backend Sections

Interpreter and LLVM are parallel branches, not sequential phases:

```markdown
### Backend: Interpreter

**Result**: 42 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ @add(a: 10, b: 23)
       └─ 10 + 23 = 33
  └─ 33 + 9 = 42
→ 42
```​

</details>

### Backend: LLVM Codegen

#### ARC Pipeline

**RC ops inserted**: 6 | **Elided**: 2 | **Net ops**: 4

<details>
<summary>ARC annotations</summary>

```text
@add: +0 rc_inc, +0 rc_dec (no heap values)
@main: +2 rc_inc, +2 rc_dec (balanced)
```​

</details>

#### Generated LLVM IR

```llvm
; ModuleID = 'journey4'
target triple = "x86_64-unknown-linux-gnu"

define fastcc i64 @_ori_add(i64 %a, i64 %b) {
entry:
  ...
}
```​

#### Disassembly

```asm
_ori_add:
  push rbp
  mov rbp, rsp
  ...
```​
```

**The Generated LLVM IR section MUST include the full module** (all user functions). Runtime
declarations may be omitted. This is the raw material for the Optimal IR Comparison.

### `## Deep Scrutiny`

7 mandatory core categories + 1-4 journey-specific extras.

#### Core Category 1: Instruction Purity

Per-function table with verdict labels:

```markdown
### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @add     | 5      | 3     | 1.67x | ACCEPTABLE |
| 2 | @main    | 12     | 8     | 1.50x | NEAR-OPTIMAL |

[Per-function analysis: list every unnecessary instruction and why]
```

**Verdict thresholds:**

| Verdict | Ratio | Color |
|---------|-------|-------|
| OPTIMAL | 1.0x | Green |
| NEAR-OPTIMAL | 1.01x–1.50x | Blue |
| ACCEPTABLE | 1.51x–2.50x | Yellow |
| BLOATED | 2.51x–5.00x | Orange |
| WASTEFUL | >5.00x | Red |

#### Core Category 2: ARC Purity

Per-function table with RC counts:

```markdown
### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @main    | 2      | 2      | YES      | 1 elided       | 0 moves        |
| @process | 3      | 3      | YES      | 0 elided       | 1 move         |

**Verdict**: All functions balanced. No leaks detected.
```

#### Core Category 3: Attributes & Calling Convention

Per-function attribute presence table:

```markdown
### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @add     | YES    | YES      | N/A     | N/A      | NO   |       |
| @main    | YES    | YES      | N/A     | N/A      | NO   |       |
| @panic   | N/A    | NO       | N/A     | N/A      | YES  | [LOW-2] |
```

#### Core Category 4: Control Flow & Block Layout

```markdown
### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @add     | 2      | 0           | 0                 | 0         |       |
| @main    | 3      | 1           | 1                 | 0         | [MEDIUM-1] |

[Analysis of block structure, branch optimization opportunities]
```

#### Core Category 5: Overflow Checking

**Pass/fail gate**, not scored:

```markdown
### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add       | YES     | YES     |       |
| mul       | YES     | YES     |       |
| sub       | YES     | YES     |       |
```

#### Core Category 6: Binary Analysis

Metrics table + per-function disassembly:

```markdown
### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.26 MiB (debug) |
| .text section | 4.1 KiB |
| .rodata section | 1.2 KiB |
| User code | 128 bytes (32 instructions) |
| Runtime | 98% of binary |

#### Disassembly: @add
```asm
_ori_add:
  push rbp
  mov rbp, rsp
  add rdi, rsi
  jo .overflow
  mov rax, rdi
  pop rbp
  ret
```​

#### Disassembly: @main
```asm
_ori_main:
  ...
```​
```

#### Core Category 7: Optimal IR Comparison

**The most important category.** Per-function ideal vs actual, then module summary:

```markdown
### 7. Optimal IR Comparison

#### @add: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc i64 @_ori_add(i64 %a, i64 %b) nounwind {
  %r = add i64 %a, %b
  ret i64 %r
}
```​

```llvm
; ACTUAL (7 instructions)
define fastcc i64 @_ori_add(i64 %a, i64 %b) {
entry:
  %r = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue {i64, i1} %r, 0
  %overflow = extractvalue {i64, i1} %r, 1
  br i1 %overflow, label %panic, label %ok
ok:
  ret i64 %val
panic:
  call void @_ori_panic_overflow(...)
  unreachable
}
```​

**Delta**: +4 instructions (overflow checking — justified for safety)

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @add     | 3     | 7      | +4    | YES (overflow) | ACCEPTABLE |
| @main    | 8     | 12     | +4    | PARTIAL   | NEAR-OPTIMAL |
```

The **Justified** column is important: overflow checking instructions are *expected* overhead.
Only unjustified overhead (redundant branches, unnecessary allocas) should lower the verdict.

#### Journey-Specific Categories (8+)

Use the `Feature: Aspect` naming convention:

```markdown
### 8. Closures: Representation

[Analysis of how closures are lowered to LLVM structs]

### 9. Closures: Safety Analysis

[Analysis of closure capture correctness, lifetime safety]
```

- Minimum 1 journey-specific category
- Maximum 4 journey-specific categories
- Total categories per journey: 8–11

### `## Findings`

Summary table followed by detailed sections for each finding:

```markdown
## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM   | Control Flow | Redundant branch in @main | NEW | J4 |
| 2 | LOW      | Attributes | Missing nounwind on @add | CONFIRMED | J1 |
| 3 | NOTE     | ARC | Excellent borrow elision | NEW | J4 |

### MEDIUM-1: Redundant unconditional branch in @main

**Location**: @main, block `entry` → block `ret`
**Impact**: 1 unnecessary instruction per call
**Fix**: Merge entry and ret blocks when unconditional
**First seen**: Journey 4
**Found in**: Control Flow & Block Layout (Category 4)

### LOW-2: Missing nounwind on @add

**Location**: @add function declaration
**Impact**: LLVM generates unnecessary exception handling tables
**Fix**: Add `nounwind` attribute to all non-unwinding functions
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Excellent borrow elision on struct parameters

**Location**: @distance parameter `p: Point`
**Impact**: Positive — avoids unnecessary rc_inc/rc_dec pair
**Found in**: ARC Purity (Category 2)
```

**Inline annotation**: When a finding is discovered within a scrutiny category, annotate it
inline with `[SEVERITY-N]` (e.g., `[MEDIUM-1]`). The finding is then detailed in this section.

#### Severity Scale

| Severity | Criteria | Color |
|----------|----------|-------|
| **CRITICAL** | Wrong output, crash, data corruption, behavioral mismatch, RC on scalars, silent miscompilation | Red |
| **HIGH** | >2x performance regression, missing safety-critical attributes (`nounwind`/`noalias`/`readonly`/`memory`), significant unnecessary RC ops, missing `fastcc` | Orange |
| **MEDIUM** | 1.5-2x overhead, missing secondary attributes, unfolded constants, redundant alloca/store/load, suboptimal struct layout | Yellow |
| **LOW** | Minor inefficiency (<1.5x), cosmetic attribute gaps, empty basic blocks, pre-optimization dead code | Blue |
| **NOTE** | Positive observation, good practice, excellent optimization | Green |

#### Cross-Reference Status

| Status | Meaning |
|--------|---------|
| **NEW** | First seen in this journey |
| **CONFIRMED** | Previously seen, still present |
| **REGRESSED** | Previously working, now broken |
| **FIXED** | Previously broken, now working |

### `## Codegen Quality Score`

Mandatory weighted scoring table:

```markdown
## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 20% | 8/10 | 1.67x avg overhead |
| ARC Correctness | 20% | 10/10 | All balanced |
| Attributes & Safety | 15% | 7/10 | Missing nounwind on 1 fn |
| Control Flow | 15% | 9/10 | 1 redundant branch |
| IR Quality | 20% | 7/10 | 3 unjustified instructions |
| Binary Quality | 10% | 8/10 | Standard debug size |

**Overall: 8.2 / 10**
```

**Score calculation**: `sum(weight_i × score_i) / sum(weight_i)`

Overflow Checking is pass/fail, not scored — it's a gate, not a gradient.

The `score` and `score_breakdown` in frontmatter MUST match this table.

### `## Verdict`

2-3 sentence qualitative summary. Not redundant with the score — the score is quantitative,
the verdict is qualitative and suitable for web UI cards.

```markdown
## Verdict

Journey 4's struct codegen is near-optimal. Field access compiles to efficient GEP+load
sequences. The main overhead comes from overflow checking on arithmetic (ACCEPTABLE) and
a missing `nounwind` attribute. ARC is perfectly balanced with zero leaks.
```

### `## Cross-Journey Observations` (Optional)

Only include when there are meaningful cross-journey patterns to report:

```markdown
## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J8 | CONFIRMED |
| fastcc usage | J1 | J8 | CONFIRMED |
| Redundant branches | J1 | J8 | CONFIRMED |

[Optional prose about patterns observed across journeys]
```

## Code Block Language Tags

Four standardized tags — no bare ``` blocks:

| Tag | Usage |
|-----|-------|
| `ori` | Ori source code |
| `llvm` | LLVM IR |
| `asm` | Native disassembly (x86_64) |
| `text` | Eval traces, token streams, AST dumps, other non-code output |

## Score Weight Definitions

| Category | Weight | Source Scrutiny Category | What It Measures |
|----------|--------|------------------------|------------------|
| Instruction Efficiency | 20% | 1. Instruction Purity | Actual vs ideal instruction ratio |
| ARC Correctness | 20% | 2. ARC Purity | RC balance, elision, move semantics |
| Attributes & Safety | 15% | 3. Attributes & CC | Attribute presence, fastcc usage |
| Control Flow | 15% | 4. Control Flow & Block Layout | Empty blocks, redundant branches |
| IR Quality | 20% | 7. Optimal IR Comparison | Unjustified overhead delta |
| Binary Quality | 10% | 6. Binary Analysis | Size, symbol quality, native code |

## Complete Example Skeleton

```markdown
---
journey: 1
slug: arithmetic
theme: "I am arithmetic"
date: 2026-03-03
status: PASS
expected: 33
eval_result: 33
aot_result: 33
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
learning_objectives:
  - "Understand how arithmetic expressions are lowered to LLVM IR"
  - "See overflow checking in action"
  - "Compare ideal vs actual codegen for simple functions"
features:
  - arithmetic
  - function_calls
  - let_bindings
  - int_literals
feature_description: "Basic arithmetic with function calls and let bindings"
score: 8.8
score_breakdown:
  instruction_efficiency: 8
  arc_correctness: 10
  attributes_safety: 8
  control_flow: 9
  ir_quality: 8
  binary_quality: 9
overflow_check: PASS
bugs_found: []
related_journeys: []
---

# Journey 1: "I am arithmetic"

## Source

```ori
// Journey 1: "I am arithmetic"
// Slug: arithmetic
// Difficulty: simple
// Features: arithmetic, function_calls, let_bindings, int_literals
// Expected: 10 + 23 = 33

@add (a: int, b: int) -> int = a + b

@main () -> int = add(a: 10, b: 23)
```​

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 33        | 33       | (none) | (none) | PASS   |
| AOT     | 33        | 33       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens — the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 18 | **Keywords**: 2 | **Identifiers**: 6 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(add) LParen Ident(a) Colon Ident(int)
Comma Ident(b) Colon Ident(int) RParen Arrow
Ident(int) Eq Ident(a) Plus Ident(b) Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int)
Eq Ident(add) LParen Ident(a) Colon Lit(10)
Comma Ident(b) Colon Lit(23) RParen Semi
```​

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) — a tree structure that represents the grammatical structure of the program.

**Nodes**: 12 | **Max depth**: 3 | **Functions**: 2 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @add
│  ├─ Params: (a: int, b: int)
│  ├─ Return: int
│  └─ Body: BinOp(+)
│       ├─ Ident(a)
│       └─ Ident(b)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Call(@add)
        ├─ a: Lit(10)
        └─ b: Lit(23)
```​

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 8 | **Types inferred**: 4 | **Unifications**: 6 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@add (a: int, b: int) -> int = a + b
//                               ^ int (Add<int, int> -> int)

@main () -> int = add(a: 10, b: 23)
//                ^ int (return type of @add)
```​

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 2 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Function bodies lowered to canonical expression form
- Call arguments normalized to positional order
```​

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead — parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@add: no heap values — pure scalar arithmetic
@main: no heap values — pure scalar arithmetic
```​

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 33 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ @add(a: 10, b: 23)
       └─ 10 + 23 = 33
→ 33
```​

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@add: +0 rc_inc, +0 rc_dec (no heap values)
@main: +0 rc_inc, +0 rc_dec (no heap values)
```​

</details>

#### Generated LLVM IR

```llvm
; ModuleID = 'journey1'
target triple = "x86_64-unknown-linux-gnu"

define fastcc i64 @_ori_add(i64 %a, i64 %b) {
entry:
  %r = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue {i64, i1} %r, 0
  %overflow = extractvalue {i64, i1} %r, 1
  br i1 %overflow, label %panic, label %ok
ok:
  ret i64 %val
panic:
  call void @_ori_panic_overflow()
  unreachable
}

define fastcc i64 @_ori_main() {
entry:
  %r = call fastcc i64 @_ori_add(i64 10, i64 23)
  ret i64 %r
}
```​

#### Disassembly

```asm
_ori_add:
  push rbp
  mov rbp, rsp
  add rdi, rsi
  jo .overflow
  mov rax, rdi
  pop rbp
  ret
```​

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @add     | 7      | 3     | 2.33x | ACCEPTABLE |
| 2 | @main    | 3      | 3     | 1.00x | OPTIMAL |

@add overhead is from overflow checking (justified).

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @add     | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: No heap values. Zero RC operations. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @add     | YES    | NO       | N/A     | N/A      | NO   | [LOW-1] |
| @main    | YES    | NO       | N/A     | N/A      | NO   | [LOW-1] |

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @add     | 3      | 0           | 0                 | 0         |       |
| @main    | 1      | 0           | 0                 | 0         |       |

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add       | YES     | YES     | Uses llvm.sadd.with.overflow |

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.26 MiB (debug) |
| .text section | 2.8 KiB |
| User code | 48 bytes |

#### Disassembly: @add

```asm
_ori_add:
  add rdi, rsi
  jo .panic
  mov rax, rdi
  ret
```​

### 7. Optimal IR Comparison

#### @add: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc i64 @_ori_add(i64 %a, i64 %b) nounwind {
  %r = add i64 %a, %b
  ret i64 %r
}
```​

```llvm
; ACTUAL (7 instructions)
define fastcc i64 @_ori_add(i64 %a, i64 %b) {
entry:
  %r = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue {i64, i1} %r, 0
  %overflow = extractvalue {i64, i1} %r, 1
  br i1 %overflow, label %panic, label %ok
ok:
  ret i64 %val
panic:
  call void @_ori_panic_overflow()
  unreachable
}
```​

**Delta**: +4 instructions (overflow checking — justified for safety)

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @add     | 3     | 7      | +4    | YES       | ACCEPTABLE |
| @main    | 3     | 3      | +0    | N/A       | OPTIMAL |

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW      | Attributes | Missing nounwind on user functions | NEW | J1 |

### LOW-1: Missing nounwind on user functions

**Location**: @add, @main function declarations
**Impact**: LLVM generates unnecessary exception handling tables
**Fix**: Add `nounwind` attribute to all non-unwinding functions
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 20% | 8/10 | Overflow adds justified overhead |
| ARC Correctness | 20% | 10/10 | No heap values, zero RC ops |
| Attributes & Safety | 15% | 8/10 | Missing nounwind |
| Control Flow | 15% | 10/10 | Clean control flow |
| IR Quality | 20% | 8/10 | Overflow justified, nounwind missing |
| Binary Quality | 10% | 9/10 | Compact user code |

**Overall: 8.8 / 10**

## Verdict

Journey 1's arithmetic codegen is clean. The only overhead comes from overflow checking
(justified for safety) and a missing `nounwind` attribute. ARC is irrelevant for pure
scalar arithmetic — zero RC operations. The ideal IR comparison shows that without
overflow checking, codegen would be OPTIMAL.
```​

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-04 | Initial schema based on 50-question design session |
