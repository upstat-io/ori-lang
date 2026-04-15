---
section: "05"
title: "AIMS: Owned Element Contract"
status: not-started
reviewed: false
goal: "AIMS analysis treats iterator-yielded elements as owned, removing borrowed suppression rules"
success_criteria:
  - "collect_iter_element_defs in borrowed_defs.rs removed or reworked"
  - "walk_dec.rs no longer suppresses RcDec for iterator element variables"
  - "instr_dispatch.rs no longer registers iterator scratch as borrowed forwarding"
  - "For-loop bodies correctly receive RcDec at scope exit for loop variables"
  - "AIMS contract analysis reflects owned iterator yields"
sections:
  - id: "05.1"
    title: "Remove collect_iter_element_defs suppression"
    status: not-started
  - id: "05.2"
    title: "Update walk_dec.rs dec suppression"
    status: not-started
  - id: "05.3"
    title: "Update instr_dispatch.rs borrowed forwarding"
    status: not-started
  - id: "05.4"
    title: "Verify AIMS contract consistency"
    status: not-started
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
---

# 05 AIMS: Owned Element Contract

This is the most architecturally sensitive section. The AIMS pipeline currently treats iterator-yielded elements as **borrowed** from the source collection — it suppresses RcDec on these variables because the collection's Drop handles cleanup. With the new owned protocol, elements yielded by `__iter_next` are owned (+1 RC) and AIMS must emit standard RcDec for them.

**WARNING:** Changes to AIMS analysis interact with the entire ARC pipeline. Every change must be verified against the full test suite AND the AIMS property tests (proptest lattice verification from plans/llvm-verification-tooling §04).

## 05.1 Remove collect_iter_element_defs Suppression

**File:** `compiler/ori_arc/src/aims/emit_rc/borrowed_defs.rs` (lines 123-197)

`collect_iter_element_defs` marks variables projected from `__iter_next` results as "borrowed definitions" — AIMS then suppresses RcDec for them. With owned elements, this suppression is incorrect.

- [ ] **Option A (preferred):** Remove `collect_iter_element_defs` entirely. With owned elements, the standard AIMS analysis will emit RcDec for loop variables at scope exit — which is correct.
- [ ] **Option B (if A causes issues):** Keep the function but change its semantics — instead of marking elements as borrowed (skip dec), mark them as "owned by protocol" (emit dec normally). This is a softer migration path.
- [ ] **IMPORTANT:** The `iter_elems` set (args[1] phantom type markers) must STILL be excluded from RcDec — they are zero-valued phantom parameters, not real values. Only the element suppression changes; the phantom suppression stays.
- [ ] **TDD:** `for x in [str_list].iter() do print(x)` — after this change, AIMS should emit RcDec on `x` at the end of the loop body. Verify via `ORI_DUMP_AFTER_ARC=1`.
- [ ] Run AIMS property tests: `timeout 150 cargo test -p ori_arc -- lattice::prop_tests` — verify no regressions in lattice properties.

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 05.2 Update walk_dec.rs Dec Suppression

**File:** `compiler/ori_arc/src/aims/realize/walk_dec.rs`

Lines 140-142 in `emit_last_use_decs` skip RcDec for variables in `iter_element_defs`. Lines 94-96 in `emit_defined_dead` do the same.

- [ ] Remove or update the `iter_element_defs` checks in both functions
- [ ] Ensure the standard AIMS dec logic fires for iterator element variables
- [ ] **Keep** the borrowed-parameter and inline-enum suppression (those are unrelated)
- [ ] **TDD:** Verify via `ORI_DUMP_AFTER_ARC=1` that for-loop body variables receive RcDec

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting 05.3:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 05.3 Update instr_dispatch.rs Borrowed Forwarding

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (line ~48)

The codegen registers the iterator scratch pointer for borrowed forwarding. With owned elements, this forwarding may need adjustment.

- [ ] Read the borrowed forwarding logic and determine if it still applies
- [ ] If the forwarding assumed borrowed elements, update it to reflect owned semantics
- [ ] If the forwarding is about the scratch BUFFER (not the element), it may still be correct — verify
- [ ] **TDD:** Compile and run a for-loop with str elements; verify correct RC under `ORI_CHECK_LEAKS=1`

- [ ] **Subsection close-out (05.3)** — MANDATORY before starting 05.4:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 05.4 Verify AIMS Contract Consistency

- [ ] Run `ORI_VERIFY_ARC=1 timeout 150 cargo test -p ori_llvm` — verify ARC IR correctness checks pass
- [ ] Run AIMS lattice property tests: `timeout 150 cargo test -p ori_arc -- lattice::prop_tests`
- [ ] Run full suite: `timeout 150 ./test-all.sh`
- [ ] Verify no new `VerifyError` variants fire for iterator-related code
- [ ] Spot-check `ORI_DUMP_AFTER_ARC=1` output for a representative iterator program — confirm RcDec placement is correct for loop variables

- [ ] **Subsection close-out (05.4)** — MANDATORY before starting Section 06:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] AIMS iterator element suppression removed/reworked
- [ ] For-loop variables receive standard RcDec at scope exit
- [ ] AIMS property tests all pass
- [ ] ARC verification (`ORI_VERIFY_ARC=1`) passes
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
