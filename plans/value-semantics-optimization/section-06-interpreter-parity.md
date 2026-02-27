---
section: "06"
title: "Interpreter COW Parity"
status: not-started
goal: "The interpreter achieves the same COW semantics as the LLVM backend using Arc::make_mut()"
inspired_by:
  - "Rust std::sync::Arc::make_mut — built-in COW for Arc-wrapped data"
  - "Rust std::borrow::Cow — clone-on-write wrapper"
depends_on: ["02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "List COW via Arc::make_mut()"
    status: not-started
  - id: "06.2"
    title: "Map & Set COW via Arc::make_mut()"
    status: not-started
  - id: "06.3"
    title: "String COW in Interpreter"
    status: not-started
  - id: "06.4"
    title: "Slice Support in Interpreter"
    status: not-started
  - id: "06.5"
    title: "Behavioral Equivalence Verification"
    status: not-started
  - id: "06.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Interpreter COW Parity

**Status:** Not Started
**Goal:** The interpreter (`ori_eval`) uses Rust's `Arc::make_mut()` to achieve COW semantics for all collection mutations. Behavioral equivalence between the interpreter and LLVM backend is verified via dual-execution tests.

**Context:** Currently, the interpreter clones `Arc<Vec<Value>>` on every mutation:
```rust
let mut result = (*items).clone();  // Always clones, even when Arc refcount is 1
result.push(args.swap_remove(0));
Ok(Value::list(result))
```

`Arc::make_mut()` is Rust's built-in COW for `Arc`: if the refcount is 1, it gives a mutable reference to the inner data (no clone). If the refcount is > 1, it clones the inner data, reduces the refcount, and returns a mutable reference to the clone. This is *exactly* the COW semantics we implement in the runtime (§02).

**Reference implementations:**
- **Rust** `std::sync::Arc::make_mut()`: The standard library provides this exact pattern. We just need to use it instead of `(*items).clone()`.

**Depends on:** Sections 02-05 (all COW operations must be defined before the interpreter can implement them).

---

## 06.1 List COW via Arc::make_mut()

**File(s):** `compiler/ori_eval/src/methods/collections.rs`

- [ ] Replace all list mutation patterns from clone-always to COW:

  **Before:**
  ```rust
  "push" => {
      require_args("push", &args, 1)?;
      let mut result = (*items).clone();
      result.push(args.swap_remove(0));
      Ok(Value::list(result))
  }
  ```

  **After:**
  ```rust
  "push" => {
      require_args("push", &args, 1)?;
      let mut items_arc = items;  // Move the Arc
      Arc::make_mut(&mut items_arc).push(args.swap_remove(0));
      Ok(Value::List(Heap(items_arc)))
  }
  ```

  **Key insight:** `Arc::make_mut()` requires `&mut Arc<T>`. Currently, the interpreter receives `items: &Heap<Vec<Value>>` (a shared reference to the Arc). To use `make_mut`, we need to *move* the Arc out of the Value, modify it, and put it back. This requires changing the method dispatch signature to take `Value` by ownership (not by reference).

  **Design decision — dispatch ownership:**

  **(a) Clone the Arc before make_mut** (safe, minimal change):
  ```rust
  let mut items_arc = items.clone();  // Arc clone is cheap (RC inc)
  Arc::make_mut(&mut items_arc).push(elem);
  // If RC was 1: make_mut gives &mut, push is in-place. The clone made RC=2,
  // then make_mut detected RC>1 and cloned the Vec. WRONG — defeats COW!
  ```
  **This defeats COW.** The clone bumps RC to 2, so make_mut always clones.

  **(b) Take ownership of Value in method dispatch** (recommended):
  Change `dispatch_list_method` to take `receiver: Value` (by move) instead of `receiver: &Value`. This gives ownership of the Arc, so `make_mut` can check the true RC.

  ```rust
  fn dispatch_list_method(
      receiver: Value,      // Takes ownership (was &Value)
      method: Name,
      args: Vec<Value>,
  ) -> Result<Value, EvalError> {
      match receiver {
          Value::List(mut items) => {
              // items is Heap<Vec<Value>> = Arc<Vec<Value>>
              match method_name {
                  "push" => {
                      Heap::make_mut(&mut items).push(args.swap_remove(0));
                      Ok(Value::List(items))
                  }
                  // ... other methods
              }
          }
          _ => unreachable!()
      }
  }
  ```

  **Trade-off:** This requires updating the method dispatch call site to pass the receiver by value. For non-mutating methods (len, is_empty, etc.), the receiver is consumed unnecessarily — but since the Arc itself is cheap to clone, and most methods either return a primitive or the same Arc, this is acceptable.

  **Recommended:** Option (b) — take ownership. This is the only way to get true COW with Arc::make_mut().

- [ ] Update `dispatch_list_method` signature to take `Value` by ownership

- [ ] Update all list mutation methods:
  - `push` → `Heap::make_mut(&mut items).push(elem)`
  - `pop` → `Heap::make_mut(&mut items).pop()`
  - `reverse` → `Heap::make_mut(&mut items).reverse()`
  - `concat` / `add` → `Heap::make_mut(&mut items).extend(other.iter().cloned())`
  - `insert` → `Heap::make_mut(&mut items).insert(index, elem)`
  - `remove` → `Heap::make_mut(&mut items).remove(index)`
  - `sort` → `Heap::make_mut(&mut items).sort_by(cmp)`

- [ ] Non-mutating methods should clone the Arc (cheap) if needed, or borrow:
  - `len`, `is_empty`, `first`, `last`, `contains` → can borrow (no mutation)
  - `iter`, `clone`, `debug` → return new value, don't mutate

- [ ] Unit tests:
  - Push to list with single Arc reference → no Vec clone (verify via Arc::strong_count)
  - Push to list with two Arc references → Vec cloned, original unchanged
  - Chain of 100 pushes on unique list → no intermediate Vec clones

---

## 06.2 Map & Set COW via Arc::make_mut()

**File(s):** `compiler/ori_eval/src/methods/collections.rs`

- [ ] Update `dispatch_map_method` signature to take `Value` by ownership

- [ ] Update map mutations:
  - `insert` → `Heap::make_mut(&mut map).insert(key, value)`
  - `remove` → `Heap::make_mut(&mut map).remove(&key)`

- [ ] Update `dispatch_set_method` signature to take `Value` by ownership

- [ ] Update set mutations:
  - `insert` → `Heap::make_mut(&mut set).insert(key, value)`
  - `remove` → `Heap::make_mut(&mut set).remove(&key)`
  - `union` → merge into `make_mut` version
  - `intersection` → retain matching elements
  - `difference` → remove matching elements

- [ ] Unit tests for map/set COW with Arc reference counting

---

## 06.3 String COW in Interpreter

**File(s):** `compiler/ori_eval/src/methods/collections.rs`, `compiler/ori_patterns/src/value/mod.rs`

The interpreter's `Value::Str` currently uses `Heap<Cow<'static, str>>`. The `Cow` already provides some COW behavior (borrowed vs owned), but it's not the same as the runtime's SSO + heap COW.

- [ ] **Decision: Keep `Heap<Cow<str>>` for interpreter strings** — the interpreter doesn't need SSO (it's already using Rust's heap allocator, and the overhead is dwarfed by interpretation cost). The COW behavior comes from `Cow::to_mut()` which clones borrowed strings on first mutation.

- [ ] For string concat, use `Cow::to_mut()`:
  ```rust
  "concat" | "add" => {
      let other = require_str_arg("concat", &mut args, 0)?;
      Cow::to_mut(&mut Heap::make_mut(&mut s)).push_str(&other);
      Ok(Value::Str(s))
  }
  ```

- [ ] Update `dispatch_str_method` signature to take `Value` by ownership

- [ ] Unit tests for string COW behavior

---

## 06.4 Slice Support in Interpreter

**File(s):** `compiler/ori_eval/src/methods/collections.rs`, `compiler/ori_patterns/src/value/mod.rs`

The interpreter can represent slices as regular `Value::List` values (since `Vec<Value>` is heap-allocated and Arc-managed). A slice in the interpreter is just a `Value::List` with a subset of elements.

- [ ] **Design decision**: The interpreter uses Rust's `Vec::drain` / `Vec::split_off` for slicing, producing a new `Vec` that shares element `Value`s via Arc cloning. This is not truly zero-copy (the `Vec` struct is new, elements are Arc-cloned), but:
  - It's semantically correct (elements are shared, not deep-cloned)
  - Arc clone is O(1) per element (just RC increment)
  - The interpreter's primary goal is correctness, not performance
  - Full zero-copy slicing in the interpreter would require a custom slice type

- [ ] Add `slice` method to list dispatch:
  ```rust
  "slice" => {
      let start = require_int_arg("slice", &mut args, 0)? as usize;
      let end = require_int_arg("slice", &mut args, 1)? as usize;
      let sliced: Vec<Value> = items[start..end].to_vec();
      Ok(Value::list(sliced))
  }
  ```

- [ ] Add `take` and `drop` methods:
  ```rust
  "take" => {
      let n = require_int_arg("take", &mut args, 0)? as usize;
      let taken: Vec<Value> = items[..n.min(items.len())].to_vec();
      Ok(Value::list(taken))
  }
  "drop" => {
      let n = require_int_arg("drop", &mut args, 0)? as usize;
      let dropped: Vec<Value> = items[n.min(items.len())..].to_vec();
      Ok(Value::list(dropped))
  }
  ```

- [ ] Add `substring` and `trim` to string dispatch (if not already present)

---

## 06.5 Behavioral Equivalence Verification

**File(s):** `scripts/dual-exec-verify.sh`

- [ ] Extend `dual-exec-verify.sh` to include COW-specific test programs:
  - List push sequences (verify final result matches)
  - Shared list divergence (verify original unchanged after copy mutation)
  - Slice operations (verify slice content and independence)
  - String concat chains (verify final string matches)
  - Map/set mutations (verify contents after insert/remove sequences)

- [ ] Create test programs in `tests/spec/collections/cow/`:
  ```ori
  use std.testing { assert_eq }

  @test cow_push_unique {
      let list = [1, 2, 3]
      let list = list.push(4)
      let list = list.push(5)
      assert_eq(list, [1, 2, 3, 4, 5])
  }

  @test cow_push_shared {
      let a = [1, 2, 3]
      let b = a              // Sharing: RC = 2
      let b = b.push(4)      // COW: b gets its own copy
      assert_eq(a, [1, 2, 3])    // a unchanged
      assert_eq(b, [1, 2, 3, 4]) // b has the push
  }

  @test cow_slice {
      let list = [1, 2, 3, 4, 5]
      let slice = list.slice(1, 4)
      assert_eq(slice, [2, 3, 4])
      let modified = slice.push(6)
      assert_eq(modified, [2, 3, 4, 6])
      assert_eq(slice, [2, 3, 4])  // Slice unchanged
  }
  ```

- [ ] Run `dual-exec-verify.sh` on all COW test programs:
  ```bash
  ./scripts/dual-exec-verify.sh tests/spec/collections/cow/
  ```
  Expected: 0 mismatches.

---

## 06.6 Completion Checklist

- [ ] `dispatch_list_method` takes `Value` by ownership
- [ ] All list mutations use `Arc::make_mut()` / `Heap::make_mut()`
- [ ] `dispatch_map_method` takes `Value` by ownership
- [ ] All map mutations use `Heap::make_mut()`
- [ ] `dispatch_set_method` takes `Value` by ownership
- [ ] All set mutations use `Heap::make_mut()`
- [ ] String concat uses `Cow::to_mut()`
- [ ] Slice operations (slice, take, drop, substring, trim) implemented
- [ ] `dual-exec-verify.sh` passes with 0 mismatches on all COW tests
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `dual-exec-verify.sh tests/spec/collections/cow/` reports 0 mismatches. All existing spec tests pass via both interpreter (`cargo st`) and AOT (`./llvm-test.sh`). Arc::make_mut() is used consistently — no remaining `(*items).clone()` patterns in collection dispatch.
