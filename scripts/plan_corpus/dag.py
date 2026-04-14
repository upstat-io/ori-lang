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
# §02.1 DAG Construction
# ---------------------------------------------------------------------------


# Dependency-verb lexicon consumed by the PROSE_VERB scanner. Includes
# explicit rewrite markers AND in-place-update verbs from the live corpus
# ("update in place of", "reprioritize", "reorder") — without the extended
# verbs, test case (c) is unimplementable per TPR-02-002-codex-r1.
DEPENDENCY_VERBS: frozenset[str] = frozenset({
    "depends on",
    "requires",
    "blocked by",
    "prerequisite",
    "unblocks",
    "supersedes",
    "rewrites",
    "rewrite of",
    "obsoletes",
    "update in place of",
    "update of",
    "reprioritize",
    "reorder",
})

INFORMATIONAL_VERBS: frozenset[str] = frozenset({
    "see also", "related", "inspired by", "cf.",
})


# Plan-directory path reference regex (used by the PROSE_VERB scanner).
# Captures `plans/<slug>/...` tokens.
_PLAN_PATH_RE = re.compile(r"plans/([A-Za-z0-9_\-][A-Za-z0-9_\-/]*)")


@dataclass
class Dag:
    """The constructed dependency graph consumed by §02.2 classifiers and §03.

    - `nodes`: set of NodeId across all seven schema classes.
    - `edges`: EXPLICIT_DEPENDS_ON only; body-inferred references never appear.
    - `references`: ALL source-kinds including PROSE_VERB, HTML_COMMENT_*, YAML_COMMENT.
    - `subsystem_to_nodes`: normalized-subsystem -> set of owning nodes.
    - `resolution_findings`: DEAD_REFERENCE / SCHEMA_VIOLATION findings from
      resolve_dep, enriched with precise source_line.
    - `findings`: All Findings produced during construction (currently only
      CYCLE from SCC detection; §02.2 classifiers append to this list).
    - `name_index`: reference to Corpus.name_index for classifier reuse.
    """

    nodes: set[NodeId] = field(default_factory=set)
    edges: list[Edge] = field(default_factory=list)
    references: list[Reference] = field(default_factory=list)
    subsystem_to_nodes: dict[str, set[NodeId]] = field(default_factory=dict)
    resolution_findings: list = field(default_factory=list)  # list[Finding]
    findings: list = field(default_factory=list)  # list[Finding]
    name_index: dict[str, Path] = field(default_factory=dict)

    def to_json(self) -> dict:
        """Stable JSON-serializable representation for diagnostic dumps."""
        return {
            "nodes": sorted(
                [{"kind": n.kind.value, "path": str(n.path)} for n in self.nodes],
                key=lambda d: (d["kind"], d["path"]),
            ),
            "edges": [
                {
                    "from": str(e.from_node.path),
                    "to": str(e.to_node.path),
                    "source_kind": e.source_kind.value,
                }
                for e in self.edges
            ],
            "references": [
                {
                    "from": str(r.from_node.path),
                    "target": r.target,
                    "source_kind": r.source_kind.value,
                    "source_line": r.source_line,
                }
                for r in self.references
            ],
            "subsystem_to_nodes": {
                subsystem: sorted(str(n.path) for n in nodes)
                for subsystem, nodes in self.subsystem_to_nodes.items()
            },
        }


def enrich_resolve_dep_finding(finding, dep_id: str, yaml_line: int,
                                declaring_file: Path):
    """Rebuild a resolve_dep-produced Finding with precise source_line + evidence.

    Finding K: resolve_dep returns source=plan_dir/"index.md" or plan_dir on
    failure. §03 SafeFix needs the exact YAML line of the offending depends_on
    list item. This helper rebuilds the Finding with source=<declaring_file>,
    source_line=yaml_line, evidence=(dep_id,) so SafeFix can apply
    remove_yaml_list_entry(source, source_line, dep_id) without further parsing.
    """
    from .types import Finding
    return Finding(
        category=finding.category,
        subtype=finding.subtype,
        severity=finding.severity,
        source=declaring_file,
        source_line=yaml_line,
        source_column=finding.source_column,
        target=finding.target,
        target_line=finding.target_line,
        description=finding.description,
        recommended_fix=finding.recommended_fix,
        evidence=(dep_id,),
        dependency_chain=finding.dependency_chain,
        source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
    )


def _node_kind_for_bucket(bucket_name: str) -> NodeKind:
    return {
        "indexes": NodeKind.PLAN_INDEX,
        "plan_sections": NodeKind.PLAN_SECTION,
        "roadmap_sections": NodeKind.ROADMAP_SECTION,
        "overviews": NodeKind.OVERVIEW,
        "bug_sections": NodeKind.BUG_TRACKER_SECTION,
        "fix_bug_files": NodeKind.FIX_BUG,
        "completed_indexes": NodeKind.COMPLETED_INDEX,
    }[bucket_name]


def _find_yaml_list_line(text: str, field_name: str, item_value: str) -> int | None:
    """Best-effort: find the 1-indexed line of a `field_name:` list item.

    Used to enrich resolve_dep findings with precise YAML line numbers (Finding K).
    Returns None if the field or item cannot be located.
    """
    lines = text.split("\n")
    in_field = False
    field_re = re.compile(rf"^{re.escape(field_name)}\s*:\s*(.*)$")
    item_re = re.compile(r"^\s*-\s*\"?([^\"]+?)\"?\s*$")
    for i, line in enumerate(lines):
        m = field_re.match(line)
        if m:
            in_field = True
            rhs = m.group(1).strip()
            # Inline scalar list (not our case for depends_on but handle
            # gracefully).
            if rhs and not rhs.startswith("["):
                continue
            continue
        if in_field:
            # Still inside the list if line starts with `- `/spaces+`-`
            if item_re.match(line):
                im = item_re.match(line)
                if im.group(1).strip() == item_value:
                    return i + 1
                continue
            # Non-list line ends the block if it's a new top-level key.
            if line and not line.startswith(" ") and not line.startswith("-"):
                break
    return None


def _read_body_text(path: Path) -> tuple[str, int]:
    """Return (body_text, body_offset) — thin wrapper over read_text_strict
    + split_frontmatter_strict. Returns empty-string body on parse failure
    (caller already has a parse-error Finding in Corpus.gaps).
    """
    from .parser import read_text_strict, split_frontmatter_strict
    from .types import CorpusParseError
    try:
        text = read_text_strict(path)
        _, body_offset = split_frontmatter_strict(text, path)
        lines = text.split("\n")
        body = "\n".join(lines[body_offset:])
        return body, body_offset
    except CorpusParseError:
        return "", 0


def _scan_body_for_references(
    from_node: NodeId, body: str, file_text: str, body_offset: int,
) -> list[Reference]:
    """Collect PROSE_VERB + HTML_COMMENT_CONVENTION + YAML_COMMENT references.

    Code-fence regions in the body are masked out (Finding D semantic pin).
    Informational verbs (see also, related, inspired by, cf.) do NOT produce
    references — they are documentation noise, not dependency signal.
    """
    refs: list[Reference] = []

    # HTML_COMMENT_CONVENTION — parse_html_comments handles code-fence exclusion.
    html_refs = parse_html_comments(body)
    for r in html_refs:
        # Patch the placeholder from_node.
        refs.append(Reference(
            from_node=from_node,
            target=r.target,
            source_kind=r.source_kind,
            source_line=r.source_line,
            source_column=r.source_column,
            raw_text=r.raw_text,
        ))

    # PROSE_VERB — plan-path tokens preceded by a dependency verb.
    code_regions = strip_code_blocks(body)

    def in_code(line_1idx: int) -> bool:
        return any(s <= line_1idx <= e for s, e, _k in code_regions)

    lines = body.split("\n")
    for line_idx, line in enumerate(lines):
        line_no = line_idx + 1
        if in_code(line_no):
            continue
        # Walk each plan-path match and inspect preceding text for a verb.
        for m in _PLAN_PATH_RE.finditer(line):
            match_start = m.start()
            preceding = line[:match_start].lower()
            # Match any verb as a phrase-suffix of the preceding text.
            matched_verb: str | None = None
            for verb in DEPENDENCY_VERBS:
                if preceding.rstrip().endswith(verb):
                    matched_verb = verb
                    break
            if matched_verb is None:
                continue
            plan_slug = m.group(1).rstrip("/.,;: \t")
            refs.append(Reference(
                from_node=from_node,
                target=f"plans/{plan_slug}/",
                source_kind=SourceKind.PROSE_VERB,
                source_line=line_no,
                source_column=match_start,
                raw_text=m.group(0),
            ))

    # YAML_COMMENT — scan the raw frontmatter text for `# comment` content
    # containing a DEPENDENCY_VERB or plans/<slug>/ reference.
    yaml_comments = extract_yaml_comments(file_text, body_offset)
    for line_no, col, comment_text in yaml_comments:
        # Look for plans/ path inside the comment.
        for m in _PLAN_PATH_RE.finditer(comment_text):
            plan_slug = m.group(1).rstrip("/.,;: \t")
            refs.append(Reference(
                from_node=from_node,
                target=f"plans/{plan_slug}/",
                source_kind=SourceKind.YAML_COMMENT,
                source_line=line_no,
                source_column=col,
                raw_text=comment_text,
            ))
        # Also capture BUG-\d{2}-\d{3} references (bug-tracker scope).
        for m in re.finditer(r"BUG-\d{2}-\d{3}", comment_text):
            refs.append(Reference(
                from_node=from_node,
                target=m.group(0),
                source_kind=SourceKind.YAML_COMMENT,
                source_line=line_no,
                source_column=col,
                raw_text=comment_text,
            ))

    return refs


def _extract_subsystem_tokens(body: str) -> set[str]:
    """Extract candidate subsystem tokens from a body for normalization.

    Scans for:
      - `compiler/ori_X/...` path prefixes
      - bare `ori_X` crate identifiers
      - logical aliases registered in SUBSYSTEM_ALIASES (case-sensitive)
    Code-fence regions are NOT excluded here — subsystem mapping is coarse
    and code fences often contain the richest subsystem signals (symbol
    names, struct refs, file paths).
    """
    tokens: set[str] = set()
    for m in re.finditer(r"compiler/[A-Za-z_][A-Za-z0-9_]*", body):
        tokens.add(m.group(0))
    for m in re.finditer(r"\bori_[a-z_]+\b", body):
        tokens.add(m.group(0))
    # Logical aliases: scan for each alias as a whole word.
    for alias in SUBSYSTEM_ALIASES:
        if alias in ("AIMS", "ARC", "FIP", "COW", "TRMC"):
            # Uppercase tokens — use word boundaries.
            if re.search(rf"\b{re.escape(alias)}\b", body):
                tokens.add(alias)
        elif "::" in alias or "Plan" in alias or "Registry" in alias or "Def" in alias or "Tree" in alias or "Expr" in alias:
            if alias in body:
                tokens.add(alias)
    return tokens


def _tarjan_scc(nodes: Iterable[NodeId], edges: list[Edge]) -> list[list[NodeId]]:
    """Return non-trivial strongly-connected components.

    A non-trivial SCC has size > 1 OR is a self-loop. Trivial single-node
    SCCs (the common case) are filtered out. Used by build_dag to emit
    CYCLE findings.
    """
    adj: dict[NodeId, list[NodeId]] = {n: [] for n in nodes}
    for e in edges:
        if e.from_node in adj:
            adj[e.from_node].append(e.to_node)

    index_counter = [0]
    stack: list[NodeId] = []
    on_stack: set[NodeId] = set()
    index: dict[NodeId, int] = {}
    lowlink: dict[NodeId, int] = {}
    result: list[list[NodeId]] = []

    def strongconnect(v: NodeId) -> None:
        index[v] = index_counter[0]
        lowlink[v] = index_counter[0]
        index_counter[0] += 1
        stack.append(v)
        on_stack.add(v)
        for w in adj.get(v, []):
            if w not in index:
                strongconnect(w)
                lowlink[v] = min(lowlink[v], lowlink[w])
            elif w in on_stack:
                lowlink[v] = min(lowlink[v], index[w])
        if lowlink[v] == index[v]:
            comp: list[NodeId] = []
            while True:
                w = stack.pop()
                on_stack.discard(w)
                comp.append(w)
                if w == v:
                    break
            # Non-trivial only: size > 1 OR self-loop.
            if len(comp) > 1 or any(w == v for w in adj.get(v, [])):
                result.append(comp)

    for n in list(adj.keys()):
        if n not in index:
            strongconnect(n)

    return result


def build_dag(corpus) -> Dag:
    """Construct the Dag from a discovered Corpus.

    Pipeline:
      1. Populate nodes from all seven corpus buckets.
      2. Parse EXPLICIT_DEPENDS_ON -> Edge (via resolve_dep, enriched).
      3. Scan bodies for PROSE_VERB / HTML_COMMENT / YAML_COMMENT references
         (Reference records only; no edges — §02.0 SSOT rule).
      4. Build subsystem_to_nodes from body-token normalization.
      5. Run Tarjan SCC on edges; emit CYCLE findings for non-trivial SCCs.
    """
    from .docgen import resolve_dep
    from .types import (
        Finding, FindingCategory, FindingSubtype, Severity,
    )

    dag = Dag(name_index=dict(corpus.name_index))

    # 1. Populate nodes.
    bucket_map = {
        "indexes": corpus.indexes,
        "plan_sections": corpus.plan_sections,
        "roadmap_sections": corpus.roadmap_sections,
        "overviews": corpus.overviews,
        "bug_sections": corpus.bug_sections,
        "fix_bug_files": corpus.fix_bug_files,
        "completed_indexes": corpus.completed_indexes,
    }
    for bname, bucket in bucket_map.items():
        nk = _node_kind_for_bucket(bname)
        for path in bucket:
            dag.nodes.add(NodeId(nk, path))

    # 2. Parse depends_on edges.
    # Plan-level depends_on comes from index.md files; section-level comes
    # from section data. fix-BUG files can also have depends_on.
    deps_sources: list[tuple[NodeId, Path, Path, dict]] = []
    for path, data in corpus.indexes.items():
        deps_sources.append((NodeId(NodeKind.PLAN_INDEX, path), path, path.parent, data))
    for path, data in corpus.plan_sections.items():
        deps_sources.append((NodeId(NodeKind.PLAN_SECTION, path), path, path.parent, data))
    for path, data in corpus.roadmap_sections.items():
        deps_sources.append((NodeId(NodeKind.ROADMAP_SECTION, path), path, path.parent, data))
    for path, data in corpus.fix_bug_files.items():
        deps_sources.append((NodeId(NodeKind.FIX_BUG, path), path, path.parent, data))

    for from_node, declaring_file, plan_dir, data in deps_sources:
        depends_on = data.get("depends_on") or []
        if not isinstance(depends_on, list):
            continue
        for dep_id in depends_on:
            if not isinstance(dep_id, str):
                continue
            # Find the YAML line of this specific list item for §03 SafeFix (Finding K).
            from .parser import read_text_strict
            try:
                raw_text = read_text_strict(declaring_file)
                yaml_line = _find_yaml_list_line(raw_text, "depends_on", dep_id)
            except Exception:
                yaml_line = None

            resolved = resolve_dep(dep_id, plan_dir, corpus)
            if isinstance(resolved, Path):
                # Find the NodeId for the resolved path (it exists in one of
                # the buckets already). If not found, the target is a file
                # outside the corpus (e.g., the plan dir exists but the
                # resolved section is missing) — propagate a Finding.
                target_nid = next(
                    (n for n in dag.nodes if n.path == resolved),
                    None,
                )
                if target_nid is None:
                    dag.resolution_findings.append(Finding(
                        category=FindingCategory.DEAD_REFERENCE,
                        subtype=FindingSubtype.SECTION_FILE_NOT_FOUND,
                        severity=Severity.HIGH,
                        source=declaring_file,
                        source_line=yaml_line,
                        description=f"depends_on target not in corpus: {dep_id}",
                        recommended_fix="Ensure the target file exists and is classified",
                        evidence=(dep_id,),
                        source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
                    ))
                    continue
                ref = Reference(
                    from_node=from_node,
                    target=dep_id,
                    source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
                    source_line=yaml_line if yaml_line else 0,
                    source_column=None,
                    raw_text=dep_id,
                )
                dag.references.append(ref)
                dag.edges.append(Edge(
                    from_node=from_node,
                    to_node=target_nid,
                    source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
                    reference=ref,
                ))
            else:
                # resolve_dep returned a Finding (DEAD_REFERENCE or
                # SCHEMA_VIOLATION). Enrich with precise line info.
                enriched = enrich_resolve_dep_finding(
                    resolved,
                    dep_id=dep_id,
                    yaml_line=yaml_line or 0,
                    declaring_file=declaring_file,
                )
                dag.resolution_findings.append(enriched)

    # 3. Scan bodies for body-inferred references.
    for from_node in dag.nodes:
        body, body_offset = _read_body_text(from_node.path)
        if not body:
            continue
        from .parser import read_text_strict
        try:
            file_text = read_text_strict(from_node.path)
        except Exception:
            file_text = ""
        body_refs = _scan_body_for_references(from_node, body, file_text, body_offset)
        dag.references.extend(body_refs)

    # 4. Build subsystem_to_nodes mapping.
    for from_node in dag.nodes:
        body, _ = _read_body_text(from_node.path)
        if not body:
            continue
        tokens = _extract_subsystem_tokens(body)
        for tok in tokens:
            normalized = normalize_subsystem(tok)
            if normalized is None:
                continue
            dag.subsystem_to_nodes.setdefault(normalized, set()).add(from_node)

    # 5. Tarjan SCC for cycle detection.
    sccs = _tarjan_scc(dag.nodes, dag.edges)
    for scc in sccs:
        scc_paths = tuple(n.path for n in scc)
        dag.findings.append(Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.CYCLE,
            severity=Severity.HIGH,
            source=scc[0].path,
            description=f"cycle of {len(scc)} nodes in depends_on graph",
            recommended_fix="Break the cycle by removing or restructuring one of the depends_on edges",
            dependency_chain=scc_paths,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        ))

    return dag


# ---------------------------------------------------------------------------
# §02.2 Conflict Classifiers
# ---------------------------------------------------------------------------


def _transitive_successors(dag: Dag, start: NodeId) -> set[NodeId]:
    """BFS from `start` following EXPLICIT_DEPENDS_ON edges. Returns all
    nodes reachable from `start`, including `start` itself."""
    adj: dict[NodeId, list[NodeId]] = {}
    for e in dag.edges:
        adj.setdefault(e.from_node, []).append(e.to_node)
    seen: set[NodeId] = set()
    stack: list[NodeId] = [start]
    while stack:
        cur = stack.pop()
        if cur in seen:
            continue
        seen.add(cur)
        for nxt in adj.get(cur, []):
            if nxt not in seen:
                stack.append(nxt)
    return seen


def _has_edge(dag: Dag, frm: NodeId, to: NodeId) -> bool:
    return any(e.from_node == frm and e.to_node == to for e in dag.edges)


_STATUS_CACHE: dict[tuple[int, NodeId], str | None] = {}


def _node_status(dag: Dag, corpus, node: NodeId) -> str | None:
    """Best-effort status derivation using §01.4 normalize_status.

    Cached by (id(dag), node) — classifiers run many pair-iterations and
    each _node_status otherwise does a fresh file read + frontmatter parse.
    Cache is keyed on `id(dag)` to avoid cross-test contamination when
    pytest constructs multiple fresh Dags.
    """
    from .normalizer import normalize_status

    key = (id(dag), node)
    if key in _STATUS_CACHE:
        return _STATUS_CACHE[key]

    buckets = {
        NodeKind.PLAN_INDEX: corpus.indexes,
        NodeKind.PLAN_SECTION: corpus.plan_sections,
        NodeKind.ROADMAP_SECTION: corpus.roadmap_sections,
        NodeKind.OVERVIEW: corpus.overviews,
        NodeKind.BUG_TRACKER_SECTION: corpus.bug_sections,
        NodeKind.FIX_BUG: corpus.fix_bug_files,
        NodeKind.COMPLETED_INDEX: corpus.completed_indexes,
    }
    data = buckets[node.kind].get(node.path)
    if data is None:
        _STATUS_CACHE[key] = None
        return None

    body, _ = _read_body_text(node.path)

    child_statuses = None
    if node.kind is NodeKind.PLAN_INDEX:
        child_statuses = []
        plan_dir = node.path.parent
        for p, d in corpus.plan_sections.items():
            if p.parent == plan_dir:
                s = d.get("status")
                if isinstance(s, str):
                    child_statuses.append(s)

    declared = data.get("status") if isinstance(data.get("status"), str) else None
    try:
        ns = normalize_status(data, body, node.path, child_statuses)
        # normalize_status returns "" for `derived` when the body has no
        # signal (empty sections, no checkboxes). In that case fall back to
        # the declared frontmatter status — same field normalize_status
        # would have reported if the body had agreed with it.
        result = ns.derived if ns.derived else declared
    except Exception:
        result = declared

    _STATUS_CACHE[key] = result
    return result


_CONTRADICTION_PAIRS: tuple[tuple[str, str], ...] = (
    ("remove", "keep"), ("keep", "remove"),
    ("refactor", "preserve"), ("preserve", "refactor"),
    ("rewrite", "retain"), ("retain", "rewrite"),
    ("delete", "maintain"), ("maintain", "delete"),
    ("replace", "extend"),
)


def _goals_contradict(goal_a: str | None, goal_b: str | None) -> tuple[bool, str]:
    """Coarse contradiction heuristic: look for shared subject noun-phrase
    and opposing verbs. Returns (contradicts, rationale)."""
    if not goal_a or not goal_b:
        return False, ""
    a, b = goal_a.lower(), goal_b.lower()
    # Simple shared-word + opposing-verb check. Good enough for the known
    # test cases; §03 may refine.
    shared_words = (set(re.findall(r"[a-z_]{4,}", a)) &
                    set(re.findall(r"[a-z_]{4,}", b)))
    # Drop common English words that are not subject nouns.
    stopwords = {"this", "that", "with", "from", "into", "when", "then",
                 "must", "need", "have", "take", "using", "which", "while",
                 "their", "there", "where", "would", "could", "should",
                 "shall", "being", "about", "above", "after", "again",
                 "against", "because", "before", "between", "during",
                 "these", "those", "plan", "section", "node"}
    shared_nouns = shared_words - stopwords
    if not shared_nouns:
        return False, ""
    for va, vb in _CONTRADICTION_PAIRS:
        if va in a and vb in b:
            return True, f"shared subject: {sorted(shared_nouns)[:3]}; verbs: {va!r} vs {vb!r}"
    return False, ""


def classify_conflict(dag: Dag, corpus) -> list:
    """CONFLICT classifier: shared subsystem + contradictory goals + no edge.

    Short-circuits when A transitively depends on B — that is prerequisite
    coordination, NOT conflict (Finding G dependency-edge short-circuit).
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []
    seen_pairs: set[tuple[NodeId, NodeId]] = set()

    for subsystem, nodes in dag.subsystem_to_nodes.items():
        node_list = sorted(nodes)
        for i, a in enumerate(node_list):
            for b in node_list[i + 1:]:
                key = (a, b) if (str(a.path), str(b.path)) < (str(b.path), str(a.path)) else (b, a)
                if key in seen_pairs:
                    continue
                seen_pairs.add(key)
                # Dependency-edge short-circuit.
                a_reachable = _transitive_successors(dag, a)
                b_reachable = _transitive_successors(dag, b)
                if b in a_reachable or a in b_reachable:
                    continue
                # Fetch goals.
                goal_a = _get_data(corpus, a).get("goal") if _get_data(corpus, a) else None
                goal_b = _get_data(corpus, b).get("goal") if _get_data(corpus, b) else None
                contradicts, rationale = _goals_contradict(goal_a, goal_b)
                if not contradicts:
                    continue
                findings.append(Finding(
                    category=FindingCategory.DAG_CONFLICT,
                    subtype=FindingSubtype.CONFLICT,
                    severity=Severity.HIGH,
                    source=a.path,
                    target=b.path,
                    description=f"Shared subsystem {subsystem!r} with contradictory goals",
                    recommended_fix="Decide precedence between the plans or add a depends_on edge to serialize them",
                    evidence=(rationale,),
                    source_kind=None,
                ))
    return findings


def _get_data(corpus, node: NodeId) -> dict | None:
    buckets = {
        NodeKind.PLAN_INDEX: corpus.indexes,
        NodeKind.PLAN_SECTION: corpus.plan_sections,
        NodeKind.ROADMAP_SECTION: corpus.roadmap_sections,
        NodeKind.OVERVIEW: corpus.overviews,
        NodeKind.BUG_TRACKER_SECTION: corpus.bug_sections,
        NodeKind.FIX_BUG: corpus.fix_bug_files,
        NodeKind.COMPLETED_INDEX: corpus.completed_indexes,
    }
    return buckets[node.kind].get(node.path)


def classify_superseded(dag: Dag, corpus) -> list:
    """SUPERSEDED: case (i) reroute w/ non-empty supersedes list AND no
    rewrite-section, OR case (ii) in-place-update claim with no completion
    marker and no target back-reference.

    Stays purely structural — no git timestamps (§02 purity contract).
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []

    # Case (i): reroute plan index with non-empty supersedes.
    for node in sorted(dag.nodes):
        if node.kind is not NodeKind.PLAN_INDEX:
            continue
        data = _get_data(corpus, node)
        if not data:
            continue
        if not data.get("reroute"):
            continue
        supersedes = data.get("supersedes") or []
        if not isinstance(supersedes, list) or not supersedes:
            continue
        # Does the reroute plan have a body-level rewrite mention for each?
        # Coarse heuristic: scan the plan-dir bodies for supersedes entries.
        plan_dir = node.path.parent
        all_bodies = ""
        for p, _d in corpus.plan_sections.items():
            if p.parent == plan_dir:
                b, _ = _read_body_text(p)
                all_bodies += "\n" + b
        for target in supersedes:
            if not isinstance(target, str):
                continue
            if target.lower() not in all_bodies.lower():
                findings.append(Finding(
                    category=FindingCategory.DAG_CONFLICT,
                    subtype=FindingSubtype.SUPERSEDED,
                    severity=Severity.MEDIUM,
                    source=node.path,
                    description=f"reroute plan claims to supersede {target!r} but no section discusses the rewrite",
                    recommended_fix="Add a section that rewrites the superseded target, or remove the supersedes entry",
                    evidence=(target,),
                    source_kind=None,
                ))

    # Case (ii): in-place-update claim without completion marker.
    # For each plan section body, look for DEPENDENCY_VERBS followed by a
    # plans/<slug>/ path. If the target exists in the DAG, check for
    # source-side <!-- update-complete:resolves=<target> --> OR target-side
    # <!-- updated-by:<source> --> OR depends_on edge from source->target.
    for ref in dag.references:
        if ref.source_kind is not SourceKind.PROSE_VERB:
            continue
        # Inspect the raw preceding context via the reference's source line
        # to pick out the verb. For simplicity, consider any DEPENDENCY_VERB
        # that triggered this reference as a potential in-place-update.
        body, _ = _read_body_text(ref.from_node.path)
        if not body:
            continue
        lines = body.split("\n")
        if ref.source_line < 1 or ref.source_line > len(lines):
            continue
        line = lines[ref.source_line - 1]
        trigger_verb = None
        for v in ("rewrites", "rewrite of", "update in place of", "update of",
                  "reprioritize", "reorder", "obsoletes", "supersedes"):
            if v in line.lower():
                trigger_verb = v
                break
        if trigger_verb is None:
            continue
        # Resolve target: ref.target is a "plans/<slug>/" string. Check
        # against any DAG node whose path starts with that prefix.
        slug = ref.target.rstrip("/").split("/")[-1]
        targets = [n for n in dag.nodes if slug in str(n.path)]
        if not targets:
            continue
        for target in targets:
            # (a) target exists — yes
            # (b) source lacks <!-- update-complete:resolves=<target> --> marker
            has_completion = any(
                r.source_kind is SourceKind.HTML_COMMENT_CONVENTION
                and r.from_node == ref.from_node
                and "update-complete" in r.raw_text
                and slug in r.target
                for r in dag.references
            )
            # (c) target lacks <!-- updated-by --> back-ref OR depends_on edge
            has_back_ref = any(
                r.source_kind is SourceKind.HTML_COMMENT_CONVENTION
                and r.from_node == target
                and "updated-by" in r.raw_text
                for r in dag.references
            ) or _has_edge(dag, target, ref.from_node) or _has_edge(dag, ref.from_node, target)

            if has_completion or has_back_ref:
                continue
            findings.append(Finding(
                category=FindingCategory.DAG_CONFLICT,
                subtype=FindingSubtype.SUPERSEDED,
                severity=Severity.MEDIUM,
                source=ref.from_node.path,
                source_line=ref.source_line,
                target=target.path,
                description=(
                    f"in-place-update claim ({trigger_verb!r}) with no structural completion marker — "
                    "potentially stale; needs §03 timestamp verification"
                ),
                recommended_fix="Complete the rewrite and add <!-- update-complete:resolves=<target> --> or remove the claim",
                evidence=(trigger_verb, ref.raw_text),
                source_kind=ref.source_kind,
            ))
    return findings


def classify_blocked(dag: Dag, corpus) -> list:
    """BLOCKED: edge A->B where A is active-equivalent and B is blocked-equivalent.

    "Active-equivalent": plan-level active, section-level in-progress.
    "Blocked-equivalent": plan-level queued/research, section-level not-started.
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity

    active = {"active", "in-progress"}
    blocked = {"queued", "research", "not-started"}

    findings: list = []
    for e in dag.edges:
        src_status = _node_status(dag, corpus, e.from_node)
        dst_status = _node_status(dag, corpus, e.to_node)
        if src_status in active and dst_status in blocked:
            findings.append(Finding(
                category=FindingCategory.DAG_CONFLICT,
                subtype=FindingSubtype.BLOCKED,
                severity=Severity.HIGH,
                source=e.from_node.path,
                source_line=e.reference.source_line,
                target=e.to_node.path,
                description=(
                    f"active node depends on blocked prerequisite "
                    f"(source status {src_status!r}, target status {dst_status!r})"
                ),
                recommended_fix="Resume work on the prerequisite, or pause the dependent plan",
                dependency_chain=(e.from_node.path, e.to_node.path),
                source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
            ))
    return findings


def classify_cross_edge_temporal_drift(dag: Dag, corpus) -> list:
    """STATUS_CONTRADICTION / CROSS_EDGE_TEMPORAL_DRIFT: dependent's status
    presupposes a state the prerequisite has not reached.

    Only the cross-edge subtype is emitted here. TPR_STALE_VS_EDIT is §03's
    responsibility (requires git commit timestamps).
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []
    for e in dag.edges:
        src = _node_status(dag, corpus, e.from_node)
        dst = _node_status(dag, corpus, e.to_node)
        drift = False
        if src == "complete" and dst in ("not-started", "in-progress"):
            drift = True
        elif src == "in-progress" and dst == "not-started":
            drift = True
        if not drift:
            continue
        findings.append(Finding(
            category=FindingCategory.STATUS_CONTRADICTION,
            subtype=FindingSubtype.CROSS_EDGE_TEMPORAL_DRIFT,
            severity=Severity.MEDIUM,
            source=e.from_node.path,
            target=e.to_node.path,
            description=(
                f"dependent status {src!r} presupposes prerequisite reaching "
                f"complete, but prerequisite is {dst!r}"
            ),
            recommended_fix="Either downgrade dependent status or advance prerequisite",
            dependency_chain=(e.from_node.path, e.to_node.path),
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        ))
    return findings


def classify_missing_dependency(dag: Dag, corpus) -> list:
    """MISSING_DEPENDENCY: body-inferred reference without a matching edge.

    Two signal sources:
      (a) Reference with source_kind in {PROSE_VERB, HTML_COMMENT_CONVENTION,
          YAML_COMMENT} that resolves to a DAG node without an
          EXPLICIT_DEPENDS_ON edge from the source.
      (b) Pairs of plans sharing a subsystem with BOTH active AND no edge
          between them — interference risk (TPR-02-003-gemini r1 semantic pin).
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []
    body_kinds = {SourceKind.PROSE_VERB, SourceKind.HTML_COMMENT_CONVENTION,
                  SourceKind.YAML_COMMENT}

    # Signal (a).
    for ref in dag.references:
        if ref.source_kind not in body_kinds:
            continue
        # Resolve target to a DAG node by slug match.
        slug = ref.target.rstrip("/").split("/")[-1]
        if not slug:
            continue
        target = next(
            (n for n in dag.nodes if n.path.parent.name == slug or n.path.name == slug + ".md"),
            None,
        )
        if target is None:
            continue
        if _has_edge(dag, ref.from_node, target):
            continue
        findings.append(Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.MEDIUM,
            source=ref.from_node.path,
            source_line=ref.source_line,
            source_column=ref.source_column,
            target=target.path,
            description=(
                f"body-inferred reference to {ref.target} via {ref.source_kind.value}; "
                "no matching depends_on edge declared"
            ),
            recommended_fix=f"Add `depends_on: [...]` entry referencing {target.path.name!r}",
            evidence=(ref.raw_text,),
            source_kind=ref.source_kind,
        ))

    # Signal (b): shared-subsystem active-pairs without edge.
    for subsystem, nodes in dag.subsystem_to_nodes.items():
        nodes_list = sorted(nodes)
        for i, a in enumerate(nodes_list):
            for b in nodes_list[i + 1:]:
                sa = _node_status(dag, corpus, a)
                sb = _node_status(dag, corpus, b)
                if sa not in ("active", "in-progress") or sb not in ("active", "in-progress"):
                    continue
                if _has_edge(dag, a, b) or _has_edge(dag, b, a):
                    continue
                # Avoid duplicate pair.
                findings.append(Finding(
                    category=FindingCategory.DAG_CONFLICT,
                    subtype=FindingSubtype.MISSING_DEPENDENCY,
                    severity=Severity.MEDIUM,
                    source=a.path,
                    target=b.path,
                    description=(
                        f"active plans share subsystem {subsystem!r} with no depends_on "
                        "edge — interference risk"
                    ),
                    recommended_fix="Serialize by adding a depends_on edge between the plans",
                    source_kind=None,
                ))
    return findings


def classify_dead_reference(dag: Dag, corpus) -> list:
    """DEAD_REFERENCE: pointer to nonexistent plan/directory/spec.

    Sources:
      - dag.resolution_findings (already DEAD_REFERENCE from resolve_dep)
      - body Reference records pointing at plans/<slug>/ that don't exist on
        disk. If the target is only in plans/completed/<slug>/, emit LOW
        severity (case (h) — completed plans are real but the annotation is
        stale).
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = list(dag.resolution_findings)
    body_kinds = {SourceKind.PROSE_VERB, SourceKind.HTML_COMMENT_CONVENTION,
                  SourceKind.YAML_COMMENT}

    # Compute plan slugs present on disk (both active and completed).
    active_slugs: set[str] = set()
    completed_slugs: set[str] = set()
    for p in corpus.indexes:
        active_slugs.add(p.parent.name)
    for p in corpus.completed_indexes:
        completed_slugs.add(p.parent.name)

    for ref in dag.references:
        if ref.source_kind not in body_kinds:
            continue
        target = ref.target
        # Expect plans/<slug>/ or plans/<slug>/<rest>
        if target.startswith("plans/"):
            slug = target[len("plans/"):].split("/")[0]
        else:
            slug = target.split("/")[0]
        if not slug:
            continue
        if slug in active_slugs:
            continue
        # Also skip references targeting BUG-\d+ etc. (not plans)
        if re.match(r"BUG-\d{2}-\d{3}", slug):
            continue
        # Severity ladder: EXPLICIT_DEPENDS_ON is already handled in
        # resolution_findings (HIGH). Body-kind severity:
        if slug in completed_slugs:
            severity = Severity.LOW
            desc = f"reference points at completed plan {slug!r}; annotation is stale"
            fix = "Update or remove the annotation — the plan is archived"
        else:
            if ref.source_kind is SourceKind.PROSE_VERB:
                severity = Severity.LOW
            else:
                severity = Severity.MEDIUM
            desc = f"reference to nonexistent plan directory {target!r}"
            fix = "Check the plan slug or remove the reference"
        findings.append(Finding(
            category=FindingCategory.DEAD_REFERENCE,
            subtype=FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
            severity=severity,
            source=ref.from_node.path,
            source_line=ref.source_line,
            source_column=ref.source_column,
            description=desc,
            recommended_fix=fix,
            evidence=(ref.raw_text,),
            source_kind=ref.source_kind,
        ))
    return findings


def classify_redundant_dependency(dag: Dag, corpus) -> list:
    """REDUNDANT_DEPENDENCY: A->C direct when A->B->...->C transitively exists.

    Conservative: only emit when the transitive chain has depth >= 3
    (depth-2 redundancies may be intentional readability declarations).
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []

    # Build adjacency.
    adj: dict[NodeId, set[NodeId]] = {}
    for e in dag.edges:
        adj.setdefault(e.from_node, set()).add(e.to_node)

    # For each edge A->C, BFS from A's successors (excluding C) to see if
    # any path of length >= 2 reaches C.
    for e in dag.edges:
        A, C = e.from_node, e.to_node
        # BFS depth-tracking.
        queue: list[tuple[NodeId, int]] = [(n, 1) for n in adj.get(A, set()) if n != C]
        reached = False
        depth_reached = 0
        seen: set[NodeId] = set()
        while queue:
            cur, depth = queue.pop(0)
            if cur in seen:
                continue
            seen.add(cur)
            if cur == C:
                reached = True
                depth_reached = max(depth_reached, depth)
                continue
            for nxt in adj.get(cur, set()):
                if nxt not in seen:
                    queue.append((nxt, depth + 1))
        if not reached:
            continue
        # Depth-2 is A->B->C; the direct A->C edge is then depth-1.
        # The plan's conservative rule: depth >= 3 for the redundant chain.
        if depth_reached < 2:
            continue
        # Find an explicit chain for evidence.
        chain = (A.path, C.path)
        findings.append(Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.REDUNDANT_DEPENDENCY,
            severity=Severity.LOW,
            source=A.path,
            target=C.path,
            description=(
                f"direct edge {A.path.name} -> {C.path.name} is redundant "
                f"(depth-{depth_reached + 1} transitive path exists)"
            ),
            recommended_fix="Remove the direct edge or keep it as an intentional readability declaration",
            dependency_chain=chain,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        ))
    return findings


def classify_orphaned_plan(dag: Dag, corpus) -> list:
    """ORPHANED_PLAN: plan-index node with no edges in/out AND no supersedes
    AND not body-referenced."""
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []

    # All body-referenced plan slugs.
    referenced_slugs: set[str] = set()
    for r in dag.references:
        if r.source_kind is SourceKind.EXPLICIT_DEPENDS_ON:
            # Edges are the "in" check; skip here.
            continue
        t = r.target
        if t.startswith("plans/"):
            referenced_slugs.add(t[len("plans/"):].split("/")[0])

    in_degree: dict[NodeId, int] = {}
    out_degree: dict[NodeId, int] = {}
    for e in dag.edges:
        out_degree[e.from_node] = out_degree.get(e.from_node, 0) + 1
        in_degree[e.to_node] = in_degree.get(e.to_node, 0) + 1

    for node in sorted(dag.nodes):
        if node.kind is not NodeKind.PLAN_INDEX:
            continue
        if in_degree.get(node, 0) > 0 or out_degree.get(node, 0) > 0:
            continue
        data = _get_data(corpus, node)
        if not data:
            continue
        supersedes = data.get("supersedes") or []
        if isinstance(supersedes, list) and supersedes:
            continue
        slug = node.path.parent.name
        if slug in referenced_slugs:
            continue
        # Also skip plan-index nodes whose plan sections have outgoing edges
        # — the plan is active through its sections.
        plan_dir = node.path.parent
        plan_section_nodes = [
            n for n in dag.nodes
            if n.kind is NodeKind.PLAN_SECTION and n.path.parent == plan_dir
        ]
        if any(
            out_degree.get(n, 0) > 0 or in_degree.get(n, 0) > 0
            for n in plan_section_nodes
        ):
            continue
        findings.append(Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.ORPHANED_PLAN,
            severity=Severity.LOW,
            source=node.path,
            description=f"plan {slug!r} has no depends_on edges in or out and no body references",
            recommended_fix="Verify this plan is still needed; file a bug if truly abandoned",
            source_kind=None,
        ))
    return findings


CLASSIFIERS = (
    classify_conflict,
    classify_superseded,
    classify_blocked,
    classify_cross_edge_temporal_drift,
    classify_missing_dependency,
    classify_dead_reference,
    classify_redundant_dependency,
    classify_orphaned_plan,
)


def run_classifiers(dag: Dag, corpus) -> list:
    """Run every classifier in registration order, flatten the findings."""
    out: list = []
    for clf in CLASSIFIERS:
        out.extend(clf(dag, corpus))
    return out


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
    # §02.1
    "Dag",
    "build_dag",
    "enrich_resolve_dep_finding",
    "DEPENDENCY_VERBS",
    "INFORMATIONAL_VERBS",
    # §02.2
    "classify_conflict",
    "classify_superseded",
    "classify_blocked",
    "classify_cross_edge_temporal_drift",
    "classify_missing_dependency",
    "classify_dead_reference",
    "classify_redundant_dependency",
    "classify_orphaned_plan",
    "CLASSIFIERS",
    "run_classifiers",
]
