#!/usr/bin/env python3
"""Permanent regression tests for envelope_invariants.validate_envelope_invariants.

These tests pin both the positive cases (clean envelopes pass) and the negative
cases (each invariant fires on a corresponding violation). They are the
permanent regression guard for the moved invariants — every invariant that was
formerly enforced by JSON Schema (and got dropped because the OpenAI Structured
Outputs API rejected those keywords) has matching positive + negative tests
here.

Every test follows the <subject>_<scenario>_<expected> naming convention and
encodes ONLY the behavior under test in the function name. Provenance (the
schema rewrite that motivated this file, the OpenAI keyword subset, the
BUG-08-003 trail) lives in this docstring and the per-test ``Regression:``
comments — never in the function names.

Why these invariants live in code, not in the schema:
    The findings-schema.json file is kept within the OpenAI Structured
    Outputs JSON Schema subset (no ``if``/``then``, ``pattern``, ``maxLength``,
    ``minimum``, or ``format``) so a future phase can re-enable API-level
    enforcement without another rewrite. BUG-08-003 phase 2 removed the
    ``--output-schema`` flag from the codex invocation (commit a5a2753f),
    so the schema is currently only consumed by the parser-layer validators
    for both codex and gemini — but the subset discipline stays as insurance.
    Each invariant in this module is the code-level home for a constraint
    that the JSON Schema subset cannot carry regardless of API enforcement.

Run directly:
    python3 .claude/skills/dual-tpr/scripts/test_envelope_invariants.py
Or via the transport test harness:
    bash .claude/skills/dual-tpr/scripts/transport-tests.sh
"""

import os
import sys

# The module under test lives in the same directory.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from envelope_invariants import validate_envelope_invariants  # noqa: E402


def make_envelope(**overrides) -> dict:
    """Return a minimal valid envelope; overrides shallow-merge into the base."""
    base = {
        "schema_version": "1.0",
        "status": "complete",
        "reviewer": "codex",
        "skill": "tpr-review",
        "scope_actually_reviewed": {
            "expanded_beyond_packet": False,
            "files_read": [],
            "rules_consulted": [],
        },
        "findings": [],
        "no_findings": True,
    }
    base.update(overrides)
    return base


def make_finding(**overrides) -> dict:
    """Return a minimal valid finding object; overrides shallow-merge into the base."""
    base = {
        "ordinal": 1,
        "severity": "high",
        "location": "compiler/foo.rs:42",
        "title": "Add dec on early-exit branch",
        "evidence": "evidence body",
        "impact": "impact body",
        "basis": "fresh_verification",
    }
    base.update(overrides)
    return base


# Each test case is (name, envelope, expected) where expected is None when the
# invariants should pass, or a substring of the expected error when they should
# fail. The substring match keeps the tests robust to message wording changes
# while still pinning the specific invariant that fired.
CASES = [
    # ── Positive: clean envelopes pass ───────────────────────────────────────
    (
        "no_findings_envelope_with_empty_findings_passes_invariants",
        make_envelope(),
        None,
    ),
    (
        "envelope_with_one_valid_finding_passes_invariants",
        make_envelope(no_findings=False, findings=[make_finding()]),
        None,
    ),
    # ── no_findings invariant ────────────────────────────────────────────────
    # Regression: the schema's top-level if/then formerly enforced this; the
    # OpenAI subset rejects if/then so it moved to envelope_invariants.py.
    (
        "no_findings_true_with_non_empty_findings_fails_no_findings_invariant",
        make_envelope(findings=[make_finding()]),
        "no_findings invariant",
    ),
    # ── expansion_reason invariant ───────────────────────────────────────────
    # Regression: the schema's nested if/then on scope_actually_reviewed
    # formerly enforced this; moved to envelope_invariants.py.
    (
        "expanded_beyond_packet_true_without_expansion_reason_fails_invariant",
        make_envelope(scope_actually_reviewed={
            "expanded_beyond_packet": True,
            "files_read": [],
            "rules_consulted": [],
        }),
        "expansion_reason must be a non-empty string",
    ),
    (
        "expanded_beyond_packet_true_with_whitespace_only_expansion_reason_fails_invariant",
        make_envelope(scope_actually_reviewed={
            "expanded_beyond_packet": True,
            "expansion_reason": "  \t\n",
            "files_read": [],
            "rules_consulted": [],
        }),
        "expansion_reason must be a non-empty string",
    ),
    (
        "expanded_beyond_packet_true_with_real_expansion_reason_passes_invariants",
        make_envelope(scope_actually_reviewed={
            "expanded_beyond_packet": True,
            "expansion_reason": "Walked the ARC pass to verify dec balance.",
            "files_read": [],
            "rules_consulted": [],
        }),
        None,
    ),
    # ── location regex invariant ─────────────────────────────────────────────
    # Regression: the schema's pattern on findings[].location formerly enforced
    # this; OpenAI subset rejects pattern so it moved to envelope_invariants.py.
    (
        "finding_location_with_leading_slash_fails_location_invariant",
        make_envelope(no_findings=False, findings=[make_finding(location="/abs/path:5")]),
        "location invariant",
    ),
    (
        "finding_location_with_leading_dot_slash_fails_location_invariant",
        make_envelope(no_findings=False, findings=[make_finding(location="./rel/path:5")]),
        "location invariant",
    ),
    (
        "finding_location_without_line_number_fails_location_invariant",
        make_envelope(no_findings=False, findings=[make_finding(location="path/to/file.rs")]),
        "location invariant",
    ),
    (
        "finding_location_with_dotfile_path_passes_invariants",
        make_envelope(no_findings=False, findings=[make_finding(location=".gitignore:3")]),
        None,
    ),
    # ── title length invariant ───────────────────────────────────────────────
    # Regression: the schema's maxLength on findings[].title formerly enforced
    # this; OpenAI subset rejects maxLength so it moved to envelope_invariants.py.
    (
        "finding_title_longer_than_200_chars_fails_title_invariant",
        make_envelope(no_findings=False, findings=[make_finding(title="x" * 201)]),
        "title invariant",
    ),
    (
        "finding_title_exactly_200_chars_passes_invariants",
        make_envelope(no_findings=False, findings=[make_finding(title="x" * 200)]),
        None,
    ),
    # ── ordinal minimum invariant ────────────────────────────────────────────
    # Regression: the schema's minimum on findings[].ordinal formerly enforced
    # this; OpenAI subset rejects minimum so it moved to envelope_invariants.py.
    (
        "finding_ordinal_zero_fails_ordinal_invariant",
        make_envelope(no_findings=False, findings=[make_finding(ordinal=0)]),
        "ordinal invariant",
    ),
    (
        "finding_ordinal_negative_fails_ordinal_invariant",
        make_envelope(no_findings=False, findings=[make_finding(ordinal=-1)]),
        "ordinal invariant",
    ),
    (
        "finding_ordinal_as_string_fails_ordinal_invariant",
        make_envelope(no_findings=False, findings=[make_finding(ordinal="1")]),
        "ordinal invariant",
    ),
    # ── citation URL invariant ───────────────────────────────────────────────
    # Regression: the schema's format=uri on citations[].url formerly enforced
    # this; OpenAI subset rejects format so it moved to envelope_invariants.py.
    (
        "citation_with_https_url_passes_invariants",
        make_envelope(no_findings=False, findings=[
            make_finding(citations=[{"url": "https://example.com/path"}])
        ]),
        None,
    ),
    (
        "citation_with_file_uri_passes_invariants",
        make_envelope(no_findings=False, findings=[
            make_finding(citations=[{"url": "file:///tmp/foo.txt"}])
        ]),
        None,
    ),
    (
        "citation_with_bare_path_fails_citation_url_invariant",
        make_envelope(no_findings=False, findings=[
            make_finding(citations=[{"url": "/tmp/foo.txt"}])
        ]),
        "citations[0].url invariant",
    ),
    (
        "citation_with_empty_url_fails_citation_url_invariant",
        make_envelope(no_findings=False, findings=[
            make_finding(citations=[{"url": ""}])
        ]),
        "citations[0].url invariant",
    ),
]


def main() -> int:
    passed = 0
    failed = 0
    failed_names: list[str] = []
    for name, envelope, expected in CASES:
        result = validate_envelope_invariants(envelope)
        if expected is None:
            if result is None:
                passed += 1
            else:
                failed += 1
                failed_names.append(name)
                print(f"  FAIL: {name}")
                print(f"    expected None (pass), got: {result!r}")
        else:
            if result is not None and expected in result:
                passed += 1
            else:
                failed += 1
                failed_names.append(name)
                print(f"  FAIL: {name}")
                print(f"    expected error containing {expected!r}, got: {result!r}")

    total = passed + failed
    print(f"  envelope_invariants: {passed}/{total} cases passed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
