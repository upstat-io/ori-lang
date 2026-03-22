# RC Header elem_dec_fn — Proper Element Cleanup for Fat Pointer Collections

## Mission

Eliminate all element-level memory leaks and double-frees when iterating over collections whose elements require Drop semantics (`[str]`, `[[T]]`, `[{name: str}]`, closures, etc.) by storing the `elem_dec_fn` pointer in the RC allocation header.

## Problem Statement

When iterating a list `[T]` where `T` has Drop, two independent `ori_buffer_rc_dec` calls race to reach RC=0:

1. **Iterator Drop** (`IterState::List` -> `ori_buffer_rc_dec(data, len, cap, es, elem_dec_fn)`) -- passes whatever `elem_dec_fn` was stored in the `IterState::List` field at creation. Since the `iter-rc-contract` plan (2026-03-18), `emit_list_iter` passes the REAL `elem_dec_fn` (not NULL), so the iterator now carries a valid function.
2. **Explicit RcDec** (`emit_buffer_rc_dec_list_or_set` -> `ori_buffer_rc_dec(data, len, cap, es, elem_dec_fn)`) — passes the real `elem_dec_fn` via `get_or_generate_elem_dec_fn(elem_type)`.

**With the iter-rc-contract fix, both paths now carry real `elem_dec_fn`.** However, the header-based approach (this plan) provides defense-in-depth: even if a future code path passes NULL, the header ensures the function is available. The ordering dependency is eliminated — whoever reaches zero reads from the header.

### Current Workarounds (to be removed by this plan)

1. **Phantom `__for_coll_N` binding** in `lower_for` (`loops.rs` lines 169-184) — threads the collection through the loop header to force AIMS to order the explicit RcDec after `ori_iter_drop`. Works for simple in-function iteration, fails for function parameters and nested loops. Only covers `List | Set` (line 177), NOT maps despite the comment claiming otherwise.
2. **Dummy reference after `ori_iter_drop`** in exit block (`for_iterator.rs` lines 192-207) — keeps collection alive past iterator drop. Defeated by invariant-param elimination for function parameters.

### Patterns that fail with workarounds

| Pattern | Failure Mode |
|---------|-------------|
| `[str]` passed to function, iterated inside | Double-free: function's RcDec + caller's cleanup both fire |
| `[[int]]` nested iteration (intermittent) | Double-free: inner list freed by inner iterator, then outer cleanup |
| `for w in words do { push_to_other_list(w) }` | Potential element use-after-free if iterator dec reaches zero |

## Solution: RC Header V5

**Design Note — Maps**: Maps require TWO cleanup functions (`key_dec_fn` + `val_dec_fn`), not one. The single `elem_dec_fn` header slot works for lists and sets, but maps need a different approach. **Decision (resolved)**: option (c) — `emit_map_iter` passes real functions (codegen-based, no header change for maps). Implemented by iter-rc-contract plan (2026-03-18). See Section 01.3 for decision rationale and Section 02.3 for verification.

**Design Note — Sets**: Sets use `ori_set_buffer_rc_dec(data, cap, len, elem_size, elem_dec_fn)` which uses hash table layout (`[metadata | elements]`), NOT contiguous array layout. The header-based `elem_dec_fn` works for sets (single function). Set construction codegen (`CtorKind::SetLiteral` in `construction.rs`) stores `elem_dec_fn` and `elem_count` at literal construction time (completed in Section 02.1). Sets do NOT need `elem_count` for cleanup (they use metadata scanning), but the field is stored for consistency.

Extend the RC allocation header to 32 bytes, adding `elem_dec_fn` and `elem_count` slots:

```
V3 (original): [data_size: i64 | strong_count: i64 | data ...]                                  = 16 bytes
V4 (elem_dec): [data_size: i64 | elem_dec_fn: ptr | strong_count: i64 | data ...]               = 24 bytes
V5 (current):  [data_size: i64 | elem_dec_fn: ptr | elem_count: i64 | strong_count: i64 | data] = 32 bytes
```

**Key invariant**: `strong_count` remains at `data_ptr - 8`. All existing RC operations are unchanged. `elem_dec_fn` sits at `data_ptr - 24`, `elem_count` at `data_ptr - 16`.

When `ori_buffer_rc_dec` is called with a non-NULL `elem_dec_fn`, it stores the function pointer in the header (write-once — first non-NULL wins). When `ori_buffer_rc_dec` reaches zero, it reads the stored `elem_dec_fn` from the header and calls it on each element before freeing the buffer. (`ori_rc_dec` is unaffected — it handles non-buffer RC objects like structs/strings via `drop_fn`, not element iteration.)

This eliminates the ordering dependency: **whoever reaches zero uses the stored function**, regardless of what the caller passed.

## Section Dependency Graph

```
Section 01: RC Header Extension
    |---> Section 02: Codegen & Runtime Integration
              |---> Section 03: Remove Workarounds & Simplify
                        |---> Section 04: Combinatorial Test Matrix
                                  |---> Section 05: Verification & Cleanup
```

Strictly sequential — each section depends on the previous.

## Implementation Sequence

1. **Section 01** — Modify `ori_rt` runtime: extend header, update alloc/free/realloc, add `elem_dec_fn` store/load helpers, update all RC dec functions to read from header.
2. **Section 02** — Wire up codegen: list and set construction stores `elem_dec_fn` AND `elem_count` in the RC header at buffer creation time. Map iteration already passes real `key_dec_fn`/`val_dec_fn` (fixed by iter-rc-contract plan, 2026-03-18). ALL COW slow paths (list, set, and map-to-list) propagate `elem_dec_fn` (and `elem_count` for list buffers) from old to new buffer. Buffer-creating runtime functions (`ori_map_keys_to_list`, `ori_map_values_to_list`, `ori_str_split`, `ori_set_to_list`, `write_array_to_list`) extended with `elem_dec_fn` parameter. Codegen emits header-store calls after `ori_iter_collect` and `ori_iter_collect_set` return. `alloc_set_hash_buffer` and `rehash_set` centralize `elem_dec_fn` propagation for set buffer allocations. `ori_args_from_argv` stores `elem_count` for the `[str]` args list. Four ABI sync points (runtime + LLVM IR + codegen must be single-commit changes).
3. **Section 03** — Remove `__for_coll_N` phantom binding workaround (for-do path), for-yield `coll_param` collection threading (for-yield path), dummy references in exit blocks (both paths), dead `elem_dec_fn` parameter from `ori_iter_from_list` and `IterState::List`. Simplify `lower_for`, `lower_for_iterator`, `lower_for_yield_iterator`, `lower_break`, and `lower_continue`.
4. **Section 04** — Write combinatorial test matrix: 9 type categories x 12 language features x 4 execution modes. Split `fat_ptr_iter.rs` into directory module.
5. **Section 05** — Full verification pass: all tests green, valgrind clean, code journeys re-run, unignore tests, documentation updated.

**CRITICAL — ABI boundary**: The RC header size change affects BOTH `ori_rt` (Rust runtime) AND `ori_llvm` (LLVM codegen). Both must agree on the header layout. Any LLVM IR that hardcodes pointer offsets for RC header fields (e.g., GEP with constant offsets) must be updated simultaneously. The `runtime_functions.rs` declarations in `ori_llvm` must match the updated `ori_rt` function signatures. This is a single-commit change — partial updates break the ABI contract.

## Plan-Level Warnings

1. **`compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` is 1485 lines** — far above the 500-line limit, but the file self-documents its exemption (it is a pure static data table). `ori_buffer_store_elem_dec` and `ori_buffer_store_elem_count` have been added. Do not add any logic here.

2. **ABI boundary is a single-commit change** — Sections 01 and 02 modify both `ori_rt` (Rust runtime) and `ori_llvm` (LLVM codegen). The `RC_HEADER_SIZE` constant change, the `ori_rc_alloc` pointer arithmetic change, and the LLVM IR constant offsets must all be committed together. Partial updates break the ABI contract and produce silent memory corruption.

3. **COW slow path propagation (Section 02.1) is the most complex item** — There are 20+ runtime functions across `cow.rs`, `cow_structural.rs`, `cow_sort.rs`, `query.rs`, `slice.rs`, `mod.rs`, `iterator/consumers.rs`, `set/cow/basic.rs`, `set/cow/algebra.rs`, and `set/mod.rs` that allocate new collection buffers via `ori_rc_alloc`. List buffer allocations must propagate BOTH `elem_dec_fn` AND `elem_count`. Set hash table buffer allocations must propagate `elem_dec_fn` only (sets use metadata scanning, not `elem_count`, for cleanup). Additionally, `ori_map_keys_to_list`, `ori_map_values_to_list`, `ori_str_split`, and `ori_set_to_list` require signature changes to accept `elem_dec_fn`, which cascades to LLVM IR declarations and codegen call sites. Consider adding a `debug_assert!` in `ori_buffer_rc_dec` that warns if a buffer reaches zero with both NULL header and non-NULL parameter.

4. **Map approach decided: codegen-based (option c)** — Maps need TWO cleanup functions (key + value) that cannot fit in a single header slot. Section 01.3 resolved this: maps use the codegen-based approach where `emit_map_iter` passes real `key_dec_fn`/`val_dec_fn`. The header `elem_dec_fn` slot is used for lists and sets only.

5. **Pre-existing stale "8-byte header" references in `list/mod.rs`** — Lines 83, 131, and 199 reference "8-byte refcount header" which was wrong even for V3 (16 bytes). These must be fixed to "32-byte RC header (V5)" as part of Section 02 cleanup. Similarly, `.claude/rules/runtime.md` line 29 says "8-byte header" (flagged in Section 05).

6. **Pre-existing decorative banners in touched files** — `list/mod.rs` has 2 decorative banners, `iterator/consumers.rs` has 8 decorative banners. Per hygiene rules, these must be replaced with plain section comments when the files are touched during Section 02 implementation.

7. **Plan references two phantom functions** — The original plan referenced `ori_list_filter` and `ori_list_slice_copy` in `query.rs`, but these functions do not exist. The actual allocating functions in `query.rs` are `ori_list_reverse` (line 122) and `ori_list_concat` (line 170). Also `ori_list_slice_materialized` was misspelled; the actual function is `ori_list_materialize_slice` (line 152 of `slice.rs`). These have been corrected in Sections 01 and 02.

8. **`construction.rs` was at 499 lines** — **Resolved in Section 02**: extracted `emit_variant_via_alloca` and `emit_variant_via_insertvalue` into `variant_construction.rs` (169 lines). `construction.rs` now 358 lines.

9. **`cow_sort.rs` was at 499 lines** — **Resolved in Section 02** (TPR-02-025): converted to directory module `cow_sort/`. `cow_sort/mod.rs` (324 lines) + `cow_sort/sort.rs` (215 lines). Both well under 500-line limit.

10. **Function name mismatch in original plan** — Section 02 referenced `ori_map_keys` and `ori_map_values` but the actual runtime function names are `ori_map_keys_to_list` and `ori_map_values_to_list`. The plan also referenced `emit_list_iter_collect` in `list_builtins.rs` but the actual function is `emit_iter_collect` in `builtins/iterator_consumers.rs`. Corrected in Section 02.

11. **`ori_args_from_argv` creates a `[str]` list buffer** — `lib.rs:303` allocates via `ori_rc_alloc` for `@main(args: [str])` programs. Added to Section 02 with header-store requirements. The `elem_dec_fn` for `str` is an LLVM-generated thunk (same issue as `ori_str_split`), so either add a parameter (ABI sync point) or rely on deferred store. `elem_count` can be stored internally.

12. **Section 02 has 4 ABI sync points** — `ori_map_keys_to_list`, `ori_map_values_to_list`, `ori_str_split`, and `ori_set_to_list` all require simultaneous updates to runtime signature + LLVM IR declaration + codegen call site. A consolidated reference table is in Section 02.N. Partial updates produce silent memory corruption or linker errors.

13. **Dead `_ea` (elem_align) computations in set COW** — `set/cow/basic.rs` (2 sites) and `set/cow/algebra.rs` (3 sites) accept `elem_align` from codegen, compute `_ea = elem_align.max(1) as usize`, then discard it. All actual buffer allocations hardcode alignment `8`. When adding `elem_dec_fn` propagation, resolve this: either pass `_ea` to allocation functions or remove the dead computation.

14. **`map/mod.rs` re-export masks unused import** — **Resolved in Section 02**: the `#[allow(unused_imports)]` was removed. `META_EMPTY` IS used within `mod.rs` itself (line 50); the original plan claim that it was unused was incorrect.

15. **Second `vec![0u8; elem_size]` allocation in `cow_sort.rs`** — Plan warning 9 only mentions line 256 (`reverse_cow`). There is a second identical allocation at line 458 (`apply_permutation_in_place`). Both should use stack arrays.

16. **`compiler/ori_rt/src/iterator/mod.rs` has 2 decorative banners** — lines 43 and 60 (`// ── Extern C API — Core`, `// ── Extern C API — Cleanup`). Per hygiene rules, replace with plain section comments when touched during Section 03.2.5 implementation.

17. **`compiler/ori_rt/src/iterator/tests.rs` has 22 decorative banners** — test section markers use `// ──` decorative style. Since Section 03.2.5 modifies 30+ `ori_iter_from_list` calls in this file, the banners should be replaced with plain `// Section name` comments in the same commit.

18. **Section 01 body status drifted from frontmatter** — Frontmatter says `status: complete` but body text said `**Status:** In Progress`. Fixed to `Complete`.

## Success Criteria

- [ ] `test_str_list_passed_to_two_functions` passes reliably (currently active, not ignored)
- [ ] `test_nested_list_iteration` passes reliably (currently active, not ignored)
- [ ] Full combinatorial test matrix passes (Section 04)
- [ ] Zero regressions: all existing tests pass (`timeout 150 ./test-all.sh`)
- [ ] All tests pass in release build (`cargo b --release`) — debug and release differ due to FastISel
- [ ] Valgrind clean on all fat pointer iteration patterns
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all AOT tests
- [ ] Code journeys J15-J17 re-run with improved scores
- [ ] Phantom `__for_coll_N` workaround completely removed (for-do path)
- [ ] For-yield `coll_param` collection threading completely removed (for-yield path)
- [ ] `for_coll_counter` field and `ForYieldContext::coll_param` field removed from `ArcLowerer` / `ForYieldContext`
- [ ] Dead `elem_dec_fn` parameter removed from `ori_iter_from_list` and `IterState::List`
- [x] Map iteration passes real `key_dec_fn`/`val_dec_fn` (not NULL) (implemented by iter-rc-contract plan, 2026-03-18)
- [ ] No stale "16-byte header" or "24-byte header" references in codebase (V5 = 32 bytes)
