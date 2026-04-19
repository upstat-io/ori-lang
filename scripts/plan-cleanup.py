#!/usr/bin/env python3
"""
Plan-doc cleanup — applies scanner-detected auto-fixes before /commit-push stages.

Called from /commit-push Step 4 (after fmt-all.sh, before git add -A).
Idempotent. Silent when clean. Always exits 0 (cleanup failures do not block commits).

Handles three gate payloads from roadmap_scan.py --json:
- stale_frontmatter.focus_plan_mismatches  — rewrite `status:` in section/subsection frontmatter
- stale_plan_annotations.cleanup_plan      — invoke plan-annotations.sh --cleanup-only
- bug_marker_drift.auto_fix_edits          — insert `Superseded by:` line into bug-tracker entry

Usage:
  python3 scripts/plan-cleanup.py           # default: run cleanup, report if fixes applied
  python3 scripts/plan-cleanup.py --quiet   # suppress summary line even on fixes
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


def repo_root() -> Path:
    return Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True
        ).strip()
    )


def run_scanner(root: Path) -> dict | None:
    try:
        out = subprocess.check_output(
            ["python3", ".claude/skills/continue-roadmap/roadmap_scan.py", "--json"],
            cwd=root,
            text=True,
            stderr=subprocess.DEVNULL,
        )
        return json.loads(out)
    except Exception:
        return None


# ---- 2a: stale frontmatter ----


def decide_new_status(issue: str) -> str | None:
    if "complete but" in issue and "unchecked" in issue:
        return "in-progress"
    if "not-started but" in issue and "checked" in issue:
        return "in-progress"
    if "in-progress but all items checked" in issue:
        return "complete"
    if "in-progress but 0 items checked" in issue:
        return "not-started"
    return None


def rewrite_toplevel_status(path: Path, new_status: str) -> bool:
    text = path.read_text()
    lines = text.splitlines(keepends=True)
    if not lines or lines[0].rstrip() != "---":
        return False
    closing = None
    for i, line in enumerate(lines[1:], start=1):
        if line.rstrip() == "---":
            closing = i
            break
    if closing is None:
        return False
    status_re = re.compile(r"^(status:\s*)\S+(\s*)$")
    for i in range(1, closing):
        m = status_re.match(lines[i])
        if m:
            current = lines[i].split(":", 1)[1].strip()
            if current == new_status:
                return False
            lines[i] = f"{m.group(1)}{new_status}{m.group(2)}\n"
            path.write_text("".join(lines))
            return True
    return False


def rewrite_subsection_status(path: Path, subsection_id: str, new_status: str) -> bool:
    text = path.read_text()
    lines = text.splitlines(keepends=True)
    target_re = re.compile(rf'^\s*-\s*id:\s*"?{re.escape(subsection_id)}"?\s*$')
    status_re = re.compile(r"^(\s+status:\s*)\S+(\s*)$")
    for i, line in enumerate(lines):
        if target_re.match(line):
            for j in range(i + 1, min(i + 8, len(lines))):
                # End of this entry: next `- id:` or un-indented line
                if re.match(r"^\s*-\s*id:", lines[j]) and j > i:
                    break
                if lines[j] and not lines[j].startswith(" "):
                    break
                m = status_re.match(lines[j])
                if m:
                    current = lines[j].split(":", 1)[1].strip()
                    if current == new_status:
                        return False
                    lines[j] = f"{m.group(1)}{new_status}{m.group(2)}\n"
                    path.write_text("".join(lines))
                    return True
            break
    return False


def fix_frontmatter(mismatch: dict, root: Path) -> bool:
    plan = mismatch.get("plan", "")
    location = mismatch.get("location", "")
    issue = mismatch.get("issue", "")
    new_status = decide_new_status(issue)
    if new_status is None:
        return False
    if "#" in location:
        filename, sub_id = location.split("#", 1)
    else:
        filename, sub_id = location, None
    path = root / "plans" / plan / filename
    if not path.exists():
        return False
    if sub_id:
        return rewrite_subsection_status(path, sub_id, new_status)
    return rewrite_toplevel_status(path, new_status)


# ---- 2b: stale plan annotations ----


def run_plan_annotations_cleanup(plan_name: str, root: Path) -> bool:
    script = root / ".claude/skills/impl-hygiene-review/plan-annotations.sh"
    if not script.exists():
        return False
    result = subprocess.run(
        ["bash", str(script), "--cleanup-only", "--plan", plan_name],
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


# ---- 2c: bug marker drift ----


def apply_bug_marker_edit(edit: dict, root: Path) -> bool:
    file_path = root / edit.get("file", "")
    if not file_path.exists():
        return False
    header_lineno = edit.get("header_lineno", 0)  # 1-based
    insert_line = edit.get("insert_line", "").rstrip("\n") + "\n"
    if not insert_line.strip():
        return False

    lines = file_path.read_text().splitlines(keepends=True)
    if header_lineno < 1 or header_lineno > len(lines):
        return False

    end = header_lineno  # 0-based index of first line AFTER header
    for j in range(header_lineno, len(lines)):
        line = lines[j]
        if not line.strip():
            break
        if line.lstrip().startswith("- ["):
            break
        if "Superseded by:" in line:
            return False  # idempotent
        end = j + 1

    lines.insert(end, insert_line)
    file_path.write_text("".join(lines))
    return True


# ---- main ----


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--quiet", action="store_true", help="Suppress summary output.")
    args = ap.parse_args()

    try:
        root = repo_root()
    except Exception:
        return 0

    data = run_scanner(root)
    if data is None:
        return 0

    gates = data.get("gates", {})

    frontmatter_fixed = 0
    annotations_fixed = False
    markers_fixed = 0

    sf = gates.get("stale_frontmatter", {}).get("payload", {})
    for m in sf.get("focus_plan_mismatches", []):
        if fix_frontmatter(m, root):
            frontmatter_fixed += 1

    spa = gates.get("stale_plan_annotations", {}).get("payload", {})
    if spa.get("count", 0) > 0:
        plan_name = spa.get("cleanup_plan")
        if plan_name and run_plan_annotations_cleanup(plan_name, root):
            annotations_fixed = True

    bmd = gates.get("bug_marker_drift", {}).get("payload", {})
    for edit in bmd.get("auto_fix_edits", []):
        if apply_bug_marker_edit(edit, root):
            markers_fixed += 1

    total = frontmatter_fixed + (1 if annotations_fixed else 0) + markers_fixed
    if total > 0 and not args.quiet:
        print(
            f"plan-cleanup: {frontmatter_fixed} frontmatter, "
            f"{1 if annotations_fixed else 0} annotations, "
            f"{markers_fixed} markers"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
