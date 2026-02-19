---
section: "07"
title: Shared Derive Strategy
status: not-started
goal: Eliminate eval/LLVM derive implementation duplication via a shared derivation strategy pattern
sections:
  - id: "07.1"
    title: Duplication Analysis
    status: not-started
  - id: "07.2"
    title: Strategy Pattern Design
    status: not-started
  - id: "07.3"
    title: Eval Backend Adaptation
    status: not-started
  - id: "07.4"
    title: LLVM Backend Adaptation
    status: not-started
  - id: "07.5"
    title: Completion Checklist
    status: not-started
---

# Section 07: Shared Derive Strategy

**Status:** Not Started
**Goal:** Define the logical structure of each trait derivation once in `ori_ir`, then have eval and LLVM backends interpret that structure. Adding a new derived trait means defining its strategy once, not implementing the same logic twice in different representations.

**Depends on:** Section 01 (trait metadata), Section 04 (LLVM derive factory)

**Reference compilers:**
- **Swift** `lib/Sema/DerivedConformances.cpp` — Per-trait strategy classes (`DerivedConformance_Equatable.cpp`, `DerivedConformance_Hashable.cpp`) define derivation logic; the codegen layer interprets.
- **Rust** `compiler/rustc_builtin_macros/src/deriving/` — Per-trait derivation modules (`eq.rs`, `hash.rs`, `cmp.rs`) generate AST expansion; shared via `TraitDef` struct.
- **Lean 4** `src/Lean/Compiler/IR/RC.lean` — Derivation strategies expressed as IR transformations, interpreted by multiple backends.

**Current state:** Each derived trait is fully implemented in two places:
- `ori_eval/interpreter/derived_methods.rs` — tree-walking evaluation
- `ori_llvm/codegen/derive_codegen/` — LLVM IR generation

Both independently implement the same logical algorithms (lexicographic comparison, hash combining, field-by-field equality) in different representations.

---

## 07.1 Duplication Analysis

### Logical Structure Comparison

| Trait | Eval Logic | LLVM Logic | Shared Pattern |
|-------|-----------|-----------|----------------|
| **Eq** | For each field: `field1 == field2`, short-circuit on `false` | For each field: emit `icmp eq`, branch on `false` | ForEachField + AllEqual |
| **Clone** | For each field: clone value, reconstruct struct | For each field: copy/RC-inc, construct struct | ForEachField + CloneField |
| **Hashable** | For each field: `hash_combine(acc, hash(field))` | For each field: emit hash call, emit combine | ForEachField + HashCombine |
| **Printable** | Format as "TypeName(field1, field2, ...)" | Emit string concat calls | FormatFields("(", ", ", ")") |
| **Debug** | Format as "TypeName { f1: v1, f2: v2 }" | (not yet in LLVM) | FormatFields("{ ", ", ", " }") |
| **Default** | For each field: produce default value | For each field: emit zero/default | ForEachField + DefaultValue |
| **Comparable** | For each field: compare, short-circuit on non-Equal | For each field: emit compare, branch on non-Equal | ForEachField + LexicographicCmp |

### What's Actually Duplicated

The **field iteration logic** and **composition strategy** are identical. What differs is the **primitive operations** (eval uses `Value::eq()`, LLVM emits `icmp eq`). The abstraction should capture the iteration/composition, not the primitives.

---

## 07.2 Strategy Pattern Design

### Core Types

```rust
// compiler/ori_ir/src/derives/strategy.rs

/// The high-level strategy for deriving a trait.
///
/// Each derived trait has a strategy that describes:
/// 1. What to do for each field (the operation)
/// 2. How to combine field results (the composition)
/// 3. What to do for sum types (variant dispatch)
///
/// Backends (eval, LLVM) interpret this strategy in their own
/// representation (Values vs LLVM IR).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeriveStrategy {
    /// How to handle struct fields.
    pub struct_strategy: StructStrategy,
    /// How to handle sum type variants.
    pub sum_strategy: SumStrategy,
}

/// Strategy for deriving a trait on a struct (product type).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StructStrategy {
    /// Apply operation to each field pair, combine results.
    /// Used by: Eq (AllEqual), Comparable (Lexicographic), Hashable (HashCombine)
    ForEachField {
        /// The per-field operation.
        field_op: FieldOperation,
        /// How to combine per-field results.
        combine: CombineStrategy,
        /// Initial accumulator value.
        initial: InitialValue,
    },

    /// Format fields into a string.
    /// Used by: Printable, Debug
    FormatFields {
        /// Text before all fields.
        prefix: FormatPrefix,
        /// Text between fields.
        separator: &'static str,
        /// Text after all fields.
        suffix: &'static str,
        /// Whether to include field names ("f1: v1" vs "v1").
        include_names: bool,
    },

    /// Produce a default value for each field.
    /// Used by: Default
    DefaultConstruct,

    /// Copy/clone each field.
    /// Used by: Clone
    CloneFields,
}

/// What operation to apply to each field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldOperation {
    /// Call the field's `eq` method: (lhs.field, rhs.field) -> bool
    Equals,
    /// Call the field's `compare` method: (lhs.field, rhs.field) -> Ordering
    Compare,
    /// Call the field's `hash` method: (field) -> int
    Hash,
}

/// How to combine per-field results into a final result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombineStrategy {
    /// All fields must be true: `f1 && f2 && ...` — short-circuit on false.
    AllTrue,
    /// Lexicographic: use first non-Equal result, or Equal if all equal.
    Lexicographic,
    /// Hash combine: `hash_combine(hash_combine(init, h1), h2)`.
    HashCombine,
}

/// Initial accumulator for field combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InitialValue {
    /// Start with `true` (for AllTrue).
    True,
    /// Start with `Ordering::Equal` (for Lexicographic).
    OrderingEqual,
    /// Start with a seed integer (for HashCombine).
    HashSeed,
}

/// How to format the type name prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatPrefix {
    /// "TypeName(" — used by Printable
    TypeNameParen,
    /// "TypeName { " — used by Debug
    TypeNameBrace,
}

/// Strategy for deriving a trait on a sum type (enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SumStrategy {
    /// Match on variant tag; compare same variants field-by-field,
    /// different variants use tag ordering.
    MatchVariants,
    /// Not supported for this trait (e.g., Default on sum types).
    NotSupported,
}
```

### Strategy Assignment

```rust
impl DerivedTrait {
    /// Get the derivation strategy for this trait.
    pub fn strategy(&self) -> DeriveStrategy {
        match self {
            DerivedTrait::Eq => DeriveStrategy {
                struct_strategy: StructStrategy::ForEachField {
                    field_op: FieldOperation::Equals,
                    combine: CombineStrategy::AllTrue,
                    initial: InitialValue::True,
                },
                sum_strategy: SumStrategy::MatchVariants,
            },
            DerivedTrait::Clone => DeriveStrategy {
                struct_strategy: StructStrategy::CloneFields,
                sum_strategy: SumStrategy::MatchVariants,
            },
            DerivedTrait::Hashable => DeriveStrategy {
                struct_strategy: StructStrategy::ForEachField {
                    field_op: FieldOperation::Hash,
                    combine: CombineStrategy::HashCombine,
                    initial: InitialValue::HashSeed,
                },
                sum_strategy: SumStrategy::MatchVariants,
            },
            DerivedTrait::Printable => DeriveStrategy {
                struct_strategy: StructStrategy::FormatFields {
                    prefix: FormatPrefix::TypeNameParen,
                    separator: ", ",
                    suffix: ")",
                    include_names: false,
                },
                sum_strategy: SumStrategy::MatchVariants,
            },
            DerivedTrait::Debug => DeriveStrategy {
                struct_strategy: StructStrategy::FormatFields {
                    prefix: FormatPrefix::TypeNameBrace,
                    separator: ", ",
                    suffix: " }",
                    include_names: true,
                },
                sum_strategy: SumStrategy::MatchVariants,
            },
            DerivedTrait::Default => DeriveStrategy {
                struct_strategy: StructStrategy::DefaultConstruct,
                sum_strategy: SumStrategy::NotSupported,
            },
            DerivedTrait::Comparable => DeriveStrategy {
                struct_strategy: StructStrategy::ForEachField {
                    field_op: FieldOperation::Compare,
                    combine: CombineStrategy::Lexicographic,
                    initial: InitialValue::OrderingEqual,
                },
                sum_strategy: SumStrategy::MatchVariants,
            },
        }
    }
}
```

- [ ] Define `DeriveStrategy`, `StructStrategy`, `SumStrategy`, `FieldOperation`, `CombineStrategy`, `InitialValue`, `FormatPrefix` types in `ori_ir/derives/strategy.rs`
- [ ] Implement `DerivedTrait::strategy()` for all 7 traits
- [ ] Unit tests: each trait's strategy matches expected structure
- [ ] Verify all types are Salsa-compatible (`Clone, Debug, PartialEq, Eq, Hash`)

---

## 07.3 Eval Backend Adaptation

### Current Architecture

`eval_derived_method()` dispatches to per-trait handlers:
```rust
match info.trait_kind {
    DerivedTrait::Eq => self.eval_derived_eq(receiver, info, args),
    DerivedTrait::Comparable => self.eval_derived_comparable(receiver, info, args),
    // ...
}
```

Each handler independently implements field iteration and result composition.

### Strategy-Driven Architecture

```rust
pub fn eval_derived_method(
    &mut self, receiver: Value, info: &DerivedMethodInfo, args: &[Value],
) -> EvalResult {
    let strategy = info.trait_kind.strategy();

    match &strategy.struct_strategy {
        StructStrategy::ForEachField { field_op, combine, initial } => {
            self.eval_for_each_field(receiver, info, args, *field_op, *combine, *initial)
        }
        StructStrategy::FormatFields { prefix, separator, suffix, include_names } => {
            self.eval_format_fields(receiver, info, *prefix, separator, suffix, *include_names)
        }
        StructStrategy::DefaultConstruct => {
            self.eval_default_construct(info)
        }
        StructStrategy::CloneFields => {
            self.eval_clone_fields(receiver, info)
        }
    }
}

fn eval_for_each_field(
    &mut self, receiver: Value, info: &DerivedMethodInfo, args: &[Value],
    field_op: FieldOperation, combine: CombineStrategy, initial: InitialValue,
) -> EvalResult {
    let other = args.first();
    let mut acc = match initial {
        InitialValue::True => Value::Bool(true),
        InitialValue::OrderingEqual => Value::ordering_equal(),
        InitialValue::HashSeed => Value::Int(0xcbf29ce484222325_u64 as i64), // FNV offset
    };

    for field_name in &info.field_names {
        let field_val = receiver.field(field_name)?;
        let field_result = match field_op {
            FieldOperation::Equals => {
                let other_field = other.unwrap().field(field_name)?;
                self.call_method(&field_val, "eq", &[other_field])?
            }
            FieldOperation::Compare => {
                let other_field = other.unwrap().field(field_name)?;
                self.call_method(&field_val, "compare", &[other_field])?
            }
            FieldOperation::Hash => {
                self.call_method(&field_val, "hash", &[])?
            }
        };

        acc = match combine {
            CombineStrategy::AllTrue => {
                if field_result == Value::Bool(false) {
                    return Ok(Value::Bool(false)); // short-circuit
                }
                acc
            }
            CombineStrategy::Lexicographic => {
                if acc != Value::ordering_equal() {
                    return Ok(acc); // short-circuit on first non-equal
                }
                field_result
            }
            CombineStrategy::HashCombine => {
                self.call_function("hash_combine", &[acc, field_result])?
            }
        };
    }

    Ok(acc)
}
```

### Migration Approach

**Phase 1:** Implement the strategy-driven dispatch alongside existing per-trait handlers. Use a feature flag or cfg(test) to route through the new path.

**Phase 2:** Validate equivalence — for each trait, both paths produce the same results on the same inputs.

**Phase 3:** Delete the per-trait handlers, keeping only the strategy-driven dispatch.

- [ ] Implement `eval_for_each_field()` using strategy types
- [ ] Implement `eval_format_fields()` using strategy types
- [ ] Implement `eval_default_construct()` using strategy types
- [ ] Implement `eval_clone_fields()` using strategy types
- [ ] Equivalence tests: strategy-driven output == per-trait output for all 7 traits
- [ ] Replace per-trait dispatch with strategy dispatch
- [ ] Delete `eval_derived_eq()`, `eval_derived_hash()`, etc. (6 functions)
- [ ] `./test-all.sh` passes

---

## 07.4 LLVM Backend Adaptation

### Strategy-Driven Codegen

The LLVM backend interprets the same `DeriveStrategy` but emits IR instead of evaluating values.

```rust
pub fn compile_derives(fc, module, type_name, type_idx, type_name_str, fields) {
    for trait_kind in &type_decl.derives {
        let strategy = trait_kind.strategy();

        with_derive_function(fc, *trait_kind, type_name, type_idx, type_name_str, fields,
            |fc, func_id, abi, self_val, other_val, fields| {
                match &strategy.struct_strategy {
                    StructStrategy::ForEachField { field_op, combine, initial } => {
                        emit_for_each_field(fc, self_val, other_val, fields, *field_op, *combine, *initial)
                    }
                    StructStrategy::FormatFields { prefix, separator, suffix, include_names } => {
                        emit_format_fields(fc, self_val, fields, type_name_str, *prefix, separator, suffix, *include_names)
                    }
                    StructStrategy::DefaultConstruct => {
                        emit_default_construct(fc, fields, type_idx)
                    }
                    StructStrategy::CloneFields => {
                        emit_clone_fields(fc, self_val, fields)
                    }
                }
            },
        );
    }
}
```

### Shared Field Iteration

The `ForEachField` strategy uses `emit_field_operation()` from Section 04.3:

```rust
fn emit_for_each_field(
    fc: &mut FunctionCompiler,
    self_val: ValueId,
    other_val: Option<ValueId>,
    fields: &[FieldDef],
    field_op: FieldOperation,
    combine: CombineStrategy,
    initial: InitialValue,
) -> Option<ValueId> {
    let op = match field_op {
        FieldOperation::Equals => FieldOp::Eq,
        FieldOperation::Compare => FieldOp::Compare,
        FieldOperation::Hash => FieldOp::Hash,
    };

    let mut acc = match initial {
        InitialValue::True => fc.emit_const_bool(true),
        InitialValue::OrderingEqual => fc.emit_const_ordering_equal(),
        InitialValue::HashSeed => fc.emit_const_i64(FNV_OFFSET),
    };

    for field in fields {
        let lhs = fc.emit_field_access(self_val, field);
        let rhs = other_val.map(|o| fc.emit_field_access(o, field));

        let result = emit_field_operation(fc, op, lhs, rhs, field.type_idx);

        acc = match combine {
            CombineStrategy::AllTrue => {
                // Branch: if !result, return false immediately
                let continue_bb = fc.append_basic_block("eq_continue");
                let false_bb = fc.append_basic_block("eq_false");
                fc.emit_cond_br(result, continue_bb, false_bb);
                fc.position_at_end(false_bb);
                fc.emit_return(fc.emit_const_bool(false));
                fc.position_at_end(continue_bb);
                acc // acc stays true until we check all fields
            }
            CombineStrategy::Lexicographic => {
                // Branch: if acc != Equal, return acc immediately
                let is_equal = fc.emit_icmp_eq(acc, fc.emit_const_ordering_equal());
                let continue_bb = fc.append_basic_block("cmp_continue");
                let done_bb = fc.append_basic_block("cmp_done");
                fc.emit_cond_br(is_equal, continue_bb, done_bb);
                fc.position_at_end(done_bb);
                fc.emit_return(acc);
                fc.position_at_end(continue_bb);
                result // next field's result becomes new acc
            }
            CombineStrategy::HashCombine => {
                fc.emit_call("ori_hash_combine", &[acc, result])
            }
        };
    }

    Some(acc)
}
```

- [ ] Implement `emit_for_each_field()` using strategy types
- [ ] Implement `emit_format_fields()` using strategy types
- [ ] Implement `emit_default_construct()` using strategy types
- [ ] Implement `emit_clone_fields()` using strategy types
- [ ] Refactor `compile_derives()` to use strategy dispatch
- [ ] Delete per-trait `compile_derive_*()` functions (6-7 functions)
- [ ] `./llvm-test.sh` passes
- [ ] `./test-all.sh` passes

---

## 07.5 Completion Checklist

- [ ] `DeriveStrategy` and related types defined in `ori_ir/derives/strategy.rs`
- [ ] `DerivedTrait::strategy()` returns correct strategy for all 7 traits
- [ ] Eval backend uses strategy-driven dispatch, not per-trait handlers
- [ ] LLVM backend uses strategy-driven dispatch, not per-trait handlers
- [ ] Both backends produce identical output to the previous per-trait implementations
- [ ] Per-trait handler functions deleted from both backends
- [ ] Adding a new derived trait with `ForEachField` strategy requires zero eval/LLVM code changes — just the strategy definition
- [ ] Adding a new derived trait with a novel strategy (new `StructStrategy` variant) requires one eval handler + one LLVM handler + the strategy definition
- [ ] Unit tests: strategy correctness for all 7 traits
- [ ] Integration tests: derived trait behavior unchanged for all spec tests
- [ ] `./test-all.sh` passes with zero regressions

**Exit Criteria:** The derivation algorithm is defined once (strategy), interpreted twice (eval, LLVM). Adding Eq-like traits (ForEachField with AllTrue) requires zero backend changes. The codebase has one place that describes "how Eq derives work" — not two places implementing it independently.
