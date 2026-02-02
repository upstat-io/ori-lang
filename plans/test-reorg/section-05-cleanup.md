---
section: 5
title: Cleanup
status: not-started
goal: Remove empty test blocks, update CI, update documentation
sections:
  - id: "5.1"
    title: Remove Empty Test Modules
    status: not-started
  - id: "5.2"
    title: Update CI Configuration
    status: not-started
  - id: "5.3"
    title: Update Documentation
    status: not-started
  - id: "5.4"
    title: Final Verification
    status: not-started
---

# Section 5: Cleanup

**Status:** 📋 Planned
**Goal:** Remove empty test blocks, update CI, update documentation

---

## 5.1 Remove Empty Test Modules

- [ ] Find all source files with empty or near-empty `mod tests` blocks:
  ```bash
  grep -r "mod tests" compiler/ --include="*.rs" -l
  ```

- [ ] For each file:
  - [ ] If `mod tests` is empty (no tests), remove the block entirely
  - [ ] If `mod tests` has < 5 trivial tests, consider keeping inline
  - [ ] Ensure no orphaned `#[cfg(test)]` attributes

- [ ] Run full test suite to verify no breakage:
  ```bash
  ./test-all
  ```

---

## 5.2 Update CI Configuration

- [ ] Update `.github/workflows/test.yml` to run phase tests:
  ```yaml
  - name: Run phase tests
    run: |
      cargo test --test phases

  - name: Run spec tests (interpreter)
    run: |
      cargo st tests/spec/

  - name: Run spec tests (LLVM)
    run: |
      ./target/release/ori test --backend=llvm tests/spec/
  ```

- [ ] Add test naming convention enforcement (optional):
  - [ ] Ensure phase test files follow `{category}.rs` naming
  - [ ] Ensure test functions follow `test_{description}` naming

- [ ] Verify CI passes on all platforms:
  - [ ] Linux
  - [ ] macOS
  - [ ] Windows (if supported)

---

## 5.3 Update Documentation

- [ ] Update CLAUDE.md testing guidelines:
  - [ ] Add reference to `tests/phases/` for phase tests
  - [ ] Clarify when to use `tests/spec/` vs `tests/phases/`
  - [ ] Document the 200-line inline test limit

- [ ] Add README to `tests/phases/`:
  ```markdown
  # Phase Tests

  Tests organized by compiler phase.

  ## Structure
  - `parse/` — Lexer and parser tests
  - `typeck/` — Type system tests
  - `eval/` — Interpreter tests
  - `codegen/` — LLVM backend tests
  - `common/` — Shared test utilities

  ## When to Use
  - **Phase tests**: Testing compiler internals, edge cases, implementation details
  - **Spec tests**: Testing user-facing behavior, language features

  ## Running
  ```bash
  cargo test --test phases          # All phase tests
  cargo test --test phases parse    # Just parse phase
  cargo test --test phases codegen  # Just codegen phase
  ```
  ```

- [ ] Document test helper usage in `tests/phases/common/README.md`

---

## 5.4 Final Verification

- [ ] Run full test suite:
  ```bash
  ./test-all
  ```

- [ ] Verify no modules exceed 200 lines of inline tests:
  ```bash
  # Script to check inline test sizes
  for file in $(find compiler -name "*.rs"); do
    lines=$(sed -n '/mod tests/,/^}/p' "$file" | wc -l)
    if [ "$lines" -gt 200 ]; then
      echo "VIOLATION: $file has $lines lines of inline tests"
    fi
  done
  ```

- [ ] Verify all phase tests run successfully:
  ```bash
  cargo test --test phases
  ```

- [ ] Create summary report:
  - [ ] Total inline test modules remaining
  - [ ] Total inline test lines remaining
  - [ ] Phase test coverage by phase
  - [ ] Any remaining violations

---

## Completion Checklist

- [ ] All empty `mod tests` blocks removed
- [ ] CI updated and passing
- [ ] CLAUDE.md updated
- [ ] `tests/phases/README.md` created
- [ ] `tests/phases/common/README.md` created
- [ ] Full test suite passing
- [ ] No modules > 200 lines of inline tests
- [ ] Summary report generated

**Exit Criteria:** Clean codebase with no violations; all documentation updated; CI passing; clear guidelines for future test placement.
