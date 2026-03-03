---
section: "03"
title: "Platform Validation"
status: not-started
goal: "Verify the full test suite passes on macOS ARM64"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "Rust Unit Tests"
    status: not-started
  - id: "03.2"
    title: "Ori Spec Tests"
    status: not-started
  - id: "03.3"
    title: "AOT Pipeline"
    status: not-started
---

# Section 03: Platform Validation

**Status:** Not Started
**Goal:** Run the full test suite on macOS ARM64 and report any platform-specific failures.

**Context:** Beyond the stack overflow, there may be other aarch64-specific issues.
Known concern: `c_char` is `u8` on aarch64 vs `i8` on x86_64 — this was fixed in
`ori_rt` but there could be remaining spots.

**Depends on:** Section 01 (working build). Can run in parallel with Section 02.

---

## 03.1 Rust Unit Tests

```bash
# Run all Rust tests (includes LLVM unit tests)
cargo test --workspace 2>&1 | tee /tmp/rust-tests.txt

# Count results
grep -E "^test result:" /tmp/rust-tests.txt

# Show any failures
grep -E "(FAILED|panicked)" /tmp/rust-tests.txt
```

**What to paste back:** The `test result:` summary lines and any failures.

---

## 03.2 Ori Spec Tests

```bash
# Run interpreter spec tests
cargo run -p oric --bin ori -- test tests/ 2>&1 | tee /tmp/spec-tests.txt

# Show summary (last 10 lines)
tail -10 /tmp/spec-tests.txt
```

**What to paste back:** The summary line (passed/failed/skipped counts).

---

## 03.3 AOT Pipeline

Only run this if Section 02 has been resolved (or if `ori build` works on your Mac).

```bash
# Try building a simple program
cat > /tmp/aot-test.ori << 'EOF'
@main () -> void = {
    let $x = 42;
    print(msg: str(value: x));
}
EOF

cargo run -p oric --bin ori -- build /tmp/aot-test.ori -o /tmp/aot-test 2>&1 | tee /tmp/aot-build.txt

# If build succeeded, run it
/tmp/aot-test

# Run AOT integration tests
cargo test -p ori_llvm 2>&1 | tee /tmp/llvm-tests.txt
grep -E "^test result:" /tmp/llvm-tests.txt
```

**What to paste back:** Build output, runtime output, and test summary.

---

## Completion Checklist

- [ ] `cargo test --workspace` — report pass/fail counts
- [ ] `ori test tests/` — report pass/fail counts
- [ ] `ori build` — report if AOT works after Section 02 fix
- [ ] Any platform-specific failures documented
