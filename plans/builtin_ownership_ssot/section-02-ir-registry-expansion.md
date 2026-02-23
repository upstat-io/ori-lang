---
section: "02"
title: "Expand IR Registry to Cover Codegen-Backed Types"
status: not-started
goal: "Every codegen builtin handler has a corresponding MethodDef with ownership"
files:
  - compiler/ori_ir/src/builtin_type/mod.rs
  - compiler/ori_ir/src/builtin_methods/mod.rs
  - compiler/ori_ir/src/builtin_methods/collections.rs
  - compiler/ori_ir/src/builtin_methods/wrappers.rs
  - compiler/ori_ir/src/builtin_methods/tests.rs
---

# Section 02: Expand IR Registry to Cover Codegen-Backed Types

**Status:** Not Started
**Goal:** Every `declare_builtins!` entry in `ori_llvm` has a corresponding `MethodDef` in `ori_ir` with explicit ownership and type flow. Methods not yet implemented in codegen are added to the IR registry when they get implemented — tracked via roadmap checkboxes.

---

## 02.0 Scope: Codegen-Backed Only

The original plan attempted to add ~133 entries for all TYPECK methods across 11 types. This revised scope adds entries **only for methods that have LLVM codegen handlers today** — approximately 49 new entries across 6 types.

**Why not all TYPECK methods?** Many methods exist only in the type checker and evaluator (e.g., `list.push`, `map.remove`, `Option.and_then`). Adding MethodDef entries for methods without codegen handlers creates registry bloat with no consumer. The enforcement test (`every_codegen_builtin_has_ir_method_def`) ensures that any *future* codegen handler requires a MethodDef — methods get added to the registry when they get implemented.

**Roadmap integration:** Each method that gets an LLVM codegen handler should include a checkbox: "Add MethodDef to IR registry with `receiver_borrows`". This mirrors the existing "Write LLVM test" checkbox pattern.

---

## 02.1 BuiltinType Additions

**File:** `compiler/ori_ir/src/builtin_type/mod.rs`

Current `BuiltinType` covers: Int, Float, Bool, Str, Char, Byte, Unit, Never, Duration, Size, Ordering, List, Map, Option, Result, Range, Set, Channel.

**Missing for codegen-backed types:**

```rust
pub enum BuiltinType {
    // ... existing variants ...
    Iterator,   // NEW — codegen has 15 entries for this type
    Tuple,      // NEW — codegen has 4 entries for this type
}
```

Update these methods for new variants:
- `from_name()` — add `"Iterator" => Some(Self::Iterator)`, `"tuple" => Some(Self::Tuple)`
- `name()` — add `Self::Iterator => "Iterator"`, `Self::Tuple => "tuple"`
- `is_container()` — `Iterator` and `Tuple` are containers (parameterized)

**NOT added:** `DoubleEndedIterator`, `Error` — these have no codegen handlers today. Added when codegen is implemented.

---

## 02.2 ReturnSpec Additions

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs`

```rust
pub enum ReturnSpec {
    // ... existing variants ...

    /// Returns an Iterator over the element type.
    /// Used by: list.iter(), Set.iter(), map.iter(), range.iter(), str.iter()
    IteratorElement,
}
```

**NOT added (deferred to when methods are implemented):**
- `KeyList` / `ValueList` / `EntryList` — for map.keys(), map.values(), map.entries()
- Tuple return variants — for Iterator.next()

---

## 02.3 File Split

At ~211 entries (162 existing + 49 new), splitting is recommended but not strictly required. Split into submodules by category:

### New File Layout

```
compiler/ori_ir/src/builtin_methods/
├── mod.rs              — types (MethodDef, ParamSpec, ReturnSpec), query functions,
│                         BUILTIN_METHODS composed from submodule arrays
├── primitives.rs       — int, float, bool, char, byte (61 entries)
├── special_types.rs    — str, Duration, Size, Ordering (62 entries, existing + str.iter)
├── collections.rs      — list, map, Set, range (24 entries, NEW)
├── wrappers.rs         — Option, Result, Iterator, tuple (39 entries, NEW)
└── tests.rs            — tests for all submodules
```

### Composition Pattern

Each submodule exports a `const` array:

```rust
// collections.rs
pub(super) const METHODS: &[MethodDef] = &[
    MethodDef::new(BuiltinType::List, "len", &[], ReturnSpec::Type(BuiltinType::Int), None, true),
    // ...
];
```

`mod.rs` composes them:

```rust
/// Iterate over all builtin method definitions across all submodules.
pub fn all_methods() -> impl Iterator<Item = &'static MethodDef> {
    primitives::METHODS.iter()
        .chain(special_types::METHODS.iter())
        .chain(collections::METHODS.iter())
        .chain(wrappers::METHODS.iter())
}
```

Update `find_method()`, `methods_for()`, `borrowing_method_names()`, and `method_borrows_receiver()` to use `all_methods()`.

---

## 02.4 New Entries: Codegen-Backed Methods

These are the exact methods that have `declare_builtins!` entries in `ori_llvm` but lack `MethodDef` entries in `ori_ir`. All `receiver_borrows: true`. All `type_flow: TypeFlow::Standard` unless noted.

### collections.rs — list, map, Set, range (24 entries)

**list** (7 entries):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `len` | `[]` | `Type(Int)` | None | `Standard` | collections.rs |
| `is_empty` | `[]` | `Type(Bool)` | None | `Standard` | collections.rs |
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | collections.rs |
| `iter` | `[]` | `IteratorElement` | None | `Standard` | collections.rs |
| `equals` | `[SelfType]` | `Type(Bool)` | `"Eq"` | `Standard` | compound_traits.rs |
| `compare` | `[SelfType]` | `Type(Ordering)` | `"Comparable"` | `Standard` | compound_traits.rs |
| `hash` | `[]` | `Type(Int)` | `"Hashable"` | `Standard` | compound_traits.rs |

**map** (4 entries):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `len` | `[]` | `Type(Int)` | None | `Standard` | collections.rs |
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | collections.rs |
| `iter` | `[]` | `IteratorElement` | None | `Standard` | collections.rs |
| `is_empty` | `[]` | `Type(Bool)` | None | `Standard` | — (not in codegen yet but trivially derivable; omit until codegen adds it) |

**Wait — map.is_empty is NOT in codegen.** Correcting: map has 3 entries from codegen:

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `len` | `[]` | `Type(Int)` | None | `Standard` | collections.rs |
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | collections.rs |
| `iter` | `[]` | `IteratorElement` | None | `Standard` | collections.rs |

**Set** (3 entries):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `len` | `[]` | `Type(Int)` | None | `Standard` | collections.rs |
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | collections.rs |
| `iter` | `[]` | `IteratorElement` | None | `Standard` | collections.rs |

**range** (1 entry):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `iter` | `[]` | `IteratorElement` | None | `Standard` | collections.rs |

**Subtotal: 14 entries**

### wrappers.rs — Option, Result, Iterator, tuple (35 entries)

**Option** (8 entries):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `is_some` | `[]` | `Type(Bool)` | None | `Standard` | option_result.rs |
| `is_none` | `[]` | `Type(Bool)` | None | `Standard` | option_result.rs |
| `unwrap` | `[]` | `InnerType` | None | `Standard` | option_result.rs |
| `unwrap_or` | `[Any]` | `InnerType` | None | `Standard` | option_result.rs |
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | option_result.rs |
| `equals` | `[SelfType]` | `Type(Bool)` | `"Eq"` | `Standard` | compound_traits.rs |
| `compare` | `[SelfType]` | `Type(Ordering)` | `"Comparable"` | `Standard` | compound_traits.rs |
| `hash` | `[]` | `Type(Int)` | `"Hashable"` | `Standard` | compound_traits.rs |

**Result** (9 entries):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `is_ok` | `[]` | `Type(Bool)` | None | `Standard` | option_result.rs |
| `is_err` | `[]` | `Type(Bool)` | None | `Standard` | option_result.rs |
| `unwrap` | `[]` | `InnerType` | None | `Standard` | option_result.rs |
| `unwrap_err` | `[]` | `InnerType` | None | `Standard` | option_result.rs |
| `unwrap_or` | `[Any]` | `InnerType` | None | `Standard` | option_result.rs |
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | option_result.rs |
| `equals` | `[SelfType]` | `Type(Bool)` | `"Eq"` | `Standard` | compound_traits.rs |
| `compare` | `[SelfType]` | `Type(Ordering)` | `"Comparable"` | `Standard` | compound_traits.rs |
| `hash` | `[]` | `Type(Int)` | `"Hashable"` | `Standard` | compound_traits.rs |

**Iterator** (14 entries, excluding `__iter_next` which is a pipeline-internal method):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `take` | `[Int]` | `SelfType` | None | `Standard` | iterator.rs |
| `skip` | `[Int]` | `SelfType` | None | `Standard` | iterator.rs |
| `chain` | `[SelfType]` | `SelfType` | None | `Standard` | iterator.rs |
| `enumerate` | `[]` | `SelfType` | None | `Standard` | iterator.rs |
| `zip` | `[Any]` | `SelfType` | None | `Standard` | iterator.rs |
| `map` | `[Closure]` | `SelfType` | None | **`ClosureOutputBecomesElement { closure_param: 0 }`** | iterator.rs |
| `filter` | `[Closure]` | `SelfType` | None | `Standard` | iterator.rs |
| `collect` | `[]` | `ListElement` | None | `Standard` | iterator.rs |
| `count` | `[]` | `Type(Int)` | None | `Standard` | iterator.rs |
| `any` | `[Closure]` | `Type(Bool)` | None | `Standard` | iterator.rs |
| `all` | `[Closure]` | `Type(Bool)` | None | `Standard` | iterator.rs |
| `find` | `[Closure]` | `OptionElement` | None | `Standard` | iterator.rs |
| `for_each` | `[Closure]` | `Void` | None | `Standard` | iterator.rs |
| `fold` | `[Any, Closure]` | `Any` | None | **`Accumulator { init_param: 0, closure_param: 1 }`** | iterator.rs |

**Note:** `rfold` (on `DoubleEndedIterator`) also uses `Accumulator { init_param: 0, closure_param: 1 }` — added when DEI entries are created.

**Note:** `flat_map` (deferred to roadmap) uses `ClosureOutputFlatElement { closure_param: 0 }`.

**tuple** (4 entries):

| Method | Params | Returns | Trait | TypeFlow | Source |
|--------|--------|---------|-------|----------|--------|
| `clone` | `[]` | `SelfType` | `"Clone"` | `Standard` | compound_traits.rs |
| `equals` | `[SelfType]` | `Type(Bool)` | `"Eq"` | `Standard` | compound_traits.rs |
| `compare` | `[SelfType]` | `Type(Ordering)` | `"Comparable"` | `Standard` | compound_traits.rs |
| `hash` | `[]` | `Type(Int)` | `"Hashable"` | `Standard` | compound_traits.rs |

**Subtotal: 35 entries**

---

## 02.5 Codegen Aliases and Pipeline Methods

These codegen entries do NOT need MethodDef entries — they're handled by exemption lists in the enforcement test:

**Codegen aliases** (mapped to canonical names at test time):
- `length` → `len` (on list, map, Set, str)
- `is_equal` → `equals` (on list, Option, Result, tuple)

**ARC pipeline methods** (reached via lowering/desugaring, not user calls):
- `("Iterator", "__iter_next")` — for-loop iteration protocol
- `("Ordering", "to_int")` — ordering → int conversion
- `("int", "byte")` — constructor lowering
- `("int", "f")` — constructor lowering
- `("int", "to_int")` — identity conversion
- `("str", "concat")` — interpolation lowering
- `("str", "to_str")` — identity in format paths

---

## 02.6 Entry Count Summary

| Submodule | Types | Entries |
|-----------|-------|--------|
| `primitives.rs` | int, float, bool, char, byte | 61 (existing, moved) |
| `special_types.rs` | str, Duration, Size, Ordering | 62 (existing, moved) |
| `collections.rs` | list, map, Set, range | 14 (**new**) |
| `wrappers.rs` | Option, Result, Iterator, tuple | 35 (**new**) |
| **Total** | **15** | **~172** |

---

## 02.7 Roadmap Updates — Add MethodDef Checkbox to Every Method

**Goal:** Every method in the roadmap gets a `**MethodDef**` checkbox, mirroring the existing `**LLVM Support**` and `**LLVM Rust Tests**` pattern. This ensures MethodDef entries are added as part of the implementation process, not as an afterthought.

### New checkbox pattern

Every method item gains a `**MethodDef**` sub-checkbox:

```markdown
- [ ] **Implement**: `[T].push(value: T) -> [T]` — modules/prelude.md § List
  - [ ] **MethodDef**: `ori_ir/src/builtin_methods/collections.rs` — list push with `receiver_borrows: true`
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list push tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list push
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list push codegen
```

For methods that already have codegen AND are getting MethodDef entries in this section, mark the checkbox `[x]`.

### Roadmap files to update

#### `plans/roadmap/section-07B-option-result.md`

Add `**MethodDef**` checkbox to every method item:

**7B.1 Option Functions** — add to each of: `is_some`, `is_none`, `Option.map`, `Option.unwrap_or`, `Option.ok_or`, `Option.and_then`, `Option.filter`

```markdown
- [x] **Implement**: `is_some(x)` — spec/11-built-in-functions.md § is_some [done] (2026-02-10)
  - [x] **MethodDef**: `ori_ir/src/builtin_methods/wrappers.rs` — Option is_some with `receiver_borrows: true`
  ...
```

- `is_some` → `[x]` (codegen exists, MethodDef added in this section)
- `is_none` → `[x]`
- `Option.map` → `[ ]` (no codegen yet)
- `Option.unwrap_or` → `[x]`
- `Option.ok_or` → `[ ]`
- `Option.and_then` → `[ ]`
- `Option.filter` → `[ ]`

**7B.2 Result Functions** — add to each of: `is_ok`, `is_err`, `Result.map`, `Result.map_err`, etc.

- `is_ok` → `[x]`
- `is_err` → `[x]`
- `Result.map` → `[ ]`
- `Result.map_err` → `[ ]`
- (etc.)

#### `plans/roadmap/section-07C-collections.md`

**7C.1 Collection Functions:**
- `len(x)` → `[x]` (already in IR as primitive method)
- `is_empty(x)` → `[x]`

**7C.2 Collection Methods on `[T]`:**
- `[T].map` → `[ ]` (no codegen for list.map yet)
- `[T].filter` → `[ ]`
- `[T].fold` → `[ ]`
- `[T].find` → `[ ]`
- `[T].any` → `[ ]`
- `[T].all` → `[ ]`
- `[T].first` → `[ ]`
- `[T].last` → `[ ]`
- `[T].take` → `[ ]`
- `[T].skip` → `[ ]`
- `[T].reverse` → `[ ]`
- `[T].sort` → `[ ]`
- `[T].contains` → `[ ]`
- `[T].push` → `[ ]`
- `[T].concat` → `[ ]`

**7C.3 Range Methods:**
- `Range.map` → `[ ]`
- `Range.filter` → `[ ]`
- `Range.fold` → `[ ]`
- `Range.collect` → `[ ]`
- `Range.contains` → `[ ]`

**7C.4 Collection Methods (len, is_empty):**
- `[T].len()` → `[x]` (MethodDef added in this section)
- `[T].is_empty()` → `[x]`
- `{K:V}.len()` → `[x]`
- `{K:V}.is_empty()` → `[ ]` (no codegen)
- `str.len()` → `[x]` (already in IR)
- `str.is_empty()` → `[x]` (already in IR)
- `Set<T>.len()` → `[x]`
- `Set<T>.is_empty()` → `[ ]` (no codegen)

**7C.5 Comparable Methods:** Already in IR for primitive types.

**7C.6 Iterator Traits:** All `[ ]` (trait formalization not started)

**7C.7 Debug Trait:** All `[ ]`

#### `plans/roadmap/section-21A-llvm.md`

If this section tracks builtin codegen methods, add `**MethodDef**` checkboxes to each. Many will be `[x]` since the 9 primitive types already have MethodDef entries.

### Enforcement: the test catches forgetting

The enforcement test from Section 05 (`every_codegen_builtin_has_ir_method_def`) ensures that if someone checks off `**LLVM Support**` without checking off `**MethodDef**`, the test suite fails. This makes the checkbox not just documentation but a structurally-enforced requirement.

### Methods deferred to roadmap checkboxes

These TYPECK methods have no codegen handler yet. They get `[ ]` MethodDef checkboxes in the roadmap, to be checked when codegen is implemented:

**list:** push, pop, get, set, first, last, contains, index_of, slice, take, skip, reverse, sort, sort_by, map, filter, flat_map, fold, reduce, any, all, find, zip, enumerate, chunk, window, dedup, flatten, join, repeat, concat, append, insert, remove, swap, split_at, partition, count, sum, product, min, max, min_by, max_by, unique, sorted

**map:** is_empty, get, contains_key, contains, insert, remove, update, merge, keys, values, entries

**Set:** is_empty, contains, insert, remove, union, intersection, difference, to_list

**Option:** map, and_then, flat_map, filter, or, or_else, ok_or, iter, expect, debug

**Result:** map, map_err, and_then, or_else, ok, err, expect, expect_err, has_trace, trace, debug

**Iterator:** join, next, cycle, flat_map, flatten

**tuple:** len, debug

**range:** len, count, is_empty, contains, to_list, collect, step_by

**error, Channel, DoubleEndedIterator:** entire types deferred — roadmap items created when these types get codegen

---

## 02.8 Consistency Test Updates

**File:** `compiler/oric/src/eval/tests/methods/consistency.rs`

After adding codegen-backed entries:
- `COLLECTION_TYPES` — reduce from 11 to ~5 (list, map, Set, range, tuple now in IR; Iterator if added)
- `EVAL_METHODS_NOT_IN_IR` — reduce by entries now covered
- `TYPECK_METHODS_NOT_IN_IR` — reduce by entries now covered

**Do NOT eliminate these lists entirely** — many TYPECK/EVAL methods still won't have IR entries (they don't have codegen). The lists track genuine gaps, not implementation todos.

---

## 02.9 Implementation Order

1. Add `Iterator` and `Tuple` variants to `BuiltinType` + update `from_name()`, `name()`, etc.
2. Add `IteratorElement` variant to `ReturnSpec`
3. Extract existing 162 entries into `primitives.rs` and `special_types.rs` (mechanical move)
4. Create `collections.rs` with 14 new entries
5. Create `wrappers.rs` with 35 new entries
6. Update `mod.rs`: compose submodule arrays into `all_methods()`, update query functions
7. Update consistency tests: reduce gap lists
8. Verify: `cargo c -p ori_ir`, `cargo t -p ori_ir`, `cargo t -p oric`

---

## 02.10 Verification

- [ ] `cargo c -p ori_ir` — compiles with all new entries
- [ ] `cargo t -p ori_ir` — IR registry tests pass
- [ ] `cargo t -p oric` — consistency tests pass with updated gap lists
- [ ] Every `declare_builtins!` entry (non-alias, non-pipeline) has a corresponding MethodDef
- [ ] All new entries have explicit `receiver_borrows: true`
- [ ] File sizes under 500 lines per submodule

---

## Appendix: Full TYPECK Method Reference

For reference when implementing future methods, the exhaustive TYPECK method lists are preserved here. These are NOT implementation targets for this section — they document what exists in TYPECK for future roadmap items.

<details>
<summary>list (49 TYPECK entries)</summary>

all, any, append, chunk, clone, compare, contains, count, debug, enumerate, equals, filter, find, first, flat_map, flatten, fold, for_each, get, group_by, hash, is_empty, iter, join, last, len, map, max, max_by, min, min_by, partition, pop, prepend, product, push, reduce, reverse, skip, skip_while, sort, sort_by, sorted, sum, take, take_while, unique, window, zip
</details>

<details>
<summary>map (17 TYPECK entries)</summary>

clone, contains, contains_key, debug, entries, equals, get, hash, insert, is_empty, iter, keys, len, merge, remove, update, values
</details>

<details>
<summary>Set (14 TYPECK entries)</summary>

clone, contains, debug, difference, equals, hash, insert, intersection, into, is_empty, iter, len, remove, to_list, union
</details>

<details>
<summary>Option (18 TYPECK entries)</summary>

and_then, clone, compare, debug, equals, expect, filter, flat_map, hash, is_none, is_some, iter, map, ok_or, or, or_else, unwrap, unwrap_or
</details>

<details>
<summary>Result (20 TYPECK entries)</summary>

and_then, clone, compare, debug, equals, err, expect, expect_err, has_trace, hash, is_err, is_ok, map, map_err, ok, or_else, trace, trace_entries, unwrap, unwrap_err, unwrap_or
</details>

<details>
<summary>Iterator (18 TYPECK entries)</summary>

all, any, chain, collect, count, cycle, enumerate, filter, find, flat_map, flatten, fold, for_each, join, map, next, skip, take, zip
</details>

<details>
<summary>DoubleEndedIterator (5 TYPECK entries)</summary>

last, next_back, rev, rfind, rfold
</details>

<details>
<summary>range (8 TYPECK entries)</summary>

collect, contains, count, is_empty, iter, len, step_by, to_list
</details>

<details>
<summary>tuple (6 TYPECK entries)</summary>

clone, compare, debug, equals, hash, len
</details>

<details>
<summary>error (8 TYPECK entries)</summary>

clone, debug, has_trace, message, to_str, trace, trace_entries, with_trace
</details>

<details>
<summary>Channel (9 TYPECK entries)</summary>

close, is_closed, is_empty, len, receive, recv, send, try_receive, try_recv
</details>
