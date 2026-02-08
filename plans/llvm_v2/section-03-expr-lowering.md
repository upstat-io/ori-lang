---
section: "03"
title: Expression Lowering Modules
status: not-started
goal: Split the monolithic compile_expr dispatch into focused, independently testable modules with clear inputs and outputs
sections:
  - id: "03.1"
    title: Lowering Architecture
    status: not-started
  - id: "03.2"
    title: Literal & Identifier Lowering
    status: not-started
  - id: "03.3"
    title: Operator Lowering
    status: not-started
  - id: "03.4"
    title: Control Flow Lowering
    status: not-started
  - id: "03.5"
    title: Collection Lowering
    status: not-started
  - id: "03.6"
    title: Error Handling Lowering
    status: not-started
  - id: "03.7"
    title: Completion Checklist
    status: not-started
---

# Section 03: Expression Lowering Modules

**Status:** Not Started
**Goal:** The current `compile_expr` is a giant match on `ExprKind` with ~50 arms in one file. Split this into focused modules where each handles a semantic category. Each module is independently testable and has clear inputs (typed AST node + context) and outputs (ValueId or control flow).

**Reference compilers:**
- **Gleam** -- Separate structs for Erlang/JavaScript backends, each with focused methods
- **Go** -- SSA pass pipeline with ordered, composable transformations
- **Roc** -- `build_exp_stmt()` dispatches to specialized functions per IR construct

**Current state:** `ori_llvm/src/` contains `builder.rs` (~1500 lines, expression compilation + type mapping + locals + phi logic), plus related code spread across:
- `builtin_methods/` (numeric, ordering, units)
- `collections/` (indexing, lists, maps, ranges, strings, structs, tuples, wrappers)
- `control_flow.rs`
- `functions/` (body, builtins, calls, expressions, helpers, lambdas, phi, sequences)
- `matching.rs`
- `operators.rs`
- `types.rs`
- `context.rs`, `compile_ctx.rs`, `declare.rs`, `evaluator.rs`, `module.rs`, `traits.rs`, `runtime.rs`

---

## 03.1 Lowering Architecture

**Tier 1 / Tier 2 dispatch evolution:**
- In **Tier 1**, `ExprLowerer` dispatches on `ExprKind` (typed AST) directly. The lowering modules translate expression nodes straight to LLVM IR without an intermediate ARC representation.
- In **Tier 2** (after `ori_arc` is implemented), the same module structure adapts to dispatch on `ArcInstr` from the ARC IR. The LLVM emission logic -- literal construction, operator codegen, control flow branching -- remains the same; only the input representation changes from expression tree nodes to ARC IR instructions.
- The ARC IR is provided by Section 06 (`ori_arc`). Cross-reference Section 06.0 for the `ArcInstr` enum and the AST-to-ARC-IR lowering that produces the input for Tier 2 dispatch.

### Conventions

**`ExprId::INVALID` sentinel pattern:** Throughout the AST, `ExprId::INVALID` (value `u32::MAX`) represents absent optional children. For example, `If { else_branch: ExprId::INVALID }` means no else branch; `For { guard: ExprId::INVALID }` means no guard; `Block { result: ExprId::INVALID }` means a unit-typed block. Lowering code **must** check `expr_id != ExprId::INVALID` (or use `expr_id.is_valid()`) before lowering optional sub-expressions. Lowering an INVALID ExprId would index past the arena and panic.

**ValueId + debug assertions:** The `ExprLowerer` uses a uniform `ValueId` API. All lowering functions return `Option<ValueId>` (None for void/unit expressions). `debug_assert!` is used to verify type invariants (e.g., that an add operand is actually an integer ValueId). TypeInfo-driven dispatch in `ExprLowerer` ensures correctness at the semantic level; debug assertions catch internal bugs during development.

**Central dispatcher** delegates to focused modules:

```rust
/// Lower a typed AST expression to LLVM IR.
///
/// This is the main dispatch point. Each ExprKind category delegates
/// to a specialized lowering module. The match is exhaustive with no
/// catch-all — a compiler error occurs when a new ExprKind variant is
/// added, forcing an explicit lowering decision.
pub struct ExprLowerer<'a, 'ctx> {
    builder: &'a mut IrBuilder<'ctx>,
    /// Shared reference — TypeInfoStore uses interior mutability (RefCell)
    /// so get() and storage_type_id() take &self. See Section 01.5.
    type_info: &'a TypeInfoStore<'a>,
    scope: Scope,
    arena: &'a ExprArena,
    expr_types: &'a [Idx],
    pool: &'a Pool,

    /// Active loop context for break/continue, if inside a loop.
    loop_ctx: Option<LoopContext>,
}

impl<'a, 'ctx> ExprLowerer<'a, 'ctx> {
    /// Lower an expression, returning its LLVM value.
    ///
    /// Every ExprKind variant is listed explicitly — no catch-all `_ =>`.
    /// Adding a new variant produces a compiler error here, forcing an
    /// explicit lowering decision.
    ///
    /// PRECONDITION: `expr_id` must be valid (not `ExprId::INVALID`).
    /// Callers must check `expr_id != ExprId::INVALID` before calling
    /// this method. Passing INVALID would index past the arena bounds
    /// and panic. Optional sub-expressions (e.g., else branches, guards)
    /// are checked at their call sites — see the ExprId::INVALID sentinel
    /// pattern in Section 03.1 conventions.
    pub fn lower(&mut self, expr_id: ExprId) -> Option<ValueId> {
        debug_assert!(expr_id != ExprId::INVALID, "lower() called with INVALID ExprId");
        let expr = self.arena.get(expr_id);
        let result_type = self.expr_types[expr_id.index()];

        match expr.kind {
            // === Literals (03.2 — lower_literals.rs) ===
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Unit
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::TemplateFull(_)
            | ExprKind::TemplateLiteral { .. }
                => self.lower_literal(expr_id, &expr.kind),

            // === Identifiers & references (03.2 — lower_literals.rs) ===
            ExprKind::Ident(_)
            | ExprKind::Const(_)
            | ExprKind::FunctionRef(_)
            | ExprKind::HashLength
                => self.lower_literal(expr_id, &expr.kind),

            // === Operators (03.3 — lower_operators.rs) ===
            ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Cast { .. }
                => self.lower_operator(expr_id, &expr.kind),

            // === Control flow (03.4 — lower_control_flow.rs) ===
            ExprKind::If { .. }
            | ExprKind::Loop { .. }
            | ExprKind::For { .. }
            | ExprKind::Block { .. }
            | ExprKind::Break(_)
            | ExprKind::Continue(_)
            | ExprKind::Assign { .. }
                => self.lower_control_flow(expr_id, &expr.kind, result_type),

            // === Pattern matching (Section 10 — lower_control_flow.rs initially) ===
            ExprKind::Match { .. }
                => self.lower_control_flow(expr_id, &expr.kind, result_type),

            // === Functions & calls (Section 04 — lower_calls.rs) ===
            // See Section 04 for sret calling convention and large-struct
            // return handling.
            ExprKind::Call { .. }
            | ExprKind::CallNamed { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::MethodCallNamed { .. }
            | ExprKind::Lambda { .. }
                => self.lower_function_expr(expr_id, &expr.kind),

            // === Collections (03.5 — lower_collections.rs) ===
            ExprKind::List(_)
            | ExprKind::ListWithSpread(_)
            | ExprKind::Map(_)
            | ExprKind::MapWithSpread(_)
            | ExprKind::Tuple(_)
            | ExprKind::Struct { .. }
            | ExprKind::StructWithSpread { .. }
            | ExprKind::Range { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index { .. }
                => self.lower_collection(expr_id, &expr.kind),

            // === Error handling (03.6 — lower_error_handling.rs) ===
            ExprKind::Ok(_)
            | ExprKind::Err(_)
            | ExprKind::Some(_)
            | ExprKind::None
            | ExprKind::Try(_)
                => self.lower_error_handling(expr_id, &expr.kind),

            // === Bindings (lower_control_flow.rs) ===
            ExprKind::Let { .. }
                => self.lower_let_binding(expr_id, &expr.kind),

            // === Ori-specific constructs (lower_constructs.rs) ===
            // Handles: run/try/match/for/print/panic/todo/parallel/
            //          spawn/recurse/cache/timeout/with/catch
            ExprKind::FunctionSeq(_)
            | ExprKind::FunctionExp(_)
            | ExprKind::SelfRef
            | ExprKind::Await(_)
            | ExprKind::WithCapability { .. }
                => self.lower_construct(expr_id, &expr.kind),

            // === Error recovery ===
            ExprKind::Error => None,
        }
    }
}
```

**Module structure:**

| File | Responsibility | Approx lines |
|------|---------------|-------------|
| `lower_literals.rs` | Int, float, bool, char, string, unit, duration, size, ident, const, function ref, hash length, template literals | ~150 |
| `lower_operators.rs` | Binary ops (arithmetic, comparison, logical, bitwise, range, coalesce), unary ops, cast | ~250 |
| `lower_control_flow.rs` | If/else, match, loop, for, block, break, continue, assign, let bindings | ~400 |
| `lower_calls.rs` | Call, CallNamed, MethodCall, MethodCallNamed, Lambda | ~200 |
| `lower_collections.rs` | List, Map, Tuple, Struct (with/without spread), Range, Field, Index | ~300 |
| `lower_error_handling.rs` | Ok, Err, Some, None, Try | ~150 |
| `lower_constructs.rs` | FunctionSeq, FunctionExp, SelfRef, Await, WithCapability | ~200 |

**Benefits of this structure:**
1. Each `lower_*` method is in its own file (~100-400 lines each)
2. Adding a new ExprKind variant causes a compiler error in the exhaustive match — forces an explicit lowering decision
3. Each module can be tested with a minimal `IrBuilder` + fake AST
4. The dispatcher lists every variant explicitly (~60 lines)

- [ ] Define `ExprLowerer` struct with `loop_ctx: Option<LoopContext>` field
- [ ] Implement central dispatch in `lower()` method — exhaustive, no catch-all
- [ ] Create module structure: `lower_literals.rs`, `lower_operators.rs`, `lower_control_flow.rs`, `lower_calls.rs`, `lower_collections.rs`, `lower_error_handling.rs`, `lower_constructs.rs`
- [ ] Ensure each module is `pub(crate)` with clear interface

---

## 03.2 Literal & Identifier Lowering

**File:** `lower_literals.rs` (~150 lines)

```rust
impl ExprLowerer<'_, '_> {
    pub(crate) fn lower_literal(
        &mut self, expr_id: ExprId, kind: &ExprKind,
    ) -> Option<ValueId> {
        match kind {
            ExprKind::Int(n) => Some(self.builder.const_i64(*n)),
            ExprKind::Float(bits) => Some(self.builder.const_f64(f64::from_bits(*bits))),
            ExprKind::Bool(b) => Some(self.builder.const_bool(*b)),
            ExprKind::Char(c) => Some(self.builder.const_i32(*c as i32)),
            // Unit uses i64(0) — matches TypeInfo's i64 representation for unit.
            // LLVM void cannot be stored/passed/phi'd, so unit must be a real value.
            ExprKind::Unit => Some(self.builder.const_i64(0)),
            ExprKind::String(name) => self.lower_string_literal(*name),
            ExprKind::Ident(name) => self.lower_identifier(*name),
            ExprKind::Const(name) => self.lower_constant(*name),
            ExprKind::FunctionRef(name) => self.lower_function_ref(*name),
            ExprKind::HashLength => self.lower_hash_length(),
            ExprKind::Duration { value, unit } => self.lower_duration(*value, *unit),
            ExprKind::Size { value, unit } => self.lower_size(*value, *unit),
            ExprKind::TemplateFull(name) => self.lower_template_full(*name),
            ExprKind::TemplateLiteral { head, parts } => {
                self.lower_template_literal(*head, *parts)
            }
            _ => unreachable!("non-literal passed to lower_literal"),
        }
    }

    fn lower_identifier(&mut self, name: Name) -> Option<ValueId> {
        let binding = self.scope.lookup(name)?;
        match binding {
            ScopeBinding::Immutable(val) => Some(val),
            ScopeBinding::Mutable { ptr, ty } => {
                // `ty` is already an LLVMTypeId (stored in ScopeBinding::Mutable),
                // so we resolve it directly through the builder — NOT through
                // type_info.get() which expects an Idx.
                Some(self.builder.load(ty, ptr, &name.as_str()))
            }
        }
    }

    fn lower_template_full(&mut self, name: Name) -> Option<ValueId> {
        // TemplateFull is a template literal with no interpolation —
        // equivalent to a string literal.
        self.lower_string_literal(name)
    }

    fn lower_template_literal(
        &mut self, head: Name, parts: TemplatePartRange,
    ) -> Option<ValueId> {
        // Build string by concatenating: head + (expr_to_string + text_after)*
        // Each part's expr is formatted via Display, then concatenated
        // with the text_after segment.
        todo!("string interpolation via runtime concat")
    }
}
```

- [ ] Implement all literal lowering (int, float, bool, char, unit, string)
- [ ] Implement identifier lookup from Scope (handles both mutable and immutable bindings)
- [ ] Implement constant resolution
- [ ] Implement function reference (`@name`) resolution
- [ ] Implement hash length (`#`) lowering
- [ ] Implement duration/size literal lowering
- [ ] Implement template literal lowering (both `TemplateFull` and `TemplateLiteral`)

---

## 03.3 Operator Lowering

**File:** `lower_operators.rs` (~250 lines)

Uses TypeInfo to dispatch on operand type:

```rust
impl ExprLowerer<'_, '_> {
    pub(crate) fn lower_operator(
        &mut self, expr_id: ExprId, kind: &ExprKind,
    ) -> Option<ValueId> {
        match kind {
            ExprKind::Binary { op, left, right } => {
                self.lower_binary(expr_id, *op, *left, *right)
            }
            ExprKind::Unary { op, operand } => {
                let val = self.lower(*operand)?;
                let operand_type = self.expr_types[operand.index()];
                self.lower_unary_op(*op, val, operand_type)
            }
            ExprKind::Cast { expr, ty, fallible } => {
                self.lower_cast(*expr, *ty, *fallible)
            }
            _ => unreachable!("non-operator passed to lower_operator"),
        }
    }

    /// Lower a binary operation.
    ///
    /// IMPORTANT: Short-circuit operators (And, Or, Coalesce) must NOT
    /// eagerly evaluate both sides. They use conditional branching.
    /// Range/RangeInclusive desugar to ExprKind::Range in the parser
    /// and are not expected here — but are handled defensively.
    fn lower_binary(
        &mut self, expr_id: ExprId, op: BinaryOp, left: ExprId, right: ExprId,
    ) -> Option<ValueId> {
        // Short-circuit operators need special control flow
        match op {
            BinaryOp::And => return self.lower_short_circuit_and(left, right),
            BinaryOp::Or => return self.lower_short_circuit_or(left, right),
            BinaryOp::Coalesce => return self.lower_coalesce(left, right),

            // Range operators as binary ops: these are typically desugared
            // to ExprKind::Range by the parser. If they appear here, lower
            // them as range construction (start, end, no step).
            BinaryOp::Range => {
                return self.lower_range_binop(left, right, /* inclusive */ false);
            }
            BinaryOp::RangeInclusive => {
                return self.lower_range_binop(left, right, /* inclusive */ true);
            }

            // All other operators eagerly evaluate both sides
            _ => {}
        }

        let lhs = self.lower(left)?;
        let rhs = self.lower(right)?;
        let operand_type = self.expr_types[left.index()];
        self.lower_binary_op(op, lhs, rhs, operand_type)
    }

    fn lower_binary_op(
        &mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId, ty: Idx,
    ) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);

        // Dispatch by type category + operator
        if type_info.is_integer() {
            self.lower_int_binary(op, lhs, rhs)
        } else if type_info.is_float() {
            self.lower_float_binary(op, lhs, rhs)
        } else if type_info.is_string() {
            self.lower_string_binary(op, lhs, rhs)
        } else if type_info.is_bool() {
            self.lower_bool_binary(op, lhs, rhs)
        } else {
            // User-defined operator (trait dispatch)
            self.lower_custom_binary(op, lhs, rhs, ty)
        }
    }
}
```

### Short-Circuit Operators

`And`, `Or`, and `Coalesce` must **not** eagerly evaluate both sides. They require conditional branching:

```rust
impl ExprLowerer<'_, '_> {
    /// Lower `a && b` with short-circuit evaluation.
    ///
    /// Evaluate LHS. If false, short-circuit to false.
    /// If true, evaluate RHS. Phi merge the result.
    fn lower_short_circuit_and(
        &mut self, left: ExprId, right: ExprId,
    ) -> Option<ValueId> {
        let lhs = self.lower(left)?;

        let rhs_bb = self.builder.append_block("and.rhs");
        let merge_bb = self.builder.append_block("and.merge");
        let lhs_exit = self.builder.current_block();

        self.builder.cond_br(lhs, rhs_bb, merge_bb);

        // RHS — only evaluated if LHS is true
        self.builder.position_at_end(rhs_bb);
        let rhs = self.lower(right)?;
        let rhs_exit = self.builder.current_block();
        self.builder.br(merge_bb);

        // Merge: LHS false → false, LHS true → RHS value
        self.builder.position_at_end(merge_bb);
        let false_val = self.builder.const_bool(false);
        let bool_ty = self.builder.bool_type();
        Some(self.builder.phi(bool_ty, &[
            (false_val, lhs_exit),
            (rhs, rhs_exit),
        ], "and.result"))
    }

    /// Lower `a || b` with short-circuit evaluation.
    ///
    /// Evaluate LHS. If true, short-circuit to true.
    /// If false, evaluate RHS. Phi merge the result.
    fn lower_short_circuit_or(
        &mut self, left: ExprId, right: ExprId,
    ) -> Option<ValueId> {
        let lhs = self.lower(left)?;

        let rhs_bb = self.builder.append_block("or.rhs");
        let merge_bb = self.builder.append_block("or.merge");
        let lhs_exit = self.builder.current_block();

        self.builder.cond_br(lhs, merge_bb, rhs_bb);

        // RHS — only evaluated if LHS is false
        self.builder.position_at_end(rhs_bb);
        let rhs = self.lower(right)?;
        let rhs_exit = self.builder.current_block();
        self.builder.br(merge_bb);

        // Merge: LHS true → true, LHS false → RHS value
        self.builder.position_at_end(merge_bb);
        let true_val = self.builder.const_bool(true);
        let bool_ty = self.builder.bool_type();
        Some(self.builder.phi(bool_ty, &[
            (true_val, lhs_exit),
            (rhs, rhs_exit),
        ], "or.result"))
    }

    /// Lower `a ?? b` (coalesce) with short-circuit evaluation.
    ///
    /// Extract the tag from LHS (Option or Result). If the LHS has a
    /// value, unwrap it. Otherwise, evaluate RHS. Phi merge the result.
    ///
    /// IMPORTANT: Option and Result have inverted "has-value" tag semantics:
    /// - Option: Some = tag != 0, None = tag == 0
    /// - Result: Ok = tag == 0, Err = tag != 0
    /// The coalesce operator must check the correct tag condition based on
    /// the LHS type. See Section 03.6 for full tag semantics.
    fn lower_coalesce(
        &mut self, left: ExprId, right: ExprId,
    ) -> Option<ValueId> {
        let lhs = self.lower(left)?;
        let lhs_type = self.expr_types[left.index()];
        let type_info = self.type_info.get(lhs_type);

        // Extract tag from the tagged union
        let tag = self.builder.extract_value(lhs, 0, "coalesce.tag");

        let has_value_bb = self.builder.append_block("coalesce.has_value");
        let no_value_bb = self.builder.append_block("coalesce.no_value");
        let merge_bb = self.builder.append_block("coalesce.merge");

        // Branch based on whether LHS has a value.
        // Option: has_value = tag != 0 (Some)
        // Result: has_value = tag == 0 (Ok)
        // Tag is i8 (see Section 01 TypeInfo — tag fields are 1 byte)
        let zero = self.builder.const_i8(0);
        let has_value = if type_info.is_option() {
            self.builder.icmp_ne(tag, zero, "has_value")
        } else {
            // Result type
            self.builder.icmp_eq(tag, zero, "has_value")
        };
        self.builder.cond_br(has_value, has_value_bb, no_value_bb);

        // Has value: unwrap LHS
        self.builder.position_at_end(has_value_bb);
        let unwrapped = self.builder.extract_value(lhs, 1, "coalesce.unwrap");
        let has_value_exit = self.builder.current_block();
        self.builder.br(merge_bb);

        // No value: evaluate RHS
        self.builder.position_at_end(no_value_bb);
        let rhs = self.lower(right)?;
        let no_value_exit = self.builder.current_block();
        self.builder.br(merge_bb);

        // Merge — result type is the unwrapped inner type, which equals
        // the RHS type (both sides of ?? produce the same unwrapped type).
        // Use storage_type_id() to bridge TypeInfo's BasicTypeEnum to the
        // ID-based LLVMTypeId that phi() expects.
        self.builder.position_at_end(merge_bb);
        let result_ty = self.type_info.storage_type_id(
            self.expr_types[right.index()],
            self.builder,
        );
        Some(self.builder.phi(result_ty, &[
            (unwrapped, has_value_exit),
            (rhs, no_value_exit),
        ], "coalesce.result"))
    }
}
```

### Arithmetic Operators

Integer arithmetic includes:
- `Add`, `Sub`, `Mul` — standard `add`/`sub`/`mul` instructions
- `Div` — signed division (`sdiv`)
- `FloorDiv` — integer floor division (the `div` keyword in Ori). Differs from `Div`: `FloorDiv` rounds toward negative infinity (like Python's `//`), while `Div` truncates toward zero (like C's `/`). For non-negative operands they are equivalent; for negative operands, FloorDiv requires a correction step after `sdiv`.
- `Mod` — signed remainder (`srem`)

Float arithmetic uses `fadd`, `fsub`, `fmul`, `fdiv` instructions.

### Range Operators

`BinaryOp::Range` (`..`) and `BinaryOp::RangeInclusive` (`..=`) exist as binary operators in the AST. The parser typically desugars these to `ExprKind::Range { start, end, step: INVALID, inclusive }`, so they rarely appear as `Binary` nodes. If they do appear in binary form, they are lowered as range construction (delegating to the same code as `ExprKind::Range` in `lower_collections.rs`).

- [ ] Implement integer arithmetic (add, sub, mul, sdiv, srem)
- [ ] Implement `FloorDiv` — integer floor division with negative correction
- [ ] Implement float arithmetic (fadd, fsub, fmul, fdiv)
- [ ] Implement comparison operators (int and float variants)
- [ ] Implement short-circuit `And`/`Or` with conditional branching (NOT eager evaluation)
- [ ] Implement short-circuit `Coalesce` (`??`) with tag extraction and conditional branching
- [ ] Implement bitwise operators (BitAnd, BitOr, BitXor, Shl, Shr)
- [ ] Implement string concatenation (via runtime call)
- [ ] Implement string comparison (via runtime call)
- [ ] Implement unary operators (Neg, Not, BitNot) — note: `Try` (`?`) is handled in Section 03.6 as `ExprKind::Try`, not as a unary op. In `lower_unary_op`, `UnaryOp::Try` should be `unreachable!("parser emits ExprKind::Try, not Unary { op: Try }")` since the parser always produces `ExprKind::Try` for the `?` operator, never `ExprKind::Unary { op: UnaryOp::Try, .. }`.
- [ ] Implement `Cast` lowering (infallible `as` and fallible `as?`)
- [ ] Handle `Range`/`RangeInclusive` binary ops (delegate to range construction)

---

## 03.4 Control Flow Lowering

**File:** `lower_control_flow.rs` (~400 lines)

### LoopContext

`ExprLowerer` carries an `Option<LoopContext>` field. When entering a loop (`Loop` or `For`), a `LoopContext` is created and stored. When exiting, it is restored to the previous value. `Break` jumps to `exit_block`, `Continue` jumps to `continue_block`.

> **Migration note:** V1 uses a pre-created phi pattern (`PhiValue` created before the loop body, with incoming values added during lowering). V2 changes to a deferred-phi pattern: break values are collected as `Vec<(ValueId, BlockId)>` during lowering, and the phi node is created after the loop body is fully lowered. Both patterns are valid LLVM IR construction strategies. The deferred-phi approach is simpler — no need to manage partially-constructed phi nodes — and matches how if/else phi merges work in this plan. Implementors should not carry over V1's `PhiValue` infrastructure.

```rust
/// Tracks the current loop's control flow targets for break/continue.
struct LoopContext {
    /// Block to jump to for `break`.
    exit_block: BlockId,
    /// Block to jump to for `continue`.
    continue_block: BlockId,
    /// Values passed to `break expr` — collected for phi merge at exit.
    break_values: Vec<(ValueId, BlockId)>,
}
```

### Block Lowering and StmtKind

Blocks contain statements (`StmtKind::Expr(ExprId)` and `StmtKind::Let { pattern, ty, init, mutable }`). Block lowering iterates statements, lowering each in sequence. The block's value is its last expression (the `result` field), or unit if `result` is `ExprId::INVALID`.

```rust
impl ExprLowerer<'_, '_> {
    /// Lower a block expression.
    ///
    /// The lowerer temporarily switches to a child scope for block-local
    /// bindings, restoring the parent scope after the block completes.
    /// This ensures that `let` bindings inside the block are visible to
    /// subsequent statements and the result expression, but do not leak
    /// into the enclosing scope.
    fn lower_block(
        &mut self, stmts: StmtRange, result: ExprId, result_type: Idx,
    ) -> Option<ValueId> {
        // Swap to child scope — block-local bindings are visible here
        // but do not leak to the parent scope.
        let parent_scope = std::mem::replace(&mut self.scope, self.scope.child());

        for stmt in self.arena.stmts(stmts) {
            match &stmt.kind {
                StmtKind::Expr(expr_id) => {
                    self.lower(*expr_id);
                }
                StmtKind::Let { pattern, ty, init, mutable } => {
                    let init_val = self.lower(*init);
                    // Create binding in current (child) scope —
                    // mutable uses alloca, immutable uses SSA value
                    self.bind_pattern(*pattern, init_val, *mutable);
                }
            }
        }

        // Block result: last expression, or unit if INVALID
        let result_val = if result != ExprId::INVALID {
            self.lower(result)
        } else {
            None  // unit
        };

        // Restore parent scope — block-local bindings are discarded
        self.scope = parent_scope;
        result_val
    }
}
```

Let statements create scope bindings — mutable bindings get an alloca (stored as `ScopeBinding::Mutable`), immutable bindings store the SSA value directly (stored as `ScopeBinding::Immutable`). See Section 02.3 for the `Scope` design (including `child()` scoping, binding shadowing, and variable name resolution).

> **Note:** The `pattern` field in `StmtKind::Let` is a `BindingPatternId`, not a `Name`. The `bind_pattern` method must handle all pattern forms (simple name binding, tuple destructuring, struct destructuring, etc.), not just simple identifiers. Pattern matching infrastructure from Section 10 may be leveraged for complex destructuring patterns.

### If/Else

```rust
impl ExprLowerer<'_, '_> {
    /// Lower if/else to conditional branch + phi.
    fn lower_if(
        &mut self, cond: ExprId, then_branch: ExprId, else_branch: ExprId, result_type: Idx,
    ) -> Option<ValueId> {
        let cond_val = self.lower(cond)?;

        let then_bb = self.builder.append_block("then");
        let else_bb = self.builder.append_block("else");
        let merge_bb = self.builder.append_block("merge");

        self.builder.cond_br(cond_val, then_bb, else_bb);

        // Then branch
        self.builder.position_at_end(then_bb);
        let then_val = self.lower(then_branch);
        let then_exit = self.builder.current_block();
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Else branch (check for ExprId::INVALID = no else)
        self.builder.position_at_end(else_bb);
        let else_val = if else_branch != ExprId::INVALID {
            self.lower(else_branch)
        } else {
            None  // no else branch → unit
        };
        let else_exit = self.builder.current_block();
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Merge with phi
        self.builder.position_at_end(merge_bb);
        let mut incoming = Vec::new();
        if let Some(v) = then_val { incoming.push((v, then_exit)); }
        if let Some(v) = else_val { incoming.push((v, else_exit)); }

        let ty = self.type_info.storage_type_id(result_type, self.builder);
        self.builder.phi_from_incoming(ty, &incoming, "if_result")
    }

    /// Lower match expression using decision tree compilation (Section 10).
    fn lower_match(
        &mut self, scrutinee: ExprId, arms: ArmRange, result_type: Idx,
    ) -> Option<ValueId> {
        // Delegate to pattern compiler (Section 10)
        todo!("Section 10: decision tree compilation")
    }
}
```

### Loop

```rust
impl ExprLowerer<'_, '_> {
    /// Lower loop { body } with break/continue support.
    fn lower_loop(
        &mut self, body: ExprId, result_type: Idx,
    ) -> Option<ValueId> {
        let loop_bb = self.builder.append_block("loop");
        let exit_bb = self.builder.append_block("loop_exit");

        self.builder.br(loop_bb);
        self.builder.position_at_end(loop_bb);

        // Set up loop context for break/continue
        let prev_ctx = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: loop_bb,
            break_values: vec![],
        });

        // Lower body
        self.lower(body);
        if !self.builder.current_block_terminated() {
            self.builder.br(loop_bb);  // implicit continue
        }

        // Restore previous loop context, collect break values
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_ctx;

        self.builder.position_at_end(exit_bb);
        // Phi from break values
        if loop_ctx.break_values.is_empty() {
            None
        } else {
            let ty = self.type_info.storage_type_id(result_type, self.builder);
            self.builder.phi_from_incoming(ty, &loop_ctx.break_values, "loop_result")
        }
    }
}
```

### For Loop

The `For` node has 5 fields:
- `binding: Name` — loop variable name
- `iter: ExprId` — the iterable expression
- `guard: ExprId` — optional filter (`ExprId::INVALID` if absent). When present, acts as `for x in list if x > 0` — only iterations where the guard is true execute the body.
- `body: ExprId` — the loop body expression
- `is_yield: bool` — when `true`, the for loop builds and returns a list (list comprehension semantics: `for x in list yield x * 2` returns a new list). When `false`, the for loop is a statement-like loop returning unit.

### Assign

`Assign { target, value }` handles mutable variable assignment. The target must resolve to a `ScopeBinding::Mutable` binding; the value is lowered and stored via `builder.store()`.

- [ ] Implement if/else lowering with phi merge (handles `ExprId::INVALID` else branch)
- [ ] Implement block lowering (sequential statements via `StmtKind` + result expression)
- [ ] Implement loop lowering with break/continue via `LoopContext`
- [ ] Implement for-loop lowering (iterator protocol, guard filter, yield/list comprehension)
- [ ] Implement break/continue as branch to loop context blocks
- [ ] Implement assign lowering (store to mutable binding)
- [ ] Match lowering: initially sequential if-else (upgrade to decision trees in Section 10)

---

## 03.5 Collection Lowering

**File:** `lower_collections.rs` (~300 lines)

Each collection type uses its TypeInfo for layout:

- [ ] Implement list literal lowering (allocate array, store elements, build struct)
- [ ] Implement list-with-spread lowering (expand spread elements at runtime)
- [ ] Implement map literal lowering (allocate hash table, insert entries)
- [ ] Implement map-with-spread lowering (merge spread maps)
- [ ] Implement tuple lowering (build LLVM struct from elements)
- [ ] Implement struct literal lowering (resolve fields, build LLVM struct)
- [ ] Implement struct-with-spread lowering (copy base, override fields)
- [ ] Implement range lowering (build `{start, end, inclusive}` struct — matches Section 01's `TypeInfo::Range` layout `{i64, i64, i1}`. The AST's `Range { start, end, step, inclusive }` has a `step` field, but the runtime range struct does NOT store step — step is used at the for-loop lowering site to control iteration stride, not stored in the range value itself. When `step` is `ExprId::INVALID`, the for-loop uses a default stride of 1.)
- [ ] Implement field access lowering (`struct_gep` + load)
- [ ] Implement indexing lowering (bounds check + element access)

---

## 03.6 Error Handling Lowering

**File:** `lower_error_handling.rs` (~150 lines)

**Tag semantics (IMPORTANT):**
- **Option:** `None` = tag 0, `Some(value)` = tag != 0 (typically 1)
- **Result:** `Ok(value)` = tag 0, `Err(value)` = tag != 0 (typically 1)

Note the inversion: Option's "has value" is tag != 0, but Result's "has value" (Ok) is tag == 0. The `??` coalesce operator (Section 03.3) must check the correct tag condition depending on whether the LHS is Option or Result.

- [ ] Implement `Ok(expr)` -- tagged union with tag=0
- [ ] Implement `Err(expr)` -- tagged union with tag=1
- [ ] Implement `Some(expr)` -- tagged option with tag=1
- [ ] Implement `None` -- tagged option with tag=0
- [ ] Implement `Try(?)` -- check tag, branch on error, unwrap on success
- [ ] Implement panic codegen (call `ori_panic` runtime function)

---

## 03.7 Completion Checklist

- [ ] All ExprKind variants have corresponding lowering — exhaustive match, no catch-all
- [ ] Each lowering module is independently testable
- [ ] No module exceeds 400 lines
- [ ] Central dispatcher is < 70 lines
- [ ] Short-circuit operators (And, Or, Coalesce) use conditional branching, not eager evaluation
- [ ] `ExprId::INVALID` checked before lowering optional sub-expressions
- [ ] Scope distinguishes mutable (alloca) from immutable (SSA value) bindings
- [ ] LoopContext threaded through ExprLowerer for break/continue
- [ ] Integration test: compile multi-expression programs through new pipeline
- [ ] All existing test programs produce same output

**Exit Criteria:** Every ExprKind can be lowered to LLVM IR through focused, modular lowering functions. Adding a new ExprKind means adding one match arm and one function — the compiler enforces this via the exhaustive match.
