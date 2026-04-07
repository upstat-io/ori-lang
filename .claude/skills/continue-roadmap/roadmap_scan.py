#!/usr/bin/env python3
"""roadmap_scan.py — comprehensive roadmap scanner for /continue-roadmap.

Replaces the prior bash scanner. The output is designed to be consumed by
Claude during /continue-roadmap, not by humans directly — the workflow reads
this output and presents a user-facing summary at the end. Therefore the
format prioritizes information density and complete context over visual
polish.

One back-compat concession: a `=== REROUTES ===` block appears at the top of
the default output with the same format the previous bash scanner used, so
that .claude/skills/create-plan/SKILL.md's `sed -n '/=== REROUTES ===/,/^$/p'`
extraction continues to work without modification.

Layout:
    1. `=== REROUTES ===` block (create-plan compat)
    2. Workspace summary (plans discovered, health signals)
    3. Focus selection (which plan and section, with reason)
    4. Focus plan overview (section status list)
    5. Focus section detail (subsections, unblocked items, blockers)
    6. Bug tracker relevance (open bugs mapped to focus subsystem)
    7. Fix sections state (open fix-BUG-*.md files)
    8. Decision notes (gates the workflow needs to consider)

CLI:
    roadmap_scan.py [PLAN_DIR] [FOCUS_SECTION]   # positional, same as bash
        --json              emit structured JSON instead of rich text
        --reroutes-only     emit only the REROUTES block (fast path)
        --no-bugs           skip bug-tracker crawl
        --trace             log decisions to stderr
        --quiet             suppress health signals section

Requires: PyYAML (6.x). Error exits with install hint if missing.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

try:
    import yaml
except ImportError:
    sys.stderr.write(
        "error: PyYAML is required. install with: pip install pyyaml\n"
        "       (already installed at 6.x on this system via system packages)\n"
    )
    sys.exit(2)


# ─── Trace / debug channel ────────────────────────────────────────────────────

TRACE_ENABLED = False


def trace(msg: str) -> None:
    if TRACE_ENABLED:
        sys.stderr.write(f"[trace] {msg}\n")


# ─── Data model ───────────────────────────────────────────────────────────────


@dataclass
class Item:
    """A single `- [ ]` / `- [x]` checkbox line."""
    lineno: int
    indent: int
    content: str              # stripped content minus `- [ ] ` prefix
    raw_line: str             # original full line (for rendering)
    checked: bool
    subsection_id: str        # "07.1" or "?" if outside any `## NN.M` header
    own_blockers: list[str] = field(default_factory=list)         # direct `blocked-by:X` tags
    inherited_blockers: list[str] = field(default_factory=list)   # from parent item
    prose_blocker: str | None = None                              # `<!-- blocked: some text -->`

    @property
    def blocked(self) -> bool:
        return bool(self.own_blockers or self.inherited_blockers or self.prose_blocker)

    @property
    def effective_blockers(self) -> list[str]:
        return self.own_blockers or self.inherited_blockers


@dataclass
class Subsection:
    """A `## NN.M Title` block."""
    id: str                   # "07.1"
    title: str                # from frontmatter sections[] entry
    status: str               # frontmatter status: not-started | in-progress | complete
    items: list[Item] = field(default_factory=list)
    title_from_header: str = ""  # actual ## header text (may differ from frontmatter)

    @property
    def checked(self) -> int:
        return sum(1 for i in self.items if i.checked)

    @property
    def unchecked(self) -> int:
        return sum(1 for i in self.items if not i.checked)

    @property
    def total(self) -> int:
        return len(self.items)

    @property
    def blocked_count(self) -> int:
        return sum(1 for i in self.items if not i.checked and i.blocked)

    @property
    def mismatch(self) -> str | None:
        if self.total == 0:
            return "no-checkboxes"
        if self.status == "complete" and self.unchecked > 0:
            return f"frontmatter=complete but {self.unchecked} unchecked"
        if self.status == "not-started" and self.checked > 0:
            return f"frontmatter=not-started but {self.checked} checked"
        if self.status == "in-progress" and self.unchecked == 0 and self.total > 0:
            return "frontmatter=in-progress but all items checked"
        if self.status == "in-progress" and self.checked == 0 and self.total > 0:
            return "frontmatter=in-progress but 0 items checked"
        return None


@dataclass
class TprFinding:
    id: str            # "TPR-07-017"
    severity: str      # high | medium | low
    lineno: int
    resolved: bool     # `[x]` vs `[ ]`


@dataclass
class Section:
    """A section-*.md file."""
    plan: "Plan"
    path: Path
    number: str                           # raw frontmatter value, e.g. "07" or "0"
    title: str                            # raw frontmatter value
    status: str
    reviewed: bool | None
    tpr_status: str                       # none | findings | resolved
    tpr_updated: str | None
    depends_on: list[str] = field(default_factory=list)
    subsections: list[Subsection] = field(default_factory=list)
    flat_items: list[Item] = field(default_factory=list)
    tpr_findings: list[TprFinding] = field(default_factory=list)
    frontmatter: dict = field(default_factory=dict)

    @property
    def checked(self) -> int:
        return sum(1 for i in self.flat_items if i.checked)

    @property
    def unchecked(self) -> int:
        return sum(1 for i in self.flat_items if not i.checked)

    @property
    def total(self) -> int:
        return len(self.flat_items)

    @property
    def pct(self) -> int:
        return (self.checked * 100 // self.total) if self.total else 0

    @property
    def unblocked_items(self) -> list[Item]:
        return [i for i in self.flat_items if not i.checked and not i.blocked]

    @property
    def blocked_items(self) -> list[Item]:
        return [i for i in self.flat_items if not i.checked and i.blocked]

    @property
    def mismatch(self) -> str | None:
        if self.status == "complete" and self.unchecked > 0:
            return f"frontmatter=complete but {self.unchecked} unchecked"
        if self.status == "not-started" and self.checked > 0:
            return f"frontmatter=not-started but {self.checked} checked"
        if self.status == "in-progress" and self.unchecked == 0 and self.total > 0:
            return "frontmatter=in-progress but all items checked"
        return None

    @property
    def sort_key(self) -> tuple:
        """Natural ordering so 07.0 < 07.1 < 07.3.A etc."""
        m = re.match(r"^\"?(\d+)\"?$", str(self.number))
        if m:
            return (int(m.group(1)),)
        return (999, str(self.number))


@dataclass
class FixSection:
    """A plans/bug-tracker/fix-BUG-*.md file."""
    path: Path
    bug_id: str
    title: str
    severity: str
    status: str
    subsystem: str
    found: str | None
    tpr_status: str
    tpr_updated: str | None


@dataclass
class Bug:
    """A `- [ ]` entry in bug-tracker section-*.md files."""
    id: str              # "BUG-04-045"
    severity: str        # critical | high | medium | low
    status: str          # open | in-progress | fixed
    title: str
    lineno: int
    source_section: str  # bug-tracker section file name


@dataclass
class Reroute:
    """An entry from plans/<plan>/index.md frontmatter."""
    plan_dir: Path
    name: str             # short name or plan dir
    full_name: str        # display name
    kind: str             # "reroute" | "parallel"
    status: str           # active | queued | resolved
    order: int            # sort key; 999 default
    reviewed: bool | None


@dataclass
class Plan:
    """A directory under plans/ (or plans/completed/)."""
    name: str                                  # directory name
    dir: Path
    sections: list[Section] = field(default_factory=list)
    overview: dict | None = None               # 00-overview.md frontmatter if any
    index: dict | None = None                  # index.md frontmatter if any
    reroute: Reroute | None = None
    dep_graph: dict[str, list[str]] = field(default_factory=dict)
    # for bug-tracker specifically:
    bugs: list[Bug] = field(default_factory=list)
    fix_sections: list[FixSection] = field(default_factory=list)

    @property
    def total_checked(self) -> int:
        return sum(s.checked for s in self.sections)

    @property
    def total_items(self) -> int:
        return sum(s.total for s in self.sections)

    @property
    def pct(self) -> int:
        return (self.total_checked * 100 // self.total_items) if self.total_items else 0

    @property
    def section_status_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {"complete": 0, "in-progress": 0, "not-started": 0, "unknown": 0}
        for s in self.sections:
            key = s.status if s.status in counts else "unknown"
            counts[key] += 1
        return counts

    @property
    def open_tpr_count(self) -> int:
        return sum(sum(1 for f in s.tpr_findings if not f.resolved) for s in self.sections)


@dataclass
class Workspace:
    """The full scan result."""
    repo_root: Path
    plans_dir: Path
    all_plans: dict[str, Plan] = field(default_factory=dict)        # by dir name
    reroutes: list[Reroute] = field(default_factory=list)
    bug_tracker: Plan | None = None
    completed_plans: dict[str, Plan] = field(default_factory=dict)
    focus_plan: Plan | None = None
    focus_reason: str = ""
    focus_section: Section | None = None
    focus_section_reason: str = ""

    def active_reroutes(self) -> list[Reroute]:
        return sorted([r for r in self.reroutes if r.status == "active"], key=lambda r: r.order)

    def queued_reroutes(self) -> list[Reroute]:
        return sorted([r for r in self.reroutes if r.status == "queued"], key=lambda r: r.order)


# ─── Parser ───────────────────────────────────────────────────────────────────


FRONTMATTER_RE = re.compile(r"^---\s*$")
HEADER_RE = re.compile(r"^## +(.+?)\s*$")
SUBSECTION_ID_RE = re.compile(r"^([\w.\-]+)(?::|\s|$)")
CHECKBOX_RE = re.compile(r"^(\s*)- \[( |x|X)\] +(.*)$")
BLOCKED_BY_RE = re.compile(r"<!--\s*blocked-by:([A-Za-z0-9.\-]+)(?:\s*-->)?")
BLOCKED_PROSE_RE = re.compile(r"<!--\s*blocked:\s*(.+?)\s*-->")
TPR_ID_RE = re.compile(r"\[(TPR-[\w\-]+)\]\[(high|medium|low|critical)\]")
BUG_FRONTMATTER_BUG_ID_RE = re.compile(r"^bug:\s*\"?(BUG-[\w\-]+)\"?\s*$")
BUG_TRACKER_ENTRY_RE = re.compile(
    r"^\s*- \[( |x|X)\] +\*?\*?(BUG-[\w\-]+)\*?\*?[\s\-–]+(.+?)(?:\s*<!--.*)?$"
)
BUG_SEVERITY_RE = re.compile(r"severity:\s*(critical|high|medium|low)", re.IGNORECASE)


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def split_frontmatter(text: str) -> tuple[dict, int]:
    """Parse YAML frontmatter. Returns (parsed_dict_or_empty, body_line_offset)."""
    lines = text.splitlines()
    if not lines or not FRONTMATTER_RE.match(lines[0]):
        return {}, 0
    end = -1
    for i in range(1, len(lines)):
        if FRONTMATTER_RE.match(lines[i]):
            end = i
            break
    if end < 0:
        return {}, 0
    fm_text = "\n".join(lines[1:end])
    try:
        data = yaml.safe_load(fm_text) or {}
    except yaml.YAMLError as e:
        trace(f"frontmatter parse error: {e}")
        data = {}
    return data, end + 1


def detect_subsection_id(header_text: str) -> str:
    """`## 07.1 Discriminant Narrowing` → `07.1`; strip quotes."""
    stripped = header_text.replace('"', "").strip()
    m = SUBSECTION_ID_RE.match(stripped)
    return m.group(1).rstrip(":") if m else "?"


def parse_section_body(lines: list[str], body_start: int) -> tuple[list[Item], dict[str, str]]:
    """Walk the markdown body and extract checkboxes grouped by `## ID` header.

    Returns (flat_items, header_titles_by_id).
    header_titles_by_id maps "07.1" → the raw ## header text minus the ID.
    """
    items: list[Item] = []
    header_titles: dict[str, str] = {}
    cur_sub = "?"
    parent_blocker: list[str] = []  # inherited by nested items

    for idx in range(body_start, len(lines)):
        line = lines[idx]
        lineno = idx + 1  # 1-indexed for output

        # Reset parent blocker on `### ` boundaries
        if line.startswith("### "):
            parent_blocker = []
            continue

        hm = HEADER_RE.match(line)
        if hm:
            header_text = hm.group(1)
            cur_sub = detect_subsection_id(header_text)
            # Strip the ID prefix to get the title portion
            remaining = header_text
            if cur_sub != "?" and remaining.startswith(cur_sub):
                remaining = remaining[len(cur_sub):].lstrip(" :")
            header_titles[cur_sub] = remaining
            parent_blocker = []
            continue

        cm = CHECKBOX_RE.match(line)
        if not cm:
            continue

        indent_str, mark, content = cm.groups()
        indent = len(indent_str)
        checked = mark.lower() == "x"

        own_blockers = BLOCKED_BY_RE.findall(line)
        prose_m = BLOCKED_PROSE_RE.search(line)
        prose_blocker = None
        if prose_m and not own_blockers:
            prose_blocker = prose_m.group(1)

        # Parent inheritance: indent-0 items set the parent blocker for nested items
        inherited: list[str] = []
        if indent == 0:
            parent_blocker = list(own_blockers)
        elif indent > 0 and not own_blockers and parent_blocker:
            inherited = list(parent_blocker)

        item = Item(
            lineno=lineno,
            indent=indent,
            content=content,
            raw_line=line,
            checked=checked,
            subsection_id=cur_sub,
            own_blockers=own_blockers,
            inherited_blockers=inherited,
            prose_blocker=prose_blocker,
        )
        items.append(item)

    return items, header_titles


def parse_tpr_findings(items: list[Item]) -> list[TprFinding]:
    findings: list[TprFinding] = []
    for item in items:
        if item.subsection_id.endswith(".R") or "TPR-" in item.content:
            m = TPR_ID_RE.search(item.content)
            if m:
                findings.append(
                    TprFinding(
                        id=m.group(1),
                        severity=m.group(2),
                        lineno=item.lineno,
                        resolved=item.checked,
                    )
                )
    return findings


def parse_section_file(plan: Plan, path: Path) -> Section | None:
    """Parse a section-*.md file into a Section."""
    try:
        text = read_text(path)
    except OSError as e:
        trace(f"cannot read {path}: {e}")
        return None

    fm, body_start = split_frontmatter(text)
    if not fm:
        trace(f"{path.name}: no frontmatter, skipping")
        return None

    number = str(fm.get("section", ""))
    title = str(fm.get("title", ""))
    status = str(fm.get("status", "unknown"))
    reviewed = fm.get("reviewed")
    if isinstance(reviewed, str):
        reviewed = reviewed.lower() == "true"

    tpr_block = fm.get("third_party_review") or {}
    if isinstance(tpr_block, dict):
        tpr_status = str(tpr_block.get("status", "none"))
        tpr_updated = tpr_block.get("updated")
        if tpr_updated is not None:
            tpr_updated = str(tpr_updated)
    else:
        tpr_status = "none"
        tpr_updated = None

    depends_on = fm.get("depends_on") or []
    if not isinstance(depends_on, list):
        depends_on = [str(depends_on)]
    depends_on = [str(d) for d in depends_on]

    lines = text.splitlines()
    flat_items, header_titles = parse_section_body(lines, body_start)

    # Build subsections from frontmatter `sections:` array
    subsections: list[Subsection] = []
    sub_items_by_id: dict[str, list[Item]] = {}
    for item in flat_items:
        sub_items_by_id.setdefault(item.subsection_id, []).append(item)

    fm_sections = fm.get("sections") or []
    if isinstance(fm_sections, list):
        for entry in fm_sections:
            if not isinstance(entry, dict):
                continue
            sid = str(entry.get("id", ""))
            stitle = str(entry.get("title", ""))
            sstatus = str(entry.get("status", "unknown"))
            subsections.append(
                Subsection(
                    id=sid,
                    title=stitle,
                    status=sstatus,
                    items=sub_items_by_id.get(sid, []),
                    title_from_header=header_titles.get(sid, ""),
                )
            )

    section = Section(
        plan=plan,
        path=path,
        number=number,
        title=title,
        status=status,
        reviewed=reviewed if isinstance(reviewed, bool) else None,
        tpr_status=tpr_status,
        tpr_updated=tpr_updated,
        depends_on=depends_on,
        subsections=subsections,
        flat_items=flat_items,
        frontmatter=fm,
    )
    section.tpr_findings = parse_tpr_findings(flat_items)
    return section


def parse_index_file(path: Path) -> Reroute | None:
    """Parse plans/<plan>/index.md frontmatter into a Reroute if it has one."""
    try:
        text = read_text(path)
    except OSError:
        return None
    fm, _ = split_frontmatter(text)
    if not fm:
        return None

    is_reroute = bool(fm.get("reroute"))
    is_parallel = bool(fm.get("parallel"))
    if not (is_reroute or is_parallel):
        return None

    plan_dir = path.parent
    name = str(fm.get("name") or plan_dir.name)
    full_name = str(fm.get("full_name") or name)
    kind = "reroute" if is_reroute else "parallel"
    status = str(fm.get("status", "active"))
    try:
        order = int(fm.get("order", 999))
    except (TypeError, ValueError):
        order = 999
    reviewed = fm.get("reviewed")
    if isinstance(reviewed, str):
        reviewed = reviewed.lower() == "true"
    if not isinstance(reviewed, bool):
        reviewed = None

    return Reroute(
        plan_dir=plan_dir,
        name=name,
        full_name=full_name,
        kind=kind,
        status=status,
        order=order,
        reviewed=reviewed,
    )


def parse_fix_section(path: Path) -> FixSection | None:
    try:
        text = read_text(path)
    except OSError:
        return None
    fm, _ = split_frontmatter(text)
    if not fm:
        return None

    bug_id = str(fm.get("bug", path.stem))
    title = str(fm.get("title", ""))
    severity = str(fm.get("severity", "unknown"))
    status = str(fm.get("status", "unknown"))
    subsystem = str(fm.get("subsystem", ""))
    found = fm.get("found")
    if found is not None:
        found = str(found)

    tpr_block = fm.get("third_party_review") or {}
    if isinstance(tpr_block, dict):
        tpr_status = str(tpr_block.get("status", "none"))
        tpr_updated = tpr_block.get("updated")
        if tpr_updated is not None:
            tpr_updated = str(tpr_updated)
    else:
        tpr_status = "none"
        tpr_updated = None

    return FixSection(
        path=path,
        bug_id=bug_id,
        title=title,
        severity=severity,
        status=status,
        subsystem=subsystem,
        found=found,
        tpr_status=tpr_status,
        tpr_updated=tpr_updated,
    )


def parse_bug_tracker_bugs(plan: Plan) -> list[Bug]:
    """Scan bug-tracker section-*.md files for `- [ ] BUG-XX-NNN ...` entries."""
    bugs: list[Bug] = []
    for section in plan.sections:
        try:
            text = read_text(section.path)
        except OSError:
            continue
        for idx, line in enumerate(text.splitlines()):
            m = BUG_TRACKER_ENTRY_RE.match(line)
            if not m:
                continue
            checked_mark, bid, desc = m.groups()
            sev_m = BUG_SEVERITY_RE.search(line) or BUG_SEVERITY_RE.search(desc)
            severity = sev_m.group(1).lower() if sev_m else "unknown"
            bugs.append(
                Bug(
                    id=bid,
                    severity=severity,
                    status="fixed" if checked_mark.lower() == "x" else "open",
                    title=desc.strip().strip("*").strip(),
                    lineno=idx + 1,
                    source_section=section.path.name,
                )
            )
    return bugs


def parse_dependency_graph(overview_path: Path) -> dict[str, list[str]]:
    """Extract child → parent edges from a `## Dependency Graph` fence block.

    Scans every ``` fence under the `## Dependency Graph` header and builds
    edges from sequential Section N mentions on the same or continuation lines.
    """
    if not overview_path.exists():
        return {}
    text = read_text(overview_path)
    lines = text.splitlines()
    in_graph = False
    in_code = False
    edges: dict[str, list[str]] = {}
    last_sec = ""

    for line in lines:
        if line.startswith("## Dependency Graph"):
            in_graph = True
            continue
        if in_graph and line.startswith("## "):
            break
        if not in_graph:
            continue
        if line.startswith("```"):
            in_code = not in_code
            continue
        if not in_code:
            continue
        if not line.strip():
            continue

        is_continuation = line[:1] in (" ", "\t")
        prev = last_sec if is_continuation else ""
        matches = re.findall(r"Section (\w+)", line)
        for sec in matches:
            if prev and sec != prev:
                edges.setdefault(sec, []).append(prev)
            prev = sec
        if prev:
            last_sec = prev

    return edges


# ─── Crawler ──────────────────────────────────────────────────────────────────


def crawl_plan(plan_dir: Path) -> Plan:
    plan = Plan(name=plan_dir.name, dir=plan_dir)

    index_path = plan_dir / "index.md"
    if index_path.exists():
        plan.reroute = parse_index_file(index_path)
        text = read_text(index_path)
        fm, _ = split_frontmatter(text)
        plan.index = fm or None

    overview_path = plan_dir / "00-overview.md"
    if overview_path.exists():
        text = read_text(overview_path)
        fm, _ = split_frontmatter(text)
        plan.overview = fm or None
        plan.dep_graph = parse_dependency_graph(overview_path)

    section_files = sorted(plan_dir.glob("section-*.md"))
    for sf in section_files:
        section = parse_section_file(plan, sf)
        if section:
            plan.sections.append(section)
    plan.sections.sort(key=lambda s: s.sort_key)

    return plan


def crawl_workspace(repo_root: Path, explicit_plan_dir: Path | None = None) -> Workspace:
    plans_dir = repo_root / "plans"
    ws = Workspace(repo_root=repo_root, plans_dir=plans_dir)

    if not plans_dir.exists():
        trace(f"no plans dir at {plans_dir}")
        return ws

    # Walk every direct subdirectory of plans/
    for sub in sorted(plans_dir.iterdir()):
        if not sub.is_dir():
            continue
        if sub.name == "completed":
            for cp in sorted(sub.iterdir()):
                if cp.is_dir():
                    plan = crawl_plan(cp)
                    ws.completed_plans[cp.name] = plan
            continue
        plan = crawl_plan(sub)
        ws.all_plans[sub.name] = plan
        if plan.reroute:
            ws.reroutes.append(plan.reroute)
        if sub.name == "bug-tracker":
            plan.bugs = parse_bug_tracker_bugs(plan)
            for fx in sorted(sub.glob("fix-BUG-*.md")):
                fs = parse_fix_section(fx)
                if fs:
                    plan.fix_sections.append(fs)
            ws.bug_tracker = plan

    # Focus selection
    if explicit_plan_dir:
        key = explicit_plan_dir.name
        if key in ws.all_plans:
            ws.focus_plan = ws.all_plans[key]
            ws.focus_reason = f"explicit argument: {explicit_plan_dir}"
        else:
            trace(f"explicit plan dir not found: {explicit_plan_dir}")
    else:
        actives = ws.active_reroutes()
        reroute_actives = [r for r in actives if r.kind == "reroute"]
        if reroute_actives:
            top = reroute_actives[0]
            ws.focus_plan = ws.all_plans.get(top.plan_dir.name)
            ws.focus_reason = f"highest-priority active reroute (order={top.order})"
        elif "roadmap" in ws.all_plans:
            ws.focus_plan = ws.all_plans["roadmap"]
            ws.focus_reason = "default: no active reroutes → main roadmap"

    # Focus section: first incomplete in the focus plan
    if ws.focus_plan:
        for sec in ws.focus_plan.sections:
            if sec.unchecked > 0 or sec.status != "complete":
                ws.focus_section = sec
                ws.focus_section_reason = (
                    f"first incomplete section ({sec.status}, {sec.checked}/{sec.total})"
                )
                break

    return ws


# ─── Analysis ─────────────────────────────────────────────────────────────────


def classify_blocker_readiness(
    section_number: str,
    focus_plan: Plan,
    workspace: Workspace,
    focus_section: "Section | None" = None,
) -> tuple[str, str]:
    """Return (label, detail) for a blocking section/subsection/gate.

    Resolution order:
      1. subsection within the focus section (e.g. `07.3.A`)
      2. section in the focus plan (e.g. `18`)
      3. section in any plan (cross-plan reference)
      4. free-text gate name (not matching any known ID)

    Labels: DONE, IN_PROGRESS, READY, WAITING, GATE, UNKNOWN
    """
    # 1. Check if the ref is a subsection within the focus section
    if focus_section:
        for sub in focus_section.subsections:
            if sub.id == section_number:
                if sub.status == "complete":
                    return "DONE", f"subsection {sub.id} complete"
                if sub.status == "in-progress":
                    pct = sub.checked * 100 // sub.total if sub.total else 0
                    return "IN_PROGRESS", f"subsection {sub.id} at {pct}%"
                return "WAITING", f"subsection {sub.id} not-started"

    # 2 & 3. Check sections in focus plan, then all plans
    section = None
    for s in focus_plan.sections:
        if s.number.strip('"') == section_number.strip('"'):
            section = s
            break
    if not section:
        for plan in workspace.all_plans.values():
            for s in plan.sections:
                if s.number.strip('"') == section_number.strip('"'):
                    section = s
                    break
            if section:
                break

    if not section:
        # Not a section-graph ref — free-text gate name
        if not SECTION_ID_LIKE_RE.match(section_number):
            return "GATE", "free-text gate (no planned resolution section)"
        return "UNKNOWN", "no matching section found"
    if section.status == "complete":
        return "DONE", f"{section.checked}/{section.total} complete"
    if section.status == "in-progress":
        return "IN_PROGRESS", f"{section.pct}% complete"

    # Walk dependency chain (max 5 hops)
    chain: list[str] = []
    current = section_number
    all_ok = True
    for _ in range(5):
        parents = focus_plan.dep_graph.get(current, [])
        if not parents:
            break
        blocker_found = False
        for parent in parents:
            psec = next(
                (s for s in focus_plan.sections if s.number == parent or s.number == f'"{parent}"'),
                None,
            )
            if psec and psec.status != "complete":
                all_ok = False
                chain.append(f"Section {parent} [{psec.status}]")
                current = parent
                blocker_found = True
                break
        if not blocker_found:
            break
    if all_ok:
        return "READY", "deps satisfied" if focus_plan.dep_graph.get(section_number) else "no deps"
    return "WAITING", " ← ".join(chain)


def detect_all_mismatches(workspace: Workspace) -> list[tuple[str, str, str]]:
    """Scan every plan for frontmatter/body mismatches. Returns (plan, section, desc)."""
    out: list[tuple[str, str, str]] = []
    for plan in workspace.all_plans.values():
        for section in plan.sections:
            if section.mismatch:
                out.append((plan.name, section.path.name, section.mismatch))
            for sub in section.subsections:
                if sub.mismatch and sub.mismatch != "no-checkboxes":
                    out.append((plan.name, f"{section.path.name}#{sub.id}", sub.mismatch))
    return out


SECTION_ID_LIKE_RE = re.compile(r"^\d+(?:\.[\w.\-]+)?$")


def detect_orphan_blockers(workspace: Workspace) -> list[tuple[str, int, str]]:
    """Find `<!-- blocked-by:X -->` where X looks like a section ID but isn't known.

    Non-numeric refs (e.g. `NICHE_CODEGEN_READY`, `bug-tracker`, `plans/foo`) are
    treated as free-text gate names or cross-plan pointers — not orphans in the
    section-graph sense. Only refs matching `\\d+(\\.\\w+)*` are validated.
    """
    known_ids: set[str] = set()
    for plan in workspace.all_plans.values():
        for section in plan.sections:
            known_ids.add(section.number.strip('"'))
            for sub in section.subsections:
                known_ids.add(sub.id)

    orphans: list[tuple[str, int, str]] = []
    for plan in workspace.all_plans.values():
        for section in plan.sections:
            for item in section.flat_items:
                for b in item.own_blockers:
                    if not SECTION_ID_LIKE_RE.match(b):
                        continue  # free-text gate name, not a section-graph ref
                    root = b.split(".")[0]
                    if root not in known_ids and b not in known_ids:
                        orphans.append((section.path.name, item.lineno, b))
    return orphans


def count_stale_plan_annotations(repo_root: Path) -> int | None:
    """Invoke plan-annotations.sh --count (if available) for stale count."""
    script = repo_root / ".claude/skills/impl-hygiene-review/plan-annotations.sh"
    if not script.exists():
        return None
    import subprocess
    try:
        result = subprocess.run(
            ["bash", str(script), "--count"],
            capture_output=True,
            text=True,
            timeout=30,
            cwd=repo_root,
        )
    except (subprocess.TimeoutExpired, OSError):
        return None
    if result.returncode != 0:
        return None
    total = 0
    for line in result.stdout.splitlines():
        m = re.match(r"\s*(stale-resolved|stale-completed-plan)\s*[:=]?\s*(\d+)", line)
        if m:
            total += int(m.group(2))
    return total


# ─── Renderer ─────────────────────────────────────────────────────────────────


STATUS_TAG = {
    "complete": "done  ",
    "in-progress": "active",
    "not-started": "todo  ",
    "unknown": "???   ",
}


def tag_for(status: str) -> str:
    return STATUS_TAG.get(status, "???   ")


def short(text: str, limit: int = 100) -> str:
    text = text.replace("\n", " ")
    if len(text) <= limit:
        return text
    return text[: limit - 1] + "…"


def _plan_dir_display(plan_dir: Path, repo_root: Path) -> str:
    """Show plans/<name> instead of an absolute path."""
    try:
        return plan_dir.relative_to(repo_root).as_posix()
    except ValueError:
        return plan_dir.as_posix()


def render_reroutes(ws: Workspace, include_progress: bool = True) -> list[str]:
    """Render the legacy `=== REROUTES ===` block for create-plan sed compat."""
    out: list[str] = []
    if not ws.reroutes:
        return out
    out.append("=== REROUTES ===")
    for r in ws.active_reroutes():
        prog = ""
        if include_progress and r.plan_dir.name in ws.all_plans:
            p = ws.all_plans[r.plan_dir.name]
            prog = f" — {p.total_checked}/{p.total_items} ({p.pct}%)"
        disp = _plan_dir_display(r.plan_dir, ws.repo_root)
        out.append(f"[ACTIVE {r.kind}] {r.full_name} — {disp}{prog} (order: {r.order})")
    for r in ws.queued_reroutes():
        disp = _plan_dir_display(r.plan_dir, ws.repo_root)
        out.append(f"[queued {r.kind}] {r.full_name} — {disp} (order: {r.order})")
    out.append("")
    return out


def render_workspace_summary(ws: Workspace, repo_root: Path) -> list[str]:
    out: list[str] = ["## Workspace Summary", ""]
    out.append(f"Root: {repo_root}")
    active = ws.active_reroutes()
    queued = ws.queued_reroutes()
    out.append(
        f"Plans: {len(ws.all_plans)} total "
        f"({sum(1 for p in ws.all_plans.values() if p.reroute and p.reroute.kind == 'reroute')} reroute plans, "
        f"{sum(1 for p in ws.all_plans.values() if p.reroute and p.reroute.kind == 'parallel')} parallel, "
        f"{len(ws.completed_plans)} completed archived)"
    )
    out.append(f"Active reroutes: {sum(1 for r in active if r.kind == 'reroute')} "
               f"(reroute); {sum(1 for r in active if r.kind == 'parallel')} (parallel)")
    out.append(f"Queued reroutes: {len(queued)}")
    out.append("")
    return out


def render_health_signals(ws: Workspace, repo_root: Path, quiet: bool) -> list[str]:
    if quiet:
        return []
    out: list[str] = ["## Health Signals", ""]
    mismatches = detect_all_mismatches(ws)
    orphans = detect_orphan_blockers(ws)
    unreviewed = [
        p.name for p in ws.all_plans.values()
        if p.reroute and p.reroute.reviewed is False
    ]

    # TPR rollup across the whole workspace
    tpr_open_total = 0
    tpr_by_plan: dict[str, int] = {}
    for plan in ws.all_plans.values():
        n = plan.open_tpr_count
        tpr_open_total += n
        if n > 0:
            tpr_by_plan[plan.name] = n

    open_fix_sections: list[str] = []
    if ws.bug_tracker:
        for fx in ws.bug_tracker.fix_sections:
            if fx.status in ("in-progress", "open"):
                open_fix_sections.append(f"{fx.bug_id} ({fx.severity}, {fx.status})")

    open_bugs_by_sev: dict[str, int] = {}
    if ws.bug_tracker:
        for b in ws.bug_tracker.bugs:
            if b.status != "fixed":
                open_bugs_by_sev[b.severity] = open_bugs_by_sev.get(b.severity, 0) + 1

    out.append(f"frontmatter mismatches: {len(mismatches)}")
    if mismatches:
        for plan, loc, desc in mismatches[:10]:
            out.append(f"  {plan}/{loc}: {desc}")
        if len(mismatches) > 10:
            out.append(f"  ... {len(mismatches) - 10} more")
    out.append(f"orphan blockers: {len(orphans)}")
    if orphans:
        for path, lineno, ref in orphans[:10]:
            out.append(f"  {path}:{lineno} references unknown '{ref}'")
        if len(orphans) > 10:
            out.append(f"  ... {len(orphans) - 10} more")
    out.append(f"unreviewed plans: {len(unreviewed)} {unreviewed if unreviewed else ''}")
    out.append(f"open TPR findings (all plans): {tpr_open_total}")
    for pname, n in sorted(tpr_by_plan.items(), key=lambda x: -x[1]):
        out.append(f"  {pname}: {n}")
    out.append(f"open fix sections: {len(open_fix_sections)}")
    for fs in open_fix_sections:
        out.append(f"  {fs}")
    if open_bugs_by_sev:
        sev_str = ", ".join(f"{k}: {v}" for k, v in sorted(open_bugs_by_sev.items()))
        out.append(f"bug tracker open by severity: {sev_str}")
    stale = count_stale_plan_annotations(repo_root)
    if stale is not None:
        out.append(f"stale plan annotations (source code): {stale}")
    out.append("")
    return out


def render_focus_selection(ws: Workspace) -> list[str]:
    out: list[str] = ["## Focus Selection", ""]
    if not ws.focus_plan:
        out.append("No focus plan selected (no active reroutes, no main roadmap).")
        out.append("")
        return out
    out.append(f"Selected plan: plans/{ws.focus_plan.name}")
    out.append(f"Reason: {ws.focus_reason}")
    if ws.focus_plan.reroute and ws.focus_plan.reroute.reviewed is False:
        out.append("NOTE: this plan has reviewed: false — /review-plan gate may apply")
    if ws.focus_section:
        out.append(f"Selected section: {ws.focus_section.number} {ws.focus_section.title}")
        out.append(f"Reason: {ws.focus_section_reason}")
    else:
        out.append("Selected section: (all sections complete in focus plan)")
    out.append("")
    return out


def render_plan_overview(plan: Plan) -> list[str]:
    out: list[str] = [f"## Plan: plans/{plan.name}", ""]
    if plan.reroute:
        out.append(
            f"kind: {plan.reroute.kind}, status: {plan.reroute.status}, order: {plan.reroute.order}"
        )
        out.append(f"full name: {plan.reroute.full_name}")
        if plan.reroute.reviewed is not None:
            out.append(f"reviewed: {plan.reroute.reviewed}")
    out.append(f"progress: {plan.total_checked}/{plan.total_items} ({plan.pct}%)")
    counts = plan.section_status_counts
    out.append(
        f"sections: {counts['complete']} done / "
        f"{counts['in-progress']} in-progress / "
        f"{counts['not-started']} not-started"
    )
    out.append("")
    out.append("Sections:")
    for sec in plan.sections:
        marker = "FOCUS " if sec.status != "complete" and all(
            s.status == "complete" for s in plan.sections if s.sort_key < sec.sort_key
        ) else tag_for(sec.status)
        num = str(sec.number).strip('"')
        title = str(sec.title).strip('"')
        prog = f"({sec.checked}/{sec.total}, {sec.pct}%)" if sec.total else "(0/0)"
        mismatch_note = f" !! {sec.mismatch}" if sec.mismatch else ""
        tpr_note = ""
        if sec.tpr_status == "findings":
            open_n = sum(1 for f in sec.tpr_findings if not f.resolved)
            tpr_note = f" [TPR: {open_n} open]"
        out.append(f"  [{marker}] {num:>4}  {title}  {prog}{tpr_note}{mismatch_note}")
    out.append("")
    return out


def render_focus_section(section: Section, workspace: Workspace) -> list[str]:
    if not section:
        return []
    out: list[str] = [f"## Focus Section: {section.number} {section.title}", ""]
    out.append(f"file: {section.path.relative_to(workspace.repo_root)}")
    out.append(f"status: {section.status}")
    if section.reviewed is not None:
        out.append(f"reviewed: {section.reviewed}")
    if section.tpr_status != "none":
        open_n = sum(1 for f in section.tpr_findings if not f.resolved)
        out.append(
            f"third_party_review: {section.tpr_status} "
            f"({open_n} open finding{'s' if open_n != 1 else ''}, updated {section.tpr_updated or 'unknown'})"
        )
    if section.depends_on:
        out.append(f"depends_on: {', '.join(section.depends_on)}")
    out.append(
        f"progress: {section.checked}/{section.total} ({section.pct}%) — "
        f"{len(section.unblocked_items)} unblocked, {len(section.blocked_items)} blocked"
    )
    if section.mismatch:
        out.append(f"MISMATCH: {section.mismatch}")
    out.append("")

    # Subsections table
    if section.subsections:
        out.append("Subsections:")
        for sub in section.subsections:
            tag = tag_for(sub.status)
            prog = f"({sub.checked}/{sub.total})"
            if sub.total > 0:
                pct = sub.checked * 100 // sub.total
                prog = f"({sub.checked}/{sub.total}, {pct}%)"
            blocked = f" [{sub.blocked_count} blocked]" if sub.blocked_count else ""
            mm = f" !! {sub.mismatch}" if sub.mismatch and sub.mismatch != "no-checkboxes" else ""
            empty = " (no checkboxes)" if sub.mismatch == "no-checkboxes" else ""
            sid = str(sub.id).strip('"')
            title = str(sub.title).strip('"')
            out.append(f"  [{tag}] {sid:<6} {title}  {prog}{blocked}{empty}{mm}")
        out.append("")

    # Recently completed
    recent = [i for i in section.flat_items if i.checked][-3:]
    if recent:
        out.append("Recently completed:")
        for i in recent:
            out.append(f"  L{i.lineno}: {short(i.content, 120)}")
        out.append("")

    # Unblocked items grouped by subsection
    unblocked = section.unblocked_items
    if unblocked:
        by_sub: dict[str, list[Item]] = {}
        for i in unblocked:
            by_sub.setdefault(i.subsection_id, []).append(i)

        out.append(f"Next unblocked items ({len(unblocked)} total):")
        # Order by subsection frontmatter order, then unknowns last
        sub_order = [s.id for s in section.subsections]
        order_key = lambda sid: (sub_order.index(sid) if sid in sub_order else 999, sid)
        for sid in sorted(by_sub.keys(), key=order_key):
            sub = next((s for s in section.subsections if s.id == sid), None)
            title = f" {sub.title}" if sub else ""
            items = by_sub[sid]
            out.append(f"  ## {sid}{title} ({len(items)} items)")
            for item in items[:10]:
                out.append(f"    L{item.lineno}: {short(item.content, 100)}")
            if len(items) > 10:
                out.append(f"    ... {len(items) - 10} more")
        out.append("")

    # Blocker breakdown
    if section.blocked_items:
        blockers_by_ref: dict[str, list[Item]] = {}
        prose_blockers: list[Item] = []
        for i in section.blocked_items:
            refs = i.effective_blockers
            if refs:
                for ref in refs:
                    blockers_by_ref.setdefault(ref, []).append(i)
            elif i.prose_blocker:
                prose_blockers.append(i)

        out.append(f"Blockers ({len(section.blocked_items)} items blocked):")
        for ref, items in sorted(blockers_by_ref.items()):
            label, detail = classify_blocker_readiness(ref, section.plan, workspace, section)
            out.append(f"  {ref} [{label}: {detail}] — blocks {len(items)} items")
            subs_affected = sorted({i.subsection_id for i in items})
            out.append(f"    in subsections: {', '.join(subs_affected)}")
        if prose_blockers:
            out.append(f"  prose blockers (unplanned): {len(prose_blockers)} items")
            for i in prose_blockers[:5]:
                out.append(f"    L{i.lineno}: {short(i.prose_blocker or '', 80)}")
            if len(prose_blockers) > 5:
                out.append(f"    ... {len(prose_blockers) - 5} more")
        out.append("")

    # Open TPR findings
    open_findings = [f for f in section.tpr_findings if not f.resolved]
    if open_findings:
        out.append(f"Open TPR findings ({len(open_findings)}):")
        by_sev: dict[str, list[TprFinding]] = {}
        for f in open_findings:
            by_sev.setdefault(f.severity, []).append(f)
        for sev in ("critical", "high", "medium", "low"):
            if sev in by_sev:
                for f in by_sev[sev]:
                    out.append(f"  L{f.lineno} [{f.id}][{sev}]")
        out.append("")

    return out


def render_bug_tracker_relevance(
    workspace: Workspace,
    focus: Section | None,
    skip: bool,
) -> list[str]:
    if skip or not workspace.bug_tracker or not focus:
        return []
    out: list[str] = []
    bt = workspace.bug_tracker
    relevant: list[Bug] = [b for b in bt.bugs if b.status != "fixed" and b.severity in ("critical", "high")]
    if relevant:
        out.append("## Bug Tracker (open critical/high)")
        out.append("")
        for b in relevant[:20]:
            out.append(f"  {b.id} [{b.severity}, {b.status}] {short(b.title, 80)}")
        if len(relevant) > 20:
            out.append(f"  ... {len(relevant) - 20} more")
        out.append("")
    open_fixes = [
        fx for fx in bt.fix_sections if fx.status in ("in-progress", "open")
    ]
    if open_fixes:
        out.append("Open fix sections:")
        for fx in open_fixes:
            tpr = ""
            if fx.tpr_status == "findings":
                tpr = f", TPR: findings (updated {fx.tpr_updated or '?'})"
            out.append(f"  {fx.path.name}: {fx.bug_id} [{fx.severity}, {fx.status}]{tpr}")
            if fx.title:
                out.append(f"    {short(fx.title, 100)}")
        out.append("")
    return out


def render_decision_notes(ws: Workspace) -> list[str]:
    out: list[str] = ["## Decision Notes for continue-roadmap Workflow", ""]
    if ws.focus_plan and ws.focus_plan.reroute:
        if ws.focus_plan.reroute.reviewed is False:
            out.append("- Step 1.7 Unreviewed Plan Gate applies: focus plan has reviewed: false")
    if ws.focus_section and ws.focus_section.tpr_status == "findings":
        n = sum(1 for f in ws.focus_section.tpr_findings if not f.resolved)
        out.append(f"- Step 1.9 TPR Triage Gate applies: {n} open findings in focus section")
    if ws.focus_section and ws.focus_section.mismatch:
        out.append("- Step 1.5 Stale Frontmatter auto-fix applies to focus section")
    # Simple git check for tree cleanliness (non-fatal if fails)
    import subprocess
    try:
        result = subprocess.run(
            ["git", "status", "--short"],
            capture_output=True, text=True, timeout=10, cwd=ws.repo_root,
        )
        if result.returncode == 0:
            pending = [l for l in result.stdout.splitlines() if l.strip()]
            if pending:
                out.append(f"- Step 1.95 Clean Tree Gate applies: {len(pending)} files pending")
                for line in pending[:5]:
                    out.append(f"    {line}")
                if len(pending) > 5:
                    out.append(f"    ... {len(pending) - 5} more")
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    out.append("")
    return out


def render_rich(ws: Workspace, repo_root: Path, quiet: bool, skip_bugs: bool) -> str:
    lines: list[str] = []
    lines.extend(render_reroutes(ws))
    lines.extend(render_workspace_summary(ws, repo_root))
    lines.extend(render_health_signals(ws, repo_root, quiet))
    lines.extend(render_focus_selection(ws))
    if ws.focus_plan:
        lines.extend(render_plan_overview(ws.focus_plan))
    if ws.focus_section:
        lines.extend(render_focus_section(ws.focus_section, ws))
    lines.extend(render_bug_tracker_relevance(ws, ws.focus_section, skip_bugs))
    lines.extend(render_decision_notes(ws))
    return "\n".join(lines).rstrip() + "\n"


def render_json(ws: Workspace) -> str:
    def plan_dict(p: Plan) -> dict:
        return {
            "name": p.name,
            "dir": str(p.dir),
            "progress": {"checked": p.total_checked, "total": p.total_items, "pct": p.pct},
            "section_counts": p.section_status_counts,
            "open_tpr": p.open_tpr_count,
            "reroute": None if not p.reroute else {
                "kind": p.reroute.kind,
                "status": p.reroute.status,
                "order": p.reroute.order,
                "name": p.reroute.name,
                "full_name": p.reroute.full_name,
                "reviewed": p.reroute.reviewed,
            },
            "sections": [
                {
                    "number": s.number,
                    "title": s.title,
                    "status": s.status,
                    "checked": s.checked,
                    "total": s.total,
                    "pct": s.pct,
                    "tpr_status": s.tpr_status,
                    "tpr_open": sum(1 for f in s.tpr_findings if not f.resolved),
                    "mismatch": s.mismatch,
                    "unblocked_count": len(s.unblocked_items),
                    "blocked_count": len(s.blocked_items),
                }
                for s in p.sections
            ],
        }

    data = {
        "repo_root": str(ws.repo_root),
        "plans": {name: plan_dict(p) for name, p in ws.all_plans.items()},
        "completed": list(ws.completed_plans.keys()),
        "focus": {
            "plan": ws.focus_plan.name if ws.focus_plan else None,
            "reason": ws.focus_reason,
            "section": ws.focus_section.number if ws.focus_section else None,
            "section_reason": ws.focus_section_reason,
        },
        "health": {
            "mismatches": detect_all_mismatches(ws),
            "orphan_blockers": detect_orphan_blockers(ws),
            "unreviewed": [
                p.name for p in ws.all_plans.values()
                if p.reroute and p.reroute.reviewed is False
            ],
        },
    }
    return json.dumps(data, indent=2, default=str) + "\n"


# ─── Entry point ──────────────────────────────────────────────────────────────


def find_repo_root(start: Path) -> Path:
    cur = start.resolve()
    for _ in range(10):
        if (cur / "plans").exists() and (cur / ".claude").exists():
            return cur
        if cur.parent == cur:
            break
        cur = cur.parent
    return Path.cwd()


def main(argv: list[str] | None = None) -> int:
    global TRACE_ENABLED
    parser = argparse.ArgumentParser(
        description="Comprehensive roadmap scanner for /continue-roadmap",
    )
    parser.add_argument("plan_dir", nargs="?", default=None,
                        help="plan directory to focus (default: auto from reroutes)")
    parser.add_argument("focus_section", nargs="?", default=None,
                        help="specific section number within the plan")
    parser.add_argument("--json", action="store_true", help="emit structured JSON")
    parser.add_argument("--reroutes-only", action="store_true",
                        help="emit only the === REROUTES === block")
    parser.add_argument("--no-bugs", action="store_true", help="skip bug-tracker crawl")
    parser.add_argument("--trace", action="store_true", help="log decisions to stderr")
    parser.add_argument("--quiet", action="store_true", help="suppress health signals")
    args = parser.parse_args(argv)

    TRACE_ENABLED = args.trace

    repo_root = find_repo_root(Path.cwd())
    trace(f"repo_root = {repo_root}")

    explicit_plan_dir = None
    if args.plan_dir:
        p = Path(args.plan_dir)
        if not p.is_absolute():
            p = (repo_root / p).resolve()
        explicit_plan_dir = p
        trace(f"explicit plan dir: {explicit_plan_dir}")

    ws = crawl_workspace(repo_root, explicit_plan_dir)

    # If explicit was given and matches a reroute plan, no auto-delegation
    # If no explicit and there's an active reroute, focus was already set to the reroute
    # Still: if an explicit focus_section was given, lock the focus_section to it
    if args.focus_section and ws.focus_plan:
        for s in ws.focus_plan.sections:
            if s.number == args.focus_section or s.number.strip('"') == args.focus_section:
                ws.focus_section = s
                ws.focus_section_reason = f"explicit focus section: {args.focus_section}"
                break

    if args.reroutes_only:
        print("\n".join(render_reroutes(ws)).rstrip())
        return 0
    if args.json:
        sys.stdout.write(render_json(ws))
        return 0

    sys.stdout.write(render_rich(ws, repo_root, args.quiet, args.no_bugs))
    return 0


if __name__ == "__main__":
    sys.exit(main())
