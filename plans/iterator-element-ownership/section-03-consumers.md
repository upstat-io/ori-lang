---
section: "03"
title: "Consumers: Decrement After Use"
status: not-started
reviewed: false
goal: "Every consumer function receives elem_dec_fn and calls it after consuming each element"
success_criteria:
  - "All 11 consumer functions receive elem_dec_fn parameter"
  - "Ephemeral consumers (count, any, all, for_each) dec every element after use"
  - "Transfer consumers (collect, collect_set) skip inc (element already owned)"
  - "Selection consumers (find, last, rfind) dec skipped elements, transfer found element"
  - "Accumulation consumers (fold, rfold) dec every element after fold_fn call"
  - "Join consumer decs string elements after push_str (both paths)"
sections:
  - id: "03.1"
    title: "Ephemeral consumers: count, any, all, for_each"
    status: not-started
  - id: "03.2"
    title: "Transfer consumers: collect, collect_set"
    status: not-started
  - id: "03.3"
    title: "Selection consumers: find, last, rfind"
    status: not-started
  - id: "03.4"
    title: "Accumulation consumers: fold, rfold"
    status: not-started
  - id: "03.5"
    title: "Join consumer: both paths"
    status: not-started
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
---

# 03 Consumers: Decrement After Use

With owned elements, every consumer must clean up after use. The cleanup pattern depends on the consumer category:

- **Ephemeral** (count, any, all, for_each): dec every element after use
- **Transfer** (collect, collect_set): transfer ownership into the result — NO inc needed (element already +1 owned)
- **Selection** (find, last, rfind): transfer found element to output, dec all skipped elements
- **Accumulation** (fold, rfold): dec each element after fold_fn processes it
- **Join**: dec string element after push_str

## 03.1 Ephemeral Consumers: count, any, all, for_each

**File:** `compiler/ori_rt/src/iterator/consumers.rs`

These consumers read elements and discard them — the element is never retained past the loop iteration.

- [ ] **count** (lines 188-204): Add `elem_dec_fn` parameter. After each `state.next()`, call `elem_dec_fn(elem_buf.as_mut_ptr())` on the yielded element.
- [ ] **any** (lines 212-237): Add `elem_dec_fn` parameter. Call dec on every element evaluated by the predicate, INCLUDING the short-circuit element.
- [ ] **all** (lines 245-270): Add `elem_dec_fn` parameter. Same as `any` — dec on every element including the short-circuit one.
- [ ] **for_each** (lines 336-355): Add `elem_dec_fn` parameter. The closure borrows the element. After the closure returns, call dec.
- [ ] **TDD:** AOT tests for each with str elements + map adapter, verify zero leaks

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.2 Transfer Consumers: collect, collect_set

**File:** `compiler/ori_rt/src/iterator/consumers.rs`

These consumers move elements into a new collection. With owned elements:
- The yielded element is already +1 owned
- Copying it into the new buffer transfers ownership
- **No elem_inc_fn call needed** — remove the existing inc call
- Add `elem_dec_fn` parameter for duplicate handling in collect_set

- [ ] **collect** (lines 24-88): Remove `elem_inc_fn` call (lines 66-68). Elements are already owned — copying bytes into the new buffer IS the ownership transfer. The new buffer's `elem_dec_fn` (stored post-call by codegen) handles cleanup when the collected list is freed.
- [ ] **collect_set** (lines 102-182): Remove `elem_inc_fn` call (lines 173-175). Add `elem_dec_fn` parameter. Call dec on duplicate elements (line 151 `continue` — the duplicate was yielded as owned but rejected by the set; it must be dec'd to prevent leak).
- [ ] Update codegen signatures (Section 04): remove `elem_inc_fn` from collect/collect_set calls; add `elem_dec_fn` to collect_set.
- [ ] **TDD:** Existing collect tests should still pass. Add leak test for collect_set with duplicate str elements.

**Critical correctness argument:** Before this change, `collect` had `elem_inc_fn` to protect elements from the source iterator's Drop (which dec's elements via the buffer header's `elem_dec_fn`). With the new protocol, the source yields +1 owned elements, so the source's Drop dec's the source's original copy (RC=N → N-1) while the collected copy maintains its own RC (+1 from the yield). Net RC is balanced.

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.3 Selection Consumers: find, last, rfind

**File:** `compiler/ori_rt/src/iterator/consumers.rs`

These consumers select ONE element and discard the rest. The found element is transferred to the output; all skipped elements must be dec'd.

- [ ] **find** (lines 281-328): Add `elem_dec_fn`. Dec every element that fails the predicate. The matching element is copied to `out_ptr` — ownership transfers. When writing `Some` tag, the element is already owned. Dec remaining elements via iterator Drop.
- [ ] **last** (lines 432-466): Add `elem_dec_fn`. Dec every element EXCEPT the last one (which is transferred to output). Each iteration overwrites the previous "last" element — the overwritten one must be dec'd.
- [ ] **rfind** (lines 666-714): Add `elem_dec_fn`. This eagerly collects all elements into a `Vec<u8>` then searches backward. Each checked-and-rejected element must be dec'd. The found element transfers to output.
- [ ] **TDD:** AOT tests — `["a","b","c"].iter().map(s -> s + "x").find(where: s -> s == "bx")` with str elements, verify zero leaks. Test `last` and `rfind` similarly.

**Dangling pointer fix:** With the old protocol, `find`/`last` returned borrowed element bytes that could dangle after iterator Drop (Codex finding). With owned elements, the returned element has its own RC — no dangling.

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.4 Accumulation Consumers: fold, rfold

**File:** `compiler/ori_rt/src/iterator/consumers.rs`

These pass each element to a fold function. The fold function borrows the element. After it returns, the element must be dec'd.

- [ ] **fold** (lines 365-423): Add `elem_dec_fn`. After `fold_fn(fold_env, acc_ptr, elem_buf.as_ptr(), result_ptr)`, call dec on `elem_buf`.
- [ ] **rfold** (lines 599-657): Add `elem_dec_fn`. Same pattern — dec each element after fold_fn processes it. Note: rfold eagerly collects into Vec first, so the Vec's elements are already owned. Dec each element as it's processed during the backward fold.
- [ ] **TDD:** AOT test — `["hello","world"].iter().map(s -> s + "!").fold(initial: "", op: (acc, s) -> acc + s)` with str elements, verify zero leaks.

- [ ] **Subsection close-out (03.4)** — MANDATORY before starting 03.5:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.5 Join Consumer: Both Paths

**File:** `compiler/ori_rt/src/iterator/consumers.rs`

Join already has partial cleanup (BUG-05-002 fix: trampoline path frees heap OriStr). The direct-string path (BUG-05-003) still leaks.

- [ ] **Direct-string path** (lines 570-573): Add `elem_dec_fn`. After `push_str(s.as_str())`, call `elem_dec_fn(elem_buf.as_mut_ptr())` — this handles both SSO (no-op) and heap (dec + free) strings.
- [ ] **Trampoline path** (lines 553-569): Already has explicit `ori_str_rc_dec`. With the new protocol, the STRING produced by the trampoline is the owned copy. The original INT/FLOAT/BOOL element that was passed to the trampoline was also owned. After the trampoline converts it to a string, the original element must be dec'd if it was RC-typed (but int/float/bool are scalars — no RC, so no dec needed). The trampoline-produced OriStr still needs the existing dec. No change needed for trampoline path.
- [ ] **Remove the BUG-05-002 fix's explicit `ori_str_rc_dec` call** (lines 563-569) — replace with the uniform `elem_dec_fn` call that handles all element types.
- [ ] Update join's C signature: add `elem_dec_fn` parameter.
- [ ] **TDD:** `["aaa..."].iter().map(s -> s + "x").join(separator: ",")` zero leaks — THIS is BUG-05-003's repro.

- [ ] **Subsection close-out (03.5)** — MANDATORY before starting Section 04:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] All 11 consumer functions updated with elem_dec_fn
- [ ] collect no longer calls elem_inc_fn
- [ ] collect_set decs duplicate elements
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
