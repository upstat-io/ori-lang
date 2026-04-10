#!/usr/bin/env python3
"""
fn-rename.py — Batch test function renamer for Ori compiler.

Renames test functions, optionally adding /// doc comments with
provenance information. Designed to work with hygiene-lint.py's
test-ephemeral and test-weak findings.

Modes:

  Single rename:
    fn-rename.py --file path.rs --from test_old_name --to test_new_name
    fn-rename.py --file path.rs --from test_old --to test_new --add-doc "Regression: BUG-04-045"

  Batch rename from JSON manifest:
    fn-rename.py --batch manifest.json [--apply]

  Generate manifest from hygiene-lint output:
    hygiene-lint.py --check test-ephemeral --json | fn-rename.py --from-lint [--apply]

Manifest format (JSON):
  [
    {
      "file": "compiler/ori_eval/src/tests.rs",
      "from": "test_tpr_07_017_two_unrelated",
      "to": "test_iterator_drop_two_unrelated_sequences_release",
      "doc": "Regression: TPR-07-017"
    },
    ...
  ]

Safety:
  - Only renames `fn` declarations (not calls — test fns have no callers)
  - Verifies the old name exists exactly once in the file
  - Dry-run by default; --apply to write
  - Updates include_str! fixture paths if they contain the old name
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]

_use_color = sys.stdout.isatty()
def _c(code: str, text: str) -> str:
    return f"\033[{code}m{text}\033[0m" if _use_color else text
def red(t: str) -> str: return _c("31", t)
def green(t: str) -> str: return _c("32", t)
def cyan(t: str) -> str: return _c("36", t)
def bold(t: str) -> str: return _c("1", t)
def dim(t: str) -> str: return _c("2", t)


@dataclass
class Rename:
    file: str
    old_name: str
    new_name: str
    doc_comment: str = ""


@dataclass
class RenameResult:
    rename: Rename
    success: bool
    message: str
    diff_lines: list[str]


def apply_rename(rename: Rename, dry_run: bool) -> RenameResult:
    """Apply a single function rename."""
    path = Path(rename.file)
    if not path.is_absolute():
        path = REPO_ROOT / path

    if not path.exists():
        return RenameResult(rename, False, f"File not found: {path}", [])

    try:
        text = path.read_text(encoding="utf-8")
    except OSError as e:
        return RenameResult(rename, False, f"Read error: {e}", [])

    lines = text.splitlines(keepends=True)
    old_fn_pattern = re.compile(rf"\bfn\s+{re.escape(rename.old_name)}\b")

    # Find all occurrences of the function declaration
    matches = [(i, line) for i, line in enumerate(lines) if old_fn_pattern.search(line)]

    if len(matches) == 0:
        return RenameResult(
            rename, False,
            f"fn {rename.old_name} not found in {rename.file}", [],
        )
    if len(matches) > 1:
        return RenameResult(
            rename, False,
            f"fn {rename.old_name} found {len(matches)} times in {rename.file} — ambiguous", [],
        )

    line_idx, old_line = matches[0]
    new_line = old_fn_pattern.sub(f"fn {rename.new_name}", old_line)

    # Build diff
    diff: list[str] = [f"--- {rename.file}", f"+++ {rename.file}"]

    # If doc comment requested, check if one already exists
    new_lines = list(lines)

    if rename.doc_comment:
        # Check if previous non-blank line is already a doc comment
        has_doc = False
        j = line_idx - 1
        while j >= 0:
            prev = lines[j].strip()
            if prev == "":
                j -= 1
                continue
            if prev.startswith("///"):
                has_doc = True
            break

        if not has_doc:
            # Determine indentation from the fn line
            indent = ""
            for ch in old_line:
                if ch in (" ", "\t"):
                    indent += ch
                else:
                    break

            doc_line = f"{indent}/// {rename.doc_comment}\n"
            new_lines.insert(line_idx, doc_line)
            diff.append(f"+{doc_line.rstrip()}")
            # Adjust line_idx since we inserted
            line_idx += 1

    # Replace the fn line
    diff.append(f"-{old_line.rstrip()}")
    new_lines[line_idx] = new_line
    diff.append(f"+{new_line.rstrip()}")

    # Also update include_str! fixture paths containing the old name
    fixture_updates = 0
    for i, line in enumerate(new_lines):
        if "include_str!" in line and rename.old_name in line:
            updated = line.replace(rename.old_name, rename.new_name)
            if updated != line:
                diff.append(f"-{line.rstrip()}")
                diff.append(f"+{updated.rstrip()}")
                new_lines[i] = updated
                fixture_updates += 1

    if not dry_run:
        path.write_text("".join(new_lines), encoding="utf-8")

    msg = f"Renamed fn {rename.old_name} → {rename.new_name}"
    if rename.doc_comment:
        msg += f" (added /// doc)"
    if fixture_updates:
        msg += f" (+{fixture_updates} fixture path(s))"

    return RenameResult(rename, True, msg, diff)


def load_manifest(path: str) -> list[Rename]:
    """Load renames from a JSON manifest file."""
    with open(path) as f:
        data = json.load(f)

    renames = []
    for entry in data:
        renames.append(Rename(
            file=entry["file"],
            old_name=entry["from"],
            new_name=entry["to"],
            doc_comment=entry.get("doc", ""),
        ))
    return renames


def load_from_lint_stdin() -> list[Rename]:
    """Read hygiene-lint --json output from stdin, extract test-ephemeral findings.

    These are flagged for manual rename — we can't auto-generate new names,
    but we CAN generate a manifest template the user fills in.
    """
    data = json.load(sys.stdin)
    renames = []
    for finding in data.get("findings", []):
        if finding["check"] in ("test-ephemeral", "test-weak"):
            # Extract function name from message
            m = re.search(r"fn (\w+):", finding["message"])
            if m:
                old_name = m.group(1)
                renames.append(Rename(
                    file=finding["file"],
                    old_name=old_name,
                    new_name=f"TODO_RENAME_{old_name}",
                    doc_comment="",
                ))
    return renames


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="fn-rename",
        description="Batch test function renamer for Ori compiler hygiene.",
    )

    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--file", help="Single file to rename in")
    mode.add_argument("--batch", metavar="MANIFEST", help="JSON manifest file")
    mode.add_argument(
        "--from-lint", action="store_true",
        help="Read hygiene-lint --json from stdin, generate manifest template",
    )

    p.add_argument("--from", dest="old_name", help="Old function name (with --file)")
    p.add_argument("--to", dest="new_name", help="New function name (with --file)")
    p.add_argument("--add-doc", help="Add /// doc comment (with --file)")
    p.add_argument("--apply", action="store_true", help="Write changes (default: dry-run)")
    p.add_argument("--no-color", action="store_true", help="Disable color")
    return p


def main() -> int:
    global _use_color
    parser = build_parser()
    args = parser.parse_args()

    if args.no_color:
        _use_color = False

    dry_run = not args.apply

    if args.from_lint:
        renames = load_from_lint_stdin()
        if not renames:
            print("No test-ephemeral or test-weak findings in input.")
            return 0
        # Output as manifest template
        manifest = []
        for r in renames:
            manifest.append({
                "file": r.file,
                "from": r.old_name,
                "to": "FILL_IN_NEW_NAME",
                "doc": "",
            })
        print(json.dumps(manifest, indent=2))
        print(f"\n{dim(f'Generated manifest with {len(manifest)} entries.')}")
        print(dim("Fill in 'to' names, then run: fn-rename.py --batch manifest.json --apply"))
        return 0

    if args.file:
        if not args.old_name or not args.new_name:
            parser.error("--file requires --from and --to")
        renames = [Rename(args.file, args.old_name, args.new_name, args.add_doc or "")]
    elif args.batch:
        renames = load_manifest(args.batch)
    else:
        parser.error("Specify --file, --batch, or --from-lint")
        return 1

    if not renames:
        print("No renames to apply.")
        return 0

    # Apply renames
    success_count = 0
    fail_count = 0

    for rename in renames:
        result = apply_rename(rename, dry_run)

        if result.success:
            success_count += 1
            if dry_run:
                print(f"{cyan('[preview]')} {result.message}")
                for dl in result.diff_lines:
                    if dl.startswith("-") and not dl.startswith("---"):
                        print(f"  {red(dl)}")
                    elif dl.startswith("+") and not dl.startswith("+++"):
                        print(f"  {green(dl)}")
                    else:
                        print(f"  {dl}")
            else:
                print(f"{green('[done]')} {result.message}")
        else:
            fail_count += 1
            print(f"{red('[error]')} {result.message}")

    # Summary
    if dry_run and success_count:
        print(f"\n{bold(f'{success_count} rename(s) ready.')}")
        print(dim("Run with --apply to write changes."))

    return 1 if fail_count else (2 if dry_run and success_count else 0)


if __name__ == "__main__":
    sys.exit(main())
