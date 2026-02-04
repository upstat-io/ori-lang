# LLVM Backend: Mutable Variables Architecture

## Problem Statement

The current LLVM backend uses a flat `FxHashMap<Name, BasicValueEnum>` for all variables. When `let mut count = 0` is compiled:

1. `count` maps to LLVM value `i64 0` in the hashmap
2. `count = count + 1` computes `0 + 1 = 1`, updates hashmap to `count → i64 1`
3. **But the LLVM IR was already emitted** — it references the original SSA value

This causes infinite loops because conditions like `count >= 3` are compiled with the original value (always `false`).

## Root Cause

In SSA (Static Single Assignment) form, values are immutable. For mutable variables to work across control flow boundaries (loops, conditionals), the compiler must either:

1. **Memory-based approach**: `alloca` (stack slot) + `store` (write) + `load` (read)
2. **Phi nodes**: SSA construct to merge values from different control flow paths

The current code does neither—it just updates a hashmap during codegen, which has no effect on already-compiled IR.

## Design Principles

### Principle 1: Mutability-Driven Allocation

Unlike Rust (which analyzes usage), Ori should use **explicit mutability** as the deciding factor:

| Declaration | Storage | Access |
|------------|---------|--------|
| `let x = 5` | SSA register | Direct value |
| `let mut x = 5` | Stack (`alloca`) | `load`/`store` |

This matches Ori's language semantics where `let mut` is an explicit opt-in.

### Principle 2: Trust LLVM's mem2reg

LLVM's `mem2reg` pass automatically promotes stack allocations to registers when safe:
- Variable never has address taken
- Only accessed via `load`/`store` (no pointer arithmetic)
- All stores dominate all loads

This means we can **always use alloca for mutable variables** without worrying about optimization—LLVM handles it.

### Principle 3: Alloca at Function Entry

For `mem2reg` to work correctly, allocas must be in the function's entry block:

```llvm
define i64 @example() {
entry:
  %count = alloca i64, align 8    ; <-- ALL allocas here
  %temp = alloca i64, align 8
  store i64 0, ptr %count         ; Initialize after alloca
  br label %loop_header
  ...
}
```

## Architecture

### New Data Structures

```rust
// In compiler/ori_llvm/src/locals.rs (new file)

use inkwell::values::{BasicValueEnum, PointerValue};
use inkwell::types::BasicTypeEnum;
use rustc_hash::FxHashMap;
use ori_ir::Name;

/// How a local variable is stored in LLVM IR
#[derive(Debug, Clone, Copy)]
pub enum LocalStorage<'ll> {
    /// Immutable binding - value lives in SSA register
    /// Created by: `let x = value`
    Immutable(BasicValueEnum<'ll>),

    /// Mutable variable - value lives on stack via alloca
    /// Created by: `let mut x = value`
    Mutable {
        /// Pointer to stack slot (from alloca)
        ptr: PointerValue<'ll>,
        /// Type of the stored value (for load instruction)
        ty: BasicTypeEnum<'ll>,
    },
}

/// Manages local variable storage for a function
#[derive(Debug)]
pub struct Locals<'ll> {
    bindings: FxHashMap<Name, LocalStorage<'ll>>,
}

impl<'ll> Locals<'ll> {
    pub fn new() -> Self {
        Self {
            bindings: FxHashMap::default(),
        }
    }

    /// Bind an immutable variable (let x = value)
    pub fn bind_immutable(&mut self, name: Name, value: BasicValueEnum<'ll>) {
        self.bindings.insert(name, LocalStorage::Immutable(value));
    }

    /// Bind a mutable variable (let mut x = value)
    /// The ptr should come from an alloca at function entry
    pub fn bind_mutable(
        &mut self,
        name: Name,
        ptr: PointerValue<'ll>,
        ty: BasicTypeEnum<'ll>,
    ) {
        self.bindings.insert(name, LocalStorage::Mutable { ptr, ty });
    }

    /// Get storage info for a variable
    pub fn get(&self, name: Name) -> Option<LocalStorage<'ll>> {
        self.bindings.get(&name).copied()
    }

    /// Check if a variable is mutable
    pub fn is_mutable(&self, name: Name) -> bool {
        matches!(self.bindings.get(&name), Some(LocalStorage::Mutable { .. }))
    }

    /// Get the mutable pointer for a variable (for store operations)
    pub fn get_mutable_ptr(&self, name: Name) -> Option<(PointerValue<'ll>, BasicTypeEnum<'ll>)> {
        match self.bindings.get(&name)? {
            LocalStorage::Mutable { ptr, ty } => Some((*ptr, *ty)),
            LocalStorage::Immutable(_) => None,
        }
    }
}
```

### Builder Extensions

```rust
// In compiler/ori_llvm/src/builder.rs

impl<'ll, 'cx> Builder<'ll, 'cx> {
    /// Create an alloca at function entry (required for mem2reg)
    ///
    /// This temporarily repositions the builder to the entry block,
    /// creates the alloca, then returns to the current position.
    pub fn create_entry_alloca(
        &self,
        ty: BasicTypeEnum<'ll>,
        name: &str,
    ) -> PointerValue<'ll> {
        // Save current position
        let current_block = self.get_insert_block();

        // Get function's entry block
        let function = current_block
            .expect("must be in a block")
            .get_parent()
            .expect("block must have parent function");
        let entry_block = function.get_first_basic_block()
            .expect("function must have entry block");

        // Position at start of entry block (after existing allocas)
        match entry_block.get_first_instruction() {
            Some(first_instr) => self.position_before(&first_instr),
            None => self.position_at_end(entry_block),
        }

        // Create alloca
        let alloca = self.build_alloca(ty, name);

        // Restore position
        if let Some(block) = current_block {
            self.position_at_end(block);
        }

        alloca
    }

    /// Bind a let statement, handling mutability correctly
    pub fn compile_let_with_mutability(
        &mut self,
        pattern: &BindingPattern,
        init: ExprId,
        mutable: bool,
        locals: &mut Locals<'ll>,
        arena: &ExprArena,
        interner: &StringInterner,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile initializer
        let value = self.compile_expr(init, locals, arena, interner)?;

        // Bind with appropriate storage
        self.bind_pattern_with_mutability(pattern, value, mutable, locals, interner);

        Some(value)
    }

    /// Bind a pattern, creating allocas for mutable bindings
    fn bind_pattern_with_mutability(
        &mut self,
        pattern: &BindingPattern,
        value: BasicValueEnum<'ll>,
        mutable: bool,
        locals: &mut Locals<'ll>,
        interner: &StringInterner,
    ) {
        match &pattern.kind {
            BindingPatternKind::Name(name) => {
                if mutable {
                    // Create stack storage for mutable variable
                    let ty = value.get_type();
                    let name_str = interner.resolve(*name);
                    let ptr = self.create_entry_alloca(ty, name_str);

                    // Store initial value
                    self.build_store(ptr, value);

                    // Register as mutable
                    locals.bind_mutable(*name, ptr, ty);
                } else {
                    // Immutable: direct SSA binding
                    locals.bind_immutable(*name, value);
                }
            }

            BindingPatternKind::Tuple(patterns) => {
                // Extract tuple elements and recursively bind
                for (i, sub_pattern) in patterns.iter().enumerate() {
                    let element = self.build_extract_value(
                        value.into_struct_value(),
                        i as u32,
                        "tuple_elem",
                    );
                    self.bind_pattern_with_mutability(
                        sub_pattern, element.into(), mutable, locals, interner
                    );
                }
            }

            // ... other pattern kinds (Struct, List, Wildcard)

            BindingPatternKind::Wildcard => {
                // Discard value, no binding needed
            }
        }
    }

    /// Get a variable's value, loading from stack if mutable
    pub fn get_variable(&self, name: Name, locals: &Locals<'ll>) -> Option<BasicValueEnum<'ll>> {
        match locals.get(name)? {
            LocalStorage::Immutable(value) => Some(value),
            LocalStorage::Mutable { ptr, ty } => {
                // Load current value from stack
                Some(self.build_load(ty, ptr, "load_mut"))
            }
        }
    }

    /// Assign to a mutable variable
    pub fn assign_variable(
        &self,
        name: Name,
        value: BasicValueEnum<'ll>,
        locals: &Locals<'ll>,
    ) -> Result<(), &'static str> {
        match locals.get(name) {
            Some(LocalStorage::Mutable { ptr, .. }) => {
                self.build_store(ptr, value);
                Ok(())
            }
            Some(LocalStorage::Immutable(_)) => {
                Err("cannot assign to immutable variable")
            }
            None => {
                Err("undefined variable")
            }
        }
    }
}
```

### Updated compile_expr for Ident

```rust
// In builder.rs, compile_expr match arm for Ident

ExprKind::Ident(name) => {
    // Use get_variable which handles both immutable and mutable
    self.get_variable(*name, locals)
}
```

### Updated compile_assign

```rust
// In control_flow.rs

pub fn compile_assign(
    &mut self,
    target: ExprId,
    value: ExprId,
    locals: &mut Locals<'ll>,
    arena: &ExprArena,
    interner: &StringInterner,
) -> Option<BasicValueEnum<'ll>> {
    // Compile the value to assign
    let val = self.compile_expr(value, locals, arena, interner)?;

    let target_expr = arena.get(target);
    match &target_expr.kind {
        ExprKind::Ident(name) => {
            // Store to mutable variable
            match self.assign_variable(*name, val, locals) {
                Ok(()) => Some(val),
                Err(msg) => {
                    // This should be caught by type checker, but handle gracefully
                    eprintln!("LLVM codegen error: {}", msg);
                    None
                }
            }
        }

        ExprKind::FieldAccess { object, field } => {
            // TODO: Handle mutable field assignment
            // 1. Get object pointer (must be mutable)
            // 2. GEP to field
            // 3. Store value
            None
        }

        ExprKind::Index { object, index } => {
            // TODO: Handle mutable index assignment
            // 1. Get collection pointer (must be mutable)
            // 2. Compute element address
            // 3. Store value
            None
        }

        _ => None,
    }
}
```

## Migration Path

### Phase 1: Introduce LocalStorage (Non-Breaking)

1. Create `locals.rs` with `LocalStorage` and `Locals` types
2. Add `create_entry_alloca` to builder
3. Add `get_variable` and `assign_variable` methods
4. Keep existing `FxHashMap<Name, BasicValueEnum>` API working

### Phase 2: Update Let Compilation

1. Update `compile_let` to check `mutable` flag
2. Route mutable bindings through `create_entry_alloca` + `store`
3. Update `bind_pattern` to handle mutability

### Phase 3: Update Variable Access

1. Replace direct hashmap lookups with `get_variable`
2. Replace hashmap inserts in assign with `assign_variable`
3. Update all call sites

### Phase 4: Update Control Flow

1. Ensure loop compilation works with new storage model
2. Verify phi nodes work correctly for immutable vars in branches
3. Add tests for all control flow patterns

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_mutable_in_loop() {
    let code = r#"
        let mut count = 0,
        loop(run(
            count = count + 1,
            if count >= 3 then break,
        )),
        count
    "#;
    assert_eq!(eval(code), Value::Int(3));
}

#[test]
fn test_mutable_in_conditional() {
    let code = r#"
        let mut x = 1,
        if true then x = 2,
        x
    "#;
    assert_eq!(eval(code), Value::Int(2));
}

#[test]
fn test_immutable_unchanged() {
    let code = r#"
        let x = 5,
        x + 1
    "#;
    assert_eq!(eval(code), Value::Int(6));
}

#[test]
fn test_mutable_in_for_loop() {
    let code = r#"
        let mut sum = 0,
        for i in 1..=5 then sum = sum + i,
        sum
    "#;
    assert_eq!(eval(code), Value::Int(15));
}
```

### LLVM IR Verification

Verify generated IR for `let mut count = 0; count = count + 1`:

```llvm
; Expected:
entry:
  %count = alloca i64, align 8
  store i64 0, ptr %count, align 8
  %0 = load i64, ptr %count, align 8
  %1 = add i64 %0, 1
  store i64 %1, ptr %count, align 8
```

## Comparison with Reference Compilers

| Aspect | Rust | Zig | Ori (Proposed) |
|--------|------|-----|----------------|
| Decides memory vs SSA | Usage analysis | Type-based (`isByRef`) | Mutability flag |
| Alloca placement | Entry block | Entry block | Entry block |
| Optimization | mem2reg | mem2reg | mem2reg |
| Complexity | High (analysis pass) | Medium | Low |

## Files to Modify

| File | Changes |
|------|---------|
| `compiler/ori_llvm/src/locals.rs` | **NEW**: LocalStorage, Locals |
| `compiler/ori_llvm/src/builder.rs` | Add `create_entry_alloca`, `get_variable`, `assign_variable`, update `compile_let` |
| `compiler/ori_llvm/src/control_flow.rs` | Update `compile_assign`, verify loop handling |
| `compiler/ori_llvm/src/functions/sequences.rs` | Update `bind_pattern` for mutability |
| `compiler/ori_llvm/src/lib.rs` | Export new module |
| `compiler/ori_llvm/src/evaluator.rs` | Update to use `Locals` instead of `FxHashMap` |

## Open Questions

1. **Nested mutability**: How should `let mut (a, b) = (1, 2)` work? Currently proposed: all elements become mutable.

2. **Shadowing**: `let x = 1; let mut x = 2;` — the second binding shadows the first with different storage. This should work naturally.

3. **Closures**: Mutable variables captured by closures need careful handling (probably copy-in for now).

## Success Criteria

1. ✅ `let mut` variables work correctly in loops
2. ✅ `let mut` variables work correctly in conditionals
3. ✅ `let` (immutable) variables continue to work efficiently
4. ✅ LLVM IR uses proper `alloca`/`load`/`store` for mutable vars
5. ✅ All existing tests pass
6. ✅ New tests for mutable variable patterns pass
