---
section: "05"
title: "Sum Type Payload Extraction"
status: not-started
goal: "Match arm payload extraction uses extractvalue (2 instructions) instead of alloca+store+GEP+load (5 instructions)"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/place.rs — uses extractvalue for enum variant fields"
  - "Zig src/codegen.zig — direct field extraction from tagged unions"
depends_on: []
sections:
  - id: "05.1"
    title: "extractvalue for Union Payload Fields"
    status: not-started
  - id: "05.2"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Sum Type Payload Extraction

**Status:** Not Started
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

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs` (variant construction includes `emit_variant_via_alloca`), `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs`, and the match arm destructuring codepath (likely `Project` instruction handling).

**Note:** `emit_variant_via_alloca` is for **constructing** variant values (`{ tag, [N x i64] }`). The extraction issue is **destructuring** — accessing payload fields from an existing variant value during match arms. These may be different code paths. Verify both paths before patching.

Replace the alloca+store+GEP+load sequence with extractvalue when destructuring sum type payloads in match arms.

```llvm
; CURRENT (5 instructions per field):
%alloca = alloca { i64, [2 x i64] }
store { i64, [2 x i64] } %enum_val, ptr %alloca
%payload_ptr = getelementptr { i64, [2 x i64] }, ptr %alloca, i64 0, i32 1, i64 0
%field0 = load i64, ptr %payload_ptr

; TARGET (2 instructions per field):
%payload = extractvalue { i64, [2 x i64] } %enum_val, 1      ; get [2 x i64] payload
%field0 = extractvalue [2 x i64] %payload, 0                  ; get first field
```

- [ ] Identify the match arm codegen path that creates allocas for sum type destructuring
- [ ] Detect when the source value is an SSA value (not a pointer) — use extractvalue path
- [ ] Implement extractvalue chain: first extract the `[N x i64]` payload array, then extract individual fields
- [ ] Keep the alloca path as fallback for cases where the value is already behind a pointer
- [ ] Guardrail: preserve active-variant safety (no reads from inactive payload bytes; keep tag checks authoritative)
- [ ] Verify: J6 `_ori_extract` match arms use `extractvalue` instead of alloca
- [ ] Verify: J11 `_ori_Shape$eq` derived method uses `extractvalue`
- [ ] Verify: SROA is no longer needed to clean up the alloca (it shouldn't exist)

---

## 05.2 Completion Checklist

- [ ] Match arm payload extraction uses `extractvalue` for SSA values
- [ ] Alloca path retained only for pointer-sourced values
- [ ] J6 `_ori_extract` emits 2 instructions per field (not 5)
- [ ] J11 derived `$eq` for enums uses `extractvalue`
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] All AOT tests in `compiler/ori_llvm/tests/aot/` pass

**Exit Criteria:** IR dump of J6's match expression shows `extractvalue` chains with no `alloca` for payload access. Instruction count per match arm reduced from ~5 to ~2 per field.
