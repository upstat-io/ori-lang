"""repair_envelope.py — auto-repair common envelope schema violations.

LLM reviewers (especially Gemini) frequently produce envelopes that are
semantically correct but structurally non-conformant: missing a required
field like `no_findings`, using `"info"` instead of `"informational"` for
severity, omitting `schema_version`, etc. These are deterministic failures
that the retry loop cannot fix — a fresh Gemini invocation tends to produce
the same class of violation.

This module provides a repair pass that normalizes common violations BEFORE
schema validation. The repair is:
  - **Idempotent**: running it on already-valid input returns it unchanged.
  - **Logged**: every repair is appended to a list of human-readable strings
    so callers can emit them as warnings.
  - **Conservative**: repairs only fill in defaults or normalize known enum
    aliases. It never fabricates substantive content (e.g., evidence text).
  - **Shared**: both parse-codex.py and parse-gemini.py import this module
    for symmetric enforcement.

Usage:
    from repair_envelope import repair_envelope
    repaired, repairs = repair_envelope(envelope, default_reviewer="gemini")
    for r in repairs:
        print(f"REPAIR: {r}", file=sys.stderr)
    # then proceed with jsonschema.validate(repaired, schema)
"""

import re
from typing import Optional

# Callers (parse-codex.py, parse-gemini.py) add this directory to sys.path
# before importing repair_envelope, so sibling imports work.
from envelope_invariants import LOCATION_RE, URI_RE

# Valid enum values (source of truth: findings-schema.json)
VALID_STATUSES = {"complete", "failed_partial"}
VALID_SKILLS = {"tpr-review", "review-work", "review-plan", "tp-help", "custom"}
VALID_SEVERITIES = {"high", "medium", "low", "informational"}
VALID_BASES = {"fresh_verification", "direct_file_inspection", "git_history", "inference"}
VALID_LAYERS = {"committed", "staged", "unstaged"}
VALID_CONFIDENCES = {"high", "medium", "low"}

# Alias maps for common LLM misspellings / abbreviations
SEVERITY_ALIASES = {
    "h": "high", "critical": "high", "error": "high",
    "med": "medium", "moderate": "medium", "warning": "medium", "warn": "medium",
    "l": "low", "minor": "low", "suggestion": "low",
    "info": "informational", "note": "informational",
    "observation": "informational", "fyi": "informational",
}

BASIS_ALIASES = {
    "file_inspection": "direct_file_inspection",
    "inspection": "direct_file_inspection",
    "verified": "fresh_verification",
    "verification": "fresh_verification",
    "test_verification": "fresh_verification",
    "history": "git_history",
    "git": "git_history",
    "inferred": "inference",
    "reasoning": "inference",
}

LAYER_ALIASES = {
    "commit": "committed",
    "stage": "staged",
    "unstage": "unstaged",
    "working": "unstaged",
    "working_tree": "unstaged",
    "worktree": "unstaged",
}

CONFIDENCE_ALIASES = {
    "h": "high",
    "m": "medium",
    "l": "low",
}

# Known repo-root directories for absolute path stripping
REPO_ROOT_DIRS = {
    "compiler", "tests", "library", "diagnostics", "scripts", "docs",
    ".claude", ".gemini", "plans", "target",
}

TITLE_MAX_LENGTH = 200


def repair_envelope(
    envelope: dict,
    default_reviewer: str = "gemini",
    default_skill: str = "review-work",
) -> tuple[dict, list[str]]:
    """Repair common schema violations in a parsed envelope dict.

    Args:
        envelope: The raw parsed JSON dict from the reviewer.
        default_reviewer: Reviewer name to inject if missing ("codex" or "gemini").
        default_skill: Skill name to inject if missing.

    Returns:
        (repaired_envelope, list_of_repair_descriptions).
        The envelope is modified in-place AND returned. The repair list is
        empty if no repairs were needed.
    """
    repairs: list[str] = []

    # ── Top-level required fields ──────────────────────────────────────

    # schema_version — only valid value is "1.0"
    if envelope.get("schema_version") != "1.0":
        old = envelope.get("schema_version")
        envelope["schema_version"] = "1.0"
        if old is None:
            repairs.append("set missing schema_version to '1.0'")
        else:
            repairs.append(f"corrected schema_version from {old!r} to '1.0'")

    # status
    if "status" not in envelope:
        envelope["status"] = "complete"
        repairs.append("set missing status to 'complete'")
    elif envelope["status"] not in VALID_STATUSES:
        old = envelope["status"]
        # Normalize common variants
        normalized = str(old).lower().strip().replace(" ", "_")
        if normalized in VALID_STATUSES:
            envelope["status"] = normalized
            repairs.append(f"normalized status from {old!r} to '{normalized}'")
        else:
            envelope["status"] = "complete"
            repairs.append(f"replaced invalid status {old!r} with 'complete'")

    # reviewer
    if "reviewer" not in envelope:
        envelope["reviewer"] = default_reviewer
        repairs.append(f"set missing reviewer to '{default_reviewer}'")
    elif envelope["reviewer"] not in ("codex", "gemini"):
        old = envelope["reviewer"]
        envelope["reviewer"] = default_reviewer
        repairs.append(f"replaced invalid reviewer {old!r} with '{default_reviewer}'")

    # skill
    if "skill" not in envelope:
        envelope["skill"] = default_skill
        repairs.append(f"set missing skill to '{default_skill}'")
    elif envelope["skill"] not in VALID_SKILLS:
        old = envelope["skill"]
        # Try normalizing (e.g., "review_work" → "review-work")
        normalized = str(old).lower().strip().replace("_", "-")
        if normalized in VALID_SKILLS:
            envelope["skill"] = normalized
            repairs.append(f"normalized skill from {old!r} to '{normalized}'")
        else:
            envelope["skill"] = default_skill
            repairs.append(f"replaced invalid skill {old!r} with '{default_skill}'")

    # ── findings array ─────────────────────────────────────────────────

    if "findings" not in envelope:
        envelope["findings"] = []
        repairs.append("set missing findings to []")
    elif envelope["findings"] is None:
        envelope["findings"] = []
        repairs.append("converted null findings to []")
    elif isinstance(envelope["findings"], dict):
        envelope["findings"] = [envelope["findings"]]
        repairs.append("wrapped single finding dict in array")
    elif not isinstance(envelope["findings"], list):
        envelope["findings"] = []
        repairs.append(
            f"replaced non-array findings ({type(envelope['findings']).__name__}) with []"
        )

    findings = envelope["findings"]

    # Filter out non-dict findings (stray strings, numbers, nulls in array)
    non_dict_count = sum(1 for f in findings if not isinstance(f, dict))
    if non_dict_count:
        findings = [f for f in findings if isinstance(f, dict)]
        envelope["findings"] = findings
        repairs.append(f"removed {non_dict_count} non-dict finding(s) from array")

    # ── no_findings ────────────────────────────────────────────────────

    if "no_findings" not in envelope:
        envelope["no_findings"] = len(findings) == 0
        repairs.append(f"set missing no_findings to {envelope['no_findings']}")
    elif not isinstance(envelope["no_findings"], bool):
        envelope["no_findings"] = len(findings) == 0
        repairs.append(f"coerced non-bool no_findings to {envelope['no_findings']}")

    # Fix inconsistency between no_findings flag and findings array
    if envelope["no_findings"] and findings:
        envelope["no_findings"] = False
        repairs.append("set no_findings to false (findings array is non-empty)")
    elif not envelope["no_findings"] and not findings:
        envelope["no_findings"] = True
        repairs.append("set no_findings to true (findings array is empty)")

    # ── scope_actually_reviewed ────────────────────────────────────────

    _repair_scope(envelope, repairs)

    # ── Per-finding repairs ────────────────────────────────────────────

    for i, finding in enumerate(findings):
        if not isinstance(finding, dict):
            continue
        _repair_finding(i, finding, repairs)

    # ── verification (optional but normalize if present) ───────────────

    if "verification" in envelope:
        _repair_verification(envelope["verification"], repairs)

    return envelope, repairs


def _repair_scope(envelope: dict, repairs: list[str]) -> None:
    """Repair the scope_actually_reviewed object."""
    default_scope = {
        "expanded_beyond_packet": False,
        "files_read": [],
        "rules_consulted": [],
    }

    if "scope_actually_reviewed" not in envelope:
        envelope["scope_actually_reviewed"] = dict(default_scope)
        repairs.append("created missing scope_actually_reviewed with defaults")
        return

    scope = envelope["scope_actually_reviewed"]
    if not isinstance(scope, dict):
        envelope["scope_actually_reviewed"] = dict(default_scope)
        repairs.append(
            f"replaced non-dict scope_actually_reviewed "
            f"({type(scope).__name__}) with defaults"
        )
        return

    # Required sub-fields
    if "expanded_beyond_packet" not in scope:
        scope["expanded_beyond_packet"] = False
        repairs.append("set missing expanded_beyond_packet to false")
    elif not isinstance(scope["expanded_beyond_packet"], bool):
        scope["expanded_beyond_packet"] = bool(scope["expanded_beyond_packet"])
        repairs.append(
            f"coerced expanded_beyond_packet to bool: "
            f"{scope['expanded_beyond_packet']}"
        )

    if "files_read" not in scope:
        scope["files_read"] = []
        repairs.append("set missing files_read to []")
    elif not isinstance(scope["files_read"], list):
        scope["files_read"] = []
        repairs.append("replaced non-list files_read with []")

    if "rules_consulted" not in scope:
        scope["rules_consulted"] = []
        repairs.append("set missing rules_consulted to []")
    elif not isinstance(scope["rules_consulted"], list):
        scope["rules_consulted"] = []
        repairs.append("replaced non-list rules_consulted with []")

    # Conditional: expansion_reason required when expanded_beyond_packet=true
    if scope.get("expanded_beyond_packet") is True:
        reason = scope.get("expansion_reason", "")
        if not isinstance(reason, str) or not reason.strip():
            scope["expansion_reason"] = "reviewer expanded beyond starting packet"
            repairs.append("set missing expansion_reason for expanded review")

    # Optional sub-fields: normalize types if present
    if "git_range" in scope and scope["git_range"] is not None:
        if not isinstance(scope["git_range"], str):
            scope["git_range"] = None
            repairs.append("set non-string git_range to null")

    for arr_field in ("specs_consulted", "plans_consulted"):
        if arr_field in scope and not isinstance(scope[arr_field], list):
            scope[arr_field] = []
            repairs.append(f"replaced non-list {arr_field} with []")


def _repair_finding(idx: int, finding: dict, repairs: list[str]) -> None:
    """Repair a single finding dict."""
    prefix = f"findings[{idx}]"

    # ── ordinal ────────────────────────────────────────────────────────
    ordinal = finding.get("ordinal")
    if ordinal is None:
        finding["ordinal"] = idx + 1
        repairs.append(f"{prefix}: set missing ordinal to {idx + 1}")
    elif isinstance(ordinal, str):
        try:
            finding["ordinal"] = int(ordinal)
            repairs.append(f"{prefix}: coerced string ordinal to int")
        except ValueError:
            finding["ordinal"] = idx + 1
            repairs.append(
                f"{prefix}: replaced unparseable ordinal {ordinal!r} with {idx + 1}"
            )
    elif isinstance(ordinal, float):
        finding["ordinal"] = int(ordinal)
        repairs.append(f"{prefix}: coerced float ordinal to int")
    if isinstance(finding.get("ordinal"), int) and finding["ordinal"] < 1:
        finding["ordinal"] = idx + 1
        repairs.append(f"{prefix}: re-indexed non-positive ordinal to {idx + 1}")

    # ── severity ───────────────────────────────────────────────────────
    _repair_enum_field(
        finding, "severity", VALID_SEVERITIES, SEVERITY_ALIASES,
        prefix, repairs, default="medium",
    )

    # ── basis ──────────────────────────────────────────────────────────
    _repair_enum_field(
        finding, "basis", VALID_BASES, BASIS_ALIASES,
        prefix, repairs, default="inference",
    )

    # ── layer (optional but normalize if present) ──────────────────────
    if "layer" in finding:
        _repair_enum_field(
            finding, "layer", VALID_LAYERS, LAYER_ALIASES,
            prefix, repairs,
        )

    # ── confidence (optional but normalize if present) ─────────────────
    if "confidence" in finding:
        _repair_enum_field(
            finding, "confidence", VALID_CONFIDENCES, CONFIDENCE_ALIASES,
            prefix, repairs,
        )

    # ── location ───────────────────────────────────────────────────────
    _repair_location(finding, prefix, repairs)

    # ── title ──────────────────────────────────────────────────────────
    _repair_title(finding, prefix, repairs)

    # ── Required string fields (evidence, impact) ──────────────────────
    for field in ("evidence", "impact"):
        if field not in finding:
            finding[field] = ""
            repairs.append(f"{prefix}: set missing {field} to empty string")
        elif not isinstance(finding[field], str):
            finding[field] = str(finding[field])
            repairs.append(f"{prefix}: coerced {field} to string")

    # ── citations (optional array) ─────────────────────────────────────
    _repair_citations(finding, prefix, repairs)


def _repair_enum_field(
    obj: dict,
    field: str,
    valid: set[str],
    aliases: dict[str, str],
    prefix: str,
    repairs: list[str],
    default: Optional[str] = None,
) -> None:
    """Normalize an enum field via alias lookup."""
    val = obj.get(field)
    if val is None and default is not None:
        obj[field] = default
        repairs.append(f"{prefix}: set missing {field} to '{default}'")
        return
    if not isinstance(val, str):
        if default is not None:
            obj[field] = default
            repairs.append(
                f"{prefix}: replaced non-string {field} ({type(val).__name__}) "
                f"with '{default}'"
            )
        return

    # Already valid
    if val in valid:
        return

    # Try case-insensitive match
    lower = val.lower().strip()
    if lower in valid:
        obj[field] = lower
        repairs.append(f"{prefix}: lowercased {field} from '{val}' to '{lower}'")
        return

    # Try alias map
    normalized = lower.replace("-", "_").replace(" ", "_")
    if normalized in aliases:
        obj[field] = aliases[normalized]
        repairs.append(
            f"{prefix}: normalized {field} from '{val}' to '{aliases[normalized]}'"
        )
        return

    if lower in aliases:
        obj[field] = aliases[lower]
        repairs.append(
            f"{prefix}: normalized {field} from '{val}' to '{aliases[lower]}'"
        )
        return

    # No match — use default if available
    if default is not None:
        obj[field] = default
        repairs.append(
            f"{prefix}: replaced unrecognized {field} '{val}' with '{default}'"
        )


def _repair_location(finding: dict, prefix: str, repairs: list[str]) -> None:
    """Repair the location field to match the invariant regex."""
    loc = finding.get("location", "")
    if not isinstance(loc, str) or not loc.strip():
        finding["location"] = "unknown:1"
        repairs.append(f"{prefix}: set missing/empty location to 'unknown:1'")
        return

    loc = loc.strip()
    original = loc

    # Strip ./ prefix
    if loc.startswith("./"):
        loc = loc[2:]

    # Strip absolute path — try to find a known repo-root directory
    if loc.startswith("/"):
        parts = loc.split("/")
        found = False
        for j, part in enumerate(parts):
            if part in REPO_ROOT_DIRS:
                loc = "/".join(parts[j:])
                found = True
                break
        if not found:
            # Last resort: strip everything up to the last known-looking path
            loc = loc.lstrip("/")

    # Handle line ranges like ":10-20" → ":10"
    line_range_re = re.compile(r":(\d+)-\d+$")
    m = line_range_re.search(loc)
    if m:
        loc = loc[:m.start()] + ":" + m.group(1)

    # Add :1 if no line number
    if ":" not in loc:
        loc = loc + ":1"

    # Ensure the part after the last colon is a number
    parts = loc.rsplit(":", 1)
    if len(parts) == 2 and not parts[1].isdigit():
        loc = parts[0] + ":1"

    if loc != original:
        finding["location"] = loc
        repairs.append(f"{prefix}: repaired location from '{original}' to '{loc}'")

    # Final regex guard: if location still doesn't match the invariant regex
    # after all structural repairs, sanitize aggressively. A cosmetic location
    # format issue is not worth a 10+ minute full-review retry.
    final_loc = finding.get("location", "unknown:1")
    if not LOCATION_RE.match(final_loc):
        # Split on the last colon to preserve the path:line structure,
        # then sanitize only the path portion.
        if ':' in final_loc:
            path_part, line_part = final_loc.rsplit(':', 1)
            if not line_part.isdigit():
                path_part = final_loc
                line_part = '1'
        else:
            path_part = final_loc
            line_part = '1'
        sanitized_path = re.sub(r'[^a-zA-Z0-9_./-]', '_', path_part)
        sanitized_path = sanitized_path.lstrip('/')
        if sanitized_path.startswith('./'):
            sanitized_path = sanitized_path[2:]
        sanitized = f"{sanitized_path}:{line_part}" if sanitized_path else "unknown:1"
        if not LOCATION_RE.match(sanitized):
            sanitized = 'unknown:1'
        finding["location"] = sanitized
        repairs.append(f"{prefix}: sanitized location to '{sanitized}' (invariant regex)")


def _repair_title(finding: dict, prefix: str, repairs: list[str]) -> None:
    """Repair the title field (trailing punctuation, length)."""
    title = finding.get("title", "")
    if not isinstance(title, str):
        finding["title"] = str(title) if title else ""
        repairs.append(f"{prefix}: coerced title to string")
        title = finding["title"]

    original = title

    # Strip trailing punctuation (period, semicolon, colon)
    if title and title[-1] in ".;:":
        title = title.rstrip(".;:")

    # Truncate if too long
    if len(title) > TITLE_MAX_LENGTH:
        title = title[:TITLE_MAX_LENGTH - 3] + "..."

    if title != original:
        finding["title"] = title
        repairs.append(f"{prefix}: cleaned title (was {len(original)} chars)")


def _repair_citations(finding: dict, prefix: str, repairs: list[str]) -> None:
    """Repair the optional citations array."""
    citations = finding.get("citations")
    if citations is None:
        return  # Optional — absence is fine

    if not isinstance(citations, list):
        finding["citations"] = []
        repairs.append(f"{prefix}: replaced non-list citations with []")
        return

    for c_idx, cit in enumerate(citations):
        if not isinstance(cit, dict):
            continue
        url = cit.get("url", "")
        if not isinstance(url, str):
            cit["url"] = str(url) if url else ""
            repairs.append(
                f"{prefix}.citations[{c_idx}]: coerced url to string"
            )
            url = cit["url"]

        # Add https:// if it looks like a domain but missing scheme
        if url and not re.match(r"^[a-zA-Z][a-zA-Z0-9+.\-]*://", url):
            if "." in url and ("/" in url or url.startswith("www.")):
                cit["url"] = "https://" + url
                repairs.append(
                    f"{prefix}.citations[{c_idx}]: added https:// prefix to URL"
                )

    # Post-repair filter: remove citations whose URLs still don't match the
    # URI invariant regex. Better to lose a citation than kill a review.
    all_citations = finding.get("citations", [])
    if all_citations:
        cleaned = []
        removed_count = 0
        for cit in all_citations:
            if not isinstance(cit, dict):
                removed_count += 1
                continue
            cit_url = cit.get("url", "")
            if isinstance(cit_url, str) and URI_RE.match(cit_url):
                cleaned.append(cit)
            else:
                removed_count += 1
        if removed_count:
            finding["citations"] = cleaned
            repairs.append(
                f"{prefix}: removed {removed_count} citation(s) with invalid URLs"
            )


def _repair_verification(verification: dict, repairs: list[str]) -> None:
    """Normalize the optional verification object."""
    if not isinstance(verification, dict):
        return

    for arr_field in ("tests_rerun", "diagnostics_run", "verification_gaps"):
        if arr_field in verification and not isinstance(verification[arr_field], list):
            verification[arr_field] = []
            repairs.append(f"verification: replaced non-list {arr_field} with []")
