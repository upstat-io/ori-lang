---
bug: "BUG-04-053"
title: "String comparison bypasses Name interning in rc_insert/annotate.rs"
severity: "medium"
status: complete
goal: "Replace raw string comparisons with pre-interned Name comparisons per impl-hygiene.md §Interning Discipline"
success_criteria:
  - "No interner.lookup() + string equality in annotate.rs for identity checks"
  - "zip/chain/pop compared via Name equality, not string equality"
subsystem: "compiler/ori_arc/src/rc_insert/annotate.rs"
found: "2026-04-09"
source: "tpr-review"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-053 — String comparison bypasses Name interning

**Status:** Complete
**Severity:** medium

## 1. Root Cause Analysis

- **Symptom**: Raw string comparisons in ARC pipeline code (`callee_str == "zip"`)
- **Root cause**: LEAK:scattered-knowledge — `interner.lookup(callee)` converts Name to &str, then string-compares against literals. Bypasses the interning layer.
- **Affected files**: `compiler/ori_arc/src/rc_insert/annotate.rs` — lines 370, 406

## 1.5 Fix Consensus

- **tp-help run**: `/tmp/ori-tpr-AbVasRUT`
- **Codex**: Agrees. Suggests `BuiltinOwnershipSets` as canonical home. Notes `starts_with("ori_")` is prefix check (different class).
- **Gemini**: Same recommendation. Confirms no correctness risk.
- **Decision**: Pre-intern on `ConsumingCtx` (minimal blast radius, O(1) intern for already-interned strings). Moving to `BuiltinOwnershipSets` is valid future cleanup if more names accumulate.

## 3. Implementation

- Added `zip_name`, `chain_name`, `pop_name` fields to `ConsumingCtx`
- Interned at construction via `interner.intern("zip")` etc.
- Replaced `callee_str == "zip" || callee_str == "chain"` with `callee == ctx.zip_name || callee == ctx.chain_name`
- Replaced `callee_str == "pop"` with `callee == ctx.pop_name`
- Removed the two `let callee_str = ctx.interner.lookup(callee);` lines

## 4. Completion

16,964 tests passing. No behavioral change — semantically identical (Name equality ≡ string equality for interned names).
