---
section: "09"
title: "Algebraic Normalization Pass"
status: not-started
reviewed: false
goal: "Build a step-3b ARC IR normalization pass that rewrites expressions using exported algebraic law metadata"
success_criteria:
  - "x + 0 → x for int (identity elimination)"
  - "x + '' → x for str (identity elimination, after runtime fix)"
  - "x * 1 → x for int (identity elimination)"
  - "-(-x) → x for int (involutive, double negation)"
  - "!!b → b for bool (involutive)"
  - "Commutative operands sorted to canonical order for CSE"
  - "(a + b) + c and a + (b + c) normalize to same canonical form (associativity)"
  - "x + (-x) → 0 for int with additive inverse law (inverse elimination)"
  - "Pass fires at step 3b in AIMS pipeline (before analyze_function)"
  - "Satisfies mission criterion: algebraic normalization eliminates identity ops and canonicalizes expressions"
inspired_by:
  - "Tang 2013 Ch. 5: Flow-sensitive generic rewriting with conditional rules"
  - "LLVM instcombine: canonicalize algebraic forms, move constants to RHS"
  - "LLVM reassociate: reorder commutative expressions for CSE/PRE/LICM"
  - "Codex round 4: pass runs after typeck, before ARC analysis (not after)"
depends_on: ["07", "08"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "09.1"
    title: "Normalization Pass Framework"
    status: not-started
  - id: "09.2"
    title: "Identity Elimination"
    status: not-started
  - id: "09.3"
    title: "Involutive Rewrites"
    status: not-started
  - id: "09.4"
    title: "Commutative Canonical Ordering"
    status: not-started
  - id: "09.5"
    title: "Associativity Normalization"
    status: not-started
  - id: "09.6"
    title: "Inverse Elimination"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Algebraic Normalization Pass

**Status:** Not Started
**Goal:** Build the compiler's first algebraic optimization pass — a step-3b ARC IR normalization pass that rewrites expressions using exported algebraic law metadata. This is the core of Tang's thesis: using algebraic properties of types (identity, associativity, commutativity, inverse) to perform optimizations that LLVM cannot do because the operations are opaque function calls at the LLVM level.

**Why step 3b ARC IR, not LLVM:** At the ARC IR level, primitive operations are represented as `ArcInstr::Let { value: ArcValue::PrimOp { op: PrimOp::Binary(Add), args: [x, zero] } }` — the operation type and arguments are directly visible. User-defined operator syntax also remains `PrimOp`; the exact impl identity is exported separately by Section 07.5 because the instruction itself does not carry it. After LLVM emission, these become opaque instructions or calls that LLVM cannot optimize algebraically. The normalization pass MUST fire before the ARC analysis (step 4) because it changes the variable liveness graph (removing Let/PrimOp instructions removes use-def chains).

**Critical IR distinction:** Most binary/unary operators (both built-in and user-defined) lower to `Let { value: PrimOp { op, args } }` in ARC IR. The type checker resolves user-defined operators via trait dispatch, but the AST retains `ExprKind::Binary` which canonicalizes to `CanExpr::Binary` and lowers to `PrimOp::Binary(op)`. The `Apply` path (`ArcInstr::Apply { func, args }`) is for explicit method calls (`x.add(rhs: y)`), not operator syntax (`x + y`).

**Exceptions — NOT PrimOps:** `BinaryOp::And` (`&&`), `BinaryOp::Or` (`||`), and `BinaryOp::Coalesce` (`??`) are intercepted by `lower_binary()` in `compiler/ori_arc/src/lower/expr/mod.rs` BEFORE PrimOp emission and lowered to control-flow IR (branches via `lower_short_circuit_and()`, `lower_short_circuit_or()`, `lower_coalesce()`). They NEVER appear as `PrimOp::Binary(And)` / `PrimOp::Binary(Or)` / `PrimOp::Binary(Coalesce)` in ARC IR.

This means the normalization pass has ONE dispatch path:
- **PrimOp path**: Match on `PrimOp::Binary(op)` and `PrimOp::Unary(op)` — covers ALL overloadable operators (arithmetic, comparison, bitwise, user-defined). Does NOT cover `&&`/`||`/`??` (control-flow IR).

The type of the operands (`func.var_types[dst]`) determines which laws apply. For built-in overloadable operators (int/str/list/bitwise/not), laws are installed in Section 08 on the stdlib operator impls. For user types (Matrix, Vector), laws come from user-declared `laws []` on their operator impls. The `AlgebraLawIndex` maps `(type_idx, trait_name)` to law sets — the normalization pass queries this with the operand type and the trait corresponding to the `BinaryOp`/`UnaryOp` when such a trait exists.

**Pipeline position:** After `compute_var_reprs()` (step 3) and `normalize_function()` (step 3a, TRMC), before `analyze_function()` (step 4).

**Purity gate:** For user-defined operator implementations, rewrites only fire if Section 07.5 exported an exact impl/provenance entry and the corresponding `MemoryContract` proves the operator is pure enough for the rewrite. For stdlib built-in operators, purity is encoded directly in the exported summary (`BuiltinPure`). Section 09 must not reconstruct callee names from `PrimOp`.

**Success Criteria:**

- [ ] `ORI_DUMP_AFTER_ARC=1` shows normalized ARC IR (identity ops eliminated, operands reordered)
- [ ] `x + 0` compiles to `x` for int (no Add call in ARC IR)
- [ ] `s + ""` compiles to `s` for str (no concat call, after Section 06 runtime fix)
- [ ] `x & -1` compiles to `x` for int (BitAnd identity elimination)
- [ ] `x | 0` compiles to `x` for int (BitOr identity elimination)
- [ ] `-(-x)` compiles to `x` for int
- [ ] `!!b` compiles to `b` for bool
- [ ] Commutative operand ordering: `3 + x` normalizes to `x + 3` (constant on right)
- [ ] `(a + b) + c` and `a + (b + c)` produce identical ARC IR
- [ ] `x + (-x)` compiles to `0` for int

**Depends on:** Section 07 (exported law/provenance index) and Section 08 (stdlib laws must be installed).

**Subsumes:** Section 05.2 (reflexive peepholes) and Section 05.3 (boolean/bitwise simplifications) — both are implemented within this section's normalization pass rather than as standalone peepholes. Section 09.2 covers bitwise identity cases (`x & -1 → x`, `x | 0 → x`, `x ^ 0 → x`) and bitwise annihilators (`x & 0 → 0`, `x | -1 → -1`). Section 09.3 covers double negation (`!!x → x`, `~~x → x`). The reflexive peephole (`x == x → true`) is an additional rewrite added during 09.2 for completeness.

**CRITICAL IR FACT: `&&` / `||` are NEVER PrimOps.** `lower_binary()` in `compiler/ori_arc/src/lower/expr/mod.rs` intercepts `BinaryOp::And`/`Or` and routes them to `lower_short_circuit_and()`/`lower_short_circuit_or()`, producing control-flow IR (branches). Only bitwise `&`/`|` (`BitAnd`/`BitOr`) reach PrimOp. Therefore `x && true → x` and `x || false → x` are impossible to express as ARC IR peepholes. All boolean `&&`/`||` simplification items have been removed.

---

## 09.1 Normalization Pass Framework

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs` (NEW), `compiler/ori_arc/src/pipeline/aims_pipeline.rs`

Build the pass infrastructure: a function that walks ARC IR, queries the exported algebra summary, and applies rewrites.

- [ ] Create `compiler/ori_arc/src/aims/normalize/algebra.rs`
- [ ] Define pass entry point:
  ```rust
  pub fn normalize_algebra(
      func: &mut ArcFunction,
      pool: &Pool,
      interner: &StringInterner,
      contracts: &FxHashMap<Name, MemoryContract>,
      algebra_laws: &AlgebraLawIndex, // law queries (built by Section 07.5)
  ) -> AlgebraNormResult {
      // Walk all blocks, all instructions
      // For each instruction:
      //   Let { value: PrimOp { op, args } } → ALL operators (built-in AND user-defined)
      //      - Identify operation from PrimOp::Binary(op)/Unary(op)
      //      - Map BinaryOp → trait name (Add, Mul, etc.) when one exists
      //      - Identify type from func.var_types[dst]
      //      - Query algebra_laws for this (type, trait_name) pair
      //      - NOTE: &&/||/?? are NEVER PrimOps — they are control-flow IR and will not appear here
      //      - Attempt rewrites in priority order
      // Return: count of rewrites applied
  }
  ```
- [ ] Add tracing: `ORI_LOG=ori_arc::aims::normalize::algebra=debug` for rewrite logging
- [ ] Add phase dump integration: `ORI_DUMP_AFTER_ALGEBRA=1` for pre/post normalization IR dump
- [ ] Insert call at step 3b in `aims_pipeline.rs`:
  ```rust
  // Step 3a: normalize_with_trmc (existing)
  // Step 3b: normalize_algebra (NEW)
  let algebra_result = normalize_algebra(
      &mut func, config.pool, config.interner, config.contracts,
      &algebra_laws,  // AlgebraLawIndex — see plumbing note below
  );
  // Step 3c: If algebra normalization created fresh variables (associativity
  // rewrites do this), re-run compute_var_reprs() to avoid stale ValueRepr map.
  if algebra_result.created_fresh_vars {
      compute_var_reprs(&func, config.pool, config.classifier);
  }
  // Step 4: analyze_function (existing)
  ```
- [ ] **var_reprs staleness**: Associativity rewrites (09.5) create fresh `ArcVarId`s for intermediate results. These won't have entries in the `var_reprs` map computed at step 3. The normalization pass MUST either: (a) populate var_reprs for new variables itself, or (b) signal that var_reprs must be recomputed. Option (b) is simpler — follow the TRMC pattern in `normalize_with_trmc()` which re-runs step 3 when it transforms the IR. Identity elimination and commutative reordering do NOT create fresh vars and don't trigger recomputation.
- [ ] **Early exit**: if `algebra_laws.impls.is_empty()`, return immediately with zero rewrites — the pass is a no-op when no laws are installed. This ensures the pass can be safely inserted into the pipeline even before Section 08 installs stdlib laws.
- [ ] Define rewrite priority order: identity > involutive > commutative ordering > associativity > inverse
- [ ] Purity gate: read purity/provenance from `AlgebraLawIndex`. Builtin/stdlib operator entries may be marked `BuiltinPure`; user-defined operator entries must carry an exact `contract_key` exported by Section 07.5. If the entry is missing or the contract shows effects, skip the rewrite. Do NOT reconstruct callee names inside `ori_arc`.
- [ ] **No builtin `&&`/`||` path needed**: `BinaryOp::And` / `Or` are intercepted by `lower_binary()` before PrimOp emission and lowered to control-flow IR (branches). They NEVER appear as `PrimOp::Binary(And)` / `PrimOp::Binary(Or)` in ARC IR. Bitwise `BitAnd`/`BitOr` DO appear as PrimOps — their identity/annihilator rewrites are handled via the algebra law schema (Section 08 installs laws on BitAnd/BitOr traits)

**Law-export plumbing (PREREQUISITE — implemented in Section 07.5):**

The normalization pass needs to query algebraic laws (e.g., "does `impl Add for int` declare `associative`?"). These laws are stored in `ori_types`'s `TraitRegistry` (in `TraitEntry.algebra_laws` and `ImplEntry.declared_laws`, added by Section 07). However, `ori_arc` cannot access `TraitRegistry` — there is no path from `ori_types` to `ori_arc` for trait metadata.

**Section 07.5 owns the solution**: it creates `AlgebraLawIndex`, builds it from `TraitRegistry`, enriches it with exact operator provenance, and threads it through `FunctionCompiler` → `run_arc_pipeline()` → `AimsPipelineConfig`. This section (09) is the consumer — it receives `&AlgebraLawIndex` via `AimsPipelineConfig` and queries it during normalization. If Section 07.5 is not complete, this section is blocked.

- [ ] Verify `AlgebraLawIndex` is available on `AimsPipelineConfig` (implemented by Section 07.5)

- [ ] Write smoke test: `ArcFunction` with `Let { value: PrimOp { op: Binary(Add), args: [x, zero_lit] } }` → normalization replaces with `Let { value: Var(x) }`
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

---

## 09.2 Identity Elimination

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs`

When an operator call has one argument that is the identity element (as determined by the law's `provider_trait::provider_method`), replace the call with the other argument.

- [ ] Detect identity patterns (all operators go through PrimOp — see IR distinction note above):
  - `Let { dst, ty, value: PrimOp { op: Binary(Add), args: [x, identity] } }` — check if `identity` ArcVarId is defined as `Let { value: Literal(Int(0)) }` (or the appropriate identity for the operation)
  - Both directions: right identity `op(x, e) → x` and left identity `op(e, x) → x`
- [ ] Identifying the identity element:
  - Trace the ArcVarId back to its defining `Let` instruction. Match against known identity constants:
    - `PrimOp::Binary(Add)` + `LitValue::Int(0)` → additive identity for int
    - `PrimOp::Binary(Mul)` + `LitValue::Int(1)` → multiplicative identity for int
    - `PrimOp::Binary(BitAnd)` + `LitValue::Int(-1)` → BitAnd identity (all bits set)
    - `PrimOp::Binary(BitOr)` + `LitValue::Int(0)` → BitOr identity
    - `PrimOp::Binary(BitXor)` + `LitValue::Int(0)` → BitXor identity
    - String concat (`PrimOp::Binary(Add)` on `Tag::Str`) + empty string literal → str identity
  - Also detect annihilators (absorbing elements):
    - `PrimOp::Binary(BitAnd)` + `LitValue::Int(0)` → 0 (BitAnd annihilator)
    - `PrimOp::Binary(BitOr)` + `LitValue::Int(-1)` → -1 (BitOr annihilator)
  - **NOTE: `BinaryOp::And` / `Or` (logical `&&`/`||`) are NEVER PrimOps** — they are lowered to control-flow IR (branches) by `lower_short_circuit_and()`/`lower_short_circuit_or()`. Do NOT try to match `PrimOp::Binary(And)` or `PrimOp::Binary(Or)` — they do not exist in ARC IR.
  - For user types (same PrimOp path, different type tag): look up the type + trait in `algebra_laws` for `Identity` law → check if operand is a call to the provider method (e.g., `Zero::zero()`)
- [ ] Replacement: replace the instruction with `Let { dst, ty, value: Var(other_operand) }`
- [ ] Handle RC implications: since we're running BEFORE ARC analysis (step 3b < step 4), RC ops haven't been inserted yet — the replacement is just variable substitution. Dead code from the eliminated identity value will be cleaned up by subsequent passes.
- [ ] **Reflexive peephole (from Section 05.2)**: In the same pass walker, detect `PrimOp::Binary(Eq)` where both args are the same `ArcVarId` AND the type is a builtin non-float primitive (`int`, `bool`, `char`, `byte`, `str`) → replace with `LitValue::Bool(true)`. Similarly `NotEq` same-var → `false`. And comparison reflexives: `x < x` → `false`, `x <= x` → `true`, `x > x` → `false`, `x >= x` → `true`. **Soundness guard**: do NOT fire for float or user-defined/custom Eq; `PrimOp` alone does not prove reflexivity there.
- [ ] Write tests:
  - `x + 0 → x` (int add identity)
  - `0 + x → x` (int add left identity)
  - `x * 1 → x` (int mul identity)
  - `x + "" → x` (str concat identity — only valid after Section 06)
  - `"" + x → x` (str concat left identity)
  - `x & -1 → x` (BitAnd identity, all bits set)
  - `x | 0 → x` (BitOr identity)
  - `x ^ 0 → x` (BitXor identity)
  - `x & 0 → 0` (BitAnd annihilator)
  - `x | -1 → -1` (BitOr annihilator)
  - same-var builtin comparison (`s == s`) rewrites to constant
  - same-var user-defined/custom Eq does NOT rewrite without extra provenance
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

**Matrix dimensions:**
- Types: int (+, *, &, |, ^), float (excluded — no identity law), str (+), [T] (+), builtin same-var comparison vs user-defined same-var comparison
- Patterns: right identity, left identity, both identity (0 + 0 → 0), nested identity, annihilator
- Semantic pin: `x + 0` produces ARC IR with NO PrimOp::Binary(Add) instruction (only passes with normalization)
- Negative pins: `float_x + 0.0` is NOT eliminated (float has no identity law); user-defined `x == x` is NOT rewritten by the builtin reflexive peephole; `x && true` is NOT matchable (not a PrimOp — it's control-flow IR)

---

## 09.3 Involutive Rewrites

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs`

When a unary operator is applied twice, and the operator has the `Involutive` law, eliminate both applications.

- [ ] Detect pattern (all unary operators go through PrimOp):
  - `Let { dst, value: PrimOp { op: Unary(Neg), args: [y] } }` where `y` is defined by `Let { value: PrimOp { op: Unary(Neg), args: [x] } }` → replace with `Let { dst, value: Var(x) }`
- [ ] Query: look up the unary trait (Neg, Not) → check for `Involutive` law on the impl via `algebra_laws`
- [ ] Replacement: replace outer instruction with `Let { dst, ty, value: Var(x) }`
- [ ] Write tests:
  - `-(-x) → x` for int
  - `!!b → b` for bool
  - `~~n → n` for int bitwise not
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

**Matrix dimensions:**
- Types: int (Neg, BitNot), bool (Not)
- Patterns: double negation, triple negation (-(-(-x)) → -x), mixed (not relevant for fixed types)
- Semantic pin: `-(-x)` produces ARC IR with NO PrimOp::Unary(Neg) instructions
- Negative pin: user-defined type with Neg but WITHOUT `Involutive` law → `-(-x)` is NOT eliminated (the law must be declared)

---

## 09.4 Commutative Canonical Ordering

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs`

For commutative operators, sort operands into a canonical order so CSE can match equivalent expressions. Example: `3 + x` and `x + 3` should produce the same ARC IR.

- [ ] Define canonical ordering for ARC operands:
  - Constants sort after variables (constants on right: `x + 3`, not `3 + x`)
  - Among variables, sort by `ArcVarId` (lower ID first)
  - Among constants, sort by value
- [ ] Detect pattern (all binary ops are PrimOp):
  - `Let { value: PrimOp { op: Binary(op), args: [a, b] } }` where op has `Commutative` law for this type AND `a` sorts after `b` → swap args to `[b, a]`
- [ ] Write tests:
  - `3 + x → x + 3` (constant moves to right)
  - `y + x → x + y` if x.id < y.id (canonical variable ordering)
  - `x + 3` unchanged (already canonical)
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

**Matrix dimensions:**
- Types: int (Add, Mul — commutative), str (NOT commutative — concat is ordered), float (commutative)
- Patterns: constant-variable swap, variable-variable ordering, already-canonical no-op
- Semantic pin: `3 + x` produces ARC IR with operands in `[x, 3]` order (only passes with canonicalization)
- Negative pin: `str` concat `"a" + s` is NOT reordered to `s + "a"` (no commutative law for str)

**Note:** This interacts with LLVM's own canonicalization — LLVM also moves constants to the right for int/float. But for user types (matrix, vector), LLVM sees opaque calls and can't reorder. This pass handles those cases.

---

## 09.5 Associativity Normalization

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs`

For associative operators, flatten nested applications into a canonical right-leaning (or left-leaning) tree. This enables CSE to match `(a + b) + c` and `a + (b + c)`.

- [ ] Define canonical form: right-leaning tree (a + (b + (c + d))) — matches LLVM's convention
- [ ] Detect nested application patterns. In ARC IR, nesting is indirect — the outer instruction references a dst variable whose defining instruction is the inner operation:
  - `Let { dst: d1, value: PrimOp { op, args: [inner_dst, c] } }` where `inner_dst` is defined by `Let { dst: inner_dst, value: PrimOp { op: same_op, args: [a, b] } }` AND `op` has `Associative` law for this type
  - Create new intermediate: `Let { dst: new_var, value: PrimOp { op, args: [b, c] } }`, then rewrite outer: `Let { dst: d1, value: PrimOp { op, args: [a, new_var] } }`
- [ ] **Note on fresh defs**: Associativity rewrites CREATE new intermediate variables (`new_var` above). This means `compute_var_reprs()` (step 3) will be stale after this pass. See step 3b var_reprs staleness note below.
- [ ] Chain: apply repeatedly until no more left-nesting exists (fixed point, at most O(depth) iterations)
- [ ] Combine with commutative ordering (09.4): after flattening, sort the flat list, then rebuild the tree
- [ ] **Soundness guard**: only flatten if BOTH inner and outer applications use the same operator AND the type has the associative law. Mixed operators (e.g., `(a + b) * c`) are NOT flattened.
- [ ] Write tests:
  - `(a + b) + c → a + (b + c)` for int
  - `((a + b) + c) + d → a + (b + (c + d))` (deep flattening)
  - `(a + b) * c` NOT flattened (different operators)
  - `(a +_float b) + c` NOT flattened (float has no associative law)
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

**Matrix dimensions:**
- Types: int (Add, Mul — both associative), str (Add — associative), float (NOT associative)
- Patterns: depth 2, depth 3, depth 4, mixed operators, mixed types
- Semantic pin: `(a + b) + c` and `a + (b + c)` produce identical ARC IR post-normalization
- Negative pin: float `(a + b) + c` is NOT reassociated

---

## 09.6 Inverse Elimination

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs`

When an operator is applied to a value and its inverse, replace with the identity element.

- [ ] Detect pattern (indirect through ArcVarId defs — all operators are PrimOp):
  - `Let { dst, value: PrimOp { op: Binary(Add), args: [x, neg_x] } }` where `neg_x` is defined by `Let { value: PrimOp { op: Unary(Neg), args: [x_same] } }` AND `x == x_same` (same ArcVarId) AND Add has `Inverse` law with `Neg` as inverse → replace with `Let { dst, value: Literal(Int(0)) }` (or call to `Zero::zero()` for user types)
  - Also detect commuted form: `args: [neg_x, x]` → same result
- [ ] Replacement: replace with a call to the identity provider (`Zero::zero()` for additive inverse, `One::one()` for multiplicative)
- [ ] **Restriction**: only for types where both the operator and the inverse are declared with laws. `int: Add` has both `Inverse` (Neg) and `Identity` (Zero) → fires. `str: Add` has `Identity` but NOT `Inverse` → does not fire.
- [ ] Write tests:
  - `x + (-x) → 0` for int
  - `(-x) + x → 0` for int (commuted)
  - `x + (-y)` NOT eliminated (different variables)
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

**Matrix dimensions:**
- Types: int (Add with Neg inverse)
- Patterns: `x + (-x)`, `(-x) + x`, `x + (-y)` (should NOT fire), nested `(x + (-x)) + y → y` (chain with identity)
- Semantic pin: `x + (-x)` produces `0` constant in ARC IR

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.1–09.6 implementation work

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] Normalization pass framework exists at step 3b in AIMS pipeline
- [ ] Identity elimination fires for all stdlib types with identity laws
- [ ] Involutive rewrites fire for Neg and Not
- [ ] Commutative canonical ordering sorts operands for CSE
- [ ] Associativity normalization flattens nested applications
- [ ] Inverse elimination replaces `x + (-x)` with zero
- [ ] Float correctly excluded from all rewrites except commutativity
- [ ] Exported provenance + purity gate prevents rewrites on user operators without exact pure-contract evidence
- [ ] `ORI_DUMP_AFTER_ALGEBRA=1` shows normalized IR
- [ ] `timeout 150 ./test-all.sh` green (debug AND release)
- [ ] Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/algebra/` passes — normalization changes ARC IR, which changes LLVM codegen; eval path is unchanged; verify both produce identical observable output
- [ ] `ORI_CHECK_LEAKS=1` clean on all algebraic test programs (normalization removes PrimOp instructions, changing liveness; verify RC ops remain balanced)
- [ ] Plan annotation cleanup
- [ ] **Plan sync**
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** The normalization pass transforms ARC IR using exported algebraic law metadata. `ORI_DUMP_AFTER_ALGEBRA=1` demonstrates identity elimination, canonical ordering, and associativity normalization. Float is correctly excluded, and user-defined operator rewrites only fire when exact provenance + purity data exists. All tests pass in debug and release. Interpreter and LLVM produce identical results for all normalized programs (dual-execution parity).
