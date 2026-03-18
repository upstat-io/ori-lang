# RC Header elem_dec_fn — Proper Element Cleanup for Fat Pointer Collections

## Mission

Eliminate all element-level memory leaks and double-frees when iterating over collections whose elements require Drop semantics (`[str]`, `[[T]]`, `[{name: str}]`, closures, etc.) by storing the `elem_dec_fn` pointer in the RC allocation header.

## Problem Statement

When iterating a list `[T]` where `T` has Drop, two independent `ori_buffer_rc_dec` calls race to reach RC=0:

1. **Iterator Drop** (`IterState::List` → `ori_buffer_rc_dec(data, len, cap, es, elem_dec_fn)`) — passes whatever `elem_dec_fn` was stored in the `IterState::List` field at creation. Currently, `emit_list_iter` in `list_builtins.rs` passes NULL at construction time, so the iterator carries NULL. <!-- reviewed: accuracy fix — NULL comes from emit_list_iter construction, not from Drop ignoring a field -->
2. **Explicit RcDec** (`emit_buffer_rc_dec_list_or_set` → `ori_buffer_rc_dec(data, len, cap, es, elem_dec_fn)`) — passes the real `elem_dec_fn` via `get_or_generate_elem_dec_fn(elem_type)`. <!-- reviewed: accuracy fix — corrected function name from emit_rc_dec_heap -->

**Whoever reaches zero determines whether elements are cleaned up.** If the iterator's NULL-carrying call wins, elements leak. If the explicit call wins, cleanup happens. This ordering is non-deterministic across patterns (function params, nested loops, break paths).

### Current Workarounds (to be removed by this plan)

1. **Phantom `__for_coll` binding** in `lower_for` — threads the collection through the loop header to force AIMS to order the explicit RcDec after `ori_iter_drop`. Works for simple in-function iteration, fails for function parameters and nested loops.
2. **Dummy reference after `ori_iter_drop`** in exit block — keeps collection alive past iterator drop. Defeated by invariant-param elimination for function parameters.

### Patterns that fail with workarounds

| Pattern | Failure Mode |
|---------|-------------|
| `[str]` passed to function, iterated inside | Double-free: function's RcDec + caller's cleanup both fire |
| `[[int]]` nested iteration (intermittent) | Double-free: inner list freed by inner iterator, then outer cleanup |
| `for w in words do { push_to_other_list(w) }` | Potential element use-after-free if iterator dec reaches zero |

## Solution: RC Header V4

<!-- reviewed: added design note about maps -->
**Design Note — Maps**: Maps require TWO cleanup functions (`key_dec_fn` + `val_dec_fn`), not one. The single `elem_dec_fn` header slot works for lists and sets, but maps need a different approach. Options: (a) store a combined descriptor pointer, (b) add a second slot (32-byte header), (c) use a separate mechanism for maps. `emit_map_iter` currently passes NULL for both, confirming maps have the same ordering issue. This must be resolved in Section 01.3 / Section 02.3 before implementation.

<!-- reviewed: completeness fix — added design note about set hash tables -->
**Design Note — Sets**: Sets use `ori_set_buffer_rc_dec(data, cap, len, elem_size, elem_dec_fn)` which uses hash table layout (`[metadata | elements]`), NOT contiguous array layout. The header-based `elem_dec_fn` works for sets (single function), but the codegen must also store the `elem_dec_fn` at set construction time, not just list construction. `emit_set_construct` in `construction.rs` must be updated alongside list construction.

Extend the RC allocation header from 16 to 24 bytes, adding an `elem_dec_fn` slot:

```
V3 (current): [data_size: i64 | strong_count: i64 | data ...]   = 16 bytes header
V4 (new):     [data_size: i64 | elem_dec_fn: ptr | strong_count: i64 | data ...] = 24 bytes header
```

<!-- reviewed: CRITICAL accuracy fix — strong_count MUST remain at data_ptr - 8. The original
     V4 layout [data_size | strong_count | elem_dec_fn | data] would break ALL existing RC
     operations (ori_rc_inc, ori_rc_dec, ori_rc_count, ori_rc_is_unique, ori_buffer_rc_dec)
     which rely on strong_count being at data_ptr - 8. The corrected layout places elem_dec_fn
     between data_size and strong_count so that data_ptr - 8 = strong_count (unchanged),
     data_ptr - 16 = elem_dec_fn, data_ptr - 24 = data_size. -->

When `ori_buffer_rc_dec` is called with a non-NULL `elem_dec_fn`, it stores the function pointer in the header (write-once — first non-NULL wins). When `ori_buffer_rc_dec` reaches zero, it reads the stored `elem_dec_fn` from the header and calls it on each element before freeing the buffer. (`ori_rc_dec` is unaffected — it handles non-buffer RC objects like structs/strings via `drop_fn`, not element iteration.) <!-- reviewed: accuracy fix — ori_rc_dec does not participate in element cleanup -->

This eliminates the ordering dependency: **whoever reaches zero uses the stored function**, regardless of what the caller passed.

## Section Dependency Graph

```
Section 01: RC Header Extension
    └──→ Section 02: Codegen & Runtime Integration
              └──→ Section 03: Remove Workarounds & Simplify
                        └──→ Section 04: Combinatorial Test Matrix
                                  └──→ Section 05: Verification & Cleanup
```

Strictly sequential — each section depends on the previous.

## Implementation Sequence

1. **Section 01** — Modify `ori_rt` runtime: extend header, update alloc/free/realloc, add `elem_dec_fn` store/load.
2. **Section 02** — Wire up codegen: list construction (`emit_construct` / `CtorKind::ListLiteral` in `construction.rs`) stores `elem_dec_fn` in the RC header at buffer creation time; iterator Drop reads it from the header via `ori_buffer_rc_dec`. <!-- reviewed: accuracy fix — elem_dec_fn is stored at list construction, not at emit_list_iter -->
3. **Section 03** — Remove phantom binding workaround, dummy reference, simplify `lower_for` and `lower_for_iterator`.
4. **Section 04** — Write combinatorial test matrix: 8 type categories × 8 language features × 3 execution modes.
5. **Section 05** — Full verification pass: all tests green, valgrind clean, code journeys re-run, unignore tests.

<!-- reviewed: completeness fix — added critical note about ABI boundary -->
**CRITICAL — ABI boundary**: The RC header size change affects BOTH `ori_rt` (Rust runtime) AND `ori_llvm` (LLVM codegen). Both must agree on the header layout. Any LLVM IR that hardcodes pointer offsets for RC header fields (e.g., GEP with constant offsets) must be updated simultaneously. The `runtime_functions.rs` declarations in `ori_llvm` must match the updated `ori_rt` function signatures. This is a single-commit change — partial updates break the ABI contract.

## Plan-Level Warnings <!-- reviewed: hygiene fix -->

1. **`runtime_functions.rs` is 1438 lines** — far above the 500-line limit, but the file self-documents its exemption (it is a pure static data table). Adding `ori_buffer_store_elem_dec` to this file is acceptable, but do not add any logic here.

2. **ABI boundary is a single-commit change** — Sections 01 and 02 modify both `ori_rt` (Rust runtime) and `ori_llvm` (LLVM codegen). The `RC_HEADER_SIZE` constant change, the `ori_rc_alloc` pointer arithmetic change, and the LLVM IR constant offsets must all be committed together. Partial updates break the ABI contract and produce silent memory corruption.

3. **COW slow path propagation (Section 02.1) is the most complex item** — There are 5 COW functions in `cow.rs` that allocate new buffers on the slow path. Each must propagate `elem_dec_fn` from old to new header. This is easy to implement but hard to audit exhaustively. Consider adding a `debug_assert!` in `ori_buffer_rc_dec` that warns if a buffer reaches zero with both NULL header and non-NULL parameter.

4. **Map approach must be decided before implementation** — Section 01.3 flags the map two-slot problem but recommends option (c) (codegen-based, not header-based). This decision should be finalized in review before starting implementation, as it affects whether maps benefit from the header at all.

## Success Criteria

- [ ] `test_str_list_passed_to_two_functions` passes (currently `#[ignore]`)
- [ ] `test_nested_list_iteration` passes (currently `#[ignore]`)
- [ ] Full combinatorial test matrix passes (Section 04)
- [ ] Zero regressions: all existing tests pass (`timeout 150 ./test-all.sh`)
- [ ] All tests pass in release build (`cargo b --release`) — debug and release differ due to FastISel <!-- reviewed: completeness fix -->
- [ ] Valgrind clean on all fat pointer iteration patterns
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all AOT tests
- [ ] Code journeys J15-J17 re-run with improved scores
- [ ] Phantom `__for_coll` workaround completely removed
- [ ] Dead `elem_dec_fn` parameter removed from `ori_iter_from_list` and `IterState::List` <!-- reviewed: completeness fix -->
- [ ] Map iteration passes real `key_dec_fn`/`val_dec_fn` (not NULL) <!-- reviewed: completeness fix -->
- [ ] No stale "16-byte header" references in codebase <!-- reviewed: completeness fix -->
