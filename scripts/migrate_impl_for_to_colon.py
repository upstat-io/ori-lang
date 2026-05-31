#!/usr/bin/env python3
"""Rewrite `impl [generics] Trait for Type {` -> `impl [generics] Type: Trait {`.

Per grammar.ebnf:312 trait_impl (colon-only; the `for`-form has no production).
Balanced-bracket aware: trait/type/generics may contain nested `<>`, `[]`, `()`.

Usage:
    migrate_impl_for_to_colon.py --check <files...>   # report transforms, write nothing
    migrate_impl_for_to_colon.py --apply <files...>   # rewrite in place

Only single-line `impl ... for ... {` headers are transformed (the corpus has no
multi-line impl headers). A line without a depth-0 ` for ` and a trailing `{` after
the type is left untouched (skips prose / comments / strings).
"""
import sys

OPEN = {"<": ">", "[": "]", "(": ")"}
CLOSE = {">": "<", "]": "[", ")": "("}


def _find_kw_for(s: str, start: int) -> int:
    """Return index of the ` for ` keyword at bracket-depth 0 at/after start, else -1."""
    depth = 0
    i = start
    n = len(s)
    while i < n:
        c = s[i]
        if c in OPEN:
            depth += 1
        elif c in CLOSE:
            depth -= 1
        elif depth == 0 and c == "f" and s[i : i + 3] == "for":
            before_ok = i == 0 or s[i - 1].isspace()
            after = s[i + 3] if i + 3 < n else " "
            if before_ok and (after.isspace()):
                return i
        i += 1
    return -1


def _consume_generics(s: str, start: int) -> int:
    """If s[start] starts an `impl<...>` generics block, return index past the matching `>`; else start."""
    if start >= len(s) or s[start] != "<":
        return start
    depth = 0
    i = start
    while i < len(s):
        if s[i] == "<":
            depth += 1
        elif s[i] == ">":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return start  # unbalanced — give up


def transform_line(line: str):
    """Return (new_line, changed: bool). Only transforms a depth-0 `impl ... for ... {` header."""
    stripped = line.lstrip()
    indent = line[: len(line) - len(stripped)]
    if not stripped.startswith("impl"):
        return line, False
    rest = stripped[len("impl") :]
    if not (rest[:1].isspace() or rest[:1] == "<"):
        return line, False  # `implements`, `impl_foo`, etc.

    # Optional generics immediately after `impl`.
    g_start = len(stripped) - len(rest)
    # skip spaces between `impl` and a possible `<`
    j = g_start
    while j < len(stripped) and stripped[j].isspace():
        j += 1
    gen_end = _consume_generics(stripped, j)
    generics = stripped[j:gen_end] if gen_end > j else ""
    after_gen = gen_end

    # Find depth-0 ` for `.
    for_idx = _find_kw_for(stripped, after_gen)
    if for_idx < 0:
        return line, False
    brace_idx = stripped.find("{", for_idx)
    if brace_idx < 0:
        return line, False  # no block opener — prose/comment, skip

    trait = stripped[after_gen:for_idx].strip()
    type_ = stripped[for_idx + 3 : brace_idx].strip()
    tail = stripped[brace_idx:].rstrip("\n")  # the `{` and anything after (drop the line's own newline; re-added once below)
    if not trait or not type_:
        return line, False

    gen = generics  # e.g. "<T: Clone>"
    new_stripped = f"impl{gen} {type_}: {trait} {tail}"
    return indent + new_stripped + ("\n" if line.endswith("\n") else ""), True


def process(path: str, apply: bool) -> int:
    with open(path, "r", encoding="utf-8") as fh:
        lines = fh.readlines()
    changes = []
    out = []
    for ln, line in enumerate(lines, 1):
        new, changed = transform_line(line)
        out.append(new)
        if changed:
            changes.append((ln, line.rstrip("\n"), new.rstrip("\n")))
    if changes:
        for ln, old, new in changes:
            print(f"  {path}:{ln}")
            print(f"    - {old.strip()}")
            print(f"    + {new.strip()}")
    if apply and changes:
        with open(path, "w", encoding="utf-8") as fh:
            fh.writelines(out)
    return len(changes)


import re as _re

# A colon trait_impl header line opening a body (`{` or `{}` at end). Every such
# header in a migrated corpus file is a migration product (the corpus had zero
# pre-existing colon impls), so each is followed by exactly one spurious blank
# line from the pre-fix double-newline bug.
_COLON_IMPL_HEADER_RE = _re.compile(r"^\s*impl\b[^=]*:.*\{\}?\s*$")


def fix_spurious_blanks(path: str, apply: bool) -> int:
    """Remove exactly one blank line immediately following each colon-impl header
    line (the pre-fix double-newline bug's spurious blank). Returns removals."""
    with open(path, "r", encoding="utf-8") as fh:
        lines = fh.readlines()
    out = []
    removed = 0
    i = 0
    n = len(lines)
    while i < n:
        out.append(lines[i])
        if _COLON_IMPL_HEADER_RE.match(lines[i]) and i + 1 < n and lines[i + 1].strip() == "":
            removed += 1  # drop exactly one following blank line
            i += 2
            continue
        i += 1
    if removed:
        print(f"  {path}: {removed} spurious blank line(s)")
    if apply and removed:
        with open(path, "w", encoding="utf-8") as fh:
            fh.writelines(out)
    return removed


def main(argv) -> int:
    if len(argv) < 3 or argv[1] not in ("--check", "--apply", "--fix-spurious-blanks", "--fix-spurious-blanks-apply"):
        print(__doc__)
        return 2
    if argv[1].startswith("--fix-spurious-blanks"):
        apply = argv[1] == "--fix-spurious-blanks-apply"
        total = sum(fix_spurious_blanks(p, apply) for p in argv[2:])
        verb = "removed" if apply else "would remove"
        print(f"\n{verb} {total} spurious blank line(s) across {len(argv) - 2} file(s)")
        return 0
    apply = argv[1] == "--apply"
    total = 0
    for path in argv[2:]:
        total += process(path, apply)
    verb = "rewrote" if apply else "would rewrite"
    print(f"\n{verb} {total} line(s) across {len(argv) - 2} file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
