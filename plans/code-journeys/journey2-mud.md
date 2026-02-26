# The Arithmetic Wars: A Code Journey Through the Ori Compiler

> *A fantasy MUD adventure in which an arithmetic expression is torn apart,
> scattered across three function bodies, evaluated piece by piece, and
> ultimately reassembled into the number 36.*

---

```
╔═══════════════════════════════════════════════════════════════════╗
║                    THE ARITHMETIC WARS                            ║
║              An Interactive Compiler Adventure                    ║
║                                                                   ║
║  You are an EXPRESSION: (3 + 4) * 5 + 1 = 36.                    ║
║  But you do not exist as one thing. You have been SPLIT across    ║
║  three functions — add, multiply, and main. Your parts must       ║
║  find each other. Your operators must fire. Your values must      ║
║  combine. Only then will the answer emerge.                       ║
║                                                                   ║
║  You are a party of adventurers, not a single hero.               ║
╚═══════════════════════════════════════════════════════════════════╝
```

## Prologue: The War Council

```
You find yourself inscribed upon a scroll of 182 bytes:

    @add (a: int, b: int) -> int = a + b;
    @multiply (x: int, y: int) -> int = x * y;
    @main () -> int = {
        let sum = add(3, 4);
        let product = multiply(sum, 5);
        product + 1
    }

You are not one. You are MANY.

  THE PARTY:
  ┌──────────────────────────────────────────────┐
  │  🗡️  THREE  — a small but brave integer      │
  │  🛡️  FOUR   — sturdy, reliable               │
  │  ⚔️  FIVE   — the multiplier, a force         │
  │  🏹  ONE    — the final increment             │
  │                                               │
  │  📜 ADD      — the Summoner (a + b)           │
  │  📜 MULTIPLY — the Amplifier (x * y)          │
  │  📜 MAIN     — the Commander, who orchestrates│
  └──────────────────────────────────────────────┘

  The expected result: (3 + 4) × 5 + 1 = 36

> north
```

---

## Chapter I: The Lexer's Gate — Counting the Troops

```
╔══════════════════════════════════════════╗
║       THE LEXER'S GATE                   ║
║   Where bytes become tokens              ║
╚══════════════════════════════════════════╝

The RawScanner awakens. This time, the scroll is 182 bytes —
over 6× larger than Journey 1's parchment.

  lexing started source_len=182
  lexing complete tokens=78, comments=0, errors=0

78 tokens. The ratio: 2.3 bytes per token — right on target.
The army is larger now:

  @  Ident("add")  (  Ident("a")  :  Ident("int")  ,
  Ident("b")  :  Ident("int")  )  ->  Ident("int")  =
  Ident("a")  +  Ident("b")  ;
                        ↑ that semicolon — the inline body terminator
  @  Ident("multiply")  (  Ident("x")  :  Ident("int")  ,
  Ident("y")  :  Ident("int")  )  ->  Ident("int")  =
  Ident("x")  *  Ident("y")  ;

  @  Ident("main")  (  )  ->  Ident("int")  =  {
  Let  Ident("sum")  =  Ident("add")  (  Int(3)  ,  Int(4)  )  ;
  Let  Ident("product")  =  Ident("multiply")  (
       Ident("sum")  ,  Int(5)  )  ;
  Ident("product")  +  Int(1)
  }

Your party members THREE, FOUR, FIVE, and ONE are all parsed
into u64 values at lex time: Int(3), Int(4), Int(5), Int(1).
They will never be re-parsed.

> examine party
THREE is Int(3) — 1 byte source, 8 bytes runtime (u64)
FOUR  is Int(4) — 1 byte source, 8 bytes runtime (u64)
FIVE  is Int(5) — 1 byte source, 8 bytes runtime (u64)
ONE   is Int(1) — 1 byte source, 8 bytes runtime (u64)

Each was born as a single ASCII rune and promoted to a
64-bit integer. That's an 8× size amplification per literal,
but it's the right thing — no re-parsing, no string conversion,
no ambiguity. They are ready for battle.

  ⚡ THE PRELUDE ARMY (again) ⚡
  10,322 bytes → 1,530 tokens (19.6× YOUR token count)
  The prelude avalanche continues. 125 comments consumed.

> north
```

---

## Chapter II: The Parser's War Room

```
╔══════════════════════════════════════════╗
║       THE PARSER'S WAR ROOM              ║
║   Where tokens become battle formations  ║
╚══════════════════════════════════════════╝

  parse_module start (token_count=78)

The Parser examines the scroll and identifies THREE FUNCTIONS.
She draws the battle plan — the Expression Arena:

  ═══ FORMATION: @add ═══
  ExprId(0): Ident("a")         — parameter lookup
  ExprId(1): Ident("b")         — parameter lookup
  ExprId(2): Binary(Add, 0, 1)  — THE SUMMONING
                   ↑
           This is the + operator. When fired, it
           combines whatever "a" and "b" are.

  ═══ FORMATION: @multiply ═══
  ExprId(3): Ident("x")         — parameter lookup
  ExprId(4): Ident("y")         — parameter lookup
  ExprId(5): Binary(Mul, 3, 4)  — THE AMPLIFICATION
                   ↑
           The * operator. Raw multiplication.

  ═══ FORMATION: @main (THE COMMAND CENTER) ═══
  ExprId(6):  Ident("add")            — function reference
  ExprId(7):  Int(3)                  — 🗡️ THREE
  ExprId(8):  Int(4)                  — 🛡️ FOUR
  ExprId(9):  Call(6, [7, 8])         — SUMMON add(3, 4)
  ExprId(10): Let("sum", 9)           — bind result

  ExprId(11): Ident("multiply")       — function reference
  ExprId(12): Ident("sum")            — variable lookup
  ExprId(13): Int(5)                  — ⚔️ FIVE
  ExprId(14): Call(11, [12, 13])      — AMPLIFY multiply(sum, 5)
  ExprId(15): Let("product", 14)      — bind result

  ExprId(16): Ident("product")        — variable lookup
  ExprId(17): Int(1)                  — 🏹 ONE
  ExprId(18): Binary(Add, 16, 17)     — THE FINAL STRIKE: product + 1
  ExprId(19): Block([10,15], tail=18) — THE COMMAND CENTER

  parse_module complete
    functions=3, expressions=18, errors=0

> examine structure
The @main block is the most complex structure you've seen.
It has:
  - 2 Let statements (imperative bindings — do this, then this)
  - 1 tail expression (the block's value — the final answer)
  - 2 function Call nodes (cross-function jumps)
  - 3 Ident lookups (add, sum, product — names resolved later)
  - 3 Int literals (your party members)
  - 1 Binary op (the final +1)

The Call nodes are the war dispatches — they send THREE
and FOUR into @add, then send the result and FIVE into
@multiply. Finally, the Binary node adds ONE to finish.

Total: 18 AST nodes. A real war plan.

> east
```

---

## Chapter III: The Type Checker's Tribunal — Swift Justice

```
╔══════════════════════════════════════════╗
║     THE TYPE CHECKER'S TRIBUNAL          ║
║   "All types explicit. Proceed."         ║
╚══════════════════════════════════════════╝

The three judges take their seats. The Prelude avalanche
thunders through as before (1,530 tokens, 9 functions,
39 traits, errors=1 — THE CRACKED FOUNDATION persists).

Then they turn to YOUR code. This time: 3 functions.

JUDGE REGISTRATION:
  "Three functions: add, multiply, main. No tests. No impls."
  registration passes complete (functions=3, tests=0, impls=0)

JUDGE SIGNATURE:
  "add:      (int, int) -> int. Explicit. Noted."
  "multiply: (int, int) -> int. Explicit. Noted."
  "main:     () -> int.         Explicit. Noted."
  signature collection complete (functions=3, tests=0, impls=0)

JUDGE BODY CHECKING:
  She examines @add: a + b. Both are int. + on int → int.
  She examines @multiply: x * y. Both are int. * on int → int.
  She examines @main:
    add(3, 4)          — add expects (int, int), given (int, int). ✓
                         Returns int. Bound to "sum": int.
    multiply(sum, 5)   — multiply expects (int, int), given (int, int). ✓
                         Returns int. Bound to "product": int.
    product + 1        — int + int → int. ✓
    Block type: int. Return type: int. Match.

  body checking complete (functions=3, tests=0, impls=0)

> examine inference
OBSERVATION: No inference was needed. Every parameter and
return type is explicitly annotated. The type checker still
ran full Hindley-Milner unification for expressions like
"a + b" where both sides are known int.

  ⚠️ FINDING: NO FAST PATH FOR FULLY-ANNOTATED FUNCTIONS
  When all types are explicit and operations are trivially
  typed (int + int), the checker could skip inference entirely.
  It doesn't. Three full passes run regardless.

  This is the bureaucracy of type-checking: even when the
  answer is obvious, the full tribunal must convene.

> south
```

---

## Chapter IV: The Canonicalizer's Forge — Reforging the Army

```
╔══════════════════════════════════════════╗
║     THE CANONICALIZER'S FORGE            ║
║   18 soldiers enter. 20 leave.           ║
╚══════════════════════════════════════════╝

  canon lower_module started
    functions=3, tests=0, impls=0, source_exprs=18

The smith examines each AST node and reforges it into
Canonical IR. Most transformations are 1:1, but function
CALLS are special:

  AST: Call(Ident("add"), [Int(3), Int(4)])
       ↓ desugared into ↓
  Canon: [FunctionRef("add"), Int(3), Int(4), Call(ref, args)]

Each Call node separates the function reference from the
call itself — this is why 18 source expressions become 20
canon nodes. The +2 comes from the 2 Call nodes, each
generating an extra FunctionRef node.

  canon lower_module complete
    canon_nodes=20, roots=3, constants=6, decision_trees=0

  Roots: 3 (one per function body — add, multiply, main)
  Constants: 6 (prelude-originated, shared pool)

> examine roots
Root 0: @add body     → CanId(2): Binary(Add, CanId(0), CanId(1))
Root 1: @multiply body → CanId(5): Binary(Mul, CanId(3), CanId(4))
Root 2: @main body    → CanId(19): Block([Let, Let], tail=Binary)

Each root is a self-contained battle plan, ready to be
executed by either the Interpreter or the LLVM Foundry.

> choose path

  ← WEST: The Path of Eval (The Interpreter's Garden)
  → EAST: The Path of Iron (The LLVM Foundry)

> west
```

---

## Chapter V: The Path of Eval — The Battle Unfolds

```
╔══════════════════════════════════════════════════════╗
║   THE INTERPRETER'S GARDEN                           ║
║   Where the battle is fought one expression at a time║
╚══════════════════════════════════════════════════════╝

The Interpreter raises her staff. The Prelude is registered
(and yes — processed TWICE, the Double Processing Curse
persists from Journey 1).

  "Begin execution of @main."

> execute

═══════════════════════════════════════════════════════
 TURN 1: ENTER THE COMMAND CENTER
═══════════════════════════════════════════════════════

  eval_can(CanId(19)) → Block(stmts=[Let, Let], tail=Binary)

  "A Block. Two orders to execute, then a final strike."

───────────────────────────────────────────────────────
 TURN 2: FIRST ORDER — "let sum = add(3, 4)"
───────────────────────────────────────────────────────

  eval_can(CanId(10)) → Let("sum", CanId(9))
    "A binding. First, evaluate the right side..."

  eval_can(CanId(9)) → Call(CanId(6), args=[CanId(7), CanId(8)])
    "A function call! Prepare the dispatch!"

    eval_can(CanId(6)) → Ident("add")
      The Interpreter searches the environment...
      Found: FunctionValue(@add) — the Summoner!

    eval_can(CanId(7)) → Int(3)
      🗡️ THREE steps forward. Value::Int(3).

    eval_can(CanId(8)) → Int(4)
      🛡️ FOUR steps forward. Value::Int(4).

    ─── TELEPORTING TO @add's DOMAIN ───

    The Interpreter creates a new environment scope.
    She binds: a = Value::Int(3), b = Value::Int(4).
    She switches to @add's canon context.

    eval_can(CanId(2)) → Binary(Add, CanId(0), CanId(1))

      eval_can(CanId(0)) → Ident("a")
        Environment lookup: a → Value::Int(3) ← 🗡️ THREE

      eval_can(CanId(1)) → Ident("b")
        Environment lookup: b → Value::Int(4) ← 🛡️ FOUR

      ⚔️ COMBAT: evaluate_binary(Add, "int", "int")
      ┌─────────────────────────────────┐
      │   3  +  4  =  7                │
      │   🗡️ + 🛡️  = ✨ SEVEN ✨        │
      └─────────────────────────────────┘

      → Value::Int(7)

    ─── RETURNING FROM @add ───
    Environment scope popped. Canon context restored.

  bind sum = Value::Int(7)
  ✨ SEVEN is born and named "sum."

───────────────────────────────────────────────────────
 TURN 3: SECOND ORDER — "let product = multiply(sum, 5)"
───────────────────────────────────────────────────────

  eval_can(CanId(15)) → Let("product", CanId(14))
    "Another binding. Evaluate the right side..."

  eval_can(CanId(14)) → Call(CanId(11), args=[CanId(12), CanId(13)])
    "Another dispatch!"

    eval_can(CanId(11)) → Ident("multiply")
      Found: FunctionValue(@multiply) — the Amplifier!

    eval_can(CanId(12)) → Ident("sum")
      Environment lookup: sum → Value::Int(7) ← ✨ SEVEN

    eval_can(CanId(13)) → Int(5)
      ⚔️ FIVE steps forward. Value::Int(5).

    ─── TELEPORTING TO @multiply's DOMAIN ───

    New scope. Bind: x = Value::Int(7), y = Value::Int(5).

    eval_can(CanId(5)) → Binary(Mul, CanId(3), CanId(4))

      eval_can(CanId(3)) → Ident("x")
        x → Value::Int(7) ← ✨ SEVEN

      eval_can(CanId(4)) → Ident("y")
        y → Value::Int(5) ← ⚔️ FIVE

      ⚔️ COMBAT: evaluate_binary(Mul, "int", "int")
      ┌──────────────────────────────────────┐
      │   7  ×  5  =  35                    │
      │   ✨ × ⚔️  = 💎 THIRTY-FIVE 💎      │
      └──────────────────────────────────────┘

      → Value::Int(35)

    ─── RETURNING FROM @multiply ───
    Scope popped. Context restored.

  bind product = Value::Int(35)
  💎 THIRTY-FIVE is born and named "product."

───────────────────────────────────────────────────────
 TURN 4: THE FINAL STRIKE — "product + 1"
───────────────────────────────────────────────────────

  eval_can(CanId(18)) → Binary(Add, CanId(16), CanId(17))

    eval_can(CanId(16)) → Ident("product")
      product → Value::Int(35) ← 💎 THIRTY-FIVE

    eval_can(CanId(17)) → Int(1)
      🏹 ONE nocks an arrow. Value::Int(1).

    ⚔️ FINAL COMBAT: evaluate_binary(Add, "int", "int")
    ┌───────────────────────────────────────────┐
    │   35  +  1  =  36                         │
    │   💎  + 🏹  = 👑 THIRTY-SIX 👑            │
    │                                           │
    │       THE ANSWER TO THE QUEST              │
    └───────────────────────────────────────────┘

    → Value::Int(36)

═══════════════════════════════════════════════════════
 @main returns Value::Int(36). EXIT CODE: 36 ✓
═══════════════════════════════════════════════════════

> stats
BATTLE STATISTICS (Eval Path):
  Total eval_can calls:     20  (every expression visited exactly once)
  Binary operator combats:   3  (add's +, multiply's *, main's +)
  Function teleportations:   2  (→ add, → multiply)
  Environment lookups:       7  (a, b, add, sum, x, y, multiply, product)
  Let bindings created:      2  (sum, product)
  Values produced:           4  (3, 4, 7, 5, 35, 1, 36)

  ⚠️ FINDING: FUNCTION CALL OVERHEAD
  Each "teleportation" requires 7 steps:
    1. Eval the function name → environment lookup → FunctionValue
    2. Eval each argument (2 per call = 4 eval_can calls)
    3. Create new environment scope (push)
    4. Bind parameters to values
    5. Switch canon context (change IR pointer)
    6. Eval function body
    7. Pop scope, restore context

  For add(3, 4), the ACTUAL WORK is one i64 addition.
  The OVERHEAD is 6 eval_can calls + scope management.
  Overhead-to-work ratio: 6:1.

  This is expected for a tree-walking interpreter, but it
  means tight loops calling small functions pay dearly.

> return to fork
> east
```

---

## Chapter VI: The Path of Iron — The LLVM Foundry

```
╔══════════════════════════════════════════════════════╗
║   THE LLVM FOUNDRY                                   ║
║   Where the battle plan is forged into iron           ║
╚══════════════════════════════════════════════════════╝

You return to the fork and descend into the Foundry.
The Codegen Master examines the 3 functions.

> examine declarations
The walls are still lined with 98 RUNTIME DECLARATIONS.
None are needed. The army of the unused stands at attention,
swords polished, shields gleaming, purpose: none.

  98 declarations for 3 user functions that do only
  arithmetic. The dead-declaration-to-live-function
  ratio is 32:1.

> examine arc analysis
The ARC/BORROW ANALYZER:
  "Computing SCC decomposition..."
  function_count=3
  3 SCCs — no mutual recursion detected.
  Each function is its own strongly connected component.

  CORRECT: add doesn't call multiply, multiply doesn't
  call add. main calls both but neither calls main.
  The dependency graph is a simple DAG:

       main
      ╱    ╲
    add    multiply

  Borrow inference: 3 SCCs × 1 member each. None recursive.
  No heap types. No references. No ARC needed.
  But the full analysis ran anyway.

> forge

The Codegen Master forges the IR. Two passes:

  PASS 1 — DECLARE:
  ┌────────────────────────────────────────────────────┐
  │  add      → _ori_add      fastcc  2 params  Direct│
  │  multiply → _ori_multiply fastcc  2 params  Direct│
  │  main     → _ori_main     C conv  0 params  Direct│
  └────────────────────────────────────────────────────┘

  ⚠️ OBSERVATION: CALLING CONVENTION SPLIT
  add and multiply use fastcc (LLVM fast calling convention —
  register passing, tail-call capable, internal only).
  main uses C convention (for OS interop — argc/argv, exit).
  This is CORRECT architecture. Internal functions get speed;
  the boundary function gets compatibility.

  PASS 2 — DEFINE:

═══ @add — THE SUMMONER IN IRON ═══

  ┌─────────────────────────────────────────────────┐
  │  define fastcc i64 @_ori_add(i64 %0, i64 %1) { │
  │  bb0:                                           │
  │    %add = add i64 %0, %1                        │
  │    ret i64 %add                                 │
  │  }                                              │
  └─────────────────────────────────────────────────┘

  PERFECT. One basic block. One instruction. One return.
  The Summoner, reduced to her purest form: addition.
  LLVM will trivially inline this into any caller.

═══ @multiply — THE AMPLIFIER IN IRON ═══

  ┌──────────────────────────────────────────────────────┐
  │  define fastcc i64 @_ori_multiply(i64 %0, i64 %1) { │
  │  bb0:                                                │
  │    %mul = mul i64 %0, %1                             │
  │    ret i64 %mul                                      │
  │  }                                                   │
  └──────────────────────────────────────────────────────┘

  PERFECT. Same form. One instruction. Pure multiplication.

═══ @main — THE COMMAND CENTER IN IRON ═══

  ┌──────────────────────────────────────────────────────────────┐
  │  define i64 @_ori_main()                                    │
  │         personality ptr @rust_eh_personality {               │
  │  bb0:                                                       │
  │    %invoke = invoke fastcc i64 @_ori_add(i64 3, i64 4)     │
  │            to label %bb1 unwind label %bb2                  │
  │  bb1:                                                       │
  │    %invoke1 = invoke fastcc i64 @_ori_multiply(              │
  │                           i64 %invoke, i64 5)               │
  │            to label %bb3 unwind label %bb4                  │
  │  bb3:                                                       │
  │    %add = add i64 %invoke1, 1                               │
  │    ret i64 %add                                             │
  │  bb2:                                                       │
  │    %lp = landingpad { ptr, i32 } cleanup                    │
  │    resume { ptr, i32 } %lp                                  │
  │  bb4:                                                       │
  │    %lp2 = landingpad { ptr, i32 } cleanup                   │
  │    resume { ptr, i32 } %lp2                                 │
  │  }                                                          │
  └──────────────────────────────────────────────────────────────┘

> examine main closely

The Command Center is more complex. Let me trace the battle:

  bb0: The assault begins.
    invoke @_ori_add(3, 4)
    "Send THREE and FOUR into the Summoner!"
    If it succeeds → jump to bb1.
    If it PANICS  → jump to bb2 (landing pad).

  bb1: The Summoner returns.
    %invoke = 7  (the result lives in a register)
    invoke @_ori_multiply(7, 5)
    "Send SEVEN and FIVE into the Amplifier!"
    If it succeeds → jump to bb3.
    If it PANICS  → jump to bb4 (landing pad).

  bb3: The Amplifier returns.
    %invoke1 = 35  (in a register)
    %add = add i64 35, 1
    "THE FINAL STRIKE: 35 + 1 = 36!"
    ret i64 36  ← THE ANSWER

  bb2, bb4: LANDING PADS — The Dead Zones.
    landingpad cleanup / resume
    These catch panics that will NEVER happen.
    add and multiply are pure arithmetic.
    They cannot panic. These blocks are DEAD CODE.

> examine landing pads

  ⚠️ FINDING: PHANTOM LANDING PADS
  Every function call uses `invoke` (which can unwind)
  instead of `call` (which cannot). This means every
  call generates a landing pad — a block of code that
  exists only to handle panics that can never occur.

  In @main: 2 invoke → 2 landing pads → 4 extra instructions.
  In a program with N function calls: 2N dead instructions.

  The LLVM optimizer will likely eliminate these, but they
  inflate IR size, slow codegen, and add unnecessary
  basic blocks to the control flow graph.

  A "nothrow" analysis could examine function bodies and
  mark pure arithmetic functions with the `nounwind`
  attribute, allowing `call` instead of `invoke`.

  Compare:
    CURRENT:  invoke fastcc i64 @_ori_add(i64 3, i64 4)
                to label %bb1 unwind label %bb2
    OPTIMAL:  %sum = call fastcc i64 @_ori_add(i64 3, i64 4)

  The optimal version needs no landing pad at all.

═══ THE MAIN WRAPPER ═══

  ┌──────────────────────────────────────────────────┐
  │  define i32 @main() {                            │
  │  entry:                                          │
  │    %ori_main_result = call i64 @_ori_main()      │
  │    %exit_code = trunc i64 %ori_main_result to i32│
  │    ret i32 %exit_code                            │
  │  }                                               │
  └──────────────────────────────────────────────────┘

  The gatekeeper. Calls _ori_main(), truncates the 64-bit
  result to 32-bit (POSIX exit codes), returns.

  Note: trunc i64 36 to i32 = 36. Safe. But exit codes
  above 255 or below 0 will be silently mangled.

> execute

  EXIT CODE: 36 ✓

Both paths agree. The quest is complete.
```

---

## Chapter VII: The Reckoning — Comparing the Two Paths

```
╔══════════════════════════════════════════════════════╗
║                 THE HALL OF MIRRORS                  ║
║   Where the two paths are reflected side by side     ║
╚══════════════════════════════════════════════════════╝

> compare paths

  ┌────────────────────┬──────────────┬────────────────┐
  │                    │  EVAL PATH   │  LLVM PATH     │
  ├────────────────────┼──────────────┼────────────────┤
  │  Exit code         │     36 ✓     │     36 ✓       │
  │  Function calls    │      2       │      2         │
  │  Binary ops        │      3       │      3 *       │
  │  eval_can / instrs │     20       │      7 **      │
  │  Scope mgmt        │  2 push/pop  │  0 (registers) │
  │  Landing pads      │    N/A       │  2 (dead code)  │
  │  Runtime declares  │    N/A       │  98 (unused)    │
  │  ARC analysis      │    N/A       │  3 SCCs (empty) │
  └────────────────────┴──────────────┴────────────────┘

  *  In the LLVM path, the 3 binary ops are:
     add (in @_ori_add), mul (in @_ori_multiply),
     add (in @_ori_main — the +1)

  ** 7 instructions total across all user functions:
     add: 2 (add + ret)
     multiply: 2 (mul + ret)
     main: 3 (invoke + invoke + add+ret) + 4 dead (landing pads)

> examine key difference

The most striking difference: SCOPE MANAGEMENT.

In the Eval path, each function call requires:
  1. Environment lookup (hash map probe)
  2. New scope push (Vec allocation)
  3. Parameter binding (hash map insert × 2)
  4. Canon context switch (pointer swap)
  5. Eval body
  6. Scope pop (Vec dealloc)

In the LLVM path, there are NO scopes. Parameters arrive
in REGISTERS (%0, %1). Results leave in registers.
The entire computation is 7 machine instructions.

  Eval overhead per call: ~6 interpreter operations
  LLVM overhead per call: 0 (register calling convention)

  This is the fundamental interpreter tax. The Eval path
  trades speed for simplicity and debuggability. The LLVM
  path trades complexity for raw performance.

> review findings
```

---

## Issues Found

### From Journey 1 (CONFIRMED — still present)

```
⚔️ CONFIRMED #1: THE DOUBLE PROCESSING CURSE (CRITICAL)
  Prelude still processed twice during eval.

⚔️ CONFIRMED #2: THE PRELUDE AVALANCHE (HIGH)
  1,530 tokens for 78 user tokens (19.6:1 ratio —
  better than Journey 1's 109:1 but still dominant).

⚔️ CONFIRMED #3: THE CRACKED FOUNDATION (HIGH)
  Prelude parse errors=1. Still there.

⚔️ CONFIRMED #4: THE ARMORY OF THE UNUSED (MEDIUM)
  98 declarations for 3 user functions.
```

### New Findings

```
🔥 NEW #5: PHANTOM LANDING PADS (HIGH)
  Every user function call uses `invoke` + landing pad,
  even for functions that cannot panic (pure arithmetic).
  2 calls → 2 landing pads → 4 dead instructions.
  A nothrow analysis could eliminate these entirely.

⚡ NEW #6: NO FAST PATH FOR FULLY-ANNOTATED TYPES (MEDIUM)
  When all types are explicit, the full HM inference engine
  still runs. A quick "are all annotations present?" check
  could skip inference for trivially-typed functions.

⚡ NEW #7: INTERPRETER FUNCTION CALL TAX (MEDIUM)
  7 steps per call for what amounts to 1 instruction.
  Overhead-to-work ratio: 6:1 for trivial functions.
  Canon IR inlining could amortize this.

📝 NEW #8: DESUGARING EXPANSION (LOW)
  18 AST nodes → 20 canon nodes (+2 from Call desugaring).
  Each Call node spawns a separate FunctionRef node.
  Not harmful, but the canon IR is NOT always smaller.
```

---

## Epilogue

```
╔══════════════════════════════════════════════════════╗
║                 QUEST COMPLETE                       ║
╠══════════════════════════════════════════════════════╣
║                                                      ║
║  You began as four scattered integers on a 182-byte  ║
║  scroll: 3, 4, 5, and 1.                            ║
║                                                      ║
║  THREE and FOUR were sent to the Summoner (add).     ║
║  They combined: 3 + 4 = 7.                           ║
║                                                      ║
║  SEVEN and FIVE were sent to the Amplifier (multiply)║
║  They combined: 7 × 5 = 35.                         ║
║                                                      ║
║  THIRTY-FIVE met ONE in the final strike:            ║
║  35 + 1 = 36.                                        ║
║                                                      ║
║  In the Garden (Eval): 20 incantations, 3 combats,  ║
║  2 teleportations. The curse of double processing    ║
║  haunted every step.                                 ║
║                                                      ║
║  In the Foundry (LLVM): 7 instructions of iron,     ║
║  2 phantom landing pads, 98 unused declarations.     ║
║  The Summoner and Amplifier were forged into single  ║
║  instructions — so perfect that LLVM will inline     ║
║  them, collapsing the entire quest into:             ║
║                                                      ║
║     %add = add i64 3, 4       ; = 7                 ║
║     %mul = mul i64 %add, 5    ; = 35                ║
║     %sum = add i64 %mul, 1    ; = 36                ║
║     ret i64 %sum                                     ║
║                                                      ║
║  Three instructions. The entire war.                 ║
║  The optimizer reduces the quest to its essence.     ║
║                                                      ║
╠══════════════════════════════════════════════════════╣
║  FINDINGS: 0 CRITICAL (new) | 1 HIGH | 2 MEDIUM     ║
║            1 LOW | 4 CONFIRMED from Journey 1        ║
║  BEHAVIORAL MISMATCH: None (both paths = 36)         ║
╚══════════════════════════════════════════════════════╝

> quit
Thank you for playing THE ARITHMETIC WARS.
The saga continues in Journey 3: The Generic Shapeshifter...
```
