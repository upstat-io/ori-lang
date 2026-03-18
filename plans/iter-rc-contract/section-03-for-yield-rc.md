---
section: "03"
title: "Fix For-Yield RC Scoping"
status: not-started
goal: "Eliminate the spurious extra RcDec on the source collection in for-yield by properly scoping the collection variable so it dies at the Jump to header, matching the for-do pattern"
third_party_review: false
depends_on:
  - "01"
  - "02"
sections:
  - id: "03.1"
    title: "Failed Approaches (Reference)"
    status: not-started
  - id: "03.2"
    title: "Correct Approach: Scope Isolation + Block Param Threading"
    status: not-started
  - id: "03.3"
    title: "AIMS Interaction Verification"
    status: not-started
  - id: "03.4"
    title: "For-Yield RC Balance Tests"
    status: not-started
  - id: "03.5"
    title: "For-Yield break/continue Support"
    status: not-started
---

# Section 03: Fix For-Yield RC Scoping

**Status:** Not Started
**Goal:** Fix the for-yield lowering so the source collection's RC is correctly balanced: 1 alloc + 1 inc (for the iterator) = 2 refs, and exactly 2 decs (one from `ori_iter_drop`, one from the AIMS pipeline). Currently, the AIMS pipeline emits 3 decs because the original collection variable is still visible in the post-loop scope.

**Context:** This is the critical path fix. Section 02 (elem_dec_fn) ensures element cleanup works correctly regardless of which dec reaches zero. This section ensures the correct NUMBER of decs. Without this fix, the double-free occurs even with the correct `elem_dec_fn`.

**Design reference**: For-do's `__for_coll` phantom mechanism (`compiler/ori_arc/src/lower/control_flow/loops.rs:174-181`) works because it binds the collection as a mutable variable (via `scope.bind_mutable()` at line 180, only for `List | Set` tags -- line 174) BEFORE `.iter()`, then the loop infrastructure threads it through header/body/latch/exit as a block parameter. The exit block's `pre_scope` restore + param rebinding (`for_iterator.rs:206-209`) handles scope cleanup. The original variable's last use becomes the Jump to header, and the AIMS backward analysis sees only the block-param copy in the loop, emitting exactly one dec (from the dummy let in the exit block at `for_iterator.rs:196-204`).

---

## 03.1 Failed Approaches (Reference)

This subsection documents approaches that were attempted or considered and rejected, to prevent re-discovery of the same dead ends.

### (a) Broad iter_element_defs Suppression

**Approach**: Expand the `iter_element_defs` set (defined in `compiler/ori_arc/src/aims/emit_rc/helpers.rs:197`, checked at `realize/walk.rs:343-345`) to include the source collection variable, suppressing `emit_defined_dead`'s RcDec for it.

**Why it fails**: `iter_element_defs` is designed for iterator-ELEMENT projections (variables created by `Project(iter_next_result, 1)`) -- elements borrowed from the buffer. The source collection is not an element; it is the OWNER of the buffer. Suppressing its dec removes a legitimate cleanup, causing a leak on the non-for-yield path.

### (b) Direct Dummy Reference in Exit Block

**Approach**: Emit a dummy `Let { Var(coll_param) }` in the exit block after `ori_iter_drop` -- already implemented in current code (`for_yield.rs:337-341`).

**Why it fails**: The dummy reference keeps the block-param copy alive, but the original variable (`iter_val` in the enclosing scope) is a DIFFERENT `ArcVarId`. The AIMS backward analysis tracks RC by `ArcVarId`, not by name or by "what underlying allocation". The original variable still appears as "defined but unused" in post-loop blocks, and `emit_defined_dead` emits an extra dec for it.

### (c) Scope Shadowing

**Approach**: After emitting the `.iter()` call, rebind the original variable's name in the scope to a fresh sentinel `ArcVarId` (e.g., `ArcVarId::INVALID` or a new scalar let), so the AIMS analysis doesn't see the original variable as needing cleanup.

**Why it fails**: The AIMS backward analysis operates on `ArcVarId`s found in the IR, not on scope names. Even if the scope is rebound, the original `ArcVarId` still exists as a defined variable in the ARC IR (it was defined before the for-yield expression). The analysis will still find it and emit a dec. Additionally, this approach risks breaking name resolution for variables that shadow the collection name inside the loop body.

### (d) Phantom Threading Without Scope Isolation

**Approach**: Thread `coll_var` through the header block as a block param (current implementation in `for_yield.rs:250-255`) but without modifying the enclosing scope to remove the original variable.

**Why it fails**: This is the current implementation. It creates the block-param copy correctly and threads it through the loop, but the original variable remains in scope. The AIMS analysis sees two "owners" of the same allocation: the original variable (which it wants to dec at its last use) and the block-param copy (which gets a dec via the dummy let). This produces one extra dec.

- [ ] Document each failed approach with the specific `ArcVarId` values and block indices from a real ARC IR dump (use `ORI_DUMP_AFTER_ARC=1`)
- [ ] For each approach, identify the exact AIMS rule (emit_defined_dead, emit_last_use_decs, or edge_cleanup) that produces the incorrect dec
- [ ] Confirm that none of the failed approaches are still partially implemented in the codebase (search for dead code from reverted attempts)

---

## 03.2 Correct Approach: Scope Isolation + Block Param Threading

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_yield.rs` (prepare_iterator, lower_for_yield_iterator)

The correct fix must achieve parity with the for-do path's `__for_coll` mechanism. The key insight is that in for-do, the original collection variable's LAST USE is the Jump to the header block (where it is passed as a Jump argument). After that Jump, the variable is never referenced again -- only the header block param is used. This means the AIMS backward analysis sees the original variable as "consumed at Jump" and does NOT emit a separate dec for it.

**Implementation approach**:

1. **In `prepare_iterator()`** (for List/Set collections): Return `coll_var = Some(iter_val)` as currently done. No change needed here.

2. **In `lower_for_yield_iterator()`**: The original variable must be consumed by the Jump to header. Currently, line 254 builds entry args:
   ```rust
   let entry_args: Vec<_> = coll_var.into_iter().collect();
   self.builder.terminate_jump(header_block, entry_args);
   ```
   This correctly passes the original variable as a Jump argument. The original variable's last use IS this Jump.

3. **Scope modification**: After the Jump to header, the for-yield should NOT have the original collection variable visible in any subsequent block. The header block param (`coll_param`) takes over. The body and exit blocks should reference only `coll_param`, not the original `iter_val`. Currently, the for-yield exit block references `coll_param` (line 337-341), which is correct. But if any AIMS analysis considers the original variable's scope to extend beyond the for-yield expression, the extra dec appears.

4. **The real fix**: Ensure the AIMS backward analysis correctly identifies the original variable's last use as the Jump to header. This may require:
   - Verifying that the Jump args are correctly tracked as "uses" of the original variable in the AIMS state map
   - Verifying that the original variable is NOT referenced in any instruction or terminator after the Jump
   - If the variable appears in the function's entry block's defined set, ensure its liveness doesn't extend past the Jump

5. **Matching for-do structure**: The for-do path works because `scope.bind_mutable(__for_coll_N, iter_val)` adds the collection to the mutable bindings list, which then gets threaded through the loop infrastructure as a header param. The key difference: the for-do path's `lower_for_iterator()` uses the existing mutable-var-threading loop infrastructure (header params for ALL mutable vars). The for-yield path manually adds a single block param. Both should produce the same AIMS behavior if the original variable's last use is correctly identified.

6. **Alternative implementation approaches (if step 4 fails):**

   - **Approach E: Kill the original variable after `.iter()`.** After `prepare_iterator()` returns, emit a "consuming" instruction for the original collection variable (e.g., an Apply to a no-op builtin or a Let that assigns it to a new variable that is immediately dead). This makes the variable's last use the consuming instruction, and the AIMS analysis will NOT emit a separate dec because the variable is "consumed" rather than "defined and dead."

   - **Approach F: Add the collection to `iter_element_defs`.** The `collect_iter_element_defs()` function (`emit_rc/helpers.rs:197`) suppresses RcDec for iterator-element projections. Adding the source collection variable to this set would suppress the spurious dec. This differs from failed approach (a) in that it targets only the specific variable causing the problem, not all variables with iterator-element projections. The risk is that the collection's legitimate final dec (needed for cleanup) is also suppressed. To mitigate: only suppress the duplicate dec (the one from `emit_defined_dead`), not the one from the dummy let in the exit block. This requires distinguishing the two decs, which may require adding a flag to the variable.

   The implementer should try the fix in step 4 first (identifying Jump args as uses), then fall back to Approach E if that doesn't work. Approach F is the last resort.

- [ ] Add ARC IR dump assertions: after for-yield lowering, the original collection variable appears only as a Jump arg to header (no other references in subsequent blocks)
- [ ] Verify AIMS backward analysis identifies the Jump arg as the variable's last use (check transfer function in `compiler/ori_arc/src/aims/transfer/mod.rs`)
- [ ] If AIMS does NOT identify Jump args as uses: fix the transfer function to include Jump args in the "uses" set
- [ ] If AIMS already identifies Jump args: investigate why the extra dec still appears -- check whether `emit_defined_dead` fires before `emit_last_use_decs` can consume the variable
- [ ] Implement the fix: restructure `lower_for_yield_iterator()` to match for-do's scope isolation pattern
- [ ] Verify with `ORI_DUMP_AFTER_ARC=1` that the ARC IR has exactly 2 decs for the source collection (1 from `ori_iter_drop`, 1 from AIMS)
- [ ] Verify guard_skip path: when a guard is present, the `guard_skip` block jumps back to header with `coll_param` as arg (line 300-301). Confirm this path does NOT create an extra dec for the collection.

---

## 03.3 AIMS Interaction Verification

**File size warning**: `walk.rs` is 595 lines (limit: 500). If any code changes are needed in `walk.rs`, split it first. Candidate split: extract `emit_pre_instr_incs_unified` (lines 173-270, 97 lines) and `emit_post_instr_decs_unified` + `emit_defined_dead` + `emit_last_use_decs` (lines 277-436, 159 lines) into `walk/incs.rs` and `walk/decs.rs` submodules. The parent `walk.rs` keeps `walk_body_unified` as the dispatch hub.

**File size warning**: `realize/mod.rs` is 505 lines (at boundary). `transfer/mod.rs` is 516 lines. Both exceed the 500-line limit. If Section 03 requires changes to either, split first.

**File(s):** `compiler/ori_arc/src/aims/realize/walk.rs` (emit_defined_dead at line 308, emit_last_use_decs at line 366), `compiler/ori_arc/src/aims/realize/mod.rs` (realize_rc_reuse), `compiler/ori_arc/src/aims/emit_rc/helpers.rs` (collect_iter_element_defs at line 197, collect_defined_vars at line 150 -- re-exported from `emit_rc/mod.rs`), `compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs` (edge cleanup logic)

After implementing the fix, verify that the AIMS pipeline produces correct RC operations for all for-yield patterns:

1. **emit_defined_dead**: Should NOT emit a dec for the source collection variable if its last use is the Jump to header. The variable is "defined and used" (consumed by Jump), not "defined and dead."

2. **emit_last_use_decs**: The source collection's last-use dec should be the one emitted at the Jump to header. This dec is an ownership transfer (the Jump arg passes the reference to the header block param), not a cleanup dec.

3. **edge_cleanup**: The Switch terminator at the header's `__iter_next` check should NOT emit extra RcInc/RcDec for the collection block param. The block param flows through the Branch (has_more/exhausted) without being modified.

4. **collect_iter_element_defs**: Should still correctly suppress decs for element projections (variables created by `Project(next_result, 1)`). This mechanism is orthogonal to the source collection fix.

5. **propagate_borrowed_closure interaction** (`emit_rc/helpers.rs:289-320`): This function propagates borrowed status through Jump args to block params. If the collection variable is passed as a Jump arg to the header, the header block param inherits borrowed status. The dummy let in the exit block (`emit_let(coll_ty, ArcValue::Var(coll_param))`) creates a Let alias of the borrowed param, which also becomes borrowed via propagation. **Critical invariant:** The collection variable passed via Jump is NOT a `Project` destination -- it was created by `lower_expr(iter)` (typically a `Construct` or function return). It should NOT be in the `project_borrowed_defs` or `all_borrowed_defs` sets. Verify that `propagate_borrowed_closure` does not incorrectly mark the collection as borrowed. If it does, the collection's dec will be suppressed, causing a leak (rc=1 remaining after iterator drop decrements rc from 2 to 1).

   The for-do path avoids this because `scope.bind_mutable(__for_coll, iter_val)` makes the phantom a mutable binding. Mutable bindings are threaded through the entire loop infrastructure (header, body, latch, exit) as block params. The AIMS analysis sees the exit param as a "defined and used" variable (via the dummy let), not a borrowed variable. The for-yield path must achieve the same classification for its `coll_param`.

- [ ] Add ARC IR assertion: for-yield on `[str]` has exactly 1 RcInc and 2 RcDec for the source list data (1 from iterator drop, 1 from AIMS)
- [ ] Add ARC IR assertion: for-yield on `[str]` has zero RcDec for iterator-element projections (suppressed by iter_element_defs)
- [ ] Verify edge_cleanup at the header Switch does not produce extra ops for the collection block param
- [ ] Run `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1` on a for-yield `[str]` program and verify zero audit findings
- [ ] **STYLE cleanup**: Split merged doc comment in `helpers.rs:177-196` -- separate the doc for `collect_iter_element_defs` from the doc for `collect_project_borrowed_defs`
- [ ] **STYLE cleanup**: Add missing `///` doc comment to `collect_project_borrowed_defs` at `helpers.rs:236`

---

## 03.4 For-Yield RC Balance Tests

**File(s):** `compiler/ori_llvm/tests/aot/` (new test files)

Comprehensive tests verifying correct RC balance for for-yield with different element types:

| Test | Element Type | Expected Behavior |
|------|-------------|-------------------|
| `for_yield_str_elements` | `[str]` | Correct output, zero leaks, zero double-frees |
| `for_yield_nested_list` | `[[int]]` | Correct output, zero leaks, zero double-frees |
| `for_yield_option_str` | `[Option<str>]` | Correct output, zero leaks, zero double-frees |
| `for_yield_closure` | `[(int) -> int]` | Correct output, zero leaks, zero double-frees |
| `for_yield_struct` | `[{name: str}]` | Correct output, zero leaks, zero double-frees |
| `for_yield_guard_str` | `[str]` with guard | Correct output (filtered), zero leaks |
| `for_yield_nested_loops` | `[[str]]` nested | Correct output, zero leaks |
| `for_yield_empty_list` | `[str]` (empty) | Empty result, zero leaks |

- [ ] Write all 8 AOT tests listed in the table above in `compiler/ori_llvm/tests/aot/`
- [ ] Each test runs with both debug and release builds
- [ ] Each test uses `assert_aot_success` which automatically sets `ORI_CHECK_LEAKS=1` and verifies exit code 0
- [ ] Each test verifies behavioral output (correct values, not just no-crash)
- [ ] Add Valgrind test programs for `[str]` and `[Option<str>]` for-yield in `tests/valgrind/`
- [ ] Run `diagnostics/dual-exec-verify.sh` on all 8 for-yield test programs to confirm interpreter-vs-AOT parity

---

## 03.5 For-Yield break/continue Support (Mandatory Gate for Test Matrix P2/P8)

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_yield.rs` (lower_for_yield_iterator)

**File size warning:** `for_yield.rs` is currently 409 lines. Adding `LoopContext` + break/continue handling could push it past 500 lines. Consider extracting the yield loop body into a helper method, or splitting `for_yield.rs` into `for_yield_option.rs` and `for_yield_iterator.rs` before implementing.

**Gap:** `lower_for_yield_iterator()` does not set up a `LoopContext` (no `loop_ctx` assignment). The for-do path (`for_iterator.rs:154-159`) sets `self.loop_ctx = Some(LoopContext { exit_block, continue_block: header_block, mutable_vars })` before lowering the body. Without this, `break` and `continue` inside a for-yield body cannot resolve their target blocks.

**Impact:** Per Ori spec (Clause 16.10):
- `break` in for-yield stops iteration and returns the accumulated list
- `break value` appends `value` to the list and returns it
- `continue` skips the current element (no yield for this iteration)
- `continue value` yields `value` instead of the body result

All four are valid in for-yield but cannot work in AOT without `LoopContext`.

**Implementation approach:**

1. Create a `LoopContext` with `exit_block` and `continue_block: header_block` before `self.lower_expr(body)`
2. For `break`: jump to exit block (after pushing accumulated list to exit params)
3. For `break value`: call `ori_list_push(list_ptr, value, elem_size)` then jump to exit block
4. For `continue`: jump back to header (no push)
5. For `continue value`: call `ori_list_push(list_ptr, value, elem_size)` then jump to header
6. Mutable vars: for-yield does not currently thread mutable vars. If the body mutates outer variables, the `LoopContext.mutable_vars` must be populated. This requires the same mutable-var-threading infrastructure as for-do.

**Decision required:** This is a significant lowering change. Options:
- **(a) Fix now** as part of this plan (correct, but scope expansion)
- **(b) Skip P2/P8 for-yield tests** with `#skip("for-yield break/continue not yet lowered in AOT")` and track as a follow-up plan item

- [ ] Decide: fix break/continue lowering now (option a) or skip P2/P8 for-yield tests (option b)
- [ ] If fixing: add `LoopContext` setup before `self.lower_expr(body)` in `lower_for_yield_iterator()`
- [ ] If fixing: handle `break` -- push nothing, jump to exit with accumulated list
- [ ] If fixing: handle `break value` -- call `ori_list_push` then jump to exit
- [ ] If fixing: handle `continue` -- jump to header (skip push)
- [ ] If fixing: handle `continue value` -- call `ori_list_push` then jump to header
- [ ] If skipping: add `#skip("for-yield break/continue not yet lowered in AOT")` to all P2 and P8 for-yield tests in Section 05

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] For-yield on `[str]` produces correct output with zero leaks and zero double-frees
- [ ] For-yield on `[Option<str>]` produces correct output with zero leaks and zero double-frees
- [ ] ARC IR for for-yield shows exactly 2 RcDec for source collection (not 3)
- [ ] AIMS backward analysis correctly identifies original variable's last use as Jump to header
- [ ] No `emit_defined_dead` dec emitted for consumed collection variable
- [ ] All 8 AOT tests from 03.4 pass in debug and release
- [ ] All existing for-do tests pass unchanged
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `timeout 150 cargo test -p ori_llvm`
- [ ] `diagnostics/dual-exec-verify.sh` passes for all for-yield test programs

---

## Section 03 Exit Criteria

For-yield loops produce exactly 2 RcDec for the source collection (matching for-do). The AIMS pipeline does not emit spurious extra decs. AOT tests verify correct element cleanup for all fat-pointer element types. Interpreter-vs-AOT parity is confirmed via dual-exec-verify.
