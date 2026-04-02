---
section: "07"
title: "Tooling & CLI"
status: open
goal: "Track and resolve all known tooling and CLI bugs"
sections: []
---

# Section 07: Tooling & CLI

**Subsystem:** `compiler/oric/`, `compiler/ori_fmt/`, `compiler/ori_diagnostic/`

Bugs in the CLI (`ori run`, `ori check`, `ori test`, `ori fmt`), formatter, diagnostic output, test runner, and build tooling.

---

## Open Bugs

- [x] `[BUG-07-001][medium]` **`--target` with missing/invalid value should show valid targets** — found by manual.
  Resolved: OBE on 2026-04-02. Target validation now active — `ori build --target=foo` emits `error[E5004]: target 'foo' is not supported` with full list of supported targets.
  Repro: `ori build hello.ori --target=` or `ori build hello.ori --target=foo`
  Expected: error listing valid targets (from `SUPPORTED_TARGETS` / `list_targets()`)
  Actual: proceeds with invalid target, fails cryptically at link time
  Subsystem: `compiler/oric/src/commands/build_options/parse_args.rs` (line 20 — accepts any string), `compiler/oric/src/commands/targets/mod.rs` (has `list_targets()` and `SUPPORTED_TARGETS`)
  Found: 2026-03-28 | Source: manual
  Note: Related to BUG-04-001 (cross-compilation failure). Early validation here would prevent the confusing linker error.

- [ ] `[BUG-07-002][medium]` **`dual-exec-verify.sh` exits 0 with zero verified tests** — found by tpr-review.
  Repro: `./diagnostics/dual-exec-verify.sh tests/spec/repr/float_narrowing/` reports "LLVM coverage gap: 36, Verified: 0, Total verified: 0/0 (0%)" then prints "DUAL-EXECUTION: ALL VERIFIED" and exits 0.
  Expected: Zero-coverage runs should either warn or exit non-zero — a "verified" result with 0 verifications is misleading.
  Subsystem: `diagnostics/dual-exec-verify.sh`
  Found: 2026-03-29 | Source: tpr-review (TPR-05-016)

---

## Resolved Bugs

- None.
