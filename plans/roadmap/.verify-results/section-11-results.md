# Section 11: Foreign Function Interface (FFI) -- Verification Results

**Verified**: 2026-03-19
**Section status**: 0/260 (0%) -- all items marked `[ ]`
**Verdict**: Status is INACCURATE. Several items marked `[ ]` are partially or fully implemented. The section should be ~15-20% complete based on parser, IR, formatter, spec, and unsafe block infrastructure that already exists.

---

## Methodology

Spot-checked 15 items across all 11 subsections to confirm whether genuinely not implemented. Searched compiler crates (ori_ir, ori_parse, ori_types, ori_eval, ori_llvm, ori_fmt) for FFI-related code. Ran existing tests with `timeout 150`.

---

## Subsection Summaries

### 11.1 Extern Block Syntax -- PARTIALLY IMPLEMENTED (should be ~60%)

The roadmap claims this is 0% but significant implementation exists:

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| `extern` keyword (Lexer) | `[ ]` | Implemented: `KwExtern = 48` in `ori_ir/src/token/tag.rs` | **STALE** |
| `parse_extern_block()` (Parser) | `[ ]` | Implemented: `ori_parse/src/grammar/item/extern_def.rs` (235 lines). Full parsing of convention, `from`, `as`, params, C variadics. | **STALE** |
| `ExternBlock` AST node | `[ ]` | Implemented: `ori_ir/src/ast/items/extern_def.rs` (81 lines). `ExternBlock`, `ExternItem`, `ExternParam` structs. | **STALE** |
| `ExternItem` variants | `[ ]` | Implemented: includes `is_c_variadic`, `alias`, params, return_ty | **STALE** |
| `from "lib"` library spec | `[ ]` | Implemented: parser handles `from` contextual keyword | **STALE** |
| `as "name"` name mapping | `[ ]` | Implemented: parser handles `as` with string literal | **STALE** |
| Spec: `spec/26-ffi.md` | `[ ]` | Implemented: `docs/ori_lang/v2026/spec/26-ffi.md` exists (250+ lines), covers extern blocks, C types, CPtr, #repr, unsafe, JsValue, JsPromise | **STALE** |
| Grammar: `grammar.ebnf` | (in completion checklist) | Implemented: `extern_block`, `extern_item`, `extern_params`, `extern_param`, `c_variadic` all defined | **STALE** |
| Parser tests | `[ ]` | Implemented: `compiler/oric/tests/phases/parse/extern_def.rs` (20 tests, all passing). Covers basic C/JS, empty block, from/as, variadics, visibility, multiple items, error cases. | **STALE** |
| Formatter | (not listed) | Implemented: `compiler/ori_fmt/src/declarations/extern_def.rs` (77 lines) | N/A |
| Type checker: Validate extern | `[ ]` | NOT implemented: `ori_types` has no extern block processing | CONFIRMED `[ ]` |
| Codegen: LLVM `declare` | `[ ]` | NOT implemented: `ori_llvm` has no extern block codegen | CONFIRMED `[ ]` |
| Spec test: `tests/spec/ffi/extern_blocks.ori` | `[ ]` | NOT implemented: no `tests/spec/ffi/` directory exists | CONFIRMED `[ ]` |

**Tests run**: `timeout 150 cargo test -p oric --test phases -- extern_def` -- 20 passed, 0 failed.

### 11.2 C ABI Types -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| `CPtr` in type system | `[ ]` | NOT implemented: no CPtr variant in ori_ir or ori_types | CONFIRMED `[ ]` |
| C type aliases (`c_int`, etc.) | `[ ]` | NOT implemented: no c_int/c_long/etc. in any compiler crate | CONFIRMED `[ ]` |
| Spec section | `[ ]` | Partially exists in `spec/26-ffi.md` section 26.4.2 | Spec exists but no impl |

### 11.3 #repr Attribute -- PARTIALLY IMPLEMENTED (should be ~40%)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| Parser: `#repr("c")` | `[ ]` | Implemented: `ori_parse/src/grammar/attr/mod.rs` has `ReprAttr` enum (C, Packed, Transparent, Aligned(u64)) and `parse_repr_attr()` | **STALE** |
| Parser: `#repr("packed")` | `[ ]` | Implemented: handled in `parse_repr_attr()` | **STALE** |
| Parser: `#repr("transparent")` | `[ ]` | Implemented: handled in `parse_repr_attr()` | **STALE** |
| Parser: `#repr("aligned", N)` | `[ ]` | Implemented: handled in `parse_repr_attr()` with power-of-two validation | **STALE** |
| IR: `ReprKind` enum | `[ ]` | Partially: `ReprAttr` exists in parser attrs (not in IR as `ReprKind`) | **STALE** |
| Type checker: Validate | `[ ]` | NOT implemented: no repr validation in ori_types | CONFIRMED `[ ]` |
| Codegen: LLVM layout | `[ ]` | NOT implemented: no repr-aware LLVM struct layout | CONFIRMED `[ ]` |

### 11.4 Unsafe Expressions -- PARTIALLY IMPLEMENTED (should be ~60%)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| `unsafe` keyword | `[ ]` | Implemented: `KwUnsafe = 35` in token/tag.rs | **STALE** |
| Parser: unsafe blocks | `[ ]` | Implemented: `parse_unsafe_expr()` in `ori_parse/src/grammar/expr/primary/specials.rs` | **STALE** |
| AST: `Unsafe(ExprId)` | `[ ]` | Implemented: in `ori_ir/src/ast/expr.rs` and `canon/expr.rs` | **STALE** |
| Type checker: `Unsafe` | `[ ]` | Implemented: `ExprKind::Unsafe(inner) => infer_expr(engine, arena, *inner)` in ori_types (transparent) | **STALE** |
| Evaluator: Execute | `[ ]` | Implemented: `CanExpr::Unsafe(inner) => self.eval_can(inner)` in ori_eval (transparent) | **STALE** |
| Spec test: unsafe_block.ori | `[ ]` | Implemented: `tests/spec/capabilities/unsafe_block.ori` (6 tests, all passing) | **STALE** |
| Grammar: `unsafe_expr` | (in checklist) | Implemented: `unsafe_expr = "unsafe" block_expr .` in grammar.ebnf | **STALE** |
| LLVM codegen for unsafe | `[ ]` | NOT implemented: no Unsafe handling in ori_llvm | CONFIRMED `[ ]` |
| `in_unsafe` flag (type checker) | `[ ]` | NOT implemented: no unsafe context tracking | CONFIRMED `[ ]` |
| Compile-fail: unsafe outside block | `[ ]` | NOT implemented: no compile-fail tests | CONFIRMED `[ ]` |

NOTE: Current unsafe implementation is transparent (no capability enforcement). The `uses Unsafe` capability is mentioned in the grammar/spec but not enforced -- unsafe blocks evaluate their body without any special checks. This is correct for Phase 1 but items about enforcing unsafe context are genuinely incomplete.

**Tests run**: `timeout 150 cargo st tests/spec/capabilities/unsafe_block.ori` -- all 6 tests pass (within full 4181-test suite).
**Tests run**: `timeout 150 cargo test -p ori_parse -- unsafe` -- 2 passed, 0 failed.

### 11.5 FFI Capability -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| `FFI` capability in type system | `[ ]` | NOT implemented: no FFI capability in ori_types | CONFIRMED `[ ]` |
| `uses FFI` enforcement | `[ ]` | NOT implemented | CONFIRMED `[ ]` |

### 11.6 Callbacks (Native) -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| Function pointer types | `[ ]` | NOT implemented for FFI context | CONFIRMED `[ ]` |
| Trampoline generation | `[ ]` | NOT implemented (existing "trampoline" code is for iterator/closure callbacks, not FFI) | CONFIRMED `[ ]` |

### 11.7 Build System Integration -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| ori.toml native section | `[ ]` | NOT implemented: no ori.toml parsing exists | CONFIRMED `[ ]` |
| pkg-config integration | `[ ]` | NOT implemented | CONFIRMED `[ ]` |

### 11.8 compile_error Built-in -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| `compile_error` parsing | `[ ]` | NOT implemented: existing `compile_error!()` in Rust code is Rust's macro, not Ori's builtin | CONFIRMED `[ ]` |
| Type checker trigger | `[ ]` | NOT implemented | CONFIRMED `[ ]` |

### 11.9 WASM Target -- PARTIALLY IMPLEMENTED (infrastructure exists, not FFI-specific)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| WASM codegen | `[ ]` | WASM compilation infrastructure exists (`ori_llvm/src/aot/wasm/`, linker, config, WASI) but no JS FFI glue for `extern "js"` blocks | CONFIRMED `[ ]` for FFI-specific items |

NOTE: The WASM compilation pipeline exists (target, linker, config, wasi support, JS binding generation infrastructure) but is not wired to extern "js" blocks. The WASM work belongs more to Section 14 (Targets) than Section 11.

### 11.10 JsValue and Async -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| `JsValue` type | `[ ]` | NOT implemented: no JsValue in compiler crates | CONFIRMED `[ ]` |
| `JsPromise<T>` type | `[ ]` | NOT implemented | CONFIRMED `[ ]` |

### 11.11 Deep FFI -- NOT IMPLEMENTED (0% correct)

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| Error protocols | `[ ]` | NOT implemented | CONFIRMED `[ ]` |
| `out` parameters | `[ ]` | NOT implemented | CONFIRMED `[ ]` |
| Ownership annotations | `[ ]` | NOT implemented | CONFIRMED `[ ]` |
| `[byte]` elision | `[ ]` | NOT implemented | CONFIRMED `[ ]` |
| Parametric FFI capability | `[ ]` | NOT implemented | CONFIRMED `[ ]` |

---

## Artifacts Found (not reflected in roadmap)

These implemented items exist but are all marked `[ ]` in the section:

1. **Spec file**: `docs/ori_lang/v2026/spec/26-ffi.md` -- comprehensive (250+ lines covering extern blocks, C types, CPtr, #repr, unsafe, JsValue, JsPromise)
2. **Proposals**: `proposals/approved/platform-ffi-proposal.md`, `proposals/approved/deep-ffi-proposal.md`, `proposals/approved/repr-extensions-proposal.md`, `proposals/approved/unsafe-semantics-proposal.md` -- all approved
3. **Grammar**: `grammar.ebnf` has extern_block and unsafe_expr productions
4. **Parser**: Full extern block parser with 20 passing tests
5. **IR**: ExternBlock, ExternItem, ExternParam AST nodes
6. **Formatter**: Extern block formatting
7. **Unsafe**: Full pipeline from parser through evaluator, 6 passing spec tests
8. **#repr parser**: All 4 variants (C, Packed, Transparent, Aligned) parsed

---

## Summary

| Subsection | Roadmap Status | Actual Status | Items Stale |
|------------|---------------|---------------|-------------|
| 11.1 Extern Block Syntax | 0% | ~60% | 10 items should be `[x]` |
| 11.2 C ABI Types | 0% | 0% (spec exists) | 0 |
| 11.3 #repr Attribute | 0% | ~40% | 5 items should be `[x]` |
| 11.4 Unsafe Expressions | 0% | ~60% | 7 items should be `[x]` |
| 11.5 FFI Capability | 0% | 0% | 0 |
| 11.6 Callbacks | 0% | 0% | 0 |
| 11.7 Build System | 0% | 0% | 0 |
| 11.8 compile_error | 0% | 0% | 0 |
| 11.9 WASM Target | 0% | 0% (infra exists elsewhere) | 0 |
| 11.10 JsValue/Async | 0% | 0% | 0 |
| 11.11 Deep FFI | 0% | 0% | 0 |

**Total stale items**: ~22 items should be marked `[x]` that are currently `[ ]`.
**Estimated true completion**: ~15-20% (vs reported 0%).
**True remaining work**: Type checker integration for extern blocks, LLVM codegen for extern calls, CPtr type, C type aliases, FFI capability enforcement, unsafe context tracking, all WASM/JS FFI, all Deep FFI, build system integration, compile_error builtin.
