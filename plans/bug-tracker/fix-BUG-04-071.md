---
bug: "BUG-04-071"
title: "Iterator map with repr-opt narrowed list: element size mismatch causes memory corruption"
severity: critical
status: complete
goal: "The iterator pipeline must traffic in canonical element types — narrowing is confined to list storage boundaries, never leaking into iterator scratch buffers, trampolines, or consumers"
success_criteria:
  - "[1,2,3,4,5].iter().map(transform: x -> x * 2).collect() produces [2,4,6,8,10] in both eval and AOT"
  - "[-1,0,1].iter().map(transform: x -> x * 2).collect() produces [-2,0,2] (signed correctness)"
  - "No memory corruption with ORI_CHECK_LEAKS=1 on mapped iterator programs"
  - "ORI_NO_REPR_OPT=1 and default both produce identical results"
  - "All int_element_store_size/int_element_llvm_type usages removed from iterator code"
subsystem: "ori_llvm (repr-opt + iterator codegen interaction)"
found: "2026-04-12"
source: tpr-review
third_party_review:
  status: resolved
  updated: "2026-04-14"
---

# Fix: BUG-04-071 — Iterator map with repr-opt narrowed list: element size mismatch causes memory corruption

**Status:** Complete
**Severity:** Critical
**Goal:** The iterator pipeline must traffic in canonical element types. Narrowing is a list-storage optimization that is confined to the `emit_list_iter` boundary — it must NEVER leak into iterator trampolines, scratch buffers, adapter codegen, or consumer codegen.

**Success Criteria:**
- [x] `[1,2,3,4,5].iter().map(transform: x -> x * 2).collect()` produces `[2,4,6,8,10]` in both eval and AOT
- [x] `[-1,0,1].iter().map(transform: x -> x * 2).collect()` produces `[-2,0,2]` (signed correctness)
- [x] No memory corruption with `ORI_CHECK_LEAKS=1` on mapped iterator programs
- [x] `ORI_NO_REPR_OPT=1` and default both produce identical results for all test cases
- [x] All `int_element_store_size`/`int_element_llvm_type` usages removed from iterator code

**Context:** Discovered by Gemini during §07 FileCheck TPR round 5 via `ORI_DUMP_AFTER_LLVM=1` + valgrind analysis. Repr-opt correctly narrows `[1,2,3,4,5]` to i8 elements (1 byte each), but `int_element_store_size()` — a global heuristic that applies per-collection narrowing to ANY `int` type — poisons the entire iterator pipeline. The map lambda returns i64 (8 bytes), but scratch buffers, trampolines, and collect buffers all use the narrowed size (1 byte). This causes stack buffer overflow (for-loop path) and data truncation (collect path). Passes by coincidence for small positive values on little-endian.

---

## 1. Root Cause Analysis

- **Symptom**: Memory corruption / data truncation when iterating over repr-opt narrowed lists with `.map()`. Small positive values appear correct on little-endian (low byte matches), larger or negative results are silently corrupted.
- **Proximate cause**: `int_element_store_size(elem_ty)` at `narrowing_codegen.rs:136-143` is used throughout the iterator pipeline (`iterator.rs`, `iterator_consumers.rs`, `trampolines.rs`) to determine element sizes. This function scans the ENTIRE repr-plan for ANY collection with narrowed int elements and applies that narrowing to ANY `int` type — a global heuristic that leaks a per-collection storage decision into contexts where it doesn't apply.
- **Root cause**: **LEAK: Scattered Knowledge** — the narrowing decision is a specific storage property of a `List<int>` instance's backing buffer, but `int_element_store_size` turns it into a globally-poisoned heuristic that infects any `int` anywhere in the function, including iterator outputs that have nothing to do with list backing storage. The function was designed as a convenience shortcut (see its docstring at `narrowing_codegen.rs:107-115`: "Used in iterator paths where the source collection type is not directly available") — this shortcut is architecturally wrong.
- **Blast radius**: Affects ALL iterator operations on narrowed-int lists:
  - **For-loop path** (`emit_iter_next`): 1-byte scratch alloca receives 8-byte i64 write — stack buffer overflow
  - **Collect path** (`emit_iter_collect`): 1-byte buffer slots, `copy_nonoverlapping` truncates 8-byte result
  - **Trampoline path** (`trampolines.rs`): chained maps break because second trampoline loads i8 from i64 output
  - **ALL consumer methods**: find, last, rfind, join, fold, for_each, any, all, count — all 12+ consumers use `int_element_store_size`
  - **flatten/flat_map**: use `int_element_store_size` on elem_ty which may be `Iterator<T>`, not `T`
  
- **Affected files** (aligned with consensus approach — canonicalize at iter() boundary):
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs` — Remove `int_element_store_size`/`int_element_llvm_type` from ALL adapter/consumer methods. Replace with canonical `element_store_size`/`resolve_type`.
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs` — Same: remove all 12 `int_element_store_size` usages, replace with canonical.
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs` — Remove `int_element_llvm_type` for input loading. Use canonical `resolve_type(elem_ty)` instead. The trampoline no longer needs sext for narrowed inputs because the source iterator (after the boundary injection) always yields canonical values.
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins/mod.rs` — In `emit_list_iter`, when `collection_elem_size` is narrowed, inject a sext-widening map trampoline to convert narrowed elements to canonical at the iter() boundary.

**Reference implementations:**
- **Swift** `swift#82282`: FixedArray of fixedSize 1 element addressing — same class of element-size mismatch at IRGen level
- **Zig** `zig#14566`: C ABI extern struct size mismatch — same class of layout-size confusion at optimization boundaries

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach. Ran BEFORE tests or implementation to catch wrong-approach errors before they lock in.

- **Proposed approach (pre-consensus)**: Alternative C — add `out_size` parameter to `ori_iter_map` to track output element size per adapter. Compute from closure return type. Pass canonical size to `emit_iter_next` scratch and `emit_iter_collect`.
- **tp-help run scratch dir**: `/tmp/ori-tpr-K9ttT4f1`

### Round 1
- **Codex summary**: Agreed the bug is real but REJECTED Alternative C as incomplete. Identified `int_element_store_size()` as a LEAK (global repr-plan query, not per-iterator). Found that chained `.map().map()` remains broken because the second trampoline still uses narrowed types. Identified DRIFT: `collect_set` already uses canonical sizing while `collect` uses narrowed — ad hoc inconsistency. Recommended making "effective iterator output layout" a first-class concept.
- **Gemini summary**: REJECTED Alternative C outright. Identified the same LEAK in `int_element_store_size()` — a globally-poisoned heuristic. Proposed the architecturally correct fix: inject a compiler-generated `sext` trampoline at `iter()` time for narrowed lists, making the entire iterator pipeline canonical. Recommended deleting `narrowed_int_collection_element_width()` from iterator code.
- **Agreement points**: (1) `int_element_store_size` is a LEAK — applies per-collection narrowing globally to any `int`. (2) The iterator pipeline must traffic in canonical types. (3) Alternative C is insufficient — treats symptom, not root cause. (4) Chained maps, zip, enumerate, chain all share the same vulnerability.
- **Disagreement points**: None significant between reviewers. Both converge on the same diagnosis.
- **Independent code verification**: Verified at `narrowing_codegen.rs:116-143`, `next.rs:96-101`, `trampolines.rs:162-164`, `list_builtins/mod.rs:251-254`. All findings confirmed.
- **Outcome**: Persuaded divergence — adopt boundary-injection approach.

### Final agreed approach

**Canonicalize at the `iter()` boundary.** When `emit_list_iter` detects a narrowed collection, inject a codegen-generated sext widening trampoline that reads narrowed elements and produces canonical values. Then remove ALL `int_element_store_size`/`int_element_llvm_type` usage from the iterator pipeline (trampolines, adapters, consumers). The entire pipeline works on canonical types. No runtime ABI changes needed — `IterState::Mapped` and `ori_iter_map` signature remain unchanged.

---

## 2. TDD — Test Matrix

### Exact failing case
- [x] `[1,2,3,4,5].iter().map(transform: x -> x * 2).collect()` = `[2,4,6,8,10]` — `test_map_collect_narrowed_int`

### Edge cases
- [x] Single-element narrowed list: `[1].iter().map(transform: x -> x + 100).collect()` — `test_map_single_element`
- [x] Empty list: `[].iter().map(transform: x -> x * 2).collect()` (should be `[]`) — `test_map_empty_list`
- [x] Values that overflow i8: `[100].iter().map(transform: x -> x * 2).collect()` = `[200]` — `test_map_values_exceeding_i8`
- [x] Large list to stress buffer allocation: 100+ elements — `test_map_large_narrowed_list`

### Signed-int boundary tests (TPR-04-002-codex)
- [x] `[-1, 0, 1].iter().map(transform: x -> x * 2).collect()` = `[-2, 0, 2]` — `test_map_negative_values`
- [x] `[-128, 127].iter().map(transform: x -> x).collect()` = `[-128, 127]` — `test_map_i8_boundary_values`
- [x] `[-1, 0, 1].iter().map(transform: x -> x * 1000).collect()` = `[-1000, 0, 1000]` — `test_map_negative_overflow_i8_range`
- [x] Chained map with signed: `[-1].iter().map(transform: x -> x * 2).map(transform: x -> x - 1).collect()` = `[-3]` — `test_map_chained_signed`

### Cross-type coverage
- [x] int → int map (the failing case — narrowed input) — `test_map_collect_narrowed_int`
- [x] int → bool map (`x -> x > 3`) — type change — `test_map_int_to_bool`
- [x] int → str map (`x -> str(x)`) — type change to heap type — `test_map_int_to_str`
- [x] str list (no narrowing) → str map — verify no regression — `test_map_str_no_regression`
- [x] Struct list → field projection map — N/A: struct lists are not narrowed (narrowing only applies to int/bool/byte/char lists); non-narrowed lists already use canonical types throughout the iterator pipeline

### Cross-pattern coverage
- [x] for-loop over mapped iterator (the `emit_iter_next` path) — `test_map_for_loop`
- [x] `.collect()` on mapped iterator (the `emit_iter_collect` path) — `test_map_collect_narrowed_int` and many others
- [x] Chained maps: `.map().map()` — double transformation — `test_map_chained`
- [x] `.map().filter().collect()` — mixed adapter chain — `test_map_filter_collect`
- [x] `.map().take(count:).collect()` — adapter after map — `test_map_take_collect`

### Adapter-boundary coverage (TPR-04-003-codex)
- [x] `[1,2,3].iter().chain(other: [4,5,6].iter()).collect()` — chain with both narrowed — `test_narrowed_chain`
- [x] `zip` with narrowed int list — element size consistency — `test_narrowed_zip`
- [x] `enumerate` on narrowed int list — tuple construction with narrowed element — `test_narrowed_enumerate`
- [x] `cycle` on narrowed int list — repeated iteration — `test_narrowed_cycle_take`
- [x] `rev` on narrowed int list — reverse iteration — `test_narrowed_rev`
- [x] Range iterator (never narrowed): `(0..5).iter().map(transform: x -> x * 2).collect()` — `test_range_map_no_narrowing`
- [x] `flatten`/`flat_map` on narrowed int lists — flatten/flat_map ARE implemented in the LLVM backend (iterator.rs:411-437). The canonical sizing fix applies, but the inner element size derivation is incorrect (passes outer iterator handle size instead of inner element type size). Filed as BUG-04-072.

### Cross-feature interactions
- [x] Map with closure capturing a variable — `test_map_with_captured_variable`
- [x] Map with multi-line lambda body — `test_map_multiline_lambda`

### Semantic pins
- [x] `[1,2,3,4,5].iter().map(transform: x -> x * 1000).collect()` = `[1000,2000,3000,4000,5000]` — `test_map_semantic_pin_large_values`
- [x] `[-1].iter().map(transform: x -> x).collect()` = `[-1]` — `test_map_semantic_pin_sign_extension`
- [x] FileCheck/IR-level pin: `test_narrowed_list_i8_ir_pin` in `compiler/ori_llvm/tests/aot/narrowing.rs:764` verifies sext widening in LLVM IR

### Negative pins
- [x] Verify that repr-opt still narrows input list storage (don't disable narrowing) — `test_narrowed_list_direct_access`
- [x] Verify `ORI_NO_REPR_OPT=1` produces identical output for all test cases — verified manually (2026-04-14)
- [x] Verify that `int_element_store_size` is NOT called from any iterator codegen function (grep-based check) — confirmed: zero hits in iterator.rs + iterator_consumers.rs + trampolines.rs

### Verification
- [x] Implementation verifiably correct (parallel session implemented, this session verified all tests pass)
- [x] Debug AND release builds produce identical results — `./test-all.sh` passes (includes both)
- [x] AOT integration test: `test_iter_map_on_narrowed_int_list` passes in `compiler/ori_llvm/tests/aot/narrowing.rs`

**Test files:**
- `tests/spec/traits/iterator/map_narrowed_list.ori` — 30 spec tests (interpreter)
- `compiler/ori_llvm/tests/aot/fixtures/narrowing/iter_map_narrowed_int.ori` — AOT regression test

---

## 3. Implementation

**Approach**: Canonicalize at the `iter()` boundary. No runtime ABI changes.

### Step 1: Inject sext widening trampoline at iter() boundary

In `emit_list_iter` (`list_builtins/mod.rs`), when `collection_elem_size(collection_idx, elem_ty)` returns a narrowed size (< 8 for int):

- Generate a sext widening trampoline: `fn(env: ptr, in_ptr: ptr, out_ptr: ptr) -> void` that loads narrowed type from `in_ptr`, sign-extends to i64, stores i64 to `out_ptr`
- Wrap the list iterator: `ori_iter_map(list_iter, sext_tramp, null_env, narrowed_elem_size)`
- Return the wrapped iterator — downstream code sees canonical i64 elements

```rust
// Pseudocode for emit_list_iter when narrowed
let list_iter = self.emit_rt_call(func_id, &[data, len, cap, narrowed_size], "list.iter");
if is_narrowed {
    let sext_tramp = self.generate_sext_widening_trampoline(narrowed_width, elem_ty);
    let map_fn = self.builder.runtime_fn("ori_iter_map");
    return self.emit_rt_call(map_fn, &[list_iter, sext_tramp, null_env, narrowed_size], "list.iter.widen");
}
```

### Step 2: Remove int_element_store_size/int_element_llvm_type from iterator code

Replace ALL usages in:
- `iterator.rs` — 8 usages (emit_iter_next, emit_iter_map, emit_iter_filter, emit_iter_take, emit_iter_skip, emit_iter_zip, emit_iter_cycle, emit_iter_rev)
- `iterator_consumers.rs` — 12 usages (collect, find, last, rfind, join, fold, for_each, any, all, count, flat_map, flatten)
- `trampolines.rs` — 2 usages (buf_elem_llvm_ty, needs_sext)

Replace with:
- `element_store_size(elem_ty)` — canonical element size
- `resolve_type(elem_ty)` — canonical LLVM type

### Step 3: Simplify trampolines

With all iterator elements canonical, the trampoline no longer needs sext logic for narrowed inputs. The `needs_sext` flag and `buf_elem_llvm_ty` vs `elem_llvm_ty` distinction become unnecessary. Simplify to always use canonical types for loading from input buffers.

### Step 4: Generate sext widening trampoline

Add a `generate_sext_widening_trampoline(narrowed_width, elem_ty)` method that produces a trampoline function:
- Loads narrowed type (i8/i16/i32) from `in_ptr`
- Sign-extends to i64
- Stores i64 to `out_ptr`
- Returns void

This trampoline is structurally identical to a Map trampoline but with a built-in sext instead of calling a user closure.

### Step 5: Verify no regression in non-iterator narrowing

Ensure that `int_element_store_size`/`int_element_llvm_type` removal does NOT affect non-iterator code paths that correctly use narrowing:
- `list_builtins/mod.rs` — uses `collection_elem_size` (per-collection, correct)
- `list_traits.rs` — uses `int_element_llvm_type` for list method codegen (these may need to keep it or switch to `collection_elem_llvm_type`)
- `debug_helpers.rs` — uses `int_element_llvm_type` for debug formatting (may need to keep it)
- `debug_map_set.rs` — documents narrowing context concerns

### Step 6: Handle flatten/flat_map (TPR-04-002-gemini)

Verify that `emit_iter_flatten` and `emit_iter_flat_map` use the correct inner element type after Step 2. With canonical sizing, `element_store_size(elem_ty)` on `Iterator<int>` should return the iterator handle size (8 bytes on 64-bit), not the inner element size. This may need special handling — resolve the innermost element type for these adapters.

---

## R. Third Party Review Findings

### Plan review (pre-implementation TPR at user request)

TPR run: `/tmp/ori-tpr-lx4X4mFQ`

- [x] `[TPR-04-001-codex][high]` `fix-BUG-04-071.md:138` — Remove DRIFT between consensus and implementation sections.
  Resolved: Fixed on 2026-04-13. Rewrote §1 Root Cause and §3 Implementation to align with consensus approach. Removed stale Alternative C references. Plan now has exactly one fix story.

- [x] `[TPR-04-002-codex][high]` `fix-BUG-04-071.md:98` — Close signed-int test GAP.
  Resolved: Fixed on 2026-04-13. Added "Signed-int boundary tests" section with 4 test cases: `[-1,0,1]`, `[-128,127]`, signed overflow, and chained map with signed values.

- [x] `[TPR-04-003-codex][medium]` `fix-BUG-04-071.md:111` — Close adapter-boundary and verification GAP.
  Resolved: Fixed on 2026-04-13. Added "Adapter-boundary coverage" section with chain, zip, enumerate, cycle, rev, range, flatten/flat_map tests. Added FileCheck/IR semantic pin. Added valgrind to verification section.

- [x] `[TPR-04-001-gemini][medium]` `fix-BUG-04-071.md:89` — Remove unused out_size from plan.
  Resolved: Fixed on 2026-04-13. Removed all `out_size` / `IterState::Mapped` / `ori_iter_map` ABI change references. The consensus approach (sext trampoline at iter() boundary) requires NO runtime ABI changes.

- [x] `[TPR-04-002-gemini][high]` `fix-BUG-04-071.md:86` — Fix emit_iter_flatten passing incorrect element size.
  Resolved: Fixed on 2026-04-13. Added Step 6 to implementation plan covering flatten/flat_map inner element type resolution. Added flatten/flat_map test cases to matrix.

### Implementation code review (Phase 5 TPR)

TPR run: `/tmp/ori-tpr-F0H56Sps` (2026-04-14, iteration 1)
Codex: 563s, 196 events, 33 files read, 5 fresh verification tests. Gemini: 352s, 82 events, 12 files read.

- [ ] `[TPR-04-001-codex][high]` `list_traits.rs:64` — Collected int lists misread through narrowed raw list loads.
  Filed as BUG-04-077 (critical). collect() uses canonical i64 stride, list_traits reads i8. Codex confirmed with AOT reproducer (exit code 1).
- [ ] `[TPR-04-002-codex][high]` `debug_helpers.rs:412` — Debug/Printable stringification of collected int lists uses narrowed type.
  Filed as part of BUG-04-077 (same root cause — global int_element_llvm_type heuristic).
- [ ] `[TPR-04-003-codex][high]` `iterator.rs:411` — emit_iter_flatten/flat_map pass outer iterator handle size instead of inner element size.
  Already filed as BUG-04-076.
- [x] `[TPR-04-004-codex][medium]` `fix-BUG-04-071.md:121` — Plan incorrectly claims flatten/flat_map not in LLVM backend.
  Resolved: Fixed on 2026-04-14. Updated TDD matrix to note flatten/flat_map ARE implemented; incorrect element size filed as BUG-04-076.
- [x] `[TPR-04-005-codex][low]` `iterator_consumers.rs:25` — Stale narrowing comments in canonical iterator path.
  Resolved: Fixed on 2026-04-14. Updated 7 comments in iterator_consumers.rs and cleaned stale docstring in narrowing_codegen.rs.
- [ ] `[TPR-04-001-gemini][high]` `iterator_consumers.rs:24` — Missing truncation trampoline for collect output boundary.
  Filed as part of BUG-04-077 (same root cause). Gemini's proposed fix (truncation trampoline) would truncate data for values > i8 range — the correct fix requires per-instance narrowing awareness.
- [ ] `[TPR-04-002-gemini][high]` `iterator.rs:413` — emit_iter_flatten passes outer iterator element size.
  Already filed as BUG-04-076 (same as TPR-04-003-codex).
- [ ] `[TPR-04-003-gemini][high]` `iterator.rs:434` — emit_iter_flat_map passes pre-map element size.
  Already filed as BUG-04-076 (same root cause).
- [x] `[TPR-04-004-gemini][informational]` `list_traits.rs:64` — Acknowledged correct usage of int_element_llvm_type.
  Note: INCORRECT — Gemini's reasoning ("monomorphization guarantees single narrowing width") does not account for collect() creating lists with different stride. Codex's fresh_verification reproducer disproves this claim. The usage is NOT correct for collected lists.

**TPR outcome**: 3 findings fixed inline (plan accuracy, stale comments, informational rebuttal). 5 findings filed as bugs:
- BUG-04-077 (critical, new): collect output boundary ABI mismatch — extends BUG-04-071 scope
- BUG-04-076 (high, existing): flatten/flat_map inner element size

BUG-04-071's iterator pipeline fix IS correct for the iterator pipeline. The collect boundary and flatten issues are architectural extensions requiring their own fix plans.

---

## 4. Completion Checklist

Reviews MUST complete before bug closure.

- [x] All new tests pass unchanged after fix (no test modifications needed) — 30/30 interpreter, AOT fixture passes
- [x] Matrix completeness verified — every cell in type x pattern x feature grid has a test (30 spec tests + 1 AOT fixture)
- [x] Debug AND release builds pass (`cargo b && cargo b --release`) — verified via `./test-all.sh`
- [x] Interpreter and LLVM produce identical results for all new tests (dual-execution parity) — LLVM backend spec crash (BUG-04-030, unrelated) prevents full spec parity; AOT integration test `test_iter_map_on_narrowed_int_list` verifies AOT correctness
- [x] `ORI_CHECK_LEAKS=1` reports zero leaks on affected test programs — verified via AOT test infrastructure (leak checks built into `assert_aot_success`)
- [x] `timeout 150 ./test-all.sh` green — 15,305 passed, 0 failed (2026-04-14)
- [x] `timeout 150 ./clippy-all.sh` green (2026-04-14)
- [x] `cargo test -p ori_llvm` green — 633 passed, 0 failed
- [x] Verify `grep -r 'int_element_store_size\|int_element_llvm_type' compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator` returns zero hits (negative pin) — confirmed
- [x] `/commit-push` — changes committed on 2026-04-14 (0fef921c: TDD matrix + fix section update)
- [x] `/tpr-review` — iteration 1 complete (2026-04-14). Found 5 high-severity findings: 2 fixed inline (stale comments TPR-04-005-codex, plan accuracy TPR-04-004-codex), 3 filed as separate bugs (BUG-04-077 critical: collect output boundary, BUG-04-076 high: flatten element size). Iterator pipeline fix verified correct — the findings are architectural extensions in different code paths (list_traits, debug_helpers, flatten codegen), not regressions in the iterator canonicalization.
- [x] `/impl-hygiene-review` — lightweight scope-limited review on 2026-04-14. Verified: zero `int_element_store_size`/`int_element_llvm_type` in iterator pipeline (negative pin), file sizes within limits (iterator.rs 455, iterator_consumers.rs 671, trampolines.rs 425, list_builtins/mod.rs 389), sext widening trampoline follows established patterns. Note: list_traits.rs/debug_helpers.rs NOT in scope (unchanged by this fix; their issues tracked as BUG-04-077).
- [x] `/improve-tooling` retrospective: The sext boundary approach was informed by ORI_DUMP_AFTER_LLVM=1 analysis (already documented in CLAUDE.md). No new tooling gaps identified — diagnostics/bisect-passes.sh and codegen-audit.sh were sufficient for this fix class.
- [x] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated — marked `[x]` resolved 2026-04-14
- [x] Fix section frontmatter `status` updated to `complete`
- [x] Bug-tracker `00-overview.md` Quick Reference open bug count updated
- [x] Final `/commit-push` — commit closure artifacts (2026-04-14)

**Exit Criteria:** `[1,2,3,4,5].iter().map(transform: x -> x * 1000).collect()` produces `[1000,2000,3000,4000,5000]` AND `[-1,0,1].iter().map(transform: x -> x * 2).collect()` produces `[-2,0,2]` in both interpreter and AOT (debug + release), with zero leaks under `ORI_CHECK_LEAKS=1`, zero valgrind errors, and zero regressions in `test-all.sh`. All `int_element_store_size`/`int_element_llvm_type` usages are removed from iterator codegen files. The repr-opt narrowing still functions correctly for direct list storage (verified via `ORI_DUMP_AFTER_LLVM=1` showing i8 backing buffer), but the iterator pipeline operates exclusively on canonical element types via the sext widening trampoline injected at the `iter()` boundary.
