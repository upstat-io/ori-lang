---
section: "02"
title: "Stack Overflow Diagnosis"
status: not-started
goal: "Identify the exact recursion cycle causing the macOS ARM64 stack overflow during ori build"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Reproduce and Capture Backtrace"
    status: not-started
  - id: "02.2"
    title: "Isolate the Phase"
    status: not-started
  - id: "02.3"
    title: "Narrow the Codegen Path"
    status: not-started
---

# Section 02: Stack Overflow Diagnosis

**Status:** Not Started
**Goal:** Find the exact function cycle causing the stack overflow so it can be fixed on Linux.

**Context:** CI shows `ori build` on macOS ARM64 takes 67 seconds then crashes with
`thread 'main' has overflowed its stack`. The 67s runtime strongly suggests infinite
recursion (a legitimate deep stack would blow instantly). This happens on a trivial
`@main () -> void = print(msg: "hello from AOT");` program.

**Depends on:** Section 01 (working build).

---

## 02.1 Reproduce and Capture Backtrace

Run these commands and **paste the full output** back. The backtrace will show the
repeating call pattern.

```bash
# Create the test program
cat > /tmp/smoke.ori << 'EOF'
@main () -> void = print(msg: "hello from AOT");
EOF

# Attempt 1: Full backtrace (this will take ~60s then crash)
RUST_BACKTRACE=full cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>&1 | tee /tmp/stack-overflow-backtrace.txt

# The backtrace file will be large. Look for the repeating pattern:
grep -c "ori_llvm" /tmp/stack-overflow-backtrace.txt
# High count = recursion is in LLVM codegen

grep -c "ori_types" /tmp/stack-overflow-backtrace.txt
# High count = recursion is in type checker

grep -c "inkwell" /tmp/stack-overflow-backtrace.txt
# High count = recursion is in LLVM API calls

# Show the most frequent function names in the backtrace:
grep -oP '\d+: +\S+' /tmp/stack-overflow-backtrace.txt | awk '{print $2}' | sort | uniq -c | sort -rn | head -20
```

**What to paste back:**
1. The last ~200 lines of `/tmp/stack-overflow-backtrace.txt`
2. The output of the `grep -c` commands
3. The top-20 most frequent functions

---

## 02.2 Isolate the Phase

These commands test each compiler phase independently to find exactly where the recursion starts.

```bash
# A. Does the interpreter work? (no LLVM codegen involved)
cargo run -p oric --bin ori -- run /tmp/smoke.ori
# Expected: prints "hello from AOT" — if this crashes too, the bug is pre-codegen

# B. Does type checking work?
cargo run -p oric --bin ori -- check /tmp/smoke.ori
# Expected: no errors — if this hangs, the bug is in typeck

# C. Dump after each phase to see where it hangs:
# (Run each one separately — if one hangs, that's the phase)

# Parse phase (should be instant):
ORI_DUMP_AFTER_PARSE=1 cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>/tmp/parse-dump.txt &
PHASE_PID=$!
sleep 5
kill $PHASE_PID 2>/dev/null
head -50 /tmp/parse-dump.txt

# Type check phase:
ORI_DUMP_AFTER_TYPECK=1 cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>/tmp/typeck-dump.txt &
PHASE_PID=$!
sleep 5
kill $PHASE_PID 2>/dev/null
head -50 /tmp/typeck-dump.txt

# ARC phase:
ORI_DUMP_AFTER_ARC=1 cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>/tmp/arc-dump.txt &
PHASE_PID=$!
sleep 5
kill $PHASE_PID 2>/dev/null
head -50 /tmp/arc-dump.txt

# LLVM IR phase:
ORI_DUMP_AFTER_LLVM=1 cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>/tmp/llvm-dump.txt &
PHASE_PID=$!
sleep 5
kill $PHASE_PID 2>/dev/null
head -50 /tmp/llvm-dump.txt
```

**What to paste back:**
1. Result of `ori run` (A) — does interpreter work?
2. Result of `ori check` (B) — does typeck work?
3. Which phase dump commands produced output vs which hung
4. First 50 lines of any dump files that were produced

---

## 02.3 Narrow the Codegen Path

If the issue is confirmed in LLVM codegen (most likely), these help narrow further.

```bash
# Try with tracing to see what the codegen is doing:
ORI_LOG=ori_llvm=debug cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>&1 | head -500 | tee /tmp/codegen-trace.txt

# Try with codegen audit (might catch the issue before stack overflow):
ORI_AUDIT_CODEGEN=1 cargo run -p oric --bin ori -- build /tmp/smoke.ori -o /tmp/smoke 2>&1 | head -500 | tee /tmp/codegen-audit.txt

# Check if the issue is in LLVM's optimization passes vs our codegen:
# (--emit-llvm-ir would dump IR before LLVM optimizes — if this works,
#  the issue is in LLVM passes, not our IR generation)
# Note: this flag may not exist yet. If it errors, skip this step.
```

**What to paste back:**
1. First 500 lines of codegen trace
2. First 500 lines of codegen audit
3. Any observations about which log messages repeat

---

## Suspect List (Most to Least Likely)

Based on the CI evidence (67s hang on trivial program, aarch64-only):

1. **SEH catch thunk emission** — The LLVM 21 upgrade added Windows SEH catch
   trampolines (`catch_thunk.rs`). On macOS (which uses Itanium EH, not SEH),
   this code shouldn't run — but if there's a codepath that doesn't properly
   gate on the EH model, it could recurse through personality function setup.

2. **ARC cleanup codegen** — The ARC pipeline generates cleanup code for
   exception handling. If the cleanup code itself triggers more cleanup code
   generation, that's infinite recursion.

3. **LLVM 21 aarch64 backend** — LLVM's own optimization passes running on
   our generated IR might hit a cycle. This would show as `LLVM::` frames
   in the backtrace rather than `ori_llvm::` frames.

4. **Type resolution during codegen** — If monomorphization or type resolution
   during codegen triggers re-entry into the type checker, and that re-entry
   triggers more codegen, that's a cycle.

---

## Completion Checklist

- [ ] Stack overflow reproduced on Mac
- [ ] Backtrace captured and repeating pattern identified
- [ ] Phase isolated (typeck vs ARC vs codegen vs LLVM passes)
- [ ] Root cause function(s) identified
- [ ] Findings pasted back for remote fix
