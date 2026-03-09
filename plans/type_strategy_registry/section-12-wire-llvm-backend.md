---
plan: "type_strategy_registry"
section: "12"
title: "Wire LLVM Backend (ori_llvm) — OpStrategy Dispatch & Builtin Simplification"
status: complete
reviewed: true
depends_on:
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
  - "08"
  - "09"
  - "11"
subsections:
  - id: "12.1"
    title: "Replace emit_binary_op Type Guards with OpStrategy Dispatch"
    status: complete
  - id: "12.2"
    title: "Idx-to-TypeTag Bridge for LLVM"
    status: complete
  - id: "12.3"
    title: "Replace emit_unary_op Type Guards with OpStrategy Dispatch"
    status: complete
  - id: "12.4"
    title: "Remove receiver_borrowed from BuiltinRegistration"
    status: complete
  - id: "12.5"
    title: "Simplify declare_builtins! Macro"
    status: complete
  - id: "12.6"
    title: "Delete borrowing_names_from_table() Function"
    status: complete
  - id: "12.7"
    title: "ARC Pipeline Methods Migration"
    status: complete
  - id: "12.8"
    title: "BuiltinTable Registry Validation"
    status: complete
  - id: "12.9"
    title: "Validation & Regression"
    status: complete
---

# Section 12: Wire LLVM Backend (ori_llvm) — OpStrategy Dispatch & Builtin Simplification

**This is the section that eliminates the string comparison ordering bug class permanently.** The recent fix that added `is_str` guards for `Lt`, `Gt`, `LtEq`, `GtEq` in `emit_binary_op` was correct but brittle: the same bug class will reappear whenever a new comparable type is added (e.g., `Duration`, user-defined types with operator overloads on primitive-like representations). After this section, adding a new type's operator semantics is a registry entry, not a code change in `emit_binary_op`.

**Context:** Section 11 wires `ori_arc` to read ownership from the registry instead of from `ori_arc::borrowing_builtin_names()`. This section completes the downstream half of that work: the LLVM backend stops producing the ownership data (`receiver_borrowed`, test-only `borrowing_names_from_table()`) and instead consumes operator strategy data from the registry.

---

## The Bug This Section Prevents

**Root cause (historical):** `emit_binary_op` at `arc_emitter/operators.rs:21` dispatches binary operators using a cascade of `is_float`/`is_str` boolean guards:

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

**After this section:** The dispatch table is the registry. Adding `Duration` comparison means setting `OpStrategy::IntInstr` on each of `DURATION.operators.{lt, gt, lt_eq, gt_eq}` in `ori_registry`. The `emit_binary_op` function has no type-specific guards to forget.

---

## 12.1 Replace emit_binary_op Type Guards with OpStrategy Dispatch

**THE KEY TRANSFORMATION.** This is what prevents the string ordering bug class permanently.

### Problem

`emit_binary_op` at `arc_emitter/operators.rs:21-122` currently uses two boolean guards (`is_float`, `is_str`) to select between three code paths (float instructions, string runtime calls, integer instructions) for ~15 binary operators. The function is ~100 lines of nested match arms with guards (including a list-concat special case), and the "integer instructions" fallthrough is actually the default for every unrecognized type.

The function receives `lhs_ty: Idx` and `arc_func: &ArcFunction` from the call site (`emit_primop` in `value_emission.rs:87`), which extracts the type from `func.var_type(arc_args[0])`. This `Idx` is the key to the registry lookup.

### BEFORE (current code, ~100 lines, at `arc_emitter/operators.rs:21-122`)

```rust
pub(super) fn emit_binary_op(
    &mut self,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
    lhs_ty: Idx,
    arc_func: &ori_arc::ir::ArcFunction,
) -> ValueId {
    // Trait dispatch for non-primitive types (user-defined operator impls)
    if !lhs_ty.is_primitive() {
        if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
            return result;
        }
        if let Some(result) = self.emit_comparison_via_trait(op, lhs, rhs, lhs_ty) {
            return result;
        }
    }

    let type_info = self.type_info.get(lhs_ty);
    let is_float = matches!(type_info, super::super::type_info::TypeInfo::Float);
    let is_str = matches!(type_info, super::super::type_info::TypeInfo::Str);

    // List + list → concat (same as str + str → concat)
    if matches!(op, BinaryOp::Add) {
        if let super::super::type_info::TypeInfo::List { element } = type_info {
            let cm = self.cow_mode_const(arc_func);
            if let Some(val) = self.emit_list_concat_cow(lhs, rhs, element, cm) {
                return val;
            }
        }
    }

    match op {
        BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
        BinaryOp::Add if is_str => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
        BinaryOp::Add => self.builder.checked_add(lhs, rhs, "add"),
        BinaryOp::Sub if is_float => self.builder.fsub(lhs, rhs, "sub"),
        BinaryOp::Sub => self.builder.checked_sub(lhs, rhs, "sub"),
        BinaryOp::Mul if is_float => self.builder.fmul(lhs, rhs, "mul"),
        BinaryOp::Mul => self.builder.checked_mul(lhs, rhs, "mul"),
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
        BinaryOp::Lt if is_str => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
            .unwrap_or_else(|| self.builder.icmp_slt(lhs, rhs, "lt")),
        BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
        // ... (Gt, LtEq, GtEq follow the same pattern)
        BinaryOp::And => self.builder.and(lhs, rhs, "and"),
        BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
        BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
        BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
        BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
        BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
        BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
        BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
        BinaryOp::Coalesce => { /* extract tag/payload, select between payload and rhs */ }
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => { /* warning + zero */ }
    }
}
```
**Note**: The AFTER code must also handle (1) the `arc_func` parameter for list-concat COW, (2) `checked_add`/`checked_sub`/`checked_mul` for integer overflow detection, and (3) the list-concat special case which is type-info-driven, not just `is_float`/`is_str`.

### AFTER (registry-driven, ~70 lines)

```rust
pub(super) fn emit_binary_op(
    &mut self,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
    lhs_ty: Idx,
    arc_func: &ori_arc::ir::ArcFunction,
) -> ValueId {
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

    // List + list → COW concat (type-info-driven, not OpStrategy)
    if matches!(op, BinaryOp::Add) {
        if let super::super::type_info::TypeInfo::List { element } = self.type_info.get(lhs_ty) {
            let cm = self.cow_mode_const(arc_func);
            if let Some(val) = self.emit_list_concat_cow(lhs, rhs, element, cm) {
                return val;
            }
        }
    }

    // Registry-driven dispatch for primitive/builtin types.
    let Some(type_tag) = self.idx_to_type_tag(lhs_ty) else {
        unreachable!("binary op {op:?} on unmapped type idx {lhs_ty:?} — should have used trait dispatch");
    };
    let strategy = self.op_strategy_for_binary(type_tag, op);

    match strategy {
        OpStrategy::IntInstr => self.emit_int_binary_op(op, lhs, rhs),
        OpStrategy::FloatInstr => self.emit_float_binary_op(op, lhs, rhs),
        OpStrategy::UnsignedCmp => self.emit_unsigned_binary_op(op, lhs, rhs),
        OpStrategy::BoolLogic => self.emit_bool_binary_op(op, lhs, rhs),
        OpStrategy::RuntimeCall { fn_name, .. } => {
            self.emit_runtime_binary_op(fn_name, op, lhs, rhs, lhs_ty)
        }
        OpStrategy::Unsupported => {
            // ICE: the type checker should have rejected this operation.
            // If we reach here, it's a compiler bug, not a user error.
            unreachable!(
                "binary op {:?} on type {:?} reached codegen with Unsupported strategy — \
                 type checker should have rejected this",
                op, type_tag
            );
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
        BinaryOp::Add => self.builder.checked_add(lhs, rhs, "add"),
        BinaryOp::Sub => self.builder.checked_sub(lhs, rhs, "sub"),
        BinaryOp::Mul => self.builder.checked_mul(lhs, rhs, "mul"),
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
            unreachable!("desugared op {op:?} should not reach emit_int_binary_op")
        }
    }
}
/// Extract the coalesce operation (`??`) into its own helper.
///
/// This is the inline code from the current `BinaryOp::Coalesce` arm,
/// extracted verbatim.
fn emit_coalesce(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
    // opt ?? default → extract tag, if Some(0) return payload else default
    let tag = self.builder.extract_value(lhs, 0, "coal.tag").unwrap_or(lhs);
    let payload = self.builder.extract_value(lhs, 1, "coal.val").unwrap_or(lhs);
    let zero = self.builder.const_i64(0);
    let is_some = self.builder.icmp_eq(tag, zero, "is_some");
    self.builder.select(is_some, payload, rhs, "coal")
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
        _ => unreachable!("unsupported float binary op {op:?}"),
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
        _ => unreachable!("unsupported unsigned binary op {op:?}"),
    }
}
/// Emit a binary op using boolean logic instructions.
///
/// Used for `bool` equality (`==`/`!=`) and logical operators (`&&`/`||`).
/// Ordering operators on `bool` use `UnsignedCmp` instead.
fn emit_bool_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
    match op {
        BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
        BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
        BinaryOp::And => self.builder.and(lhs, rhs, "and"),
        BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
        _ => unreachable!("unsupported bool binary op {op:?}"),
    }
}

/// Emit a binary op via runtime function call.
///
/// **Design note**: Currently hardcodes `ori_str_*` dispatch because string
/// comparison ops (lt/gt/le/ge) require post-processing: `ori_str_compare`
/// returns `i8` (Ordering), which must be compared against ordering constants
/// to produce a `bool`. The `fn_name` from `OpStrategy::RuntimeCall` alone
/// is insufficient — the caller also needs to know whether to post-process.
/// When additional `RuntimeCall` types are added (e.g., Duration runtime ops),
/// generalize this function to use `fn_name` directly instead of hardcoded
/// `ori_str_*` names. The `_fn_name` and `_lhs_ty` parameters are reserved
/// for that future generalization.
fn emit_runtime_binary_op(
    &mut self,
    _fn_name: &str,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
    _lhs_ty: Idx,
) -> ValueId {
    match op {
        BinaryOp::Add => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
        BinaryOp::Eq => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
        BinaryOp::NotEq => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
        BinaryOp::Lt => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
            .expect("str Lt comparison should always succeed"),
        BinaryOp::Gt => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Greater)
            .expect("str Gt comparison should always succeed"),
        BinaryOp::LtEq => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::LessOrEqual)
            .expect("str LtEq comparison should always succeed"),
        BinaryOp::GtEq => self.emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::GreaterOrEqual)
            .expect("str GtEq comparison should always succeed"),
        _ => unreachable!("unsupported runtime binary op {op:?}"),
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
        BinaryOp::FloorDiv => type_def.operators.floor_div,
        BinaryOp::Eq => type_def.operators.eq,
        BinaryOp::NotEq => type_def.operators.neq,
        BinaryOp::Lt => type_def.operators.lt,
        BinaryOp::Gt => type_def.operators.gt,
        BinaryOp::LtEq => type_def.operators.lt_eq,
        BinaryOp::GtEq => type_def.operators.gt_eq,
        BinaryOp::BitAnd => type_def.operators.bit_and,
        BinaryOp::BitOr => type_def.operators.bit_or,
        BinaryOp::BitXor => type_def.operators.bit_xor,
        BinaryOp::Shl => type_def.operators.shl,
        BinaryOp::Shr => type_def.operators.shr,

        BinaryOp::And | BinaryOp::Or => OpStrategy::IntInstr, // no OpDefs field; always integer and/or
        BinaryOp::Coalesce => OpStrategy::IntInstr, // structural, not type-dependent
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
            OpStrategy::Unsupported // desugared before reaching ARC IR — ICE if they slip through
        }
    }
}
```

### Implementation steps

- [x] Add `ori_registry` dependency to `ori_llvm/Cargo.toml` (may already exist from Section 11)
- [x] Implement `idx_to_type_tag()` (see 12.2)
- [x] Implement `op_strategy_for_binary()` as shown above
- [x] Extract `emit_int_binary_op()` from the current match arms
- [x] Extract `emit_float_binary_op()` from the current match arms
- [x] Extract `emit_unsigned_binary_op()` (new; currently bool/byte/char fall through to int signed ops)
- [x] Create `emit_bool_binary_op()` (new helper for `BoolLogic` strategy — handles bool eq/neq/and/or)
- [x] Extract `emit_runtime_binary_op()` from the current `is_str` arms
- [x] Extract `emit_coalesce()` from the inline `Coalesce` match arm (operators.rs:101-115) into a new method
- [x] Rewrite `emit_binary_op()` to use the dispatch pattern shown above
- [x] Delete `is_float` and `is_str` local variables
- [x] Clean up verbose `super::super::type_info::TypeInfo` paths in `operators.rs` (lines 42-43, 47, 144, 233) — replace with a `use crate::codegen::type_info::TypeInfo;` import. Most will be removed when deleting `is_float`/`is_str`, but verify no remnants in the trait dispatch helpers.
- [x] Verify: `cargo cl` (LLVM clippy) passes
- [x] Verify: `./llvm-test.sh` passes with identical LLVM IR output
> **Warning: correctness change embedded in refactor.** The `UnsignedCmp` strategy for `bool`/`byte`/`char` changes the *operator path* from signed to unsigned comparison. This is a correctness fix, not a pure refactor. Verify separately with targeted tests before combining with the dispatch restructure. Consider making the unsigned fix a separate commit for bisectability.

### Critical detail: UnsignedCmp for bool/byte/char

The current code does not distinguish signed vs unsigned comparison for all primitive types. The `is_float` guard handles float, and everything else falls through to signed integer ops (`icmp_slt`, etc.). However, `builtins/traits.rs` already correctly uses unsigned comparison for `bool`, `byte`, and `char` in the *trait method path* (`emit_comparison_predicate` dispatches to `emit_unsigned_predicate`).

After this transformation, the *binary operator path* (`emit_binary_op`) must also use unsigned comparison for `bool`, `byte`, and `char`. This is a correctness improvement: currently `byte_a < byte_b` via the operator path would use signed comparison (`icmp slt`) while `byte_a.is_less(byte_b)` via the trait path would correctly use unsigned comparison (`icmp ult`). The registry unifies this: `BYTE.operators.lt = OpStrategy::UnsignedCmp` (and all other comparison fields).

### Critical detail: Coalesce is structural, not type-driven

`BinaryOp::Coalesce` (`??`) operates on the Option/Result tag regardless of the inner type. It extracts field 0 (tag), field 1 (payload), and selects between payload and the default value. This is not type-dependent in the usual sense. After the transformation, `Coalesce` maps to `OpStrategy::IntInstr` (it uses `icmp eq` and `select` on i64 values) and is handled in `emit_int_binary_op` or stays as a separate inline block.

---

## 12.2 Idx-to-TypeTag Bridge for LLVM

### Problem

The LLVM backend works with `Idx` (type pool indices from `ori_types`). The registry uses `TypeTag` (a small enum defined in `ori_registry`). The `emit_binary_op` function receives `lhs_ty: Idx` and needs to look up `ori_registry::find_type(TypeTag)`.

The mapping from `Idx` to `TypeTag` is straightforward for primitive types (fixed indices 0-11), but dynamic types need the `Pool` + `Tag` to determine the type category.

### Current partial mapping

The `TypeInfo` enum in `type_info/info.rs:33-86` already performs this classification. The `TypeInfoStore::get(idx)` method returns a `TypeInfo` variant that directly corresponds to a `TypeTag`. The `builtin_type_name()` method on `TypeInfo` maps `TypeInfo` variants to string names like `"int"`, `"float"`, `"str"` — these are the same names used as `TypeTag` discriminants.

### Solution: idx_to_type_tag() function

```rust
/// Map a type pool `Idx` to a registry `TypeTag` for OpStrategy lookup.
///
/// This is the bridge between the type checker's pool-based type system
/// and the registry's static type tag system. For primitive types (Idx 0-11),
/// the mapping is a direct match on the well-known index constants.
/// For dynamic types, we consult the `TypeInfoStore`.
fn idx_to_type_tag(&self, idx: Idx) -> Option<TypeTag> {
    // Fast path: well-known primitive indices (0-11, skipping Idx::ERROR=8)
    let tag = match idx {
        Idx::INT => TypeTag::Int,
        Idx::FLOAT => TypeTag::Float,
        Idx::BOOL => TypeTag::Bool,
        Idx::STR => TypeTag::Str,
        Idx::CHAR => TypeTag::Char,
        Idx::BYTE => TypeTag::Byte,
        Idx::UNIT => TypeTag::Unit,
        Idx::NEVER => TypeTag::Never,
        // Idx::ERROR (index 8) falls through to dynamic path → TypeInfo::Error → None
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
                TypeInfo::Channel { .. } => TypeTag::Channel,
                TypeInfo::Function { .. } => TypeTag::Function,
                TypeInfo::Error => TypeTag::Error,
                TypeInfo::Unit => TypeTag::Unit,
                TypeInfo::Never => TypeTag::Never,
                // Struct and Enum are handled by trait dispatch (non-primitives).
                // Returning None signals "use trait dispatch or ICE".
                // Note: TypeTag::DoubleEndedIterator has no separate TypeInfo variant;
                // DEI values use TypeInfo::Iterator at the LLVM level.
                _ => return None,
            }
        }
    };
    Some(tag)
}
```
**Design note**: The return type should be `Option<TypeTag>` rather than `TypeTag`, since `TypeTag` has no `Unknown` variant. The caller (`emit_binary_op`/`emit_unary_op`) already handles the non-primitive path via trait dispatch before reaching `idx_to_type_tag`, so `None` here indicates a compiler bug. Types like `Struct` and `Enum` never reach OpStrategy dispatch because `!lhs_ty.is_primitive()` sends them through `emit_binary_op_via_trait` first. However, note that `is_primitive()` checks `idx < 64`, so well-known primitives (0-11) AND reserved indices (12-63) are considered "primitive" — only dynamic types (idx >= 64) take the trait dispatch path.

### Implementation steps

- [x] Add `TypeTag` import from `ori_registry` to `arc_emitter/operators.rs`
- [x] Implement `idx_to_type_tag()` as a method on `ArcIrEmitter` returning `Option<TypeTag>`
- [x] Unit test: every `Idx::*` constant maps to the expected `TypeTag`
- [x] Unit test: dynamic types (constructed via `Pool`) map correctly
> **Warning: `operators.rs` is currently a flat file (363 lines).** After adding `idx_to_type_tag()`, `op_strategy_for_binary()`, `op_strategy_for_unary()`, and 6 new helper functions (`emit_int_binary_op`, `emit_float_binary_op`, `emit_unsigned_binary_op`, `emit_bool_binary_op`, `emit_runtime_binary_op`, `emit_coalesce`), the file will likely exceed 500 lines. Plan to convert `operators.rs` to a directory module (`operators/mod.rs` + `operators/strategy.rs` or similar) as part of this section, or at minimum add a `#[cfg(test)] mod tests;` declaration pointing to `operators/tests.rs`.

### Performance note

The fast path (primitive `Idx` constants 0-11) is a single match on `u32` — zero overhead vs the current `matches!(self.type_info.get(lhs_ty), TypeInfo::Float)` pattern which also calls `type_info.get()`. The dynamic path has the same cost as the current code. Net performance: neutral or slightly better (one `type_info.get()` call instead of two separate `is_float`/`is_str` calls).

---

## 12.3 Replace emit_unary_op Type Guards with OpStrategy Dispatch

### Problem
`emit_unary_op` at `arc_emitter/operators.rs:129-161` has the same pattern as `emit_binary_op`:

```rust
pub(super) fn emit_unary_op(
    &mut self,
    op: UnaryOp,
    operand: ValueId,
    operand_ty: Idx,
) -> ValueId {
    if !operand_ty.is_primitive() {
        if let Some(result) = self.emit_unary_op_via_trait(op, operand, operand_ty) {
            return result;
        }
    }

    let is_float = matches!(
        self.type_info.get(operand_ty),
        super::super::type_info::TypeInfo::Float
    );

    match op {
        UnaryOp::Neg if is_float => self.builder.fneg(operand, "neg"),
        UnaryOp::Neg => self.builder.checked_neg(operand, "neg"),
        UnaryOp::Not => self.builder.not(operand, "not"),
        UnaryOp::BitNot => {
            let all_ones = self.builder.const_i64(-1);
            self.builder.xor(operand, all_ones, "bitnot")
        }
        UnaryOp::Try => { /* warning + zero fallback */ }
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
    let Some(type_tag) = self.idx_to_type_tag(operand_ty) else {
        unreachable!("unary op {op:?} on unmapped type idx {operand_ty:?} — should have used trait dispatch");
    };
    let strategy = self.op_strategy_for_unary(type_tag, op);
    match strategy {
        OpStrategy::IntInstr => match op {
            UnaryOp::Neg => self.builder.checked_neg(operand, "neg"),
            UnaryOp::Not => self.builder.not(operand, "not"),
            UnaryOp::BitNot => {
                let all_ones = self.builder.const_i64(-1);
                self.builder.xor(operand, all_ones, "bitnot")
            }
            UnaryOp::Try => {
                unreachable!("try op should not reach unary expression emitter");
            }
        },
        OpStrategy::FloatInstr => match op {
            UnaryOp::Neg => self.builder.fneg(operand, "neg"),
            _ => {
                unreachable!("unsupported float unary op {op:?}");
            }
        },
        _ => {
            unreachable!("unary op {op:?} on type {type_tag:?} with no unary OpStrategy");
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
        UnaryOp::Not => type_def.operators.not,
        UnaryOp::BitNot => type_def.operators.bit_not,
        UnaryOp::Try => OpStrategy::Unsupported, // desugared
    }
}
```

### Implementation steps

- [x] Implement `op_strategy_for_unary()` as a method on `ArcIrEmitter`
- [x] Rewrite `emit_unary_op()` to use strategy dispatch
- [x] Delete `is_float` local variable
- [x] Verify: `cargo cl` passes
- [x] Verify: `./llvm-test.sh` passes

---

## 12.4 Remove receiver_borrowed from BuiltinRegistration

### Problem
`BuiltinRegistration` in `builtins/mod.rs:111-121` has a `receiver_borrowed: bool` field:

```rust
pub(crate) struct BuiltinRegistration {
    pub type_name: &'static str,
    pub method_name: &'static str,
    pub receiver_borrowed: bool,
}
```
This field is consumed by `borrowing_names_from_table()` (lines 256-274, `#[cfg(test)]` only) to build a `FxHashSet<Name>` of methods that borrow their receiver for sync tests against `ori_arc`. After Section 11, this information comes from `ori_registry` instead. The field and all `borrow: true`/`borrow: false` annotations across the submodules become dead code.

**Important**: There are NO production callers of this data in `ori_llvm` — the `receiver_borrowed` field is only used in test code. The production `borrowing_builtin_names` function lives in `ori_arc::borrow::builtins::mod.rs`, which already derives from `ori_registry`.

### Affected files and entry counts

| File | Entries | Current Pattern |
|------|---------|-----------------|
| `primitives.rs` | 25 entries | `("int", "clone", borrow: true)` |
| `collections/mod.rs` | 66 entries | `("str", "clone", borrow: true)` — includes list/map/Set/range methods |
| `traits.rs` | 73 entries | `("int", "equals", borrow: true)` |
| `compound_traits.rs` | 16 entries | `("list", "equals", borrow: true)` |
| `option_result.rs` | 11 entries | `("Option", "is_some", borrow: true)` |
| `iterator.rs` | 15 entries | `("Iterator", "__iter_next", borrow: true)` |
| `trampolines.rs` | 0 entries | empty `declare_builtins! {}` |
| **Total** | **206 entries** | 189 `borrow: true`, **17 `borrow: false`** |

The 17 `borrow: false` entries are mutation methods in `collections/mod.rs` (e.g., `list.push`, `list.pop`, `list.reverse`, `list.sort`, `list.set`, `list.insert`, `list.remove`, `list.concat`, `list.add`, `map.insert`, `map.remove`, `Set.insert`, `Set.remove`, `Set.union`, `Set.intersection`, `Set.difference`, `list.sort_stable`). Three additional submodules (`compound_type_impls.rs`, `list_traits.rs`, `iterator_consumers.rs`) exist but do NOT use `declare_builtins!` and have zero borrow annotations.

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
**collections/mod.rs (66 entries — partial list, see file for complete list):**
Str: `clone`, `length`, `len`, `is_empty`, `concat`, `to_str`, `contains`, `starts_with`, `ends_with`, `trim`, `substring`, `slice`, `split`, `upper`, `lower`, `byte_len`, `as_bytes`, `to_bytes`, `from_utf8`, `from_utf8_unchecked`, `iter`, `replace`, `char_at`, `index_of`, `join`, and more.
List: `clone`, `length`, `len`, `is_empty`, `iter`, `first`, `last`, `contains`, `index_of`, `slice`, `updated`, `concat` (borrow: false), `add` (borrow: false), `push` (borrow: false), `pop` (borrow: false), `reverse` (borrow: false), `sort` (borrow: false), `sort_stable` (borrow: false), `set` (borrow: false), `insert` (borrow: false), `remove` (borrow: false).
Map: `clone`, `length`, `len`, `is_empty`, `iter`, `get`, `contains_key`, `keys`, `values`, `entries`, `insert` (borrow: false), `remove` (borrow: false).
Set: `clone`, `length`, `len`, `is_empty`, `iter`, `contains`, `insert` (borrow: false), `remove` (borrow: false), `union` (borrow: false), `intersection` (borrow: false), `difference` (borrow: false).
Range: `iter`.

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

- [x] Remove `receiver_borrowed` field from `BuiltinRegistration` struct in `builtins/mod.rs`
- [x] Update `declare_builtins!` macro definition — see 12.5
- [x] Check `builtins/collections/mod.rs` line count after removing `borrow:` annotations — currently 528 lines with a documented 500-line exemption. Removing 66 `borrow:` annotations should bring it to ~462 lines. If under 500, remove the exemption comment (lines 6-8).
- [x] Update all 206 entries across 6 submodules (remove `, borrow: true` or `, borrow: false`)
- [x] Remove all references to `receiver_borrowed` in `BuiltinTable` and related code
- [x] Delete or rewrite `consuming_builtins_sync_with_ori_arc` test (reads `reg.receiver_borrowed`)
- [x] Ensure `borrowing_builtins_sync_with_ori_arc` deletion (12.6) is done atomically with this step
- [x] Verify: `cargo cl` passes

---

## 12.5 Simplify declare_builtins! Macro

### BEFORE (current macro definition, `builtins/mod.rs:52-73`)

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

- [x] Update `macro_rules! declare_builtins!` — remove `borrow: $borrow:expr` from pattern, remove `receiver_borrowed: $borrow` from `BuiltinRegistration` construction
- [x] Update `primitives.rs`: remove `, borrow: true` from 25 entries
- [x] Update `collections/mod.rs`: remove `, borrow: true`/`, borrow: false` from 66 entries
- [x] Update `traits.rs`: remove `, borrow: true` from 73 entries
- [x] Update `compound_traits.rs`: remove `, borrow: true` from 16 entries
- [x] Update `option_result.rs`: remove `, borrow: true` from 11 entries
- [x] Update `iterator.rs`: remove `, borrow: true` from 15 entries
- [x] Update `trampolines.rs`: no entries to change (empty declaration), but verify the empty macro invocation compiles
- [x] Verify: `cargo cl` passes

### Mechanical transformation

This is a pure find-and-replace operation. The regex for the entry change is:

```
(, borrow: (true|false))
```
Replace with empty string. Of the 206 entries, 189 use `borrow: true` and 17 use `borrow: false` (mutation methods in `collections/mod.rs` such as `push`, `pop`, `reverse`, `sort`, `insert`, `remove`). The regex handles both values.
- [x] Update `builtins/mod.rs:37-46` doc comment — currently shows old `borrow:` syntax in `/// # Usage` example. Update to show the new 2-tuple entry syntax.
- [x] Remove the `receiver_borrowed` doc comment on `BuiltinRegistration` (`builtins/mod.rs:117-120`) — field will be deleted.
- [x] Review `builtins/mod.rs:160-164` — the NOTE comment on `BuiltinTable` mentions `receiver_borrowed` as the reason for `#[allow(dead_code)]`. After removing the field, determine whether `BuiltinTable` should move under `#[cfg(test)]`. If test-only, move it now rather than deferring.

---
## 12.6 Delete borrowing_names_from_table() Function

### Current location


`builtins/mod.rs:256-274` (`#[cfg(test)]` only):

```rust
#[cfg(test)]
fn borrowing_names_from_table(interner: &ori_ir::StringInterner) -> rustc_hash::FxHashSet<Name> {
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

### Callers

**No production callers.** This function is `#[cfg(test)]` only. It is used in:

1. `builtins/tests.rs:210` — `borrowing_names_from_table(&interner)` in the `borrowing_builtins_sync_with_ori_arc` test
2. `builtins/tests.rs:19` — `use super::borrowing_names_from_table;`

There is **no re-export** from `arc_emitter/mod.rs`. The production `borrowing_builtin_names` function lives in `ori_arc::borrow::builtins::mod.rs:103` and already derives from `ori_registry`.

### Prerequisites

After `receiver_borrowed` is removed from `BuiltinRegistration` (12.4), the test that uses this function will need to be deleted or rewritten to use `ori_registry` directly.

### Implementation steps

- [x] Delete `borrowing_names_from_table()` at `builtins/mod.rs:256-274`
- [x] Delete or rewrite the `borrowing_builtins_sync_with_ori_arc` test in `builtins/tests.rs` (its sync job is superseded by registry-based validation in 12.8)
- [x] If `receiver_borrowed` was the only reason `BuiltinTable.entries` exposed registration details, simplify `BuiltinTable` accordingly
- [x] Verify: `cargo cl` passes

---
## 12.7 ARC Pipeline Methods Migration

### Current state

`builtins/tests.rs:62-89` — the `arc_pipeline_methods()` function dynamically builds the list from `ProtocolBuiltin::ALL`:

```rust
fn arc_pipeline_methods() -> Vec<(&'static str, &'static str)> {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let mut methods: Vec<(&str, &str)> = Vec::new();
    for &pb in ProtocolBuiltin::ALL {
        if pb == ProtocolBuiltin::Index {
            continue; // ARC borrowing intrinsic, not a BuiltinTable method
        }
        methods.push(("Iterator", pb.name()));
    }
    // ... plus additional hardcoded entries
    methods
}
```
Additional test data at `builtins/tests.rs:46-60`:
```rust
const CODEGEN_ALIASES: &[(&str, &str)] = &[("length", "len"), ("is_equal", "equals")];
const TRAIT_DISPATCH_METHODS: &[&str] = &[
    "is_less", "is_greater", "is_less_or_equal", "is_greater_or_equal",
];
```
These are methods that reach the `BuiltinTable` dispatch through codegen paths other than the registry — they are valid entries that would otherwise be flagged as "phantom" by `no_phantom_builtin_entries`.

### Decision

`arc_pipeline_methods()` is about **codegen routing** (which methods are emitted by the ARC lowering pipeline vs the builtin method dispatch path), not about **type behavior**. This is a codegen-internal concern, not a registry concern.

**Keep as test-local knowledge.** After the registry migration, the phantom test should still verify that every `BuiltinTable` entry has backing in either the registry or the `arc_pipeline_methods()` list. The migration changes the verification source (registry instead of `TYPECK_BUILTIN_METHODS`) but keeps the ARC pipeline exception list.

### Updated test pattern

```rust
#[test]
fn no_phantom_builtin_entries() {
    let table = builtin_table();
    let arc_pipeline: HashSet<(&str, &str)> = arc_pipeline_methods().iter().copied().collect();
    let mut phantom = Vec::new();
    for (type_name, method_name) in table.all_registered() {
        // Direct registry match: find the TypeDef by legacy name, then check method
        let in_registry = ori_registry::BUILTIN_TYPES.iter().any(|td| {
            ori_registry::legacy_type_name(td.name) == type_name
                && td.methods.iter().any(|m| m.name == method_name)
        });
        if in_registry {
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

- [x] Update `no_phantom_builtin_entries` to verify against `ori_registry` instead of `TYPECK_BUILTIN_METHODS` (already uses `registry_method_set()` from `ori_registry::BUILTIN_TYPES`)
- [x] Remove `CODEGEN_ALIASES` constant (the registry should use canonical names; if aliases are needed, the registry handles them) — **kept**: registry doesn't include `length`→`len` or `is_equal`→`equals` aliases for non-canonical codegen paths; removing would cause phantom test failures
- [x] Remove `TRAIT_DISPATCH_METHODS` constant (trait methods are now in the registry as regular `MethodDef` entries) — **kept**: `is_less`/`is_greater`/etc. only in registry for `Ordering`, not for `int`/`float`/etc.
- [x] Keep `arc_pipeline_methods()` as codegen-internal test knowledge
- [x] Update `builtin_coverage_above_threshold` test to compare against registry instead of `TYPECK_BUILTIN_METHODS` (already uses `registry_method_set()`)
- [x] Verify: `cargo test -p ori_llvm` passes

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
    let arc_pipeline: HashSet<(&str, &str)> = arc_pipeline_methods().iter().copied().collect();
    let mut missing = Vec::new();
    for (type_name, method_name) in table.all_registered() {
        if arc_pipeline.contains(&(type_name, method_name)) {
            continue;
        }
        // Registry must know about this method.
        // find_type_by_name uses registry names (PascalCase for List/Set/etc.),
        // but BuiltinTable uses legacy names (lowercase). Use the reverse of
        // legacy_type_name() or iterate BUILTIN_TYPES to find the match.
        let type_def = ori_registry::BUILTIN_TYPES.iter().find(|td| {
            ori_registry::legacy_type_name(td.name) == type_name
        });
        let Some(type_def) = type_def else {
            missing.push(format!("  ({type_name}, {method_name}) — unknown type"));
            continue;
        };
        let has_method = type_def.methods.iter().any(|m| m.name == method_name);
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
**Note on 12.7/12.8 ordering**: If `CODEGEN_ALIASES` and `TRAIT_DISPATCH_METHODS` are removed in 12.7, this test (`registry_covers_all_builtin_codegen`) must either inline equivalent logic or the removal must be deferred until the registry includes these method names. Currently, the `no_phantom_builtin_entries` test uses both constants, and this replacement test must handle the same cases.

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
        OpStrategy::BoolLogic,
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
            ("floor_div", ops.floor_div),
            ("eq", ops.eq),
            ("neq", ops.neq),
            ("lt", ops.lt),
            ("gt", ops.gt),
            ("lt_eq", ops.lt_eq),
            ("gt_eq", ops.gt_eq),
            ("neg", ops.neg),
            ("not", ops.not),
            ("bit_and", ops.bit_and),
            ("bit_or", ops.bit_or),
            ("bit_xor", ops.bit_xor),
            ("bit_not", ops.bit_not),
            ("shl", ops.shl),
            ("shr", ops.shr),
        ] {
            match strategy {
                OpStrategy::Unsupported => {} // Fine, type doesn't support this op
                OpStrategy::RuntimeCall { fn_name, .. } => {
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

- [x] Write `registry_covers_all_builtin_codegen` test in `builtins/tests.rs` — **kept existing `no_phantom_builtin_entries`** which already validates against `ori_registry::BUILTIN_TYPES`; same coverage
- [x] Write `registry_op_strategies_cover_all_operators` test in `builtins/tests.rs`
- [x] **Unit tests for `idx_to_type_tag`** and **`op_strategy_for_binary`/`op_strategy_for_unary`** should go in an `operators/tests.rs` sibling file (or add `#[cfg(test)] mod tests;` to `operators.rs` with a sibling `tests.rs`) — `idx_to_type_tag` tests exist in `arc_emitter/tests.rs` from 12.2
- [x] Delete or replace `no_phantom_builtin_entries` (superseded) — **kept**: already uses registry as source of truth
- [x] Delete or replace `builtin_coverage_above_threshold` (superseded; registry is 100% authoritative) — **kept**: provides useful coverage tracking metric
- [x] When writing `registry_covers_all_builtin_codegen`, do not carry forward dead type entries from `codegen_types` in `builtins/tests.rs:155-160` — it includes `"error"` and `"Channel"` which have zero `BuiltinTable` entries.
- [x] Verify: `cargo test -p ori_llvm` passes

---

## 12.9 Validation & Regression

### Build verification

- [x] `cargo c -p ori_llvm` (standard check)
- [x] `cargo cl` (clippy with LLVM feature)
- [x] `cargo b` (debug build: oric + ori_rt)
- [x] `cargo b --release` (release build: oric + ori_rt)

### Test verification

- [x] `./llvm-test.sh` (LLVM unit tests)
- [x] `./test-all.sh` (full test suite — 12,478 passed, 0 failed)
- [x] `cargo test -p ori_llvm` (all ori_llvm tests including the new registry validation tests)

### Specific regression targets

The following tests must produce **identical** LLVM IR before and after this change. The transformation is a refactor of dispatch structure, not a change in emitted instructions.

**String comparison tests** (the original bug class):
- [x] `compiler/ori_llvm/tests/aot/strings.rs` — string equality, ordering, concatenation
- [x] `compiler/ori_llvm/tests/aot/conversions.rs` — string conversion paths
- [x] Any spec tests in `tests/spec/` that exercise string `<`, `>`, `<=`, `>=`

**Float comparison tests:**
- [x] Float equality (`==`, `!=`) produces `fcmp oeq`/`fcmp one`
- [x] Float ordering (`<`, `>`, `<=`, `>=`) produces `fcmp olt`/`fcmp ogt`/`fcmp ole`/`fcmp oge`

**Integer comparison tests:**
- [x] Signed comparison for `int` produces `icmp slt`/`icmp sgt`/`icmp sle`/`icmp sge`
- [x] Unsigned comparison for `byte` produces `icmp ult`/`icmp ugt`/`icmp ule`/`icmp uge`

**Operator trait dispatch tests** (non-primitive types must still work):
- [x] User-defined struct with `Add`/`Eq`/`Comparable` trait impls
- [x] The `!lhs_ty.is_primitive()` guard is preserved, so non-primitive types still use `emit_binary_op_via_trait` and `emit_comparison_via_trait`

**Edge cases:**
- [x] `Coalesce` (`??`) still works on `Option` and `Result` types
- [x] `Range`/`RangeInclusive`/`MatMul` trigger `unreachable!()` (these ops are desugared before ARC IR; reaching codegen is a compiler bug)
- [x] `Duration` and `Size` operators use `IntInstr` (they are i64 under the hood)
- [x] `Ordering` type does not have arithmetic operators (only Eq/Comparable)

### Grep verification (post-migration)

After all steps are complete, these greps must return **zero results** in `arc_emitter/operators.rs`:

```bash
# No is_float/is_str guards in emit_binary_op or emit_unary_op
grep -n 'is_float\|is_str' compiler/ori_llvm/src/codegen/arc_emitter/operators.rs
# Result: 0 matches (all type discrimination is via OpStrategy)

# No receiver_borrowed in BuiltinRegistration
grep -rn 'receiver_borrowed' compiler/ori_llvm/src/codegen/arc_emitter/
# Result: 0 matches
# No borrowing_names_from_table function or receiver_borrowed references
grep -rn 'borrowing_names_from_table' compiler/ori_llvm/src/
# Result: 0 matches
```
And this grep must return results confirming the new pattern:

```bash
# OpStrategy dispatch is in place
grep -n 'OpStrategy' compiler/ori_llvm/src/codegen/arc_emitter/operators.rs
# Result: matches in emit_binary_op and emit_unary_op

# idx_to_type_tag bridge exists
grep -n 'idx_to_type_tag' compiler/ori_llvm/src/codegen/arc_emitter/operators.rs
# Result: definition + call sites in emit_binary_op, emit_unary_op
```

### Release binary verification

Per LLVM backend rules, debug and release can differ due to FastISel behavior. Both must be tested:

- [x] `cargo b && ./test-all.sh` (debug)
- [x] `cargo b --release && ./test-all.sh` (release)

---

## Exit Criteria

All of the following must be true before this section is marked complete:

- [x] **No `is_float`/`is_str` guards in `emit_binary_op`** — OpStrategy dispatch is the only type discrimination path
- [x] **No `is_float` guard in `emit_unary_op`** — OpStrategy dispatch handles float negation
- [x] **`OpStrategy` dispatch in place** — `emit_binary_op` and `emit_unary_op` call `idx_to_type_tag()` and `op_strategy_for_binary()`/`op_strategy_for_unary()`
- [x] **`receiver_borrowed` removed** from `BuiltinRegistration` and all 206 `declare_builtins!` entries
- [x] **`declare_builtins!` simplified** — entry syntax is `("type", "method") => handler`
- [x] **`borrowing_names_from_table()` deleted** — test-only function removed, sync test deleted
- [x] **`consuming_builtins_sync_with_ori_arc` test deleted** — both ownership sync tests removed (superseded by registry)
- [x] **Registry validation tests passing** — `no_phantom_builtin_entries` (registry-backed) and `registry_op_strategies_cover_all_operators`
- [x] **`./llvm-test.sh` passes** (debug and release)
- [x] **`./test-all.sh` passes** (debug and release — 12,478 passed, 0 failed)
- [x] **Grep verification clean** — no `is_float`/`is_str` in `emit_binary_op`/`emit_unary_op`, no `receiver_borrowed`, no `borrowing_names_from_table`
- [x] **Net code deletion** — removed 206 `borrow:` annotations, `receiver_borrowed` field, `borrowing_names_from_table()`, 2 sync tests
- [x] If `operators.rs` exceeds 500 lines after adding strategy dispatch helpers, split into `operators/mod.rs` + `operators/strategy.rs` (dispatch logic) + `operators/tests.rs` (unit tests) — split done (325 + 305 lines)

---

## Implementation Order
> **Warning (step ordering).** 12.2 (Idx-to-TypeTag bridge) MUST be implemented and tested before 12.1/12.3 because the bridge function is used by `emit_binary_op`/`emit_unary_op`. The plan's dependency graph already shows this, but the prose in 12.1's implementation steps lists "Implement `idx_to_type_tag()` (see 12.2)" inline, which could mislead an implementer into thinking it's a quick inline step. It is a separate subsection with its own tests for a reason: the mapping correctness is load-bearing.

The subsections have the following dependency chain:
```
12.2 (Idx-to-TypeTag bridge)
  ↓
12.1 (emit_binary_op) ←─ requires 12.2
  ↓
12.3 (emit_unary_op) ←─ requires 12.2
  ↓
12.4+12.5+12.6 (ATOMIC: remove receiver_borrowed + simplify macro + delete tests)
  ↓              All three must happen in a single commit — field removal
  ↓              breaks macro invocations and test compilation simultaneously.
  ↓              Also includes consuming_builtins_sync_with_ori_arc deletion.
12.7 (ARC pipeline methods) ←─ requires 12.4 (test data restructuring)
  ↓
12.8 (validation tests) ←─ requires 12.1, 12.4, 12.7
  ↓
12.9 (full validation) ←─ requires all above
```
Recommended execution: 12.2, then 12.1+12.3 together, then 12.4+12.5+12.6 as one atomic change, then 12.7, then 12.8, then 12.9.
