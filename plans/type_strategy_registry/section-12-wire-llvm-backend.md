---
plan: "type_strategy_registry"
section: "12"
title: "Wire LLVM Backend (ori_llvm) — OpStrategy Dispatch & Builtin Simplification"
status: not-started
depends_on:
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
  - "08"
  - "11"
subsections:
  - id: "12.1"
    title: "Replace emit_binary_op Type Guards with OpStrategy Dispatch"
    status: not-started
  - id: "12.2"
    title: "Idx-to-TypeTag Bridge for LLVM"
    status: not-started
  - id: "12.3"
    title: "Replace emit_unary_op Type Guards with OpStrategy Dispatch"
    status: not-started
  - id: "12.4"
    title: "Remove receiver_borrowed from BuiltinRegistration"
    status: not-started
  - id: "12.5"
    title: "Simplify declare_builtins! Macro"
    status: not-started
  - id: "12.6"
    title: "Delete borrowing_builtin_names() Function"
    status: not-started
  - id: "12.7"
    title: "ARC_PIPELINE_METHODS Migration"
    status: not-started
  - id: "12.8"
    title: "BuiltinTable Registry Validation"
    status: not-started
  - id: "12.9"
    title: "Validation & Regression"
    status: not-started
---

# Section 12: Wire LLVM Backend (ori_llvm) — OpStrategy Dispatch & Builtin Simplification

**Status:** Not Started
**Goal:** Eliminate the ad-hoc `is_float`/`is_str` type guard pattern in `emit_binary_op` and `emit_unary_op` permanently, replacing it with `OpStrategy` dispatch driven by `ori_registry` lookups. Remove `receiver_borrowed` from `BuiltinRegistration` (ownership now comes from the registry). Simplify the `declare_builtins!` macro. Delete `borrowing_builtin_names()`. Add registry-backed validation tests.

**This is the section that eliminates the string comparison ordering bug class permanently.** The recent fix that added `is_str` guards for `Lt`, `Gt`, `LtEq`, `GtEq` in `emit_binary_op` was correct but brittle: the same bug class will reappear whenever a new comparable type is added (e.g., `Duration`, user-defined types with operator overloads on primitive-like representations). After this section, adding a new type's operator semantics is a registry entry, not a code change in `emit_binary_op`.

**Context:** Section 11 wires `ori_arc` to read ownership from the registry instead of from `borrowing_builtin_names()`. This section completes the downstream half of that work: the LLVM backend stops producing the ownership data (`receiver_borrowed`, `borrowing_builtin_names()`) and instead consumes operator strategy data from the registry.

---

## The Bug This Section Prevents

**Root cause (historical):** `emit_binary_op` at `arc_emitter/mod.rs:1525` dispatches binary operators using a cascade of `is_float`/`is_str` boolean guards:

```rust
let is_float = matches!(self.type_info.get(lhs_ty), TypeInfo::Float);
let is_str = matches!(self.type_info.get(lhs_ty), TypeInfo::Str);
match op {
    BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
    BinaryOp::Add if is_str => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
    BinaryOp::Add => self.builder.add(lhs, rhs, "add"),  // int fallthrough
    // ...
    BinaryOp::Lt if is_float => self.builder.fcmp_olt(lhs, rhs, "lt"),
    BinaryOp::Lt if is_str => self.emit_str_cmp_predicate(...),
    BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),  // int fallthrough
    // ...
}
```

**Failure mode:** When the `is_str` guard was missing for `Lt`/`Gt`/`LtEq`/`GtEq`, string comparisons silently fell through to the int path (`icmp slt`), comparing raw pointer bits instead of string contents. This produced correct results for equal strings (same interning) and wrong results for unequal strings. The bug was invisible in simple tests and manifested only in specific string ordering scenarios.

**Why the fix is fragile:** The pattern requires every new comparable type to add a new boolean guard AND new match arms for EVERY operator. Missing even one arm for one operator on one type produces silent wrong results with no compiler error, no runtime error, and no test failure unless that exact type/operator combination is tested.

**After this section:** The dispatch table is the registry. Adding `Duration` comparison means adding `OpStrategy::IntInstr` to `DURATION.operators.cmp` in `ori_registry`. The `emit_binary_op` function has no type-specific guards to forget.

---

## 12.1 Replace emit_binary_op Type Guards with OpStrategy Dispatch

**THE KEY TRANSFORMATION.** This is what prevents the string ordering bug class permanently.

### Problem

`emit_binary_op` at `arc_emitter/mod.rs:1525-1611` currently uses two boolean guards (`is_float`, `is_str`) to select between three code paths (float instructions, string runtime calls, integer instructions) for ~15 binary operators. The function is 86 lines of nested match arms with guards, and the "integer instructions" fallthrough is actually the default for every unrecognized type.

The function receives `lhs_ty: Idx` from the call site (`emit_primop` at line 1509), which extracts it from `func.var_type(arc_args[0])`. This `Idx` is the key to the registry lookup.

### BEFORE (current code, 86 lines)

```rust
fn emit_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId, lhs_ty: Idx) -> ValueId {
    // Trait dispatch for non-primitive types (user-defined operator impls)
    if !lhs_ty.is_primitive() {
        if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
            return result;
        }
        if let Some(result) = self.emit_comparison_via_trait(op, lhs, rhs, lhs_ty) {
            return result;
        }
    }

    let is_float = matches!(self.type_info.get(lhs_ty), super::type_info::TypeInfo::Float);
    let is_str = matches!(self.type_info.get(lhs_ty), super::type_info::TypeInfo::Str);

    match op {
        BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
        BinaryOp::Add if is_str => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
        BinaryOp::Add => self.builder.add(lhs, rhs, "add"),
        BinaryOp::Sub if is_float => self.builder.fsub(lhs, rhs, "sub"),
        BinaryOp::Sub => self.builder.sub(lhs, rhs, "sub"),
        BinaryOp::Mul if is_float => self.builder.fmul(lhs, rhs, "mul"),
        BinaryOp::Mul => self.builder.mul(lhs, rhs, "mul"),
        BinaryOp::Div if is_float => self.builder.fdiv(lhs, rhs, "div"),
        BinaryOp::Div => self.builder.sdiv(lhs, rhs, "div"),
        BinaryOp::Mod if is_float => self.builder.frem(lhs, rhs, "rem"),
        BinaryOp::Mod => self.builder.srem(lhs, rhs, "rem"),
        BinaryOp::Eq if is_float => self.builder.fcmp_oeq(lhs, rhs, "eq"),
        BinaryOp::Eq if is_str => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
        BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
        BinaryOp::NotEq if is_float => self.builder.fcmp_one(lhs, rhs, "ne"),
        BinaryOp::NotEq if is_str => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
        BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
        BinaryOp::Lt if is_float => self.builder.fcmp_olt(lhs, rhs, "lt"),
        BinaryOp::Lt if is_str => self.emit_str_cmp_predicate(lhs, rhs, CmpPredicate::Less)
            .unwrap_or_else(|| self.builder.icmp_slt(lhs, rhs, "lt")),
        BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
        BinaryOp::Gt if is_float => self.builder.fcmp_ogt(lhs, rhs, "gt"),
        BinaryOp::Gt if is_str => self.emit_str_cmp_predicate(lhs, rhs, CmpPredicate::Greater)
            .unwrap_or_else(|| self.builder.icmp_sgt(lhs, rhs, "gt")),
        BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
        BinaryOp::LtEq if is_float => self.builder.fcmp_ole(lhs, rhs, "le"),
        BinaryOp::LtEq if is_str => self.emit_str_cmp_predicate(lhs, rhs, CmpPredicate::LessOrEqual)
            .unwrap_or_else(|| self.builder.icmp_sle(lhs, rhs, "le")),
        BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
        BinaryOp::GtEq if is_float => self.builder.fcmp_oge(lhs, rhs, "ge"),
        BinaryOp::GtEq if is_str => self.emit_str_cmp_predicate(lhs, rhs, CmpPredicate::GreaterOrEqual)
            .unwrap_or_else(|| self.builder.icmp_sge(lhs, rhs, "ge")),
        BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
        BinaryOp::And => self.builder.and(lhs, rhs, "and"),
        BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
        BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
        BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
        BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
        BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
        BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
        BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
        BinaryOp::Coalesce => { /* ... */ }
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => { /* desugared */ }
    }
}
```

### AFTER (registry-driven, ~60 lines)

```rust
fn emit_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId, lhs_ty: Idx) -> ValueId {
    // Trait dispatch for non-primitive types (user-defined operator impls).
    // Non-primitives use compiled method functions, not OpStrategy.
    if !lhs_ty.is_primitive() {
        if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
            return result;
        }
        if let Some(result) = self.emit_comparison_via_trait(op, lhs, rhs, lhs_ty) {
            return result;
        }
    }

    // Registry-driven dispatch for primitive/builtin types.
    let type_tag = self.idx_to_type_tag(lhs_ty);
    let strategy = self.op_strategy_for_binary(type_tag, op);

    match strategy {
        OpStrategy::IntInstr => self.emit_int_binary_op(op, lhs, rhs),
        OpStrategy::FloatInstr => self.emit_float_binary_op(op, lhs, rhs),
        OpStrategy::UnsignedCmp => self.emit_unsigned_binary_op(op, lhs, rhs),
        OpStrategy::BoolInstr => self.emit_bool_binary_op(op, lhs, rhs),
        OpStrategy::RuntimeCall { fn_name } => {
            self.emit_runtime_binary_op(fn_name, op, lhs, rhs, lhs_ty)
        }
        OpStrategy::Unsupported => {
            tracing::warn!(?op, ?type_tag, "binary op on type with no OpStrategy");
            self.builder.const_i64(0)
        }
    }
}
```

### New helper functions

Each strategy branch delegates to a focused helper that contains the `match op` for that instruction family. These helpers already partially exist (the match arms are currently inline in `emit_binary_op`) and are extracted verbatim:

```rust
/// Emit a binary op using signed integer LLVM instructions.
fn emit_int_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
    match op {
        BinaryOp::Add => self.builder.add(lhs, rhs, "add"),
        BinaryOp::Sub => self.builder.sub(lhs, rhs, "sub"),
        BinaryOp::Mul => self.builder.mul(lhs, rhs, "mul"),
        BinaryOp::Div => self.builder.sdiv(lhs, rhs, "div"),
        BinaryOp::Mod => self.builder.srem(lhs, rhs, "rem"),
        BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
        BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
        BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
        BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
        BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
        BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
        BinaryOp::And => self.builder.and(lhs, rhs, "and"),
        BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
        BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
        BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
        BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
        BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
        BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
        BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
        BinaryOp::Coalesce => self.emit_coalesce(lhs, rhs),
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
            tracing::warn!(?op, "desugared op in binary expression");
            self.builder.const_i64(0)
        }
    }
}

/// Emit a binary op using floating-point LLVM instructions.
fn emit_float_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
    match op {
        BinaryOp::Add => self.builder.fadd(lhs, rhs, "add"),
        BinaryOp::Sub => self.builder.fsub(lhs, rhs, "sub"),
        BinaryOp::Mul => self.builder.fmul(lhs, rhs, "mul"),
        BinaryOp::Div => self.builder.fdiv(lhs, rhs, "div"),
        BinaryOp::Mod => self.builder.frem(lhs, rhs, "rem"),
        BinaryOp::Eq => self.builder.fcmp_oeq(lhs, rhs, "eq"),
        BinaryOp::NotEq => self.builder.fcmp_one(lhs, rhs, "ne"),
        BinaryOp::Lt => self.builder.fcmp_olt(lhs, rhs, "lt"),
        BinaryOp::Gt => self.builder.fcmp_ogt(lhs, rhs, "gt"),
        BinaryOp::LtEq => self.builder.fcmp_ole(lhs, rhs, "le"),
        BinaryOp::GtEq => self.builder.fcmp_oge(lhs, rhs, "ge"),
        _ => {
            tracing::warn!(?op, "unsupported float binary op");
            self.builder.const_i64(0)
        }
    }
}

/// Emit a binary op using unsigned integer comparison instructions.
///
/// Used for bool, byte, char where comparison is unsigned but arithmetic
/// may still use signed instructions (or be unsupported).
fn emit_unsigned_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
    match op {
        BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
        BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
        BinaryOp::Lt => self.builder.icmp_ult(lhs, rhs, "lt"),
        BinaryOp::Gt => self.builder.icmp_ugt(lhs, rhs, "gt"),
        BinaryOp::LtEq => self.builder.icmp_ule(lhs, rhs, "le"),
        BinaryOp::GtEq => self.builder.icmp_uge(lhs, rhs, "ge"),
        BinaryOp::And => self.builder.and(lhs, rhs, "and"),
        BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
        _ => {
            tracing::warn!(?op, "unsupported unsigned binary op");
            self.builder.const_i64(0)
        }
    }
}

/// Emit a binary op via runtime function call (string concat, eq, ne, compare).
fn emit_runtime_binary_op(
    &mut self,
    base_fn: &str,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
    lhs_ty: Idx,
) -> ValueId {
    match op {
        BinaryOp::Add => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
        BinaryOp::Eq => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
        BinaryOp::NotEq => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
        BinaryOp::Lt => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
            .unwrap_or_else(|| self.builder.const_i64(0)),
        BinaryOp::Gt => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Greater)
            .unwrap_or_else(|| self.builder.const_i64(0)),
        BinaryOp::LtEq => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::LessOrEqual)
            .unwrap_or_else(|| self.builder.const_i64(0)),
        BinaryOp::GtEq => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::GreaterOrEqual)
            .unwrap_or_else(|| self.builder.const_i64(0)),
        _ => {
            tracing::warn!(?op, base_fn, "unsupported runtime binary op");
            self.builder.const_i64(0)
        }
    }
}
```

### op_strategy_for_binary helper

Reads the `OpDefs` from the registry `TypeDef` and selects the correct `OpStrategy` field for the given `BinaryOp`:

```rust
/// Look up the OpStrategy for a binary operation on a builtin type.
fn op_strategy_for_binary(&self, type_tag: TypeTag, op: BinaryOp) -> OpStrategy {
    let Some(type_def) = ori_registry::find_type(type_tag) else {
        return OpStrategy::Unsupported;
    };
    match op {
        BinaryOp::Add => type_def.operators.add,
        BinaryOp::Sub => type_def.operators.sub,
        BinaryOp::Mul => type_def.operators.mul,
        BinaryOp::Div => type_def.operators.div,
        BinaryOp::Mod => type_def.operators.rem,
        BinaryOp::Eq | BinaryOp::NotEq => type_def.operators.eq,
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq => {
            type_def.operators.cmp
        }
        // Logical/bitwise/shift: always IntInstr (only valid on int/bool).
        BinaryOp::And | BinaryOp::Or => type_def.operators.eq, // bool ops
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
        | BinaryOp::Shl | BinaryOp::Shr | BinaryOp::FloorDiv => {
            type_def.operators.add // int ops
        }
        BinaryOp::Coalesce => OpStrategy::IntInstr, // structural, not type-dependent
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
            OpStrategy::Unsupported // desugared before reaching ARC IR
        }
    }
}
```

### Implementation steps

- [ ] Add `ori_registry` dependency to `ori_llvm/Cargo.toml` (may already exist from Section 11)
- [ ] Implement `idx_to_type_tag()` (see 12.2)
- [ ] Implement `op_strategy_for_binary()` as shown above
- [ ] Extract `emit_int_binary_op()` from the current match arms
- [ ] Extract `emit_float_binary_op()` from the current match arms
- [ ] Extract `emit_unsigned_binary_op()` (new; currently bool/byte/char fall through to int signed ops)
- [ ] Extract `emit_bool_binary_op()` (subset of int with logical ops)
- [ ] Extract `emit_runtime_binary_op()` from the current `is_str` arms
- [ ] Extract `emit_coalesce()` from the current `Coalesce` arm
- [ ] Rewrite `emit_binary_op()` to use the dispatch pattern shown above
- [ ] Delete `is_float` and `is_str` local variables
- [ ] Verify: `cargo cll` (LLVM clippy) passes
- [ ] Verify: `./llvm-test.sh` passes with identical LLVM IR output

### Critical detail: UnsignedCmp for bool/byte/char

The current code does not distinguish signed vs unsigned comparison for all primitive types. The `is_float` guard handles float, and everything else falls through to signed integer ops (`icmp_slt`, etc.). However, `builtins/traits.rs` already correctly uses unsigned comparison for `bool`, `byte`, and `char` in the *trait method path* (`emit_comparison_predicate` dispatches to `emit_unsigned_predicate`).

After this transformation, the *binary operator path* (`emit_binary_op`) must also use unsigned comparison for `bool`, `byte`, and `char`. This is a correctness improvement: currently `byte_a < byte_b` via the operator path would use signed comparison (`icmp slt`) while `byte_a.is_less(byte_b)` via the trait path would correctly use unsigned comparison (`icmp ult`). The registry unifies this: `BYTE.operators.cmp = OpStrategy::UnsignedCmp`.

### Critical detail: Coalesce is structural, not type-driven

`BinaryOp::Coalesce` (`??`) operates on the Option/Result tag regardless of the inner type. It extracts field 0 (tag), field 1 (payload), and selects between payload and the default value. This is not type-dependent in the usual sense. After the transformation, `Coalesce` maps to `OpStrategy::IntInstr` (it uses `icmp eq` and `select` on i64 values) and is handled in `emit_int_binary_op` or stays as a separate inline block.

---

## 12.2 Idx-to-TypeTag Bridge for LLVM

### Problem

The LLVM backend works with `Idx` (type pool indices from `ori_types`). The registry uses `TypeTag` (a small enum defined in `ori_registry`). The `emit_binary_op` function receives `lhs_ty: Idx` and needs to look up `ori_registry::find_type(TypeTag)`.

The mapping from `Idx` to `TypeTag` is straightforward for primitive types (fixed indices 0-11), but dynamic types need the `Pool` + `Tag` to determine the type category.

### Current partial mapping

The `TypeInfo` enum in `type_info/mod.rs` already performs this classification. The `TypeInfoStore::get(idx)` method returns a `TypeInfo` variant that directly corresponds to a `TypeTag`. The `builtin_type_name()` method on `TypeInfo` (lines 286-313) maps `TypeInfo` variants to string names like `"int"`, `"float"`, `"str"` — these are the same names used as `TypeTag` discriminants.

### Solution: idx_to_type_tag() function

```rust
/// Map a type pool `Idx` to a registry `TypeTag` for OpStrategy lookup.
///
/// This is the bridge between the type checker's pool-based type system
/// and the registry's static type tag system. For primitive types (Idx 0-11),
/// the mapping is a direct match on the well-known index constants.
/// For dynamic types, we consult the `TypeInfoStore`.
fn idx_to_type_tag(&self, idx: Idx) -> TypeTag {
    // Fast path: well-known primitive indices
    match idx {
        Idx::INT => TypeTag::Int,
        Idx::FLOAT => TypeTag::Float,
        Idx::BOOL => TypeTag::Bool,
        Idx::STR => TypeTag::Str,
        Idx::CHAR => TypeTag::Char,
        Idx::BYTE => TypeTag::Byte,
        Idx::UNIT => TypeTag::Unit,
        Idx::NEVER => TypeTag::Never,
        Idx::DURATION => TypeTag::Duration,
        Idx::SIZE => TypeTag::Size,
        Idx::ORDERING => TypeTag::Ordering,
        _ => {
            // Dynamic types: consult TypeInfoStore
            match self.type_info.get(idx) {
                TypeInfo::Int => TypeTag::Int,
                TypeInfo::Float => TypeTag::Float,
                TypeInfo::Bool => TypeTag::Bool,
                TypeInfo::Char => TypeTag::Char,
                TypeInfo::Byte => TypeTag::Byte,
                TypeInfo::Str => TypeTag::Str,
                TypeInfo::Duration => TypeTag::Duration,
                TypeInfo::Size => TypeTag::Size,
                TypeInfo::Ordering => TypeTag::Ordering,
                TypeInfo::List { .. } => TypeTag::List,
                TypeInfo::Map { .. } => TypeTag::Map,
                TypeInfo::Set { .. } => TypeTag::Set,
                TypeInfo::Tuple { .. } => TypeTag::Tuple,
                TypeInfo::Option { .. } => TypeTag::Option,
                TypeInfo::Result { .. } => TypeTag::Result,
                TypeInfo::Range => TypeTag::Range,
                TypeInfo::Iterator { .. } => TypeTag::Iterator,
                _ => TypeTag::Unknown,
            }
        }
    }
}
```

### Implementation steps

- [ ] Add `TypeTag` import from `ori_registry` to `arc_emitter/mod.rs`
- [ ] Implement `idx_to_type_tag()` as a method on `ArcIrEmitter`
- [ ] Add `TypeTag::Unknown` variant to `ori_registry` if not already present (for unrecognized dynamic types)
- [ ] Unit test: every `Idx::*` constant maps to the expected `TypeTag`
- [ ] Unit test: dynamic types (constructed via `Pool`) map correctly

### Performance note

The fast path (primitive `Idx` constants 0-11) is a single match on `u32` — zero overhead vs the current `matches!(self.type_info.get(lhs_ty), TypeInfo::Float)` pattern which also calls `type_info.get()`. The dynamic path has the same cost as the current code. Net performance: neutral or slightly better (one `type_info.get()` call instead of two separate `is_float`/`is_str` calls).

---

## 12.3 Replace emit_unary_op Type Guards with OpStrategy Dispatch

### Problem

`emit_unary_op` at `arc_emitter/mod.rs:1618-1645` has the same pattern as `emit_binary_op`:

```rust
fn emit_unary_op(&mut self, op: UnaryOp, operand: ValueId, operand_ty: Idx) -> ValueId {
    if !operand_ty.is_primitive() {
        if let Some(result) = self.emit_unary_op_via_trait(op, operand, operand_ty) {
            return result;
        }
    }

    let is_float = matches!(
        self.type_info.get(operand_ty),
        super::type_info::TypeInfo::Float
    );

    match op {
        UnaryOp::Neg if is_float => self.builder.fneg(operand, "neg"),
        UnaryOp::Neg => self.builder.neg(operand, "neg"),
        UnaryOp::Not => self.builder.not(operand, "not"),
        UnaryOp::BitNot => {
            let all_ones = self.builder.const_i64(-1);
            self.builder.xor(operand, all_ones, "bitnot")
        }
        UnaryOp::Try => { /* desugared */ }
    }
}
```

### AFTER

```rust
fn emit_unary_op(&mut self, op: UnaryOp, operand: ValueId, operand_ty: Idx) -> ValueId {
    if !operand_ty.is_primitive() {
        if let Some(result) = self.emit_unary_op_via_trait(op, operand, operand_ty) {
            return result;
        }
    }

    let type_tag = self.idx_to_type_tag(operand_ty);
    let strategy = self.op_strategy_for_unary(type_tag, op);

    match strategy {
        OpStrategy::IntInstr => match op {
            UnaryOp::Neg => self.builder.neg(operand, "neg"),
            UnaryOp::Not => self.builder.not(operand, "not"),
            UnaryOp::BitNot => {
                let all_ones = self.builder.const_i64(-1);
                self.builder.xor(operand, all_ones, "bitnot")
            }
            UnaryOp::Try => {
                tracing::warn!("try op in unary expression");
                self.builder.const_i64(0)
            }
        },
        OpStrategy::FloatInstr => match op {
            UnaryOp::Neg => self.builder.fneg(operand, "neg"),
            _ => {
                tracing::warn!(?op, "unsupported float unary op");
                self.builder.const_i64(0)
            }
        },
        _ => {
            tracing::warn!(?op, ?type_tag, "unary op on type with no unary OpStrategy");
            self.builder.const_i64(0)
        }
    }
}
```

### op_strategy_for_unary helper

```rust
fn op_strategy_for_unary(&self, type_tag: TypeTag, op: UnaryOp) -> OpStrategy {
    let Some(type_def) = ori_registry::find_type(type_tag) else {
        return OpStrategy::Unsupported;
    };
    match op {
        UnaryOp::Neg => type_def.operators.neg,
        UnaryOp::Not | UnaryOp::BitNot => type_def.operators.add, // int-flavored
        UnaryOp::Try => OpStrategy::Unsupported, // desugared
    }
}
```

### Implementation steps

- [ ] Implement `op_strategy_for_unary()` as a method on `ArcIrEmitter`
- [ ] Rewrite `emit_unary_op()` to use strategy dispatch
- [ ] Delete `is_float` local variable
- [ ] Verify: `cargo cll` passes
- [ ] Verify: `./llvm-test.sh` passes

---

## 12.4 Remove receiver_borrowed from BuiltinRegistration

### Problem

`BuiltinRegistration` in `builtins/mod.rs:107-117` has a `receiver_borrowed: bool` field:

```rust
pub(crate) struct BuiltinRegistration {
    pub type_name: &'static str,
    pub method_name: &'static str,
    pub receiver_borrowed: bool,
}
```

This field is consumed by `borrowing_builtin_names()` (lines 266-286) to build a `FxHashSet<Name>` of methods that borrow their receiver. After Section 11, this information comes from `ori_registry` instead. The field and all 164 `borrow: true`/`borrow: false` annotations across 7 submodules become dead code.

### Affected files and entry counts

| File | Entries | Current Pattern |
|------|---------|-----------------|
| `primitives.rs` | 25 entries | `("int", "clone", borrow: true)` |
| `collections.rs` | 21 entries | `("str", "clone", borrow: true)` |
| `traits.rs` | 73 entries | `("int", "equals", borrow: true)` |
| `compound_traits.rs` | 16 entries | `("list", "equals", borrow: true)` |
| `option_result.rs` | 11 entries | `("Option", "is_some", borrow: true)` |
| `iterator.rs` | 15 entries | `("Iterator", "__iter_next", borrow: true)` |
| `trampolines.rs` | 0 entries | empty `declare_builtins! {}` |
| **Total** | **161 entries** | all `borrow: true` |

Note: The `mod.rs` macro definition and the `BuiltinRegistration` struct definition account for 3 additional occurrences of `borrow:` in the file (total 164 across all files).

### Complete entry list by submodule

**primitives.rs (25 entries):**
1. `("int", "clone", borrow: true)`
2. `("int", "to_int", borrow: true)`
3. `("int", "byte", borrow: true)`
4. `("int", "f", borrow: true)`
5. `("int", "to_float", borrow: true)`
6. `("int", "into", borrow: true)`
7. `("int", "to_str", borrow: true)`
8. `("int", "abs", borrow: true)`
9. `("float", "clone", borrow: true)`
10. `("float", "to_int", borrow: true)`
11. `("float", "to_str", borrow: true)`
12. `("float", "abs", borrow: true)`
13. `("bool", "clone", borrow: true)`
14. `("bool", "to_int", borrow: true)`
15. `("bool", "to_str", borrow: true)`
16. `("char", "clone", borrow: true)`
17. `("char", "to_int", borrow: true)`
18. `("byte", "clone", borrow: true)`
19. `("byte", "to_int", borrow: true)`
20. `("Duration", "clone", borrow: true)`
21. `("Duration", "to_str", borrow: true)`
22. `("Size", "clone", borrow: true)`
23. `("Size", "to_str", borrow: true)`
24. `("Ordering", "clone", borrow: true)`
25. `("Ordering", "to_int", borrow: true)`

**collections.rs (21 entries):**
1. `("str", "clone", borrow: true)`
2. `("str", "length", borrow: true)`
3. `("str", "len", borrow: true)`
4. `("str", "is_empty", borrow: true)`
5. `("str", "concat", borrow: true)`
6. `("str", "to_str", borrow: true)`
7. `("str", "iter", borrow: true)`
8. `("list", "clone", borrow: true)`
9. `("list", "length", borrow: true)`
10. `("list", "len", borrow: true)`
11. `("list", "is_empty", borrow: true)`
12. `("list", "iter", borrow: true)`
13. `("map", "clone", borrow: true)`
14. `("map", "length", borrow: true)`
15. `("map", "len", borrow: true)`
16. `("map", "iter", borrow: true)`
17. `("Set", "clone", borrow: true)`
18. `("Set", "length", borrow: true)`
19. `("Set", "len", borrow: true)`
20. `("Set", "iter", borrow: true)`
21. `("range", "iter", borrow: true)`

**traits.rs (73 entries):**
Scalar trait methods for 7 types (int, float, bool, char, byte, Duration, Size) x 8 methods (equals, is_equal, compare, hash, is_less, is_greater, is_less_or_equal, is_greater_or_equal) = 56 entries (not all types have all methods; int/float/bool/char/byte have all 8, Duration/Size have all 8).
Plus 8 string trait methods (str: equals, is_equal, compare, hash, is_less, is_greater, is_less_or_equal, is_greater_or_equal).
Plus 9 Ordering methods (equals, compare, hash, is_less, is_equal, is_greater, is_less_or_equal, is_greater_or_equal, reverse).
Total: 73 entries (verified against file).

**compound_traits.rs (16 entries):**
1-4: `("list", "equals"/"is_equal"/"compare"/"hash", borrow: true)`
5-8: `("Option", "equals"/"is_equal"/"compare"/"hash", borrow: true)`
9-12: `("Result", "equals"/"is_equal"/"compare"/"hash", borrow: true)`
13: `("tuple", "clone", borrow: true)`
14-16: `("tuple", "equals"/"compare"/"hash", borrow: true)`

**option_result.rs (11 entries):**
1-5: `("Option", "is_some"/"is_none"/"unwrap"/"unwrap_or"/"clone", borrow: true)`
6-11: `("Result", "is_ok"/"is_err"/"unwrap"/"unwrap_err"/"unwrap_or"/"clone", borrow: true)`

**iterator.rs (15 entries):**
1: `("Iterator", "__iter_next", borrow: true)`
2-6: `("Iterator", "take"/"skip"/"chain"/"enumerate"/"zip", borrow: true)`
7-8: `("Iterator", "map"/"filter", borrow: true)`
9-15: `("Iterator", "collect"/"count"/"any"/"all"/"find"/"for_each"/"fold", borrow: true)`

### Implementation steps

- [ ] Remove `receiver_borrowed` field from `BuiltinRegistration` struct in `builtins/mod.rs`
- [ ] Update `declare_builtins!` macro definition — see 12.5
- [ ] Update all 161 entries across 6 submodules (remove `, borrow: true`)
- [ ] Remove all references to `receiver_borrowed` in `BuiltinTable` and related code
- [ ] Verify: `cargo cll` passes

---

## 12.5 Simplify declare_builtins! Macro

### BEFORE (current macro definition, `builtins/mod.rs:54-75`)

```rust
macro_rules! declare_builtins {
    ($emitter:ident, $ctx:ident; $(
        ($type_name:expr, $method:expr, borrow: $borrow:expr) => $body:expr
    ),* $(,)?) => {
        #[allow(dead_code, unused_variables, ...)]
        pub(super) fn dispatch<'scx: 'ctx, 'ctx>(
            $emitter: &mut $crate::codegen::arc_emitter::ArcIrEmitter<'_, 'scx, 'ctx, '_>,
            $ctx: &super::BuiltinCtx<'_>,
        ) -> Option<$crate::codegen::value_id::ValueId> {
            match ($ctx.type_name, $ctx.method) {
                $(($type_name, $method) => $body,)*
                _ => None,
            }
        }

        pub(super) const REGISTERED: &[super::BuiltinRegistration] = &[
            $(super::BuiltinRegistration {
                type_name: $type_name,
                method_name: $method,
                receiver_borrowed: $borrow,
            },)*
        ];
    };
}
```

### AFTER

```rust
macro_rules! declare_builtins {
    ($emitter:ident, $ctx:ident; $(
        ($type_name:expr, $method:expr) => $body:expr
    ),* $(,)?) => {
        #[allow(dead_code, unused_variables, ...)]
        pub(super) fn dispatch<'scx: 'ctx, 'ctx>(
            $emitter: &mut $crate::codegen::arc_emitter::ArcIrEmitter<'_, 'scx, 'ctx, '_>,
            $ctx: &super::BuiltinCtx<'_>,
        ) -> Option<$crate::codegen::value_id::ValueId> {
            match ($ctx.type_name, $ctx.method) {
                $(($type_name, $method) => $body,)*
                _ => None,
            }
        }

        pub(super) const REGISTERED: &[super::BuiltinRegistration] = &[
            $(super::BuiltinRegistration {
                type_name: $type_name,
                method_name: $method,
            },)*
        ];
    };
}
```

### BEFORE (entry syntax)

```rust
("str", "length", borrow: true) => emitter.emit_str_length(ctx.arg_vals[0]),
```

### AFTER (entry syntax)

```rust
("str", "length") => emitter.emit_str_length(ctx.arg_vals[0]),
```

### Implementation steps

- [ ] Update `macro_rules! declare_builtins!` — remove `borrow: $borrow:expr` from pattern, remove `receiver_borrowed: $borrow` from `BuiltinRegistration` construction
- [ ] Update `primitives.rs`: remove `, borrow: true` from 25 entries
- [ ] Update `collections.rs`: remove `, borrow: true` from 21 entries
- [ ] Update `traits.rs`: remove `, borrow: true` from 73 entries
- [ ] Update `compound_traits.rs`: remove `, borrow: true` from 16 entries
- [ ] Update `option_result.rs`: remove `, borrow: true` from 11 entries
- [ ] Update `iterator.rs`: remove `, borrow: true` from 15 entries
- [ ] Update `trampolines.rs`: no entries to change (empty declaration), but verify the empty macro invocation compiles
- [ ] Verify: `cargo cll` passes

### Mechanical transformation

This is a pure find-and-replace operation. The regex for the entry change is:

```
(, borrow: (true|false))
```

Replace with empty string. Every entry in the codebase currently uses `borrow: true` (verified: all 161 entries). No entries use `borrow: false`.

---

## 12.6 Delete borrowing_builtin_names() Function

### Current location

`builtins/mod.rs:266-286`:

```rust
pub fn borrowing_builtin_names(interner: &ori_ir::StringInterner) -> rustc_hash::FxHashSet<Name> {
    let table = builtin_table();
    let mut names = rustc_hash::FxHashSet::default();
    for (&type_name, methods) in &table.entries {
        if type_name == "Iterator" {
            continue;
        }
        for (&method_name, reg) in methods {
            if !reg.receiver_borrowed {
                continue;
            }
            if method_name == "iter" {
                continue;
            }
            names.insert(interner.intern(method_name));
        }
    }
    names
}
```

### Callers (2 call sites)

1. `evaluator.rs:377` — `crate::codegen::arc_emitter::borrowing_builtin_names(interner)`
2. `function_compiler/mod.rs:106` — `crate::codegen::arc_emitter::borrowing_builtin_names(interner)`

### Re-export

`arc_emitter/mod.rs:17`:
```rust
pub use builtins::borrowing_builtin_names;
```

### Prerequisites

Section 11 must be complete first. Section 11 replaces these call sites with `ori_registry`-based queries. After Section 11, these callers will have been changed to:

```rust
let borrowing_builtins = ori_registry::borrowing_method_names(interner);
```

Or equivalent registry query. Only after the callers are migrated can this function be deleted.

### Implementation steps

- [ ] Verify: no remaining callers of `borrowing_builtin_names()` (grep confirmation)
- [ ] Delete the function body at `builtins/mod.rs:266-286`
- [ ] Delete the re-export at `arc_emitter/mod.rs:17` (`pub use builtins::borrowing_builtin_names;`)
- [ ] If `receiver_borrowed` was the only reason `BuiltinTable.entries` exposed registration details, simplify `BuiltinTable` accordingly
- [ ] Verify: `cargo cll` passes

---

## 12.7 ARC_PIPELINE_METHODS Migration

### Current state

`builtins/tests.rs:45-49`:

```rust
const ARC_PIPELINE_METHODS: &[(&str, &str)] = &[
    ("Iterator", "__iter_next"),
    ("Ordering", "to_int"),
    ("int", "to_int"),
];
```

These are methods that reach the `BuiltinTable` dispatch through codegen paths other than `TYPECK_BUILTIN_METHODS` — they are valid entries that would otherwise be flagged as "phantom" by `no_phantom_builtin_entries`.

### Decision

`ARC_PIPELINE_METHODS` is about **codegen routing** (which methods are emitted by the ARC lowering pipeline vs the builtin method dispatch path), not about **type behavior**. This is a codegen-internal concern, not a registry concern.

**Keep as test-local knowledge.** After the registry migration, the phantom test should still verify that every `BuiltinTable` entry has backing in either the registry or the `ARC_PIPELINE_METHODS` list. The migration changes the verification source (registry instead of `TYPECK_BUILTIN_METHODS`) but keeps the ARC pipeline exception list.

### Updated test pattern

```rust
#[test]
fn no_phantom_builtin_entries() {
    let table = builtin_table();
    let arc_pipeline: HashSet<(&str, &str)> = ARC_PIPELINE_METHODS.iter().copied().collect();

    let mut phantom = Vec::new();
    for (type_name, method_name) in table.all_registered() {
        // Direct registry match: method exists in ori_registry
        if ori_registry::find_method_by_name(type_name, method_name).is_some() {
            continue;
        }
        // ARC-pipeline method (codegen-internal)
        if arc_pipeline.contains(&(type_name, method_name)) {
            continue;
        }
        phantom.push(format!("  ({type_name}, {method_name})"));
    }

    assert!(phantom.is_empty(), /* ... */);
}
```

### Implementation steps

- [ ] Update `no_phantom_builtin_entries` to verify against `ori_registry` instead of `TYPECK_BUILTIN_METHODS`
- [ ] Remove `CODEGEN_ALIASES` constant (the registry should use canonical names; if aliases are needed, the registry handles them)
- [ ] Remove `TRAIT_DISPATCH_METHODS` constant (trait methods are now in the registry as regular `MethodDef` entries)
- [ ] Keep `ARC_PIPELINE_METHODS` as codegen-internal test knowledge
- [ ] Update `builtin_coverage_above_threshold` test to compare against registry instead of `TYPECK_BUILTIN_METHODS`
- [ ] Verify: `cargo test -p ori_llvm` passes

---

## 12.8 BuiltinTable Registry Validation

### Purpose

New test: every entry in `BuiltinTable` must have a corresponding entry in the registry. This replaces the current `no_phantom_builtin_entries` test that compares against `TYPECK_BUILTIN_METHODS`.

The new test is strictly stronger: instead of checking that the method is known to the type checker, it checks that the method is declared in the single source of truth.

### Test: registry_covers_all_builtin_codegen

```rust
/// Every BuiltinTable entry must be backed by a registry MethodDef
/// or an explicit ARC-pipeline exception.
#[test]
fn registry_covers_all_builtin_codegen() {
    let table = builtin_table();
    let arc_pipeline: HashSet<(&str, &str)> = ARC_PIPELINE_METHODS.iter().copied().collect();

    let mut missing = Vec::new();
    for (type_name, method_name) in table.all_registered() {
        if arc_pipeline.contains(&(type_name, method_name)) {
            continue;
        }
        // Registry must know about this method
        let type_tag = ori_registry::type_tag_from_name(type_name);
        if type_tag.is_none() {
            missing.push(format!("  ({type_name}, {method_name}) — unknown type"));
            continue;
        }
        let type_def = ori_registry::find_type(type_tag.unwrap());
        if type_def.is_none() {
            missing.push(format!("  ({type_name}, {method_name}) — no TypeDef"));
            continue;
        }
        let has_method = type_def.unwrap().methods.iter().any(|m| m.name == method_name);
        if !has_method {
            missing.push(format!("  ({type_name}, {method_name}) — not in registry"));
        }
    }

    assert!(
        missing.is_empty(),
        "BuiltinTable has {} entries not backed by ori_registry:\n{}",
        missing.len(),
        missing.join("\n"),
    );
}
```

### Test: registry_op_strategies_cover_all_operators

```rust
/// Every type with operators in the registry must have its OpStrategy
/// handled by emit_binary_op's strategy dispatch.
#[test]
fn registry_op_strategies_cover_all_operators() {
    use ori_registry::{OpStrategy, BUILTIN_TYPES};

    let strategies_handled = [
        OpStrategy::IntInstr,
        OpStrategy::FloatInstr,
        OpStrategy::UnsignedCmp,
        OpStrategy::BoolInstr,
        // RuntimeCall checked separately per fn_name
    ];

    for type_def in BUILTIN_TYPES {
        let ops = &type_def.operators;
        for (field_name, strategy) in [
            ("add", ops.add),
            ("sub", ops.sub),
            ("mul", ops.mul),
            ("div", ops.div),
            ("rem", ops.rem),
            ("eq", ops.eq),
            ("cmp", ops.cmp),
            ("neg", ops.neg),
        ] {
            match strategy {
                OpStrategy::Unsupported => {} // Fine, type doesn't support this op
                OpStrategy::RuntimeCall { fn_name } => {
                    // Verify the runtime function exists (will be checked at link time)
                    assert!(!fn_name.is_empty(), "{}.operators.{field_name} has empty RuntimeCall fn_name", type_def.name);
                }
                other => {
                    assert!(
                        strategies_handled.contains(&other),
                        "{}.operators.{field_name} has unhandled strategy {:?}",
                        type_def.name,
                        other,
                    );
                }
            }
        }
    }
}
```

### Implementation steps

- [ ] Write `registry_covers_all_builtin_codegen` test in `builtins/tests.rs`
- [ ] Write `registry_op_strategies_cover_all_operators` test in `builtins/tests.rs`
- [ ] Delete or replace `no_phantom_builtin_entries` (superseded)
- [ ] Delete or replace `builtin_coverage_above_threshold` (superseded; registry is 100% authoritative)
- [ ] Verify: `cargo test -p ori_llvm` passes

---

## 12.9 Validation & Regression

### Build verification

- [ ] `cargo c -p ori_llvm` (standard check)
- [ ] `cargo cll` (clippy with LLVM feature)
- [ ] `cargo bl` (debug build: oric + ori_rt)
- [ ] `cargo blr` (release build: oric + ori_rt)

### Test verification

- [ ] `./llvm-test.sh` (LLVM unit tests)
- [ ] `./test-all.sh` (full test suite)
- [ ] `cargo test -p ori_llvm` (all ori_llvm tests including the new registry validation tests)

### Specific regression targets

The following tests must produce **identical** LLVM IR before and after this change. The transformation is a refactor of dispatch structure, not a change in emitted instructions.

**String comparison tests** (the original bug class):
- [ ] `compiler/ori_llvm/tests/aot/strings.rs` — string equality, ordering, concatenation
- [ ] `compiler/ori_llvm/tests/aot/conversions.rs` — string conversion paths
- [ ] Any spec tests in `tests/spec/` that exercise string `<`, `>`, `<=`, `>=`

**Float comparison tests:**
- [ ] Float equality (`==`, `!=`) produces `fcmp oeq`/`fcmp one`
- [ ] Float ordering (`<`, `>`, `<=`, `>=`) produces `fcmp olt`/`fcmp ogt`/`fcmp ole`/`fcmp oge`

**Integer comparison tests:**
- [ ] Signed comparison for `int` produces `icmp slt`/`icmp sgt`/`icmp sle`/`icmp sge`
- [ ] Unsigned comparison for `byte` produces `icmp ult`/`icmp ugt`/`icmp ule`/`icmp uge`

**Operator trait dispatch tests** (non-primitive types must still work):
- [ ] User-defined struct with `Add`/`Eq`/`Comparable` trait impls
- [ ] The `!lhs_ty.is_primitive()` guard is preserved, so non-primitive types still use `emit_binary_op_via_trait` and `emit_comparison_via_trait`

**Edge cases:**
- [ ] `Coalesce` (`??`) still works on `Option` and `Result` types
- [ ] `Range`/`RangeInclusive`/`MatMul` still produce the warning + zero fallback
- [ ] `Duration` and `Size` operators use `IntInstr` (they are i64 under the hood)
- [ ] `Ordering` type does not have arithmetic operators (only Eq/Comparable)

### Grep verification (post-migration)

After all steps are complete, these greps must return **zero results** in `arc_emitter/mod.rs`:

```bash
# No is_float/is_str guards in emit_binary_op or emit_unary_op
grep -n 'is_float\|is_str' compiler/ori_llvm/src/codegen/arc_emitter/mod.rs
# Result: 0 matches (all type discrimination is via OpStrategy)

# No receiver_borrowed in BuiltinRegistration
grep -rn 'receiver_borrowed' compiler/ori_llvm/src/codegen/arc_emitter/
# Result: 0 matches

# No borrowing_builtin_names function or re-export
grep -rn 'borrowing_builtin_names' compiler/ori_llvm/src/
# Result: 0 matches
```

And this grep must return results confirming the new pattern:

```bash
# OpStrategy dispatch is in place
grep -n 'OpStrategy' compiler/ori_llvm/src/codegen/arc_emitter/mod.rs
# Result: matches in emit_binary_op and emit_unary_op

# idx_to_type_tag bridge exists
grep -n 'idx_to_type_tag' compiler/ori_llvm/src/codegen/arc_emitter/mod.rs
# Result: definition + call sites in emit_binary_op, emit_unary_op
```

### Release binary verification

Per LLVM backend rules, debug and release can differ due to FastISel behavior. Both must be tested:

- [ ] `cargo bl && ./test-all.sh` (debug)
- [ ] `cargo blr && ./test-all.sh` (release)

---

## Exit Criteria

All of the following must be true before this section is marked complete:

- [ ] **No `is_float`/`is_str` guards in `emit_binary_op`** — OpStrategy dispatch is the only type discrimination path
- [ ] **No `is_float` guard in `emit_unary_op`** — OpStrategy dispatch handles float negation
- [ ] **`OpStrategy` dispatch in place** — `emit_binary_op` and `emit_unary_op` call `idx_to_type_tag()` and `op_strategy_for_binary()`/`op_strategy_for_unary()`
- [ ] **`receiver_borrowed` removed** from `BuiltinRegistration` and all 161 `declare_builtins!` entries
- [ ] **`declare_builtins!` simplified** — entry syntax is `("type", "method") => handler`
- [ ] **`borrowing_builtin_names()` deleted** — function and re-export removed, no callers remain
- [ ] **Registry validation tests passing** — `registry_covers_all_builtin_codegen` and `registry_op_strategies_cover_all_operators`
- [ ] **`./llvm-test.sh` passes** (debug and release)
- [ ] **`./test-all.sh` passes** (debug and release)
- [ ] **Grep verification clean** — no `is_float`/`is_str` in `emit_binary_op`/`emit_unary_op`, no `receiver_borrowed`, no `borrowing_builtin_names`
- [ ] **Net code deletion** — this section should delete more lines than it adds (the ad-hoc guards and `borrow:` annotations are replaced by registry lookups)

---

## Implementation Order

The subsections have the following dependency chain:

```
12.2 (Idx-to-TypeTag bridge)
  ↓
12.1 (emit_binary_op) ←─ requires 12.2
  ↓
12.3 (emit_unary_op) ←─ requires 12.2
  ↓
12.5 (declare_builtins! macro) ←─ must happen before 12.4
  ↓
12.4 (remove receiver_borrowed) ←─ requires 12.5
  ↓
12.6 (delete borrowing_builtin_names) ←─ requires 12.4, Section 11
  ↓
12.7 (ARC_PIPELINE_METHODS) ←─ can be parallel with 12.6
  ↓
12.8 (validation tests) ←─ requires 12.1, 12.4, 12.7
  ↓
12.9 (full validation) ←─ requires all above
```

Recommended execution: 12.2, then 12.1+12.3 together, then 12.5+12.4 together, then 12.6+12.7 together, then 12.8, then 12.9.
