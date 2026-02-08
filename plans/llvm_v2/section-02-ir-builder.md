---
section: "02"
title: IrBuilder & ID-Based Value System
status: complete
goal: Replace direct inkwell usage with a safe, ID-based builder that eliminates lifetime pain and makes instruction emission ergonomic
sections:
  - id: "02.1"
    title: ID-Based Value & Type System
    status: complete
  - id: "02.2"
    title: IrBuilder Core
    status: complete
  - id: "02.3"
    title: Scope & Local Variable Management
    status: complete
  - id: "02.4"
    title: Block & Control Flow Management
    status: complete
  - id: "02.5"
    title: Completion Checklist
    status: complete
---

# Section 02: IrBuilder & ID-Based Value System

**Status:** Complete
**Goal:** A builder abstraction that wraps inkwell with safety, eliminates LLVM lifetime complexity, and provides ergonomic instruction emission. The key innovation is ID-based references that decouple from LLVM's arena lifetime.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_llvm/src/builder.rs` -- `GenericBuilder<'a, 'll, CX>` with two-context system, RAII Drop
- **Zig** `lib/std/zig/llvm/Builder.zig` -- Pure Zig IR with ID-based references, deferred LLVM involvement
- **Roc** `crates/compiler/gen_llvm/src/llvm/build.rs` -- `BuilderExt` trait wrapping inkwell to unwrap Results

**Current state:** `ori_llvm/src/builder.rs` is ~1500 lines. It wraps inkwell's Builder but also contains expression compilation, type mapping, local variable management, and phi node logic all in one struct. The `'ll` lifetime threads through everything.

---

## 02.1 ID-Based Value & Type System

**Problem:** inkwell values like `BasicValueEnum<'ctx>` carry a lifetime tied to the LLVM Context. This lifetime must thread through every function, struct, and trait in the codegen. It makes refactoring painful and prevents storing values across compilation boundaries.

**Solution (from Zig):** Wrap LLVM values in IDs:

```rust
/// Opaque handle to an LLVM value. No lifetime parameter.
///
/// Valid only within the IrBuilder that created it.
/// The underlying inkwell value is stored in the builder's value arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueId(u32);

/// Opaque handle to an LLVM type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LLVMTypeId(u32);

/// Opaque handle to an LLVM basic block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(u32);

/// Opaque handle to an LLVM function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionId(u32);

impl ValueId {
    pub const NONE: Self = Self(u32::MAX);
    pub fn is_none(self) -> bool { self.0 == u32::MAX }
}
```

**Value Arena** (stored inside IrBuilder):

```rust
/// Stores all LLVM values created during codegen.
/// Values are referenced by ValueId index.
struct ValueArena<'ctx> {
    values: Vec<BasicValueEnum<'ctx>>,
    types: Vec<BasicTypeEnum<'ctx>>,
    blocks: Vec<BasicBlock<'ctx>>,
    functions: Vec<FunctionValue<'ctx>>,
}

impl<'ctx> ValueArena<'ctx> {
    fn push_value(&mut self, v: BasicValueEnum<'ctx>) -> ValueId {
        let id = ValueId(self.values.len() as u32);
        self.values.push(v);
        id
    }

    fn get_value(&self, id: ValueId) -> BasicValueEnum<'ctx> {
        self.values[id.0 as usize]
    }
}
```

**Context scoping constraint:** ValueIds (and all other IDs) are scoped to a single LLVM Context. Each `IrBuilder` owns its own `ValueArena` backed by one Context. For parallel codegen (Section 12), each thread gets its own LLVM Context and its own `IrBuilder` with its own ID namespace. **IDs from one context MUST NOT be used in another** — they would index into the wrong arena and produce incorrect or dangling values. This is enforced by the fact that `IrBuilder` is not `Send`/`Sync` (the underlying inkwell Context is not thread-safe). Cross-context communication uses serialized module bitcode, not ValueIds.

- [x] Define `ValueId`, `LLVMTypeId`, `BlockId`, `FunctionId` newtypes
- [x] Implement `ValueArena` with push/get for each entity type
- [x] Ensure `ValueId::NONE` sentinel works for optional values
- [x] Add debug assertion: IDs cannot exceed arena length (catches cross-context misuse)
- [ ] Benchmark: ID indirection cost vs lifetime elimination benefit

---

## 02.2 IrBuilder Core

The central builder that wraps inkwell's `Builder` with safety and ergonomics:

```rust
/// Safe, ergonomic wrapper over inkwell's instruction builder.
///
/// All instruction emission methods return ValueId (not inkwell types).
/// LLVM lifetime is contained within this struct; callers never see 'ctx.
///
/// Design from Rust's GenericBuilder + Roc's BuilderExt.
/// **Design note:** The existing `Builder` holds `&'a CodegenCx`, which bundles
/// context, module, and type caches into one struct. IrBuilder takes the same
/// approach: it holds a `&'ctx CodegenCx` (or equivalent) rather than individual
/// component references. This keeps the API surface clean and matches the existing
/// pattern. The individual fields shown below are the logical components that
/// CodegenCx provides; during implementation, they will likely be accessed through
/// a single `&'ctx CodegenCx` reference.
pub struct IrBuilder<'ctx> {
    /// The underlying inkwell builder.
    builder: Builder<'ctx>,

    /// LLVM context (for creating types and constants).
    context: &'ctx Context,

    /// LLVM module (for declaring functions and globals).
    module: &'ctx Module<'ctx>,

    /// Value/type/block storage (ID-based).
    arena: ValueArena<'ctx>,

    /// Current function being compiled.
    current_function: Option<FunctionId>,

    /// Current insertion point (basic block).
    current_block: Option<BlockId>,
}

impl<'ctx> IrBuilder<'ctx> {
    // === Constants ===
    pub fn const_i8(&mut self, val: i8) -> ValueId;
    pub fn const_i32(&mut self, val: i32) -> ValueId;
    pub fn const_i64(&mut self, val: i64) -> ValueId;
    pub fn const_f64(&mut self, val: f64) -> ValueId;
    pub fn const_bool(&mut self, val: bool) -> ValueId;
    pub fn const_null_ptr(&mut self) -> ValueId;
    pub fn const_string(&mut self, s: &str) -> ValueId;
    pub fn build_global_string_ptr(&mut self, value: &str) -> ValueId;

    // === Memory ===
    pub fn alloca(&mut self, ty: LLVMTypeId, name: &str) -> ValueId;
    /// Alloca at function entry block (critical for mem2reg promotion).
    /// Always inserts at the start of the entry block, regardless of
    /// current insertion point.
    pub fn create_entry_alloca(&mut self, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn load(&mut self, ty: LLVMTypeId, ptr: ValueId, name: &str) -> ValueId;
    pub fn store(&mut self, val: ValueId, ptr: ValueId);
    pub fn gep(&mut self, ty: LLVMTypeId, ptr: ValueId, indices: &[ValueId], name: &str) -> ValueId;
    /// Struct field access by index (typed GEP into struct).
    /// `ty` is the struct type being pointed to — required by LLVM's opaque
    /// pointer semantics (GEP needs the pointee type to compute field offsets).
    pub fn struct_gep(&mut self, ty: LLVMTypeId, ptr: ValueId, index: u32) -> ValueId;

    // === Signed Arithmetic ===
    pub fn add(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn sub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn mul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn sdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn srem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn neg(&mut self, val: ValueId, name: &str) -> ValueId;

    // === Unsigned Arithmetic ===
    pub fn udiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn urem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn lshr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;

    // === Float Arithmetic ===
    pub fn fadd(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn fsub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn fmul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn fdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn frem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn fneg(&mut self, val: ValueId, name: &str) -> ValueId;

    // === Bitwise Operations ===
    pub fn and(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn or(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn xor(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn not(&mut self, val: ValueId, name: &str) -> ValueId;
    pub fn shl(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn ashr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;

    // === Comparisons ===
    pub fn icmp_eq(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn icmp_ne(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn icmp_slt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn icmp_sgt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn icmp_sle(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn icmp_sge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    pub fn fcmp_oeq(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId;
    // ... other float comparisons

    // === Conversions ===
    pub fn bitcast(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn trunc(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn sext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn zext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn si_to_fp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn fp_to_si(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn uitofp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn fptoui(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn ptr_to_int(&mut self, ptr: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;
    pub fn int_to_ptr(&mut self, int: ValueId, ty: LLVMTypeId, name: &str) -> ValueId;

    // === Control Flow ===
    pub fn br(&mut self, dest: BlockId);
    pub fn cond_br(&mut self, cond: ValueId, then_bb: BlockId, else_bb: BlockId);
    pub fn select(&mut self, cond: ValueId, then_val: ValueId, else_val: ValueId, name: &str) -> ValueId;
    pub fn switch(&mut self, val: ValueId, default: BlockId, cases: &[(ValueId, BlockId)]);
    pub fn ret(&mut self, val: ValueId);
    pub fn ret_void(&mut self);
    pub fn unreachable(&mut self);

    // === Aggregates ===
    pub fn extract_value(&mut self, agg: ValueId, index: u32, name: &str) -> ValueId;
    pub fn insert_value(&mut self, agg: ValueId, val: ValueId, index: u32, name: &str) -> ValueId;
    pub fn build_struct(&mut self, ty: LLVMTypeId, fields: &[ValueId], name: &str) -> ValueId;

    // === Calls ===
    pub fn call(&mut self, func: FunctionId, args: &[ValueId], name: &str) -> ValueId;
    pub fn call_indirect(&mut self, fn_ty: LLVMTypeId, ptr: ValueId, args: &[ValueId], name: &str) -> ValueId;

    // === Type Registration ===

    /// Register an inkwell type and return an opaque ID.
    /// Used to bridge TypeInfo (which returns BasicTypeEnum) with the
    /// ID-based builder. TypeInfoStore::storage_type_id() calls this
    /// internally — most callers should use that convenience method instead.
    pub fn register_type(&mut self, ty: BasicTypeEnum<'ctx>) -> LLVMTypeId;

    // === Primitive Type Convenience Methods ===
    //
    // Convenience methods for common primitive types. Each creates/registers
    // the corresponding LLVM type and returns an LLVMTypeId. Frequently used
    // in expression lowering to avoid routing every primitive through
    // TypeInfoStore (e.g., for phi node types in short-circuit operators,
    // loop break values, and if/else merge points).

    pub fn bool_type(&mut self) -> LLVMTypeId;   // i1
    pub fn i32_type(&mut self) -> LLVMTypeId;     // i32
    pub fn i64_type(&mut self) -> LLVMTypeId;     // i64
    pub fn f64_type(&mut self) -> LLVMTypeId;     // f64
    pub fn unit_type(&mut self) -> LLVMTypeId;    // i64 (unit representation)
    pub fn ptr_type(&mut self) -> LLVMTypeId;     // ptr (opaque pointer)

    // === Phi Nodes ===
    pub fn phi(&mut self, ty: LLVMTypeId, incoming: &[(ValueId, BlockId)], name: &str) -> ValueId;

    // === Position Management ===
    /// Save the current builder position and return an RAII guard.
    /// When the guard is dropped, the builder position is restored.
    /// Essential for emitting entry-block allocas or out-of-order IR.
    pub fn save_position(&mut self) -> BuilderPositionGuard<'_>;
}
```

**Debug assertions:** All arithmetic and comparison methods include `debug_assert!` type checking to catch type mismatches during development. For example, `add()` asserts both operands are integer-typed, `fadd()` asserts both are float-typed. These assertions have zero cost in release builds but catch internal bugs early.

- [x] Implement `IrBuilder` struct with `ValueArena`
- [x] Implement all constant creation methods (including `build_global_string_ptr`)
- [x] Implement all memory operation methods (including `create_entry_alloca`, `struct_gep`)
- [x] Implement all signed arithmetic methods (including `neg`)
- [x] Implement all unsigned arithmetic methods (`udiv`, `urem`, `lshr`)
- [x] Implement all float arithmetic methods (including `fneg`, `frem`)
- [x] Implement all bitwise operation methods (`and`, `or`, `xor`, `not`, `shl`, `ashr`)
- [x] Implement all comparison methods
- [x] Implement all conversion methods (including `uitofp`, `fptoui`, `ptr_to_int`, `int_to_ptr`)
- [x] Implement `select` (conditional move)
- [x] Implement all control flow methods
- [x] Implement aggregate and call methods
- [x] Implement phi node construction
- [x] Implement `save_position()` / manual restore pattern (RAII deferred due to `&mut self` borrow friction)
- [x] Implement `current_block()` public accessor (unwraps Option with expect)
- [x] Implement primitive type convenience methods (`bool_type()`, `i32_type()`, `i64_type()`, `f64_type()`, `unit_type()`, `ptr_type()`)
- [x] Add `#[inline]` on hot-path methods
- [x] Add `debug_assert!` type checking on all arithmetic/comparison ops

---

## 02.3 Scope & Local Variable Management

**Dependency:** Requires the `im` crate (persistent/immutable data structures) for efficient scope nesting. Add `im = "15"` to `ori_llvm/Cargo.toml`.

**Migration note:** The existing codebase has `Locals` (struct with `FxHashMap<Name, LocalStorage>`) and `LocalStorage` enum (`Immutable(BasicValueEnum)` / `Mutable { ptr, ty }`) in `builder.rs`. The V2 `Scope` and `ScopeBinding` replace these with two key improvements: (1) `im::HashMap` enables O(1) scope cloning via structural sharing (vs. O(n) `FxHashMap::clone()` for each nested scope), and (2) ID-based values (`ValueId` / `LLVMTypeId`) instead of lifetime-bearing inkwell types. The naming change from `Locals`/`LocalStorage` to `Scope`/`ScopeBinding` reflects the broader responsibility (scoped binding management, not just local variable storage).

```rust
/// Binding kinds: immutable (SSA value) vs mutable (alloca + load/store).
#[derive(Clone, Copy)]
pub enum ScopeBinding {
    /// Immutable binding — the value is an SSA value, no alloca needed.
    /// Used for `let x = ...` (non-mutable) bindings.
    Immutable(ValueId),

    /// Mutable binding — the value is a pointer to an alloca.
    /// Load to read, store to write. Used for `let mut x = ...`.
    /// `ty` is the LLVM type of the pointed-to value, needed for typed loads
    /// (LLVM opaque pointers require the pointee type at load/store sites).
    /// We store LLVMTypeId rather than Idx because loads are LLVM operations
    /// that need the LLVM type directly — storing Idx would require a
    /// TypeInfoStore lookup on every load, adding unnecessary indirection.
    Mutable { ptr: ValueId, ty: LLVMTypeId },
}

/// Tracks local variable bindings for the current function.
///
/// Uses a persistent map (`im::HashMap`) for efficient scope
/// nesting without cloning the entire map. The `im` crate uses
/// structural sharing, making `clone()` O(1).
///
/// Design from Roc's Scope<'a, 'ctx>.
pub struct Scope {
    /// Variable name → binding (immutable SSA value or mutable alloca).
    bindings: im::HashMap<Name, ScopeBinding>,
}

impl Scope {
    /// Create a child scope (for blocks, lambdas).
    /// O(1) clone via persistent map structural sharing.
    pub fn child(&self) -> Self {
        Self { bindings: self.bindings.clone() }
    }

    /// Bind an immutable variable (SSA value, no alloca).
    pub fn bind_immutable(&mut self, name: Name, val: ValueId) {
        self.bindings.insert(name, ScopeBinding::Immutable(val));
    }

    /// Bind a mutable variable (alloca pointer + LLVM pointee type).
    pub fn bind_mutable(&mut self, name: Name, ptr: ValueId, ty: LLVMTypeId) {
        self.bindings.insert(name, ScopeBinding::Mutable { ptr, ty });
    }

    /// Look up a variable.
    pub fn lookup(&self, name: Name) -> Option<ScopeBinding> {
        self.bindings.get(&name).copied()
    }
}
```

- [x] Implement `ScopeBinding` enum (Immutable / Mutable)
- [x] Implement `Scope` with persistent `im::HashMap`
- [x] Add `im` crate dependency to `ori_llvm/Cargo.toml`
- [x] Implement scope nesting for blocks and lambdas
- [x] Handle variable shadowing (new binding replaces old in child scope)
- [x] Handle mutable bindings (alloca via `create_entry_alloca` + load/store)
- [ ] Handle closures (captured variable lifting) — deferred to Section 03/04

---

## 02.4 Block & Control Flow Management

```rust
impl<'ctx> IrBuilder<'ctx> {
    /// Create a new basic block in the current function.
    pub fn append_block(&mut self, name: &str) -> BlockId {
        let func = self.current_function_value();
        let bb = self.context.append_basic_block(func, name);
        self.arena.push_block(bb)
    }

    /// Set the insertion point to the end of a block.
    pub fn position_at_end(&mut self, block: BlockId) {
        let bb = self.arena.get_block(block);
        self.builder.position_at_end(bb);
        self.current_block = Some(block);
    }

    /// Return the current insertion block.
    ///
    /// Panics if no block has been set (i.e., before `position_at_end` is called).
    /// Used by expression lowering to record the "exit block" for phi nodes
    /// (e.g., capturing which block a branch came from for short-circuit operators,
    /// if/else merge, loop break values).
    pub fn current_block(&self) -> BlockId {
        self.current_block.expect("no current block set")
    }

    /// Check if the current block has a terminator.
    pub fn current_block_terminated(&self) -> bool {
        self.current_block
            .map(|id| self.arena.get_block(id).get_terminator().is_some())
            .unwrap_or(true)
    }

    /// Build a phi node from incoming values (handles single-value optimization).
    ///
    /// If there's only one incoming value, returns it directly (no phi needed).
    /// This is a common optimization from the current codebase.
    pub fn phi_from_incoming(
        &mut self,
        ty: LLVMTypeId,
        incoming: &[(ValueId, BlockId)],
        name: &str,
    ) -> Option<ValueId> {
        match incoming.len() {
            0 => None,
            1 => Some(incoming[0].0),
            _ => Some(self.phi(ty, incoming, name)),
        }
    }
}
```

- [x] Implement block creation and positioning
- [x] Implement `current_block_terminated()` check
- [x] Implement `phi_from_incoming()` with single-value optimization
- [x] Handle unreachable blocks (mark with `unreachable` instruction)
- [ ] Handle merge blocks for if/match/loop — deferred to Section 03 (expression lowering)

---

## 02.5 Completion Checklist

- [x] `IrBuilder` with all instruction emission methods (including unsigned ops, float negation/remainder, bitwise ops, integer negation, casts, select, position guard)
- [x] ID-based value system (ValueId, LLVMTypeId, BlockId, FunctionId)
- [x] `Scope` with persistent map (`im::HashMap`) and `ScopeBinding` enum (Immutable/Mutable)
- [x] Block and control flow management
- [ ] All existing Builder functionality preserved — verified during Section 03 migration
- [x] No `'ll` lifetime exposed to callers (uses `IrBuilder<'scx, 'ctx>` two-lifetime pattern)
- [x] `debug_assert!` type checking on arithmetic/comparison ops
- [x] Tests for each instruction category (37 tests across 3 files)

**Exit Criteria:** The IrBuilder can emit all LLVM IR that the current Builder can — including bitwise operations (`and`, `or`, `xor`, `not`, `shl`, `ashr`), integer negation (`neg`), and float remainder (`frem`) — without exposing inkwell lifetimes to calling code.
