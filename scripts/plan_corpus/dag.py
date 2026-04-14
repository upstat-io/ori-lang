"""§02 DAG builder and classifiers — the cross-plan coherence auditor.

Homes the DAG-specific types (`NodeKind`, `NodeId`, `Reference`, `Edge`, `Dag`),
the `build_dag` entry point, the 8 conflict classifiers, and the `DagReport`
handoff schema consumed by §03.

SourceKind, Finding, FindingCategory, FindingSubtype all live in `types.py`;
this module imports them rather than re-defining (LEAK:scattered-knowledge
guard per `impl-hygiene.md` §SSOT).

Pure library — no git, no I/O beyond reading already-loaded `ValidatedFile`
bodies. §03's write-back phase is where git timestamps legitimately enter.
"""

from __future__ import annotations

import enum
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

from .schema import FileClass
from .types import SourceKind


# ---------------------------------------------------------------------------
# §02.0 Node Model
# ---------------------------------------------------------------------------


class NodeKind(enum.Enum):
    """One-to-one with `FileClass` — every schema class is a node class.

    Ordering reflects the natural "outer→inner" reading order: plan-level
    artifacts first (PLAN_INDEX, OVERVIEW), then per-section artifacts, then
    leaf bug artifacts, then the completed-plan archive.
    """

    PLAN_INDEX = "plan_index"
    PLAN_SECTION = "plan_section"
    ROADMAP_SECTION = "roadmap_section"
    OVERVIEW = "overview"
    BUG_TRACKER_SECTION = "bug_tracker_section"
    FIX_BUG = "fix_bug"
    COMPLETED_INDEX = "completed_index"

    @classmethod
    def from_file_class(cls, fc: FileClass) -> "NodeKind":
        """Map FileClass -> NodeKind via name identity.

        FileClass and NodeKind share exact names so this is a pure lookup;
        any drift is caught by the exhaustiveness test.
        """
        return cls[fc.name]


@dataclass(frozen=True)
class NodeId:
    """(kind, path) identity for DAG nodes. Frozen + hashable + orderable.

    Ordering compares `(kind.value, str(path))` — `NodeKind` is a plain enum
    without IntEnum semantics, so we compare via its string value. This keeps
    NodeId deterministically sortable for reproducible DAG traversal output
    without coupling ordering to integer enum values.
    """

    kind: NodeKind
    path: Path

    def __lt__(self, other: "NodeId") -> bool:
        if not isinstance(other, NodeId):
            return NotImplemented
        return (self.kind.value, str(self.path)) < (other.kind.value, str(other.path))

    def __le__(self, other: "NodeId") -> bool:
        if not isinstance(other, NodeId):
            return NotImplemented
        return (self.kind.value, str(self.path)) <= (other.kind.value, str(other.path))

    def __gt__(self, other: "NodeId") -> bool:
        if not isinstance(other, NodeId):
            return NotImplemented
        return (self.kind.value, str(self.path)) > (other.kind.value, str(other.path))

    def __ge__(self, other: "NodeId") -> bool:
        if not isinstance(other, NodeId):
            return NotImplemented
        return (self.kind.value, str(self.path)) >= (other.kind.value, str(other.path))

    @classmethod
    def from_validated_file(cls, vf) -> "NodeId":
        """Build a NodeId from a ValidatedFile without re-classifying.

        Kept as a staticmethod rather than an import-time dep to avoid
        discovery.py -> dag.py coupling (dag.py -> discovery.py would be the
        wrong direction per the package DAG).
        """
        return cls(NodeKind.from_file_class(vf.file_class), vf.path)


@dataclass(frozen=True)
class Reference:
    """A raw reference extracted from a file's body or frontmatter.

    References carry enough info to disambiguate `Finding.id` collisions
    (Finding J): `source_column` + the raw_text hash serve as tie-breakers.
    `target` is the raw string as it appears in the file; resolution happens
    via `plan_corpus.resolve_dep` during §02.1 DAG construction.
    """

    from_node: NodeId
    target: str
    source_kind: SourceKind
    source_line: int
    source_column: int | None
    raw_text: str


@dataclass(frozen=True)
class Edge:
    """A DAG edge. Only EXPLICIT_DEPENDS_ON references are promoted to edges.

    Body-inferred references (PROSE_VERB, HTML_COMMENT_CONVENTION,
    YAML_COMMENT) are collected as Reference records but NEVER become edges —
    they feed MISSING_DEPENDENCY per the §02.0 SSOT rule. The constructor
    enforces this to keep shadow edges impossible by construction.
    """

    from_node: NodeId
    to_node: NodeId
    source_kind: SourceKind
    reference: Reference

    def __post_init__(self) -> None:
        if self.source_kind is not SourceKind.EXPLICIT_DEPENDS_ON:
            raise ValueError(
                f"Edge source_kind must be EXPLICIT_DEPENDS_ON per §02.0 SSOT "
                f"(body-inferred references feed MISSING_DEPENDENCY, never "
                f"shadow edges). Got {self.source_kind.name}."
            )


# ---------------------------------------------------------------------------
# §02.0 Subsystem Normalization
# ---------------------------------------------------------------------------


# Alias table combining (A) workspace crates and (B) logical aliases that map
# user-facing identifiers (types, constructs, subsystem nicknames) back to
# their owning crate. Canonical values are crate names (`ori_arc`, etc.).
#
# Source A — auto-populated from Cargo.toml workspace members.
# Source B — hand-maintained logical aliases; extend when a new subsystem
# nickname becomes load-bearing in plan prose.
SUBSYSTEM_ALIASES: dict[str, str] = {
    # --- Source A: workspace crates (self-aliases) ---
    "ori_ir": "ori_ir",
    "ori_registry": "ori_registry",
    "ori_diagnostic": "ori_diagnostic",
    "ori_lexer": "ori_lexer",
    "ori_lexer_core": "ori_lexer",
    "ori_types": "ori_types",
    "ori_parse": "ori_parse",
    "ori_patterns": "ori_patterns",
    "ori_eval": "ori_eval",
    "ori_fmt": "ori_fmt",
    "ori_arc": "ori_arc",
    "ori_repr": "ori_repr",
    "ori_canon": "ori_canon",
    "ori_compiler": "ori_compiler",
    "ori_stack": "ori_stack",
    "oric": "oric",
    "ori_llvm": "ori_llvm",
    "ori_rt": "ori_rt",
    "ori_test_harness": "ori_test_harness",
    # --- Source B: logical aliases ---
    "AIMS": "ori_arc",
    "ArcClassifier": "ori_arc",
    "ARC": "ori_arc",
    "FIP": "ori_arc",
    "COW": "ori_arc",
    "TRMC": "ori_arc",
    "Tag::Var": "ori_types",
    "TypeRegistry": "ori_types",
    "TraitRegistry": "ori_types",
    "ReprPlan": "ori_repr",
    "MethodRegistry": "ori_registry",
    "MethodDef": "ori_registry",
    "LLVM": "ori_llvm",
    "DecisionTree": "ori_patterns",
    "CanExpr": "ori_canon",
}


_CRATE_PATH_RE = re.compile(r"compiler/([A-Za-z_][A-Za-z0-9_]*)")


def normalize_subsystem(raw: str) -> str | None:
    """Normalize a subsystem identifier to its canonical crate name.

    Accepts workspace crate names, `compiler/<crate>/...` paths, or logical
    aliases. Returns None for unrecognized tokens (callers must filter None
    before contributing to `subsystem_to_nodes`).

    §02.0 "Define SUBSYSTEM_ALIASES" item requires every workspace crate's
    display name to appear in the normalized output — verified by
    TestSubsystemAliases.test_every_workspace_crate_is_normalized.
    """
    if not raw:
        return None

    # Direct table hit.
    if raw in SUBSYSTEM_ALIASES:
        return SUBSYSTEM_ALIASES[raw]

    # compiler/<crate>/... path extraction.
    m = _CRATE_PATH_RE.search(raw)
    if m:
        crate = m.group(1)
        if crate in SUBSYSTEM_ALIASES:
            return SUBSYSTEM_ALIASES[crate]

    return None


# ---------------------------------------------------------------------------
# §02.0 Body-text helpers: code-fence exclusion, YAML-comment scan,
# HTML-comment grammar.
# ---------------------------------------------------------------------------


_FENCE_RE = re.compile(r"^\s{0,3}```")


def strip_code_blocks(body: str) -> list[tuple[int, int, str]]:
    """Return 1-indexed (start_line, end_line, kind) regions covering every
    fenced or indented code block in `body`.

    Fenced blocks: matched pairs of ``` markers. An unclosed fence extends to
    EOF (CommonMark permits this).

    Indented blocks: runs of lines starting with 4+ spaces where the run is
    preceded by a blank line (CommonMark-ish heuristic; tight enough to
    exclude multi-column tables and loose enough to catch real code blocks).

    Used to mask out code-fence regions before heuristic matching in
    §02.1/§02.2 body scanners — Finding D semantic pin: template paths
    inside fences MUST NOT produce DEAD_REFERENCE findings.
    """
    lines = body.split("\n")
    regions: list[tuple[int, int, str]] = []

    # Pass 1: fenced blocks.
    i = 0
    while i < len(lines):
        if _FENCE_RE.match(lines[i]):
            start = i + 1  # 1-indexed start line (the opening ``` line)
            j = i + 1
            while j < len(lines) and not _FENCE_RE.match(lines[j]):
                j += 1
            end = j + 1 if j < len(lines) else len(lines)
            regions.append((start, end, "fenced"))
            i = j + 1
        else:
            i += 1

    # Pass 2: indented blocks. Only detect outside fenced regions.
    def in_fenced(line_1idx: int) -> bool:
        return any(s <= line_1idx <= e and k == "fenced" for s, e, k in regions)

    i = 0
    while i < len(lines):
        if in_fenced(i + 1):
            i += 1
            continue
        # Indented block starts at line that is 4+ spaces AND preceded by
        # blank line (or BOF).
        if lines[i].startswith("    ") and lines[i].strip():
            is_bof = i == 0
            preceded_blank = (not is_bof) and not lines[i - 1].strip()
            if is_bof or preceded_blank:
                start = i + 1
                j = i
                while j < len(lines) and (
                    lines[j].startswith("    ") or not lines[j].strip()
                ):
                    j += 1
                end = j  # last indented line (1-indexed = j since start was i+1)
                if end >= start:
                    regions.append((start, end, "indented"))
                i = j
                continue
        i += 1

    regions.sort()
    return regions


_YAML_COMMENT_RE = re.compile(r"#\s*(.+?)\s*$")


def extract_yaml_comments(text: str, body_offset: int) -> list[tuple[int, int, str]]:
    """Return (line_number_1idx, column_0idx, comment_text) tuples for every
    `# ...` comment found on frontmatter lines.

    `body_offset` bounds the scan to the `---` ... `---` region only
    (lines strictly before `body_offset`). PyYAML strips comments at parse
    time — this is the raw-text post-parse pass that recovers them, used by
    the YAML_COMMENT reference classifier.
    """
    lines = text.split("\n")
    out: list[tuple[int, int, str]] = []
    # Frontmatter region: lines before body_offset (exclusive).
    upper = min(body_offset, len(lines))
    for line_idx in range(upper):
        line = lines[line_idx]
        # Find the first '#' that is NOT inside a quoted string. Cheap
        # heuristic: scan left-to-right tracking single/double quote state.
        in_single = False
        in_double = False
        hash_pos = -1
        for i, ch in enumerate(line):
            if ch == "'" and not in_double:
                in_single = not in_single
            elif ch == '"' and not in_single:
                in_double = not in_double
            elif ch == "#" and not in_single and not in_double:
                hash_pos = i
                break
        if hash_pos < 0:
            continue
        # Skip leading-# whole-line comments with no content (rare in YAML
        # but drop for consistency). Keep if there's useful text after.
        comment_body = line[hash_pos + 1:].strip()
        if not comment_body:
            continue
        # Skip the frontmatter fence itself which starts with `---` and has
        # no `#`; defensive only.
        out.append((line_idx + 1, hash_pos, comment_body))
    return out


# HTML comment grammar: the reserved verb set consumed by §02.2 classifiers.
# TPR-02-003-codex r1: hyphens ARE allowed in target tokens (plan slugs are
# hyphenated, e.g. `jit-exception-handling/04B`); only whitespace and the
# `,` separator are excluded per-token.
# TPR-02-001-codex r3: extended to include the three round-2 markers
# (`rewrites`, `update-complete`, `updated-by`) needed by SUPERSEDED case (ii).
_HTML_COMMENT_RE = re.compile(
    r"<!--\s*"
    r"(blocked-by|unblocks|supersedes|resolves|rewrites|update-complete|updated-by)"
    r"\s*:\s*"
    r"([^ \t\r\n,]+(?:,[^ \t\r\n,]+)*)"
    r"\s*-->"
)


def parse_html_comments(body: str) -> list[Reference]:
    """Parse structured HTML comments into Reference records.

    Recognized verbs and their semantics (§02.0):
      - `blocked-by:ID`      — forward reference (self blocked by ID)
      - `unblocks:ID`        — reverse reference (self unblocks ID)
      - `supersedes:ID`      — supersession reference
      - `resolves:ID`        — bug-fix reference
      - `rewrites:ID`        — rewrite reference (feeds SUPERSEDED case (ii))
      - `update-complete:resolves=TARGET` — source-side completion marker
      - `updated-by:SOURCE`  — target-side back-reference

    Comments inside fenced or indented code blocks are excluded (Finding D
    semantic pin). Comma-separated target lists produce one Reference each.

    Returns Reference objects with:
      from_node = placeholder NodeId(PLAN_SECTION, <unknown>) — the real
        from_node is patched by the DAG builder when it knows the owning
        ValidatedFile (§02.1 wires it).
      source_kind = HTML_COMMENT_CONVENTION
      source_line = 1-indexed line number of the match start
      source_column = 0-indexed column number of the `<!--` marker
      target = the single resolved target string (comma-separated lists
        become multiple Reference records, one per target)
      raw_text = full `<!-- ... -->` match text
    """
    code_regions = strip_code_blocks(body)
    lines = body.split("\n")

    def in_code(line_1idx: int) -> bool:
        return any(s <= line_1idx <= e for s, e, _k in code_regions)

    placeholder = NodeId(NodeKind.PLAN_SECTION, Path("<unresolved>"))
    refs: list[Reference] = []

    for line_idx, line in enumerate(lines):
        line_no = line_idx + 1
        if in_code(line_no):
            continue
        for m in _HTML_COMMENT_RE.finditer(line):
            targets_raw = m.group(2)
            column = m.start()
            raw = m.group(0)
            for tok in targets_raw.split(","):
                tok = tok.strip()
                if not tok:
                    continue
                refs.append(Reference(
                    from_node=placeholder,
                    target=tok,
                    source_kind=SourceKind.HTML_COMMENT_CONVENTION,
                    source_line=line_no,
                    source_column=column,
                    raw_text=raw,
                ))
    return refs


# ---------------------------------------------------------------------------
# Placeholder surface for §02.1+ (Dag dataclass, build_dag, classifiers).
# Kept empty here so §02.0 lands as a focused commit. §02.1 fills it.
# ---------------------------------------------------------------------------


__all__ = [
    # Node model
    "NodeKind",
    "NodeId",
    "Reference",
    "Edge",
    # Subsystem
    "SUBSYSTEM_ALIASES",
    "normalize_subsystem",
    # Body helpers
    "strip_code_blocks",
    "extract_yaml_comments",
    "parse_html_comments",
]
