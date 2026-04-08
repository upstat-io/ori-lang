#!/usr/bin/env python3
"""
Regression tests for plan-annotations.py --fix mode.

These cover the conservative-removal helper `remove_annotations_from_line`
plus the safety predicate `is_safe_id_context`. They are the permanent
regression guard for the comment-stripping logic — every new pattern or
edge case observed in the wild must land here as a test case BEFORE the
pattern is added to the script.

Run: `python3 .claude/skills/impl-hygiene-review/test_plan_annotations_fix.py`

Exit code 0 = all pass, 1 = at least one failure.
"""

from __future__ import annotations

import os
import sys
import types

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))


def _load_module() -> types.ModuleType:
    """Load plan-annotations.py as a module without import-machinery
    name issues (the dash in the filename blocks normal `import`)."""
    mod = types.ModuleType("plan_annotations")
    mod.__file__ = os.path.join(SCRIPT_DIR, "plan-annotations.py")
    sys.modules["plan_annotations"] = mod
    with open(mod.__file__, encoding="utf-8") as f:
        exec(f.read(), mod.__dict__)  # noqa: S102 — module loader
    return mod


m = _load_module()


# (input line, ids on the line, expected result; expected=None means delete)
CASES: list[tuple[str, list[str], str | None]] = [
    # ─── Pattern 1: top-of-file `//!` doc with `ID:` lead ───
    (
        "//! Regression tests for BUG-04-013: AOT wrapper extraction methods must",
        ["BUG-04-013"],
        "//! Regression tests for AOT wrapper extraction methods must",
    ),
    # ─── Pattern 2: section header with decorative dashes ───
    (
        "// ---- TPR-04-021: Debug derive str field leak fix ----",
        ["TPR-04-021"],
        "// ---- Debug derive str field leak fix ----",
    ),
    # ─── Pattern 3: indented `// ID: ...` ───
    (
        "    // TPR-07-008: `ori_iter_drop` consumes the iterator handle (frees",
        ["TPR-07-008"],
        "    // `ori_iter_drop` consumes the iterator handle (frees",
    ),
    # ─── Pattern 4: bracketed at start of comment ───
    (
        "// [TPR-04-005] Fix the foo issue",
        ["TPR-04-005"],
        "// Fix the foo issue",
    ),
    # ─── Pattern 5: parens at end of comment ───
    (
        "// Fix the foo issue (TPR-04-005)",
        ["TPR-04-005"],
        "// Fix the foo issue",
    ),
    # ─── Empty-comment deletion: bare ID is the only content ───
    (
        "// TPR-04-005",
        ["TPR-04-005"],
        None,
    ),
    # ─── Decorative-line deletion: ID was the only content of a header ───
    (
        "// ---- TPR-04-021 ----",
        ["TPR-04-021"],
        None,
    ),
    # ─── String literal: must NOT be touched (not a comment) ───
    (
        'let s = "BUG-04-013 is fake";',
        ["BUG-04-013"],
        'let s = "BUG-04-013 is fake";',
    ),
    # ─── Attribute string: must NOT be touched ───
    (
        '#[ignore = "blocked by BUG-04-013"]',
        ["BUG-04-013"],
        '#[ignore = "blocked by BUG-04-013"]',
    ),
    # ─── Dash-prefix `— ID fix.` (em-dash, end of phrase) ───
    (
        "// Semantic pin: slice strings in tuples — TPR-04-003 fix.",
        ["TPR-04-003"],
        "// Semantic pin: slice strings in tuples",
    ),
    # ─── Dash-suffix `ID — text` (em-dash, mid-sentence) ───
    (
        "// Semantic pin: TPR-04-006 — splitting a substring (slice-backed string) must not crash.",
        ["TPR-04-006"],
        "// Semantic pin: splitting a substring (slice-backed string) must not crash.",
    ),
    # ─── Another dash-prefix variant ───
    (
        "// Semantic pin: derived Clone on struct with slice-string field — TPR-04-005 fix.",
        ["TPR-04-005"],
        "// Semantic pin: derived Clone on struct with slice-string field",
    ),
    # ─── Mid-sentence parens with following period — keep the period ───
    (
        "            // `Project` site (TPR-07-011). Skip here.",
        ["TPR-07-011"],
        "            // `Project` site. Skip here.",
    ),
    # ─── Multi-ID slash list, only first ID stale ───
    (
        "    // TPR-07-016 / TPR-07-017 / TPR-07-019: track which take-project",
        ["TPR-07-016"],
        "    // TPR-07-017 / TPR-07-019: track which take-project",
    ),
    # ─── Multi-ID slash list, all IDs stale ───
    (
        "// TPR-07-016 / TPR-07-017 / TPR-07-019: text",
        ["TPR-07-016", "TPR-07-017", "TPR-07-019"],
        "// text",
    ),
    # ─── Bare ID followed by " fix" (no dash) ───
    (
        "// initial TPR-07-016 fix.",
        ["TPR-07-016"],
        "// initial fix.",
    ),
    # ─── Mid-sentence with sentence boundary period ───
    (
        "/// skipped per TPR-07-016. In-class block params",
        ["TPR-07-016"],
        "/// skipped per In-class block params",
    ),
    # ─── SAFETY: possessive form must NOT be stripped ───
    (
        "// predecessor (bb2 latch). With TPR-07-020's function-entry",
        ["TPR-07-020"],
        "// predecessor (bb2 latch). With TPR-07-020's function-entry",
    ),
    # ─── SAFETY: compound prefix `post-ID` must NOT be stripped ───
    (
        "// post-TPR-07-008. Previously `Borrowed` here was a shadow",
        ["TPR-07-008"],
        "// post-TPR-07-008. Previously `Borrowed` here was a shadow",
    ),
    # ─── SAFETY: slash-continuation `ID/008` must NOT strip ID ───
    (
        "// Semantic pin for TPR-02-006 fix + TPR-02-007/008 refinement.",
        ["TPR-02-006", "TPR-02-007"],
        "// Semantic pin for fix + TPR-02-007/008 refinement.",
    ),
    # ─── Comma-suffixed ID mid-sentence ───
    (
        "/// site handles their drop per TPR-07-011, and emitting an entry-time",
        ["TPR-07-011"],
        "/// site handles their drop per and emitting an entry-time",
    ),
    # ─── Stranded period after parens — line should be deleted ───
    (
        "        // (TPR-03-053).",
        ["TPR-03-053"],
        None,
    ),
    # ─── Parens at start of comment with following sentence ───
    (
        "/// (TPR-01-021). Once a type is computed, it is cached and returned",
        ["TPR-01-021"],
        "/// Once a type is computed, it is cached and returned",
    ),
    # ─── Comma-suffixed ID at start of comment ───
    (
        "/// After TPR-04-042, imported surfaces no longer suppress narrowing",
        ["TPR-04-042"],
        "/// After imported surfaces no longer suppress narrowing",
    ),
    # ─── Stranded "See" reference — delete the whole line ───
    (
        "/// See TPR-07-008.",
        ["TPR-07-008"],
        None,
    ),
    (
        "    // See TPR-04-005.",
        ["TPR-04-005"],
        None,
    ),
    # ─── Indented doc-comment body MUST preserve internal indent ───
    # Doc comments use `///   item` / `///   item` for list formatting;
    # collapsing the internal spaces would be a destructive reformat.
    (
        "///   contains anything to drop. TPR-07-011.",
        ["TPR-07-011"],
        "///   contains anything to drop.",
    ),
]


# Line-number stripping test cases — `(input, expected)`. None means
# the line should be deleted entirely.
LINE_NUMBER_CASES: list[tuple[str, str | None]] = [
    # Single ref mid-comment
    (
        "        // The JIT path checks this in evaluator/compile.rs:383; the AOT path",
        "        // The JIT path checks this in evaluator/compile.rs; the AOT path",
    ),
    # Parenthesized backref
    (
        "    // use-statement span via `e.span.unwrap_or(imp.span)` (imports.rs:210).",
        "    // use-statement span via `e.span.unwrap_or(imp.span)` (imports.rs).",
    ),
    # Multiple refs on one line
    (
        "// ori_map_remove_cow (cow.rs:373 empty sentinel path, cow.rs:391 unique fast path).",
        "// ori_map_remove_cow (cow.rs empty sentinel path, cow.rs unique fast path).",
    ),
    # Line range form `:42-52`
    (
        "    // Collect struct candidates (same pattern as int.rs:42-52).",
        "    // Collect struct candidates (same pattern as int.rs).",
    ),
    # In a string — must NOT be touched
    (
        'let s = "compile.rs:383 is the entry";',
        'let s = "compile.rs:383 is the entry";',
    ),
    # In code (not a comment) — must NOT be touched
    (
        "use foo::bar; // see compile.rs:383",
        "use foo::bar; // see compile.rs",
    ),
]


def main() -> int:
    passed = 0
    failed = 0
    print("=== Annotation strip tests ===")
    for line, ids, expected in CASES:
        result, _changed = m.remove_annotations_from_line(line, ids)
        ok = result == expected
        status = "OK  " if ok else "FAIL"
        print(f"  [{status}] {line!r}")
        if not ok:
            print(f"           got: {result!r}")
            print(f"           exp: {expected!r}")
        if ok:
            passed += 1
        else:
            failed += 1
    print()
    print("=== Line-number strip tests ===")
    for line, expected in LINE_NUMBER_CASES:
        result, _changed = m.strip_line_numbers_in_line(line)
        # If no change happened, strip_line_numbers_in_line returns the
        # original line; the expected value should also be the original.
        ok = result == expected
        status = "OK  " if ok else "FAIL"
        print(f"  [{status}] {line!r}")
        if not ok:
            print(f"           got: {result!r}")
            print(f"           exp: {expected!r}")
        if ok:
            passed += 1
        else:
            failed += 1
    print()
    print(f"{passed}/{passed + failed} test cases passed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
