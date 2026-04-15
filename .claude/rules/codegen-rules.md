---
paths:
  - "compiler/ori_llvm/**/*.rs"
  - "compiler/ori_rt/**/*.rs"
  - "compiler/ori_llvm/tests/**/*.ori"
---

# Codegen Emission Formal Ruleset

This document defines the **laws** of the codegen emission layer — the boundary between AIMS analysis output and executable machine code. The AIMS formal ruleset (`aims-rules.md`) governs **what** memory operations to emit; this document governs **how** those operations become LLVM IR and runtime calls. If the code violates a rule stated here, the code has a bug.

**Interface with AIMS**: The AIMS pipeline (aims-rules.md §7–§8) produces an `ArcFunction` with RC operations annotated as ARC IR instructions (`RcInc`, `RcDec`, `IsShared`, `Set`, `Reuse`). The `ArcIrEmitter` in `ori_llvm/src/codegen/arc_emitter/` translates these instructions to LLVM IR according to the rules below. AIMS rules RL-1 through RL-34 specify the emission *decisions*; codegen rules specify the emission *implementation*. The two form a continuous chain: analysis → realization → emission → runtime ABI.

**Relationship to llvm.md and runtime.md**: Those files are *operational* guides (how to build, debug, test). This document is *normative* (what the emission layer must guarantee). When they conflict, this document is authoritative.

**Scope**: This ruleset covers the ARC emission core, type resolution, ABI, narrowing, trampolines, iterators, and runtime contract. Additional emission surfaces — derive codegen, exception handling, pattern matching, control flow, main/test wrappers — are documented in their respective operational guides and will be formalized in future extensions as they mature.

---

## Notation

- **SHALL** = mandatory requirement (violation = implementation bug)
- **SHOULD** = recommended practice (violation = design smell, may be justified)
- `resolve_type(idx)` = the canonical LLVM type for an Ori type index
- `element_store_size(idx)` = the byte size for storing an element of type `idx`
- `abi_size(idx)` = the ABI-level size of type `idx` in bytes
- **canonical type** = the LLVM type produced by `resolve_type()` for a given Ori type, without repr-opt narrowing applied
- **narrowed type** = an LLVM type smaller than canonical, produced by repr-opt analysis (e.g. i32 instead of i64 for a range-proven int)
- **storage boundary** = the point where values are read from or written to a collection's backing buffer
- Rules are numbered `CATEGORY-N`. Categories: `TR` (type resolution), `AB` (ABI), `NR` (narrowing), `TM` (trampoline), `RE` (RC emission), `IT` (iterator), `RT` (runtime contract), `AT` (LLVM attributes), `VR` (verification)
- Cross-references: aims-rules.md rules prefixed with `AIMS:` (e.g., `AIMS:RL-1`)

---

## §1 Type Resolution

Every Ori type maps to exactly one LLVM type via `resolve_type()`. The mapping is deterministic and context-free — the same `Idx` always produces the same LLVM type.

### TR-1 — Canonical Type Mapping

The following mapping SHALL be the single source of truth for Ori → LLVM type translation. Source: `TypeInfo::storage_type()` in `codegen/type_info/info.rs` and `TypeLayoutResolver` in `codegen/type_info/layout_resolver.rs`.

| Ori Type | LLVM Type | Size (bytes) | Notes |
|----------|-----------|-------------|-------|
| `int` | `i64` | 8 | |
| `float` | `f64` | 8 | |
| `bool` | `i1` | 1 | |
| `byte` | `i8` | 1 | |
| `char` | `i32` | 4 | |
| `void` / `()` | `i64` (unit sentinel) | 8 | |
| `str` | `{ i64, i64, ptr }` | 24 | Fat pointer: len, cap, data |
| `[T]` | `{ i64, i64, ptr }` | 24 | Fat pointer: len, cap, data |
| `{K: V}` | `{ i64, i64, ptr }` | 24 | Same fat pointer layout as list/str |
| `Set<T>` | `{ i64, i64, ptr }` | 24 | Same fat pointer layout as list/str |
| `Option<T>` | `TypeLayoutResolver` | varies | Tag + payload; niche-encoded when possible |
| `Result<T, E>` | `TypeLayoutResolver` | varies | Tag + max(T, E); niche-encoded when possible |
| Struct | struct of resolved fields | varies | `TypeLayoutResolver`; field order from `ReprPlan` |
| Tuple | struct of resolved elements | varies | `TypeLayoutResolver` |
| Function/Closure | `{ ptr, ptr }` | 16 | fn_ptr + env_ptr (`TypeInfo::Function`) |
| `Duration` | `i64` (nanoseconds) | 8 | |
| `Size` | `i64` (bytes) | 8 | |
| `Ordering` | `i8` | 1 | Less=0, Equal=1, Greater=2 |
| `Range` | `{ i64, i64, i64, i64 }` | 32 | start, end, step, inclusive flag |
| Enum | `TypeLayoutResolver` | varies | Tag + max payload; may be niche-encoded |
| `Iterator<T>` | `ptr` (opaque handle) | 8 | Runtime iterator state |
| `Channel` | `ptr` (opaque handle) | 8 | Runtime channel handle |
| `Never` | `i64` | 8 | Same storage as unit in canonical table |

**Critical notes:**
- `{K: V}` and `Set<T>` use the same 24-byte fat pointer as lists/strings — NOT opaque `ptr`. All four collection types share the `{ i64, i64, ptr }` representation.
- `Option<T>` and `Result<T, E>` use `TypeLayoutResolver`, which may niche-encode (eliding the tag when the payload has a niche, e.g. `Option<ptr>` uses null niche) or tagged-pointer encode. Do NOT assume a fixed `{ tag, payload }` layout.
- `Never` is stored as `i64` in the canonical storage table, not `void`.

Rationale: Deterministic mapping prevents ABI mismatches between declaration and definition passes. The two-pass compilation model (`declare_all` → `define_all`) requires that both passes agree on types.

### TR-2 — Full Resolution Before Translation

All type indices SHALL be fully resolved via `pool.resolve_fully(idx)` before LLVM type construction. Unresolved type variables (`Tag::Var`) SHALL NOT reach codegen — their presence is a type checker bug (cross-phase invariant contract per `impl-hygiene.md` §Cross-Phase Invariant Contracts).

Rationale: Unresolved type variables produce `ptr` or poison types in LLVM IR, causing silent miscompilation rather than clean errors.

### TR-3 — Aggregate Field Ordering

Struct and tuple fields SHALL be ordered according to the `ReprPlan` memory layout when a repr-opt plan is available. When no `ReprPlan` exists, declaration order is used. The `original_index` field in `ReprPlan` entries maps between declaration order (type pool) and memory order (LLVM struct).

Rationale: Field reordering for alignment reduces padding. All code that accesses struct fields must use the reordered index, not the declaration index. A mismatch between construction order and extraction order is a silent data corruption bug.

### TR-4 — Collection Fat Pointer Layout

All collection types (list, string, map, set) SHALL use the fat pointer layout `{ len: i64, cap: i64, data: ptr }`. This is a fixed 24-byte structure — it does NOT vary by element type or collection kind.

Rationale: The runtime (`ori_rt`) expects this exact layout for all collection operations. Element type information is encoded in `elem_size` arguments to runtime functions, not in the pointer structure.

### TR-5 — Closure Representation

All closures SHALL use the `{ fn_ptr: ptr, env_ptr: ptr }` layout. `CLOSURE_FIELD_FN = 0`, `CLOSURE_FIELD_ENV = 1` (defined in `ori_ir`). The `fn_ptr` field points to a function with `fastcc` calling convention. The `env_ptr` field is either a pointer to a heap-allocated capture environment or `null` for closures with no captures.

Rationale: Closure representation is shared between the ARC lowerer (which constructs closures), the emitter (which calls through them), and trampolines (which unpack them). All three must agree on field indices.

---

## §2 ABI Computation

The ABI layer decides how each parameter and return value is passed between caller and callee at the LLVM IR level.

### AB-1 — Size Threshold for Indirect Passing

Types with `abi_size > 16` bytes SHALL be passed indirectly (via pointer). Types with `abi_size <= 16` bytes SHALL be passed directly (by value).

The 16-byte threshold is load-bearing:
- Matches the SystemV AMD64 ABI two-register limit (RDI+RSI or XMM0+XMM1)
- Avoids FastISel aggregate spill bugs (see AB-5)
- All 24-byte fat-pointer types (str, [T], {K:V}, Set<T>) are always indirect

Rationale: BUG-04-071 class — incorrect size computation leads to register misalignment and SIGSEGV on some targets.

### AB-2 — ParamPassing Classification

Parameters SHALL be classified into one of:

| Mode | Condition | LLVM Semantics |
|------|-----------|----------------|
| `Direct` | `abi_size <= 16` ∧ (owned ∨ scalar) | Passed by value in registers |
| `Indirect { alignment }` | `abi_size > 16` ∧ (owned ∨ scalar) | Passed as `ptr` (caller allocates, callee reads) |
| `Reference` | borrowed ∧ non-scalar | Passed as `ptr` (caller retains ownership) |
| `Void` | type is void/unit/Never | No parameter emitted |

Basic classification is computed by `compute_param_passing()` in `codegen/abi/mod.rs`. Ownership-aware classification (which adds `Reference` for borrowed params) is computed by `compute_param_passing_with_ownership()`.

### AB-3 — ReturnPassing Classification

Return values SHALL be classified into one of:

| Mode | Condition | LLVM Semantics |
|------|-----------|----------------|
| `Direct` | `abi_size <= 16` | Returned in registers |
| `Sret { alignment }` | `abi_size > 16` | Caller-allocated `sret` pointer as first parameter |
| `Void` | Return type is void/unit/Never | No return value |

### AB-4 — Sret on ARM64

When using `Sret` return passing, the sret pointer SHALL be emitted with the LLVM `sret` attribute via `call_indirect_with_sret()`. On ARM64, `sret` uses the dedicated `X8` register — without the attribute, LLVM places the pointer in `X0` instead of `X8`, causing register misalignment and SIGSEGV.

Rationale: Empirical: map trampoline SIGSEGV on aarch64 traced to missing sret attribute. Cross-language: zig#1450 (identical sret miscompilation), rust#148239 (sret argument misordering).

### AB-5 — FastISel Aggregate Restriction

Struct loads exceeding 16 bytes in JIT mode SHALL NOT use a single `load %BigStruct, ptr` instruction. Instead, per-field `struct_gep` + `load` + `insert_value` SHALL be used.

Rationale: FastISel (used in debug/JIT) mishandles large aggregate spills. Symptoms: SIGSEGV in release only with identical IR in both builds. Entry-block allocas, `noredzone`, and calling convention changes do NOT fix this. See `IrBuilder::load()` and `FunctionCompiler::load_param_values()` for the decomposition implementation.

### AB-6 — Calling Convention Assignment

| Context | Convention | LLVM CC |
|---------|-----------|---------|
| Ori functions | `fastcc` | Fast calling convention |
| Runtime functions | `ccc` | C calling convention |
| Trampolines | `ccc` | C calling convention (called by runtime) |
| Main wrapper | `ccc` | C calling convention (entry from OS) |

`fastcc` enables tail-call optimization for Ori-to-Ori calls. Runtime functions use `ccc` because `ori_rt` is compiled as a C-ABI static library.

### AB-7 — Ownership-Aware ABI

When AIMS borrow inference (aims-rules.md §5) determines a parameter is `Borrowed`, the ABI SHALL reflect this in the `ParamAbi` via `compute_param_passing_with_ownership()`. Borrowed parameters transfer no RC responsibility to the callee — the caller retains ownership.

---

## §3 Narrowing Boundaries

Representation optimization (repr-opt) can narrow types below their canonical width — e.g., an `int` proven to fit in `[0, 255]` is stored as `i8` in a collection's backing buffer. Narrowing is a **storage optimization** and SHALL NOT leak beyond the storage boundary.

This section is the formalization of the BUG-04-071 fix. The bug was caused by narrowed element sizes leaking into the iterator pipeline (trampolines, scratch buffers, collect allocation), causing memory corruption.

### NR-1 — Storage Boundary Principle (THE foundational rule)

Narrowing decisions SHALL be consumed ONLY by code that directly reads/writes collection backing buffers or struct fields. All other code — including iterator pipeline operations, trampolines, scratch buffers, runtime function calls, and consumer allocations — SHALL use canonical element sizes (`element_store_size(elem_ty)` or `resolve_type(elem_ty)`).

**The storage boundary is the `emit_list_iter` function.** Inside: narrowed sizes for buffer access. Outside: canonical sizes for everything else. The sext widening map adapter (NR-4) is the bridge.

Rationale: The narrowed representation is an internal optimization of the collection's physical layout. Code above the storage boundary operates on logical values, not physical representation. Leaking physical representation into logical operations causes type-width mismatches (i8 where i64 expected), buffer overruns (allocating 1 byte where 8 needed), and register misalignment.

### NR-2 — Collection Element Size Scoping

`collection_elem_size(collection_idx, elem_ty)` SHALL be called ONLY at storage-boundary sites:
- List element indexing (`emit_list_index`)
- List element storage (construction, `emit_construct`)
- List data allocation/deallocation (`emit_drop_list_free_data`, `emit_buffer_rc_dec_*`)
- List iteration source creation (`emit_list_iter`) — for the **source buffer stride only**
- List slice operations (`emit_list_slice`, `emit_list_take`, `emit_list_drop`)
- Map/set internal storage

At all non-storage sites, `element_store_size(elem_ty)` SHALL be used instead. This function returns the canonical byte size for the element type.

**IT-1 boundary exception**: The `emit_list_iter` widening path (NR-4) is a storage-boundary site — it intentionally passes the narrowed `elem_size` to both `ori_iter_from_list` AND the wrapping `ori_iter_map`. This is the ONE place where a runtime iterator function receives a narrowed size, and it is correct because the `ori_iter_from_list` source reads from the narrowed buffer and the wrapping `ori_iter_map`'s `in_size` must match the source's output stride.

Rationale: `collection_elem_size` consults the `ReprPlan` for narrowed widths. Calling it at non-storage sites (e.g., when sizing a trampoline scratch buffer) injects narrowed sizes where canonical sizes are expected.

### NR-3 — Iterator Pipeline Uses Canonical Types

The entire iterator pipeline — from the first adapter (map, filter, take, skip, etc.) through all consumers (fold, collect, count, any, all, etc.) — SHALL operate exclusively on canonical element types.

Specifically:
- Trampoline element loads/stores SHALL use `resolve_type(elem_ty)` (TR-1), never `collection_elem_llvm_type()`
- Consumer scratch buffers SHALL be sized by `element_store_size(elem_ty)`, never `collection_elem_size()`
- Collect allocation SHALL use canonical element sizes
- Runtime iterator functions (`ori_iter_next`, `ori_iter_map`, etc.) receive `elem_size` in canonical bytes — **except** the IT-1 boundary pair (`ori_iter_from_list` + widening `ori_iter_map`) which intentionally receives narrowed sizes

Rationale: The runtime iterator state machine (`ori_rt/src/iterator/state.rs`) uses `elem_size` for `memcpy` between internal buffers. If the codegen passes a narrowed size but the runtime copies canonical-width data, it reads/writes past buffer boundaries.

### NR-4 — Sext Widening at iter() Boundary

When a list has narrowed integer elements, `emit_list_iter` SHALL inject a sign-extension widening step at the iterator boundary. The implementation is:

1. Call `ori_iter_from_list(data, len, cap, narrowed_elem_size)` — creating the source iterator with the **narrowed** buffer stride
2. Wrap that iterator with `ori_iter_map(iter, sext_trampoline, null_env, narrowed_elem_size)` — 4 parameters. The `in_size` matches the narrowed stride; the sext trampoline internally writes canonical i64 to `out_ptr`, so downstream consumers see canonical elements without an explicit output-size parameter.

After the wrapping `ori_iter_map` call:
- The output iterator's `elem_size` is the **canonical** i64 size (8 bytes)
- All downstream iterator operations see canonical i64 elements
- The narrowed buffer element size is consumed ONLY by the source iterator and the wrapping map

The sext widening trampoline is generated by `generate_sext_widening_trampoline()` in `codegen/arc_emitter/builtins/trampolines.rs`.

Rationale: This is the bridge between storage-boundary narrowing and canonical-pipeline operation. Without it, the iterator would copy narrowed bytes into canonical-sized slots, reading garbage for the upper bytes.

### NR-5 — Local Variable Narrowing

Local variable narrowing (integer narrowing phase B) inserts `trunc` + `sext` pairs at the variable's **definition site**, not at use sites. The narrowed value is immediately widened back to canonical width. The trunc+sext pair constrains the LLVM value range, enabling downstream LLVM optimization passes to exploit the narrower range.

Exclusions:
- Function parameters are excluded (ABI contract requires canonical types at function boundaries)
- Non-int types are excluded (only `Tag::Int` variables are narrowed)
- Already-narrow types (`bits <= width * 8`) pass through unchanged

Implemented in `narrow_local_if_needed()` and `compute_narrowed_vars()` in `narrowing_codegen.rs`.

### NR-6 — Struct Field Narrowing

Struct fields are narrowed at construction and widened at extraction:

- **Construction** (`trunc_for_narrowed_struct`): canonical values are truncated to the narrowed field width before `insert_value` into the LLVM struct. Only fields whose pool type is `Tag::Int` (canonical i64) or `Tag::Float` (canonical f64) are affected. Naturally narrow types (`Byte` → i8, `Char` → i32) are NOT truncated.
- **Extraction** (`sext_narrowed_field`): narrowed fields are sign-extended (int) or fp-extended (float) back to canonical width after `extract_value`. The destination type determines whether widening is needed.

Construction and extraction MUST be symmetric — a value stored via `trunc_for_narrowed_struct` MUST be recoverable via `sext_narrowed_field` with identical semantics.

---

## §4 Trampolines

Trampolines bridge Ori closures (fastcc, `{fn_ptr, env_ptr}`) to C-ABI function pointers expected by the runtime's iterator operations.

### TM-1 — Trampoline Variants

Four trampoline variants SHALL exist, each matching a specific runtime callback signature:

| Variant | Signature | Purpose |
|---------|-----------|---------|
| `Map` | `(env: ptr, in_ptr: ptr, out_ptr: ptr) -> void` | Transform element, write result |
| `Predicate` | `(env: ptr, elem_ptr: ptr) -> i8` | Test element, return boolean |
| `ForEach` | `(env: ptr, elem_ptr: ptr) -> void` | Process element, discard result |
| `Fold` | `(env: ptr, acc_ptr: ptr, elem_ptr: ptr, out_ptr: ptr) -> void` | Combine accumulator + element |

All trampolines use `ccc` (C calling convention) because they are called by the runtime.

### TM-2 — Canonical Types in Trampolines

All trampolines SHALL use `resolve_type(elem_ty)` for element type resolution — never narrowed or collection-specific types. By the time elements reach user trampolines, they have already passed through the widening map adapter (NR-4) and are always canonical.

Rationale: BUG-04-071 consensus. Trampolines operate on logical values, not physical collection representation.

### TM-3 — Indirect Passing in Trampolines

When `abi_size(elem_ty) > 16`, the trampoline SHALL pass the element pointer directly to the Ori closure without loading. When `abi_size(elem_ty) <= 16`, the trampoline SHALL load the element from the buffer pointer before calling.

The same threshold (AB-1) applies to accumulator types in `Fold` trampolines and result types in `Map` trampolines.

### TM-4 — Sret Handling in Trampolines

When the Ori closure returns an indirect type (`abi_size > 16`), the trampoline SHALL use `call_indirect_with_sret()` to emit the sret attribute. This is mandatory on ARM64 (AB-4) and correct on all targets.

### TM-5 — Boolean Conversion

`Predicate` trampolines SHALL convert the Ori `i1` boolean return to `i8` via `zext` before returning. The runtime expects C-ABI `i8` (0 or 1), not LLVM `i1`.

### TM-6 — Closure Unpacking Protocol

All trampolines SHALL unpack the Ori closure from the `env` parameter using the closure field indices from `ori_ir`:
1. `struct_gep(closure_type, env_ptr, CLOSURE_FIELD_FN)` → load `fn_ptr`
2. `struct_gep(closure_type, env_ptr, CLOSURE_FIELD_ENV)` → load `env_ptr`

The Ori closure struct is stored to an alloca by `build_trampoline()`, and its pointer is passed as the trampoline's `env` argument. This indirection is necessary because the runtime passes a single `env` pointer, but Ori closures are two-word structs.

### TM-7 — Builder State Preservation

Trampoline generation creates a new LLVM function, which changes the builder's insertion point. The builder position and current function SHALL be saved before generation and restored after:
```
saved_pos = builder.save_position()
saved_func = builder.current_function()
// ... generate trampoline ...
builder.restore_position(saved_pos)
builder.set_current_function(saved_func)
```

Failure to restore corrupts the calling function's emission — instructions are emitted into the wrong function.

### TM-8 — Trampoline Verification

Every generated trampoline SHALL be verified via `fn_val.verify(true)` when `verify_arc` is enabled (`ORI_VERIFY_ARC=1`). Verification failure SHALL be logged via `tracing::error!` and recorded via `builder.record_codegen_error()`.

---

## §5 RC Emission

ARC IR RC instructions (`RcInc`, `RcDec`, `IsShared`, `Set`, `Reuse`) are translated to LLVM IR by `ArcIrEmitter`. These rules govern the translation.

### RE-1 — Closure-Aware RC

`RcInc` and `RcDec` on closure types SHALL extract the `env_ptr` (field 1) and operate on it — not on the closure struct itself. The closure struct `{fn_ptr, env_ptr}` is a value type; only the `env_ptr` (which may point to a heap-allocated capture environment) needs RC management.

The `env_ptr` SHALL be null-checked before any RC operation. A null `env_ptr` means the closure has no captures and no heap allocation — RC operations are skipped.

### RE-2 — Scalar Exemption

RC operations SHALL NOT be emitted for scalar types (`ArcClass::Scalar`). Scalar types (int, float, bool, byte, char, Duration, Size, void) have no heap allocation and no reference count. Emitting RC on a scalar is a classification bug (aims-rules.md RE-2 correspondence).

### RE-3 — Drop Function Generation

Per-type drop functions SHALL be generated on demand and cached by mangled type name (`_ori_drop$<mangled_type>`). The drop function:
- For structs: calls field drop functions in field order
- For enums: switches on tag, calls variant-specific drops
- For closures: drops the capture environment (recursive drop of captured values)
- For collections: calls the runtime's buffer cleanup function with the appropriate `elem_dec_fn`

Drop functions SHALL NOT be generated for scalar types (RE-2).

### RE-4 — IsShared Emission

`IsShared` emission depends on the value's representation (`ValueRepr`):

- **`ValueRepr::RcPointer`** (heap-allocated with RC header): emit inline `GEP + load + icmp`:
  1. GEP to the `strong_count` field in the RC header
  2. Load the count
  3. `icmp sgt count, 1` → `i1` result (signed comparison; refcounts are non-negative)

- **Non-pointer representations** (inline aggregates, fat values without RC headers): emit a constant `true` (i.e., "always shared"). This forces the slow path (clone before mutate), which is conservative but correct — there is no RC header to inspect.

The inline GEP+load+icmp path for `RcPointer` avoids function call overhead on the COW hot path. The constant-true fallback for non-pointer values prevents undefined behavior from attempting to read an RC header that does not exist.

See `codegen/arc_emitter/instr_dispatch.rs` for the representation-split emission.

### RE-5 — COW Mutation Pattern

`Set` (field mutation) and `SetTag` (discriminant mutation) SHALL be guarded by `IsShared`:
1. Check `IsShared(obj)` → if shared, clone before mutating
2. GEP to the field/tag
3. Store the new value

The clone-before-mutate path creates a new allocation, copies all fields, decrements the original's RC, and mutates the clone. This is the "copy-on-write" optimization controlled by aims-rules.md DP-9.

### RE-6 — Reuse Emission

`Reuse` instructions SHALL emit a two-path pattern:
- **Fast path**: reuse token valid → reset fields in-place (no allocation)
- **Slow path**: reuse token invalid → `RcDec` the old allocation + fresh `Construct`

The reuse token is an `IsShared` check at the point where the source value's uniqueness is provable. If unique, the memory is reused; if shared, a new allocation is made.

---

## §6 Iterator Emission

Iterator operations are emitted as calls to `ori_rt` iterator functions, with trampolines bridging Ori closures to C-ABI callbacks.

### IT-1 — Iterator Source Creation

`emit_list_iter` SHALL:
1. Extract `data`, `len`, `cap` from the list fat pointer (TR-4)
2. Compute `elem_size` — using `collection_elem_size()` for the **source buffer stride** (storage boundary)
3. Call `ori_iter_from_list(data, len, cap, elem_size)` — 4 parameters, `cap` is third
4. If the list has narrowed int elements (NR-4), wrap with `ori_iter_map(iter, sext_trampoline, null_env, narrowed_elem_size)` — 4 parameters. The sext trampoline writes canonical i64 to `out_ptr`, making downstream elements canonical.

After step 4, the iterator's `elem_size` is ALWAYS canonical — all downstream operations (NR-3) see canonical sizes.

**Note**: `elem_dec_fn` is NOT passed to `ori_iter_from_list`. The runtime retrieves element cleanup information from the V5 RC header at cleanup time.

### IT-2 — Adapter Emission

Iterator adapters are emitted as calls to `ori_iter_*` runtime functions. Each adapter that accepts a closure SHALL build a trampoline (§4) to bridge the closure. The adapter's output `elem_size` is determined by the adapter's result type — always canonical.

Most adapters map 1:1 to runtime functions (`map` → `ori_iter_map`, `filter` → `ori_iter_filter`, etc.). Exceptions:
- `flat_map(f)` is lowered as `map(f)` followed by `flatten` — there is no `ori_iter_flat_map` runtime entry point. Both `emit_iter_flatten()` and `emit_iter_flat_map()` pass `element_store_size(elem_ty)` as `inner_elem_size` to `ori_iter_flatten`, where `elem_ty` is the outer iterator element type (the iterator handle). **Caveat (BUG-04-076)**: for both `flatten` and `flat_map`, when the inner element type differs in size from the iterator handle (e.g., `Iterator<bool>` has 8-byte handle but inner elements are 1 byte), the flatten stage uses the wrong stride. Current tests only exercise same-sized types. See `builtins/iterator.rs`.
- `rev` creates a reversed iterator wrapper via `ori_iter_rev`.

### IT-3 — Consumer Emission

Iterator consumers (`fold`, `collect`, `count`, `any`, `all`, `find`, `for_each`, `join`, `last`, `rfind`, `rfold`) are emitted as calls to `ori_iter_*` runtime functions. Consumers that accept closures SHALL build trampolines.

`collect` SHALL allocate the output list with canonical element sizes. The `elem_size` passed to the runtime's collect function is `element_store_size(result_elem_ty)`, not any narrowed size.

### IT-4 — Iterator Drop

Iterators SHALL be dropped when they go out of scope or when a `break` exits an iteration loop early. The drop is emitted as a call to `ori_iter_drop(iter_handle)`. The runtime's drop function walks the adapter chain and calls `elem_dec_fn` on remaining elements.

Missing iterator drops on early exit cause memory leaks for heap-typed elements. This is enforced by the `IterDrop` protocol builtin in ARC IR (aims-rules.md, protocol builtins table).

### IT-5 — Join-to-Str Trampoline

The `join` consumer requires a special trampoline (`generate_join_to_str_trampoline`) that converts elements to strings before joining. This trampoline calls the element's `to_str` method via the Ori closure protocol, then writes the resulting `str` (24 bytes, indirect) to the output pointer.

---

## §7 Runtime ABI Contract

The codegen layer and the runtime library (`ori_rt`) communicate exclusively through a C-ABI function interface. Both sides must agree exactly on types, sizes, and calling conventions.

### RT-1 — Signature Agreement

Every runtime function declaration in `ori_llvm` SHALL match the corresponding `extern "C"` function in `ori_rt` exactly:
- Parameter count and types
- Return type
- Calling convention (`ccc`)
- Function name (no mangling — `#[no_mangle]`)

A mismatch is a silent ABI violation that may cause SIGSEGV, data corruption, or undefined behavior. Changes to `ori_rt` function signatures MUST update `ori_llvm` declarations in the same commit.

### RT-2 — RC Header Layout (V5)

The RC header is a 32-byte structure preceding heap-allocated data:

| Field | Type | Offset | Purpose |
|-------|------|--------|---------|
| `data_size` | `i64` | 0 | Size of the data payload |
| `elem_dec_fn` | `ptr` | 8 | Element destructor (or null) |
| `elem_count` | `i64` | 16 | Number of elements (for collections) |
| `strong_count` | `i64` | 24 | Reference count |

For non-buffer RC objects, `drop_fn` is stored at `elem_dec_fn` offset (reused field).

Codegen accesses the `strong_count` field via GEP at a fixed offset. Any change to the header layout requires synchronized updates in both `ori_rt` (header definition) and `ori_llvm` (GEP offsets).

### RT-3 — c_char Portability

All C string pointers in both `ori_rt` and `ori_llvm` SHALL use `std::ffi::c_char`, never `i8`. `c_char` is `i8` on x86_64 but `u8` on aarch64 — hardcoding `i8` breaks ARM builds. LLVM's opaque `ptr` type is unaffected (this rule applies to Rust-side type annotations only).

Affected functions: `ori_panic_cstr`, `ori_args_from_argv`, and any future C string FFI.

### RT-4 — elem_dec_fn Contract

The `elem_dec_fn` function pointer stored in RC headers and passed to iterator functions SHALL be:
- **Non-null** for heap-typed elements (str, [T], {K:V}, Set<T>, closures, structs containing heap types)
- **Null** for scalar-typed elements (int, float, bool, byte, char, Duration, Size)

The runtime calls `elem_dec_fn` on each element during collection/iterator cleanup. A null pointer for a heap type causes leaked memory. A non-null pointer for a scalar type causes spurious RC operations on non-RC memory (undefined behavior).

### RT-5 — Iterator State Size Contract

The runtime's `MAX_ELEM_SIZE` constant (in `ori_rt/src/iterator/state.rs`) defines the maximum inline element size for iterator state. Codegen SHALL NOT pass an `elem_size` exceeding `MAX_ELEM_SIZE` to any iterator runtime function. The runtime asserts this at entry (`assert_elem_size`).

Currently `MAX_ELEM_SIZE` is 256 bytes — more than sufficient for any Ori value type (largest is `str` at 24 bytes, but nested aggregates could be larger).

### RT-6 — String/List Representation Agreement

Codegen and runtime SHALL agree on the `{ len: i64, cap: i64, data: ptr }` representation for strings, lists, maps, and sets (TR-4). The field order (len, cap, data) is fixed. GEP indices in codegen must match struct field offsets in the runtime.

---

## §8 LLVM Attributes

AIMS analysis results are surfaced to LLVM as function/parameter attributes, enabling standard optimization passes to exploit ownership and purity proofs.

### AT-1 — Borrowed Parameter Attributes

Borrowed non-scalar parameters (determined by AIMS borrow inference, aims-rules.md §5) SHALL receive LLVM attributes reflecting their read-only, non-aliasing nature:
- `readonly` — the callee does not modify the pointed-to data
- `nonnull` — valid Ori references are never null
- `dereferenceable(N)` — the pointed-to allocation is at least N bytes

These attributes are emitted in `FunctionCompiler` during the declare phase. Source: `codegen/function_compiler/mod.rs`.

### AT-2 — Sret Return Attributes

When a function uses sret return passing (AB-3), the sret parameter SHALL receive:
- `noalias` — the sret pointer does not alias any other argument
- `sret(T)` — marks the parameter as the struct return slot

### AT-3 — Purity Attributes

The purity analysis pass (`codegen/function_compiler/purity_analysis.rs`) SHALL emit:
- `memory(none)` — for functions proven to have no side effects (pure functions)
- `memory(read)` — for functions that only read memory (no writes, no allocations)

These attributes enable LLVM to perform aggressive optimization (CSE, dead store elimination, loop-invariant code motion) on pure Ori functions.

### AT-4 — Nounwind Attribute

Functions proven to never unwind (no panic paths, no invoke instructions) SHALL receive the `nounwind` attribute. This is determined by the nounwind analysis in `codegen/function_compiler/nounwind/`. The `nounwind` attribute enables LLVM to eliminate unnecessary unwind tables and landing pads.

### AT-5 — Relationship to AIMS RL-29/30/31 (Target-System Rules)

AIMS rules RL-29 (`noalias` on fresh returns), RL-30 (`memory(...)` from `EffectSummary`), and RL-31 (alias metadata from `project_alias_sources`) describe the **target-system** attribute export — they are NOT yet shipped.

**The currently shipped attributes (AT-1 through AT-4) derive from simpler analysis, not from the AIMS RL-29/30/31 pipeline:**

| Shipped Rule | Actual Analysis Source | NOT Derived From |
|-------------|----------------------|------------------|
| AT-1 (borrowed param attrs) | Basic borrow inference (`ownership/`) | RL-31 (which requires `project_alias_sources` metadata — unshipped) |
| AT-2 (sret `noalias`) | Sret pointer is a fresh caller alloca | RL-29 (which requires `ReturnContract.preserves_freshness` — partially shipped) |
| AT-3 (purity `memory(...)`) | ARC-IR shape + ABI analysis | RL-30 (which requires full `EffectSummary` — partially shipped) |
| AT-4 (nounwind) | Control-flow analysis (no panic/invoke) | N/A — not an AIMS rule |

**When RL-29/30/31 are fully shipped**, they will provide more precise attribute derivation (e.g., RL-29 will enable `noalias` on return values beyond just sret, RL-30 will derive `memory(...)` from interprocedural effect summaries instead of local ARC-IR shape). At that point, the AT-* rules will be updated to reference AIMS contracts as their analysis source. Until then, the shipped attributes are standalone and should not be conflated with the target-system rules.

---

## §9 Verification

### VR-1 — Per-Function LLVM IR Verification

Every emitted LLVM function SHALL be verified via `fn_val.verify(true)` at the following checkpoints:
- After `emit_arc_function` completes function body emission
- After each trampoline generation (TM-8)
- After derive codegen generates a derived method

When `ORI_VERIFY_ARC=1` is set, verification runs at all checkpoints. In normal mode, verification runs only at the post-emission checkpoint.

### VR-2 — Module-Level Verification

The full LLVM module SHALL be verified before optimization passes run. Module verification catches cross-function issues (e.g., type mismatches in call instructions, undefined global references) that per-function verification misses.

### VR-3 — Post-Optimization Verification

When `ORI_VERIFY_EACH=1` is set, LLVM IR verification SHALL run after every optimization pass. This identifies which specific pass breaks IR well-formedness. ~30-60% slower — diagnostic use only.

### VR-4 — Codegen Audit

When `ORI_AUDIT_CODEGEN=1` is set, the emission layer SHALL perform runtime audit checks:
- RC balance (every `rc_inc` matched by `rc_dec`)
- COW sequencing (`IsShared` before `Set`)
- ABI argument type matching
- Aggregate load sizes (AB-5)
- Safety invariants

`ORI_AUDIT_STRICT=1` enables pessimistic checking (flags borderline cases). `ORI_AUDIT_FUNCTION=name` filters to a specific function.

### VR-5 — Debug/Release Parity

Debug (`cargo b`) and release (`cargo b --release`) builds SHALL produce identical observable output for the same input program. FastISel (debug/JIT) and the full backend (release/AOT) may generate different instruction sequences but MUST produce the same execution results.

Known divergence point: FastISel's aggregate handling (AB-5) — this is the reason for the 16-byte threshold rule.

### VR-6 — Alive2 Translation Validation

Pure functions in the codegen test corpus (`tests/alive2/`) SHALL be verifiable via `alive-tv` (pre-opt → post-opt IR equivalence). The `diagnostics/alive2-verify.sh --corpus` script runs the curated corpus. `--all-codegen` runs the full sweep.

This provides SMT-solver-backed proof that LLVM optimization passes preserve codegen semantics for all possible inputs — not just test cases.

---

## §10 Prior Art Cross-Reference

| System | Relevant Pattern | Ori Correspondence |
|--------|-----------------|-------------------|
| **Swift SILGen** | SIL → LLVM IR emission with ownership annotations | ARC IR → LLVM IR via `ArcIrEmitter` |
| **Swift IRGen** | Type lowering with ABI classification | `resolve_type()` + `FunctionAbi` |
| **Rust codegen_ssa** | Generic LLVM emission layer with ABI computation | Two-pass declare/define in `FunctionCompiler` |
| **Roc LLVM backend** | C ABI compatibility for returning structs (roc#295) | Sret handling (AB-3, AB-4) |
| **Lean 4 EmitLLVM** | RC insertion at IR level, emitted as runtime calls | RC emission (§5) via runtime functions |
| **Koka backend** | FBIP/reuse with C runtime | Reuse emission (RE-6) + runtime contract (§7) |
| **Zig CodeGen** | Self-hosted LLVM backend; sret fix (zig#1450) | Type resolution (§1) + sret (AB-4) |

### Interface with AIMS Rules

| AIMS Rule | Codegen Rule | Interface |
|-----------|-------------|-----------|
| RL-1 (RC inc emission) | RE-1 (closure-aware RC) | AIMS decides WHEN to inc; codegen decides HOW |
| RL-2 (RC dec emission) | RE-1, RE-3 (closure-aware + drop functions) | AIMS decides WHEN to dec; codegen generates the drop tree |
| RL-5 (COW emission) | RE-4, RE-5 (IsShared + COW mutation) | AIMS emits `IsShared`+`Set`; codegen implements the branch |
| RL-10 (reuse emission) | RE-6 (reuse two-path) | AIMS emits `Reuse`; codegen implements fast/slow paths |
| RL-29 (fresh return → `noalias`) | AT-2 (sret — partial) | **Target**: analysis freshness → return noalias. **Shipped**: sret noalias only (caller alloca) |
| RL-30 (effects → `memory(...)`) | AT-3 (purity — partial) | **Target**: EffectSummary → memory attrs. **Shipped**: ARC-IR shape analysis only |
| RL-31 (alias → metadata) | *Not yet shipped* | **Target**: `!alias.scope`/`!noalias` metadata. **Shipped**: no alias metadata emission |
| VF-1 through VF-8 | VR-1 through VR-6 | AIMS verifies analysis; codegen verifies emission |

---

## Appendix A: Element Size Decision Matrix

Which function to call for element sizes, by context:

| Context | Function | Returns |
|---------|----------|---------|
| List buffer read/write | `collection_elem_size(collection_idx, elem_ty)` | Narrowed or canonical |
| Map/set buffer operations | `collection_elem_size(collection_idx, elem_ty)` | Narrowed or canonical |
| Iterator source creation | `collection_elem_size()` for both `ori_iter_from_list` AND the wrapping `ori_iter_map` `in_size` (NR-4 exception) | See IT-1 |
| Trampoline element load/store | `resolve_type(elem_ty)` → LLVM type size | Always canonical |
| Consumer scratch buffer | `element_store_size(elem_ty)` | Always canonical |
| Collect output allocation | `element_store_size(elem_ty)` | Always canonical |
| Runtime function `elem_size` arg | `element_store_size(elem_ty)` | Always canonical (except IT-1 boundary — see above) |
| Struct field | ReprPlan field width | Narrowed (struct-scoped) |
| Local variable | `narrow_local_if_needed()` | Canonical (trunc+sext pair) |

This matrix is the operational summary of NR-1 through NR-6. When uncertain which function to call, consult this table.

## Appendix B: Trampoline Type Decision Table

For each trampoline variant, the element and result type handling:

| Variant | Element ≤16B | Element >16B | Result ≤16B | Result >16B |
|---------|-------------|-------------|-------------|-------------|
| Map | load + pass by value | pass pointer | store result | sret |
| Predicate | load + pass by value | pass pointer | zext i1→i8 | N/A (always i8) |
| ForEach | load + pass by value | pass pointer | N/A (void) | N/A (void) |
| Fold | load elem + load acc | pass ptrs | store result | sret |
