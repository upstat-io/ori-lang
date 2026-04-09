#!/usr/bin/env python3
"""
hygiene-lint.py — Static analysis for Ori compiler hygiene rules.

Runs deterministic, pattern-based hygiene checks that don't require
AI judgment. Complements the /impl-hygiene-review skill by handling
the mechanical surface-level checks, freeing AI context for LEAK,
SSOT, and algorithmic DRY analysis.

Checks (--check <id> to run specific ones, comma-separated):

  file-length      Files > 500 lines (test files exempt)
  fn-length        Functions > 100 lines (target < 50)
  nesting-depth    Brace nesting > 4 levels in function bodies
  test-ephemeral   Ephemeral IDs in test function names
  test-weak        Weak descriptors in test function names
  unsafe-safety    unsafe blocks without // SAFETY: comment
  banners          Decorative comment banners (// ===, // ---)  [fixable]
  commented-code   Commented-out Rust code in comments           [fixable]
  bare-allow       #[allow(clippy::...)] without reason
  println-in-lib   println!/eprintln! in library crates
  bare-todo        // TODO without (phase): format
  unwrap-prod      .unwrap() in prod code without justification
  catch-all-arms   _ => unreachable!()/todo!() in match arms
  string-identity  String equality == "..." in non-test prod code
  result-unit      Result<T, ()> that should be Option<T>
  glob-imports     use foo::* outside #[cfg(test)] modules
  missing-docs     pub items without /// documentation
  lib-bodies       lib.rs files containing function bodies

Modes:

  (default)        Run all checks, report findings grouped by check
  --check X,Y      Run only specified checks
  --scope P [P..]  Restrict to given paths (default: compiler/)
  --fix            Preview auto-fixes for fixable findings (dry-run)
  --apply          With --fix, write changes to disk
  --json           Machine-readable JSON output
  --summary        Summary counts only, no details
  --no-color       Disable color output

Exit codes: 0 = clean, 1 = findings, 2 = fix preview (--fix w/o --apply)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

REPO_ROOT = Path(__file__).resolve().parents[3]
COMPILER_DIR = REPO_ROOT / "compiler"

# ─── Color helpers ───────────────────────────────────────────

_use_color = sys.stdout.isatty()

def _c(code: str, text: str) -> str:
    return f"\033[{code}m{text}\033[0m" if _use_color else text

def red(t: str) -> str: return _c("31", t)
def yellow(t: str) -> str: return _c("33", t)
def green(t: str) -> str: return _c("32", t)
def cyan(t: str) -> str: return _c("36", t)
def bold(t: str) -> str: return _c("1", t)
def dim(t: str) -> str: return _c("2", t)


# ─── Check registry ─────────────────────────────────────────

# (category, severity, auto_fixable, description)
# Only project-specific checks that clippy/rust-analyzer cannot do.
# Clippy handles: unwrap_used, missing_docs, result_unit_err, panic,
# todo, dbg_macro, wildcard_imports, allow_attributes_without_reason,
# undocumented_unsafe_blocks, print_stdout/print_stderr.
# Those are NOT duplicated here — clippy runs on every commit via lefthook.
CHECK_DEFS: dict[str, tuple[str, str, bool, str]] = {
    "file-length":     ("BLOAT",    "minor", False, "Files exceeding 500-line limit"),
    "fn-length":       ("BLOAT",    "minor", False, "Functions exceeding 100-line limit"),
    "nesting-depth":   ("BLOAT",    "minor", False, "Brace nesting exceeding 4 levels"),
    "test-ephemeral":  ("NAMING",   "major", False, "Ephemeral IDs in test function names"),
    "test-weak":       ("NAMING",   "minor", False, "Weak descriptors in test function names"),
    "banners":         ("BLOAT",    "minor", True,  "Decorative comment banners"),
    "commented-code":  ("BLOAT",    "minor", False, "Commented-out Rust code (review needed)"),
    "bare-todo":       ("BLOAT",    "minor", False, "// TODO without (phase): format"),
    "catch-all-arms":  ("GAP",      "major", False, "_ => unreachable!()/todo!() catch-all"),
    "string-identity": ("LEAK",     "minor", False, "String equality bypassing interning"),
    "lib-bodies":      ("BLOAT",    "minor", False, "lib.rs containing function bodies"),
    "deny-unsafe":     ("LEAK",     "major", False, "Pure crate missing #![deny(unsafe_code)]"),
    "ignore-tracking": ("BLOAT",    "minor", False, "#[ignore] without tracking artifact"),
    "phase-bleeding":  ("LEAK",     "critical", False, "Crate imports from higher-level crate"),
    "swallowed-error": ("LEAK",     "minor", False, "Error silently swallowed"),
}

ALL_CHECK_IDS = set(CHECK_DEFS.keys())


# Pure crates that MUST have #![deny(unsafe_code)]
PURE_CRATES = {
    "ori_ir", "ori_diagnostic", "ori_types", "ori_eval", "ori_patterns",
    "ori_parse", "ori_lexer", "ori_registry",
}

# Crate dependency order (lower number = lower level).
# A crate may only import from crates with LOWER numbers.
CRATE_ORDER: dict[str, int] = {
    "ori_ir": 0, "ori_diagnostic": 0, "ori_registry": 0,
    "ori_lexer_core": 1, "ori_lexer": 2,
    "ori_parse": 3,
    "ori_patterns": 4, "ori_types": 4,
    "ori_eval": 5, "ori_arc": 5, "ori_repr": 5, "ori_canon": 5,
    "ori_llvm": 6, "ori_compiler": 6,
    "ori_fmt": 4, "ori_lsp": 7, "ori_stack": 1,  # shared utility, used by ori_parse+
    "oric": 8, "ori_rt": 0,
}

# Sync-point enums — shared between catch-all-arms check and enum-drift.py
# These are the IR enums where catch-all `_ =>` arms hide cross-phase drift
SYNC_POINT_ENUMS = {
    "CanExpr", "ExprKind", "TypeTag", "DerivedTrait",
    "TokenKind", "CollectionMethod", "IteratorValue",
    "StmtKind", "BinaryOp", "UnaryOp",
}


# ─── Finding ─────────────────────────────────────────────────

@dataclass
class Finding:
    check_id: str
    file: str       # relative to REPO_ROOT
    line: int
    message: str
    category: str = ""
    severity: str = "minor"
    auto_fixable: bool = False

    def __post_init__(self):
        if self.check_id in CHECK_DEFS:
            cat, sev, fix, _ = CHECK_DEFS[self.check_id]
            self.category = cat
            self.severity = sev
            self.auto_fixable = fix

    def to_dict(self) -> dict:
        return {
            "check": self.check_id,
            "file": self.file,
            "line": self.line,
            "message": self.message,
            "category": self.category,
            "severity": self.severity,
            "auto_fixable": self.auto_fixable,
        }


# ─── File discovery ──────────────────────────────────────────

def is_test_file(path: Path) -> bool:
    """Test files are exempt from file-length limit."""
    name = path.name
    if name == "tests.rs" or name.endswith("_test.rs"):
        return True
    # Files under a tests/ directory (e.g., compiler/ori_llvm/tests/)
    return "tests" in path.parts



def discover_files(scope_paths: list[Path]) -> list[Path]:
    """Find all .rs files in scope, excluding target/ and .git/."""
    files: list[Path] = []
    for scope in scope_paths:
        if scope.is_file() and scope.suffix == ".rs":
            files.append(scope.resolve())
        elif scope.is_dir():
            for f in scope.rglob("*.rs"):
                s = str(f)
                if "/target/" in s or "/.git/" in s:
                    continue
                files.append(f.resolve())
    return sorted(set(files))

def rel_path(path: Path) -> str:
    """Path relative to repo root for display."""
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


# ─── Regex patterns ──────────────────────────────────────────

# Ephemeral IDs in test names (underscore form)
EPHEMERAL_TEST_RE = re.compile(
    r"fn\s+(test_\w*(?:"
    r"tpr_\d+_\d+|bug_\d+_\d+|cross_\d+_\d+|"
    r"section_\d+|§\d+|phase_[a-z]|"
    r"roadmap_\d+|issue_\d+|fix_\d+|"
    r"\d{4}_\d{2}_\d{2}"  # dates
    r")\w*)"
    r"\s*\(",
    re.IGNORECASE,
)

# Weak test name descriptors
WEAK_TEST_RE = re.compile(
    r"fn\s+(test_(?:\w+_)?(?:"
    r"works|works_correctly|it_works|"
    r"basic|simple|default|normal|"
    r"correct|valid|ok|sanity|"
    r"handles_\w+|check_\w+|verify_\w+"
    r"))\s*\(",
)

# Bare unit test names (test_iterator, test_parse, test_eval, etc.)
BARE_TEST_RE = re.compile(
    r"fn\s+(test_(?:iterator|parse|eval|collect|clone|debug|display|"
    r"hash|compare|drop|new|default|from|into|index|len|"
    r"empty|push|pop|insert|remove|get|set|add|sub|mul|div|fmt"
    r"))\s*\("
)

# Decorative banners
BANNER_RE = re.compile(r"^\s*//\s*[═━─=\-\*~]{3,}\s*$")

# Commented-out Rust code (conservative heuristic).
# Requires STRONG code signals — a keyword alone is not enough because
# words like "for", "return", "if", "match", "use", "type" appear in
# English prose. We require either:
#   - keyword + syntax structure (e.g., `fn name(`, `let x =`, `for x in`)
#   - method chain (`.unwrap()`, `.map(`)
#   - closing brace alone on a line
#   - attribute (`#[`)
COMMENTED_CODE_RE = re.compile(
    r"^\s*//\s*"
    r"(?:"
    r"fn\s+\w+\s*[\(<]|"           # fn name( or fn name<
    r"let\s+(?:mut\s+)?\w+\s*[=:]|"  # let x = or let x:
    r"if\s+\w+\s*[.!=<>({]|"       # if x. or if x == or if x {
    r"else\s*\{|"                   # else {
    r"match\s+\w+|"                 # match expr
    r"for\s+\w+\s+in\s|"           # for x in (not "for their memory")
    r"while\s+\w+\s*[.!=<>({]|"    # while x. or while x ==
    r"struct\s+\w+|"               # struct Name
    r"enum\s+\w+|"                 # enum Name
    r"impl\s+\w+|"                 # impl Name
    r"use\s+\w+::|"               # use crate:: (not "use the tool")
    r"pub\s+(?:fn|struct|enum|mod|type|trait|use|const)\s|"
    r"mod\s+\w+\s*[;{]|"          # mod name; or mod name {
    r"type\s+\w+\s*[=<]|"         # type Name = or type Name<
    r"trait\s+\w+|"               # trait Name
    r"return\s+\w+[;.(]|"         # return expr; (not "return value without")
    r"break\s*;|continue\s*;|"    # break; continue;
    r"\.unwrap\(\)|\.expect\(|\.map\(|\.filter\(|\.collect\(|"
    r"\}\s*$|"                     # } alone
    r"#\["                         # attribute
    r")"
)

# Bare #[allow(clippy::...)] — no reason attribute

# println!/eprintln! (but not in macros defining them)

# Bare TODO without (phase): format
BARE_TODO_RE = re.compile(r"//\s*TODO(?!\()")

# .unwrap() — simple detection

# Catch-all match arms
CATCH_ALL_RE = re.compile(r"_\s*=>\s*(?:unreachable|todo)!\s*\(")

# String identity in non-test code: == "..."

# lib.rs function bodies
LIB_FN_BODY_RE = re.compile(r"^\s*(?:pub\s+)?fn\s+\w+")

# Function start (for fn-length and nesting-depth)
FN_START_RE = re.compile(
    r"^\s*(?:pub(?:\((?:crate|super)\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?"
    r"fn\s+(\w+)"
)


# ─── Check implementations ───────────────────────────────────

def check_file_length(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: files > 500 lines (test files exempt)."""
    if is_test_file(path):
        return []
    count = len(lines)
    if count > 500:
        return [Finding(
            "file-length", rel_path(path), 1,
            f"{count} lines (limit: 500, over by {count - 500})",
        )]
    return []


def check_fn_length(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: functions > 100 lines."""
    findings: list[Finding] = []
    i = 0
    while i < len(lines):
        m = FN_START_RE.match(lines[i])
        if m:
            fn_name = m.group(1)
            fn_start = i
            # Find the opening brace
            depth = 0
            found_open = False
            j = i
            while j < len(lines):
                for ch in lines[j]:
                    if ch == '{':
                        depth += 1
                        found_open = True
                    elif ch == '}':
                        depth -= 1
                if found_open and depth == 0:
                    fn_end = j
                    fn_lines = fn_end - fn_start + 1
                    if fn_lines > 100:
                        findings.append(Finding(
                            "fn-length", rel_path(path), fn_start + 1,
                            f"fn {fn_name}: {fn_lines} lines (limit: 100)",
                        ))
                    i = j + 1
                    break
                j += 1
            else:
                i += 1
        else:
            i += 1
    return findings


def check_nesting_depth(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: brace nesting > 4 levels within function bodies."""
    findings: list[Finding] = []
    i = 0
    while i < len(lines):
        m = FN_START_RE.match(lines[i])
        if m:
            fn_name = m.group(1)
            depth = 0
            max_depth = 0
            max_depth_line = i
            found_open = False
            j = i
            while j < len(lines):
                for ch in lines[j]:
                    if ch == '{':
                        depth += 1
                        found_open = True
                        if depth > max_depth:
                            max_depth = depth
                            max_depth_line = j
                    elif ch == '}':
                        depth -= 1
                if found_open and depth == 0:
                    # Function-level brace is depth 1, so nesting > 4
                    # means depth > 5 (fn { if { for { match { if { ... } } } } })
                    if max_depth > 5:
                        findings.append(Finding(
                            "nesting-depth", rel_path(path), max_depth_line + 1,
                            f"fn {fn_name}: nesting depth {max_depth - 1} (limit: 4)",
                        ))
                    i = j + 1
                    break
                j += 1
            else:
                i += 1
        else:
            i += 1
    return findings


def check_test_ephemeral(path: Path, lines: list[str]) -> list[Finding]:
    """NAMING: ephemeral IDs in test function names."""
    findings: list[Finding] = []
    for i, line in enumerate(lines):
        m = EPHEMERAL_TEST_RE.search(line)
        if m:
            fn_name = m.group(1)
            findings.append(Finding(
                "test-ephemeral", rel_path(path), i + 1,
                f"fn {fn_name}: ephemeral ID in test name",
            ))
    return findings


def check_test_weak(path: Path, lines: list[str]) -> list[Finding]:
    """NAMING: weak descriptors in test function names."""
    findings: list[Finding] = []
    for i, line in enumerate(lines):
        m = WEAK_TEST_RE.search(line)
        if m:
            fn_name = m.group(1)
            findings.append(Finding(
                "test-weak", rel_path(path), i + 1,
                f"fn {fn_name}: weak descriptor in test name",
            ))
        else:
            m = BARE_TEST_RE.search(line)
            if m:
                fn_name = m.group(1)
                findings.append(Finding(
                    "test-weak", rel_path(path), i + 1,
                    f"fn {fn_name}: bare unit name — needs scenario + expected",
                ))
    return findings




def check_banners(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: decorative comment banners."""
    findings: list[Finding] = []
    for i, line in enumerate(lines):
        if BANNER_RE.match(line):
            # Don't flag rule/separator lines in module docs
            stripped = line.strip()
            if stripped.startswith("//!"):
                continue
            findings.append(Finding(
                "banners", rel_path(path), i + 1,
                f"decorative banner: {stripped[:60]}",
            ))
    return findings


def check_commented_code(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: commented-out Rust code.

    Only flags code-like comments that form ISOLATED blocks — meaning
    the lines above and below the block are blank or non-comment. This
    distinguishes dead code (standalone, surrounded by blank lines) from
    pseudocode documentation (embedded in a larger comment block with
    prose explaining what the code models).
    """
    findings: list[Finding] = []

    def _is_comment(line: str) -> bool:
        s = line.lstrip()
        return s.startswith("//") or s.startswith("///") or s.startswith("//!")

    def _is_code_like(line: str) -> bool:
        s = line.lstrip()
        if s.startswith("///") or s.startswith("//!"):
            return False
        return bool(COMMENTED_CODE_RE.match(line))

    # Pass 1: find all code-like comment lines
    code_lines: set[int] = set()
    for i, line in enumerate(lines):
        if _is_code_like(line):
            code_lines.add(i)

    if not code_lines:
        return findings

    # Pass 2: group consecutive code-like lines into blocks
    sorted_lines = sorted(code_lines)
    blocks: list[list[int]] = []
    current_block: list[int] = [sorted_lines[0]]

    for idx in sorted_lines[1:]:
        if idx == current_block[-1] + 1:
            current_block.append(idx)
        else:
            blocks.append(current_block)
            current_block = [idx]
    blocks.append(current_block)

    # Pass 3: check each block's boundaries
    rp = rel_path(path)
    for block in blocks:
        first = block[0]
        last = block[-1]

        # Check line above: is it blank or non-comment?
        above_is_boundary = True
        if first > 0:
            above = lines[first - 1]
            if _is_comment(above) and above.strip() not in ("//", "///"):
                above_is_boundary = False

        # Check line below: is it blank or non-comment?
        below_is_boundary = True
        if last < len(lines) - 1:
            below = lines[last + 1]
            if _is_comment(below) and below.strip() not in ("//", "///"):
                below_is_boundary = False

        # Only flag if the block is isolated (not embedded in doc comments)
        if not (above_is_boundary and below_is_boundary):
            continue

        # Skip code-like comments near #[test] — often pseudocode docs
        near_test = False
        for j in range(max(0, first - 5), first):
            if "#[test]" in lines[j] or "fn test_" in lines[j]:
                near_test = True
                break
        if near_test:
            continue

        for i in block:
            findings.append(Finding(
                "commented-code", rp, i + 1,
                f"commented-out code: {lines[i].strip()[:70]}",
            ))

    return findings






def check_bare_todo(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: // TODO without (phase): format."""
    findings: list[Finding] = []
    for i, line in enumerate(lines):
        if BARE_TODO_RE.search(line):
            stripped = line.lstrip()
            # Skip doc comments
            if stripped.startswith("///") or stripped.startswith("//!"):
                continue
            findings.append(Finding(
                "bare-todo", rel_path(path), i + 1,
                f"format as // TODO(phase): description — {line.strip()[:60]}",
            ))
    return findings




def check_catch_all_arms(path: Path, lines: list[str]) -> list[Finding]:
    """GAP: _ => unreachable!()/todo!() catch-all arms.

    Only flags catch-alls near sync-point enum references (CanExpr::,
    ExprKind::, TypeTag::, DerivedTrait::, TokenKind::, etc.) to avoid
    flagging local refinement matches like `match char_kind { ... }`.
    """
    # Sync-point enums — SSOT shared with enum-drift.py KNOWN_ENUMS
    # These are the IR enums whose catch-alls indicate cross-phase drift
    sync_enums = SYNC_POINT_ENUMS
    findings: list[Finding] = []
    text = "\n".join(lines)

    # Only scan files that reference at least one sync-point enum
    has_sync_enum = any(f"{e}::" in text for e in sync_enums)
    if not has_sync_enum:
        return findings

    for i, line in enumerate(lines):
        if CATCH_ALL_RE.search(line):
            # Check if a sync-point enum is referenced within ~30 lines above
            context = "\n".join(lines[max(0, i - 30):i])
            near_sync = any(f"{e}::" in context for e in sync_enums)
            if near_sync:
                findings.append(Finding(
                    "catch-all-arms", rel_path(path), i + 1,
                    f"catch-all arm hides unhandled variants: {line.strip()[:70]}",
                ))
    return findings


def check_string_identity(path: Path, lines: list[str]) -> list[Finding]:
    """LEAK: string equality == "..." in non-test prod code.

    Only flags comparisons that look like identifier/name checks (the kind
    that should use Name interning). Skips CLI flags, OS names, file
    extensions, error messages, and format strings — those are legitimate
    string comparisons.
    """
    if is_test_file(path):
        return []
    findings: list[Finding] = []
    in_test_mod = False

    # Patterns that indicate interner-adjacent comparisons (likely bugs)
    interner_adjacent_re = re.compile(
        r"(?:name|ident|method|trait_name|type_name|field|callee|"
        r"variant|label|keyword|tag|module|crate_name|func_name)"
        r"[._\w]*\s*==\s*\"",
        re.IGNORECASE,
    )

    for i, line in enumerate(lines):
        if "#[cfg(test)]" in line:
            in_test_mod = True
        if in_test_mod:
            continue
        # Only flag interner-adjacent patterns, not all == "..."
        if interner_adjacent_re.search(line):
            stripped = line.lstrip()
            if stripped.startswith("//"):
                continue
            # Skip macros and format strings
            if any(m in line for m in [
                "assert", "format!", "write!", "panic!",
                "trace!", "debug!", "info!", "warn!", "error!",
            ]):
                continue
            findings.append(Finding(
                "string-identity", rel_path(path), i + 1,
                f"string comparison — use Name interning? {line.strip()[:60]}",
            ))
    return findings








def check_lib_bodies(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: lib.rs containing function bodies."""
    if path.name != "lib.rs":
        return []
    findings: list[Finding] = []
    for i, line in enumerate(lines):
        if LIB_FN_BODY_RE.match(line):
            # Check if this line or nearby lines have a { indicating a body
            # (vs just a signature like `pub fn foo(...) -> T;`)
            remaining = "".join(lines[i:min(i + 5, len(lines))])
            if "{" in remaining and not remaining.strip().endswith(";"):
                fn_match = re.search(r"fn\s+(\w+)", line)
                fn_name = fn_match.group(1) if fn_match else "?"
                findings.append(Finding(
                    "lib-bodies", rel_path(path), i + 1,
                    f"lib.rs should be index-only — fn {fn_name} has a body",
                ))
    return findings


# ─── New checks (batch 2) ─────────────────────────────────────

def check_deny_unsafe(path: Path, lines: list[str]) -> list[Finding]:
    """LEAK: pure crates must have #![deny(unsafe_code)]."""
    if path.name != "lib.rs":
        return []
    crate = ""
    try:
        rel = path.relative_to(COMPILER_DIR)
        crate = rel.parts[0]
    except (ValueError, IndexError):
        return []
    if crate not in PURE_CRATES:
        return []
    text = "\n".join(lines[:30])  # only check top of file
    if "#![deny(unsafe_code)]" in text:
        return []
    return [Finding(
        "deny-unsafe", rel_path(path), 1,
        f"pure crate {crate} missing #![deny(unsafe_code)] in lib.rs",
    )]





IGNORE_RE = re.compile(r'#\[ignore(?:\s*=\s*"([^"]*)")?\]')
TRACKING_RE = re.compile(r"(?:BUG-|TPR-|CROSS-|plans/|issue|FIND-|TASK-)", re.IGNORECASE)

def check_ignore_tracking(path: Path, lines: list[str]) -> list[Finding]:
    """BLOAT: #[ignore] without a tracking artifact reference."""
    findings: list[Finding] = []
    for i, line in enumerate(lines):
        m = IGNORE_RE.search(line)
        if m:
            reason = m.group(1) or ""
            if not TRACKING_RE.search(reason):
                findings.append(Finding(
                    "ignore-tracking", rel_path(path), i + 1,
                    f"#[ignore] without tracking artifact: {line.strip()[:70]}",
                ))
    return findings


USE_CRATE_RE = re.compile(r"^\s*use\s+(?:crate::)?(\w+)")

def check_phase_bleeding(path: Path, lines: list[str]) -> list[Finding]:
    """LEAK: crate imports from a higher-level crate (upward dependency).

    Uses CRATE_ORDER to detect when a low-level crate (e.g., ori_lexer)
    imports from a higher-level crate (e.g., ori_parse).
    """
    findings: list[Finding] = []
    src_crate = ""
    try:
        rel = path.relative_to(COMPILER_DIR)
        src_crate = rel.parts[0]
    except (ValueError, IndexError):
        return []

    src_level = CRATE_ORDER.get(src_crate, -1)
    if src_level < 0:
        return []

    for i, line in enumerate(lines):
        m = USE_CRATE_RE.match(line)
        if m:
            target = m.group(1)
            # Only check ori_* crate imports
            if not target.startswith("ori_"):
                continue
            # Skip self-imports (crate:: within the same crate)
            if target == src_crate:
                continue
            target_level = CRATE_ORDER.get(target, -1)
            if target_level < 0:
                continue
            # Level-0 crates (ori_ir, ori_diagnostic, ori_registry, ori_rt)
            # are all peers — allow cross-imports between them
            if src_level == 0 and target_level == 0:
                continue
            if target_level > src_level:
                findings.append(Finding(
                    "phase-bleeding", rel_path(path), i + 1,
                    f"{src_crate} (level {src_level}) imports {target} (level {target_level})",
                ))
    return findings


SWALLOWED_IF_LET_RE = re.compile(r"if\s+let\s+Ok\s*\(")
SWALLOWED_ERR_OK_RE = re.compile(r"Err\s*\(\s*_\s*\)\s*=>\s*Ok\s*\(")

def check_swallowed_error(path: Path, lines: list[str]) -> list[Finding]:
    """LEAK: error silently swallowed (if let Ok, Err(_) => Ok, unwrap_or_default)."""
    if is_test_file(path):
        return []
    findings: list[Finding] = []
    in_test_mod = False
    for i, line in enumerate(lines):
        if "#[cfg(test)]" in line:
            in_test_mod = True
        if in_test_mod:
            continue
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        # if let Ok(...) = ... without else
        if SWALLOWED_IF_LET_RE.search(line):
            # Check next few lines for else
            has_else = False
            for j in range(i + 1, min(i + 10, len(lines))):
                if "else" in lines[j]:
                    has_else = True
                    break
                if lines[j].strip() == "}" or re.match(r"^\s*let\s", lines[j]):
                    break
            if not has_else:
                findings.append(Finding(
                    "swallowed-error", rel_path(path), i + 1,
                    f"if let Ok() without else — error silently dropped: {stripped[:60]}",
                ))
        # Err(_) => Ok(...)
        if SWALLOWED_ERR_OK_RE.search(line):
            findings.append(Finding(
                "swallowed-error", rel_path(path), i + 1,
                f"Err(_) => Ok() — error silently swallowed: {stripped[:60]}",
            ))
    return findings


# ─── Check dispatcher ────────────────────────────────────────

# Map check IDs to their implementation functions
CHECK_FNS: dict[str, callable] = {
    "file-length":     check_file_length,
    "fn-length":       check_fn_length,
    "nesting-depth":   check_nesting_depth,
    "test-ephemeral":  check_test_ephemeral,
    "test-weak":       check_test_weak,
    "banners":         check_banners,
    "commented-code":  check_commented_code,
    "bare-todo":       check_bare_todo,
    "catch-all-arms":  check_catch_all_arms,
    "string-identity": check_string_identity,
    "lib-bodies":      check_lib_bodies,
    "deny-unsafe":     check_deny_unsafe,
    "ignore-tracking": check_ignore_tracking,
    "phase-bleeding":  check_phase_bleeding,
    "swallowed-error": check_swallowed_error,
}


def run_checks(files: list[Path], active_checks: set[str]) -> list[Finding]:
    """Run all active checks across all files."""
    all_findings: list[Finding] = []

    for path in files:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except (OSError, UnicodeDecodeError):
            continue

        lines = text.splitlines()

        for check_id in active_checks:
            fn = CHECK_FNS.get(check_id)
            if fn:
                all_findings.extend(fn(path, lines))

    return all_findings


# ─── Auto-fix ────────────────────────────────────────────────

def apply_fixes(findings: list[Finding], dry_run: bool) -> dict[str, list[str]]:
    """Apply auto-fixes for fixable findings. Returns {file: [diff lines]}."""
    fixable = [f for f in findings if f.auto_fixable]
    if not fixable:
        return {}

    # Group by file
    by_file: dict[str, set[int]] = defaultdict(set)
    for f in fixable:
        abs_path = str(REPO_ROOT / f.file)
        by_file[abs_path].add(f.line)

    diffs: dict[str, list[str]] = {}

    for filepath, line_nums in by_file.items():
        path = Path(filepath)
        try:
            original = path.read_text(encoding="utf-8")
        except OSError:
            continue

        orig_lines = original.splitlines(keepends=True)
        new_lines = []
        removed = []

        for i, line in enumerate(orig_lines):
            if (i + 1) in line_nums:
                removed.append(f"-{line.rstrip()}")
            else:
                new_lines.append(line)

        if removed:
            rel = rel_path(path)
            diff = [f"--- {rel}", f"+++ {rel}"]
            diff.extend(removed)
            diffs[rel] = diff

            if not dry_run:
                path.write_text("".join(new_lines), encoding="utf-8")

    return diffs


# ─── Reporters ───────────────────────────────────────────────

def report_text(findings: list[Finding]) -> None:
    """Human-readable grouped report."""
    if not findings:
        print(green("✓ No hygiene findings."))
        return

    by_check: dict[str, list[Finding]] = defaultdict(list)
    for f in findings:
        by_check[f.check_id].append(f)

    # Sort checks by severity (critical > major > minor)
    sev_order = {"critical": 0, "major": 1, "minor": 2, "info": 3}
    sorted_checks = sorted(
        by_check.keys(),
        key=lambda cid: (sev_order.get(CHECK_DEFS[cid][1], 9), cid),
    )

    for check_id in sorted_checks:
        check_findings = by_check[check_id]
        cat, sev, fixable, desc = CHECK_DEFS[check_id]
        fix_tag = " [fixable]" if fixable else ""

        header = f"[{cat}] {check_id}: {desc}{fix_tag}"
        if sev in ("critical", "major"):
            print(f"\n{bold(red(header))}")
        else:
            print(f"\n{bold(yellow(header))}")

        for f in sorted(check_findings, key=lambda x: (x.file, x.line)):
            loc = f"{f.file}:{f.line}"
            print(f"  {cyan(loc)} — {f.message}")

        print(f"  {dim(f'({len(check_findings)} findings)')}")

    # Summary
    print(f"\n{bold('─── Summary ───')}")
    total = len(findings)
    fixable_count = sum(1 for f in findings if f.auto_fixable)
    by_sev: dict[str, int] = defaultdict(int)
    by_cat: dict[str, int] = defaultdict(int)
    for f in findings:
        by_sev[f.severity] += 1
        by_cat[f.category] += 1

    print(f"  Total: {bold(str(total))} findings ({fixable_count} auto-fixable)")
    for sev in ["critical", "major", "minor"]:
        if by_sev[sev]:
            print(f"  {sev}: {by_sev[sev]}")
    for cat in sorted(by_cat):
        print(f"  {cat}: {by_cat[cat]}")


def report_json(findings: list[Finding]) -> None:
    """Machine-readable JSON output."""
    output = {
        "total": len(findings),
        "auto_fixable": sum(1 for f in findings if f.auto_fixable),
        "findings": [f.to_dict() for f in findings],
        "by_check": {},
    }
    by_check: dict[str, int] = defaultdict(int)
    for f in findings:
        by_check[f.check_id] += 1
    output["by_check"] = dict(by_check)
    print(json.dumps(output, indent=2))


def report_summary(findings: list[Finding]) -> None:
    """Counts per check, no details."""
    by_check: dict[str, int] = defaultdict(int)
    for f in findings:
        by_check[f.check_id] += 1

    total = len(findings)
    fixable = sum(1 for f in findings if f.auto_fixable)

    print(f"{bold('Hygiene Lint Summary')}")
    print(f"{'─' * 60}")

    sev_order = {"critical": 0, "major": 1, "minor": 2, "info": 3}
    for check_id in sorted(
        by_check.keys(),
        key=lambda c: (sev_order.get(CHECK_DEFS[c][1], 9), c),
    ):
        count = by_check[check_id]
        cat, sev, fixable_flag, desc = CHECK_DEFS[check_id]
        fix = " [fix]" if fixable_flag else ""
        sev_color = red if sev in ("critical", "major") else yellow
        print(f"  {sev_color(f'{count:4d}')}  {check_id:<20s} [{cat}] {desc}{fix}")

    print(f"{'─' * 60}")
    print(f"  {bold(f'{total:4d}')}  total ({fixable} auto-fixable)")


# ─── CLI ─────────────────────────────────────────────────────

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="hygiene-lint",
        description="Static analysis for Ori compiler hygiene rules.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=f"Available checks: {', '.join(sorted(ALL_CHECK_IDS))}",
    )
    p.add_argument(
        "--scope", nargs="+", metavar="PATH",
        help="Paths to scan (default: compiler/)",
    )
    p.add_argument(
        "--check", metavar="CHECKS",
        help="Comma-separated check IDs to run (default: all)",
    )
    p.add_argument(
        "--fix", action="store_true",
        help="Preview auto-fixes for fixable findings (dry-run)",
    )
    p.add_argument(
        "--apply", action="store_true",
        help="With --fix, write changes to disk",
    )
    p.add_argument("--json", action="store_true", help="JSON output")
    p.add_argument("--summary", action="store_true", help="Summary counts only")
    p.add_argument("--no-color", action="store_true", help="Disable color")
    p.add_argument(
        "--list-checks", action="store_true",
        help="List all available checks and exit",
    )
    return p


def main() -> int:
    global _use_color
    parser = build_parser()
    args = parser.parse_args()

    if args.no_color:
        _use_color = False

    if args.list_checks:
        print(f"{'ID':<20s} {'Cat':<10s} {'Sev':<8s} {'Fix':<5s} Description")
        print("─" * 80)
        for cid in sorted(CHECK_DEFS):
            cat, sev, fix, desc = CHECK_DEFS[cid]
            print(f"{cid:<20s} {cat:<10s} {sev:<8s} {'yes' if fix else 'no':<5s} {desc}")
        return 0

    # Determine scope
    if args.scope:
        scope_paths = []
        for s in args.scope:
            p = Path(s)
            if not p.is_absolute():
                p = REPO_ROOT / p
            scope_paths.append(p)
    else:
        scope_paths = [COMPILER_DIR]

    # Determine active checks
    if args.check:
        active = set()
        for c in args.check.split(","):
            c = c.strip()
            if c not in ALL_CHECK_IDS:
                print(f"Unknown check: {c}", file=sys.stderr)
                print(f"Available: {', '.join(sorted(ALL_CHECK_IDS))}", file=sys.stderr)
                return 1
            active.add(c)
    else:
        active = ALL_CHECK_IDS.copy()

    # Discover files
    files = discover_files(scope_paths)
    if not files:
        print("No .rs files found in scope.", file=sys.stderr)
        return 0

    # Run checks
    findings = run_checks(files, active)

    # Handle fix mode
    if args.fix:
        fixable = [f for f in findings if f.auto_fixable]
        if not fixable:
            print("No auto-fixable findings.")
            return 0

        diffs = apply_fixes(findings, dry_run=not args.apply)

        if args.apply:
            print(f"Fixed {len(fixable)} findings across {len(diffs)} files.")
            return 0
        else:
            # Show diff preview
            for filepath, diff_lines in diffs.items():
                for dl in diff_lines:
                    if dl.startswith("---") or dl.startswith("+++"):
                        print(bold(dl))
                    elif dl.startswith("-"):
                        print(red(dl))
            print(f"\n{len(fixable)} findings fixable across {len(diffs)} files.")
            print(dim("Run with --fix --apply to write changes."))
            return 2

    # Report
    if args.json:
        report_json(findings)
    elif args.summary:
        report_summary(findings)
    else:
        report_text(findings)

    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
