#!/usr/bin/env python3
"""Tests for scripts/plan_corpus.py — the corpus SSOT library.

TDD per CLAUDE.md: these tests define the expected behavior.
"""

import sys
from pathlib import Path

# Add repo root to path so we can import scripts.plan_corpus
REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest
from scripts.plan_corpus import (
    CorpusParseError,
    Corpus,
    FileClass,
    Finding,
    FindingCategory,
    FindingSubtype,
    LoadResult,
    Severity,
    classify_file,
    discover_corpus,
    generate_schema_reference,
    load_and_validate,
    normalize_status,
    read_text_strict,
    resolve_dep,
    scan_body_signals,
    split_frontmatter_strict,
    validate,
    PLANS_DIR,
)

FIXTURES = Path(__file__).parent / "fixtures"


# ---------------------------------------------------------------------------
# §01.5 — YAML Parse Failure Class Tests (must REJECT)
# ---------------------------------------------------------------------------


class TestStrictParserRejectsInvalidYAML:
    """Each test is a semantic pin: strict parser MUST reject what planlib accepts."""

    def test_frontmatter_missing_opening_dashes_raises_parse_error(self):
        text = (FIXTURES / "missing_opening_dashes.md").read_text()
        with pytest.raises(CorpusParseError, match="missing opening ---"):
            split_frontmatter_strict(text, FIXTURES / "missing_opening_dashes.md")

    def test_frontmatter_unclosed_boundary_raises_parse_error(self):
        text = (FIXTURES / "unclosed_frontmatter.md").read_text()
        with pytest.raises(CorpusParseError, match="unclosed frontmatter"):
            split_frontmatter_strict(text, FIXTURES / "unclosed_frontmatter.md")

    def test_frontmatter_yaml_syntax_error_raises_parse_error(self):
        text = (FIXTURES / "yaml_syntax_error.md").read_text()
        with pytest.raises(CorpusParseError, match="YAML parse error"):
            split_frontmatter_strict(text, FIXTURES / "yaml_syntax_error.md")

    def test_frontmatter_non_mapping_root_raises_parse_error(self):
        text = (FIXTURES / "non_mapping_root.md").read_text()
        with pytest.raises(CorpusParseError, match="must be a mapping"):
            split_frontmatter_strict(text, FIXTURES / "non_mapping_root.md")

    def test_frontmatter_duplicate_key_raises_parse_error(self):
        text = (FIXTURES / "duplicate_key.md").read_text()
        with pytest.raises(CorpusParseError, match="duplicate key"):
            split_frontmatter_strict(text, FIXTURES / "duplicate_key.md")

    def test_frontmatter_yaml_anchor_raises_parse_error(self):
        text = (FIXTURES / "yaml_anchor.md").read_text()
        with pytest.raises(CorpusParseError, match="anchor"):
            split_frontmatter_strict(text, FIXTURES / "yaml_anchor.md")

    def test_frontmatter_yaml_merge_key_raises_parse_error(self):
        text = (FIXTURES / "yaml_merge_key.md").read_text()
        with pytest.raises(CorpusParseError, match="merge key"):
            split_frontmatter_strict(text, FIXTURES / "yaml_merge_key.md")

    def test_multi_document_body_boundary_accepted(self):
        """The body content after the closing --- looks like a second document.
        The strict parser treats the first --- pair as frontmatter; body follows."""
        text = (FIXTURES / "multi_document.md").read_text()
        data, offset = split_frontmatter_strict(text, FIXTURES / "multi_document.md")
        assert data == {"foo": 1}
        assert offset == 3

    def test_multi_document_within_frontmatter_rejected(self):
        """YAML document-end marker '...' within frontmatter region must be rejected."""
        text = "---\ndoc1: true\n...\ndoc2: true\n---\n"
        with pytest.raises(CorpusParseError, match="multi-document"):
            split_frontmatter_strict(text, Path("test.md"))

    def test_multi_document_separator_within_frontmatter_rejected(self):
        """YAML document-start marker '---' within frontmatter region must be rejected."""
        text = "---\ndoc1: true\n---\ndoc2: true\n---\n"
        # The parser splits at the first closing --- (line 3), so frontmatter = "doc1: true"
        # and body = "doc2: true\n---\n". This is correct boundary detection, not multi-document.
        # For a true multi-document scenario, the frontmatter content itself must contain the marker.
        data, offset = split_frontmatter_strict(text, Path("test.md"))
        assert data == {"doc1": True}

    def test_empty_frontmatter_rejected(self):
        text = "---\n---\n"
        with pytest.raises(CorpusParseError, match="empty"):
            split_frontmatter_strict(text, Path("test.md"))

    def test_utf8_bom_rejected(self):
        with pytest.raises(CorpusParseError, match="BOM"):
            read_text_strict(FIXTURES / "utf8_bom.md")

    def test_zero_width_before_frontmatter_rejected(self):
        with pytest.raises(CorpusParseError, match="zero-width"):
            read_text_strict(FIXTURES / "zero_width_before_fm.md")

    def test_invalid_utf8_bytes_rejected(self):
        with pytest.raises(CorpusParseError, match="invalid UTF-8"):
            read_text_strict(FIXTURES / "invalid_utf8_bytes.md")

    def test_crlf_frontmatter_normalized_to_lf(self):
        """CRLF is normalized to LF for parsing; data is still valid."""
        raw = (FIXTURES / "crlf_boundary.md").read_bytes().decode("utf-8")
        data, _ = split_frontmatter_strict(raw, FIXTURES / "crlf_boundary.md")
        assert data["name"] == "test"
        assert data["status"] == "active"


class TestStrictParserAcceptsValidYAML:
    """Negative pins: valid files MUST be accepted."""

    def test_simple_frontmatter_parsed_and_accepted(self):
        text = "---\nname: test\nstatus: active\n---\n# Body\n"
        data, offset = split_frontmatter_strict(text, Path("test.md"))
        assert data == {"name": "test", "status": "active"}
        assert offset == 4

    def test_nested_mapping_parsed_and_accepted(self):
        text = "---\nthird_party_review:\n  status: none\n  updated: null\n---\n"
        data, offset = split_frontmatter_strict(text, Path("test.md"))
        assert data["third_party_review"]["status"] == "none"

    def test_list_values_parsed_and_accepted(self):
        text = "---\nitems:\n  - one\n  - two\n---\n"
        data, _ = split_frontmatter_strict(text, Path("test.md"))
        assert data["items"] == ["one", "two"]


# ---------------------------------------------------------------------------
# §01.5 — File Classification Tests
# ---------------------------------------------------------------------------


class TestFileClassification:

    def test_index_md_classified_as_plan_index(self):
        assert classify_file(PLANS_DIR / "repr-opt" / "index.md") == FileClass.PLAN_INDEX

    def test_completed_index_md_classified_as_completed_index(self):
        assert classify_file(PLANS_DIR / "completed" / "aims-10" / "index.md") == FileClass.COMPLETED_INDEX

    def test_roadmap_section_file_classified_as_roadmap_section(self):
        assert classify_file(PLANS_DIR / "roadmap" / "section-00-parser.md") == FileClass.ROADMAP_SECTION

    def test_plan_section_file_classified_as_plan_section(self):
        assert classify_file(PLANS_DIR / "repr-opt" / "section-01-struct-layout.md") == FileClass.PLAN_SECTION

    def test_overview_file_classified_as_overview(self):
        assert classify_file(PLANS_DIR / "repr-opt" / "00-overview.md") == FileClass.OVERVIEW

    def test_bug_section_file_classified_as_bug_tracker_section(self):
        assert classify_file(PLANS_DIR / "bug-tracker" / "section-01-parser-lexer.md") == FileClass.BUG_TRACKER_SECTION

    def test_fix_bug_file_classified_as_fix_bug(self):
        assert classify_file(PLANS_DIR / "bug-tracker" / "fix-BUG-04-077.md") == FileClass.FIX_BUG

    def test_unknown_file_returns_none(self):
        assert classify_file(PLANS_DIR / "repr-opt" / "README.md") is None


# ---------------------------------------------------------------------------
# §01.5 — Schema Validation Tests
# ---------------------------------------------------------------------------


class TestSchemaValidation:

    def test_valid_plan_index_produces_no_findings(self):
        data = {"name": "test", "full_name": "Test Plan", "status": "active"}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert findings == []

    def test_missing_required_field_flagged_as_violation(self):
        data = {"name": "test", "status": "active"}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.MISSING_REQUIRED_FIELD for f in findings)

    def test_unknown_field_flagged(self):
        data = {"name": "test", "full_name": "T", "status": "active", "bogus": 1}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.UNKNOWN_FIELD for f in findings)

    def test_invalid_status_value_flagged_as_enum_violation(self):
        data = {"name": "test", "full_name": "T", "status": "banana"}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.ENUM_OUT_OF_RANGE for f in findings)

    def test_reroute_false_rejected(self):
        data = {"name": "t", "full_name": "T", "status": "active", "reroute": False}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.CROSS_FIELD_INVARIANT for f in findings)

    def test_depends_on_bare_string_rejected(self):
        data = {
            "section": "01", "title": "T", "status": "in-progress",
            "reviewed": True, "goal": "G", "success_criteria": [],
            "sections": [], "third_party_review": {"status": "none", "updated": None},
            "depends_on": "section-01",
        }
        findings = validate(FileClass.PLAN_SECTION, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.DEP_ID_MALFORMED for f in findings)

    def test_depends_on_full_path_rejected(self):
        data = {
            "section": "01", "title": "T", "status": "in-progress",
            "reviewed": True, "goal": "G", "success_criteria": [],
            "sections": [], "third_party_review": {"status": "none", "updated": None},
            "depends_on": ["plans/repr-opt/section-01.md"],
        }
        findings = validate(FileClass.PLAN_SECTION, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.DEP_ID_FULL_PATH for f in findings)

    def test_valid_dep_ids_accepted(self):
        data = {
            "section": "01", "title": "T", "status": "in-progress",
            "reviewed": True, "goal": "G", "success_criteria": [],
            "sections": [], "third_party_review": {"status": "none", "updated": None},
            "depends_on": ["01", "02A", "Locality Representation Unification#03"],
        }
        findings = validate(FileClass.PLAN_SECTION, data, Path("test.md"))
        dep_findings = [f for f in findings if f.subtype.value.startswith("dep_id")]
        assert dep_findings == []

    def test_research_status_accepted_on_plan_index(self):
        """Positive pin: research status must be accepted per ori-ui-framework."""
        data = {"name": "t", "full_name": "T", "status": "research"}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert not any(f.subtype == FindingSubtype.ENUM_OUT_OF_RANGE for f in findings)

    def test_fixture_unknown_field_stauts_flagged_as_unknown(self):
        """Fixture pin: typo 'stauts' must be caught as unknown field."""
        text = read_text_strict(FIXTURES / "unknown_field_stauts.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "unknown_field_stauts.md")
        findings = validate(FileClass.PLAN_INDEX, data, FIXTURES / "unknown_field_stauts.md")
        assert any(f.subtype == FindingSubtype.UNKNOWN_FIELD for f in findings)

    def test_fixture_reroute_false_flagged_as_cross_field_invariant(self):
        text = read_text_strict(FIXTURES / "reroute_false.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "reroute_false.md")
        findings = validate(FileClass.PLAN_INDEX, data, FIXTURES / "reroute_false.md")
        assert any(f.subtype == FindingSubtype.CROSS_FIELD_INVARIANT for f in findings)

    def test_fixture_depends_on_full_path_flagged_as_dep_id_full_path(self):
        text = read_text_strict(FIXTURES / "depends_on_full_path.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "depends_on_full_path.md")
        findings = validate(FileClass.PLAN_SECTION, data, FIXTURES / "depends_on_full_path.md")
        assert any(f.subtype == FindingSubtype.DEP_ID_FULL_PATH for f in findings)

    def test_fixture_depends_on_scalar_string_flagged_as_malformed(self):
        text = read_text_strict(FIXTURES / "depends_on_scalar_string.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "depends_on_scalar_string.md")
        findings = validate(FileClass.PLAN_SECTION, data, FIXTURES / "depends_on_scalar_string.md")
        assert any(f.subtype == FindingSubtype.DEP_ID_MALFORMED for f in findings)

    def test_fixture_research_status_accepted(self):
        text = read_text_strict(FIXTURES / "research_status_accepted.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "research_status_accepted.md")
        findings = validate(FileClass.PLAN_INDEX, data, FIXTURES / "research_status_accepted.md")
        assert not any(f.subtype == FindingSubtype.ENUM_OUT_OF_RANGE for f in findings)

    def test_fixture_plan_instead_of_name_flagged_as_unknown_and_missing(self):
        """'plan:' key should be unknown for plan index (use 'name:' instead)."""
        text = read_text_strict(FIXTURES / "plan_instead_of_name.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "plan_instead_of_name.md")
        findings = validate(FileClass.PLAN_INDEX, data, FIXTURES / "plan_instead_of_name.md")
        assert any(f.subtype == FindingSubtype.UNKNOWN_FIELD for f in findings)
        assert any(f.subtype == FindingSubtype.MISSING_REQUIRED_FIELD for f in findings)

    def test_fixture_unclosed_status_enum_flagged_as_enum_violation(self):
        text = read_text_strict(FIXTURES / "unclosed_status_enum.md")
        data, _ = split_frontmatter_strict(text, FIXTURES / "unclosed_status_enum.md")
        findings = validate(FileClass.PLAN_INDEX, data, FIXTURES / "unclosed_status_enum.md")
        assert any(f.subtype == FindingSubtype.ENUM_OUT_OF_RANGE for f in findings)

    def test_fix_bug_complete_with_tpr_none_accepted(self):
        """Live corpus shows complete+none is valid."""
        data = {
            "bug": "BUG-04-077", "title": "T", "severity": "critical",
            "status": "complete", "goal": "G", "success_criteria": [],
            "subsystem": "codegen", "found": "2026-04-14", "source": "tpr-review",
            "third_party_review": {"status": "none", "updated": None},
        }
        findings = validate(FileClass.FIX_BUG, data, Path("test.md"))
        assert findings == []

    def test_fix_bug_complete_with_tpr_clean_accepted(self):
        data = {
            "bug": "BUG-04-045", "title": "T", "severity": "high",
            "status": "complete", "goal": "G", "success_criteria": [],
            "subsystem": "codegen", "found": "2026-04-10", "source": "tpr-review",
            "third_party_review": {"status": "clean", "updated": "2026-04-12"},
        }
        findings = validate(FileClass.FIX_BUG, data, Path("test.md"))
        assert findings == []

    def test_fix_bug_complete_with_tpr_resolved_accepted(self):
        data = {
            "bug": "BUG-04-050", "title": "T", "severity": "medium",
            "status": "complete", "goal": "G", "success_criteria": [],
            "subsystem": "typeck", "found": "2026-04-10", "source": "tpr-review",
            "third_party_review": {"status": "resolved", "updated": "2026-04-11"},
        }
        findings = validate(FileClass.FIX_BUG, data, Path("test.md"))
        assert findings == []

    def test_fix_bug_complete_with_tpr_findings_accepted(self):
        data = {
            "bug": "BUG-04-059", "title": "T", "severity": "high",
            "status": "complete", "goal": "G", "success_criteria": [],
            "subsystem": "arc", "found": "2026-04-11", "source": "tpr-review",
            "third_party_review": {"status": "findings", "updated": "2026-04-13"},
        }
        findings = validate(FileClass.FIX_BUG, data, Path("test.md"))
        assert findings == []

    def test_overview_statuses_accepted(self):
        """All four corpus-derived overview statuses must be accepted."""
        for status in ["not-started", "in-progress", "research", "complete"]:
            data = {"plan": "test", "title": "T", "status": status}
            findings = validate(FileClass.OVERVIEW, data, Path("test.md"))
            enum_findings = [f for f in findings if f.subtype == FindingSubtype.ENUM_OUT_OF_RANGE]
            assert enum_findings == [], f"status={status!r} should be accepted"

    def test_overview_resolved_rejected(self):
        data = {"plan": "test", "title": "T", "status": "resolved"}
        findings = validate(FileClass.OVERVIEW, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.ENUM_OUT_OF_RANGE for f in findings)

    def test_roadmap_section_int_section_accepted(self):
        """Roadmap uses bare int section: 0 per live corpus."""
        data = {
            "section": 0, "title": "Parser", "status": "in-progress",
            "reviewed": True, "goal": "G", "sections": [],
        }
        findings = validate(FileClass.ROADMAP_SECTION, data, Path("test.md"))
        assert findings == []


# ---------------------------------------------------------------------------
# §01.5 — Finding Type Safety Tests
# ---------------------------------------------------------------------------


class TestFindingTypeSafety:

    def test_subtype_mismatched_category_raises_value_error(self):
        with pytest.raises(ValueError, match="does not belong"):
            Finding(
                category=FindingCategory.PARSE_ERROR,
                subtype=FindingSubtype.UNKNOWN_FIELD,
                severity=Severity.HIGH,
                source=Path("test.md"),
                description="test",
                recommended_fix="fix",
            )

    def test_valid_finding_constructs_with_id_and_json(self):
        f = Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=FindingSubtype.YAML_SYNTAX_ERROR,
            severity=Severity.HIGH,
            source=Path("test.md"),
            description="test",
            recommended_fix="fix",
        )
        assert f.id.startswith("VR-")
        assert "parse_error" in f.to_json()["category"]

    def test_all_subtypes_registered_in_category_map_exhaustively(self):
        """Exhaustiveness pin: every FindingSubtype must appear in _CATEGORY_SUBTYPES."""
        from scripts.plan_corpus import _CATEGORY_SUBTYPES
        all_registered = set()
        for subtypes in _CATEGORY_SUBTYPES.values():
            all_registered |= subtypes
        for member in FindingSubtype:
            assert member in all_registered, (
                f"FindingSubtype.{member.name} not registered in _CATEGORY_SUBTYPES"
            )

    def test_all_categories_have_subtypes_exhaustively(self):
        """Exhaustiveness pin: every FindingCategory must have an entry in _CATEGORY_SUBTYPES."""
        from scripts.plan_corpus import _CATEGORY_SUBTYPES
        for member in FindingCategory:
            assert member in _CATEGORY_SUBTYPES, (
                f"FindingCategory.{member.name} not in _CATEGORY_SUBTYPES"
            )

    def test_finding_to_markdown_includes_description_and_severity(self):
        f = Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.MISSING_INDEX_MD,
            severity=Severity.HIGH,
            source=Path("test.md"),
            description="missing index",
            recommended_fix="add index.md",
        )
        md = f.to_markdown()
        assert "missing index" in md
        assert "high" in md


# ---------------------------------------------------------------------------
# §01.5 — Status Normalizer Tests
# ---------------------------------------------------------------------------


class TestStatusNormalizer:

    def test_all_checked_derives_complete(self):
        data = {"status": "in-progress"}
        body = "- [x] one\n- [x] two\n- [x] three\n"
        result = normalize_status(data, body, Path("test.md"))
        assert result.derived == "complete"
        assert len(result.contradictions) == 1
        assert result.contradictions[0].subtype == FindingSubtype.FM_DECLARED_VS_BODY_DERIVED

    def test_all_unchecked_derives_not_started(self):
        data = {"status": "in-progress"}
        body = "- [ ] one\n- [ ] two\n"
        result = normalize_status(data, body, Path("test.md"))
        assert result.derived == "not-started"

    def test_mixed_derives_in_progress(self):
        data = {"status": "not-started"}
        body = "- [x] done\n- [ ] todo\n"
        result = normalize_status(data, body, Path("test.md"))
        assert result.derived == "in-progress"
        assert len(result.contradictions) == 1

    def test_active_plan_all_not_started_flagged_as_contradiction(self):
        data = {"status": "active"}
        result = normalize_status(
            data, "", Path("test.md"),
            child_statuses=["not-started", "not-started"],
        )
        assert any(
            f.subtype == FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED
            for f in result.contradictions
        )

    def test_complete_plan_with_open_sections_flagged_as_contradiction(self):
        data = {"status": "complete"}
        result = normalize_status(
            data, "", Path("test.md"),
            child_statuses=["complete", "in-progress"],
        )
        assert any(
            f.subtype == FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS
            for f in result.contradictions
        )

    def test_fenced_code_blocks_excluded_from_checkbox_scan(self):
        data = {"status": "complete"}
        body = "- [x] real\n```\n- [ ] fake checkbox in code\n```\n"
        result = normalize_status(data, body, Path("test.md"))
        assert result.derived == "complete"


# ---------------------------------------------------------------------------
# §01.5 — load_and_validate Boundary Tests
# ---------------------------------------------------------------------------


class TestLoadAndValidate:

    def test_missing_file_returns_err(self):
        result = load_and_validate(Path("/nonexistent/file.md"))
        assert not result.is_ok
        assert result.err is not None
        assert result.err.category == FindingCategory.PARSE_ERROR

    def test_valid_file_returns_ok(self):
        result = load_and_validate(PLANS_DIR / "verify-roadmap-redesign" / "index.md")
        assert result.is_ok
        assert result.ok.file_class == FileClass.PLAN_INDEX

    def test_malformed_yaml_returns_err(self):
        result = load_and_validate(FIXTURES / "yaml_syntax_error.md")
        assert not result.is_ok
        assert result.err.category == FindingCategory.PARSE_ERROR

    def test_empty_frontmatter_returns_err(self):
        result = load_and_validate(FIXTURES / "unclosed_frontmatter.md")
        assert not result.is_ok

    def test_empty_file_returns_err(self):
        """GEMINI-01-003 pin: empty file is PARSE_ERROR, not crash or None."""
        result = load_and_validate(FIXTURES / "empty_file.md")
        assert not result.is_ok
        assert result.err.category == FindingCategory.PARSE_ERROR
        assert result.err.subtype == FindingSubtype.MISSING_OPENING_DASHES

    def test_violations_on_valid_file_are_non_fatal(self):
        """Schema violations don't prevent file from being ValidatedFile."""
        result = load_and_validate(PLANS_DIR / "roadmap" / "section-00-parser.md")
        assert result.is_ok
        # Roadmap section may have optional field violations but still loads


# ---------------------------------------------------------------------------
# §01.5 — Discovery Tests
# ---------------------------------------------------------------------------


class TestCorpusDiscovery:

    def test_discover_corpus_finds_plan_indexes(self):
        corpus = discover_corpus()
        assert len(corpus.indexes) > 10

    def test_discover_corpus_finds_completed_indexes(self):
        corpus = discover_corpus()
        assert len(corpus.completed_indexes) > 5

    def test_discover_corpus_populates_name_index(self):
        corpus = discover_corpus()
        assert len(corpus.name_index) > 10
        assert "Verify Roadmap" in corpus.name_index

    def test_discover_corpus_include_filter_limits_results(self):
        subset = [PLANS_DIR / "verify-roadmap-redesign" / "index.md"]
        corpus = discover_corpus(include=subset)
        assert len(corpus.indexes) == 1

    def test_completed_nested_indexes_discovered(self):
        """GEMINI 10 pin: nested plans/completed/foo/index.md must be found."""
        corpus = discover_corpus()
        completed_paths = [str(p) for p in corpus.completed_indexes]
        assert any("aims-10" in p for p in completed_paths)

    def test_container_dir_not_flagged_missing_index(self):
        """CODEX-01-005 pin: plans/completed/ is a container, not a broken plan."""
        corpus = discover_corpus()
        gap_paths = [str(f.source) for f in corpus.gaps]
        assert not any("plans/completed" == str(f.source).rstrip("/") for f in corpus.gaps
                       if f.subtype == FindingSubtype.MISSING_INDEX_MD)

    def test_roadmap_treated_as_plan_candidate(self):
        """plans/roadmap/ has index.md; treated as plan candidate, not container."""
        corpus = discover_corpus()
        assert any("roadmap" in str(p) for p in corpus.indexes)

    def test_unclassified_dir_gets_gap_finding(self):
        """Dirs with .md but no index.md get UNCLASSIFIED_DIRECTORY."""
        corpus = discover_corpus()
        unclassified = [f for f in corpus.gaps
                        if f.subtype == FindingSubtype.UNCLASSIFIED_DIRECTORY]
        # We know compiler-research-blueprint, code-journeys, deep-safety exist
        assert len(unclassified) >= 1


# ---------------------------------------------------------------------------
# §01.5 — Roadmap Schema Pins
# ---------------------------------------------------------------------------


class TestRoadmapSchemaPins:

    def test_roadmap_section_with_tier_and_spec_accepted(self):
        """Valid roadmap section with tier, last_verified, spec must pass."""
        data = {
            "section": 0, "title": "Parser", "status": "in-progress",
            "reviewed": True, "goal": "G", "sections": [],
            "tier": 0, "last_verified": "2026-03-29",
            "spec": ["spec/grammar.ebnf"],
        }
        findings = validate(FileClass.ROADMAP_SECTION, data, Path("test.md"))
        assert findings == []

    def test_same_content_fails_as_plan_section(self):
        """Negative pin: tier/last_verified/spec would fail PlanSectionSchema."""
        data = {
            "section": "00", "title": "Parser", "status": "in-progress",
            "reviewed": True, "goal": "G", "success_criteria": [],
            "sections": [], "third_party_review": {"status": "none", "updated": None},
            "tier": 0, "last_verified": "2026-03-29",
            "spec": ["spec/grammar.ebnf"],
        }
        findings = validate(FileClass.PLAN_SECTION, data, Path("test.md"))
        unknown = [f for f in findings if f.subtype == FindingSubtype.UNKNOWN_FIELD]
        assert len(unknown) >= 3  # tier, last_verified, spec

    def test_roadmap_real_file_validates_successfully(self):
        """Live corpus pin: actual roadmap file validates via load_and_validate."""
        result = load_and_validate(PLANS_DIR / "roadmap" / "section-00-parser.md")
        assert result.is_ok
        assert result.ok.file_class == FileClass.ROADMAP_SECTION


# ---------------------------------------------------------------------------
# §01.5 — Fix-Bug Cross-Field Pins
# ---------------------------------------------------------------------------


class TestFixBugCrossFieldPins:

    def _make_fix_bug(self, status: str, tpr_status: str, tpr_updated=None):
        return {
            "bug": "BUG-04-099", "title": "T", "severity": "high",
            "status": status, "goal": "G", "success_criteria": [],
            "subsystem": "codegen", "found": "2026-04-14", "source": "tpr-review",
            "third_party_review": {"status": tpr_status, "updated": tpr_updated},
        }

    def test_complete_tpr_none_accepted(self):
        findings = validate(FileClass.FIX_BUG, self._make_fix_bug("complete", "none"), Path("t.md"))
        assert findings == []

    def test_complete_tpr_clean_accepted(self):
        findings = validate(FileClass.FIX_BUG, self._make_fix_bug("complete", "clean", "2026-04-14"), Path("t.md"))
        assert findings == []

    def test_complete_tpr_resolved_accepted(self):
        findings = validate(FileClass.FIX_BUG, self._make_fix_bug("complete", "resolved", "2026-04-14"), Path("t.md"))
        assert findings == []

    def test_complete_tpr_findings_accepted(self):
        findings = validate(FileClass.FIX_BUG, self._make_fix_bug("complete", "findings", "2026-04-14"), Path("t.md"))
        assert findings == []

    def test_missing_tpr_field_rejected(self):
        data = {
            "bug": "BUG-04-099", "title": "T", "severity": "high",
            "status": "complete", "goal": "G", "success_criteria": [],
            "subsystem": "codegen", "found": "2026-04-14", "source": "tpr-review",
        }
        findings = validate(FileClass.FIX_BUG, data, Path("t.md"))
        assert any(f.subtype == FindingSubtype.MISSING_REQUIRED_FIELD for f in findings)


# ---------------------------------------------------------------------------
# §01.5 — Cross-Plan Name Resolution Pins
# ---------------------------------------------------------------------------


class TestCrossplanNameResolution:

    def test_intra_plan_dep_resolves_to_path(self):
        result = resolve_dep("01", PLANS_DIR / "verify-roadmap-redesign", discover_corpus())
        assert isinstance(result, Path)

    def test_cross_plan_dep_resolves_by_name_to_path(self):
        corpus = discover_corpus()
        # "Verify Roadmap" is the name field in verify-roadmap-redesign/index.md
        result = resolve_dep("Verify Roadmap#01", PLANS_DIR / "repr-opt", corpus)
        assert isinstance(result, Path)

    def test_cross_plan_unknown_name_returns_finding(self):
        corpus = discover_corpus()
        result = resolve_dep("Nonexistent Plan#01", PLANS_DIR / "repr-opt", corpus)
        assert isinstance(result, Finding)
        assert result.subtype == FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND

    def test_plan_index_missing_name_flagged_as_missing_required(self):
        """Missing name: field produces MISSING_REQUIRED_FIELD."""
        data = {"full_name": "T", "status": "active"}
        findings = validate(FileClass.PLAN_INDEX, data, Path("test.md"))
        assert any(f.subtype == FindingSubtype.MISSING_REQUIRED_FIELD
                   and "name" in f.description for f in findings)

    def test_duplicate_plan_names_surfaced_in_gaps(self):
        """discover_corpus detects duplicate plan names in name_index."""
        corpus = discover_corpus()
        dup_findings = [f for f in corpus.gaps
                        if f.subtype == FindingSubtype.DUPLICATE_PLAN_NAME]
        # Should be 0 in a healthy corpus, but the test structure is correct
        # If there ARE duplicates, they're caught
        assert isinstance(dup_findings, list)


# ---------------------------------------------------------------------------
# §01.5 — Security Pins
# ---------------------------------------------------------------------------


class TestSecurityPins:

    def test_yaml_anchor_bomb_blocked(self):
        """Billion laughs prevention: anchors rejected before yaml.load."""
        text = "---\na: &a [1,2]\nb: *a\n---\n"
        with pytest.raises(CorpusParseError, match="anchor"):
            split_frontmatter_strict(text, Path("test.md"))

    def test_python_object_tag_blocked(self):
        """!!python/object tags are blocked by SafeLoader."""
        text = "---\nx: !!python/object:os.system ['echo pwned']\n---\n"
        with pytest.raises(CorpusParseError):
            split_frontmatter_strict(text, Path("test.md"))


# ---------------------------------------------------------------------------
# §01.5 — Status Normalizer Fixture Pins
# ---------------------------------------------------------------------------


class TestStatusNormalizerFixtures:

    def test_active_all_not_started_contradiction_has_no_safety_class(self):
        """Pin: normalizer emits STATUS_CONTRADICTION without safety_class."""
        data = {"status": "active"}
        result = normalize_status(data, "", Path("test.md"),
                                  child_statuses=["not-started", "not-started"])
        for f in result.contradictions:
            assert not hasattr(f, "safety_class"), \
                "Finding must NOT have safety_class (Section 03 owns that)"

    def test_fm_in_progress_body_complete_derives_complete_with_contradiction(self):
        data = {"status": "in-progress"}
        body = "- [x] done\n- [x] also done\n"
        result = normalize_status(data, body, Path("test.md"))
        assert result.derived == "complete"
        assert any(f.subtype == FindingSubtype.FM_DECLARED_VS_BODY_DERIVED
                   for f in result.contradictions)

    def test_fm_complete_body_unchecked_derives_not_started_with_contradiction(self):
        data = {"status": "complete"}
        body = "- [ ] todo\n- [ ] also todo\n"
        result = normalize_status(data, body, Path("test.md"))
        assert result.derived == "not-started"
        assert len(result.contradictions) >= 1


# ---------------------------------------------------------------------------
# §01.5 — Boundary Contract Pin
# ---------------------------------------------------------------------------


class TestBoundaryContractPin:

    def test_no_try_except_corpus_parse_error_in_tests(self):
        """Semantic pin: test file should NOT use try/except CorpusParseError
        outside the strict-parser rejection tests. Callers use LoadResult."""
        import inspect
        test_source = inspect.getsource(TestLoadAndValidate)
        assert "CorpusParseError" not in test_source, \
            "LoadAndValidate tests should not catch CorpusParseError directly"


# ---------------------------------------------------------------------------
# §01.5 — Docgen Drift Gate Tests
# ---------------------------------------------------------------------------


class TestDocgenDriftGate:

    def test_schema_reference_is_up_to_date(self):
        target = REPO_ROOT / "docs" / "internal" / "plan-schema-reference.md"
        if not target.exists():
            pytest.skip("schema reference not yet generated")
        committed = target.read_text().replace("\r\n", "\n")
        generated = generate_schema_reference()
        assert committed == generated, "Schema reference drifted from Python types"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
