# Section 12: Variadic Functions — Verification Results

**Verified**: 2026-03-28
**Status in roadmap**: not-started
**Actual status**: PARTIAL — parser infrastructure for variadic parameters and spread syntax exists across IR, parser, and type checker. No eval or codegen support.

## Summary

The variadic section is marked `not-started` but has hidden implementation at the parse/IR/typeck level:
- **Variadic parameter parsing**: COMPLETE — `...T` syntax in function signatures
- **Spread syntax in calls**: COMPLETE — `...expr` in function call arguments
- **Spread in collections**: COMPLETE — `[...a, ...b]`, `{...a, ...b}`, `P { ...orig }` all parse, type-check, and some eval
- **C variadic in extern blocks**: COMPLETE at parse level (Section 11 overlap)
- **Type checker for variadic params**: NOT implemented (no conversion of `...T` to `[T]`)
- **Evaluator for variadic calls**: NOT implemented (no arg collection or spread expansion in calls)

---

## 12.1 Homogeneous Variadics

### Lexer
- [done] `...` token (`DotDotDot`) — `compiler/ori_lexer_core/src/raw_scanner/operators.rs:209`, `compiler/ori_ir/src/token/kind.rs:121`
- [done] Distinguished from range `..` (`DotDot`) — raw scanner handles both

### Parser — Function Signatures
- [done] Variadic parameter parsing `...T` — `compiler/ori_parse/src/grammar/item/function/mod.rs:489-494`
  - Checks for `DotDotDot` after colon, sets `is_variadic = true`, parses type
- [done] `is_variadic` flag on `Param` — `compiler/ori_ir/src/ast/items/function.rs:97`

### Parser — Spread in Call Expressions
- [done] Spread `...expr` in call arguments — `compiler/ori_parse/src/grammar/expr/postfix.rs:392-423`
- [done] `is_spread` flag on `CallArg` — `compiler/ori_ir/src/ast/collections.rs:143`

### Parser — Spread in Collections (already working)
- [done] List spread `[...a, ...b]` — parsed as `ListWithSpread` / `ListElement::Spread`
- [done] Map spread `{...a, ...b}` — parsed as `MapWithSpread` / `MapElement::Spread`
- [done] Struct spread `P { ...orig, x: 10 }` — parsed as `StructWithSpread` / `StructLitField::Spread`

### Incremental
- [done] `is_variadic` copied in incremental copier — `compiler/ori_parse/src/incremental/copier.rs:861`

### Type Checker — Collections
- [done] `ListWithSpread` type inference — `compiler/ori_types/src/infer/expr/collections.rs:78`
- [done] `MapWithSpread` type inference — `compiler/ori_types/src/infer/expr/collections.rs:188`
- [done] `StructWithSpread` type inference — `compiler/ori_types/src/infer/expr/structs/mod.rs:169`

### Type Checker — Variadic Params
- [todo] No conversion of `...T` to `[T]` in type checker — `is_variadic` field not read by `ori_types`
  - Tests in `ori_types` always set `is_variadic: false`
- [todo] No spread type compatibility checking in call args
- [todo] No element type inference for variadic

### Evaluator
- [todo] No variadic arg collection (collect positional args into list)
- [todo] No spread expansion in function calls
  - NOTE: `ori_eval` does NOT handle `is_spread` on `CallArg` at all
  - Collection spread (list/map/struct) may be handled via canonical desugaring

### Codegen
- [todo] No LLVM codegen for variadic arg collection
- [todo] No codegen for spread in function calls

### Tests
- [todo] No spec tests (`tests/spec/functions/variadic.ori` does not exist)
- [todo] No Rust unit tests for variadic type checking

---

## 12.2 Minimum Argument Count

- [todo] No minimum argument validation for variadic functions
- [todo] No diagnostics for insufficient variadic args
- [todo] No tests

---

## 12.3 Trait Bounds on Variadics

- [todo] No trait bound validation on variadic parameters
- [todo] No trait object boxing for variadic trait objects
- [todo] No tests

---

## 12.4 C Variadic Interop

### Parser
- [done] C variadic `...` in extern blocks — `compiler/ori_parse/src/grammar/item/extern_def.rs:193-196`
- [done] `is_c_variadic` flag on `ExternItem` — `compiler/ori_ir/src/ast/items/extern_def.rs:47`
- [done] Parser tests for C variadic — `compiler/oric/tests/phases/parse/extern_def.rs:97-112`

### Type Checker
- [todo] No validation that C variadic callers must use unsafe
- [todo] No special handling for C variadic calls

### Codegen
- [todo] No va_list ABI generation
- [todo] No platform-specific variadic calling convention

### Tests
- [done] Parser tests for C variadic in extern blocks
- [todo] No spec tests, no codegen tests

---

## 12.5 Variadic in Patterns

- [todo] Deferred per roadmap — no implementation expected

---

## Correction Needed

The roadmap status should be changed from `not-started` to `partial`. Key completed items:
1. `...` (DotDotDot) token in lexer
2. Variadic parameter parsing (`...T`) in function signatures with `is_variadic` flag
3. Spread syntax parsing (`...expr`) in call arguments with `is_spread` flag
4. Spread in collections (list, map, struct) — full parse + typeck
5. C variadic parsing in extern blocks with parser tests

Estimated: ~30% of items have hidden implementation (mostly parse/IR layer).

The major gaps are:
- Type checker does not handle `is_variadic` for function parameters at all
- Evaluator has no variadic call support
- LLVM codegen has no variadic support
- No end-to-end tests
