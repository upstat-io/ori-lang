---
section: "05"
title: "Register-Based VM"
status: not-started
reviewed: false
goal: "Execute bytecode in a tight dispatch loop with a contiguous register file — target ≤0.5µs/call on the measured dev machine, with Python 3.12 as the stretch comparison"
inspired_by:
  - "Lua 5.4 VM (lvm.c — register-based dispatch, single C function)"
  - "CPython 3.12 ceval (Python/ceval.c — computed goto dispatch)"
  - "Roc interpreter (src/eval/interpreter.zig — flat instruction dispatch)"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "VM Core and Dispatch Loop"
    status: not-started
  - id: "05.2"
    title: "Call Frames and Scoping"
    status: not-started
  - id: "05.3"
    title: "Method Dispatch and Built-in Integration"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Register-Based VM

**Status:** Not Started
**Goal:** A bytecode execution engine that runs the instruction set from Section 04 in a tight loop. Concrete target: ≤0.5µs per function call on the measured dev machine, with Python 3.12 performance treated as the stretch comparison rather than the only acceptable number. A(3,8) should complete in under 1 second on the measured host (Python: 0.386s).

**Context:** The bytecode compiler (Section 04) flattens the recursive CanExpr tree into linear instructions. This section builds the engine that executes those instructions. The critical insight: a well-written dispatch loop spends almost all its time in a single function — the VM loop. No recursive calls, no arena lookups, no pattern matching on expression kinds. Just fetch → decode → execute → repeat.

The dispatch loop is the single most performance-critical piece of code in the entire compiler. Every nanosecond per dispatch multiplied by millions of instructions.

**Reference implementations:**
- **Lua 5.4** `lvm.c`: Single ~3000-line C function `luaV_execute()` with a `switch` on opcodes. Register-based — operands are register indices, not stack positions. The gold standard for interpreter performance.
- **CPython 3.12** `Python/ceval.c`: `_PyEval_EvalFrameDefault()` with computed goto dispatch (`USE_COMPUTED_GOTOS`). Stack-based but with specializing adaptive interpreter. Our measured target: 0.14µs/call.
- **Roc** `src/eval/interpreter.zig`: Flat dispatch over instruction tags. Arena-threaded operands.

**Depends on:** Section 04 (bytecode compilation — provides the `Chunk` and `OpCode` types).

---

## 05.1 VM Core and Dispatch Loop

**File(s):** `compiler/ori_eval/src/bytecode/vm.rs` (new)

The heart of the interpreter. A single function that loops over bytecode instructions and dispatches each one. Every allocation, branch miss, or cache miss in this loop costs millions of cycles over a program's execution.

- [ ] **Implement `VM` struct**:
  ```rust
  pub struct VM {
      /// The register file — contiguous pre-allocated storage of Values.
      /// Each call frame gets a window into this array.
      registers: Vec<Value>,
      /// Call frame stack (pre-allocated, never cloned).
      frames: Vec<CallFrame>,
      /// Current frame pointer (index into `frames`).
      frame_count: usize,
      /// Global variables.
      globals: FxHashMap<Name, Value>,
      /// Open upvalues (for closure capture).
      open_upvalues: Vec<UpvalueRef>,
  }

  struct CallFrame {
      /// The chunk being executed.
      /// SAFETY: raw pointer avoids lifetime parameter on CallFrame.
      /// Invariant: chunk outlives the frame (enforced by VM::execute taking &Chunk).
      /// Consider a safe alternative: store an index into a chunk table instead.
      chunk: *const Chunk,  // borrowed, not owned
      /// Instruction pointer (index into chunk.code).
      ip: usize,
      /// Base register offset in the register file.
      base: usize,
      /// Call metadata for backtraces and runtime diagnostics.
      call_name: Name,
      call_span: Option<Span>,
  }
  ```

- [ ] **Implement the dispatch loop** — the most critical code:
  ```rust
  impl VM {
      pub fn execute(&mut self, chunk: &Chunk) -> EvalResult {
          let mut ip: usize = 0;
          let code = &chunk.code;
          let constants = &chunk.constants;
          let base = self.current_base();

          loop {
              let instr = code[ip];
              ip += 1;
              let op = OpCode::from_byte((instr & 0xFF) as u8);
              let a = ((instr >> 8) & 0xFF) as usize + base;
              let b = ((instr >> 16) & 0xFF) as usize + base;
              let c = ((instr >> 24) & 0xFF) as usize;

              match op {
                  OpCode::LoadConst => {
                      // ABx format: [opcode:8][A:8][Bx:16]
                      // A = destination register, Bx = constant pool index
                      let dest = ((instr >> 8) & 0xFF) as usize + base;
                      let bx = ((instr >> 16) & 0xFFFF) as usize;
                      self.registers[dest] = constants[bx].clone();
                  }
                  OpCode::AddInt => {
                      let lhs = self.registers[b].as_int();
                      let rhs = self.registers[c].as_int();
                      self.registers[a] = Value::Int(lhs + rhs);
                  }
                  OpCode::Call => {
                      // Push new frame, adjust base, continue
                      // ... (see 05.2)
                  }
                  OpCode::Return => {
                      let result = std::mem::replace(
                          &mut self.registers[a],
                          Value::Void,
                      );
                      // Pop frame, restore ip and base
                      self.frame_count -= 1;
                      if self.frame_count == 0 {
                          return Ok(result);
                      }
                      let frame = &self.frames[self.frame_count - 1];
                      ip = frame.ip;
                      // Place result in caller's destination register
                      // ...
                  }
                  OpCode::TailCall => {
                      // Reuse current frame — no push/pop
                      // Reset ip to 0, rebind parameters in-place
                  }
                  // ... all other opcodes
              }
          }
      }
  }
  ```

- [ ] **Value representation**: The VM uses the same `Value` enum as the tree-walker. This is critical for compatibility — built-in functions, method dispatch, and the pattern system all operate on `Value`. Do NOT invent a separate VM-internal value type. The register file is `Vec<Value>`. This means `Value::clone()` costs (Arc bumps for heap-backed values) still apply, but the elimination of per-call Environment/CallStack allocation is the dominant win.
- [ ] **Dispatch strategy**: Start with `match` (Rust's switch). Profile. If branch prediction is a bottleneck, evaluate:
  - **Computed goto** via unsafe function pointer table (Rust equivalent of C's `&&label` extension)
  - **Direct threading** via `#[cold]`/`#[inline(always)]` hints on rare/common arms
  - Measure: if `match` dispatch costs >2ns/instruction, switch to function pointer table
- [ ] **Register file sizing**: Pre-allocate `Vec<Value>` with capacity for max expected nesting. Typical: 256 registers × 64 max frames = 16,384 slots. Grows only on deep recursion.
- [ ] **Instruction decode**: The 32-bit decode (shifts + masks) should compile to 3-4 x86 instructions. Verify with `cargo asm` or `disasm-ori.sh`.
- [ ] **Benchmark the bare dispatch loop**: Measure cycles per instruction dispatch with a no-op program (tight loop of `Move` instructions). Target: <5ns per dispatch. Use `cargo asm -p ori_eval --lib -- "VM::execute"` to verify the hot loop compiles to a tight switch with no unexpected function calls.
- [ ] **Function lookup architecture**: The VM must resolve `Value::Function(FunctionValue)` to compiled `Chunk`s. The `FunctionValue` carries a `CanId` (body reference) and `SharedCanonResult` (the canon IR). The bytecode compiler produces a `Chunk` per function; the VM needs a way to map `FunctionValue` -> `Chunk` at call time. Options: (a) eagerly compile all functions at module load time, store `Chunk`s in a lookup table keyed by `CanId`; (b) lazily compile on first call, cache in the VM; (c) store the `Chunk` directly in a new `Value::BytecodeFunction` variant. Option (a) is simplest and avoids runtime compilation overhead.
- [ ] **Carry a separate control-flow stack if needed**: The current evaluator uses `ControlAction` (4 variants: `Error`, `Break(Value)`, `Continue(Value)`, `Propagate(Value)`) as Rust `Result::Err` to unwind the Rust call stack. In the bytecode VM there is no Rust recursion to unwind. The VM must implement:
  - **Break/Continue with labels**: `CanExpr::Break { label, value }` and `CanExpr::Continue { label, value }` can target named loops (`loop:name`, `for:name`). The bytecode compiler must resolve label targets to jump offsets. For `break:name` targeting an outer loop, the VM must unwind any inner loops between the break site and the target loop.
  - **Break with value**: `break value` exits a `for...yield` loop, appending a final value to the collection. The VM must handle this at the yield collection level.
  - **Continue with value for yield**: `continue value` in `for...yield` substitutes a value. The VM must pipe this through the yield accumulator.
  - **Propagate across frames**: `?` operator emits `Propagate(Value)` which crosses function call boundaries (caught by `catch_propagation()` at the caller). In the VM, this means popping the current frame and checking the caller's propagation handler.
  - **Error with backtrace**: `Error(Box<EvalError>)` carries backtrace data. The VM must be able to capture frame information for backtraces when errors occur.

- [ ] **Safety decision for `*const Chunk` in `CallFrame`**: The sample design uses a raw pointer to avoid lifetime parameters on `CallFrame`. Evaluate alternatives: (a) chunk index into a `Vec<Chunk>` (safe, one indirection), (b) `&'a Chunk` with lifetime on `VM<'a>` (safe, constrains VM lifetime), (c) raw pointer with `// SAFETY:` invariant (performance, requires unsafe). The chunk table index (option a) is recommended for correctness-first development; raw pointer can be introduced later if profiling shows the indirection matters.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/bytecode/vm/tests.rs`):
  - **Dimensions**: opcode category (load/store, arithmetic, comparison, control flow, call/return) x value type (int, float, bool, str, void) x register count (few: 3, moderate: 20, pressure: 250)
  - **Semantic pin**: `LoadConst(r0, 42)` followed by `Return(r0)` produces `Value::Int(42)`
  - **Negative pin**: executing past end of chunk code produces clean error (not UB or panic)
  - **TDD ordering**: implement VM struct and dispatch loop skeleton FIRST, write tests for LoadConst/Return/Move, verify they fail (VM doesn't execute yet), then implement opcode handlers incrementally

---

## 05.2 Call Frames and Scoping

**File(s):** `compiler/ori_eval/src/bytecode/vm.rs`

Function calls in the bytecode VM are fundamentally different from the tree-walker: no environment allocation, no interpreter construction, no cloning. A call pushes a frame (3 words), adjusts the register window, and jumps to ip=0 of the callee's chunk.

- [ ] **Implement `Call` opcode**:
  ```rust
  OpCode::Call => {
      let func_reg = b;
      let arg_start = c;
      let arg_count = /* encoded in instruction or next word */;

      let callee = &self.registers[func_reg];
      let chunk = /* extract compiled chunk from Value::Function / MemoizedFunction */;

      // Save current frame state
      self.frames[self.frame_count - 1].ip = ip;

      // Push new frame
      let new_base = func_reg + 1; // args already in registers after func
      self.frames[self.frame_count] = CallFrame {
          chunk: chunk as *const Chunk,
          ip: 0,
          base: new_base,
      };
      self.frame_count += 1;

      // Switch to callee
      ip = 0;
      // base is now new_base (handled by frame lookup)
  }
  ```

- [ ] **Implement `TailCall` opcode**: Reuse the current frame. Move arguments into the current frame's parameter slots, reset ip to 0. This is how Ackermann's outer recursive call becomes a loop — zero stack growth.
  ```rust
  OpCode::TailCall => {
      // Move args into current frame's parameter slots
      let frame = &mut self.frames[self.frame_count - 1];
      // Copy arg registers to param registers
      // Reset ip to 0
      ip = 0;
      // No frame push — same frame, same base
  }
  ```
  **Caveat**: Ackermann uses multi-clause dispatch (`@ack (0: int, n: int)`, `@ack (m: int, 0: int)`, `@ack (m: int, n: int)`). In the current evaluator, multi-clause dispatch compiles to a `DecisionTree` evaluated at each call entry. For tail calls to work, the VM must re-enter the decision tree at ip=0, not just the body of the current clause. This means TailCall targets the function entry point (including pattern matching), not the clause body. The outer call `ack(m: m - 1, n: ack(m:, n: n - 1))` is NOT a tail call (inner `ack` must complete first), but `ack(m: m - 1, n: 1)` in clause 2 IS a tail call.

- [ ] **Implement `Return` opcode**: Pop frame, place result in caller's destination register, restore ip.
- [ ] **Implement scoping for `let` bindings**: Local variables are registers. `let x = expr` compiles to instructions that place the result in a specific register. No scope push/pop — register allocation handles it at compile time.
- [ ] **Implement closure upvalue access**: `GetUpvalue`/`SetUpvalue` access the closure's captured values. These are stored in the closure object, not in a scope chain.
- [ ] **Stack overflow protection**: Check `frame_count < MAX_FRAMES` on every `Call`. If exceeded, return a clean error (not a Rust stack overflow).
- [ ] **Matrix tests** (in `compiler/ori_eval/src/bytecode/vm/tests.rs`):
  - **Dimensions**: call type (regular, tail, mutual recursion, closure-to-closure) x depth (shallow: 5, medium: 100, deep: 1000+, overflow: max_frames+1) x return path (normal, error propagation, break across frames)
  - **Semantic pin**: Ackermann A(3,5) = 253 via bytecode VM (identical to tree-walker result)
  - **Semantic pin (tail call)**: tail-recursive countdown(1000000) completes with frame_count never exceeding 2 (verified by instrumentation)
  - **Negative pin**: `Call` when `frame_count >= MAX_FRAMES` returns `StackOverflow` error (not Rust panic)
  - **TDD ordering**: write Call/Return/TailCall tests FIRST, verify they fail, then implement frame management

---

## 05.3 Method Dispatch and Built-in Integration

**File(s):** `compiler/ori_eval/src/bytecode/vm.rs`, `compiler/ori_eval/src/bytecode/builtins.rs` (new)

The VM must integrate with Ori's existing method dispatch system (MethodDispatcher with priority chain: UserRegistry → CollectionMethod → BuiltinMethod) and built-in functions (print, assert_eq, etc.).

- [ ] **Built-in function calls**: `FunctionVal` values (registered in `function_val.rs`) are called via a fast path — direct Rust function pointer invocation, no bytecode dispatch:
  ```rust
  OpCode::Call => {
      match &self.registers[func_reg] {
          Value::FunctionVal(func_ptr, _) => {
              // Direct call to Rust function — no frame push
              let args = &self.registers[arg_start..arg_start + arg_count];
              let result = func_ptr(args)?;
              self.registers[dest] = result;
          }
          Value::Function(_) | Value::MemoizedFunction(_) => {
              // Bytecode call — push frame (as in 05.2)
          }
      }
  }
  ```

- [ ] **Method dispatch integration**: `MethodCall` opcode must preserve the full current dispatch chain, which is more complex than just the three-resolver priority chain. The actual dispatch order in `eval_method_call()` is:
  1. Print methods (pre-interned Name comparison for `print`/`println`)
  2. Associated functions on `TypeRef` receivers (user registry + derived + builtin)
  3. Callable struct field access (struct field that is a function value)
  4. MethodDispatcher chain: UserRegistry (priority 0) -> Collection (priority 1) -> Builtin (priority 2)
  
  Additionally, `dispatch_method_call()` (called from `can_eval.rs`) handles `ModuleNamespace` receivers before even calling `eval_method_call()`.
  
  ```rust
  OpCode::MethodCall => {
      let receiver = &self.registers[b];
      let method_name = constants[c].as_name();
      // Must route through the full dispatch chain, not a simplified version
      let result = self.eval_method_call(receiver.clone(), method_name, args)?;
      self.registers[a] = result;
  }
  ```

- [ ] **Keep the tree-walker path as the semantic oracle during transition**: first route complex method/iterator/protocol behavior through existing helpers, then inline or specialize only after parity is proven.
- [ ] **TPR checkpoint after 05.2**: Run `/tpr-review` after call frames and scoping are implemented. This validates the core VM execution model (dispatch + frames) before adding the complexity of method dispatch and built-in integration in 05.3. All findings must be resolved before proceeding.

- [ ] **Iterator protocol**: `ForPrepare`/`ForLoop` opcodes must work with Ori's `IteratorValue` system. The iterator state lives in a register; `ForLoop` calls `next()` and branches on exhaustion.
- [ ] **Error handling**: `Propagate` opcode implements `?` — check if value is Err/None, return early if so. This must interact correctly with the frame stack (pop frames on propagation).
- [ ] **Capability propagation**: Capabilities (`uses Random`, etc.) are looked up by name. The VM needs access to the current capability bindings. Store in a side table indexed by frame, or pass through the global scope.
- [ ] **Pattern matching in `match` expressions**: if Section 04 compiles `DecisionTree` directly, the VM must support the emitted branch/binding structure rather than assuming a simple `TestTag` chain. `DecisionTree` has 4 variants: `Switch` (branch on test kind — variant tag, literal equality, type test), `Leaf` (bind variables via `ScrutineePath` and execute arm body), `Guard` (evaluate guard expression, fall through to `on_fail` if false), and `Fail` (unreachable).
- [ ] **`FunctionExp` dispatch**: `CanExpr::FunctionExp` handles 15 special forms (`Recurse`, `Parallel`, `Spawn`, `Timeout`, `Cache`, `With`, `Print`, `Panic`, `Catch`, `Todo`, `Unreachable`, `Channel`, `ChannelIn`, `ChannelOut`, `ChannelAll`). These are currently dispatched inline in `can_eval.rs`. The VM should route these to helper functions rather than implementing each as dedicated opcodes. The `Recurse` pattern in particular has complex semantics (tail call optimization, memoization, base/step functions, parallel mode) that should remain in Rust helper code.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/bytecode/vm/tests.rs` and spec tests):
  - **Dimensions**: dispatch target (FunctionVal builtin, user method, derived method, collection method, builtin method) x receiver type (int, str, [int], {str: int}, struct, Option, Result) x FunctionExp kind (Print, Panic, Catch, Recurse -- the 4 most common of the 15 variants)
  - **Semantic pin**: `[1, 2, 3].map(x -> x * 2)` via VM produces `[2, 4, 6]` (identical to tree-walker)
  - **Semantic pin**: `catch(expr: { panic(msg: "boom") })` via VM produces `Err("boom")` (identical to tree-walker)
  - **Negative pin**: calling a non-function value via `Call` opcode produces `not_callable` error (same error as tree-walker)
  - **TDD ordering**: write builtin call tests FIRST (print, assert_eq), then method dispatch tests, then iterator tests, then FunctionExp tests

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] Ackermann A(3,8) = 2045 completes in **<1 second** (Python: 0.386s, Ori current: timeout)
- [ ] `cargo bench -p oric --bench interpreter -- ackermann_3_5` shows per-call cost **≤0.5µs** (down from 63µs)
- [ ] TailCall optimization verified: tail-recursive function uses constant stack depth (measured via frame_count)
- [ ] All built-in functions accessible from bytecode (print, assert_eq, panic, etc.)
- [ ] Method dispatch works for all collection types
- [ ] Iterator protocol works (for...yield produces correct results)
- [ ] Error propagation (?) works across bytecode frames
- [ ] VM standalone tests pass: comprehensive Rust unit/integration tests covering all opcodes, all CanExpr variants, control flow, closures, iterators, method dispatch, and error handling
- [ ] All existing Rust tests pass: `cargo t` green (tree-walker unaffected)
- [ ] `./test-all.sh` green (tree-walker path unchanged; VM tested via its own test suite)
- [ ] **Note**: Full spec test parity via `cargo st` requires Section 07 runtime integration across both entry points:
  - `ori run` / direct evaluation (`evaluated()` / `run_evaluation()`)
  - `ori test` interpreter backend (`TestRunner` + `Evaluator::load_module(...)`)
- [ ] Stack overflow produces clean error, not Rust panic
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 05` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** Ackermann A(3,8) completes in under 1 second via the bytecode VM in isolation (standalone VM test, not yet through Salsa pipeline). Per-call cost measured at ≤0.5µs by Criterion benchmarks. All built-in functions and method dispatch work. The VM is a complete, tested execution engine ready for Salsa integration in Section 07. Full Salsa pipeline integration and dual-execution parity testing happen in Sections 07 and 06 respectively.
