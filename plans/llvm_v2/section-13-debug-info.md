---
section: "13"
title: Debug Info Generation
status: not-started
goal: Generate DWARF debug info so debuggers can step through Ori source, inspect variables, and understand ARC-managed types
sections:
  - id: "13.1"
    title: Existing Infrastructure Audit
    status: not-started
  - id: "13.2"
    title: V2 Integration Points
    status: not-started
  - id: "13.3"
    title: DILocalVariable Support
    status: not-started
  - id: "13.4"
    title: ARC-Specific Debug Info
    status: not-started
  - id: "13.5"
    title: Pipeline Wiring
    status: not-started
---

# Section 13: Debug Info Generation

**Status:** Not Started
**Goal:** DWARF debug information generation so lldb/gdb can set breakpoints on Ori source lines, inspect local variables (including ARC-managed types), and step through control flow. Integrates existing `aot/debug.rs` infrastructure with the V2 pipeline.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_llvm/src/debuginfo/` -- `DIBuilder` usage, `create_function_debug_context`, `source_loc`, variable debug descriptors
- **Zig** `src/codegen/llvm.zig` -- `getDebugFile()`, `lowerDebugType()`, debug info threaded through codegen context
- **Swift** `lib/IRGen/IRGenDebugInfo.cpp` -- SIL-to-LLVM debug info mapping, ARC type representation in DWARF

**Current state:** `ori_llvm/src/aot/debug.rs` (1,265 lines) provides a comprehensive foundation. V2 must wire this into the codegen pipeline and add DILocalVariable support.

---

## 13.1 Existing Infrastructure Audit

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

**What is already done vs what is missing:**

| Component | Status | Notes |
|-----------|--------|-------|
| DICompileUnit/DIFile | Done | Per-module, created in `DebugInfoBuilder::new()` |
| DISubprogram | Done | `create_function()` with linkage name, scope, flags |
| DIType (primitives) | Done | int, float, bool, char, byte, void |
| DIType (composites) | Done | struct, enum, pointer, array, Option, Result, list, string |
| DILexicalBlock | Done | `create_lexical_block()` with scope chaining |
| Source locations | Done | `set_location()` / `set_location_from_offset()` |
| LineMap | Done | Binary search, O(log n) per lookup |
| **DILocalVariable** | **Missing** | No `llvm.dbg.declare` or `llvm.dbg.value` calls |
| **Pipeline wiring** | **Missing** | Builder exists but nothing calls it during codegen |
| **ARC type debug info** | **Missing** | No RC heap layout representation |
| **TypeInfo integration** | **Missing** | No `TypeInfo::debug_type()` dispatch |

- [ ] Add DILocalVariable support (Section 13.3)
- [ ] Wire DebugContext into V2 codegen pipeline (Section 13.5)
- [ ] Add ARC heap layout debug types (Section 13.4)
- [ ] Integrate with TypeInfo enum for per-variant debug type creation (Section 13.2)

---

## 13.2 V2 Integration Points

### DebugContext Threading

`DebugContext` lives as an `Option<DebugContext<'ctx>>` on `CodegenCx` (or the V2 `ModuleEmitter`). It is `Option` because debug info may be disabled (`DebugLevel::None`). All debug-info-producing code paths check the option:

```rust
// Pseudocode: CodegenCx field
pub struct CodegenCx<'ll, 'tcx> {
    // ... existing fields ...
    pub debug_context: Option<DebugContext<'ll>>,
}
```

Functions that emit debug info take `&Option<DebugContext>` and no-op when `None`. This avoids conditional compilation and keeps the hot path overhead to a single branch.

### IrBuilder Integration (Section 02)

The V2 `IrBuilder` wraps raw LLVM builder calls. Debug location setting integrates at the IrBuilder level so that every instruction automatically gets a source location:

- `IrBuilder::set_source_location(span: Span)` -- delegates to `DebugContext::set_location_from_offset()`
- Called at the start of each expression lowering in Section 03's `ExprLowerer::lower()`
- `IrBuilder::clear_source_location()` -- for synthetic instructions (e.g., entry block allocas)

### TypeInfo::debug_type() (Section 01)

Each `TypeInfo` variant provides a `debug_type()` method that creates the corresponding `DIType` via the `DebugInfoBuilder`:

```rust
// Pseudocode: dispatched per TypeInfo variant
impl TypeInfo {
    pub fn debug_type<'ctx>(
        &self,
        di: &DebugInfoBuilder<'ctx>,
    ) -> Result<DIType<'ctx>, DebugInfoError> {
        match self {
            TypeInfo::Int => di.int_type().map(|t| t.as_type()),
            TypeInfo::Float => di.float_type().map(|t| t.as_type()),
            TypeInfo::Bool => di.bool_type().map(|t| t.as_type()),
            TypeInfo::Str => di.string_type().map(|t| t.as_type()),
            TypeInfo::Struct { name, fields, .. } => { /* create_struct_type */ }
            TypeInfo::Enum { name, variants, .. } => { /* create_enum_type */ }
            TypeInfo::List { element, .. } => { /* di.list_type(element.debug_type(di)?) */ }
            TypeInfo::Option { payload, .. } => { /* di.option_type(...) */ }
            TypeInfo::Result { ok, err, .. } => { /* di.result_type(...) */ }
            TypeInfo::Function { .. } => { /* fat pointer {fn_ptr, env_ptr} */ }
            TypeInfo::Channel { .. } => { /* pointer to runtime channel */ }
            // Unit/Never: di.void_type() or i64 depending on representation
        }
    }
}
```

Results are cached per `TypeInfo` variant in the `TypeCache` to avoid redundant LLVM DIType creation.

- [ ] Add `debug_type()` to TypeInfo enum
- [ ] Extend TypeCache to handle composite type deduplication
- [ ] Wire TypeInfo::debug_type() into function declaration and variable binding

---

## 13.3 DILocalVariable Support (Main Gap)

This is the primary missing piece. Without DILocalVariable, debuggers cannot inspect local variables -- they can only set breakpoints and step.

### Alloca-Based Mutable Bindings: `llvm.dbg.declare`

Mutable bindings in Ori (via `let mut`) use alloca-based storage (Section 02 `ScopeBinding::Mutable`). These map to `llvm.dbg.declare` which tells the debugger "this variable lives at this alloca address for its entire scope":

```rust
// Pseudocode: emitted when processing Let with mutable binding
fn emit_dbg_declare(
    di: &DebugInfoBuilder<'ctx>,
    builder: &LLVMBuilder<'ctx>,
    alloca: PointerValue<'ctx>,
    var_name: &str,
    var_type: DIType<'ctx>,
    line: u32,
    column: u32,
    scope: DIScope<'ctx>,
) {
    let di_var = di.inner.create_auto_variable(
        scope, var_name, di.file(), line, var_type, false, DIFlags::ZERO, 0,
    );
    let debug_loc = context.create_debug_location(line, column, scope, None);
    di.inner.insert_declare_at_end(alloca, Some(di_var), None, debug_loc, block);
}
```

### SSA Immutable Values: `llvm.dbg.value`

Immutable bindings (`let x = ...`) use SSA values directly (Section 02 `ScopeBinding::Immutable`). These map to `llvm.dbg.value` which associates a debug variable with a specific SSA value at a specific program point:

```rust
// Pseudocode: emitted when processing Let with immutable binding
fn emit_dbg_value(
    di: &DebugInfoBuilder<'ctx>,
    builder: &LLVMBuilder<'ctx>,
    value: BasicValueEnum<'ctx>,
    var_name: &str,
    var_type: DIType<'ctx>,
    line: u32,
    column: u32,
    scope: DIScope<'ctx>,
) {
    let di_var = di.inner.create_auto_variable(
        scope, var_name, di.file(), line, var_type, false, DIFlags::ZERO, 0,
    );
    let debug_loc = context.create_debug_location(line, column, scope, None);
    // `instr` is the next InstructionValue after the value definition, or the
    // block terminator if the value is defined last. Unlike insert_declare_at_end,
    // the var_info parameter here is NOT Optional — pass di_var directly.
    di.inner.insert_dbg_value_before(value, di_var, None, debug_loc, instr);
}
```

### ScopeBinding to DILocalVariable Mapping

Section 02's `ScopeBinding` directly drives which debug intrinsic to use:

| ScopeBinding | Debug Intrinsic | When |
|-------------|----------------|------|
| `Immutable(value)` | `llvm.dbg.value` | At binding site |
| `Mutable(alloca)` | `llvm.dbg.declare` | At alloca creation |

### Function Parameters

Function parameters are always immutable in Ori. They map to `DILocalVariable` with `create_parameter_variable()` (not `create_auto_variable()`), using 1-based argument indices:

```rust
// Pseudocode: in function declaration (Section 04)
for (i, param) in params.iter().enumerate() {
    let di_param = di.inner.create_parameter_variable(
        subprogram.as_debug_info_scope(),
        param.name,
        (i + 1) as u32,  // 1-based argument index
        di.file(),
        param.line,
        param.debug_type,
        false,            // always_preserve
        DIFlags::ZERO,
    );
    // Parameters that are passed by value get dbg.declare on their alloca copy
    // Parameters that are borrowed get dbg.value on the SSA value
}
```

### ARC IR's ArcVarId and Synthetic Values

When ARC IR (Section 06) introduces synthetic variables (e.g., temporary RC increments, reset/reuse intermediates), these do not correspond to source-level bindings. They inherit the source location of the expression that produced them but do NOT get DILocalVariable entries. Only user-visible bindings (those with a `Name` from the source) produce debug variables.

- [ ] Add `create_auto_variable()` and `create_parameter_variable()` wrappers to DebugInfoBuilder
- [ ] Add `emit_dbg_declare()` and `emit_dbg_value()` helper methods
- [ ] Integrate with ScopeBinding creation in Section 02's IrBuilder
- [ ] Add parameter debug info in function declaration (Section 04)

---

## 13.4 ARC-Specific Debug Info

### Reference-Counted Heap Objects

ARC-managed heap objects (Section 07) have a 16-byte header `{ strong_count: i64, weak_count: i64 }` where the pointer points to `data` and the header lives at `ptr - 16`. For the alpha release, debug info represents the **raw layout**:

```rust
// Pseudocode: debug type for an RC-managed heap object
fn create_rc_heap_type<'ctx>(
    di: &DebugInfoBuilder<'ctx>,
    inner_type: DIType<'ctx>,
    inner_name: &str,
    inner_size_bits: u64,
) -> DICompositeType<'ctx> {
    let int_ty = di.int_type().unwrap().as_type();
    let fields = [
        FieldInfo { name: "strong_count", ty: int_ty, size_bits: 64, offset_bits: 0, line: 0 },
        FieldInfo { name: "weak_count", ty: int_ty, size_bits: 64, offset_bits: 64, line: 0 },
        FieldInfo { name: "data", ty: inner_type, size_bits: inner_size_bits, offset_bits: 128, line: 0 },
    ];
    di.create_struct_type(
        &format!("RC<{inner_name}>"),
        0,
        128 + inner_size_bits,
        64,
        &fields,
    )
}
```

This means in lldb, users will see `RC<MyStruct>.strong_count`, `RC<MyStruct>.weak_count`, and `RC<MyStruct>.data.field_name`. This is raw but accurate. User-friendly LLDB formatters (type summaries and synthetic children that hide the header and show `data` fields directly) are a future enhancement tracked separately.

### Source Location Preservation Through ARC IR Lowering

When typed AST is lowered to ARC IR (Section 06) and then ARC IR is lowered to LLVM IR, source locations must be preserved:

1. **AST expressions** carry `Span` from parsing
2. **ARC IR instructions** (`ArcInstr`) carry the span of the expression they originated from
3. **Synthetic ARC instructions** (RcInc, RcDec, Reset, Reuse, IsShared) inherit the span of the expression that triggered their insertion -- typically the last use site or the constructor expression
4. **LLVM instructions** get their debug location from the ARC IR instruction's span via `DebugContext::set_location_from_offset()`

This means stepping in a debugger may show the same source line multiple times (once for the value computation, once for the RC decrement). This is correct behavior and matches how Rust's debugger shows drop glue.

### Reset/Reuse and Debugger Address Watching

When constructor reuse (Section 09) performs in-place mutation via Reset/Reuse, the memory address of the object does not change. This is important for debugger watchpoints: a watch on `&my_struct` will correctly trigger when fields are mutated in the fast path. The slow path (fresh allocation) will show a different address. No special debug info is needed for this -- it follows naturally from the LLVM IR structure.

### DWARFSourceLanguage

The existing code uses `DWARFSourceLanguage::C` as a stand-in. This is acceptable for the alpha because:
- DWARF recognizes C layout conventions, which closely match Ori's
- Debuggers use the language tag primarily for expression evaluation and name demangling
- A custom `DW_LANG_lo_user`-range language ID requires debugger plugin support, which is a post-alpha effort

- [ ] Add `create_rc_heap_type()` helper to DebugInfoBuilder
- [ ] Ensure ARC IR instructions carry source spans
- [ ] Verify synthetic instructions inherit correct spans during RC insertion (Section 07)
- [ ] Document raw vs user-friendly debug experience tradeoff

---

## 13.5 Pipeline Wiring (Fix the Existing Gap)

The critical gap: `DebugInfoBuilder` and `DebugContext` exist and are well-implemented, but they are **not used** by the current codegen pipeline (`builder.rs`, `context.rs`, `module.rs`). V2 must wire debug info into four points.

### Point 1: Module Setup

When creating a `CodegenCx` (or `ModuleEmitter`), create the `DebugContext` from the source file path and text:

```rust
// In CodegenCx::new() or ModuleEmitter::new()
// Note: source_path is &Path (not &str). Use Path::new() if converting from a string.
let debug_context = DebugContext::new(
    &module, context, debug_config, source_path, source_text,
);
// Stored as Option<DebugContext<'ctx>> on CodegenCx
```

### Point 2: Function Declaration (DISubprogram)

When declaring a function (Section 04), create and attach a `DISubprogram`:

```rust
// In declare_function() or FunctionCompiler
if let Some(ref dc) = self.debug_context {
    let subroutine_type = dc.di().create_subroutine_type(return_di_type, &param_di_types);
    let subprogram = dc.create_function_with_type(
        name, Some(mangled_name), span_start, subroutine_type, is_local,
    );
    dc.di().attach_function(func_value, subprogram);
    dc.enter_function(subprogram);
    // ... emit parameter debug info (Section 13.3) ...
}
```

### Point 3: Expression Lowering (Source Locations)

At the start of each expression lowering in Section 03's `ExprLowerer`:

```rust
// In ExprLowerer::lower() dispatch
if let Some(ref dc) = self.cx.debug_context {
    let span = arena.span(expr_id);
    dc.set_location_from_offset_in_current_scope(&self.builder, span.start);
}
// ... proceed with expression lowering ...
```

### Point 4: Variable Binding (DILocalVariable)

When processing `Let` statements that bind variables (Section 03's `lower_control_flow`):

```rust
// In compile_let() or equivalent
match &binding {
    ScopeBinding::Immutable(value) => {
        if let Some(ref dc) = self.cx.debug_context {
            emit_dbg_value(dc.di(), &self.builder, *value, name, di_type, line, col, scope);
        }
    }
    ScopeBinding::Mutable(alloca) => {
        if let Some(ref dc) = self.cx.debug_context {
            emit_dbg_declare(dc.di(), &self.builder, *alloca, name, di_type, line, col, scope);
        }
    }
}
```

### Point 5: Module Finalization

Before emitting the module as object code (Section 11's `ModuleEmitter::emit()`):

```rust
// In ModuleEmitter::emit() or ObjectEmitter, before verify+optimize+emit
if let Some(ref dc) = self.debug_context {
    dc.finalize();  // Resolves forward references, validates debug info
}
// ... proceed with verify_module, optimize, emit_object ...
```

### Wiring Order

Debug info wiring depends on other V2 components:

1. **TypeInfo enum** (Section 01) -- needed for `debug_type()` dispatch
2. **IrBuilder** (Section 02) -- needed for ScopeBinding integration
3. **Expression lowering** (Section 03) -- needed for per-expression location setting
4. **Function declaration** (Section 04) -- needed for DISubprogram attachment
5. **ARC IR** (Section 06) -- needed for span preservation through ARC lowering

Debug info wiring should be done last within each component, after the core lowering logic is working.

- [ ] Add `Option<DebugContext>` field to CodegenCx
- [ ] Wire DebugContext::new() into module setup
- [ ] Add DISubprogram creation in function declaration
- [ ] Add per-expression source location setting in ExprLowerer
- [ ] Add DILocalVariable creation in Let binding processing
- [ ] Add finalize() call before module emission
- [ ] Test: verify `lldb ori_program` can set breakpoints on Ori source lines
- [ ] Test: verify `lldb` can inspect local variables with `frame variable`

**Exit Criteria:** `lldb ori_program` can set breakpoints on Ori source lines, step through expressions, and inspect local variables (including ARC-managed types shown as `RC<T>` with refcount and data fields).
