#!/usr/bin/env python3
"""Tests for scripts/verify_roadmap/patcher.py — Frontmatter Text Patcher.

TDD per CLAUDE.md: tests define expected behavior.
Section 03.4 of verify-roadmap-redesign plan.

Coverage:
  - extract_frontmatter_slice: boundaries, malformed handling
  - per-op text transformations preserve comments + key order
  - apply_patch: concurrent-session guard via SHA256 hash compare
  - apply_patch: atomic write via os.replace
  - reassemble_file: splice fm back into source
  - round-trip: extract -> modify -> reassemble -> plan_corpus.parser
"""

import hashlib
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest
from scripts.plan_corpus import split_frontmatter_strict
from scripts.verify_roadmap import (
    FmOperation,
    FmOperationKind,
    PatchResult,
    PreimageRecord,
)
from scripts.verify_roadmap.patcher import (
    extract_frontmatter_slice,
    rename_key,
    remove_key,
    replace_value,
    insert_key,
    remove_list_item,
    reassemble_file,
    apply_patch,
)


# ---------------------------------------------------------------------------
# extract_frontmatter_slice
# ---------------------------------------------------------------------------

class TestExtractFrontmatterSlice:

    def test_basic(self):
        text = "---\nname: foo\nstatus: active\n---\n# Body\n"
        fm, start, end = extract_frontmatter_slice(text)
        assert "name: foo" in fm
        assert "status: active" in fm
        # The slice should NOT include the fences
        assert "---" not in fm

    def test_offsets_round_trip(self):
        text = "---\nname: foo\n---\n# Body\n"
        fm, start, end = extract_frontmatter_slice(text)
        # Substring [start:end] returns the fm text
        assert text[start:end] == fm

    def test_no_opening_fence_returns_empty(self):
        """Malformed: no fences -> empty/zero, caller handles."""
        text = "# Just a body\nno frontmatter here\n"
        fm, start, end = extract_frontmatter_slice(text)
        assert fm == ""
        assert start == 0
        assert end == 0

    def test_no_closing_fence_returns_empty(self):
        text = "---\nname: foo\n# never closed\n"
        fm, start, end = extract_frontmatter_slice(text)
        assert fm == ""

    def test_preserves_comments(self):
        text = "---\nname: foo  # important\n# top-of-fm comment\nstatus: active\n---\n"
        fm, _, _ = extract_frontmatter_slice(text)
        assert "# important" in fm
        assert "# top-of-fm comment" in fm


# ---------------------------------------------------------------------------
# rename_key
# ---------------------------------------------------------------------------

class TestRenameKey:

    def test_basic(self):
        fm = "plan: foo\nstatus: active\n"
        out = rename_key(fm, "plan", "name")
        assert "name: foo" in out
        assert "plan:" not in out

    def test_preserves_inline_comment(self):
        """SEMANTIC PIN: preserves YAML comments on the same line."""
        fm = "plan: foo  # this is important\nstatus: active\n"
        out = rename_key(fm, "plan", "name")
        assert "name: foo  # this is important" in out

    def test_preserves_adjacent_comments(self):
        """SEMANTIC PIN: preserves comments on adjacent lines."""
        fm = "# header comment\nplan: foo\n# trailing comment\n"
        out = rename_key(fm, "plan", "name")
        assert "# header comment" in out
        assert "name: foo" in out
        assert "# trailing comment" in out

    def test_preserves_key_order(self):
        fm = "alpha: 1\nplan: foo\nzeta: 99\n"
        out = rename_key(fm, "plan", "name")
        # Order preserved: alpha, name, zeta
        lines = [l for l in out.splitlines() if l.strip() and not l.startswith("#")]
        assert lines.index("alpha: 1") < lines.index("name: foo") < lines.index("zeta: 99")

    def test_no_match_returns_unchanged(self):
        fm = "name: foo\nstatus: active\n"
        out = rename_key(fm, "plan", "name")
        assert out == fm

    def test_only_top_level_keys(self):
        """Nested 'plan' key should NOT be renamed (only ^plan: at line start)."""
        fm = "name: foo\ndepends_on:\n  - plan-a\n"
        out = rename_key(fm, "plan", "name")
        # Should NOT change "  - plan-a" inside the list
        assert "- plan-a" in out


# ---------------------------------------------------------------------------
# remove_key
# ---------------------------------------------------------------------------

class TestRemoveKey:

    def test_basic(self):
        fm = "name: foo\nreroute: false\nstatus: active\n"
        out = remove_key(fm, "reroute")
        assert "reroute" not in out
        assert "name: foo" in out
        assert "status: active" in out

    def test_preserves_other_keys(self):
        fm = "name: foo\nplan: bar\nstatus: active\n"
        out = remove_key(fm, "plan")
        assert "plan" not in out
        assert "name: foo" in out
        assert "status: active" in out

    def test_no_match_returns_unchanged(self):
        fm = "name: foo\nstatus: active\n"
        out = remove_key(fm, "missing")
        assert out == fm

    def test_handles_multi_line_value(self):
        """SEMANTIC PIN: multi-line YAML values are removed completely."""
        fm = (
            "name: foo\n"
            "third_party_review:\n"
            "  status: none\n"
            "  updated: null\n"
            "status: active\n"
        )
        out = remove_key(fm, "third_party_review")
        assert "third_party_review" not in out
        assert "status: none" not in out
        assert "updated: null" not in out
        assert "name: foo" in out
        assert "status: active" in out


# ---------------------------------------------------------------------------
# replace_value
# ---------------------------------------------------------------------------

class TestReplaceValue:

    def test_basic(self):
        fm = "status: active\nname: foo\n"
        out = replace_value(fm, "status", "queued")
        assert "status: queued" in out
        assert "active" not in out
        assert "name: foo" in out

    def test_preserves_key_formatting(self):
        fm = "status:    active\n"
        out = replace_value(fm, "status", "queued")
        # Spacing after colon preserved
        assert "status:    queued" in out

    def test_preserves_comments(self):
        fm = "status: active  # current\n"
        out = replace_value(fm, "status", "queued")
        # Inline comment preserved
        assert "# current" in out
        assert "queued" in out


# ---------------------------------------------------------------------------
# insert_key
# ---------------------------------------------------------------------------

class TestInsertKey:

    def test_after_existing(self):
        fm = "name: foo\nstatus: active\n"
        out = insert_key(fm, "reviewed", "false", after_key="status")
        lines = [l for l in out.splitlines() if l.strip()]
        assert lines.index("status: active") + 1 == lines.index("reviewed: false")

    def test_at_end_when_after_key_none(self):
        fm = "name: foo\nstatus: active\n"
        out = insert_key(fm, "reviewed", "false", after_key=None)
        assert "reviewed: false" in out
        # Should be appended near end
        lines = [l for l in out.splitlines() if l.strip()]
        assert lines[-1] == "reviewed: false"

    def test_at_end_when_after_key_missing(self):
        """If after_key not present, insert at end."""
        fm = "name: foo\nstatus: active\n"
        out = insert_key(fm, "reviewed", "false", after_key="nonexistent")
        assert "reviewed: false" in out


# ---------------------------------------------------------------------------
# remove_list_item
# ---------------------------------------------------------------------------

class TestRemoveListItem:

    def test_block_style(self):
        fm = (
            "name: foo\n"
            "depends_on:\n"
            '  - "01"\n'
            '  - "02"\n'
            "status: active\n"
        )
        out = remove_list_item(fm, "depends_on", "01")
        assert '"01"' not in out
        assert '"02"' in out

    def test_block_style_unquoted(self):
        fm = (
            "depends_on:\n"
            "  - alpha\n"
            "  - beta\n"
        )
        out = remove_list_item(fm, "depends_on", "alpha")
        assert "- alpha" not in out
        assert "- beta" in out

    def test_inline_style(self):
        fm = 'depends_on: ["01", "02"]\n'
        out = remove_list_item(fm, "depends_on", "01")
        assert '"01"' not in out
        assert '"02"' in out

    def test_no_match_returns_unchanged(self):
        fm = (
            "depends_on:\n"
            "  - alpha\n"
        )
        out = remove_list_item(fm, "depends_on", "missing")
        assert out == fm


# ---------------------------------------------------------------------------
# reassemble_file
# ---------------------------------------------------------------------------

class TestReassembleFile:

    def test_basic(self):
        original = "---\nplan: foo\nstatus: active\n---\n# Body\n"
        fm, start, end = extract_frontmatter_slice(original)
        new_fm = fm.replace("plan", "name")
        out = reassemble_file(original, new_fm, start, end)
        assert "name: foo" in out
        assert "# Body" in out
        # Fences preserved
        assert out.count("---") >= 2

    def test_preserves_body_byte_for_byte(self):
        original = "---\nname: foo\n---\n# Body\nWith multiple lines\nand stuff\n"
        fm, start, end = extract_frontmatter_slice(original)
        out = reassemble_file(original, fm, start, end)
        # Identity reassembly
        assert out == original


# ---------------------------------------------------------------------------
# Round-trip: extract -> modify -> reassemble -> plan_corpus.parser
# ---------------------------------------------------------------------------

class TestRoundTrip:

    def test_rename_via_plan_corpus_parser(self, tmp_path):
        """TPR-03-003-gemini: round-trip uses plan_corpus.parser, not yaml.safe_load."""
        original = "---\nplan: foo\nstatus: active\n---\n# Body\n"
        fm, start, end = extract_frontmatter_slice(original)
        new_fm = rename_key(fm, "plan", "name")
        out = reassemble_file(original, new_fm, start, end)
        # Parse via plan_corpus's strict parser
        path = tmp_path / "test.md"
        path.write_text(out, encoding="utf-8")
        data, _body_offset = split_frontmatter_strict(out, path)
        assert data["name"] == "foo"
        assert "plan" not in data
        assert data["status"] == "active"


# ---------------------------------------------------------------------------
# apply_patch — concurrent-session guard + atomic write
# ---------------------------------------------------------------------------

def _hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _preimage(path: Path) -> PreimageRecord:
    return PreimageRecord(
        path=path,
        content_hash=_hash(path),
        scan_timestamp=0.0,
    )


class TestApplyPatchHappyPath:

    def test_applied_true_on_match(self, tmp_path):
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\nstatus: active\n---\n# Body\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(path, ops, pre)
        assert result.applied is True
        # File now contains the rename
        assert "name: foo" in path.read_text()
        assert "plan: foo" not in path.read_text()

    def test_returns_before_and_after_hash(self, tmp_path):
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(path, ops, pre)
        assert result.before_hash == pre.content_hash
        assert result.after_hash != pre.content_hash


class TestApplyPatchConcurrentGuard:

    def test_refuses_when_hash_mismatches(self, tmp_path):
        """NEGATIVE PIN: file modified since scan -> refuse + report."""
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        # Capture preimage
        pre = _preimage(path)
        # Concurrent session modifies the file
        path.write_text("---\nplan: bar\n---\n", encoding="utf-8")
        # apply_patch should refuse
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(path, ops, pre)
        assert result.applied is False
        assert "concurrent" in result.reason.lower() or "modified" in result.reason.lower()
        # File untouched (still bar from concurrent session)
        assert "plan: bar" in path.read_text()


class TestApplyPatchMalformed:

    def test_refuses_on_no_frontmatter(self, tmp_path):
        """NEGATIVE PIN: malformed (no fences) -> refuse."""
        path = tmp_path / "test.md"
        path.write_text("# Just a body\nno frontmatter\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(FmOperationKind.RENAME_KEY, old_key="plan", new_key="name")]
        result = apply_patch(path, ops, pre)
        assert result.applied is False


class TestApplyPatchAtomicity:

    def test_no_temp_file_left_on_success(self, tmp_path):
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        apply_patch(path, ops, pre)
        # No .tmp / temp files lying around (os.replace is atomic)
        siblings = list(path.parent.iterdir())
        # Only the original file should remain
        assert len(siblings) == 1
        assert siblings[0] == path

    def test_original_intact_on_concurrent_failure(self, tmp_path):
        """File contents from concurrent modifier survive — patcher doesn't clobber."""
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(path)
        # Concurrent modify
        modified_text = "---\nname: from-concurrent-session\n---\n"
        path.write_text(modified_text, encoding="utf-8")
        ops = [FmOperation.make(FmOperationKind.RENAME_KEY, old_key="plan", new_key="name")]
        apply_patch(path, ops, pre)
        # The concurrent session's content is preserved
        assert path.read_text() == modified_text
