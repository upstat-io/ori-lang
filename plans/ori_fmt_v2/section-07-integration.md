---
section: "07"
title: Integration & Polish
status: not-started
goal: Final integration, spec update, and documentation
sections:
  - id: "07.1"
    title: Spec Update
    status: not-started
  - id: "07.2"
    title: CLI Integration
    status: not-started
  - id: "07.3"
    title: LSP Integration
    status: not-started
  - id: "07.4"
    title: WASM Integration
    status: not-started
  - id: "07.5"
    title: Performance
    status: not-started
  - id: "07.6"
    title: Documentation
    status: not-started
---

# Section 07: Integration & Polish

**Status:** 📋 Planned
**Goal:** Final integration with CLI/LSP/WASM, spec updates, and documentation

---

## 07.1 Spec Update

Update formatting spec for ChainedElseIfRule (Kotlin style).

- [ ] **Update** `docs/ori_lang/0.1-alpha/spec/16-formatting.md`
  - Lines 432-436: Change from current style to Kotlin style

**Current spec (to replace):**
```ori
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

**New spec (Kotlin style):**
```ori
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

- [ ] **Update** any related examples in spec
- [ ] **Run** `./sync-spec` to propagate changes
- [ ] **Update** CLAUDE.md formatting section if needed

---

## 07.2 CLI Integration

Ensure CLI uses new layered formatter.

- [ ] **Verify** `ori fmt` uses new formatter
  - [ ] `ori fmt <file>` — single file
  - [ ] `ori fmt <directory>` — recursive
  - [ ] `ori fmt .` — current directory
  - [ ] `ori fmt --check` — check mode
  - [ ] `ori fmt --diff` — show diff
  - [ ] `ori fmt --stdin` — stdin/stdout

- [ ] **Verify** error handling
  - [ ] Parse errors show source snippets
  - [ ] Configuration errors are clear

- [ ] **Test** `.orifmtignore` still works
- [ ] **Test** `--no-ignore` flag

---

## 07.3 LSP Integration

Ensure LSP server uses new formatter.

> **Note:** Full LSP is in Section 22.2 of roadmap. Here we ensure formatter integration works.

- [ ] **Verify** `textDocument/formatting` works
- [ ] **Verify** `textDocument/rangeFormatting` works
- [ ] **Test** format-on-save in supported editors

---

## 07.4 WASM Integration

Ensure WASM playground uses new formatter.

> **Note:** WASM playground is in Section 22.8 of roadmap.

- [ ] **Verify** `format_ori()` WASM export works
- [ ] **Test** formatting in playground UI
- [ ] **Test** URL-shared code formats correctly

---

## 07.5 Performance

Ensure no performance regression.

- [ ] **Benchmark** single file formatting
  - Target: No regression from current
  - Small file (<100 lines): < 1ms
  - Medium file (100-1000 lines): < 10ms
  - Large file (10k lines): < 100ms

- [ ] **Benchmark** directory formatting
  - Parallel processing still works
  - No regression from 2.4x speedup

- [ ] **Profile** memory usage
  - No increase in peak memory
  - Large files stay memory-efficient

- [ ] **Test** incremental formatting
  - Still provides ~30% speedup

---

## 07.6 Documentation

Update documentation for new architecture.

- [ ] **Update** `docs/tooling/formatter/` user guide
  - Explain formatting rules at high level
  - Document any new behavior

- [ ] **Create** `compiler/ori_fmt/README.md`
  - Architecture overview (5 layers)
  - How to add new rules
  - How to modify existing rules

- [ ] **Document** code comments
  - Each layer module has doc comments
  - Each rule struct is documented
  - Public API is fully documented

- [ ] **Update** `compiler/ori_fmt/src/lib.rs` crate docs
  - Overview of layered architecture
  - Entry points for users

---

## 07.7 Migration Checklist

Verify clean migration from old to new.

- [ ] **All existing tests pass**
  - Golden tests unchanged (except ChainedElseIfRule)
  - Property tests pass
  - Integration tests pass

- [ ] **Code review**
  - No duplicate logic between layers
  - Clear separation of concerns
  - No dead code from old implementation

- [ ] **Archive old code**
  - If significant, move to `_archive/` with notes
  - Otherwise, delete cleanly

---

## 07.8 Completion Checklist

- [ ] Spec updated for ChainedElseIfRule
- [ ] CLI integration verified
- [ ] LSP integration verified
- [ ] WASM integration verified
- [ ] Performance benchmarks pass
- [ ] Documentation complete
- [ ] Migration clean

**Exit Criteria:** The layered formatter architecture is fully integrated, documented, and performs at least as well as the previous implementation.

---

## Plan Completion

When Section 07 is complete, the ori_fmt_v2 plan is finished:

1. ✅ Layer 1: Token Spacing Rules
2. ✅ Layer 2: Container Packing
3. ✅ Layer 3: Shape Tracking
4. ✅ Layer 4: Breaking Rules
5. ✅ Layer 5: Formatter Orchestration
6. ✅ Testing & Validation
7. ✅ Integration & Polish

**Final Verification:**
- [ ] Target example formats correctly (see 00-overview.md)
- [ ] All 8 breaking rules demonstrated
- [ ] `./test-all` passes
- [ ] `./fmt-all --check` passes
