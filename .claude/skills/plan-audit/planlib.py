#!/usr/bin/env python3
"""Shared plan parsing library for Ori compiler plan tooling.

SSOT for plan file discovery, YAML frontmatter parsing, checkbox extraction,
and section/subsection data models. Consumers:
  - plan-audit.py      (mechanical plan verification)
  - roadmap_scan.py    (roadmap navigation / continue-roadmap)
  - plan-annotations.py (stale annotation detection)

Extracted from roadmap_scan.py (2026-04-09) to avoid SSOT violation from
multiple scripts re-parsing the same plan structure independently.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

# ─── Constants ───────────────────────────────────────────────────────────────

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
PLANS_DIR = REPO_ROOT / "plans"
COMPLETED_DIR = PLANS_DIR / "completed"

# ─── Regex patterns ──────────────────────────────────────────────────────────

FRONTMATTER_RE = re.compile(r"^---\s*$")
HEADER_RE = re.compile(r"^## +(.+?)\s*$")
SUBSECTION_ID_RE = re.compile(r"^§?([\w.\-]+)(?::|\s|$)")
CHECKBOX_RE = re.compile(r"^(\s*)- \[( |x|X)\] +(.*)$")
BLOCKED_BY_RE = re.compile(r"<!--\s*blocked-by:([A-Za-z0-9.\-]+)(?:\s*-->)?")
BLOCKED_PROSE_RE = re.compile(r"<!--\s*blocked:\s*(.+?)\s*-->")

# Plan text patterns — file:line references and symbol mentions
FILE_LINE_RE = re.compile(r"`([a-zA-Z0-9_/.\-]+\.rs):(\d+)`")
FILE_REF_RE = re.compile(r"`(compiler/[a-zA-Z0-9_/.\-]+\.rs)`")
BACKTICK_SYMBOL_RE = re.compile(r"`([A-Z][A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)`")
DEFERRAL_WORDS = re.compile(
    r"\b(bonus|future\s+(?:work|improvement|enhancement)|nice[\s-]to[\s-]have|"
    r"stretch\s+goal|lower\s+priority|optional|if\s+time\s+permits)\b",
    re.IGNORECASE,
)

# ─── Data model ──────────────────────────────────────────────────────────────


@dataclass
class Item:
    """A single `- [ ]` / `- [x]` checkbox line."""
    lineno: int
    indent: int
    content: str
    raw_line: str
    checked: bool
    subsection_id: str
    own_blockers: list[str] = field(default_factory=list)
    inherited_blockers: list[str] = field(default_factory=list)
    prose_blocker: str | None = None

    @property
    def blocked(self) -> bool:
        return bool(self.own_blockers or self.inherited_blockers or self.prose_blocker)


@dataclass
class Subsection:
    """A `## NN.M Title` block."""
    id: str
    title: str
    status: str
    items: list[Item] = field(default_factory=list)
    title_from_header: str = ""

    @property
    def checked(self) -> int:
        return sum(1 for i in self.items if i.checked)

    @property
    def unchecked(self) -> int:
        return sum(1 for i in self.items if not i.checked)

    @property
    def total(self) -> int:
        return len(self.items)

    def status_mismatch(self) -> str | None:
        if self.total == 0:
            return None
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
class SectionInfo:
    """Parsed section-*.md file — core fields for plan auditing."""
    path: Path
    filename: str
    number: str
    title: str
    status: str
    reviewed: bool | None
    goal: str | None
    success_criteria: list[str]
    depends_on: list[str]
    tpr_status: str
    tpr_updated: str | None
    subsections: list[Subsection]
    flat_items: list[Item]
    frontmatter: dict[str, Any]
    body_text: str

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

    def status_mismatch(self) -> str | None:
        if self.status == "complete" and self.unchecked > 0:
            return f"frontmatter=complete but {self.unchecked} unchecked"
        if self.status == "not-started" and self.checked > 0:
            return f"frontmatter=not-started but {self.checked} checked"
        if self.status == "in-progress" and self.unchecked == 0 and self.total > 0:
            return "frontmatter=in-progress but all items checked"
        return None

    def extract_file_refs(self) -> list[str]:
        """Extract all file path references from the body text."""
        refs = set()
        for m in FILE_REF_RE.finditer(self.body_text):
            refs.add(m.group(1))
        for m in FILE_LINE_RE.finditer(self.body_text):
            refs.add(m.group(1))
        return sorted(refs)

    def extract_file_line_refs(self) -> list[tuple[str, int]]:
        """Extract (file, line) pairs from `file.rs:123` patterns."""
        return [(m.group(1), int(m.group(2))) for m in FILE_LINE_RE.finditer(self.body_text)]

    def extract_symbols(self) -> list[str]:
        """Extract PascalCase symbol references from backtick code spans."""
        return sorted(set(m.group(1) for m in BACKTICK_SYMBOL_RE.finditer(self.body_text)))

    def extract_deferral_language(self) -> list[tuple[int, str, str]]:
        """Find deferral language. Returns [(lineno, word, context)]."""
        results = []
        for i, line in enumerate(self.body_text.splitlines(), 1):
            for m in DEFERRAL_WORDS.finditer(line):
                results.append((i, m.group(0), line.strip()))
        return results


@dataclass
class OverviewInfo:
    """Parsed 00-overview.md — plan-level metadata."""
    path: Path
    plan_name: str
    title: str
    status: str
    frontmatter: dict[str, Any]
    body_text: str
    mission_criteria: list[str]

    def extract_mission_criteria(self) -> list[str]:
        """Extract mission success criteria from body text."""
        criteria = []
        in_criteria = False
        for line in self.body_text.splitlines():
            if re.match(r"^#{1,3}\s+.*(?:mission|success)\s+criteria", line, re.IGNORECASE):
                in_criteria = True
                continue
            if in_criteria:
                if line.startswith("#"):
                    break
                cm = re.match(r"^\s*[-*]\s+(.+)", line)
                if cm:
                    criteria.append(cm.group(1).strip())
        return criteria


@dataclass
class IndexInfo:
    """Parsed index.md — reroute/parallel metadata."""
    path: Path
    frontmatter: dict[str, Any]
    is_reroute: bool
    is_parallel: bool
    name: str
    full_name: str
    status: str
    order: int


@dataclass
class PlanInfo:
    """A complete parsed plan directory."""
    name: str
    dir: Path
    overview: OverviewInfo | None
    index: IndexInfo | None
    sections: list[SectionInfo]

    @property
    def total_checked(self) -> int:
        return sum(s.checked for s in self.sections)

    @property
    def total_items(self) -> int:
        return sum(s.total for s in self.sections)

    @property
    def pct(self) -> int:
        return (self.total_checked * 100 // self.total_items) if self.total_items else 0

    def section_by_number(self, num: str) -> SectionInfo | None:
        for s in self.sections:
            if s.number == num or s.number.lstrip("0") == num.lstrip("0"):
                return s
        return None


# ─── Parsing functions ───────────────────────────────────────────────────────


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def split_frontmatter(text: str) -> tuple[dict[str, Any], int]:
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
    except yaml.YAMLError:
        data = {}
    return data, end + 1


def detect_subsection_id(header_text: str) -> str:
    stripped = header_text.replace('"', "").strip()
    m = SUBSECTION_ID_RE.match(stripped)
    return m.group(1).rstrip(":") if m else "?"


def parse_section_body(lines: list[str], body_start: int) -> tuple[list[Item], dict[str, str]]:
    """Walk markdown body, extract checkboxes grouped by `## ID` headers.
    Returns (flat_items, header_titles_by_id). Skips fenced code blocks.
    """
    items: list[Item] = []
    header_titles: dict[str, str] = {}
    cur_sub = "?"
    parent_blocker: list[str] = []
    in_fence = False

    for idx in range(body_start, len(lines)):
        line = lines[idx]
        lineno = idx + 1

        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue

        if line.startswith("### "):
            parent_blocker = []
            continue

        hm = HEADER_RE.match(line)
        if hm:
            header_text = hm.group(1)
            cur_sub = detect_subsection_id(header_text)
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
        prose_blocker = prose_m.group(1) if (prose_m and not own_blockers) else None

        inherited: list[str] = []
        if indent == 0:
            parent_blocker = list(own_blockers)
        elif indent > 0 and not own_blockers and parent_blocker:
            inherited = list(parent_blocker)

        items.append(Item(
            lineno=lineno, indent=indent, content=content,
            raw_line=line, checked=checked, subsection_id=cur_sub,
            own_blockers=own_blockers, inherited_blockers=inherited,
            prose_blocker=prose_blocker,
        ))

    return items, header_titles


def parse_section_file(path: Path) -> SectionInfo | None:
    """Parse a section-*.md file into SectionInfo."""
    try:
        text = read_text(path)
    except OSError:
        return None

    fm, body_start = split_frontmatter(text)
    if not fm:
        return None

    number = str(fm.get("section", ""))
    title = str(fm.get("title", ""))
    status = str(fm.get("status", "unknown"))

    reviewed = fm.get("reviewed")
    if isinstance(reviewed, str):
        reviewed = reviewed.lower() == "true"
    elif not isinstance(reviewed, bool):
        reviewed = None

    goal = fm.get("goal")
    if goal is not None:
        goal = str(goal)

    success_criteria = fm.get("success_criteria") or []
    if not isinstance(success_criteria, list):
        success_criteria = [str(success_criteria)]
    success_criteria = [str(c) for c in success_criteria]

    depends_on = fm.get("depends_on") or []
    if not isinstance(depends_on, list):
        depends_on = [str(depends_on)]
    depends_on = [str(d) for d in depends_on]

    tpr_block = fm.get("third_party_review") or {}
    if isinstance(tpr_block, dict):
        tpr_status = str(tpr_block.get("status", "none"))
        tpr_updated = tpr_block.get("updated")
        if tpr_updated is not None:
            tpr_updated = str(tpr_updated)
    else:
        tpr_status = "none"
        tpr_updated = None

    lines = text.splitlines()
    body_text = "\n".join(lines[body_start:])
    flat_items, header_titles = parse_section_body(lines, body_start)

    # Build subsections from frontmatter sections[] array
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
            subsections.append(Subsection(
                id=sid, title=stitle, status=sstatus,
                items=sub_items_by_id.get(sid, []),
                title_from_header=header_titles.get(sid, ""),
            ))

    return SectionInfo(
        path=path, filename=path.name, number=number, title=title,
        status=status, reviewed=reviewed, goal=goal,
        success_criteria=success_criteria, depends_on=depends_on,
        tpr_status=tpr_status, tpr_updated=tpr_updated,
        subsections=subsections, flat_items=flat_items,
        frontmatter=fm, body_text=body_text,
    )


def parse_overview_file(path: Path) -> OverviewInfo | None:
    """Parse a 00-overview.md file."""
    try:
        text = read_text(path)
    except OSError:
        return None

    fm, body_start = split_frontmatter(text)
    body_text = "\n".join(text.splitlines()[body_start:])

    plan_name = str(fm.get("plan", path.parent.name))
    title = str(fm.get("title", ""))
    status = str(fm.get("status", "unknown"))

    overview = OverviewInfo(
        path=path, plan_name=plan_name, title=title, status=status,
        frontmatter=fm, body_text=body_text, mission_criteria=[],
    )
    overview.mission_criteria = overview.extract_mission_criteria()
    return overview


def parse_index_file(path: Path) -> IndexInfo | None:
    """Parse a plans/<plan>/index.md file."""
    try:
        text = read_text(path)
    except OSError:
        return None

    fm, _ = split_frontmatter(text)
    is_reroute = bool(fm.get("reroute"))
    is_parallel = bool(fm.get("parallel"))
    plan_dir = path.parent

    return IndexInfo(
        path=path, frontmatter=fm,
        is_reroute=is_reroute, is_parallel=is_parallel,
        name=str(fm.get("name") or plan_dir.name),
        full_name=str(fm.get("full_name") or fm.get("name") or plan_dir.name),
        status=str(fm.get("status", "active")),
        order=int(fm.get("order", 999)),
    )


# ─── Plan discovery ──────────────────────────────────────────────────────────


def discover_plan_dirs() -> list[Path]:
    """Find all plan directories under plans/."""
    if not PLANS_DIR.is_dir():
        return []
    dirs = []
    for d in sorted(PLANS_DIR.iterdir()):
        if d.is_dir() and not d.name.startswith("."):
            if d.name == "completed":
                for cd in sorted(d.iterdir()):
                    if cd.is_dir():
                        dirs.append(cd)
            else:
                dirs.append(d)
    return dirs


def load_plan(plan_dir: Path) -> PlanInfo:
    """Load and parse an entire plan directory."""
    name = plan_dir.name
    if plan_dir.parent.name == "completed":
        name = f"completed/{name}"

    # Parse overview
    overview_path = plan_dir / "00-overview.md"
    overview = parse_overview_file(overview_path) if overview_path.exists() else None

    # Parse index
    index_path = plan_dir / "index.md"
    index = parse_index_file(index_path) if index_path.exists() else None

    # Parse sections
    sections: list[SectionInfo] = []
    for p in sorted(plan_dir.glob("section-*.md")):
        s = parse_section_file(p)
        if s:
            sections.append(s)

    return PlanInfo(name=name, dir=plan_dir, overview=overview, index=index, sections=sections)


def load_all_plans() -> list[PlanInfo]:
    """Discover and load all plans."""
    return [load_plan(d) for d in discover_plan_dirs()]
