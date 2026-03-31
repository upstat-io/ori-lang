---
section: "04"
title: "Codegen & LLVM"
status: open
goal: "Track and resolve all known codegen/LLVM bugs"
sections: []
---

# Section 04: Codegen & LLVM

**Subsystem:** `compiler/ori_llvm/`, `compiler/ori_arc/`

Bugs in LLVM IR generation, JIT/AOT compilation, monomorphization, ARC pipeline lowering, type lowering, and optimization.

---

## Open Bugs

- [ ] `[BUG-04-001][high]` **Cross-compilation to Windows fails: host linker used instead of cross-linker** — found by manual.
  Repro: `ori build hello.ori --target=x86_64-pc-windows-msvc` on Linux host
  Error: `R_AMD64_IMAGEBASE with __ImageBase undefined` — GNU ld receives Windows COFF object
  Root cause: `LinkerFlavor::for_target()` correctly selects `Msvc`, but `LinkerDetection::is_available()` fails (no `link.exe`/`lld-link` on Linux), fallback cascades to host `cc`. Additionally, Linux-compiled `libori_rt.a` is linked with Linux system libraries (`-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`). Three issues: (1) no validation that cross-linker exists before attempting cross-compile, (2) no cross-compiled runtime for target, (3) system library selection ignores target OS.
  Subsystem: `compiler/ori_llvm/src/aot/linker/driver.rs`, `mod.rs` (fallback logic), `gcc.rs` (system libs)
  Found: 2026-03-28 | Source: manual
  Note: Also applies to `--target=x86_64-pc-windows-gnu` (needs `x86_64-w64-mingw32-gcc`).

- [ ] `[BUG-04-003][high]` **Trait impl methods that access `self` struct fields produce LLVM verification errors in AOT** — found by continue-roadmap.
  Repro: `type Box = { w: int, h: int }` with `impl Printable for Box { @to_str (self) -> str = \`{self.w}x{self.h}\`; }` — LLVM verification: "Call parameter type does not match function signature!" Codegen extracts field 0 and passes it as the `self` parameter instead of passing the whole struct. Inherent impl methods with field access work fine; only trait impl methods are affected.
  Subsystem: `compiler/ori_llvm/src/codegen/` — `compile_impls()` trait method calling convention
  Found: 2026-03-28 | Source: continue-roadmap
  Note: Active work in roadmap section 03 (traits) and 21A (LLVM backend) touches this area.

- [x] `[BUG-04-004][high]` **AOT test `test_arc_loop_allocation` fails with exit code 1** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Same stale release binary pattern as BUG-04-002 — a fresh `cargo build` during §06 work rebuilt the release binary, and all 4 AOT tests now pass (14,584 total, 0 failures).

- [x] `[BUG-04-005][critical]` **AOT test `test_aot_derive_eq_mixed_types` segfaults (exit code -139)** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Stale release binary — same root cause as BUG-04-004.

- [x] `[BUG-04-006][high]` **Derived comparison codegen uses `icmp` on narrowed float fields** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Stale release binary — same root cause as BUG-04-004.

- [x] `[BUG-04-007][high]` **AOT test `test_float_narrowed_mixed_exact_non_exact` fails with exit code 1** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Stale release binary — same root cause as BUG-04-004.

- [ ] `[BUG-04-008][high]` **Zero-sized enum payload mismatch: `A(()) | B` triggers build_struct error and inconsistent sizing** — found by tpr-review.
  Repro: `type E = A(u: ()) | B; @main () -> int = { let x = A(()); 0 }` — hits `build_struct called with non-struct LLVM type ... i64`
  Root cause: LLVM `resolve_enum()` resolves `Idx::UNIT` to an LLVM type (likely `i64` representation) and computes non-zero payload, while `pool_type_store_size()` in `ori_arc` returns 0 for Unit, and `compute_enum_payload_layout()` in `ori_repr` also returns 0 for Unit fields. The variant `A(())` is treated as all-unit by Pool-level sizing but as payload variant by LLVM resolution. Three-way disagreement: ori_arc=0, ori_repr=0, LLVM=8.
  Subsystem: `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs` (resolve_enum), `compiler/ori_arc/src/lower/control_flow/type_layout.rs` (pool_type_store_size), `compiler/ori_repr/src/canonical/type_repr.rs` (canonical_enum)
  Found: 2026-03-30 | Source: tpr-review
  Note: Active work in repr-opt section-07 (enum repr) touches this area. Also affects `A(Never) | B`.

---

## Resolved Bugs

- [x] `[BUG-04-002][critical]` **Inherent impl method returns wrong value when type also has trait impl** — found by manual.
  Resolved: OBE on 2026-03-28. False positive — caused by stale release binary from prior session. After `cargo b --release` (force rebuild), `test_aot_multiple_impl_blocks` passes. The AOT test framework falls back to the release binary when debug lacks LLVM; the stale release binary had code from before range analysis field narrowing was fixed.

- None.
