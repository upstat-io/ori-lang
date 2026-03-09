---
reroute: true
name: "Type Registry"
full_name: "Type Strategy Registry"
status: active
reviewed: false
order: 1
---

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
**File:** `section-01-core-data-model.md` | **Status:** Complete

```
TypeTag, MemoryStrategy, Ownership, OpStrategy
ParamDef, MethodDef, OpDefs, TypeDef
ReturnTag, TypeProjection, TypeParamArity, MethodKind, DeiPropagation
pure data, no behavior, no dependencies, const-constructible
enum, struct, static, compile-time, zero-cost
receiver, borrow, owned, copy, arc
IntInstr, FloatInstr, UnsignedCmp, BoolLogic, RuntimeCall, Unsupported
schema, contract, specification, declaration
SelfType, Iterator, Unit (not Void), return type, NextResult, Fresh
extensibility, future fields, IterationDef, HashStrategy, DisplayStrategy
size assertion, documentation, operator coverage
pow, matmul, as, as?, not, bit_not, neg
```

---

### Section 02: Crate Scaffolding & Purity Enforcement
**File:** `section-02-crate-scaffolding.md` | **Status:** Complete

```
ori_registry, Cargo.toml, workspace, crate DAG
module structure, lib.rs, tags.rs, method.rs, operator.rs, type_def.rs, query.rs
defs/, defs/mod.rs, defs/int.rs, defs/str.rs
directory module, sibling tests.rs, const fn helper
purity, no behavior, no logic, no trait impls with logic
no dependencies, zero deps, foundation crate, bottom of DAG
const, static, compile-time construction
enforcement test, purity test, no functions with side effects
workspace members, cargo check, cargo test
ori_llvm excluded from workspace, path dependency
```

---

### Section 03: Primitive Type Definitions
**File:** `section-03-primitive-types.md` | **Status:** Complete

```
int, float, bool, byte, char
INT, FLOAT, BOOL, BYTE, CHAR
MemoryStrategy::Copy, value type, bitwise copy
IntInstr, FloatInstr, UnsignedCmp, BoolLogic
Ownership::Borrow, receiver ownership, Copy vs Borrow
ReturnTag::SelfType, operator trait methods
ParamDef, Param::SelfType, abbreviated MethodDef::new
MethodDef::primitive, const fn helper, 500-line limit
int.f, int.byte, int.abs, int.to_str, int.into, int.pow
float.floor, float.ceil, float.round, float.abs, float.to_str
float return type discrepancy, ori_ir SelfType bug
bool.to_str, bool.not, BoolLogic
byte.to_int, byte.to_char, byte.to_str, byte bitwise operators
char.to_int, char.to_str, char.is_alpha, char.is_digit, char.to_byte
resolve_int_method, resolve_float_method, resolve_bool_method
resolve_byte_method, resolve_char_method
cross-reference table, method count summary, signed vs unsigned
trait methods as MethodDefs, operator methods as MethodDefs
Default, Formattable, Value, Sendable (not in registry)
```

---

### Section 04: String Type Definition
**File:** `section-04-string-type.md` | **Status:** Complete

```
str, STR, string, MemoryStrategy::Arc, SSO, Small String Optimization
RuntimeCall, ori_str_concat, ori_str_eq, ori_str_compare
str.length, str.concat, str.to_upper, str.to_lower, str.trim
str.contains, str.starts_with, str.ends_with
str.slice, str.replace, str.split, str.chars
str.to_str, str.repeat, str.bytes
str.as_bytes, str.to_bytes, str.from_utf8, str.from_utf8_unchecked
str.into, Into trait, str -> Error
alias, length/len, substring/slice, parse_int/to_int, parse_float/to_float
associated function, MethodKind::Associated
Formattable, blanket impl, Iterable, DoubleEndedIterator
resolve_str_method, string comparison, string ordering
operator overloading, add operator on strings
```

---

### Section 05: Compound Type Definitions
**File:** `section-05-compound-types.md` | **Status:** Complete

```
Duration, Size, Ordering, Error
Duration.from_seconds, Duration.from_millis, Duration.nanoseconds, Duration.as_seconds
Size.bytes, Size.kilobytes, Size.from_kb, Size.to_kb
Ordering.Less, Ordering.Equal, Ordering.Greater, Ordering.then_with
Error.message, Error.trace, Error.trace_entries, Error.with_trace
compound types, measurement types, Duration/Size pattern
register_builtin_types, builtin_types.rs
const fn helper, MethodDef::compound, MethodDef::associated
directory module, sibling tests.rs, frozen fields
associated function, MethodKind::Associated
heterogeneous operators, operator alias, canonical name
format spec types, FormatType, Alignment, Sign, FormatSpec
MemoryStrategy::Copy, MemoryStrategy::Arc
```

---

### Section 06: Collection & Wrapper Types
**File:** `section-06-collection-wrapper-types.md` | **Status:** Complete

```
List, Map, Set, Range, Tuple, Option, Result, Channel
Channel.send, Channel.recv, Channel.close, Channel.try_recv
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
**File:** `section-07-iterator-types.md` | **Status:** Complete

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
**File:** `section-08-query-api.md` | **Status:** Complete

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
**File:** `section-09-wire-type-checker.md` | **Status:** Complete

```
ori_types, type checker, inference, type resolution
resolve_builtin_method, resolve_str_method, resolve_int_method
resolve_float_method, resolve_bool_method, resolve_byte_method
resolve_char_method, resolve_list_method, resolve_option_method
resolve_result_method, resolve_map_method, resolve_set_method
TYPECK_BUILTIN_METHODS, return_tag_to_idx, return type
unify_higher_order_constraints (stays in ori_types), calls/method_call.rs
DEI_ONLY_METHODS, well_known_generic_types
infer/expr/methods/mod.rs, check/well_known/mod.rs
Tag, Idx, InferEngine, type pool
resolve_receiver_and_builtin, check_range_float_iteration
check_infinite_iterator_consumed, associated function, MethodKind::Associated
tag_to_type_tag, type_tag_to_idx, resolve_projection, registry_bridge.rs
computed_returns.rs, resolve_computed_return, computed_list_return
as_bytes, to_bytes, from_utf8, from_utf8_unchecked, str methods
```

---

### Section 10: Wire Evaluator (ori_eval)
**File:** `section-10-wire-evaluator.md` | **Status:** Complete

```
ori_eval, evaluator, interpreter, dispatch
EVAL_BUILTIN_METHODS, BuiltinMethodNames, method dispatch
ITERATOR_METHOD_NAMES, CollectionMethod, CollectionMethodResolver
BuiltinMethodResolver, method_dispatch, FxHashSet
eval_type_name, name mapping, PascalCase, type name convention
dispatch_coverage.rs, METHODS_NOT_YET_IN_EVAL, COLLECTION_RESOLVER_METHODS
consistency.rs, iterator_methods_match_registry, dispatch_coverage
every_registry_method_has_eval_dispatch_handler
builtin_method_names_match_registry, exhaustive destructure
format variant sync, FormatType, Alignment, Sign
methods/helpers/mod.rs, methods/mod.rs, methods/tests.rs
resolvers/mod.rs, all_iterator_variants, from_name
```

---

### Section 11: Wire ARC & Borrow Pass (ori_arc)
**File:** `section-11-wire-arc-borrow.md` | **Status:** Complete

```
ori_arc, ARC, borrow, borrow inference, ownership
BuiltinOwnershipSets, BORROWING_METHOD_NAMES, infer_borrows_scc
borrowing_builtin_names, borrowing_method_names, LazyLock
ori_arc/borrow/builtins/mod.rs, BuiltinOwnershipSets::new, BuiltinOwnershipSets::empty
MemoryStrategy, RC increment, RC decrement, ArcClassification
receiver borrows, parameter ownership, consuming receiver, COW exclusion
iterator exclusion, derived-value exclusion, TypeTag::Iterator, ".iter()"
protocol builtins, ProtocolBuiltin, __index, ProtocolArgOwnership
CONSUMING_RECEIVER_METHOD_NAMES, SHARING_METHOD_NAMES, COW arrays
purity test, purity_no_heap_allocation_types, fixed-size array
borrow/mod.rs, borrow/builtins/mod.rs, rc_insert/annotate.rs
arc_queries/mod.rs, function_compiler/mod.rs, define_phase.rs
ori_ir/builtin_methods/mod.rs, method_borrows_receiver, borrowing_names_from_table
```

---

### Section 12: Wire LLVM Backend (ori_llvm)
**File:** `section-12-wire-llvm-backend.md` | **Status:** Not Started

```
ori_llvm, LLVM, codegen, emission, backend
emit_binary_op, emit_unary_op, is_str, is_float, type guards
OpStrategy dispatch, strategy lookup, RuntimeCall, IntInstr, FloatInstr, UnsignedCmp, BoolLogic
idx_to_type_tag, TypeTag bridge, TypeInfo, Idx mapping
BuiltinRegistration, receiver_borrowed, declare_builtins! macro, simplify macro
arc_pipeline_methods, BuiltinTable, CODEGEN_ALIASES, TRAIT_DISPATCH_METHODS
borrowing_names_from_table, delete function, test-only
emit_int_binary_op, emit_float_binary_op, emit_unsigned_binary_op, emit_bool_binary_op
emit_runtime_binary_op, emit_coalesce, op_strategy_for_binary, op_strategy_for_unary
arc_emitter/operators.rs, builtins/mod.rs, builtins/traits.rs, builtins/tests.rs
emit_str_cmp_predicate, CmpPredicate, ori_str_compare
registry_covers_all_builtin_codegen, registry_op_strategies_cover_all_operators
unsigned comparison, signed comparison, byte, char, bool, correctness fix
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
code journey, pipeline integration, differential testing, eval vs LLVM
progressive complexity, phase boundary, end-to-end verification
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Core Data Model Design | `section-01-core-data-model.md` | Complete |
| 02 | Crate Scaffolding & Purity Enforcement | `section-02-crate-scaffolding.md` | Complete |
| 03 | Primitive Type Definitions | `section-03-primitive-types.md` | Complete |
| 04 | String Type Definition | `section-04-string-type.md` | Complete |
| 05 | Compound Type Definitions | `section-05-compound-types.md` | Complete |
| 06 | Collection & Wrapper Types | `section-06-collection-wrapper-types.md` | Complete |
| 07 | Iterator Type Definitions | `section-07-iterator-types.md` | Complete |
| 08 | Query API & Lookup Functions | `section-08-query-api.md` | Complete |
| 09 | Wire Type Checker (ori_types) | `section-09-wire-type-checker.md` | Complete |
| 10 | Wire Evaluator (ori_eval) | `section-10-wire-evaluator.md` | Complete |
| 11 | Wire ARC & Borrow Pass (ori_arc) | `section-11-wire-arc-borrow.md` | Complete |
| 12 | Wire LLVM Backend (ori_llvm) | `section-12-wire-llvm-backend.md` | Not Started |
| 13 | Migrate ori_ir & Legacy Consolidation | `section-13-migrate-ori-ir.md` | Not Started |
| 14 | Enforcement Tests, Testing Matrix & Exit Criteria | `section-14-enforcement-testing.md` | Not Started |
