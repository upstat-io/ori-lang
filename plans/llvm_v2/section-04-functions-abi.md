---
section: "04"
title: Function Declaration & Calling Conventions
status: in-progress
goal: Systematic ABI handling using TypeInfo-driven calling conventions, replacing ad-hoc sret threshold
sections:
  - id: "04.1"
    title: Two-Pass Function Compilation
    status: done
  - id: "04.2"
    title: TypeInfo-Driven Calling Conventions
    status: done
  - id: "04.3"
    title: Calling Convention Selection
    status: done
  - id: "04.4"
    title: Closure & Lambda Compilation
    status: not-started
  - id: "04.5"
    title: Name Mangling & Symbol Resolution
    status: not-started
  - id: "04.6"
    title: Method Dispatch
    status: not-started
  - id: "04.7"
    title: Entry Points & Test Wrappers
    status: not-started
---

# Section 04: Function Declaration & Calling Conventions

**Status:** In Progress (04.1–04.3 done, legacy code deleted)
**Goal:** TypeInfo-driven function signature lowering where the calling convention is computed from type properties, not ad-hoc checks. Batch declare-then-define compilation. Fat-pointer closures replacing tagged i64 model. `fastcc` for internal functions.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_llvm/src/abi.rs` -- FnAbi with PassMode (Direct, Indirect, Pair)
- **Swift** `lib/IRGen/GenCall.cpp` -- NativeConventionSchema, Explosion for multi-register values
- **Roc** `crates/compiler/gen_llvm/src/llvm/build.rs` -- FAST_CALL_CONV, sret for large structs

---

## 04.1 Two-Pass Function Compilation

**Phase 1: Declaration** -- Iterate ALL functions, compute LLVM signatures, declare in module.
**Phase 2: Definition** -- Iterate ALL functions, compile bodies using declared functions.

> **Current state:** The codebase uses per-function declare+define in `ModuleCompiler::compile_function_with_sig()`. Each function is declared and immediately defined before moving to the next. This works because functions are compiled in source order and the evaluator iterates `module.functions` sequentially, but it does NOT support forward references (function A calling function B that appears later in source).
>
> Runtime functions are already pre-declared separately via `declare_runtime_functions()` before any user function compilation begins.

V2 explicitly changes to batch mode: declare ALL user functions first, then define all. This ensures forward references work correctly without relying on source order.

```rust
pub struct FunctionCompiler<'a, 'ctx> {
    builder: &'a mut IrBuilder<'ctx>,
    type_info: &'a TypeInfoStore,
    functions: FxHashMap<Name, FunctionId>,  // Declared functions
}

impl FunctionCompiler<'_, '_> {
    /// Phase 1: Declare all functions in the module.
    ///
    /// Iterates every function, computes its V2 FunctionSig (with ParamPassing,
    /// ReturnPassing, and CallConv), and declares it in the LLVM module.
    /// Runtime functions are declared separately via `declare_runtime_functions()`
    /// before this method is called.
    pub fn declare_all(&mut self, module: &Module) {
        for func in &module.functions {
            let sig = self.compute_signature(func);
            let fn_id = self.builder.declare_function(&sig);
            self.functions.insert(func.name, fn_id);
        }
    }

    /// Phase 2: Define all function bodies.
    ///
    /// Every function is already declared, so forward references resolve.
    pub fn define_all(&mut self, module: &Module) {
        for func in &module.functions {
            let fn_id = self.functions[&func.name];
            self.define_function(fn_id, func);
        }
    }
}
```

- [x] Implement declare phase with signature computation ✅ (2026-02-08)
- [x] Implement define phase with body compilation ✅ (2026-02-08)
- [x] Handle recursive functions (already declared before definition) ✅ (2026-02-08)
- [x] Handle function-level attributes (inline, cold, etc.) ✅ (2026-02-08)

## 04.2 TypeInfo-Driven Calling Conventions

> **Current state:** The current `FunctionSig` (in `evaluator.rs`) is minimal:
> ```rust
> pub struct FunctionSig {
>     pub params: Vec<Idx>,
>     pub return_type: Idx,
>     pub is_generic: bool,
> }
> ```
> It carries only type IDs and a generic flag. There is no calling convention, no parameter passing mode, and no return passing mode. The sret decision is made ad-hoc in `CodegenCx::needs_sret()` at declaration time.

V2 enhances `FunctionSig` with `ParamPassing`, `ReturnPassing`, and `CallConv`:

> **Note on `is_generic`:** V1's `FunctionSig` included `is_generic: bool`. V2 intentionally drops this field from the signature. Generic functions are not lowered directly — they are monomorphized first, producing concrete `FunctionSig` instances for each instantiation. The `FunctionCompiler::declare_all()` phase skips uninstantiated generic functions (they have no concrete types to lower). Generic filtering thus moves from the signature to the declaration phase.

```rust
pub struct FunctionSig {
    params: Vec<ParamSig>,
    return_sig: ReturnSig,
    calling_convention: CallConv,
}

pub struct ParamSig {
    name: Name,
    ty: Idx,
    passing: ParamPassing,      // From TypeInfo
    borrow: bool,               // From ARC analysis (Section 06)
}

pub struct ReturnSig {
    ty: Idx,
    passing: ReturnPassing,     // From TypeInfo
}

pub enum ParamPassing {
    /// Type fits in <=2 registers (<=16 bytes on x86-64).
    /// Passed directly by value in registers.
    Direct,
    /// Type exceeds 2 registers (>16 bytes on x86-64).
    /// Passed via `byval(T)` attribute -- caller allocates stack copy,
    /// LLVM transparently copies to callee's stack frame.
    Indirect { alignment: u32 },
    /// Unit/never types -- no value passed.
    Void,
    /// ARC'd types passed by reference (if applicable from borrow inference).
    /// NOTE: Added by Section 06 (borrow inference); not produced in Tier 1.
    /// compute_param_passing() never returns this variant — it is only set
    /// by the borrow inference pass when a parameter is proven to be borrowed.
    Reference,
}

pub enum ReturnPassing {
    /// Return value fits in registers (<=16 bytes).
    Direct,
    /// Return value too large for registers (>16 bytes).
    /// Caller provides hidden first parameter `ptr sret(T) noalias`.
    Sret { alignment: u32 },
    /// Unit/never -- function returns void.
    Void,
}

pub enum CallConv {
    /// LLVM `fastcc` -- internal Ori functions.
    Fast,
    /// LLVM `ccc` -- C calling convention for FFI, @main, @panic, runtime.
    C,
}
```

**Threshold logic:**

> **Current sret rule:** struct with >2 fields gets sret. The actual code in `context.rs`:
> ```rust
> pub fn needs_sret(&self, return_type: Idx) -> bool {
>     if return_type == Idx::UNIT || return_type == Idx::NEVER {
>         return false;
>     }
>     let llvm_ty = self.llvm_type(return_type);
>     matches!(llvm_ty, BasicTypeEnum::StructType(st) if st.count_fields() > 2)
> }
> ```
> This works because Ori struct fields are each 8 bytes (i64/f64/ptr), so >2 fields = >16 bytes on x86-64. But it's an ad-hoc check with no alignment awareness and no TypeInfo integration.

V2 replaces this with TypeInfo-driven thresholds:

```rust
fn compute_param_passing(ty: Idx, type_info: &TypeInfoStore) -> ParamPassing {
    let info = type_info.get(ty);
    // size() returns Option<u64>:
    //   Some(0) for unit/never → Void (no value passed)
    //   None for dynamically-sized → Indirect (size unknown at compile time)
    //   Some(1..=16) → Direct (fits in <=2 registers)
    //   Some(>16) → Indirect (too large for registers, use byval)
    match info.size() {
        Some(0) => ParamPassing::Void,                // unit, never
        None => ParamPassing::Indirect {              // dynamically-sized
            alignment: info.alignment(),
        },
        Some(1..=16) => ParamPassing::Direct,         // fits in <=2 registers
        Some(_) => ParamPassing::Indirect {            // >16 bytes, use byval
            alignment: info.alignment(),
        },
    }
}

fn compute_return_passing(ty: Idx, type_info: &TypeInfoStore) -> ReturnPassing {
    let info = type_info.get(ty);
    // size() returns Option<u64>:
    //   Some(0) for unit/never → Void (function returns void)
    //   None for dynamically-sized → Sret (size unknown, use hidden pointer)
    //   Some(1..=16) → Direct (fits in registers)
    //   Some(>16) → Sret (too large for registers, caller provides pointer)
    match info.size() {
        Some(0) => ReturnPassing::Void,               // unit, never
        None => ReturnPassing::Sret {                 // dynamically-sized
            alignment: info.alignment(),
        },
        Some(1..=16) => ReturnPassing::Direct,
        Some(_) => ReturnPassing::Sret {
            alignment: info.alignment(),
        },
    }
}
```

The threshold (16 bytes = 2 registers on x86-64 SysV ABI) is the same as the current ad-hoc check but is now driven by `TypeInfo::size()` rather than counting LLVM struct fields. `TypeInfo::size()` returns the ABI size (the size used for allocation and parameter passing). Note: `size()` returns `Option<u64>` — `Some(0)` means zero-sized (unit/never, mapped to Void), while `None` means dynamically-sized (mapped to Indirect/Sret, not Void).

**Coordinated sret pattern (current, preserved in V2):**
1. **Declare:** Add `sret(T)` + `noalias` attributes on param 0; function returns void
2. **Define:** Store result through the sret pointer, then `ret void`
3. **Call:** Allocate stack slot via `create_entry_alloca`, prepend pointer as arg 0, load result after call

**Indirect parameter passing (byval) -- NEW in V2:**

For parameters exceeding 16 bytes (>2 fields for Ori structs):
1. **Declare:** Add `byval(T)` attribute on the parameter
2. **Call site:** `alloca T` + `store value` + pass pointer
3. **Callee:** LLVM transparently copies byval params to the callee's stack frame -- the callee sees a local copy

This matches the sret threshold for consistency: same 16-byte boundary for both parameters and returns.

- [x] Implement `compute_signature()` using TypeInfo ✅ (2026-02-08, `abi.rs::compute_function_abi`)
- [x] Implement parameter marshaling (direct vs indirect/byval) ✅ (2026-02-08, `abi.rs::compute_param_passing`)
- [x] Implement return value handling (direct vs sret) ✅ (2026-02-08, `abi.rs::compute_return_passing`)
- [ ] Handle variadic functions (future)
- [x] Handle method receivers (self parameter) ✅ (2026-02-08, `function_compiler.rs::compile_impls`)

## 04.3 Calling Convention Selection

> **Current state:** All functions use the default C calling convention (`ccc`). There is no `fastcc` usage anywhere in the codebase.

V2 assigns calling conventions based on function role:

| Function kind | Calling convention | Reason |
|---|---|---|
| Internal Ori functions | `fastcc` | Enables tail call optimization, uses more registers |
| `@main` | `ccc` | Must match C `main()` ABI for OS entry |
| `@panic` | `ccc` | Called from C runtime |
| FFI / `extern` functions | `ccc` | Must match C ABI |
| Runtime functions (`ori_*`) | `ccc` | Implemented in Rust with `#[no_mangle]`, linked as C |
| Test wrappers (`_ori_test_*`) | `fastcc` | Internal, called from JIT and AOT test runners |

**Reference:** Roc uses the same pattern -- `FAST_CALL_CONV` for all internal Roc functions, C convention only at FFI boundaries. See `crates/compiler/gen_llvm/src/llvm/build.rs`.

`fastcc` benefits:
- Tail call optimization (TCO) -- essential for recursive Ori functions
- Potentially more registers available (callee-saved vs caller-saved differs)
- Compiler can choose optimal register allocation without ABI constraints

```rust
fn select_calling_convention(func: &Function, interner: &StringInterner) -> CallConv {
    let name = interner.lookup(func.name);
    // Note: The `@` prefix in `@main` and `@panic` is source syntax only.
    // The interned function name does NOT include `@` — it stores "main"
    // and "panic" respectively. The parser strips the prefix during interning.
    if name == "main" || name == "panic" {
        CallConv::C
    } else if func.is_extern() {
        CallConv::C
    } else {
        CallConv::Fast
    }
}
```

- [x] Set `fastcc` on internal Ori function declarations ✅ (2026-02-08, `abi.rs::select_call_conv`)
- [x] Keep `ccc` for `@main`, `@panic`, FFI, and runtime functions ✅ (2026-02-08)
- [ ] Verify tail call optimization works with `fastcc` + `musttail`

## 04.4 Closure & Lambda Compilation

### Current model (to be replaced)

The current closure implementation in `functions/lambdas.rs` uses a tagged i64 scheme:

- **Tagged i64 representation:** Closures coerce all captured values to i64. The closure value itself is an i64 where bit 0 distinguishes plain function pointers (bit 0 = 0) from boxed closures (bit 0 = 1).
- **Boxed closure struct:** `{ i8 capture_count, i64 fn_ptr, capture0: i64, capture1: i64, ... }` -- heap-allocated via `ori_closure_box()`.
- **No-capture path:** Returns the function pointer as a plain i64 (no boxing).
- **Max 8 captures:** The `call_boxed_closure` function loads a fixed maximum of 8 capture slots, regardless of actual count. Extra captures are silently dropped.
- **`__lambda_N` naming:** Uses a global `AtomicU64` counter for unique lambda names.
- **No type safety:** All values coerced to i64 at closure boundaries via `coerce_to_i64()`.

Limitations:
- i64 coercion loses type information -- structs, strings, and other non-scalar types cannot be captured correctly
- Maximum 8 captures is arbitrary and silent -- no error for >8
- Tag bit wastes the lowest bit of function pointers (works on 8-byte-aligned pointers but is fragile)
- Closure call dispatch has runtime branching (check tag bit, load capture count) even for known-static closures

### V2 model: Fat pointer

**Representation:** `{ fn_ptr: ptr, env_ptr: ptr }` -- a two-word (16-byte) fat pointer.

```llvm
%closure = type { ptr, ptr }   ; { fn_ptr, env_ptr }
```

**No-capture optimization:** When a closure has no captures, `env_ptr` is `null`. Call sites can check for null and call `fn_ptr` directly (no environment dereference).

**Environment struct:** Captures are stored in a heap-allocated, ARC-managed struct using the 8-byte header layout (consistent with all other RC'd types in Ori):

```llvm
; Environment for a closure capturing x: int and name: str
; Allocated via ori_rc_alloc(sizeof(%env_lambda_3), align)
; which returns a data pointer; header (strong_count) lives at ptr-8.
%env_lambda_3 = type {
    i64,          ; capture 'x' (int, stored at native type)
    { ptr, i64 }  ; capture 'name' (str, stored at native type)
}

; Heap layout (see Section 01.6, Section 07.3):
;   ┌──────────────────┬───────────────────────┐
;   │ strong_count: i64│ env_lambda_3 data ... │
;   └──────────────────┴───────────────────────┘
;   ^                  ^
;   ptr - 8            ptr (env_ptr stored in closure fat pointer)
```

- No i64 coercion -- captures stored at their native types
- No capture limit -- environment struct grows as needed
- No refcount inside the struct -- 8-byte header (strong_count only) lives at negative offset, consistent with all other RC'd types (str, list, map, set, channel)
- Allocation: `ori_rc_alloc(sizeof(env), align)` returns data pointer; header at ptr-8
- ARC-managed via the same header-at-negative-offset layout used by all Ori heap objects (Section 01.6, Section 07)

**Function signature:** All closures receive `env_ptr` as a hidden first parameter:

```llvm
; Lambda (x: int) -> int that captures 'x: int' (field 0) and 'name: str' (field 1)
define fastcc i64 @__lambda_3(ptr %env, i64 %arg_x) {
    ; Load captures from environment (no refcount field — header is at ptr-8)
    %x_ptr = getelementptr %env_lambda_3, ptr %env, i32 0, i32 0
    %x = load i64, ptr %x_ptr
    ; ... use %x and %arg_x ...
}
```

For no-capture closures, `env_ptr` is `null` and the function simply ignores it.

**Capture by value:** Per Ori's ARC-Safe design pillar, closures capture by value. Mutable variables are loaded at closure creation time (snapshot semantics). This is already the behavior of the current implementation.

**Calling a closure:**

```rust
fn call_closure(closure: ClosureValue, args: &[Value]) -> Value {
    let fn_ptr = extract_value(closure, 0);  // fn_ptr
    let env_ptr = extract_value(closure, 1); // env_ptr
    // Prepend env_ptr as hidden first arg
    let all_args = [env_ptr].chain(args);
    call_indirect(fn_ptr, all_args)
}
```

**Calling convention:** All Ori closures use `fastcc`, matching other internal Ori functions. Indirect closure calls (through a function pointer extracted from a fat-pointer closure) always use `fastcc`. FFI function pointers are NOT closures — they use `ccc` and require thunk wrappers to bridge between `ccc` (FFI side) and `fastcc` (Ori side) when passed as closures to Ori code.

- [ ] Implement fat-pointer closure representation `{ ptr, ptr }`
- [ ] Implement environment struct generation per lambda
- [ ] Wire captures to native types (no i64 coercion)
- [ ] Implement no-capture optimization (null env_ptr, direct call)
- [ ] ARC integration for environment structs (Section 07)
- [ ] Remove `LAMBDA_COUNTER`, `ori_closure_box`, tag-bit scheme

## 04.5 Name Mangling & Symbol Resolution

> **Current state:** The mangling scheme lives in `aot/mangle.rs`. The JIT path does NOT use mangling -- functions are declared with their unmangled interned names (e.g., `"add"`, `"process"`). Only the AOT path uses mangled names.

**Mangling format** (from `aot/mangle.rs`):
- Simple function: `_ori_<module>$<function>` (e.g., `_ori_math$add`)
- Trait implementation: `_ori_<type>$$<trait>$<method>` (e.g., `_ori_int$$Eq$equals`)
- Associated function: `_ori_<type>$A$<function>` (e.g., `_ori_Option$A$some`)
- Extension method: `_ori_<type>$$ext$<method>`
- Generic instantiation: `_ori_<module>$<function>$G<type_args>`
- `$` as module separator, `$$` for trait implementations, `$A$` for associated functions
- Type names in mangled symbols are encoded via `encode_identifier` to produce valid LLVM symbol characters (e.g., `[int]` becomes `list_int_`, `result<str, Error>` becomes `result_str_Error_`)

V2: All paths (AOT and JIT) use mangled names for consistency and to support same-name functions in different modules.

- [ ] Wire existing Mangler into FunctionCompiler for both JIT and AOT paths
- [ ] Handle overloaded functions (name + type signature)
- [ ] Handle lambda/closure names (replace `__lambda_N` with mangled anonymous names)

## 04.6 Method Dispatch

> **Current state:** Method calls are compiled in `functions/calls.rs`. The dispatch order is:
> 1. Built-in methods tried first (matched by receiver `Idx` -- e.g., `int`, `float`, `bool`, `str`, `char`, `byte`)
> 2. Module-level name lookup: `self.cx().llmod().get_function(method_name)` -- searches for a function with the unmangled method name
>
> **Limitation:** Method names must be unique across all types in a module because lookup is by unmangled name. If two types define a method with the same name (e.g., `Point.distance()` and `Line.distance()`), only one can exist in the LLVM module.

V2 uses mangled names for methods to allow same-name methods on different types:
- Method `distance` on `Point`: `_ori_<module>$Point$distance`
- Method `distance` on `Line`: `_ori_<module>$Line$distance`

The method call compiler resolves the receiver type, constructs the mangled name, and looks up the function by mangled name.

- [ ] Mangle method names using `_ori_<module>$<type>$<method>` format
- [ ] Update method call compilation to use mangled lookup
- [ ] Preserve built-in method fast path (no mangling for primitives)

## 04.7 Entry Points & Test Wrappers

### Entry points

> **Current state:** There is NO entry point wrapper generation. The JIT evaluator calls functions directly by name. For AOT, `@main` would need a C-compatible `main()` wrapper, but this is not yet implemented.

V2 must generate proper C entry point wrappers for AOT compilation:

**`@main () -> void`:**
```llvm
define i32 @main() {
    call fastcc void @_ori_main()
    ret i32 0
}
```

**`@main () -> int`:**
```llvm
define i32 @main() {
    %result = call fastcc i64 @_ori_main()
    %exit_code = trunc i64 %result to i32
    ret i32 %exit_code
}
```

**`@main (args: [str]) -> void` / `@main (args: [str]) -> int`:**
```llvm
define i32 @main(i32 %argc, ptr %argv) {
    ; Convert argc/argv to Ori [str]
    %args = call ccc ptr @ori_args_from_argv(i32 %argc, ptr %argv)
    call fastcc void @_ori_main(ptr %args)
    ret i32 0
}
```
Requires a runtime helper `ori_args_from_argv` to parse C argc/argv into an Ori `[str]` list.

**`@panic (info: PanicInfo) -> void`:**
Registered as a global function pointer that the runtime calls on panic:
```llvm
@ori_user_panic_handler = global ptr @_ori_panic
```
The runtime checks this global before using the default panic handler.

- [ ] Generate C `main()` wrapper for `@main` (all 4 signatures)
- [ ] Implement `ori_args_from_argv` runtime helper
- [ ] Generate `@panic` handler registration

### Test wrappers

Test functions are compiled as `_ori_test_<name>` with void signature:

```rust
let wrapper_name = format!("_ori_test_{test_name_str}");
let void_sig = FunctionSig {
    params: vec![],
    return_sig: ReturnSig { ty: Idx::UNIT, passing: ReturnPassing::Void },
    calling_convention: CallConv::Fast,
};
compiler.compile_function_with_sig(&test_func, arena, expr_types, Some(&void_sig));
```

This pattern is already implemented in the evaluator (`evaluator.rs` lines 468-490) using the `__test_` prefix. V2 migrates both JIT and AOT paths to the unified `_ori_test_` prefix for consistency with the `_ori_` mangling convention used by all other Ori symbols. The JIT path must be updated to use `_ori_test_` as well (migration from `__test_` to `_ori_test_`). V2 uses `fastcc` for test wrappers since they are internal.

- [ ] Migrate `__test_<name>` to `_ori_test_<name>` in both JIT and AOT paths
- [ ] Use `fastcc` for test wrappers

---

**Exit Criteria:** Functions are declared and defined in batch using TypeInfo-driven signatures. Closures use fat-pointer representation with native-typed captures. Internal functions use `fastcc`. Entry point wrappers generated for AOT. No ad-hoc sret checks, no i64 coercion, no capture limit.
