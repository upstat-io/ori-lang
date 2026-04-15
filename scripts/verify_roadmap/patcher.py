"""Frontmatter text patcher for verify-roadmap.

Section 03.4 of verify-roadmap-redesign plan.

This is the ONLY write path for frontmatter modifications. PyYAML is
read-only — yaml.dump destroys comments, key order, quoting style, and
trailing whitespace, including YAML comments that are DAG signal for
§02's `YAML_COMMENT` source kind.

All operations work on the raw text slice between the `---` fences using
line-anchored regex. The slice is reassembled into the original file
preserving everything outside the fences byte-for-byte.

Concurrent-session safety:
  1. SHA256 hash captured at scan time (PreimageRecord)
  2. Re-hash at write time; refuse if mismatched
  3. Atomic write: temp file + os.replace

Boundary detection delegates to plan_corpus.types.FRONTMATTER_FENCE —
NO third frontmatter parser introduced (LEAK:algorithmic-duplication
guard per §03.4 blind spot #5).
"""

from __future__ import annotations

import hashlib
import os
import re
import tempfile
from pathlib import Path

from scripts.plan_corpus import FRONTMATTER_FENCE

from .safety import (
    FmOperation,
    FmOperationKind,
    PatchResult,
    PreimageRecord,
)


# ---------------------------------------------------------------------------
# Frontmatter slice extraction
# ---------------------------------------------------------------------------

def extract_frontmatter_slice(text: str) -> tuple[str, int, int]:
    """Return (frontmatter_text, start_offset, end_offset).

    The slice excludes the `---` fence lines themselves but includes
    everything between them. Returns ("", 0, 0) on malformed input
    (missing opening or closing fence) — caller handles.

    Uses plan_corpus.types.FRONTMATTER_FENCE for boundary detection.
    """
    lines = text.splitlines(keepends=True)
    if not lines or not FRONTMATTER_FENCE.match(lines[0].rstrip("\r\n")):
        return ("", 0, 0)

    # Find closing fence
    closing_idx = None
    for i, line in enumerate(lines[1:], start=1):
        if FRONTMATTER_FENCE.match(line.rstrip("\r\n")):
            closing_idx = i
            break
    if closing_idx is None:
        return ("", 0, 0)

    # Compute byte offsets — start is AFTER opening fence,
    # end is BEFORE closing fence
    start_offset = len(lines[0])
    end_offset = start_offset + sum(len(l) for l in lines[1:closing_idx])
    fm_text = text[start_offset:end_offset]
    return (fm_text, start_offset, end_offset)


def reassemble_file(
    original_text: str,
    patched_fm: str,
    start_offset: int,
    end_offset: int,
) -> str:
    """Splice patched_fm back into original_text at [start_offset:end_offset].

    Preserves everything before start_offset and after end_offset
    (including the `---` fences themselves).
    """
    return original_text[:start_offset] + patched_fm + original_text[end_offset:]


# ---------------------------------------------------------------------------
# Per-operation text transformations
#
# All operate on the raw frontmatter slice STRING. None of these touch the
# filesystem — they're pure string transformations.
# ---------------------------------------------------------------------------

def rename_key(fm_text: str, old_key: str, new_key: str) -> str:
    """Rename a top-level key, preserving value/spacing/inline comments.

    Only matches `^{old_key}:` (top-level keys, not nested or in lists).
    """
    # ^old_key followed by optional whitespace + colon, capture the rest.
    pattern = re.compile(
        rf"^({re.escape(old_key)})(\s*:.*)$",
        re.MULTILINE,
    )
    return pattern.sub(rf"{re.escape(new_key)}\2", fm_text)


def remove_key(fm_text: str, key: str) -> str:
    """Remove a top-level key + its value (handles multi-line values).

    A YAML key's "value" extends until the next line that is either:
      - another top-level key (no leading whitespace, ends with `:`)
      - end of frontmatter

    Lines indented under the key are part of its value and are removed.
    """
    lines = fm_text.splitlines(keepends=True)
    out: list[str] = []
    skip_until_top_level = False
    key_pattern = re.compile(rf"^{re.escape(key)}\s*:")

    for line in lines:
        if skip_until_top_level:
            # Continue removing if this line is indented (continuation of value)
            stripped = line.lstrip()
            if not stripped or line.startswith((" ", "\t")):
                # Indented or blank — part of multi-line value
                continue
            # Top-level line — stop skipping
            skip_until_top_level = False

        if key_pattern.match(line):
            skip_until_top_level = True
            continue

        out.append(line)

    return "".join(out)


def replace_value(fm_text: str, key: str, new_value: str) -> str:
    """Replace the inline value of a top-level key, preserving formatting.

    Preserves whitespace between key and value, and any inline comment.
    """
    # ^key: <value>[  # comment]
    pattern = re.compile(
        rf"^({re.escape(key)}\s*:\s*)([^\s#]+)(.*)$",
        re.MULTILINE,
    )
    return pattern.sub(lambda m: f"{m.group(1)}{new_value}{m.group(3)}", fm_text)


def insert_key(
    fm_text: str,
    key: str,
    value: str,
    after_key: str | None,
) -> str:
    """Insert a new `key: value` line.

    If after_key is None or not present, append at end of frontmatter.
    Otherwise insert directly after the matching `^after_key:` line.
    """
    insertion = f"{key}: {value}\n"

    if after_key is not None:
        pattern = re.compile(
            rf"^({re.escape(after_key)}\s*:.*)\n",
            re.MULTILINE,
        )
        if pattern.search(fm_text):
            return pattern.sub(rf"\1\n{insertion}", fm_text, count=1)

    # Append at end (ensure trailing newline)
    if not fm_text.endswith("\n"):
        fm_text += "\n"
    return fm_text + insertion


def remove_list_item(
    fm_text: str,
    list_key: str,
    item_value: str,
) -> str:
    """Remove a single `- item` entry from a YAML list under list_key.

    Handles both block style:
        depends_on:
          - "01"
          - "02"

    And inline style:
        depends_on: ["01", "02"]
    """
    # Try inline style first
    inline_pattern = re.compile(
        rf"^({re.escape(list_key)}\s*:\s*\[)([^\]]*?)(\])",
        re.MULTILINE,
    )
    inline_match = inline_pattern.search(fm_text)
    if inline_match:
        items_str = inline_match.group(2)
        items = [it.strip() for it in items_str.split(",") if it.strip()]
        # Match either bare or quoted form
        filtered = [
            it for it in items
            if it.strip().strip('"').strip("'") != item_value
        ]
        if len(filtered) != len(items):
            new_items_str = ", ".join(filtered)
            return inline_pattern.sub(
                lambda m: f"{m.group(1)}{new_items_str}{m.group(3)}",
                fm_text,
                count=1,
            )

    # Block style: locate `^list_key:` then iterate over indented `- item` lines
    lines = fm_text.splitlines(keepends=True)
    out: list[str] = []
    in_list = False
    key_pattern = re.compile(rf"^{re.escape(list_key)}\s*:\s*$")
    # Match `  - "value"` or `  - value`
    item_pattern = re.compile(
        rf'^\s*-\s*["\']?{re.escape(item_value)}["\']?\s*(#.*)?$'
    )

    for line in lines:
        if in_list:
            # Still in the list as long as line is blank or indented `-`
            if line.startswith((" ", "\t")) or line.strip().startswith("-"):
                if item_pattern.match(line.rstrip("\r\n")):
                    # Skip this line (remove the matching item)
                    continue
            else:
                in_list = False

        if key_pattern.match(line.rstrip("\r\n")):
            in_list = True

        out.append(line)

    return "".join(out)


# ---------------------------------------------------------------------------
# Operation dispatch
# ---------------------------------------------------------------------------

_OP_DISPATCH = {
    FmOperationKind.RENAME_KEY: lambda fm, kw: rename_key(
        fm, kw["old_key"], kw["new_key"]
    ),
    FmOperationKind.REMOVE_KEY: lambda fm, kw: remove_key(fm, kw["key"]),
    FmOperationKind.REPLACE_VALUE: lambda fm, kw: replace_value(
        fm, kw["key"], kw["new_value"]
    ),
    FmOperationKind.INSERT_KEY: lambda fm, kw: insert_key(
        fm, kw["key"], kw["value"], kw.get("after_key"),
    ),
    FmOperationKind.REMOVE_LIST_ITEM: lambda fm, kw: remove_list_item(
        fm, kw["list_key"], kw["item_value"],
    ),
}


def _apply_op(fm_text: str, op: FmOperation) -> str:
    """Apply a single FmOperation to the frontmatter slice."""
    handler = _OP_DISPATCH.get(op.kind)
    if handler is None:
        raise ValueError(f"Unknown FmOperationKind: {op.kind}")
    return handler(fm_text, op.kwargs_dict())


# ---------------------------------------------------------------------------
# apply_patch — the canonical write entrypoint
# ---------------------------------------------------------------------------

def _sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def apply_patch(
    path: Path,
    operations: list[FmOperation],
    preimage: PreimageRecord,
    corpus_root: Path,
) -> PatchResult:
    """Apply frontmatter operations to path, with concurrent-session guard.

    Workflow (blind spot #6):
      1. Refuse if path escapes corpus_root (4th refuse-on-conflict trigger)
      2. Re-read path; compute SHA256 of current contents
      3. Compare against preimage.content_hash
      4. If hashes differ -> refuse (PatchResult(applied=False, ...))
      5. If hashes match: extract fm slice, apply ops, reassemble,
         write to temp file, os.replace atomically

    The patcher NEVER touches the file outside the fm slice — body,
    fences, line endings, and trailing whitespace are preserved
    byte-for-byte.

    corpus_root is the reviewed plans-directory root (typically
    ``repo_root / "plans"``). Any ``path`` whose resolved form is not
    under ``corpus_root.resolve()`` is refused before any read/write —
    a bogus or drifted Finding.source must never drive writes to
    arbitrary files outside the reviewed corpus.
    """
    # Synthetic finding_id from preimage (apply_fixes injects this)
    finding_id = "VR-patch"

    # Path-escape refusal (refuse-on-conflict trigger #4, §03.4 blind spot).
    # Resolve both sides so symlinks / `..` are normalized; use string
    # prefix compare for compatibility with Python < 3.9 (is_relative_to).
    try:
        resolved_path = path.resolve(strict=False)
        resolved_root = corpus_root.resolve(strict=False)
    except (OSError, RuntimeError) as e:
        return PatchResult(
            applied=False,
            reason=f"path resolution failed: {type(e).__name__}: {e}",
            finding_id=finding_id,
            path=path,
        )
    try:
        resolved_path.relative_to(resolved_root)
    except ValueError:
        return PatchResult(
            applied=False,
            reason=(
                f"path escapes corpus root: {resolved_path} is not under "
                f"{resolved_root}"
            ),
            finding_id=finding_id,
            path=path,
        )

    if not path.exists():
        return PatchResult(
            applied=False,
            reason=f"path does not exist: {path}",
            finding_id=finding_id,
            path=path,
        )

    current_bytes = path.read_bytes()
    current_hash = _sha256_hex(current_bytes)

    if current_hash != preimage.content_hash:
        return PatchResult(
            applied=False,
            reason=(
                "file modified since scan by concurrent session "
                f"(preimage={preimage.content_hash[:8]}, "
                f"current={current_hash[:8]})"
            ),
            finding_id=finding_id,
            path=path,
            before_hash=preimage.content_hash,
            after_hash=current_hash,
        )

    # Decode (fail if not UTF-8 — strict parity with plan_corpus)
    try:
        original_text = current_bytes.decode("utf-8")
    except UnicodeDecodeError as e:
        return PatchResult(
            applied=False,
            reason=f"file is not valid UTF-8: {e}",
            finding_id=finding_id,
            path=path,
            before_hash=current_hash,
        )

    fm_text, start, end = extract_frontmatter_slice(original_text)
    if not fm_text:
        return PatchResult(
            applied=False,
            reason="malformed frontmatter (missing fences)",
            finding_id=finding_id,
            path=path,
            before_hash=current_hash,
        )

    # Apply operations sequentially
    patched_fm = fm_text
    for op in operations:
        patched_fm = _apply_op(patched_fm, op)

    new_text = reassemble_file(original_text, patched_fm, start, end)
    new_bytes = new_text.encode("utf-8")
    new_hash = _sha256_hex(new_bytes)

    # Atomic write: temp file in same directory + os.replace
    fd, tmp_path_str = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".tmp",
        dir=path.parent,
    )
    tmp_path = Path(tmp_path_str)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(new_bytes)
        os.replace(tmp_path, path)
    except Exception:
        # Cleanup temp file if it survived
        if tmp_path.exists():
            tmp_path.unlink()
        raise

    return PatchResult(
        applied=True,
        reason="applied successfully",
        finding_id=finding_id,
        path=path,
        before_hash=current_hash,
        after_hash=new_hash,
    )
