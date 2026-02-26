# Journey 1: "I am `42`"

**Code**: `@main () -> int = { 42 }`
**Source**: 29 bytes

## My Transformation Timeline

### Stage 1: Lexer-Core (RawScanner)
**Input**: 29 bytes of source text
**Output**: Stream of `(RawTag, len)` pairs

The raw scanner splits me into 14 raw tokens (byte boundaries only):
```
At(1) Ident(4) LeftParen(1) RightParen(1) Arrow(2) Ident(3) Equal(1) LeftBrace(1)
Newline(1) Whitespace(4) Int(2) Newline(1) RightBrace(1) Newline(1)
```
- Whitespace/newline are separate tokens (3 newlines, 1 whitespace)
- No allocations, no string copies — just tag+len pairs from byte scanning

### Stage 2: Lexer (TokenCooker)
**Input**: RawTag stream
**Output**: `TokenList` (14 cooked tokens) + metadata

The cooker transforms each raw token:
```
offset=0  At      → @
offset=1  Ident   → Ident(Name(shard=9, local=3))  [interned "main"]
offset=6  LParen  → (
offset=7  RParen  → )
offset=9  Arrow   → ->
offset=12 Ident   → int                             [keyword, not interned]
offset=16 Equal   → =
offset=18 LBrace  → {
                   → Newline                         [emitted with flags]
offset=24 Int     → Int(42)                          [parsed u64]
                   → Newline
offset=27 RBrace  → }
                   → Newline
                   → Eof
```

**Key observations**:
- `"main"` is interned as `Name(shard=9, local=3)` — sharded interner
- `"int"` resolves to keyword `TokenKind::Int` (the type keyword), NOT interned
- `42` is parsed to `u64` value at lex time — no re-parsing later
- 3 Newline tokens are emitted (significant for expression termination)
- TokenFlags: SPACE_BEFORE, NEWLINE_BEFORE, LINE_START, ADJACENT tracked

**Summary**: 29 bytes → 14 tokens, 0 errors, 0 comments

### Stage 3: Parser
**Input**: 14 tokens
**Output**: Module with 1 function, ExprArena with 2 expressions

Parser dispatch:
```
parse_module start (token_count=14)
  dispatch_declaration (pos=0, kind="@")
    → entering parse context "function definition"
    → consume @, Ident("main"), (, ), ->, int, =
    → parse_expr (pos=7, kind="{")
      → parse_primary: { → block expression
        → parse_expr (pos=9, kind="integer")
          → parse_primary: Int(42) → literal expression
        → consume }, Newline
  → 1 function parsed
parse_module complete (functions=1, expressions=2)
```

**ExprArena contents** (2 nodes):
- `ExprId(0)`: `ExprKind::Int(42)` at span `24..26`
- `ExprId(1)`: `ExprKind::Block(stmts=[], tail=ExprId(0))` at span `18..28`

**Observation**: The block wrapping is mandatory syntax — `{ 42 }` creates a Block node even when there's only one expression. This means every function body has at least 2 AST nodes (Block + content).

### Stage 4: Type Checker
**Input**: Module + ExprArena + prelude import
**Output**: TypeCheckResult + Pool

**MAJOR FINDING — PRELUDE OVERHEAD**:
Before type-checking my 29-byte file, the compiler must:
1. **Lex the prelude**: 10,322 bytes → 1,530 tokens (53x my token count!)
2. **Parse the prelude**: 1,530 tokens → 9 functions, 39 traits, 46 expressions
3. **Type-check the prelude**: registration + signatures + body checking

Type checker passes for MY code:
```
registration passes complete (functions=1, tests=0, impls=0)
signature collection complete (functions=1, tests=0, impls=0)
body checking complete         (functions=1, tests=0, impls=0)
```

The `42` literal gets type `int` directly — no inference needed. The block's type is inferred from its tail expression → `int`. Return type `int` matches.

### Stage 5a: Canonicalizer
**Input**: Module + ExprArena + TypeCheckResult + Pool
**Output**: CanonResult (2 canon nodes)

```
canon lower_module started (functions=1, tests=0, impls=0, source_exprs=2)
  lower_expr ExprId(1) ExprKind::Block → CanExpr::Block
  lower_expr ExprId(0) ExprKind::Int(42) → CanExpr::Int(42)
canon lower_module complete (canon_nodes=2, roots=1, method_roots=0, constants=6, decision_trees=0)
```

**Observation**: 6 constants in the pool for a program that has only one literal! These are likely prelude constants (compile-time folded values from prelude functions). This is shared infrastructure cost, not a problem.

---

## Journey A: Eval Path

### Stage 6a: Interpreter
```
registering prelude    → str, int, float, byte, Error, repeat, hash_combine, thread_id,
                         Less, Equal, Greater, format variants
                       → type-checks AND canonicalizes prelude.ori AGAIN for the evaluator
eval_can(CanId(1))     → Block(CanRange(0..0), CanId(0))
  eval_can(CanId(0))   → Int(42)
                       → Value::Int(42)
```

**SECOND MAJOR FINDING — DOUBLE PROCESSING**:
The prelude is type-checked AND canonicalized TWICE:
1. First during type checking of the user's file (to resolve imported types)
2. Again during evaluator setup (to register prelude functions)

Both invocations show in the trace:
```
type checking path=prelude.ori  [during typed() query]
  check_module_with_imports: registration passes complete (functions=9)
  check_module_with_imports: signature collection complete (functions=9)
  check_module_with_imports: body checking complete (functions=9)
  canon lower_module complete (canon_nodes=46, roots=9, decision_trees=4)

[... later, during eval ...]
type checking path=prelude.ori  [during register_prelude]
  check_module_with_imports: registration passes complete (functions=9)
  ... (same again)
```

**Actual eval work for `42`**: 2 eval_can calls, producing `Value::Int(42)`.

### Eval Path Cost Summary
| Phase | My code | Prelude overhead |
|-------|---------|-----------------|
| Lexing | 29 bytes → 14 tokens | 10,322 bytes → 1,530 tokens |
| Parsing | 14 tokens → 2 exprs | 1,530 tokens → 46 exprs, 39 traits |
| Type check | 3 passes, 1 function | 3 passes, 9 functions (×2!) |
| Canonicalize | 2 canon nodes | 46 canon nodes (×2!) |
| Eval | 2 eval_can calls | prelude registration |

---

## Journey B: LLVM Path

### Stage 6b: LLVM Codegen
```
registering user types:
  Ordering enum (3 variants: Less, Equal, Greater)
  FormatType enum
  FormatSpec struct (6 fields)
  PanicInfo struct (4 fields)
  Alignment enum (3 variants)
  Sign enum (3 variants)

ARC/Borrow analysis:
  created ArcModuleInput (function_count=1)
  computing SCC decomposition (function_count=1) → 1 SCC
  Salsa borrow inference complete (sig_count=1, scc_count=1)

Code generation:
  declaring function name="main" symbol="_ori_main" params=0 call_conv=C return_passing=Direct
  defining function body (ARC, pre-lowered) name="main" tier=2
  generating C main() entry point wrapper (has_args=false, returns_int=true)
```

**THIRD FINDING — TYPE REGISTRATION OVERHEAD**:
LLVM codegen registers 6 user types (Ordering, FormatType, FormatSpec, PanicInfo, Alignment, Sign) even though my program uses none of them. These are prelude types that get registered "just in case."

**FOURTH FINDING — ARC ANALYSIS FOR TRIVIAL CODE**:
The SCC decomposition + borrow inference runs for `@main` even though the function returns a literal integer. No references, no allocations, no ARC — but the full analysis pipeline runs anyway.

### LLVM Path Cost Summary
| Phase | My code work | Prelude overhead |
|-------|-------------|-----------------|
| Type registration | none | 6 types registered |
| ARC/borrow analysis | 1 SCC (trivial) | Full pipeline startup |
| Code generation | 1 function declared + defined | C main() wrapper generated |
| Function IR | `ret i64 42` | ABI setup, entry block |

---

## Issues Found

### CRITICAL
1. **Double prelude processing** — The prelude is fully lexed, parsed, type-checked, and canonicalized twice: once for type checking and once for evaluation. This is the #1 performance issue for small files. Salsa should be caching the second invocation.

### HIGH
2. **Prelude 53x token amplification** — A 29-byte program requires lexing 10,322 bytes of prelude. While Salsa caches this for subsequent compilations in a session, first-compilation latency is dominated by prelude processing.

3. **Prelude parse error** — The prelude parse shows `errors=1`. A parse error in the standard library prelude should never happen — this needs investigation.

### MEDIUM
4. **Unnecessary type registration in LLVM** — 6 prelude types (Ordering, FormatSpec, PanicInfo, etc.) are registered in LLVM codegen even when unused. Lazy registration would reduce codegen startup.

5. **ARC analysis on trivial functions** — SCC decomposition and borrow inference run even for functions that clearly don't use references. A fast-path check (no heap types in signature or body) could skip this.

6. **Block wrapping overhead** — Every function body creates a Block + content node even for single expressions. While this is structurally correct, constant folding could collapse `Block([], Int(42))` → `Int(42)` in the canon IR.

### LOW
7. **6 constants in canon pool** — For a program with 1 literal, 6 constants exist (from prelude const-folding). Not a problem, but documents shared-pool cost.

8. **Name interning visibility** — Trace output shows `Name(shard=9, local=3)` for "main" which is useful for debugging but could optionally resolve to the string for readability.
