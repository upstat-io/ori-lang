# Type Strategy Registry Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Supersedes:** `plans/builtin_ownership_ssot/`

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Core Data Model Design
**File:** `section-01-core-data-model.md` | **Status:** Not Started

```
TypeTag, MemoryStrategy, Ownership, OpStrategy
ParamDef, MethodDef, OpDefs, TypeDef
pure data, no behavior, no dependencies, const-constructible
enum, struct, static, compile-time, zero-cost
receiver, borrow, owned, copy, arc
IntInstr, FloatInstr, UnsignedCmp, BoolInstr, RuntimeCall, Unsupported
schema, contract, specification, declaration
SelfType, Iterator, Void, return type
extensibility, future fields, IterationDef, HashStrategy, DisplayStrategy
```

---

### Section 02: Crate Scaffolding & Purity Enforcement
**File:** `section-02-crate-scaffolding.md` | **Status:** Not Started

```
ori_registry, Cargo.toml, workspace, crate DAG
module structure, lib.rs, core.rs, method.rs, operator.rs, type_def.rs
defs/, defs/mod.rs, defs/int.rs, defs/str.rs
purity, no behavior, no logic, no trait impls with logic
no dependencies, zero deps, foundation crate, bottom of DAG
const, static, compile-time construction
enforcement test, purity test, no functions with side effects
workspace members, cargo check, cargo test
```

---

### Section 03: Primitive Type Definitions
**File:** `section-03-primitive-types.md` | **Status:** Not Started

```
int, float, bool, byte, char
INT, FLOAT, BOOL, BYTE, CHAR
MemoryStrategy::Copy, value type, bitwise copy
IntInstr, FloatInstr, UnsignedCmp, BoolInstr
int.f, int.byte, int.abs, int.to_str
float.floor, float.ceil, float.round, float.abs, float.to_str
bool.to_str
byte.to_int, byte.to_char, byte.to_str
char.to_int, char.to_str, char.is_alpha, char.is_digit
resolve_int_method, resolve_float_method, resolve_bool_method
resolve_byte_method, resolve_char_method
```

---

### Section 04: String Type Definition
**File:** `section-04-string-type.md` | **Status:** Not Started

```
str, STR, string, MemoryStrategy::Arc
RuntimeCall, ori_str_concat, ori_str_eq, ori_str_compare
str.length, str.concat, str.to_upper, str.to_lower, str.trim
str.contains, str.starts_with, str.ends_with
str.slice, str.replace, str.split, str.chars
str.to_str, str.repeat, str.bytes
resolve_str_method, string comparison, string ordering
operator overloading, add operator on strings
```

---

### Section 05: Compound Type Definitions
**File:** `section-05-compound-types.md` | **Status:** Not Started

```
Duration, Size, Ordering, Error, Channel
Duration.from_secs, Duration.from_millis, Duration.secs, Duration.millis
Size.bytes, Size.kb, Size.mb, Size.gb
Ordering.Less, Ordering.Equal, Ordering.Greater, Ordering.then_with
error.message, error.trace
Channel.send, Channel.recv, Channel.close
compound types, special types, unit types
register_builtin_types, builtin_types.rs
```

---

### Section 06: Collection & Wrapper Types
**File:** `section-06-collection-wrapper-types.md` | **Status:** Not Started

```
List, Map, Set, Range, Tuple, Option, Result
list.len, list.push, list.pop, list.get, list.iter
map.len, map.get, map.insert, map.keys, map.values
set.len, set.contains, set.insert
range.iter, range.to_list
tuple field access, ._0, ._1
Option.is_some, Option.is_none, Option.unwrap, Option.unwrap_or
Result.is_ok, Result.is_err, Result.unwrap, Result.unwrap_or
collection methods, container types, wrapper types
COLLECTION_TYPES, resolve_list_method, resolve_option_method
resolve_result_method, resolve_map_method, resolve_set_method
```

---

### Section 07: Iterator Type Definitions
**File:** `section-07-iterator-types.md` | **Status:** Not Started

```
Iterator, DoubleEndedIterator, DEI
iterator.map, iterator.filter, iterator.fold, iterator.collect
iterator.take, iterator.skip, iterator.chain, iterator.zip
iterator.enumerate, iterator.flatten, iterator.cycle
iterator.count, iterator.any, iterator.all, iterator.find
DoubleEndedIterator.next_back, DoubleEndedIterator.rev
DoubleEndedIterator.last, DoubleEndedIterator.rfind, DoubleEndedIterator.rfold
DEI_ONLY_METHODS, ITERATOR_METHOD_NAMES
resolve_iterator_method, CollectionMethod, CollectionMethodResolver
DeiPropagation, dei_only, double-ended iterator
higher-order methods, closure parameter
```

---

### Section 08: Query API & Lookup Functions
**File:** `section-08-query-api.md` | **Status:** Not Started

```
BUILTIN_TYPES, find_type, find_method
lookup, query, search, resolve
borrowing_methods, methods_for, method_names_for
type-qualified lookup, (TypeTag, method_name)
const fn, static dispatch, O(1), hash map, perfect hash
phf, compile-time hash, lazy_static, LazyLock
iterator helpers, type enumeration
future optimization, performance, cache-friendly
```

---

### Section 09: Wire Type Checker (ori_types)
**File:** `section-09-wire-type-checker.md` | **Status:** Not Started

```
ori_types, type checker, inference, type resolution
resolve_builtin_method, resolve_str_method, resolve_int_method
resolve_float_method, resolve_bool_method, resolve_byte_method
resolve_char_method, resolve_list_method, resolve_option_method
resolve_result_method, resolve_map_method, resolve_set_method
TYPECK_BUILTIN_METHODS, type_tag_to_idx, return type
unify_higher_order_constraints (stays in ori_types), calls.rs
DEI_ONLY_METHODS, well_known_generic_types
infer/expr/methods.rs, check/well_known/mod.rs
Tag, Idx, InferEngine, type pool
```

---

### Section 10: Wire Evaluator (ori_eval)
**File:** `section-10-wire-evaluator.md` | **Status:** Not Started

```
ori_eval, evaluator, interpreter, dispatch
EVAL_BUILTIN_METHODS, BuiltinMethodNames, method dispatch
ITERATOR_METHOD_NAMES, CollectionMethod, CollectionMethodResolver
BuiltinMethodResolver, method_dispatch
function_val.rs, interpreter/mod.rs, resolvers/
register_prelude, register_function_val
format variant sync, FormatType, Alignment, Sign
methods/helpers/mod.rs, methods/mod.rs
```

---

### Section 11: Wire ARC & Borrow Pass (ori_arc)
**File:** `section-11-wire-arc-borrow.md` | **Status:** Not Started

```
ori_arc, ARC, borrow, borrow inference, ownership
borrowing_builtins, FxHashSet, infer_borrows
borrowing_method_names, borrowing_builtin_names
dependency direction, dependency inversion
ori_ir → ori_arc, backwards dependency fix
MemoryStrategy, RC increment, RC decrement
receiver borrows, parameter ownership
borrow/mod.rs, rc_insert/mod.rs, RcContext
```

---

### Section 12: Wire LLVM Backend (ori_llvm)
**File:** `section-12-wire-llvm-backend.md` | **Status:** Not Started

```
ori_llvm, LLVM, codegen, emission, backend
emit_binary_op, is_str, is_float, type guards
OpStrategy dispatch, strategy lookup, RuntimeCall
BuiltinRegistration, receiver_borrowed, declare_builtins! macro
ARC_PIPELINE_METHODS, BuiltinTable
borrowing_builtin_names, delete function
simplify macro, remove borrow: syntax
arc_emitter/mod.rs, builtins/mod.rs, builtins/traits.rs
emit_str_cmp_predicate, CmpPredicate, ori_str_compare
```

---

### Section 13: Migrate ori_ir & Legacy Consolidation
**File:** `section-13-migrate-ori-ir.md` | **Status:** Not Started

```
ori_ir, BUILTIN_METHODS, MethodDef, consolidation
builtin_methods/mod.rs, BuiltinType, ReturnSpec, ParamSpec
receiver_borrows, ReturnTag, migration
DerivedTrait, format spec, FormatType, Alignment, Sign
migration, deprecation, re-export, compatibility
4-way sync, derived trait sync, format variant sync
borrowing_method_names, method_borrows_receiver
find_method, methods_for, has_method
```

---

### Section 14: Enforcement Tests, Testing Matrix & Exit Criteria
**File:** `section-14-enforcement-testing.md` | **Status:** Not Started

```
enforcement test, testing matrix, exit criteria
every_registry_method_has_handler, every_codegen_builtin_has_registry_entry
purity enforcement, no behavior test, no dependency test
consistency.rs, allowlist elimination, gap list removal
TYPECK_BUILTIN_METHODS removal, EVAL_BUILTIN_METHODS removal
ARC_PIPELINE_METHODS removal, TYPECK_METHODS_NOT_IN_IR removal
EVAL_METHODS_NOT_IN_TYPECK removal, TYPECK_METHODS_NOT_IN_EVAL removal
grep verification, legacy removal, dead code check
regression test, spec test, AOT test, full suite
compile-time enforcement, Rust exhaustiveness, struct field required
type × method × phase matrix, cross-phase coverage
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Core Data Model Design | `section-01-core-data-model.md` | Not Started |
| 02 | Crate Scaffolding & Purity Enforcement | `section-02-crate-scaffolding.md` | Not Started |
| 03 | Primitive Type Definitions | `section-03-primitive-types.md` | Not Started |
| 04 | String Type Definition | `section-04-string-type.md` | Not Started |
| 05 | Compound Type Definitions | `section-05-compound-types.md` | Not Started |
| 06 | Collection & Wrapper Types | `section-06-collection-wrapper-types.md` | Not Started |
| 07 | Iterator Type Definitions | `section-07-iterator-types.md` | Not Started |
| 08 | Query API & Lookup Functions | `section-08-query-api.md` | Not Started |
| 09 | Wire Type Checker (ori_types) | `section-09-wire-type-checker.md` | Not Started |
| 10 | Wire Evaluator (ori_eval) | `section-10-wire-evaluator.md` | Not Started |
| 11 | Wire ARC & Borrow Pass (ori_arc) | `section-11-wire-arc-borrow.md` | Not Started |
| 12 | Wire LLVM Backend (ori_llvm) | `section-12-wire-llvm-backend.md` | Not Started |
| 13 | Migrate ori_ir & Legacy Consolidation | `section-13-migrate-ori-ir.md` | Not Started |
| 14 | Enforcement Tests, Testing Matrix & Exit Criteria | `section-14-enforcement-testing.md` | Not Started |
