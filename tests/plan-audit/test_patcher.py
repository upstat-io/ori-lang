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

    def test_after_block_list_anchor_skips_indented_items(self):
        """Regression: insert_key must skip indented continuation lines after
        a block-valued anchor (e.g., sections:) so the new key does NOT get
        wedged between the anchor and its list items, which would produce
        invalid YAML.

        See: TPR-03-001-codex / TPR-03-001-gemini (agreement finding).
        """
        from scripts.plan_corpus import split_frontmatter_strict

        fm = (
            "section: '03'\n"
            "title: Foo\n"
            "status: in-progress\n"
            "sections:\n"
            "  - id: '03.1'\n"
            "    title: First\n"
            "    status: complete\n"
            "  - id: '03.2'\n"
            "    title: Second\n"
            "    status: complete\n"
        )
        out = insert_key(
            fm,
            "third_party_review",
            "\n  status: none\n  updated: null",
            after_key="sections",
        )

        # The new key must appear AFTER the indented list items, not between
        # the sections: header and its first - id: entry.
        sections_idx = out.index("sections:")
        first_item_idx = out.index("- id: '03.1'")
        tpr_idx = out.index("third_party_review:")
        last_item_idx = out.index("- id: '03.2'")
        assert sections_idx < first_item_idx < last_item_idx < tpr_idx, (
            f"third_party_review wedged inside sections list:\n{out}"
        )

        # And the result must reparse as valid YAML through the canonical
        # parse-error-lifting boundary — `split_frontmatter_strict` rejects
        # the broken pre-fix layout (mixed mapping/sequence at same indent).
        wrapped = "---\n" + out + "---\nbody\n"
        parsed, _body_offset = split_frontmatter_strict(
            wrapped, Path("test.md")
        )
        # If we reach here without CorpusParseError, the fix held.
        assert "third_party_review" in parsed
        assert isinstance(parsed.get("sections"), list)
        assert len(parsed["sections"]) == 2
        # Both list items survived as proper mapping entries under sections
        assert parsed["sections"][0]["id"] == "03.1"
        assert parsed["sections"][1]["id"] == "03.2"


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

    def test_block_style_with_inline_comment_on_key_line(self):
        """Regression: remove_list_item must treat an inline comment on the
        key line (e.g. `depends_on: # comment`) as a valid block-list opener.

        Pre-fix the strict regex `^depends_on\\s*:\\s*$` failed to match the
        line, in_list never became True, and the item was never removed.

        See: TPR-03-003-gemini.
        """
        fm = (
            "name: foo\n"
            "depends_on: # external blockers\n"
            "  - alpha\n"
            "  - beta\n"
            "status: active\n"
        )
        out = remove_list_item(fm, "depends_on", "alpha")
        assert "- alpha" not in out
        assert "- beta" in out
        # Comment on the key line is preserved
        assert "depends_on: # external blockers" in out


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
        result = apply_patch(path, ops, pre, tmp_path)
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
        result = apply_patch(path, ops, pre, tmp_path)
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
        result = apply_patch(path, ops, pre, tmp_path)
        assert result.applied is False
        assert "concurrent" in result.reason.lower() or "modified" in result.reason.lower()
        # File untouched (still bar from concurrent session)
        assert "plan: bar" in path.read_text()

    def test_propagates_finding_id_through_refusal(self, tmp_path):
        """Regression: a refused patch must surface the originating Finding.id
        in PatchResult, not the synthetic 'VR-patch' sentinel.

        See: TPR-03-004-codex.
        """
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(path)
        # Concurrent session forces refusal
        path.write_text("---\nplan: bar\n---\n", encoding="utf-8")
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(
            path, ops, pre, tmp_path, finding_id="VR-deadbeef",
        )
        assert result.applied is False
        assert result.finding_id == "VR-deadbeef"

    def test_refuses_when_disk_changes_between_check_and_replace(self, tmp_path):
        """Regression: pre-replace CAS check refuses when a concurrent writer
        lands AFTER the initial preimage check passes but BEFORE os.replace.

        Simulated by monkey-patching `_sha256_hex` to mutate the file on its
        second invocation (which occurs at the pre-replace re-read site —
        Steps 2 and 6 in apply_patch's docstring). Without the CAS guard,
        the patcher would silently overwrite the concurrent writer's bytes.

        See: TPR-03-003-codex.
        """
        from scripts.verify_roadmap import patcher as patcher_mod

        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]

        original_sha = patcher_mod._sha256_hex
        invocation = {"count": 0}

        def racy_sha(data: bytes) -> str:
            invocation["count"] += 1
            # First invocation = Step 2 preimage check (let it pass).
            # Second invocation = Step 6 pre-replace re-read — RIGHT BEFORE
            # this we land a concurrent write so the disk hash differs.
            if invocation["count"] == 2:
                path.write_text("---\nplan: from-concurrent\n---\n", encoding="utf-8")
                # Re-read so the hash we return reflects the new content.
                return original_sha(path.read_bytes())
            return original_sha(data)

        patcher_mod._sha256_hex = racy_sha
        try:
            result = apply_patch(path, ops, pre, tmp_path)
        finally:
            patcher_mod._sha256_hex = original_sha

        assert result.applied is False
        assert (
            "between scan and replace" in result.reason.lower()
            or "modified" in result.reason.lower()
        )
        # The concurrent writer's content survives — no clobber.
        assert "plan: from-concurrent" in path.read_text()


class TestApplyPatchMalformed:

    def test_refuses_on_no_frontmatter(self, tmp_path):
        """NEGATIVE PIN: malformed (no fences) -> refuse."""
        path = tmp_path / "test.md"
        path.write_text("# Just a body\nno frontmatter\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(FmOperationKind.RENAME_KEY, old_key="plan", new_key="name")]
        result = apply_patch(path, ops, pre, tmp_path)
        assert result.applied is False


class TestApplyPatchAtomicity:

    def test_no_temp_file_left_on_success(self, tmp_path):
        path = tmp_path / "test.md"
        path.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(path)
        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        apply_patch(path, ops, pre, tmp_path)
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
        apply_patch(path, ops, pre, tmp_path)
        # The concurrent session's content is preserved
        assert path.read_text() == modified_text


# ---------------------------------------------------------------------------
# apply_patch — path-escape refusal (4th concurrent-session guard)
# Regression test for TPR-03-001-{codex,gemini}-r4 agreement finding.
# ---------------------------------------------------------------------------

class TestApplyPatchPathEscape:

    def test_refuses_path_outside_corpus_root(self, tmp_path):
        """NEGATIVE PIN: path outside corpus_root -> PatchResult(applied=False).

        A bogus or drifted Finding.source that points outside the reviewed
        plans/ directory must never drive apply_patch to rewrite arbitrary
        files. This is the 4th refuse-on-conflict trigger required by the
        §03.4 concurrent-session contract.
        """
        # corpus_root is an isolated subdir; target sits outside it.
        corpus_root = tmp_path / "plans"
        corpus_root.mkdir()
        outside = tmp_path / "outside.md"
        outside.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(outside)

        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(outside, ops, pre, corpus_root)

        assert result.applied is False
        assert "escape" in result.reason.lower() or "not under" in result.reason.lower()
        # File is untouched — original "plan: foo" still there
        assert "plan: foo" in outside.read_text()
        assert "name: foo" not in outside.read_text()

    def test_accepts_path_inside_corpus_root(self, tmp_path):
        """POSITIVE PIN: path inside corpus_root -> proceeds normally."""
        corpus_root = tmp_path / "plans"
        corpus_root.mkdir()
        inside = corpus_root / "p1" / "index.md"
        inside.parent.mkdir()
        inside.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        pre = _preimage(inside)

        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(inside, ops, pre, corpus_root)

        assert result.applied is True
        assert "name: foo" in inside.read_text()

    def test_refuses_parent_traversal(self, tmp_path):
        """NEGATIVE PIN: `..` traversal out of corpus_root is refused."""
        corpus_root = tmp_path / "plans"
        corpus_root.mkdir()
        # File placed legitimately in tmp_path/plans, but path uses .. to escape
        outside = tmp_path / "outside.md"
        outside.write_text("---\nplan: foo\n---\n", encoding="utf-8")
        # Construct a path with .. that resolves to tmp_path/outside.md
        traversal_path = corpus_root / ".." / "outside.md"
        pre = PreimageRecord(
            path=traversal_path,
            content_hash=hashlib.sha256(outside.read_bytes()).hexdigest(),
            scan_timestamp=0.0,
        )

        ops = [FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
        )]
        result = apply_patch(traversal_path, ops, pre, corpus_root)

        assert result.applied is False
        assert "escape" in result.reason.lower() or "not under" in result.reason.lower()
        assert "plan: foo" in outside.read_text()
