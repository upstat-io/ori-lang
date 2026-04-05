---
section: "04"
title: "Bytecode Compilation"
status: not-started
reviewed: false
goal: "Compile CanExpr IR to a register-based bytecode instruction set — eliminate recursive tree-walking dispatch"
inspired_by:
  - "Lua 5.4 register-based bytecode (lopcodes.h, lcode.c)"
  - "CPython 3.12 bytecode (Python/ceval.c, compile.c)"
  - "Roc interpreter (src/eval/interpreter.zig — flat dispatch)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Instruction Set Design"
    status: not-started
  - id: "04.2"
    title: "Bytecode Compiler"
    status: not-started
  - id: "04.3"
    title: "Constant Pool and Closures"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Bytecode Compilation

**Status:** Not Started
**Goal:** Compile `CanExpr` (canonical IR) into a flat bytecode representation that a tight dispatch loop can execute without recursive function calls. This eliminates the fundamental overhead of tree-walking: recursive dispatch, branch prediction misses, and pointer chasing through arena-allocated expression nodes.

**Context:** The current interpreter dispatches via `eval_can(can_id)` which arena-indexes into `CanExpr`, pattern-matches on the expression kind, and recursively calls `eval_can` for sub-expressions. Each dispatch is a function call through the Rust call stack — at minimum 3-5ns for the call/ret alone, plus branch prediction misses on the `match` (~5-10ns per miss). For deeply nested expressions, this compounds.

Bytecode compilation flattens the recursion: the compiler walks the CanExpr tree once (at compile time) and emits a linear sequence of instructions. The VM then executes these sequentially in a tight loop — no recursion, no arena indexing, no pattern matching on expression kinds.

**Reference implementations:**
- **Lua 5.4** `lopcodes.h`: 82 register-based opcodes, 3 instruction formats (ABC, ABx, AsBx), 32-bit fixed-width instructions
- **CPython 3.12** `Python/compile.c`: Stack-based bytecode compiler, ~8,000 lines. Compiles AST → bytecode with constant folding and peephole optimization
- **Roc** interpreter: Flat instruction dispatch with arena-threaded operands — similar concept, different encoding

**Depends on:** Section 01. Sections 02-03 can inform bytecode data layout, but the compiler should not wait on them.

---

## 04.1 Instruction Set Design

**File(s):** `compiler/ori_eval/src/bytecode/mod.rs` (new), `compiler/ori_eval/src/bytecode/opcode.rs` (new)

Design a register-based instruction set for Ori. Register-based (like Lua) rather than stack-based (like CPython) because:
1. Fewer instructions per operation (no dup/swap/rot)
2. Better maps to Ori's expression-based semantics (every expression produces a value in a register)
3. Simpler optimization (register allocation is explicit)

- [ ] **Design the instruction encoding**:
  ```rust
  /// 32-bit instruction: [opcode:8][A:8][B:8][C:8] or [opcode:8][A:8][Bx:16]
  #[repr(u8)]
  pub enum OpCode {
      // Load/Store
      LoadConst,      // A = constants[Bx]
      LoadNil,        // A = Void
      LoadTrue,       // A = true
      LoadFalse,      // A = false
      Move,           // A = B (register copy)

      // Arithmetic (A = B op C)
      AddInt, SubInt, MulInt, DivInt, ModInt, PowInt,
      AddFloat, SubFloat, MulFloat, DivFloat,
      Negate,         // A = -B

      // Comparison (A = B cmp C → bool)
      EqInt, NeqInt, LtInt, LeInt, GtInt, GeInt,
      EqFloat, NeqFloat, LtFloat, LeFloat,
      EqStr, EqBool,

      // String
      Concat,         // A = B + C (string concat)
      Template,       // A = template string (args in B..B+C)

      // Control flow
      Jump,           // ip += sBx (signed offset)
      JumpIfFalse,    // if !A then ip += sBx
      JumpIfTrue,     // if A then ip += sBx

      // Function calls
      Call,           // A = call B with args C..C+N
      Return,         // return A
      TailCall,       // tail-call B with args C..C+N

      // Closures
      Closure,        // A = new closure from prototype Bx
      GetUpvalue,     // A = upvalues[B]
      SetUpvalue,     // upvalues[B] = A

      // Scope
      GetLocal,       // A = locals[B] (for nested scope access)
      SetLocal,       // locals[B] = A

      // Pattern matching
      TestTag,        // A = (B.tag == C) → bool
      GetField,       // A = B.field[C]
      GetIndex,       // A = B[C]
      SetIndex,       // A[B] = C

      // Collections
      NewList,        // A = new list with B..B+C elements
      NewMap,         // A = new map with B..B+C key-value pairs
      NewTuple,       // A = new tuple with B..B+C elements
      NewStruct,      // A = new struct (type in constants[Bx])

      // Variants
      NewVariant,     // A = variant (type+variant in constants, fields in B..B+C)
      MatchVariant,   // A = (B is variant C) → bool
      Unwrap,         // A = unwrap B (Option/Result)

      // Method dispatch
      MethodCall,     // A = B.method(C..C+N) — method name in constants

      // Iterator
      ForPrepare,     // A = iterator from B
      ForLoop,        // A = next(B); if done jump sBx

      // Error handling
      Propagate,      // if B is Err/None, return B (? operator)
  }
  ```

- [ ] **Do not hard-commit to ~80 opcodes on day one**: start with a smaller semantic core ISA (roughly 25-40 opcodes) that can express every `CanExpr` (48 variants) and `DecisionTree` (4 variants) behavior. Add type-specialized fast paths only for operations that profiling proves are hot enough to justify opcode surface area, decoder complexity, and matrix-test cost. Note: the current `CanExpr` uses `CanRange { start: u32, len: u16 }` for variable-length children (args, list elements, etc.) — the bytecode compiler must decide how to encode these (inline count in instruction, or separate extended operand word).

- [ ] **Design the `Chunk` (compiled function) representation**:
  ```rust
  pub struct Chunk {
      code: Vec<u32>,              // Bytecode instructions
      constants: Vec<Value>,       // Constant pool
      prototypes: Vec<Chunk>,      // Nested function prototypes (closures)
      param_count: u8,             // Number of parameters
      register_count: u8,          // Max registers needed
      upvalue_count: u8,           // Number of captured variables
      source_map: Vec<(u32, Span)>,// Instruction index → source span (for errors)
  }
  ```

- [ ] **Document instruction encoding format**: Fixed 32-bit width, ABC or ABx format. A=destination register, B/C=source registers or constant indices.
- [ ] **Handle `CanBindingPattern` destructuring in `Let`**: `CanExpr::Let` uses `CanBindingPatternId` which can be: `Name` (simple), `Tuple` (multi-register), `Struct` (field extraction), `List` (head/rest), or `Wildcard` (discard). The bytecode compiler must emit different instruction sequences for each. Simple `Name` is one register assignment; `Tuple { (a, b) = expr }` needs `NewTuple` unwrap into N registers; `Struct { { x, y } = expr }` needs field extraction; `List { [h, ..t] = expr }` needs head/tail split.
- [ ] **Handle `DecisionTree` compilation for `Match`**: `CanExpr::Match` carries a `DecisionTreeId` referencing a pre-compiled `DecisionTree` with 4 variants: `Switch { path, test_kind, edges, default }`, `Leaf { arm_index, bindings }`, `Guard { arm_index, bindings, guard, on_fail }`, and `Fail`. The bytecode compiler must walk this tree structure, emitting `TestTag`/comparison + conditional jumps for Switch, binding loads for Leaf, guard evaluation + fallthrough for Guard. This is the most complex compilation target and should have dedicated tests.
- [ ] **Validate ISA completeness**: Walk every `CanExpr` variant (48 total, defined in `compiler/ori_ir/src/canon/expr.rs`) and every `CanBindingPattern` variant (5 total, defined in `compiler/ori_ir/src/canon/patterns.rs`) and verify each maps to one or more opcodes. No gaps. Create an exhaustiveness test that iterates all variants and asserts a mapping exists.
- [ ] **Address existing `decision_tree` TODOs**: `compiler/ori_eval/src/exec/decision_tree/mod.rs` has 2 TODOs referencing "section-07" (lines 266, 276) about `test_kind` usage and numeric discriminants. These are forward references that must be resolved during bytecode compilation -- the bytecode compiler must decide how to handle `TestKind` and discriminant-based dispatch. Track these as dependencies for 04.3.

---

## 04.2 Bytecode Compiler

**File(s):** `compiler/ori_eval/src/bytecode/compiler.rs` (new)

Compile `CanExpr` trees into `Chunk` bytecode. This is a single-pass tree walk that emits instructions and manages register allocation.

- [ ] **Implement `BytecodeCompiler`**:
  ```rust
  pub struct BytecodeCompiler<'a> {
      chunk: Chunk,
      arena: &'a CanArena,
      /// Next free register
      next_reg: u8,
      /// Local variable → register mapping
      locals: Vec<(Name, u8)>,
      /// Scope depth for local cleanup
      scope_depth: u32,
  }

  impl<'a> BytecodeCompiler<'a> {
      pub fn compile(arena: &'a CanArena, body: CanId) -> Chunk { ... }

      /// Compile an expression, placing result in `dest` register
      fn compile_expr(&mut self, expr: CanId, dest: u8) { ... }

      /// Allocate a temporary register
      fn alloc_reg(&mut self) -> u8 { ... }

      /// Free a temporary register
      fn free_reg(&mut self, reg: u8) { ... }
  }
  ```

- [ ] **Implement compilation for each CanExpr variant** (prioritized by frequency in spec tests):
  1. **Literals**: `Int`, `Float`, `Bool`, `Str`, `Char`, `Duration`, `Size`, `Unit` → `LoadConst` / `LoadTrue` / `LoadFalse` / `LoadNil`. Also `Constant` (ConstantPool index).
  2. **References**: `Ident`, `Const` → `Move` from local register. `SelfRef` → `Move` from self register. `FunctionRef` → load from function table. `TypeRef` → load type handle. `HashLength` → load collection length.
  3. **Operators**: `Binary { op, left, right }` → type-specialized opcodes per `BinaryOp`. `Unary { op, operand }` → `Negate`/`Not`/etc. `Cast` → conversion opcodes.
  4. **Bindings**: `Let { pattern, init, mutable }` → compile initializer, assign register(s) to name(s). `CanBindingPatternId` may involve destructuring (tuple, struct, list patterns). `Assign` → update register or heap.
  5. **Blocks**: `Block { stmts, result }` → compile statements sequentially, last expr is result register.
  6. **Control flow**: `If` → `JumpIfFalse` + branch compilation. `Match` → compile `DecisionTree` (Switch/Leaf/Guard/Fail). `For` → `ForPrepare`/`ForLoop` + body + yield collection. `Loop` → unconditional loop. `Break`/`Continue` with label resolution.
  7. **Calls**: `Call { func, args: CanRange }` → evaluate args into registers, `Call`. `MethodCall` → receiver + method name from constant pool.
  8. **Collections**: `List(CanRange)` → `NewList`. `Tuple(CanRange)` → `NewTuple`. `Map(CanMapEntryRange)` → `NewMap`. `Struct { name, fields: CanFieldRange }` → `NewStruct`. `Range` → range construction.
  9. **Algebraic**: `Ok`/`Err`/`Some`/`None` → wrap/unwrap opcodes.
  10. **Error handling**: `Try` (`?`) → `Propagate`. `Await` → await opcode (route through existing helper function, not a bespoke opcode).
  11. **Functions**: `Lambda { params, body }` → `Closure` with prototype index + upvalue capture.
  12. **Access**: `Field { receiver, field }` → `GetField`. `Index { receiver, index }` → `GetIndex`.
  13. **Special forms**: `FunctionExp` → route to existing helper functions (Recurse/Parallel/Print/Panic/etc.). `FormatWith` → format helper call. `WithCapability` → capability scope management. `Unsafe` → transparent (compile inner expr).
  14. **Error**: `Error` → unreachable (should not appear in valid programs).

- [ ] **Inventory the entire semantic surface, not just common expression shapes** (48 CanExpr variants total):
  - **Literals (8)**: `Int`, `Float`, `Bool`, `Str`, `Char`, `Duration`, `Size`, `Unit`
  - **Constants (1)**: `Constant` (index into `ConstantPool`)
  - **References (6)**: `Ident`, `Const`, `SelfRef`, `FunctionRef`, `TypeRef`, `HashLength`
  - **Operators (3)**: `Binary` (with `BinaryOp` — 16+ arithmetic/comparison/logic ops), `Unary` (with `UnaryOp`), `Cast` (infallible/fallible)
  - **Calls (2)**: `Call`, `MethodCall`
  - **Access (2)**: `Field`, `Index`
  - **Control flow (6)**: `If`, `Match` (carries compiled `DecisionTree` — compile that tree directly), `For` (with label/pattern/guard/yield), `Loop`, `Break` (with label/value), `Continue` (with label/value)
  - **Bindings (3)**: `Block`, `Let` (with `CanBindingPatternId`), `Assign`
  - **Functions (1)**: `Lambda` (with `CanParamRange`)
  - **Collections (5)**: `List`, `Tuple`, `Map`, `Struct`, `Range`
  - **Algebraic (4)**: `Ok`, `Err`, `Some`, `None`
  - **Error handling (2)**: `Try` (`?` operator), `Await`
  - **Safety (1)**: `Unsafe`
  - **Capabilities (1)**: `WithCapability`
  - **Special forms (2)**: `FunctionExp` (15 FunctionExpKind variants: Recurse/Parallel/Spawn/Timeout/Cache/With/Print/Panic/Catch/Todo/Unreachable/Channel/ChannelIn/ChannelOut/ChannelAll), `FormatWith`
  - **Error recovery (1)**: `Error`
  - Record which constructs must remain calls into existing helper paths during the first VM cut instead of forcing bespoke opcodes immediately (likely: `FunctionExp`, `WithCapability`, `Match` with complex `DecisionTree`, `FormatWith`)

- [ ] **Register allocation strategy**: Linear scan — each expression gets a register, freed when no longer needed. Parameters occupy registers 0..N-1. Temporaries start at N.
- [ ] **Constant folding**: Evaluate constant expressions at compile time (e.g., `2 + 3` → `LoadConst 5`). This is the existing const eval path.
- [ ] **Crate placement decision**: The bytecode module lives inside `ori_eval` (not a new crate) because it consumes `CanExpr`/`CanArena` from `ori_ir` and produces `Value` types from `ori_eval`. It is a new execution backend for the same evaluator crate. Files: `compiler/ori_eval/src/bytecode/{mod.rs, opcode.rs, compiler.rs, chunk.rs}`.
- [ ] **TPR checkpoint after 04.2**: Run `/tpr-review` after the bytecode compiler handles all 48 `CanExpr` variants. This catches ISA design issues before 04.3 (closures/multi-clause) compounds complexity. All findings must be resolved before proceeding.
- [ ] **Design `Chunk` so Section 07 can cache it without reworking the IR**: give it `Clone + Debug` and a stable, explicit layout. Do **not** block Section 04 on whether chunks live in a Salsa query or a side cache; that cache-boundary decision belongs to Section 07 once the runtime integration seam is implemented.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/bytecode/compiler/tests.rs`):
  - **Dimensions**: CanExpr category (literals, references, operators, calls, control flow, bindings, functions, collections, algebraic, error handling, special forms) x encoding format (ABC vs ABx) x register pressure (1 reg, 5 regs, max regs)
  - **Semantic pin**: compile `let $x = 2 + 3` and verify it emits `LoadConst(r0, 2), LoadConst(r1, 3), AddInt(r2, r0, r1)` (or equivalent after constant folding: `LoadConst(r0, 5)`)
  - **Negative pin**: compile `CanExpr::Error` and verify it produces an unreachable/error opcode, not a valid instruction sequence
  - **TDD ordering**: design ISA types FIRST (opcode enum, Chunk struct), write compilation tests for literals and simple expressions, verify they fail (compiler doesn't exist), then implement compiler incrementally by CanExpr category

---

## 04.3 Constant Pool and Closures

**File(s):** `compiler/ori_eval/src/bytecode/compiler.rs`, `compiler/ori_eval/src/bytecode/closure.rs` (new)

Handle the tricky parts: constant deduplication, closure compilation with upvalue capture, and multi-clause function dispatch.

- [ ] **Constant pool deduplication**: deduplicate canonical constants (`ConstValue` / `ConstantId`) rather than hashing arbitrary runtime `Value`s. The compiler should not depend on full runtime `Value` equality for semantic constants.
- [ ] **Closure compilation**: When compiling a lambda/closure:
  1. Compile the body as a nested `Chunk` (prototype)
  2. Identify free variables → these become upvalues
  3. Emit `Closure` instruction that creates the closure object with upvalue references
  4. Upvalue access via `GetUpvalue`/`SetUpvalue` in the inner function
- [ ] **Closure metadata must preserve current evaluator behavior**: default parameters, captured capabilities, memoized-function wrapping, and any source/backtrace data needed by diagnostics cannot be implicit "future work".
- [ ] **Multi-clause functions and `match` lower through the existing decision-tree contract**:
  1. Compile `DecisionTree::Switch { path: ScrutineePath, test_kind: TestKind, edges, default }` — emit scrutinee path navigation (field access, index, variant tag extraction via `ScrutineePath`), then a test chain (compare scrutinee against each edge's `TestValue`), then conditional jumps to subtree code
  2. Compile `DecisionTree::Leaf { arm_index, bindings: Vec<(Name, ScrutineePath)> }` — emit path-based binding loads (navigate scrutinee via `ScrutineePath` to extract each bound variable into a register), then jump to the arm body
  3. Compile `DecisionTree::Guard { arm_index, bindings, guard: CanId, on_fail }` — emit bindings, evaluate guard expression, `JumpIfFalse` to `on_fail` subtree code. Preserve guard fallback semantics exactly (fall through to remaining compatible arms, not just next sequential arm)
  4. Compile `DecisionTree::Fail` — emit `unreachable` (exhaustiveness guarantees this never executes)
  5. Only after the above is working, consider specialized opcodes for hot literal tests
- [ ] **Tail call detection**: When a `Call` is the last instruction before `Return`, emit `TailCall` instead — this enables the VM to reuse the current frame.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/bytecode/compiler/tests.rs`):
  - **Dimensions**: closure capture count (0, 1, 5) x nesting (flat, nested) x DecisionTree variant (Switch with literals, Leaf with bindings, Guard with fallthrough, Fail) x tail position (tail call, non-tail call)
  - **Semantic pin**: multi-clause Ackermann compiles to a DecisionTree-driven branch chain that matches the tree structure from `compiler/ori_ir/src/canon/tree/mod.rs`
  - **Negative pin**: non-tail call (inner `ack(m:, n: n - 1)` in the third clause) does NOT emit TailCall opcode
  - **TDD ordering**: write closure compilation tests and DecisionTree compilation tests FIRST, verify they fail, then implement

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All `CanExpr` variants have corresponding bytecode compilation
- [ ] `BytecodeCompiler::compile()` produces valid `Chunk` for all spec test programs
- [ ] Constant pool deduplication works (same value → same index)
- [ ] Closure compilation produces correct upvalue references
- [ ] Multi-clause dispatch compiles to branch chain
- [ ] Tail calls detected and emitted as `TailCall`
- [ ] Source maps preserved (every instruction traces to a source span)
- [ ] `./test-all.sh` green (tree-walker still used for execution — bytecode compilation is verified but not yet executed)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 04` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `BytecodeCompiler::compile()` successfully compiles all 5,800+ spec test programs to bytecode without errors. The bytecode is not yet executed (that's Section 05) — this section proves the compilation is complete and correct by round-trip verification: compile to bytecode, disassemble, verify instruction sequence matches expected output for representative programs (Ackermann, FizzBuzz, closures, pattern matching).
