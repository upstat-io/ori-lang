---
section: "02"
title: "Codegen Integration"
status: complete
goal: "All Itanium LLVM IR emitted by Ori references @ori_eh_personality, with consistent runtime declarations, EH model selection, and JIT symbol mapping"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Runtime Declaration Table"
    status: complete
  - id: "02.2"
    title: "EH Model Personality Selection"
    status: complete
  - id: "02.3"
    title: "JIT Symbol Mapping"
    status: complete
  - id: "02.4"
    title: "Test Fixtures and Residual References"
    status: complete
  - id: "02.5"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Codegen Integration

**Context:** `rust_eh_personality` is currently hardcoded in multiple real code paths, not just one declaration site. Today the relevant production/test locations are:

1. `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` (RT function declaration)
2. `compiler/ori_llvm/src/codegen/eh_model/mod.rs` (`EhModel::Itanium.personality_name()`)
3. `compiler/ori_llvm/src/codegen/eh_model/tests.rs` (expected personality test)
4. `compiler/ori_llvm/src/evaluator/runtime_mappings.rs` (JIT symbol address mapping + helper)
5. `compiler/ori_llvm/src/verify/tests.rs` (IR verifier fixture)

`arc_emitter/mod.rs` already uses `eh_model.personality_name()`, so updating the EH model layer is the correct integration point.

**Depends on:** Section 01 (`ori_eh_personality` symbol exists in `ori_rt`).

---

## 02.1 Runtime Declaration Table

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

- [x] Rename the Itanium EH personality declaration: (2026-03-03)
  ```rust
  // Before
  RtFn {
      name: "rust_eh_personality",
      params: &[Ty::I32],
      ret: Some(Ty::I32),
      attrs: &[Attr::Nounwind],
      jit_allowed: true,
  }

  // After
  RtFn {
      name: "ori_eh_personality",
      params: &[Ty::I32],
      ret: Some(Ty::I32),
      attrs: &[Attr::Nounwind],
      jit_allowed: true,
  }
  ```

- [x] Update nearby comments that still describe JIT Itanium behavior as `rust_eh_personality`. (2026-03-03)

Notes:
- Keep `jit_allowed: true` for Itanium personality.
- Keep `__CxxFrameHandler3` declaration unchanged for SEH.
- Keep `Attr::Nounwind` for personality declarations.

---

## 02.2 EH Model Personality Selection

**File(s):** `compiler/ori_llvm/src/codegen/eh_model/mod.rs`, `compiler/ori_llvm/src/codegen/eh_model/tests.rs`

- [x] Update Itanium personality selection in `EhModel::personality_name()`: (2026-03-03)
  ```rust
  // Before
  Self::Itanium => "rust_eh_personality",

  // After
  Self::Itanium => "ori_eh_personality",
  ```

- [x] Update doc comments in `eh_model/mod.rs` to describe `ori_eh_personality` on Itanium. (2026-03-03)

- [x] Update `eh_model/tests.rs` expectation: (2026-03-03)
  ```rust
  assert_eq!(EhModel::Itanium.personality_name(), "ori_eh_personality");
  ```

Why this is critical:
- `arc_emitter/mod.rs` resolves personality through `eh_model.personality_name()`. If this is not updated, emitted IR will still point at `rust_eh_personality` even if RT declarations change.

---

## 02.3 JIT Symbol Mapping

**File(s):** `compiler/ori_llvm/src/evaluator/runtime_mappings.rs`

- [x] Replace mapping arm: (2026-03-03)
  ```rust
  // Before
  "rust_eh_personality" => rust_eh_personality_addr(),

  // After
  "ori_eh_personality" => runtime::ori_eh_personality_addr(),
  ```

- [x] Delete obsolete helper: (2026-03-03)
  ```rust
  fn rust_eh_personality_addr() -> usize { ... }
  ```

- [x] Update mapping comments to reference `ori_rt/src/eh_personality.c` instead of Rust std personality internals. (2026-03-03)

Validation invariant:
- `jit_symbol_mappings_match_jit_allowed` (runtime decl tests) must remain green; the name in RT declarations and mapping must stay aligned.

---

## 02.4 Test Fixtures and Residual References

**File(s):** `compiler/ori_llvm/src/verify/tests.rs`, plus audit across `compiler/ori_llvm/src`

- [x] Update verifier fixture personality declaration: (2026-03-03)
  ```rust
  let personality = module.add_function("ori_eh_personality", personality_type, None);
  ```

- [x] Remove or rewrite stale comments mentioning `rust_eh_personality` in touched files. (2026-03-03)

- [x] Audit remaining references: (2026-03-03) — 0 matches in `compiler/ori_llvm/src`
  ```bash
  rg -n "rust_eh_personality" compiler/ori_llvm/src
  ```
  Expected after this section: 0 matches in `compiler/ori_llvm/src`.

---

## 02.5 Completion Checklist

- [x] `rg -n "rust_eh_personality" compiler/ori_llvm/src` returns 0 matches (2026-03-03)
- [x] `rg -n "ori_eh_personality" compiler/ori_llvm/src` shows hits in: (2026-03-03)
  - `codegen/runtime_decl/runtime_functions.rs`
  - `codegen/eh_model/mod.rs`
  - `codegen/eh_model/tests.rs`
  - `evaluator/runtime_mappings.rs`
  - `verify/tests.rs`
- [x] `ORI_DUMP_AFTER_LLVM=1 ori check tests/spec/functions/recursion.ori 2>&1 | grep personality` (2026-03-03)
  N/A — no current codegen emits invoke/landingpad (personality only attached to functions with unwind blocks). Wiring verified via code path: `eh_model.personality_name()` → `"ori_eh_personality"`. Will produce output after Section 03 (exception raise) is implemented.
- [x] `cargo test -p ori_llvm codegen::eh_model::tests` passes (2026-03-03) — 7/7
- [x] `cargo test -p ori_llvm codegen::runtime_decl::tests::jit_symbol_mappings_match_jit_allowed` passes (2026-03-03)
- [x] `cargo build -p ori_llvm` succeeds (2026-03-03)

**Exit Criteria:** Itanium personality resolution is consistently `ori_eh_personality` across runtime declarations, EH model selection, JIT mappings, and verifier fixtures, with zero residual `rust_eh_personality` references in `compiler/ori_llvm/src`.
