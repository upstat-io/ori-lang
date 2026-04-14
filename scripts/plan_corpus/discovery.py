"""Corpus discovery: directory classification, corpus walking, and load_and_validate.

Contains DirClass, Corpus, discover_corpus, ValidatedFile, LoadResult,
load_and_validate, and their helpers.
"""

from __future__ import annotations

import enum
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .types import (
    CorpusParseError,
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
    PLANS_DIR,
    ROADMAP_DIR,
)
from .parser import read_text_strict, split_frontmatter_strict
from .schema import FileClass, classify_file, validate


# ---------------------------------------------------------------------------
# Directory classification
# ---------------------------------------------------------------------------


class DirClass(enum.Enum):
    PLAN_CANDIDATE = "plan_candidate"
    CONTAINER = "container"
    UNKNOWN = "unknown"


_CONTAINER_DIRS = frozenset({"plans", "completed"})


def _classify_directory(dirpath: Path) -> DirClass:
    """Stage 1: classify a directory under plans/."""
    dirpath = dirpath.resolve()
    rel = dirpath.relative_to(PLANS_DIR) if dirpath.is_relative_to(PLANS_DIR) else dirpath
    dirname = rel.parts[-1] if rel.parts else dirpath.name

    if dirname in _CONTAINER_DIRS and len(rel.parts) <= 1:
        return DirClass.CONTAINER

    if (dirpath / "index.md").exists():
        return DirClass.PLAN_CANDIDATE

    if dirpath == ROADMAP_DIR:
        return DirClass.PLAN_CANDIDATE

    has_md = any(dirpath.glob("*.md"))
    if has_md:
        return DirClass.UNKNOWN

    return DirClass.CONTAINER


# ---------------------------------------------------------------------------
# Corpus
# ---------------------------------------------------------------------------


@dataclass
class Corpus:
    """The full discovered corpus with classified files and findings."""

    indexes: dict[Path, dict] = field(default_factory=dict)
    plan_sections: dict[Path, dict] = field(default_factory=dict)
    roadmap_sections: dict[Path, dict] = field(default_factory=dict)
    overviews: dict[Path, dict] = field(default_factory=dict)
    bug_sections: dict[Path, dict] = field(default_factory=dict)
    fix_bug_files: dict[Path, dict] = field(default_factory=dict)
    completed_indexes: dict[Path, dict] = field(default_factory=dict)
    name_index: dict[str, Path] = field(default_factory=dict)
    gaps: list[Finding] = field(default_factory=list)


def discover_corpus(
    root: Path | None = None,
    include: list[Path] | None = None,
) -> Corpus:
    """Walk plan directories and classify every .md file.

    Args:
        root: Plans root directory. Defaults to PLANS_DIR.
        include: If provided, only process these paths (for fast pilot runs).
    """
    if root is None:
        root = PLANS_DIR

    corpus = Corpus()

    if include is not None:
        for p in include:
            if p.is_file() and p.suffix == ".md":
                _classify_and_store(p, corpus)
        return corpus

    # Stage 1 + 2: walk directories
    for dirpath_str, dirnames, filenames in os.walk(root):
        dirpath = Path(dirpath_str)
        dir_class = _classify_directory(dirpath)

        if dir_class == DirClass.UNKNOWN:
            has_md = any(f.endswith(".md") for f in filenames)
            if has_md:
                corpus.gaps.append(Finding(
                    category=FindingCategory.GAP,
                    subtype=FindingSubtype.UNCLASSIFIED_DIRECTORY,
                    severity=Severity.LOW,
                    source=dirpath,
                    description=f"directory has .md files but no index.md: {dirpath}",
                    recommended_fix="Add index.md or move files to a plan directory",
                ))

        if dir_class == DirClass.PLAN_CANDIDATE:
            if not (dirpath / "index.md").exists() and dirpath != ROADMAP_DIR:
                corpus.gaps.append(Finding(
                    category=FindingCategory.GAP,
                    subtype=FindingSubtype.MISSING_INDEX_MD,
                    severity=Severity.HIGH,
                    source=dirpath,
                    description=f"plan candidate directory without index.md: {dirpath}",
                    recommended_fix="Add index.md to this plan directory",
                ))

        # Classify individual files
        for fname in filenames:
            if not fname.endswith(".md"):
                continue
            fpath = dirpath / fname
            _classify_and_store(fpath, corpus)

    # Build name_index from plan + completed indexes (shared loop — one SSOT for
    # DUPLICATE_PLAN_NAME detection across both index sources).
    import itertools
    for path, data in itertools.chain(
        corpus.indexes.items(), corpus.completed_indexes.items()
    ):
        name = data.get("name")
        if not name:
            continue
        if name in corpus.name_index:
            corpus.gaps.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.DUPLICATE_PLAN_NAME,
                severity=Severity.HIGH,
                source=path,
                target=corpus.name_index[name],
                description=f"duplicate plan name: {name!r}",
                recommended_fix="Each plan's name: field must be unique",
            ))
        else:
            corpus.name_index[name] = path.parent

    return corpus


def _classify_and_store(path: Path, corpus: Corpus) -> None:
    """Classify a file and store its raw frontmatter in the corpus."""
    fc = classify_file(path)
    if fc is None:
        return

    try:
        text = read_text_strict(path)
        data, _ = split_frontmatter_strict(text, path)
    except CorpusParseError as exc:
        corpus.gaps.append(Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=_parse_error_to_subtype(exc),
            severity=Severity.HIGH,
            source=path,
            source_line=getattr(exc, "line", None),
            description=str(exc),
            recommended_fix=f"Fix the frontmatter in {path}",
        ))
        return

    bucket = {
        FileClass.PLAN_INDEX: corpus.indexes,
        FileClass.PLAN_SECTION: corpus.plan_sections,
        FileClass.ROADMAP_SECTION: corpus.roadmap_sections,
        FileClass.OVERVIEW: corpus.overviews,
        FileClass.BUG_TRACKER_SECTION: corpus.bug_sections,
        FileClass.FIX_BUG: corpus.fix_bug_files,
        FileClass.COMPLETED_INDEX: corpus.completed_indexes,
    }.get(fc)

    if bucket is not None:
        bucket[path] = data


# ---------------------------------------------------------------------------
# load_and_validate boundary
# ---------------------------------------------------------------------------


@dataclass
class ValidatedFile:
    """A successfully parsed and classified file with optional schema violations."""
    path: Path
    file_class: FileClass
    data: dict[str, Any]
    body_offset: int
    body: str
    violations: list[Finding] = field(default_factory=list)


@dataclass
class LoadResult:
    """Tagged union: Ok(ValidatedFile) or Err(Finding)."""
    ok: ValidatedFile | None = None
    err: Finding | None = None

    @property
    def is_ok(self) -> bool:
        return self.ok is not None


def load_and_validate(path: Path) -> LoadResult:
    """The single CorpusParseError -> Finding boundary.

    Callers MUST check .is_ok -- no try/except needed at call sites.
    """
    try:
        text = read_text_strict(path)
        data, body_offset = split_frontmatter_strict(text, path)
    except CorpusParseError as e:
        subtype = _parse_error_to_subtype(e)
        return LoadResult(err=Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=subtype,
            severity=Severity.HIGH,
            source=path,
            source_line=e.line,
            description=str(e),
            recommended_fix="Fix the parse error and retry",
        ))

    fc = classify_file(path)
    if fc is None:
        return LoadResult(ok=ValidatedFile(
            path=path,
            file_class=FileClass.PLAN_INDEX,  # fallback
            data=data,
            body_offset=body_offset,
            body="\n".join(text.split("\n")[body_offset:]),
        ))

    violations = validate(fc, data, path)
    body_text = "\n".join(text.split("\n")[body_offset:])

    return LoadResult(ok=ValidatedFile(
        path=path,
        file_class=fc,
        data=data,
        body_offset=body_offset,
        body=body_text,
        violations=violations,
    ))


def _parse_error_to_subtype(e: CorpusParseError) -> FindingSubtype:
    """Map a CorpusParseError to the most specific FindingSubtype."""
    msg = str(e).lower()
    if "bom" in msg:
        return FindingSubtype.UTF8_BOM
    if "zero-width" in msg:
        return FindingSubtype.ZERO_WIDTH_BEFORE_FM
    if "invalid utf-8" in msg or "invalid utf8" in msg:
        return FindingSubtype.INVALID_UTF8_BYTES
    if "missing opening" in msg:
        return FindingSubtype.MISSING_OPENING_DASHES
    if "unclosed" in msg:
        return FindingSubtype.UNCLOSED_FRONTMATTER
    if "anchor" in msg or "alias" in msg:
        return FindingSubtype.YAML_ANCHOR
    if "merge key" in msg:
        return FindingSubtype.YAML_MERGE_KEY
    if "multi-document" in msg:
        return FindingSubtype.MULTI_DOCUMENT
    if "non_mapping" in msg or "must be a mapping" in msg:
        return FindingSubtype.NON_MAPPING_ROOT
    if "duplicate key" in msg:
        return FindingSubtype.DUPLICATE_KEY
    return FindingSubtype.YAML_SYNTAX_ERROR
