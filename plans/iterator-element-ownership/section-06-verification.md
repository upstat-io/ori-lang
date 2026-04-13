---
section: "06"
title: "Verification Matrix"
status: not-started
reviewed: false
goal: "Full consumer x adapter x type test matrix with leak checking"
success_criteria:
  - "Every consumer x producing-adapter x RC-type combination has a dedicated AOT test"
  - "ORI_CHECK_LEAKS=1 reports zero leaks on all matrix tests"
  - "Semantic pin tests verify owned-element protocol (would fail if inc/dec removed)"
  - "Negative pin tests verify scalar elements have zero RC overhead"
  - "Debug AND release builds pass all tests"
sections:
  - id: "06.1"
    title: "Core matrix: consumer x map adapter x str"
    status: not-started
  - id: "06.2"
    title: "Extended matrix: filter+map, skip+map chains"
    status: not-started
  - id: "06.3"
    title: "Semantic and negative pins"
    status: not-started
  - id: "06.4"
    title: "Cross-type coverage: [int], closures, nested"
    status: not-started
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: none
  updated: null
---

# 06 Verification Matrix

The verification matrix ensures that the ownership protocol is correct across ALL consumer x adapter x element-type combinations. Every cell in the matrix must pass with zero leaks under `ORI_CHECK_LEAKS=1`.

## 06.1 Core Matrix: Consumer x Map Adapter x str

**Files:** `compiler/ori_llvm/tests/aot/iterators.rs`, fixture files

The `.map()` adapter is the primary producer of owned elements. Test every consumer with a mapped str iterator.

- [ ] `[str].iter().map(s -> s + "!").collect()` — transfer semantics
- [ ] `[str].iter().map(s -> s + "!").count()` — ephemeral dec
- [ ] `[str].iter().map(s -> s + "!").any(s -> s == "hello!")` — short-circuit dec
- [ ] `[str].iter().map(s -> s + "!").all(s -> len(s) > 0)` — full traversal dec
- [ ] `[str].iter().map(s -> s + "!").find(where: s -> s == "hello!")` — selection dec
- [ ] `[str].iter().map(s -> s + "!").for_each(s -> print(msg: s))` — closure dec
- [ ] `[str].iter().map(s -> s + "!").fold(initial: "", op: (a, s) -> a + s)` — accumulation dec
- [ ] `[str].iter().map(s -> s + "!").last()` — selection dec (overwrite pattern)
- [ ] `[str].iter().map(s -> s + "!").join(separator: ",")` — string dec (BUG-05-003 repro)
- [ ] `[str].iter().map(s -> s + "!").rev().collect()` — rev storage + transfer
- [ ] All tests above: verify `assert_aot_success` (which enables `ORI_CHECK_LEAKS=1`)
- [ ] All tests above: run in both debug AND release mode

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.2 Extended Matrix: Adapter Chains

Test the interaction of ownership across adapter chains where elements are owned, passed through, and discarded at different points.

- [ ] `[str].iter().filter(s -> s != "x").map(s -> s + "!").join(separator: ",")` — filter passes borrowed, map produces owned, join decs
- [ ] `[str].iter().map(s -> s + "!").filter(s -> len(s) > 3).collect()` — map produces owned, filter decs rejected, collect transfers
- [ ] `[str].iter().map(s -> s + "!").skip(count: 1).collect()` — skip decs 1 owned element
- [ ] `[str].iter().map(s -> s + "!").take(count: 2).collect()` — take passes 2, source Drop handles rest
- [ ] `[str].iter().map(s -> s + "!").enumerate().collect()` — enumerate wraps owned
- [ ] `[str].iter().map(s -> s + "!").chain([str].iter().map(s -> s + "?")).collect()` — two owned sources chained
- [ ] All tests: `assert_aot_success` + debug/release

- [ ] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.3 Semantic and Negative Pins

- [ ] **Semantic pin (ownership):** `[str].iter().map(s -> s + "x").join(separator: ",")` MUST produce zero leaks. This test would FAIL if the inc-on-yield or dec-after-use were removed.
- [ ] **Semantic pin (filter cleanup):** `[str].iter().filter(s -> false).count()` MUST produce zero leaks. All elements are rejected and dec'd by filter. Would fail if filter doesn't dec.
- [ ] **Semantic pin (collect transfer):** `[str].iter().map(s -> s + "!").collect()` MUST produce zero leaks AND no double-free. Would fail if collect still calls elem_inc_fn (double inc = leak).
- [ ] **Negative pin (scalar no-op):** `[1,2,3].iter().map(x -> x * 2).count()` — scalar elements. `elem_inc_fn` and `elem_dec_fn` are null. Verify zero overhead (no RC ops in `ORI_TRACE_RC=1` output for element values).

- [ ] **Subsection close-out (06.3)** — MANDATORY before starting 06.4:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.4 Cross-Type Coverage

Extend the core matrix to other RC element types.

- [ ] `[[int]].iter().map(l -> [...l, 99]).collect()` — nested list elements (list-of-lists)
- [ ] `[str].iter().map(s -> s).collect()` — identity map (still produces owned copy)
- [ ] Verify interpreter and LLVM produce identical results for all matrix tests (dual-execution parity)
- [ ] Run `timeout 150 ./test-all.sh` — full regression check

- [ ] **Subsection close-out (06.4)** — MANDATORY before starting 06.N:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] Full consumer x adapter x type matrix passes with zero leaks
- [ ] Semantic pin tests verify protocol correctness
- [ ] Negative pin verifies scalar no-op
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] BUG-05-003 bug entry updated: `- [x]` with resolution
- [ ] BUG-05-002 remains fixed (verify join trampoline tests still pass)
- [ ] Bug-tracker `00-overview.md` open count updated
- [ ] `/commit-push`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
