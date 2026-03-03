---
plan: "macos-arm64-troubleshoot"
title: "macOS ARM64 Troubleshoot: Diagnose and Fix Platform Issues"
status: not-started
references:
  - "plans/llvm-codegen-fixes/"
  - ".github/workflows/ci.yml"
---

# macOS ARM64 Troubleshoot: Diagnose and Fix Platform Issues

## Mission

Get the Ori compiler building and running correctly on macOS ARM64 (Apple Silicon). The immediate blocker is a stack overflow during `ori build` on aarch64 — a trivial program takes 67 seconds before crashing, suggesting infinite recursion in the LLVM codegen path. This plan provides step-by-step commands to run on a Mac to capture diagnostics.

## Architecture

```
User's Mac (ARM64)
  │
  ├─ Section 01: Build environment (LLVM 21, Rust, homebrew)
  │
  ├─ Section 02: Stack overflow diagnosis
  │     ori build smoke.ori
  │       └─ RUST_BACKTRACE=full → find recursion cycle
  │       └─ ORI_DUMP_AFTER_TYPECK / ORI_DUMP_AFTER_LLVM → find what stage
  │       └─ Interpreter path (ori run) → verify it works (isolates codegen)
  │
  └─ Section 03: Platform validation
        cargo test → unit tests
        cargo st → spec tests
        ori build → AOT pipeline
```

## Section Dependency Graph

```
[01 Setup] ──→ [02 Stack Overflow] ──→ [03 Validation]
```

Linear — each section gates the next.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Environment Setup | `section-01-setup.md` | Not Started |
| 02 | Stack Overflow Diagnosis | `section-02-stack-overflow.md` | Not Started |
| 03 | Platform Validation | `section-03-validation.md` | Not Started |
