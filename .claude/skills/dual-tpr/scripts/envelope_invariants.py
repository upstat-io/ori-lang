"""envelope_invariants.py — code-level invariants for the dual-TPR findings envelope.

The findings-schema.json file is the canonical home for the envelope's STRUCTURE
(types, required fields, enums). It is constrained to the OpenAI Structured Outputs
JSON Schema subset because the codex CLI passes it via `--output-schema`, and the
OpenAI API rejects schemas that use `if`/`then`/`else`, `pattern`, `format`,
`maxLength`, `minimum`, and several other JSON Schema keywords (BUG-08-003).

This module is the canonical home for invariants that cannot be expressed in the
OpenAI subset:

  - location format: ^(?!/)(?!\\./)[a-zA-Z0-9_./-]+:[0-9]+$ (regex pattern)
  - title length: <= 200 chars (maxLength)
  - ordinal: >= 1 (minimum)
  - citation URL: must look like a URI (format=uri)
  - expansion_reason: required and non-empty when expanded_beyond_packet is true (if/then)
  - findings: must be empty when no_findings is true (if/then)

All three parser scripts (validate-envelope.py, parse-codex.py, parse-gemini.py)
import this module via:

    import sys, os
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from envelope_invariants import validate_envelope_invariants

The function returns None on success and a human-readable error string on failure.
The error string is suitable for the parser's `schema_violation` stderr line.

Why not put this logic inline in each parser? SSOT — three parsers all need the
same invariants enforced. Inlining would create scattered knowledge that drifts.
A single import is the canonical pattern.
"""

import re
from typing import Optional


# Regex pattern formerly enforced by JSON Schema `pattern` on findings[].location.
# Matches "path:line" with no leading slash, no leading "./", and no line ranges.
# Dotfiles like .gitignore:3 are allowed; only the literal "./prefix" is rejected.
LOCATION_RE = re.compile(r"^(?!/)(?!\./)[a-zA-Z0-9_./-]+:[0-9]+$")

# Loose URI shape: scheme://something. Mirrors the JSON Schema `format: uri` check.
# Not a strict RFC 3986 parser, but catches the common failure modes (missing
# scheme, accidentally pasting a relative path, plain text in the url field).
URI_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.\-]*://[^\s]+$")

# Maximum length of a finding title (formerly JSON Schema `maxLength: 200`).
TITLE_MAX_LENGTH = 200


def validate_envelope_invariants(envelope: dict) -> Optional[str]:
    """Validate code-level invariants on a parsed findings envelope.

    Returns None if all invariants hold; otherwise returns a human-readable
    error string describing the first violation found.

    Callers should run jsonschema.validate(envelope, schema) FIRST to catch
    structural errors, then call this function to catch the additional
    invariants that the OpenAI-compatible schema cannot express.
    """
    # Top-level invariant: when no_findings is true, findings must be empty.
    no_findings = envelope.get("no_findings", False)
    findings = envelope.get("findings", [])
    if no_findings and findings:
        return (
            f"no_findings invariant: when no_findings is true, findings must be "
            f"empty (got {len(findings)} findings)"
        )

    # scope_actually_reviewed invariant: when expanded_beyond_packet is true,
    # expansion_reason must be present and non-empty.
    scope = envelope.get("scope_actually_reviewed", {})
    if scope.get("expanded_beyond_packet") is True:
        reason = scope.get("expansion_reason", "")
        if not isinstance(reason, str) or not reason.strip():
            return (
                "scope_actually_reviewed invariant: when expanded_beyond_packet is "
                "true, expansion_reason must be a non-empty string"
            )

    # Per-finding invariants.
    for idx, finding in enumerate(findings):
        # ordinal: integer >= 1
        ordinal = finding.get("ordinal")
        if not isinstance(ordinal, int) or ordinal < 1:
            return (
                f"findings[{idx}].ordinal invariant: must be an integer >= 1 "
                f"(got {ordinal!r})"
            )

        # location: matches LOCATION_RE
        location = finding.get("location", "")
        if not isinstance(location, str) or not LOCATION_RE.match(location):
            return (
                f"findings[{idx}].location invariant: must match "
                f"^(?!/)(?!\\./)[a-zA-Z0-9_./-]+:[0-9]+$ (got {location!r})"
            )

        # title: length <= TITLE_MAX_LENGTH
        title = finding.get("title", "")
        if not isinstance(title, str) or len(title) > TITLE_MAX_LENGTH:
            return (
                f"findings[{idx}].title invariant: must be a string of length "
                f"<= {TITLE_MAX_LENGTH} (got length {len(title) if isinstance(title, str) else 'non-string'})"
            )

        # citations: each url looks like a URI
        citations = finding.get("citations", [])
        for cit_idx, citation in enumerate(citations):
            url = citation.get("url", "") if isinstance(citation, dict) else ""
            if not isinstance(url, str) or not URI_RE.match(url):
                return (
                    f"findings[{idx}].citations[{cit_idx}].url invariant: must "
                    f"look like a URI scheme://... (got {url!r})"
                )

    return None
