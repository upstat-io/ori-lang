---
section: "05"
title: "Sum Type Payload Extraction"
status: complete
goal: "Match arm payload extraction uses extractvalue (2 instructions) instead of alloca+store+GEP+load (5 instructions)"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/place.rs — uses extractvalue for enum variant fields"
  - "Zig src/codegen.zig — direct field extraction from tagged unions"
depends_on: []
sections:
  - id: "05.1"
    title: "extractvalue for Union Payload Fields"
    status: complete
---

# Section 05: Sum Type Payload Extraction

**Status:** Complete
**Goal:** When destructuring a sum type variant in a match expression, the codegen uses `extractvalue` to access payload fields directly from the SSA value, rather than spilling the entire enum to the stack via alloca+store+GEP+load.

**Context:** Sum types with record payloads use a `{ i64, [N x i64] }` representation where the first i64 is the tag and `[N x i64]` is a union of all variant payloads. When matching and destructuring, the current codegen spills the entire value to an alloca, then uses GEP to index into the payload array. This costs 5 instructions per field extraction where 2 would suffice.

The alloca approach exists because GEP requires a pointer operand, and the codegen was written to handle arbitrary nesting. For flat payload extraction, `extractvalue` is both simpler and more efficient.

Note: `Option<int>` uses flat `{ i64, i64 }` (not `[N x i64]`) and already works efficiently via `extractvalue`. The issue is specific to sum types with `[N x i64]` union representation.

**Journeys affected:** J6 (`_ori_extract`), J11 (`_ori_Shape$eq`).

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/mir/place.rs`: Uses `extractvalue` chains to access enum variant fields.
- **Zig** `src/codegen.zig`: Direct extraction from tagged union values without stack spill.

---

## 05.1 extractvalue for Union Payload Fields

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (primary — `emit_project()` at line 20, the alloca+GEP+load extraction path for enum payloads), `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs` (variant construction via `emit_variant_via_alloca` — not the extraction issue, but related for understanding the full cycle), `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs`.

**Code path confirmed:** `emit_project()` in `instr_dispatch.rs` (lines 47-92) uses `alloca` + `store` + `struct_gep` + `gep` + `load` for general enum payload extraction when `field > 0`. For non-enum types and field 0, it uses `extract_value` directly (line 96-100). The fix targets the `is_general_enum` branch to use `extractvalue` chains instead of the alloca+GEP path.

Replace the alloca+store+GEP+load sequence with extractvalue when destructuring sum type payloads in match arms.

```llvm
; BEFORE (5 instructions per field):
%alloca = alloca { i64, [2 x i64] }
store { i64, [2 x i64] } %enum_val, ptr %alloca
%payload_ptr = getelementptr { i64, [2 x i64] }, ptr %alloca, i64 0, i32 1, i64 0
%field0 = load i64, ptr %payload_ptr

; AFTER (2-3 instructions per field):
%payload = extractvalue { i64, [2 x i64] } %enum_val, 1      ; get [2 x i64] payload
%field0_raw = extractvalue [2 x i64] %payload, 0              ; get raw i64
%field0 = bitcast i64 %field0_raw to double                   ; reinterpret (only if non-i64)
```

**Current code structure in `emit_project()` (lines 28-110 of `instr_dispatch.rs`):**
1. `field > 0` AND `Result` type → alloca + struct_gep + load (reasonable for Result — typed payload fields)
2. `field > 0` AND `Enum` type (general enum) → alloca + store + struct_gep(1) + gep([N x i64]) + load (the 5-instruction path targeted by this fix)
3. `field == 0` OR non-enum type → `extract_value` (already efficient, 1 instruction)
4. Fallback: `struct_gep` + `load` for heap-allocated types (pointer-sourced values)

The fix targets case (2): replace alloca+store+GEP+GEP+load with two `extractvalue` calls. Cases (1), (3), and (4) are already reasonable.

> **TDD requirement:** Write an IR-quality test that asserts the current alloca+store+GEP+load pattern for a 2-field sum type variant BEFORE implementing. Verify the test captures the 5-instruction sequence. Then implement `extractvalue` and verify the test changes to the 2-instruction pattern. Also write a negative test for boxed enum fields (must still use alloca) BEFORE implementing.

- [x] Identify the general enum path (case 2 in `emit_project`, lines 49-80)
- [x] For the `is_general_enum` branch: when the source value is an SSA value (not a pointer), use `extractvalue {tag, [N x i64]} value, 1` to get the payload array, then `extractvalue [N x i64] payload, idx` for each field
- [x] Detect when the source value is an SSA aggregate vs. a pointer (check if the LLVM value's type is a struct type vs. pointer type)
- [x] Keep the alloca path for: (a) boxed enum fields (recursive types — need pointer dereference), (b) pointer-sourced values
- [x] Guardrail: preserve active-variant safety (no reads from inactive payload bytes; keep tag checks authoritative)
- [x] Verify: J6 `_ori_extract` match arms use `extractvalue` instead of alloca
- [x] Verify: J11 `_ori_Shape$eq` derived method uses `extractvalue`
- [x] Verify: SROA is no longer needed to clean up the alloca (it shouldn't exist)

### Edge Cases

- **Boxed enum fields (recursive types):** `emit_project` has a separate `is_boxed_enum_field` path that loads through an RC pointer (line 64-73 in `instr_dispatch.rs`). This path must remain alloca-based — the RC pointer load requires a pointer dereference that `extractvalue` cannot do. Do NOT apply the extractvalue optimization to boxed fields.
- **Result types:** `Result<int, str>` uses a typed field layout `{i64, T_ok, T_err}`, not `[N x i64]`. Result types go through a separate GEP path (line 81-91). Verify this path separately — it may also benefit from `extractvalue` when the value is SSA-sourced.
- **Multi-field variants:** `type Shape = Circle(radius: float) | Rect(width: float, height: float)` — the Rect variant has 2 payload fields at array indices 0 and 1. Both must use `extractvalue [N x i64], idx`.
- **Variant with no payload:** Unit variants (e.g., `None`) have no payload extraction — verify they are not affected by the optimization path.
- **Nested sum types in payloads:** `Option<Result<int, str>>` — the outer Option payload contains a Result. After extracting from Option's payload array, the Result is a full value that may itself need payload extraction.

### 05.1 Completion Checklist

- [x] Match arm payload extraction uses `extractvalue` for SSA values (2-3 instructions per field)
- [x] Alloca path retained for: (1) pointer-sourced values, (2) boxed enum fields (recursive types)
- [x] Result types: verify extractvalue applicability (separate code path from general enums — unchanged)
- [x] J6 `_ori_extract` IR shows `extractvalue` chains, no `alloca` for payload access
- [x] J11 `_ori_Shape$eq` derived method uses `extractvalue` for variant field comparison
- [x] IR test: match arm destructuring a 2-field variant uses exactly 2 `extractvalue` instructions per field
- [x] IR test: boxed (recursive) enum field still uses alloca+GEP+load (correctness guard)
- [x] `compiler/ori_llvm/tests/aot/ir_quality.rs` test for extractvalue payload extraction
- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green
- [x] All AOT tests in `compiler/ori_llvm/tests/aot/` pass

---

## Section 05 Exit Criteria

IR dump of J6's match expression shows `extractvalue` chains with no `alloca` for payload access. Instruction count per match arm reduced from ~5 to ~2 per field.

**Implementation details:**
- `emit_project()` fast path: checks `is_struct_value(val) && is_single_slot_type(result_ty) && !is_boxed_enum_field(...)`, then uses `extract_value` → `extract_value_any` → `reinterpret_from_i64` chain.
- Derive codegen (`enum_eq.rs`): when all variant fields are single-slot, uses `extractvalue` chains instead of alloca+store+GEP+load for field comparison.
- New IrBuilder methods: `is_single_slot_type()`, `is_struct_value()`, `reinterpret_from_i64()`, `extract_value_any()` (made public).
- Multi-word fields (str, nested structs) and boxed/recursive fields keep the alloca path.
- Type reinterpretation: `i64→i64` (identity), `i64→double` (bitcast), `i64→i1/i8/i32` (trunc), `i64→ptr` (inttoptr).
