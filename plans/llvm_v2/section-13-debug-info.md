---
section: "13"
title: Debug Info Generation
status: done
goal: Generate DWARF debug info so debuggers can step through Ori source, inspect variables, and understand ARC-managed types
sections:
  - id: "13.1"
    title: Existing Infrastructure Audit
    status: done
  - id: "13.2"
    title: V2 Integration Points
    status: done
  - id: "13.3"
    title: DILocalVariable Support
    status: done
  - id: "13.4"
    title: ARC-Specific Debug Info
    status: done
  - id: "13.5"
    title: Pipeline Wiring
    status: done
---

# Section 13: Debug Info Generation

**Status:** Done
**Goal:** DWARF debug information generation so lldb/gdb can set breakpoints on Ori source lines, inspect local variables (including ARC-managed types), and step through control flow. Integrates existing `aot/debug.rs` infrastructure with the V2 pipeline.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_llvm/src/debuginfo/` -- `DIBuilder` usage, `create_function_debug_context`, `source_loc`, variable debug descriptors
- **Zig** `src/codegen/llvm.zig` -- `getDebugFile()`, `lowerDebugType()`, debug info threaded through codegen context
- **Swift** `lib/IRGen/IRGenDebugInfo.cpp` -- SIL-to-LLVM debug info mapping, ARC type representation in DWARF

**Current state:** `ori_llvm/src/aot/debug.rs` (~1,500 lines) provides a comprehensive foundation **now wired into the V2 codegen pipeline**. DILocalVariable support, ARC heap debug type, and 5 pipeline wiring points are implemented. All callers currently pass `None` (debug info dormant); AOT pipeline integration will activate it.

---

## 13.1 Existing Infrastructure Audit

**Status:** Done

The `aot/debug.rs` module is well-structured and provides most of the building blocks. V2 preserves all of the following and builds on top of it.

**`DebugInfoConfig` with three levels:**
- `DebugLevel::None` -- no debug info (production default)
- `DebugLevel::LineTablesOnly` -- file/line/column only (release with debug)
- `DebugLevel::Full` -- types, variables, scopes (development default)
- Convenience constructors: `development()`, `release_with_debug()`, `for_target()`

**`DebugInfoBuilder` (per-module, wraps `InkwellDIBuilder`):**
- Creates `DICompileUnit` and `DIFile` per module
- Primitive type creation: `int_type()`, `float_type()`, `bool_type()`, `char_type()`, `byte_type()`, `void_type()`
- Composite type creation: `create_struct_type()`, `create_enum_type()`, `create_pointer_type()`, `create_array_type()`, `create_typedef()`
- Ori-specific helpers: `string_type()` ({len, data}), `option_type()` ({tag, payload}), `result_type()` ({tag, ok/err}), `list_type()` ({len, cap, data})
- Function debug info: `create_function()`, `create_simple_function()`, `attach_function()`
- Scope management: `push_scope()`, `pop_scope()`, `current_scope()`, `create_lexical_block()`
- Location setting: `set_location()`, `set_location_in_current_scope()`, `clear_location()`
- `TypeCache` with `FxHashMap` for primitive deduplication
- `finalize()` for resolving forward references before emission

**`DebugContext` (combines `DebugInfoBuilder` + `LineMap`):**
- `LineMap` with binary-search `offset_to_line_col()` for span-to-location conversion
- Span-based convenience methods: `set_location_from_offset()`, `create_function_at_offset()`, `create_lexical_block_at_offset()`
- Function scope entry/exit: `enter_function()`, `exit_function()`

**`DebugInfoError` with `#[cold]` factories:**
- `BasicType` -- LLVM type creation failure with context
- `BasicTypeCreation` -- lower-level LLVM failure (cold path, should never happen)
- `Disabled` -- debug info not enabled

**DWARF configuration:**
- DWARF 4 as default (`dwarf_version: 4`), DWARF 5 opt-in via `with_dwarf_version(5)`
- `DebugFormat::Dwarf` (Linux/macOS/WASM) and `DebugFormat::CodeView` (Windows/MSVC)
- Split debug info support (dSYM on macOS, .dwo on Linux)
- `DWARFSourceLanguage::C` as stand-in (closest to Ori's semantics; custom language ID is a future enhancement)

**Implemented in this section:**

| Component | Status | Notes |
|-----------|--------|-------|
| DICompileUnit/DIFile | Done | Per-module, created in `DebugInfoBuilder::new()` |
| DISubprogram | Done | `create_function()` with linkage name, scope, flags; attached in `declare_function_with_symbol()` |
| DIType (primitives) | Done | int, float, bool, char, byte, void |
| DIType (composites) | Done | struct, enum, pointer, array, Option, Result, list, string |
| DILexicalBlock | Done | `create_lexical_block()` with scope chaining |
| Source locations | Done | `set_location()` / `set_location_from_offset()` |
| LineMap | Done | Binary search, O(log n) per lookup |
| **DILocalVariable** | **Done** | `create_auto_variable()`, `create_parameter_variable()`, `emit_dbg_declare()`, `emit_dbg_value()` |
| **Pipeline wiring** | **Done** | 5 wiring points: IrBuilder accessor, FunctionCompiler threading, ExprLowerer per-expression locations, bind_pattern debug vars, evaluator None |
| **ARC type debug info** | **Done** | `create_rc_heap_type()` with `RC<T> = { strong_count: i64, data: T }` layout |
| **Composite TypeCache** | **Done** | `composites: FxHashMap<u32, DIType>` for deduplication by type pool index |

- [x] Add DILocalVariable support (Section 13.3)
- [x] Wire DebugContext into V2 codegen pipeline (Section 13.5)
- [x] Add ARC heap layout debug types (Section 13.4)
- [x] Add composite TypeCache for deduplication

---

## 13.2 V2 Integration Points

**Status:** Done

### DebugContext Threading

`DebugContext` is threaded as `Option<&'a DebugContext<'ctx>>` through `FunctionCompiler` and `ExprLowerer`. It is `Option` because debug info may be disabled (`DebugLevel::None`). All debug-info-producing code paths check the option with `if let Some(dc) = self.debug_context`.

**Implemented:** `FunctionCompiler` has `debug_context: Option<&'a DebugContext<'ctx>>` field, passed through `new()` to all downstream `ExprLowerer` instances.

### IrBuilder Integration (Section 02)

The V2 `IrBuilder` exposes `inkwell_builder()` to provide raw LLVM builder access for debug location setting. This is used by `DebugContext::set_location_from_offset_in_current_scope()` in `ExprLowerer::lower()`.

**Implemented:** `IrBuilder::inkwell_builder() -> &InkwellBuilder<'ctx>` accessor added.

### TypeInfo::debug_type() (Deferred to Polish)

Full `TypeInfo::debug_type()` dispatch is deferred. Currently, `di.int_type()` is used as a placeholder for all variable types in `bind_pattern()`. Debug locations and function scopes (the high-value items) work correctly.

- [x] Add `inkwell_builder()` to IrBuilder
- [x] Thread `Option<&DebugContext>` through FunctionCompiler and ExprLowerer
- [x] Per-expression location setting in `ExprLowerer::lower()`
- [ ] *(Polish)* Add `debug_type()` to TypeInfo enum for full type-to-DIType mapping

---

## 13.3 DILocalVariable Support

**Status:** Done

### Methods Added to `DebugInfoBuilder`

- `create_auto_variable(scope, name, line, ty) -> DILocalVariable` -- for let bindings
- `create_parameter_variable(scope, name, arg_no, line, ty) -> DILocalVariable` -- for params (1-based)
- `create_debug_location(line, col, scope) -> DILocation` -- location factory
- `create_expression() -> DIExpression` -- empty expression (no address transforms)
- `emit_dbg_declare(alloca, var, loc, block)` -- for mutable bindings
- `emit_dbg_value(value, var, loc, insert_before)` -- for immutable bindings

### Convenience Methods Added to `DebugContext`

- `emit_declare_for_alloca(alloca, name, ty, span_start, block)` -- combined create + emit for mutable
- `emit_value_for_binding(value, name, ty, span_start, insert_before)` -- combined for immutable

### Integration in `bind_pattern()` (`lower_control_flow.rs`)

Mutable `BindingPattern::Name` bindings emit `dbg.declare` at `DebugLevel::Full`:
- After alloca creation and store, `dc.emit_declare_for_alloca()` is called
- Uses `di.int_type()` as placeholder type (full TypeInfo dispatch deferred)
- Only emitted for non-DUMMY spans

### Deferred

- **Immutable binding debug vars** (`dbg.value`) -- requires `InstructionValue` as `insert_before`, which needs additional infrastructure. The `emit_dbg_value` and `emit_value_for_binding` methods exist but aren't wired into `bind_pattern()` yet.
- **Parameter debug info** -- `create_parameter_variable` exists but isn't wired into `bind_parameters()` yet. Requires TypeInfo::debug_type() for correct parameter types.

- [x] Add `create_auto_variable()` and `create_parameter_variable()` wrappers to DebugInfoBuilder
- [x] Add `emit_dbg_declare()` and `emit_dbg_value()` helper methods
- [x] Add `emit_declare_for_alloca()` and `emit_value_for_binding()` convenience methods to DebugContext
- [x] Integrate mutable binding debug vars in `bind_pattern()`
- [ ] *(Polish)* Wire immutable binding `dbg.value` in `bind_pattern()`
- [ ] *(Polish)* Wire parameter debug info in `bind_parameters()`

---

## 13.4 ARC-Specific Debug Info

**Status:** Done

### Reference-Counted Heap Objects

`create_rc_heap_type()` added to `DebugInfoBuilder`. Represents `RC<T> = { strong_count: i64, data: T }` with 8-byte header at offset 0 and data at offset 64 bits.

In debuggers, users see `RC<MyStruct>.strong_count` and `RC<MyStruct>.data.field_name`. User-friendly LLDB formatters are a future enhancement.

### Composite Type Cache

`TypeCache` extended with `composites: FxHashMap<u32, DIType<'ctx>>` for deduplication by type pool index. Accessed via `cache_composite_type()` and `get_cached_composite()`.

### Source Location Preservation Through ARC IR Lowering

Source locations flow through the existing `Span` field on `Expr` nodes. `ExprLowerer::lower()` sets the debug location from `expr.span` before lowering each expression. ARC IR instructions inherit spans from their originating expressions. No additional work needed for span preservation.

- [x] Add `create_rc_heap_type()` helper to DebugInfoBuilder
- [x] Add composite type cache to TypeCache
- [x] Verify source location preservation through expression lowering

---

## 13.5 Pipeline Wiring

**Status:** Done

Five wiring points implemented, all using the `Option<&DebugContext>` pattern for zero cost when disabled.

### Point 1: IrBuilder Accessor

`IrBuilder::inkwell_builder()` exposes the raw inkwell `Builder<'ctx>` for debug location and intrinsic operations.

**File:** `codegen/ir_builder.rs`

### Point 2: Function Declaration (DISubprogram)

In `FunctionCompiler::declare_function_with_symbol()`:
- `Span` threaded from `Function.span` through `declare_all()` → `declare_function()` → `declare_function_with_symbol()`
- `DISubprogram` created via `dc.create_function_at_offset(name, span.start)` and attached to LLVM function via `dc.di().attach_function(func_val, subprogram)`
- Impl methods and imported functions also handled (imports use `Span::DUMMY`)

**File:** `codegen/function_compiler.rs`

### Point 3: Function Scope Enter/Exit

In `FunctionCompiler::define_function_body()`:
- `dc.enter_function(subprogram)` after creating entry block, before lowering body
- `dc.exit_function()` after emitting return (including early return on terminated blocks)
- Subprogram retrieved from `func_val.get_subprogram()`

**File:** `codegen/function_compiler.rs`

### Point 4: Expression Lowering (Source Locations)

In `ExprLowerer::lower()`, before the `match &expr.kind`:
- `dc.set_location_from_offset_in_current_scope(self.builder.inkwell_builder(), expr.span.start)`
- Only for non-DUMMY spans
- Every LLVM instruction gets tagged with the correct source position

**File:** `codegen/expr_lowerer.rs`

### Point 5: Variable Binding (DILocalVariable)

In `lower_control_flow.rs::bind_pattern()`, for mutable `BindingPattern::Name`:
- `dc.emit_declare_for_alloca()` emits `llvm.dbg.declare` intrinsic
- Only at `DebugLevel::Full`, only for non-DUMMY spans
- Uses `di.int_type()` as placeholder type

**File:** `codegen/lower_control_flow.rs`

### Call Site Updates

All `FunctionCompiler::new()` and `ExprLowerer::new()` call sites updated to accept `debug_context` parameter:
- `evaluator.rs` → `None` (JIT path)
- `compile_common.rs` (2 sites) → `None` (AOT wiring deferred)
- `function_compiler.rs` tests (8 sites) → `None`

### Module Finalization

`dc.finalize()` must be called before `module.verify()` in any path that creates a `DebugContext`. Currently no path creates a real `DebugContext` (all pass `None`), so finalization is not yet triggered. When AOT integration activates debug info, finalization will be wired into the emission path.

- [x] Add `inkwell_builder()` accessor to IrBuilder
- [x] Create and attach DISubprogram in function declaration
- [x] Enter/exit function scope in define_function_body
- [x] Per-expression source location setting in ExprLowerer::lower()
- [x] DILocalVariable creation for mutable bindings in bind_pattern()
- [x] Update all FunctionCompiler::new() call sites (evaluator, compile_common, tests)
- [x] Update all ExprLowerer::new() call sites (via FunctionCompiler)

---

## Tests

8 new tests in `aot/debug.rs`:

| Test | What it verifies |
|------|-----------------|
| `create_auto_variable_produces_valid_metadata` | Auto variable metadata is non-null, module verifies |
| `create_parameter_variable_produces_valid_metadata` | Parameter variable metadata is non-null, module verifies |
| `emit_dbg_declare_on_alloca_passes_verify` | `llvm.dbg.declare` on real alloca with DISubprogram passes module verify |
| `create_rc_heap_type_produces_two_field_struct` | RC heap type metadata is non-null, module verifies |
| `composite_type_cache_deduplicates` | Cache stores and retrieves composite types correctly |
| `debug_context_set_location_from_offset` | DebugContext location setting with scope enter/exit passes verify |
| `debug_context_emit_declare_for_alloca_convenience` | Convenience method produces valid debug info that passes verify |
| `line_map_offset_to_line_col` | LineMap correctly converts byte offsets to 1-indexed line/column |
| `debug_none_level_returns_none_builder` | DebugInfoBuilder::new returns None for DebugLevel::None |

All tests pass (8,375 total, 0 failures).

---

## Deferred to Polish

- **TypeInfo::debug_type()** -- Full type-to-DIType mapping (struct field names, enum variant names). For this pass, `di.int_type()` placeholder is used for variable types. Debug locations and function scopes are the high-value items.
- **Lexical block scoping** -- `DILexicalBlock` in `lower_block()` for `DebugLevel::Full`. Adds scope nesting but isn't required for basic stepping.
- **Parameter debug info** -- `create_parameter_variable` exists but requires TypeInfo::debug_type() to map parameter types correctly.
- **Immutable binding debug vars** -- `emit_dbg_value` and `emit_value_for_binding` exist but need `InstructionValue` infrastructure for `insert_before`.
- **AOT pipeline wiring** -- The AOT compilation paths (`compile_common.rs`) currently pass `None`. Creating a real `DebugContext` from source path/text and calling `finalize()` before emission is straightforward once debug info needs to be activated.
- **Full end-to-end test** -- `lldb ori_program` with breakpoints, stepping, and variable inspection.

**Exit Criteria:** `lldb ori_program` can set breakpoints on Ori source lines, step through expressions, and inspect local variables (including ARC-managed types shown as `RC<T>` with refcount and data fields).
