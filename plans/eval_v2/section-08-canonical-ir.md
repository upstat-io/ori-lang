---
section: "08"
title: Canonical Eval IR
status: not-started
goal: Introduce a lowered intermediate representation between the parse AST and evaluation, enabling optimization passes
sections:
  - id: "08.1"
    title: EvalIR Node Design
    status: not-started
  - id: "08.2"
    title: Lowering Pass (ExprArena → EvalIR)
    status: not-started
  - id: "08.3"
    title: Optimization Pipeline
    status: not-started
  - id: "08.4"
    title: Incremental Integration
    status: not-started
---

# Section 08: Canonical Eval IR

**Status:** 📋 Planned
**Goal:** Introduce a canonical evaluation IR between the parse AST (`ExprArena`) and the interpreter, enabling optimization passes (constant folding, dead code elimination, pattern compilation) and providing a cleaner substrate for evaluation.

---

## Prior Art Analysis

### Current Ori: Direct AST Evaluation
The current evaluator operates directly on `ExprArena` nodes. This means every optimization must happen either in the parser (wrong place) or during evaluation (too late). There's no opportunity for whole-expression analysis before execution.

### Roc: Canonical → Mono Pipeline
Roc has two distinct IR levels: **Canonical** (after name resolution, before type solving) and **Mono** (after monomorphization, ready for codegen). The Mono IR is radically simpler — no polymorphism, no generics, explicit layouts, explicit RC operations. This separation enables clean optimization at each level.

### Elm: Three-Tier AST
Elm transforms expressions through **Source AST** → **Canonical AST** (fully qualified, caches loaded) → **Optimized AST** (decision trees, tail calls, dead code removed). Each tier has a distinct `Expr` type optimized for its phase.

### Zig: ZIR → AIR
Zig transforms source through **AST** → **ZIR** (untyped instructions) → **AIR** (typed, after Sema) → machine code. Sema is the bridge that evaluates comptime, resolves types, and produces AIR. The key insight: **each IR level removes a class of complexity**.

---

## 08.1 EvalIR Node Design

The EvalIR is a typed, lowered representation optimized for evaluation:

```rust
/// Arena for EvalIR nodes — SoA layout with flat extra array.
///
/// Follows the Pool pattern (ori_types::Pool): parallel arrays for fixed-size
/// per-node data, with a single `extra: Vec<u32>` for ALL variable-length data.
/// All variable-length elements are u32-sized: EvalIrId(u32) and Name(u32).
///
/// **Salsa compatibility:** EvalIrArena is intentionally NOT a Salsa-tracked
/// structure. It is a transient per-evaluation arena, created during lowering
/// and consumed by the interpreter. It contains `Value` (which may hold heap
/// data like strings/lists) that cannot satisfy Salsa's Hash/Eq requirements.
/// Salsa caching happens at the module level (TypeCheckResult), not at the
/// EvalIR level.
pub struct EvalIrArena {
    // === Parallel arrays (indexed by EvalIrId) ===
    nodes: Vec<EvalIrNode>,
    spans: Vec<Span>,

    // === Flat extra array for ALL variable-length data ===
    // Layout is variant-dependent (same pattern as ori_types::Pool::extra).
    // All elements are u32 (EvalIrId and Name are both u32 newtypes).
    // Variable-length data: [count, elem0, elem1, ...]
    // Pair data: [count, name0, value0, name1, value1, ...]
    extra: Vec<u32>,

    // === Decision tree arena (indexed by DecisionTreeId) ===
    // Compiled pattern decision trees (from Section 04), shared across match expressions.
    decision_trees: Vec<DecisionTree>,
}

/// Opaque index into `EvalIrArena.decision_trees`.
/// Cross-reference: `DecisionTree` type defined in Section 04.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct DecisionTreeId(u32);

#[derive(Copy, Clone)]
pub struct EvalIrId(u32);

/// A node in the evaluation IR.
///
/// Fixed-size enum — all variable-length data lives in `EvalIrArena.extra`.
/// Variants that had SmallVec/Vec fields now store `extra: u32` (an index into
/// the extra array). The layout at that index is variant-specific, documented
/// in the table below.
///
/// **Clone, not Copy:** EvalIrNode implements Clone but NOT Copy because
/// `Const(Value)` may contain heap-allocated data (strings, lists, maps).
/// `PoolRef(ValueId)` handles interned values; `Const(Value)` is for one-off
/// folded constants. This is intentional — measure after implementation before
/// introducing indirection. Size target: ≤ 4 words (32 bytes); may need
/// adjustment once all variants are finalized.
pub enum EvalIrNode {
    // === Constants ===
    /// Pre-evaluated constant value
    Const(Value),
    /// Reference to interned value in ValuePool (depends on Section 01: Value System)
    PoolRef(ValueId),

    // === Variables ===
    /// Variable reference (resolved to scope depth + index for fast lookup)
    Var { name: Name, depth_hint: Option<u16> },
    /// Global reference (prelude function, module-level binding)
    Global { name: Name },

    // === Operators ===
    BinaryOp { left: EvalIrId, op: BinaryOp, right: EvalIrId },
    UnaryOp { op: UnaryOp, operand: EvalIrId },
    /// Type cast (from `as` or `as?`)
    Cast { expr: EvalIrId, target_type: Idx, fallible: bool },

    // === Control Flow ===
    /// If expression with optional else
    If { cond: EvalIrId, then_branch: EvalIrId, else_branch: EvalIrId },
    /// Compiled match expression (decision tree)
    Match { scrutinee: EvalIrId, tree: DecisionTreeId },
    /// Loop with join points for break/continue
    Loop { body: EvalIrId, break_point: JoinPointId, continue_point: JoinPointId },
    /// For loop (desugared to iterator pattern).
    /// Note: AST For uses `guard: ExprId` with `ExprId::INVALID` as sentinel for
    /// "no guard". During lowering, the sentinel is converted to `Option::None`.
    For { binding: Name, iter: EvalIrId, guard: Option<EvalIrId>,
          body: EvalIrId, is_yield: bool },
    /// Block (sequence of expressions, last is the value).
    /// Extra layout: [count, id0, id1, ...]
    Block { extra: u32 },

    // === Bindings ===
    /// Let binding (simple name only, flat block model).
    /// Note: AST `Let` uses `pattern: BindingPatternId` which may be destructured
    /// (e.g., `let (a, b) = expr`). During lowering, destructured patterns become
    /// a Match node + individual Let nodes for each binding.
    /// Sequencing is handled by Block — Let nodes appear in order within
    /// the block, and the last stmt in the block is the block's value expression.
    /// There is no CPS-style `rest` field; the flat block model is simpler and
    /// avoids confusion between Let continuation and Block sequencing.
    Let { name: Name, init: EvalIrId, mutable: bool },
    /// Assignment to mutable binding
    Assign { target: EvalIrId, value: EvalIrId },

    // === Functions ===
    /// Lambda/closure creation.
    /// Extra layout: [param_count, p0, p1, ..., capture_count, c0, c1, ...]
    Lambda { body: EvalIrId, extra: u32 },
    /// Function call (named args already reordered to positional during lowering).
    /// Extra layout: [count, arg0, arg1, ...]
    Call { func: EvalIrId, extra: u32 },
    /// Method call (named args already reordered to positional during lowering).
    /// Extra layout: [count, arg0, arg1, ...]
    MethodCall { receiver: EvalIrId, method: Name, extra: u32 },

    // === Collections ===
    /// Extra layout: [count, elem0, elem1, ...]
    List { extra: u32 },
    /// Extra layout: [count, elem0, elem1, ...]
    Tuple { extra: u32 },
    /// Extra layout: [count, key0, val0, key1, val1, ...]
    Map { extra: u32 },
    /// All three fields are Option to preserve AST semantics for open-ended
    /// ranges (e.g., `..end`, `start..`, `..`). The evaluator applies defaults
    /// (0 for missing start, etc.) at execution time.
    Range { start: Option<EvalIrId>, end: Option<EvalIrId>, step: Option<EvalIrId>, inclusive: bool },
    /// Struct literal: `Point { x: 1, y: 2 }`
    /// Extra layout: [count, fname0, fval0, fname1, fval1, ...]
    Struct { name: Name, extra: u32 },

    // === Access ===
    FieldAccess { receiver: EvalIrId, field: Name },
    IndexAccess { receiver: EvalIrId, index: EvalIrId },
    TupleAccess { receiver: EvalIrId, index: u32 },

    // === Algebraic Types ===
    /// Extra layout: [count, field0, field1, ...]
    Construct { type_name: Name, variant: Name, extra: u32 },
    Some(EvalIrId),
    None,
    Ok(EvalIrId),
    Err(EvalIrId),

    // === Error Handling ===
    Try(EvalIrId),

    // === Capabilities ===
    WithCapability { capability: Name, provider: EvalIrId, body: EvalIrId },

    // === Eval Patterns (FunctionSeq/FunctionExp) ===
    /// Sequential pattern: run { let x = a; let y = b; result }
    /// Extra layout: [count, name0, val0, name1, val1, ...]
    SeqPattern { kind: SeqPatternKind, result: EvalIrId, extra: u32 },
    /// Named expression pattern: cache(key: k, ttl: 60, body)
    /// Extra layout: [count, name0, val0, name1, val1, ...]
    ExpPattern { kind: ExpPatternKind, extra: u32 },

    // === Template Literals ===
    /// Template string: `"hello {name}, you are {age} years old"`
    /// `head` is the text before the first interpolation.
    /// Template parts stored in extra array as 3 consecutive u32s per part:
    /// (expr: EvalIrId, format_spec: Name, text_after: Name).
    /// format_spec uses `Name::EMPTY` for None.
    /// Extra layout: [count, expr0, fmt0, text0, expr1, fmt1, text1, ...]
    TemplateLiteral { head: Name, extra: u32 },

    // === Control ===
    Break(Option<EvalIrId>),
    Continue(Option<EvalIrId>),
    /// Panic/todo/unreachable (terminates evaluation)
    Panic { message: EvalIrId, kind: PanicKind },

    // === Async ===
    Await(EvalIrId),

    // === Join Points (from Section 05) ===
    Join { point: JoinPoint, body: EvalIrId },
    /// Extra layout: [count, arg0, arg1, ...]
    Jump { target: JoinPointId, extra: u32 },

    // === RC Annotations (from Section 09, added by RC pass) ===
    /// Reference counting operation. See Section 09 for RcOp variants
    /// (Inc, Dec, Free, Reset, Reuse).
    Rc(RcOp),

    // === Error Recovery ===
    Invalid { span: Span },
}

/// Sequential pattern kinds (from FunctionSeq).
/// Note: FunctionSeq::Match lowers to EvalIrNode::Match (with decision tree),
/// and FunctionSeq::ForPattern lowers to EvalIrNode::For during lowering.
/// Only Run and Try retain the SeqPattern form.
pub enum SeqPatternKind { Run, Try }

/// Expression pattern kinds (from FunctionExp).
/// Panic, Todo, and Unreachable lower to EvalIrNode::Panic { kind: PanicKind::* }
/// rather than ExpPattern nodes.
pub enum ExpPatternKind {
    Recurse, Parallel, Spawn, Timeout, Cache, With,
    Print, Catch,
}

/// Panic/early-termination kinds
pub enum PanicKind { Panic, Todo, Unreachable }
```

**Extra array layout reference** — each variant with `extra: u32` stores variable-length data in `EvalIrArena.extra` starting at that index:

| Variant | Extra Layout | Accessor |
|---------|-------------|----------|
| `Block` | `[count, id0, id1, ...]` | `get_children()` |
| `Lambda` | `[param_count, p0, p1, ..., capture_count, c0, c1, ...]` | `lambda_param_count/param/capture_count/capture()` |
| `Call` | `[count, arg0, arg1, ...]` | `get_children()` |
| `MethodCall` | `[count, arg0, arg1, ...]` | `get_children()` |
| `List` | `[count, elem0, elem1, ...]` | `get_children()` |
| `Tuple` | `[count, elem0, elem1, ...]` | `get_children()` |
| `Map` | `[count, key0, val0, key1, val1, ...]` | `map_entry_count/key/value()` |
| `Struct` | `[count, fname0, fval0, fname1, fval1, ...]` | `field_count/name/value()` |
| `Construct` | `[count, field0, field1, ...]` | `get_children()` |
| `SeqPattern` | `[count, name0, val0, name1, val1, ...]` | `field_count/name/value()` |
| `ExpPattern` | `[count, name0, val0, name1, val1, ...]` | `field_count/name/value()` |
| `TemplateLiteral` | `[count, expr0, fmt0, text0, expr1, fmt1, text1, ...]` | `template_part_count/expr/format/text()` |
| `Jump` | `[count, arg0, arg1, ...]` | `get_children()` |

**Typed accessor methods** (matching Pool's pattern):

```rust
impl EvalIrArena {
    // === Core allocation and retrieval ===
    pub fn alloc(&mut self, node: EvalIrNode, span: Span) -> EvalIrId;
    pub fn get(&self, id: EvalIrId) -> &EvalIrNode;
    pub fn span_of(&self, id: EvalIrId) -> Span;

    // === Generic extra-array accessors ===
    // Each returns decoded typed data from the raw u32 extra array.

    /// Get child IDs for Block/List/Tuple/Call args/Construct fields/Jump args.
    /// Extra layout: [count, id0, id1, ...]
    pub fn get_children(&self, extra_start: u32) -> &[u32];  // caller wraps as EvalIrId

    /// Get named pair fields for Struct/ExpPattern/SeqPattern.
    /// Extra layout: [count, name0, value0, name1, value1, ...]
    pub fn field_count(&self, extra_start: u32) -> usize;
    pub fn field_name(&self, extra_start: u32, idx: usize) -> Name;
    pub fn field_value(&self, extra_start: u32, idx: usize) -> EvalIrId;

    /// Get map entries. Extra layout: [count, key0, val0, key1, val1, ...]
    pub fn map_entry_count(&self, extra_start: u32) -> usize;
    pub fn map_entry_key(&self, extra_start: u32, idx: usize) -> EvalIrId;
    pub fn map_entry_value(&self, extra_start: u32, idx: usize) -> EvalIrId;

    /// Lambda: params + captures in one extra region.
    /// Extra layout: [param_count, p0, p1, ..., capture_count, c0, c1, ...]
    pub fn lambda_param_count(&self, extra_start: u32) -> usize;
    pub fn lambda_param(&self, extra_start: u32, idx: usize) -> Name;
    pub fn lambda_capture_count(&self, extra_start: u32) -> usize;
    pub fn lambda_capture(&self, extra_start: u32, idx: usize) -> Name;

    /// Template parts. Extra layout: [count, expr0, fmt0, text0, ...]
    pub fn template_part_count(&self, extra_start: u32) -> usize;
    pub fn template_part_expr(&self, extra_start: u32, idx: usize) -> EvalIrId;
    pub fn template_part_format(&self, extra_start: u32, idx: usize) -> Option<Name>;
    pub fn template_part_text(&self, extra_start: u32, idx: usize) -> Name;

    // === Extra array construction (direct-append, no intermediate Vec) ===
    pub fn start_extra(&self) -> u32;
    pub fn push_extra(&mut self, value: u32);
    pub fn push_extra_id(&mut self, id: EvalIrId);
    pub fn push_extra_name(&mut self, name: Name);

    // === Decision tree arena (Section 04 cross-reference) ===
    pub fn alloc_decision_tree(&mut self, tree: DecisionTree) -> DecisionTreeId;
    pub fn get_decision_tree(&self, id: DecisionTreeId) -> &DecisionTree;
}
```

**Key differences from ExprArena:**
- **SoA with extra array**: Follows Pool pattern — parallel arrays for nodes/spans, single `extra: Vec<u32>` for all variable-length data with tag-driven layout (see `ori_types::Pool` for prior art)
- **Const nodes**: Pre-evaluated constant values (from Section 07)
- **PoolRef nodes**: References to interned values (depends on Section 01: ValuePool/ValueId)
- **Var with depth_hint**: Scope depth hints for fast variable lookup
- **Match with DecisionTree**: Compiled patterns (from Section 04)
- **Loop with join points**: Structured control flow (from Section 05)
- **RC annotations**: Rc(RcOp) markers (from Section 09)
- **Spans as parallel array**: `spans: Vec<Span>` indexed by `EvalIrId`, with `span_of()` accessor
- **No type annotations** — except `Cast`, which stores `target_type: Idx` inline (required for runtime cast semantics). All other type information lives in the side table.
- **Desugared**: Spread, named args, `$const`, `@func` resolved during lowering (Pipeline is already desugared at parse time — no `Pipeline` ExprKind exists)

**Desugaring table** — these ExprKind variants do NOT have EvalIR equivalents; they are lowered to simpler forms:

| ExprKind | Lowers to |
|----------|-----------|
| `CallNamed { args }` | `Call` with args reordered to positional (using FunctionSig param order) |
| `MethodCallNamed` | `MethodCall` with args reordered |
| `ListWithSpread` | `Call` to list concat builtin |
| `MapWithSpread` | `Call` to map merge builtin |
| `StructWithSpread` | `Struct` + field overlay from base |
| `Const($name)` | `Var` (resolved to const binding) |
| `FunctionRef(@name)` | `Global` (resolved to function) |
| `SelfRef` | `Var { name: self_name, .. }` |
| `HashLength` | `MethodCall { method: "len", .. }` or `Const(Int)` if known |
| `Field { field }` (numeric) | `TupleAccess { index }` — lowerer checks `interner.lookup(field).parse::<usize>()`: numeric → `TupleAccess`, named → `FieldAccess` |
| `FieldInit { name, value: None }` (shorthand) | Field with value `Var { name, depth_hint }` — shorthand `Point { x }` desugared to `Point { x: x }` |
| `Unit` | `Const(Value::Void)` |
| `Duration/Size` | `Const(Value::Duration/Size)` |
| `TemplateFull(s)` | `Const(Value::Str(s))` |
| `Call` (where callee is VariantConstructor) | `Construct (type_name, variant, fields)` |

- [ ] Define `EvalIrNode` enum in `ori_eval::ir` module (NOT a separate crate)
  - [ ] All variants listed above — variable-length data uses `extra: u32` index, not inline SmallVec/Vec
  - [ ] `SeqPatternKind`, `ExpPatternKind`, `PanicKind` enums
  - [ ] Template literal parts encoded in extra array (3 u32s per part: expr, fmt_spec, text_after) — no `IrTemplatePart` struct
  - [ ] `size_of::<EvalIrNode>()` ≤ 4 words (32 bytes) — profile and optimize (should be smaller now that variable-length data is externalized). Note: EvalIrNode is Clone not Copy due to `Const(Value)` containing heap data. Measure before adding indirection.
- [ ] Define `EvalIrArena` with SoA layout (Pool pattern)
  - [ ] `nodes: Vec<EvalIrNode>` — parallel array of node data
  - [ ] `spans: Vec<Span>` — parallel array of source spans
  - [ ] `extra: Vec<u32>` — flat array for all variable-length data
  - [ ] `alloc(node: EvalIrNode, span: Span) -> EvalIrId` — allocate node + span
  - [ ] `get(id: EvalIrId) -> &EvalIrNode` — retrieve node
  - [ ] `span_of(id: EvalIrId) -> Span` — retrieve span for diagnostics
- [ ] Implement extra array accessor methods (typed views into raw u32 data)
  - [ ] `get_children(extra_start)` — for Block, List, Tuple, Call args, Construct fields, Jump args
  - [ ] `field_count/name/value(extra_start, idx)` — for Struct, ExpPattern, SeqPattern named pairs
  - [ ] `map_entry_count/key/value(extra_start, idx)` — for Map key-value pairs
  - [ ] `lambda_param_count/param/capture_count/capture(extra_start, idx)` — for Lambda params + captures
  - [ ] `template_part_count/expr/format/text(extra_start, idx)` — for TemplateLiteral parts
- [ ] Implement extra array construction methods (direct-append, no intermediate Vec)
  - [ ] `start_extra()` — get current extra array position
  - [ ] `push_extra(value: u32)` / `push_extra_id(EvalIrId)` / `push_extra_name(Name)` — append to extra
- [ ] Define `DecisionTreeId` for referencing compiled patterns
  - [ ] Newtype: `pub struct DecisionTreeId(u32)` — opaque index into `EvalIrArena.decision_trees`
  - [ ] Add field to `EvalIrArena`: `decision_trees: Vec<DecisionTree>` (separate arena, shared across match expressions)
  - [ ] Accessor: `alloc_decision_tree(&mut self, tree: DecisionTree) -> DecisionTreeId` — allocate and return ID
  - [ ] Accessor: `get_decision_tree(&self, id: DecisionTreeId) -> &DecisionTree` — retrieve by ID
  - [ ] Cross-reference: `DecisionTree` type defined in Section 04 (`ori_eval/src/pattern/decision.rs`)
- [ ] Type side table (for type-dependent operations other than Cast)
  - [ ] `types: Vec<Idx>` parallel to `nodes` (from TypeCheckResult::expr_types)
  - [ ] `type_of(id: EvalIrId) -> Idx` — retrieve type for narrowing/diagnostics
  - [ ] Note: Cast nodes store `target_type: Idx` inline; the side table covers other nodes

---

## 08.2 Lowering Pass (ExprArena → EvalIR)

Transform the parse AST into EvalIR:

```rust
pub struct Lowerer<'a> {
    arena: &'a ExprArena,
    ir_arena: EvalIrArena,
    interner: &'a StringInterner,
    /// Type checker output — provides per-expression types, pattern resolutions,
    /// function signatures (for named arg reordering), and capability info.
    /// Note: TypeCheckResult wraps TypedModule with ErrorGuaranteed; the lowerer
    /// accesses the inner TypedModule via `type_result.typed`.
    type_result: &'a TypeCheckResult,
    pattern_compiler: PatternCompiler<'a>,
    /// ConstEvaluator does NOT hold `&mut ValuePool` — pool is passed as a
    /// parameter to its methods (try_eval, eval_const) to avoid Rust aliasing
    /// violations since Lowerer also holds `&mut ValuePool`.
    const_evaluator: ConstEvaluator<'a>,
    pool: &'a mut ValuePool,
}

impl<'a> Lowerer<'a> {
    pub fn lower_module(&mut self, module: &Module) -> EvalIrId {
        // Lower each top-level item from Module's separate field collections.
        // Module stores functions, tests, types, consts, etc. in distinct fields
        // rather than a single `items` vec.
        //
        // Direct-append pattern: push children into extra array as they're
        // lowered, then patch the count. No intermediate Vec allocation.
        let extra_start = self.ir_arena.start_extra();
        let mut count: u32 = 0;
        self.ir_arena.push_extra(0); // placeholder for count

        for func in &module.functions {
            if let Some(ir) = self.lower_item_function(func) {
                self.ir_arena.push_extra_id(ir);
                count += 1;
            }
        }
        for test in &module.tests {
            if let Some(ir) = self.lower_item_test(test) {
                self.ir_arena.push_extra_id(ir);
                count += 1;
            }
        }
        // ... repeat for module.types, module.consts, module.impls
        // Module fields handled during lowering:
        //   - functions: lower each function body
        //   - tests: lower each test body
        //   - types: skip (type-level declarations, no runtime code)
        //   - consts: lower initializer expressions
        //   - impls: lower method bodies
        //   - def_impls: lower method bodies (default implementations)
        //   - extends: lower method bodies (extension methods)
        //   - imports: skip (resolved at parse/check time, no runtime representation)
        //   - traits: skip (type-level declarations, no runtime code)

        // Patch count at the placeholder position
        self.ir_arena.extra[extra_start as usize] = count;
        self.ir_arena.alloc(EvalIrNode::Block { extra: extra_start }, module_span)
    }

    fn lower_expr(&mut self, expr_id: ExprId) -> EvalIrId {
        let span = self.arena.expr_span(expr_id);

        // Try constant evaluation first.
        // Disjoint field borrows — Rust allows splitting &mut self into
        // &mut self.field_a + &mut self.field_b, but `self.const_evaluator.try_eval(expr_id, self.pool)`
        // borrows both through `self` simultaneously, which the borrow checker rejects.
        // We extract the result from the split-borrow block, then use ir_arena outside it.
        let const_result = {
            let const_eval = &mut self.const_evaluator;
            let pool = &mut self.pool;
            const_eval.try_eval(expr_id, pool)
        };
        if let Some(value) = const_result {
            return self.ir_arena.alloc(EvalIrNode::Const(value), span);
        }

        match self.arena.expr_kind(expr_id) {
            ExprKind::Int(n) => {
                self.ir_arena.alloc(EvalIrNode::Const(Value::Int(*n)), span)
            }
            ExprKind::Binary { left, op, right } => {
                let l = self.lower_expr(*left);
                let r = self.lower_expr(*right);
                self.ir_arena.alloc(EvalIrNode::BinaryOp { left: l, op: *op, right: r }, span)
            }
            ExprKind::Match { scrutinee, arms } => {
                let s = self.lower_expr(*scrutinee);
                // Compile patterns to decision tree (Section 04)
                let tree = self.pattern_compiler.compile(arms, self.arena);
                let tree_id = self.ir_arena.alloc_decision_tree(tree);
                self.ir_arena.alloc(EvalIrNode::Match { scrutinee: s, tree: tree_id }, span)
            }
            // ... all other expression kinds
            _ => self.lower_fallback(expr_id),
        }
    }
}
```

**Lowering responsibilities:**
1. **Constant evaluation & folding**: Try to evaluate at compile time; if successful, emit `Const` node. Also fold binary/unary ops on constant children eagerly (Section 07.3 — folding is integrated into lowering, not a separate pass).
2. **Pattern compilation**: Compile match arms to decision trees
3. **Variable resolution hints**: Add scope depth hints for fast lookup
4. **Desugaring**: Spread, named args, template literals, FunctionSeq/Exp (see table above). Note: Pipeline is already desugared at parse time.
5. **Sentinel-to-Option conversion**: AST uses `ExprId::INVALID` sentinels (e.g., For guard, If else_branch); lowerer converts these to `Option<EvalIrId>::None`.
6. **Destructuring-to-match lowering**: `let (a, b) = expr` becomes Match + individual Let nodes.
7. **Block flattening**: `ExprKind::Block { stmts: StmtRange, result: ExprId }` lowers by flattening: each `Stmt`'s inner expression is lowered in order, then the result expression is appended. `StmtRange + result` becomes a flat children list in the extra array (`[count, id0, id1, ...]`).
8. **Type checker integration**: Use `type_result.typed` (TypedModule) for `expr_types`, `pattern_resolutions`, `FunctionSig` during lowering
9. **Span recording**: Track source spans for error reporting

- [ ] Implement `Lowerer` struct
  - [ ] `lower_module(module) -> EvalIrId` — entry point
  - [ ] `lower_expr(expr_id) -> EvalIrId` — recursive lowering
  - [ ] `lower_item_*()` methods — per-item-type lowering:
    - [ ] `lower_item_function` — lower function bodies
    - [ ] `lower_item_test` — lower test bodies
    - [ ] `lower_item_const` — lower const initializer expressions
    - [ ] `lower_item_impl` — lower method bodies
    - [ ] `lower_item_def_impl` — lower default implementation method bodies
    - [ ] `lower_item_extends` — lower extension method bodies
    - [ ] Skip: `imports` (resolved at parse/check time), `traits` (type-level declarations), `types` (type-level declarations)
- [ ] Lower ALL 52 ExprKind variants (no fallback/todo paths):
  - [ ] **Literals**: `Int`, `Float`, `Bool`, `String`, `Char`, `Unit` → `Const`
  - [ ] **Duration/Size**: → `Const(Value::Duration/Size)`
  - [ ] **Template literals**: `TemplateFull` → `Const(Str)`; `TemplateLiteral` → `TemplateLiteral` node with parts in extra array (3 u32s per part: expr, fmt_spec, text_after)
  - [ ] **Variables**: `Ident` → `Var`; `Const($name)` → `Var`; `FunctionRef(@name)` → `Global`; `SelfRef` → `Var`; `HashLength` → resolved
  - [ ] **Operators**: `Binary` → `BinaryOp`; `Unary` → `UnaryOp`; `Cast` → `Cast` (Cast type resolution: `ExprKind::Cast.ty` is a `ParsedTypeId`, resolved to `Idx` using type checker's `expr_types` or `resolve_type_id()` bridge during lowering)
  - [ ] **Calls**: `Call` → `Call`; `CallNamed` → `Call` (reorder args using FunctionSig); `MethodCall` → `MethodCall`; `MethodCallNamed` → `MethodCall` (reorder)
  - [ ] **Access**: `Field` → `FieldAccess` (named) or `TupleAccess` (numeric, via `interner.lookup(field).parse::<usize>()`); `Index` → `IndexAccess`
  - [ ] **Control flow**: `If` → `If`; `Match` → `Match` (compile patterns); `Loop` → `Loop`; `For` → `For`
  - [ ] **Collections**: `List` → `List`; `ListWithSpread` → desugar to concat; `Tuple` → `Tuple`; `Map` → `Map`; `MapWithSpread` → desugar to merge; `Range` → `Range`
  - [ ] **Structs**: `Struct` → `Struct`; `StructWithSpread` → desugar to field overlay
  - [ ] **Algebraic**: `Some`/`None`/`Ok`/`Err` → corresponding node; `Call` to variant constructor → `Construct`
  - [ ] **Bindings**: `Let` → `Let` (with destructuring lowered to match); `Block` → `Block` (children via extra array); `Lambda` → `Lambda` (params + captures via extra array); `Assign` → `Assign`
  - [ ] **Error handling**: `Try` → `Try`
  - [ ] **Capabilities**: `WithCapability` → `WithCapability`; `Await` → `Await`
  - [ ] **Patterns**: `FunctionSeq::Run/Try` → `SeqPattern`; `FunctionSeq::Match` → `Match` (decision tree); `FunctionSeq::ForPattern` → `For`; `FunctionExp` (cache/parallel/spawn/timeout/recurse/with/print/catch) → `ExpPattern`; `FunctionExp` (panic/todo/unreachable) → `Panic { kind: PanicKind::* }`
  - [ ] **Control**: `Break` → `Break`; `Continue` → `Continue`
  - [ ] **Error**: `Error` → `Invalid`
- [ ] Type checker integration during lowering
  - [ ] Read `expr_types[expr_id]` for cast target types
  - [ ] Read `pattern_resolutions` for variant vs variable disambiguation (pass to PatternCompiler)
  - [ ] Read `FunctionSig` for named→positional arg reordering and default parameter insertion
  - [ ] Read `capabilities` from FunctionSig for capability validation
- [ ] Integrate constant evaluation during lowering
  - [ ] Call `const_evaluator.try_eval()` for each expression
  - [ ] Replace constant expressions with `Const` nodes
- [ ] Integrate pattern compilation during lowering
  - [ ] Call `pattern_compiler.compile()` for match expressions
  - [ ] Store decision trees in IR arena

---

## 08.3 Optimization Pipeline

Run optimization passes on the EvalIR:

```rust
pub fn optimize(ir: &mut EvalIrArena, pool: &mut ValuePool) {
    // Note: Constant folding (Section 07) is integrated into the lowering pass
    // (Section 08.2) — it is NOT a separate optimization pass here.

    // Pass 1: Dead code elimination
    dead_code::eliminate(ir);

    // Pass 2: Common subexpression elimination (future)
    // cse::eliminate(ir);

    // Pass 3: Reference counting insertion (Section 09)
    // rc::insert(ir);
}
```

**Dead code elimination**:
```rust
pub fn eliminate(ir: &mut EvalIrArena) {
    // Remove unreachable code after:
    // - Constant condition if/match (dead branches removed by const folding)
    // - Break/continue/panic (subsequent code unreachable)
    // - Unused let bindings (no references)
}
```

- [ ] Implement dead code elimination
  - [ ] Remove code after unconditional break/continue/panic
  - [ ] Remove unused let bindings (conservative: only if no side effects in init)
  - [ ] Remove empty blocks
- [ ] Implement common subexpression elimination (future, optional)
  - [ ] Hash-based detection of identical subtrees
  - [ ] Replace duplicates with references to first occurrence
- [ ] Pipeline post-lowering passes in order: DCE → (CSE) → (RC insert) (constant folding is in the lowerer)
- [ ] Each pass is independently toggleable (for debugging)

---

## 08.4 Full Integration

The EvalIR is the **sole evaluation path** — all evaluation goes through lowering. The interpreter evaluates `EvalIrNode`, not `ExprKind`:

```rust
impl<'a> Interpreter<'a> {
    /// Evaluate from EvalIR (the only evaluation path)
    pub fn eval(&mut self, ir_id: EvalIrId, ir_arena: &EvalIrArena) -> EvalResult {
        let node = ir_arena.get(ir_id);
        match node {
            EvalIrNode::Const(value) => Ok(value.clone()),
            EvalIrNode::PoolRef(id) => Ok(self.pool.get(*id).to_value()),
            EvalIrNode::BinaryOp { left, op, right } => {
                let l = self.eval(*left, ir_arena)?;
                let r = self.eval(*right, ir_arena)?;
                eval_binary_op(&l, *op, &r)
            }
            EvalIrNode::Match { scrutinee, tree } => {
                let val = self.eval(*scrutinee, ir_arena)?;
                let tree = ir_arena.get_decision_tree(*tree);
                self.eval_decision_tree(&val, tree)
            }

            // === Accessor-based evaluation for variable-length nodes ===

            EvalIrNode::Block { extra } => {
                let children = ir_arena.get_children(*extra);
                let mut result = Value::Void;
                for &child_raw in children {
                    result = self.eval(EvalIrId(child_raw), ir_arena)?;
                }
                Ok(result)
            }

            EvalIrNode::Call { func, extra } => {
                let callee = self.eval(*func, ir_arena)?;
                let children = ir_arena.get_children(*extra);
                let mut args = Vec::with_capacity(children.len());
                for &arg_raw in children {
                    args.push(self.eval(EvalIrId(arg_raw), ir_arena)?);
                }
                self.call_function(callee, args)
            }

            EvalIrNode::Struct { name, extra } => {
                let count = ir_arena.field_count(*extra);
                let mut fields = Vec::with_capacity(count);
                for i in 0..count {
                    let fname = ir_arena.field_name(*extra, i);
                    let fval = self.eval(ir_arena.field_value(*extra, i), ir_arena)?;
                    fields.push((fname, fval));
                }
                Ok(Value::Struct(*name, fields))
            }

            EvalIrNode::Map { extra } => {
                let count = ir_arena.map_entry_count(*extra);
                let mut entries = Vec::with_capacity(count);
                for i in 0..count {
                    let key = self.eval(ir_arena.map_entry_key(*extra, i), ir_arena)?;
                    let val = self.eval(ir_arena.map_entry_value(*extra, i), ir_arena)?;
                    entries.push((key, val));
                }
                Ok(Value::Map(entries))
            }

            // ... all other nodes
            _ => todo!("eval for {:?}", node),
        }
    }
}
```

**Transition strategy**:
1. **Step 1**: Build lowerer and `eval()` on EvalIR in parallel with existing code. Validate with comparison tests.
2. **Step 2**: Switch all call sites to the EvalIR path. Run full test suite.
3. **Step 3**: Delete `eval_inner()`, `eval_expr()`, and all ExprArena-direct evaluation code from the interpreter.

- [ ] Create `ori_eval::ir` module with EvalIR types
  - [ ] Lives inside `ori_eval` crate (not a separate crate)
  - [ ] Module: `ori_eval::ir` (types), `ori_eval::ir::lower` (lowering pass)
- [ ] Implement `eval()` in Interpreter operating on EvalIR
  - [ ] Match on all EvalIrNode variants
  - [ ] Reuse evaluation logic (operators, method calls) from existing code during build-out
- [ ] Temporary comparison testing during development
  - [ ] Run both old and new paths, compare results to catch lowering bugs
  - [ ] Remove comparison harness once all tests pass on EvalIR path alone
- [ ] Delete ExprArena direct evaluation
  - [ ] Remove `eval_inner()` and all `ExprKind` dispatch from interpreter
  - [ ] Interpreter no longer depends on `ExprArena` for evaluation (only lowerer does)

---

## 08.5 Completion Checklist

- [ ] `EvalIrNode` enum defined with all variants — variable-length data uses `extra: u32` index (no SmallVec/Vec fields)
- [ ] `EvalIrArena` with SoA layout: parallel `nodes`/`spans` arrays + flat `extra: Vec<u32>` (Pool pattern)
- [ ] Extra array accessor methods for all variant layouts (`get_children`, `field_count/name/value`, `map_entry_*`, `lambda_*`, `template_part_*`)
- [ ] Extra array construction methods (`start_extra`, `push_extra`, `push_extra_id`, `push_extra_name`)
- [ ] `Lowerer` handles all 52 ExprKind variants (no fallback/todo paths)
- [ ] All desugarings implemented (spread, named args, template, destructuring-to-match, sentinel-to-Option, etc.)
- [ ] Lowering uses direct-append pattern (push to extra array, patch count) — no intermediate Vec for children
- [ ] Type checker outputs consumed (expr_types, pattern_resolutions, FunctionSig)
- [ ] Constant expressions replaced with `Const` nodes during lowering (Section 07.3 folding integrated)
- [ ] Match expressions compiled to decision trees during lowering
- [ ] Dead code elimination pass
- [ ] `eval()` evaluates EvalIR nodes correctly using accessor methods
- [ ] Span parallel array and type side table for error reporting and casts
- [ ] All tests pass on EvalIR path
- [ ] ExprArena direct evaluation (`eval_inner`) deleted from interpreter

### Control Flow Integration

The interpreter's `eval()` method on EvalIR returns `EvalResult` publicly. Internally, `eval_flow()` returns `EvalFlow` (from Section 05) which distinguishes `ControlAction` (break/continue/propagate) from `EvalError`. Loop and function boundary nodes catch `ControlAction` and convert to `EvalResult`. The `FlowOrError` type from Section 05 is `pub(crate)` within `ori_eval` — external callers only see `EvalResult`.

**Exit Criteria:** A canonical EvalIR is the sole evaluation substrate. All 52 ExprKind variants are lowered (with desugarings where appropriate). All type checker outputs are consumed during lowering. No direct ExprArena evaluation remains in the interpreter.
