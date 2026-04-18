"""Strict YAML frontmatter parser.

Provides read_text_strict() and split_frontmatter_strict() — the only
sanctioned way to read plan file frontmatter in the corpus tooling.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

from .types import (
    CorpusParseError,
    YAML_MERGE_KEY_RE,
    ZERO_WIDTH_CHARS,
)


class _NoDuplicateKeyLoader(yaml.SafeLoader):
    """SafeLoader subclass that raises on duplicate mapping keys AND on
    any YAML anchor (`&name`) or alias (`*name`).

    Anchor/alias rejection is done at the composer layer (not via a raw-line
    regex) because a regex cannot distinguish a real YAML alias at a value
    position from `**bold**` markdown or `**/build` glob text inside a
    quoted string value. PyYAML's composer sees post-parse events, so
    anchors and aliases are unambiguous here.
    """

    def compose_node(self, parent, index):
        # Alias events (`*name`) are rejected unconditionally.
        if self.check_event(yaml.AliasEvent):
            event = self.peek_event()
            mark = event.start_mark
            raise yaml.YAMLError(
                f"YAML alias detected at line {mark.line + 1} column {mark.column + 1} (schema-bypass vector)"
            )
        event = self.peek_event()
        # Anchored nodes (`&name ...`) are rejected too — the anchor
        # attribute is set on ScalarEvent / SequenceStartEvent /
        # MappingStartEvent when an anchor is attached.
        if event.anchor is not None:
            mark = event.start_mark
            raise yaml.YAMLError(
                f"YAML anchor `&{event.anchor}` detected at line {mark.line + 1} column {mark.column + 1} (schema-bypass vector)"
            )
        return super().compose_node(parent, index)


def _no_duplicate_key_constructor(loader, node):
    keys_seen: set[str] = set()
    for key_node, _ in node.value:
        key = loader.construct_object(key_node)
        if key in keys_seen:
            mark = key_node.start_mark
            raise yaml.YAMLError(
                f"duplicate key: {key!r}",
                mark,
            )
        keys_seen.add(key)
    return loader.construct_mapping(node)


_NoDuplicateKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
    _no_duplicate_key_constructor,
)


def _validate_yaml_features(fm_lines: list[str], path: Path) -> None:
    """Pre-scan frontmatter lines for merge keys and multi-document markers.

    Anchor/alias rejection lives in `_NoDuplicateKeyLoader.compose_node`
    (PyYAML-layer) rather than here, because a raw-line regex cannot
    distinguish real anchors/aliases from markdown / glob text inside
    quoted string values.
    """
    for line_no, line in enumerate(fm_lines, start=2):
        if YAML_MERGE_KEY_RE.search(line):
            raise CorpusParseError(
                "YAML merge key (<<:) detected",
                path,
                line=line_no,
            )

    for line_no, line in enumerate(fm_lines, start=2):
        stripped = line.rstrip("\r")
        if stripped == "---" or stripped == "...":
            raise CorpusParseError(
                "multi-document YAML marker inside frontmatter",
                path,
                line=line_no,
            )


def read_text_strict(path: Path) -> str:
    """Read a file with strict UTF-8 decoding. Never replaces invalid bytes."""
    try:
        raw = path.read_bytes()
    except OSError as e:
        raise CorpusParseError(f"cannot read file: {e}", path) from e

    if raw[:3] == b"\xef\xbb\xbf":
        raise CorpusParseError(
            "UTF-8 BOM detected at byte 0; remove the BOM",
            path,
            line=1,
        )

    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as e:
        raise CorpusParseError(
            f"invalid UTF-8 bytes at position {e.start}",
            path,
            yaml_error=e,
        ) from e

    first_line_end = text.find("\n")
    prefix = text[:first_line_end] if first_line_end >= 0 else text
    for i, ch in enumerate(prefix):
        if ch in ZERO_WIDTH_CHARS:
            raise CorpusParseError(
                f"zero-width character U+{ord(ch):04X} before opening ---",
                path,
                line=1,
                column=i + 1,
            )

    return text


def split_frontmatter_strict(
    text: str, path: Path
) -> tuple[dict[str, Any], int]:
    """Parse YAML frontmatter strictly. Returns (parsed_dict, body_line_offset).

    Raises CorpusParseError on any parse problem -- never returns {} on error.
    """
    lines = text.split("\n")

    if not lines or lines[0].rstrip("\r") != "---":
        raise CorpusParseError(
            "missing opening --- on line 1",
            path,
            line=1,
        )

    end_idx = -1
    for i in range(1, len(lines)):
        stripped = lines[i].rstrip("\r")
        if stripped == "---":
            end_idx = i
            break
        if stripped == "...":
            # '...' is a YAML document-end marker, not a frontmatter closer.
            # Its presence inside the frontmatter region means this document
            # is multi-document YAML, which we reject as a schema-bypass vector.
            raise CorpusParseError(
                "multi-document YAML marker inside frontmatter",
                path,
                line=i + 1,
            )

    if end_idx < 0:
        raise CorpusParseError("unclosed frontmatter (no closing ---)", path, line=1)

    fm_lines = lines[1:end_idx]
    fm_text = "\n".join(fm_lines)

    # Pre-scan for anchors, aliases, merge keys, multi-doc markers
    _validate_yaml_features(fm_lines, path)

    # Check for CRLF inside frontmatter region
    has_crlf = any("\r" in line for line in fm_lines)
    if has_crlf:
        fm_text = fm_text.replace("\r\n", "\n").replace("\r", "\n")

    try:
        data = yaml.load(fm_text, Loader=_NoDuplicateKeyLoader)
    except yaml.YAMLError as e:
        line_no = None
        col = None
        if hasattr(e, "problem_mark") and e.problem_mark is not None:
            line_no = e.problem_mark.line + 2  # +1 for 0-index, +1 for opening ---
            col = e.problem_mark.column + 1
        raise CorpusParseError(
            f"YAML parse error: {e}",
            path,
            line=line_no,
            column=col,
            yaml_error=e,
        ) from e

    if data is None:
        raise CorpusParseError("frontmatter is empty (null)", path, line=1)

    if not isinstance(data, dict):
        raise CorpusParseError(
            f"frontmatter root must be a mapping, got {type(data).__name__}",
            path,
            line=1,
        )

    body_offset = end_idx + 1
    return data, body_offset
