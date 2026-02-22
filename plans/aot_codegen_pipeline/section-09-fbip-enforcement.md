---
section: "09"
title: "FBIP Enforcement"
status: not-started
goal: "Promote @fbip from informational analysis to enforced compile-time annotation"
inspired_by:
  - "Koka CheckFBIP (Core/CheckFBIP.hs)"
sections:
  - id: "09.1"
    title: "Define @fbip annotation"
    status: not-started
  - id: "09.2"
    title: "Wire enforcement into pipeline"
    status: not-started
  - id: "09.3"
    title: "Diagnostics"
    status: not-started
  - id: "09.4"
    title: "Tests"
    status: not-started
---

# Section 09: FBIP Enforcement

**Status:** Not Started
**Goal:** Functions annotated `@fbip` are verified to be "functional but in-place" — all constructor reuse opportunities are realized.

**Context:** Ori already has `analyze_fbip` in `ori_arc/src/fbip/mod.rs` (237 lines) which detects missed reuse opportunities. Currently this is purely informational (debug-level tracing). The Koka compiler's `CheckFBIP.hs` enforces FBIP as a compile error — if a function is annotated as FBIP but has missed reuses, compilation fails. This gives developers a way to guarantee zero-allocation performance for critical code paths.

---

## 09.1 Define `@fbip` Annotation

**Files:** `compiler/ori_ir/src/`, `compiler/ori_parse/src/`, `compiler/ori_types/src/`

- [ ] Add `Fbip` to the function annotation system:
  - Parse `@fbip` attribute on function declarations
  - Store in the `FunctionSig` or `CanFunction` metadata
  - Propagate through canonicalization to `CanExpr`

- [ ] Decide on syntax:
  ```ori
  @fbip
  fn transform(tree: Tree) -> Tree {
      match tree {
          Leaf(x) => Leaf(x + 1)
          Node(l, r) => Node(transform(l), transform(r))
      }
  }
  ```

- [ ] The annotation is a promise by the developer: "this function should allocate zero new heap memory beyond what reset/reuse provides."

---

## 09.2 Wire Enforcement into Pipeline

**File:** `compiler/ori_arc/src/fbip/mod.rs`

- [ ] Modify `analyze_fbip` to return a result:
  ```rust
  pub struct FbipResult {
      /// Missed reuse opportunities that prevent FBIP compliance.
      pub missed_reuses: Vec<MissedReuse>,
      /// Whether the function is FBIP-compliant.
      pub is_compliant: bool,
  }

  pub struct MissedReuse {
      /// The constructor that could have been reused but wasn't.
      pub ctor: CtorInfo,
      /// Why reuse wasn't possible.
      pub reason: MissedReuseReason,
      /// Source span for the diagnostic.
      pub span: Span,
  }

  pub enum MissedReuseReason {
      /// Value escapes to a different context.
      ValueEscapes,
      /// Multiple uses prevent in-place reuse.
      MultipleUses,
      /// Type mismatch (different constructor shapes).
      TypeMismatch,
      /// Cross-function: callee doesn't return reusable token.
      CalleeNotFbip,
  }
  ```

- [ ] In `run_arc_pipeline`, after `analyze_fbip`:
  ```rust
  let fbip_result = analyze_fbip(&func, &classifier);
  if is_fbip_annotated && !fbip_result.is_compliant {
      // Emit compile error with detailed missed-reuse diagnostics
      return Err(fbip_diagnostics(fbip_result));
  }
  ```

---

## 09.3 Diagnostics

**File:** `compiler/ori_arc/src/fbip/mod.rs` or dedicated diagnostic module

- [ ] Create clear error messages for each `MissedReuseReason`:
  ```
  error[E3001]: @fbip function `transform` has missed reuse opportunities
    --> src/tree.ori:5:9
    |
  5 |     Node(l, r) => Node(transform(l), transform(r))
    |                   ^^^^ constructor `Node` allocated fresh
    |
    = help: the matched `Node` at line 4 could be reused, but `r` escapes
            to the recursive call before `l` is consumed
    = note: consider reordering to consume `r` first, or remove @fbip
  ```

- [ ] Include actionable suggestions:
  - "consider reordering operations to enable reuse"
  - "callee `f` is not @fbip — add @fbip to `f` or remove from this function"
  - "value is used multiple times — introduce a clone before the second use"

---

## 09.4 Tests

- [ ] Positive test: `@fbip` function with perfect reuse → compiles successfully
- [ ] Negative test: `@fbip` function with missed reuse → compile error with correct diagnostic
- [ ] Spec test in `tests/spec/annotations/fbip/`:
  - `fbip_tree_transform.ori` — successful FBIP tree transformation
  - `fbip_violation.ori` — expected error for non-compliant function
- [ ] Verify non-annotated functions are unaffected (no change in behavior)
- [ ] Run `./test-all.sh`

---

## 09.5 Completion Checklist

- [ ] `@fbip` annotation parsed and propagated
- [ ] `analyze_fbip` returns structured `FbipResult`
- [ ] Pipeline enforces FBIP on annotated functions
- [ ] Diagnostic messages with source spans and actionable suggestions
- [ ] Positive and negative tests
- [ ] Spec tests
- [ ] Non-annotated functions unaffected
- [ ] `./test-all.sh` passes

**Exit Criteria:** `@fbip fn f(...) = ...` compiles only if `f` achieves full constructor reuse. Violations produce clear diagnostics explaining what reuse was missed and why.
