# The Quest of Forty-Two: A Code Journey Through the Ori Compiler

> *A fantasy MUD adventure in which a humble integer literal traverses the perilous
> depths of the Ori compiler, from source text to final execution.*

---

```
╔═══════════════════════════════════════════════════════════════════╗
║                    THE QUEST OF FORTY-TWO                        ║
║              An Interactive Compiler Adventure                   ║
║                                                                  ║
║  You are 42 — a simple integer literal, born as ink on parchment.║
║  Your destiny: to become the return value of @main.              ║
║  Two roads diverge before you: the Path of Eval, and the         ║
║  Path of Iron (LLVM). Both lead to the same truth.               ║
║  ...or do they?                                                  ║
╚═══════════════════════════════════════════════════════════════════╝
```

## Prologue: The Source Scroll

```
You find yourself inscribed upon a scroll of 29 bytes:

    @main () -> int = {
        42
    }

You are the "42" — two ASCII runes, offset 24 and 25.
Around you: the sigil @, the name "main", parentheses like
stone archways, the arrow -> pointing toward destiny, and
the sacred braces { } that form your chamber.

> look
You are in a SOURCE FILE. Exits: LEXER to the north.
The parchment smells of fresh ink. A faint hum of compilation
magic fills the air.

> north
```

---

## Chapter I: The Lexer's Gate

### The Raw Scanner (Stage 1)

```
You step through the northern gate into the HALL OF THE RAW SCANNER.

The RawScanner — an ancient automaton — awakens. Its single eye
sweeps across the scroll, byte by byte, splitting reality into
14 raw fragments:

  At(1) Ident(4) LeftParen(1) RightParen(1) Arrow(2) Ident(3)
  Equal(1) LeftBrace(1) Newline(1) Whitespace(4) Int(2)
  Newline(1) RightBrace(1) Newline(1)

The automaton does not understand WHAT you are. It only knows
WHERE you begin and WHERE you end. You are tagged: Int(2) —
two bytes wide. No allocations. No copies. Just measurements.

> examine self
You are RAW TAG: Int, LENGTH: 2.
The scanner neither knows nor cares that you are "42."
You are merely a span of bytes that looks numeric.

HP: ██████████ 100%   FORM: Raw bytes
```

### The Token Cooker (Stage 2)

```
A second figure emerges from the shadows: the TOKEN COOKER.
She takes each raw fragment and TRANSFORMS it.

The Cooker reaches you — raw Int(2) at offset 24.

  "Ah," she says, parsing your bytes. "You are not merely
  Int-shaped. You ARE the integer 42."

She reaches into the scroll, reads your two runes ('4', '2'),
and COOKS you into a proper token:

  TokenKind::Int(42)    — a u64 value, parsed at lex time
  Offset: 24
  Flags: NEWLINE_BEFORE, LINE_START

> inventory
COOKED TOKEN: Int(42)
  - Value: 42 (u64, no re-parsing needed ever again)
  - Span: bytes 24..26
  - Flags: NEWLINE_BEFORE | LINE_START

Meanwhile, your companion "main" has been interned into the
NAME VAULT as Name(shard=9, local=3) — a compact reference
that all future stages will use instead of the raw string.

The keyword "int" was recognized as a TYPE SIGIL — not interned,
but transmuted directly into TokenKind::Int (the type keyword).

> look
You are in the TOKEN STREAM. 14 tokens total, 0 errors.
29 source bytes consumed. 125 comments devoured (none were yours).
A vast doorway labeled PARSER looms ahead.

> north
```

---

## Chapter II: The Parser's Labyrinth

```
╔══════════════════════════════════════════╗
║       THE PARSER'S LABYRINTH             ║
║   Where tokens become trees of meaning   ║
╚══════════════════════════════════════════╝

You enter a vast geometric hall. The Parser — a recursive
descent architect — examines each token in sequence.

  parse_module start (token_count=14)

She consumes tokens greedily:
  @ → "Ah, a function declaration!"
  Ident("main") → "Named 'main'."
  ( ) → "No parameters."
  -> int → "Returns an integer."
  = → "Here comes the body..."
  { → "A block expression!"

She descends into the block. Inside, she finds YOU:

  "An integer literal: 42."

She allocates you a home in the EXPRESSION ARENA:

  ExprId(0): ExprKind::Int(42)     at span 24..26
  ExprId(1): ExprKind::Block(      at span 18..28
               stmts=[],
               tail=ExprId(0)    ← that's you!
             )

> examine arena
The Expression Arena is a grand hall with numbered alcoves.
You occupy Alcove #0. Above you, Block occupies Alcove #1,
pointing down at you as its "tail expression" — the value
that the block evaluates to.

parse_module complete:
  functions=1, tests=0, types=0, traits=0
  impls=0, imports=0, expressions=2, errors=0

> look
You are ExprId(0) in the AST. You are the tail of a Block.
The Block is the body of @main. Two exits lie before you:
TYPE CHECKER to the east, and far beyond, the TWIN PATHS.

⚠️ OBSERVATION: The Block wrapper is mandatory. Even though
you are the ONLY expression, the parser wraps you in a Block.
Every function body = at least 2 AST nodes. This is correct
but worth noting — the architecture demands it.

> east
```

---

## Chapter III: The Type Checker's Tribunal

```
╔══════════════════════════════════════════╗
║     THE TYPE CHECKER'S TRIBUNAL          ║
║   Where all must prove their worth       ║
╚══════════════════════════════════════════╝

You enter a grand courtroom. Three judges sit upon a raised
dais: REGISTRATION, SIGNATURE, and BODY CHECKING.

But before they look at you, something extraordinary happens.

  ⚡ THE PRELUDE AVALANCHE ⚡

A vast scroll — 10,322 bytes! — is unrolled before the court.
The PRELUDE. It contains 9 functions, 39 traits, 46 expressions.
It must be processed BEFORE you can be judged.

The Lexer is summoned AGAIN:
  10,322 bytes → 1,530 tokens (53× your token count!)
  125 comments consumed.

The Parser descends into it:
  1,530 tokens → 9 functions, 39 traits, 46 expressions
  ⚠️ errors=1 — a parse error in the standard library!
  (The kingdom's foundation has a crack...)

The Type Checker processes it all:
  registration passes complete (functions=9, tests=0, impls=0)
  signature collection complete (functions=9, tests=0, impls=0)
  body checking complete (functions=9, tests=0, impls=0)

> wait
You wait. And wait. The Prelude processing takes 53× the work
that YOUR file requires. You are 29 bytes. The Prelude is 10,322.
You are an ant waiting for an elephant to pass through a doorway.

Finally, the judges turn to you.

JUDGE REGISTRATION:
  "One function: @main. No tests. No impls. Registered."
  registration passes complete (functions=1, tests=0, impls=0)

JUDGE SIGNATURE:
  "Parameters: none. Return type: int. Signature recorded."
  signature collection complete (functions=1, tests=0, impls=0)

JUDGE BODY CHECKING:
  She peers at you — Int(42).
  "You are an integer literal. Your type is int. Obviously."
  She looks at the Block containing you.
  "The block's tail is int. The function returns int. Match."
  body checking complete (functions=1, tests=0, impls=0)

> examine type
TYPE: int
No inference was needed. You are self-evident.

HP: ██████████ 100%   FORM: Typed AST node
```

---

## Chapter IV: The Canonicalizer's Forge

```
╔══════════════════════════════════════════╗
║     THE CANONICALIZER'S FORGE            ║
║   Where AST is reforged into Canon IR    ║
╚══════════════════════════════════════════╝

You descend into a hot, glowing forge. The Canonicalizer — a
smith of intermediate representations — takes your AST form
and reforges you into CANONICAL form.

  canon lower_module started
    functions=1, tests=0, impls=0, source_exprs=2

The smith examines ExprId(1) — the Block:
  "Block with empty statements, tail pointing to ExprId(0)..."
  → CanExpr::Block(CanRange(0..0), CanId(0))

Then ExprId(0) — you:
  "Integer literal 42..."
  → CanExpr::Int(42)

  canon lower_module complete
    canon_nodes=2, roots=1, method_roots=0
    constants=6, decision_trees=0

> examine constants
6 constants in the pool — but only 1 is yours!
The other 5 are prelude constants: compile-time folded values
from standard library functions. Shared infrastructure cost.

The smith also processes the PRELUDE separately:
  canon lower_module started (functions=9, source_exprs=46)
  canon lower_module complete (canon_nodes=46, roots=9,
    constants=6, decision_trees=4)

> look
You are CanId(0): CanExpr::Int(42).
Above you, CanId(1) is a Block that contains you.
Root 0 points to CanId(1) — you are reachable.

The forge has TWO exits. A sign reads:

  ← WEST: The Path of Eval (The Interpreter's Garden)
  → EAST: The Path of Iron (The LLVM Foundry)

"Choose wisely," the smith says. "Both lead to the answer.
But the journeys are... very different."

> west
```

---

## Chapter V: The Path of Eval (The Interpreter's Garden)

```
╔══════════════════════════════════════════════════╗
║   THE INTERPRETER'S GARDEN                       ║
║   Stage 6a: Where values bloom from expressions  ║
╚══════════════════════════════════════════════════╝

You step into a lush garden. The Interpreter — a gentle figure
in a green cloak — greets you.

But first, she must prepare the garden.

  "Registering prelude..." she murmurs.

  str, int, float, byte, Error, repeat, hash_combine,
  thread_id, Less, Equal, Greater, format variants...

  ⚠️ THE DOUBLE PROCESSING CURSE ⚠️

  You watch in horror as the Prelude is type-checked AND
  canonicalized A SECOND TIME. The same 10,322 bytes.
  The same 1,530 tokens. The same 9 functions.
  AGAIN.

  type checking path=prelude.ori  [during register_prelude]
    registration passes complete (functions=9)
    signature collection complete (functions=9)
    body checking complete (functions=9)
    canon lower_module complete (canon_nodes=46)

  The garden shudders. This is an ancient curse — Salsa should
  be caching this, but something prevents the memo from being
  found the second time.

Finally, the Interpreter turns to you.

> eval
The Interpreter raises her staff. Two incantations:

  eval_can(CanId(1)) → Block(CanRange(0..0), CanId(0))
    "A block. Empty statements. Evaluate the tail..."

    eval_can(CanId(0)) → Int(42)
      "An integer literal. Value is..."

    → Value::Int(42)

  "Done."

> examine self
You are now Value::Int(42). A runtime value. Alive.
The function @main returns you. The process exits with code 42.

  EXIT CODE: 42 ✓

> stats
EVAL PATH STATISTICS:
  eval_can calls:  2  (Block, then Int)
  binary ops:      0
  function calls:  0
  allocations:     0

OVERHEAD (Prelude):
  Lexing:      10,322 bytes → 1,530 tokens
  Parsing:     1,530 tokens → 46 exprs, 39 traits
  Type check:  3 passes × 9 functions (×2 = DOUBLE!)
  Canon:       46 nodes (×2 = DOUBLE!)

YOUR ACTUAL WORK:
  Lexing:      29 bytes → 14 tokens
  Parsing:     14 tokens → 2 expressions
  Type check:  3 passes × 1 function
  Canon:       2 nodes
  Eval:        2 calls

Prelude-to-you ratio: ~356:1 (by token count, double-processed)
```

---

## Chapter VI: The Path of Iron (The LLVM Foundry)

```
You return to the fork and take the EASTERN path.

╔══════════════════════════════════════════════════╗
║   THE LLVM FOUNDRY                               ║
║   Stage 6b: Where code is forged into iron       ║
╚══════════════════════════════════════════════════╝

The air grows hot. You descend into a cavernous foundry lit
by molten metal. The LLVM Codegen Master — a towering figure
of gears and fire — examines you.

> look
You are in the LLVM FOUNDRY. The walls are lined with 98
RUNTIME DECLARATIONS — spectral functions etched into the
stone, waiting to be called:

  ori_print, ori_print_int, ori_print_float, ori_print_bool,
  ori_panic, ori_panic_cstr, ori_run_main, ori_assert,
  ori_assert_eq_int, ori_assert_eq_bool, ori_assert_eq_float,
  ori_assert_eq_str, ori_list_alloc_data, ori_list_free_data,
  ori_list_new, ori_list_free, ori_list_len, ori_list_push,
  ori_list_take...

  ...and 79 more.

You stare at the 98 declarations. You are a single integer
literal. You need NONE of these.

  ⚠️ THE ARMORY OF THE UNUSED
  98 runtime function declarations. For `ret i64 42`.
  Like bringing an army to deliver a letter.

> examine types
The Codegen Master registers 6 prelude types:
  Ordering enum (3 variants: Less, Equal, Greater)
  FormatType enum
  FormatSpec struct (6 fields)
  PanicInfo struct (4 fields)
  Alignment enum (3 variants)
  Sign enum (3 variants)

None of these types appear in your code.

> examine arc
The ARC/BORROW ANALYZER awakens:
  "Computing SCC decomposition..."
  created ArcModuleInput (function_count=1)
  computing SCC decomposition (function_count=1) → 1 SCC
  Salsa borrow inference complete (sig_count=1, scc_count=1)

It finds: nothing. No references. No allocations. No ARC.
But the full analysis pipeline ran anyway — like deploying
a bomb squad to inspect an empty room.

> forge

The Codegen Master raises his hammer. CLANG!

He forges TWO functions:

  ┌─────────────────────────────────────────┐
  │  define i64 @_ori_main() {              │
  │  bb0:                                   │
  │    ret i64 42                            │
  │  }                                      │
  └─────────────────────────────────────────┘

  That's YOU. A single basic block. A single instruction.
  ret i64 42. The purest possible form.

  ┌─────────────────────────────────────────┐
  │  define i32 @main() {                   │
  │  entry:                                 │
  │    %ori_main_result = call i64          │
  │                       @_ori_main()      │
  │    %exit_code = trunc i64               │
  │                 %ori_main_result to i32 │
  │    ret i32 %exit_code                   │
  │  }                                      │
  └─────────────────────────────────────────┘

  The C main() wrapper. It calls _ori_main(), truncates the
  i64 result to i32 (because POSIX exit codes are 32-bit),
  and returns.

> examine ir
FINAL LLVM IR MODULE: "journey1"
  Declarations:  98  (runtime functions, none called)
  Definitions:    2  (_ori_main + main wrapper)
  Basic blocks:   2  (one per function)
  Instructions:   3  (ret, call, trunc+ret)
  Attributes:     3  groups (cold, nounwind, memory)

The ratio of declarations to actual instructions is 32:1.

> compile

The Codegen Master feeds the IR into the LLVM optimization
pipeline. The JIT compiler produces native x86-64 machine code.

  Compiled in 0.15s

The machine code is cached at ~/.cache/ori/journey1.

> execute

  EXIT CODE: 42 ✓

The same answer. Both paths agree.
```

---

## Chapter VII: The Reckoning

```
╔══════════════════════════════════════════════════╗
║   THE HALL OF RECKONING                          ║
║   Where the journey's truths are weighed         ║
╚══════════════════════════════════════════════════╝

You stand in a vast hall between two doorways — the garden
exit (Eval) and the foundry exit (LLVM). Both bear the same
inscription:

              EXIT CODE: 42 ✓

The Oracle of Diagnostics appears and reads the ledger:

> review findings
```

### CRITICAL Findings

```
⚔️ FINDING #1: THE DOUBLE PROCESSING CURSE
  Severity: CRITICAL
  Location: Eval path — prelude processing

  The Prelude (10,322 bytes) is fully lexed, parsed,
  type-checked, and canonicalized TWICE during a single
  compilation. Once for the type checker's import resolution,
  and again for the evaluator's function registration.

  Salsa's memoization should prevent this — but doesn't.
  This is the #1 performance issue for small files.

  Impact: 2× prelude overhead = ~20,000 bytes processed
  for a 29-byte program.
```

### HIGH Findings

```
🔥 FINDING #2: THE PRELUDE AVALANCHE
  Severity: HIGH
  Location: All paths — prelude loading

  A 29-byte program triggers 10,322 bytes of prelude
  processing. Token ratio: 1,530 vs 14 = 109:1.
  First-compilation latency is dominated by prelude.

🔥 FINDING #3: THE CRACKED FOUNDATION
  Severity: HIGH
  Location: Parser — prelude

  The prelude parse shows errors=1. A parse error in the
  standard library prelude should NEVER happen. This crack
  in the foundation may cause subtle issues downstream.
```

### MEDIUM Findings

```
⚡ FINDING #4: THE ARMORY OF THE UNUSED
  Severity: MEDIUM
  Location: LLVM codegen — runtime declarations

  98 runtime function declarations are emitted for a program
  that uses NONE of them. The actual code is `ret i64 42`.
  Lazy declaration would reduce module size and link time.

⚡ FINDING #5: THE EMPTY ROOM INSPECTION
  Severity: MEDIUM
  Location: LLVM codegen — ARC/borrow analysis

  Full SCC decomposition + borrow inference runs for @main
  even though it returns a literal. A fast-path check
  (no heap types in signature or body) could skip this.

⚡ FINDING #6: UNNECESSARY TYPE REGISTRATION
  Severity: MEDIUM
  Location: LLVM codegen — type registration

  6 prelude types (Ordering, FormatSpec, PanicInfo, etc.)
  are registered even when unused. Lazy registration would
  reduce codegen startup.
```

### LOW Findings

```
📝 FINDING #7: THE MANDATORY WRAPPER
  Severity: LOW
  Location: Parser — block wrapping

  Every function body creates Block + content, even for
  single expressions. Block([], Int(42)) could be collapsed
  to Int(42) in canon IR via constant folding.

📝 FINDING #8: PHANTOM CONSTANTS
  Severity: LOW
  Location: Canonicalizer — constant pool

  6 constants exist for a 1-literal program. These are
  prelude-originated. Not harmful, but documents shared cost.
```

---

## Epilogue

```
╔══════════════════════════════════════════════════╗
║                 QUEST COMPLETE                   ║
╠══════════════════════════════════════════════════╣
║                                                  ║
║  You are 42.                                     ║
║                                                  ║
║  You began as two ASCII runes on a 29-byte       ║
║  scroll. The Raw Scanner measured you. The        ║
║  Token Cooker named you. The Parser gave you      ║
║  a home in the Arena. The Type Checker blessed    ║
║  you as int. The Canonicalizer reforged you.      ║
║                                                  ║
║  On the Eval path, you became Value::Int(42)     ║
║  in just 2 incantations — but waited while       ║
║  the Prelude was processed TWICE.                ║
║                                                  ║
║  On the LLVM path, you became `ret i64 42` —     ║
║  a single machine instruction — but were          ║
║  surrounded by 98 unused declarations and         ║
║  6 phantom type registrations.                   ║
║                                                  ║
║  Both paths returned EXIT CODE: 42.              ║
║  The answer was always the same.                 ║
║  The journey was the interesting part.            ║
║                                                  ║
╠══════════════════════════════════════════════════╣
║  FINDINGS: 1 CRITICAL | 2 HIGH | 3 MEDIUM       ║
║            2 LOW                                 ║
║  BEHAVIORAL MISMATCH: None (both paths = 42)     ║
╚══════════════════════════════════════════════════╝

> quit
Thank you for playing THE QUEST OF FORTY-TWO.
Your adventure has been saved to the Ledger of Journeys.
```

---

## Appendix: The Adventurer's Ledger

| Metric | Eval Path | LLVM Path |
|--------|-----------|-----------|
| Exit code | 42 | 42 |
| Prelude tokens processed | 1,530 (×2!) | 1,530 |
| User tokens processed | 14 | 14 |
| Canon nodes (user) | 2 | 2 |
| Canon nodes (prelude) | 46 (×2!) | 46 |
| Runtime declarations | N/A | 98 |
| Type registrations | N/A | 6 |
| ARC analysis | N/A | 1 SCC (trivial) |
| User functions compiled | 1 | 2 (+ main wrapper) |
| Total instructions | 2 eval_can calls | 3 LLVM instructions |
| Compile time | instant | 0.15s |
