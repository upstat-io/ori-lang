"""Locality shipped-conformance verdict-boundary tests.

Per the locality shipped-conformance cross-walk:
5 explicit fixture classes the probe tuple distinguishes, replacing OR-shape
negative pin with explicit per-shape coverage.

Shipped-state ground truth: actual current shipped state is `(4, True)` because
`compiler/ori_arc/src/aims/contract/mod.rs:269` retains
`pub may_escape: bool` as a stored struct field.

  fixture_4_variant_stored (POSITIVE — current shipped state) (4, True)
      -> all manifest rules emit divergent-pending, exit 0,
         structured verdict divergent_pending
  fixture_4_variant_derived (NEGATIVE — partial-migration intermediate:
                              may_escape converted before 5th variant lands)
                              (4, False)
      -> all manifest rules emit divergent-pending, exit 0
  fixture_5_variant_stored (NEGATIVE — partial-migration intermediate:
                              5th variant added before may_escape converted)
                              (5, True)
      -> all manifest rules emit divergent-pending, exit 0
  fixture_5_variant_derived (POSITIVE — post-locality-work target) (5, False)
      -> all manifest rules emit pass, exit 0,
         structured verdict pass
  fixture_manifest_free_divergence (NEGATIVE — rule path drift outside manifest)
      -> cross-walk emits fatal divergent, exit non-zero,
         structured verdict fatal_divergent

Mock-injection per SC7: tests inject synthetic probe tuples via
conftest.mock_shipped_lattice_probe fixture (env-var override
ORI_LOCALITY_PROBE_*) — no dependency on the unified-locality dimension
landing first.

PLUS Tier-1 real-probe integration test `test_real_probe_matches_shipped_state`
(per the locality cross-walk SC8 / claude-ds-F3 cure): runs the locality-conformance binary
WITHOUT env-var overrides, exercising the real build.rs syn-AST parse path
against the actual compiler tree; asserts exit 0 + verdict `divergent_pending`
+ probe_tuple matches `(4, True)`.
"""

from __future__ import annotations

import json
from pathlib import Path


MANIFEST_RULE_IDS = {"RL-15a", "RL-18a", "DP-7", "DP-8", "CN-6", "CN-8", "IC-3"}


def _parse_envelope(stdout: str) -> dict:
    """Parse the locality-conformance binary's JSON envelope from stdout."""
    return json.loads(stdout)


def test_manifest_authored_with_expected_rule_set(workspace_root: Path) -> None:
    """Sanity: the manifest enumerates exactly the 7 locality-blocked rules."""
    manifest_path = workspace_root / "aims-proof/checker/shipped-conformance-manifest.json"
    with open(manifest_path) as f:
        manifest = json.load(f)
    rule_ids = {r["id"] for r in manifest["rules"]}
    assert rule_ids == MANIFEST_RULE_IDS, (
        f"manifest rule set drift: expected {MANIFEST_RULE_IDS}, got {rule_ids}"
    )


def test_manifest_validates_against_schema(workspace_root: Path) -> None:
    """SC3: manifest conforms to shipped-conformance-manifest.schema.json."""
    import importlib.util

    if importlib.util.find_spec("jsonschema") is None:
        import pytest

        pytest.skip("jsonschema not installed; structural validation skipped")
    import jsonschema # noqa: I001

    manifest_path = workspace_root / "aims-proof/checker/shipped-conformance-manifest.json"
    schema_path = workspace_root / "aims-proof/checker/shipped-conformance-manifest.schema.json"
    with open(manifest_path) as f:
        manifest = json.load(f)
    with open(schema_path) as f:
        schema = json.load(f)
    jsonschema.validate(instance=manifest, schema=schema)


def test_fixture_4_variant_stored_emits_divergent_pending(mock_shipped_lattice_probe) -> None:
    """POSITIVE: current shipped state (4, True) -> divergent-pending, exit 0.

    Actual current shipped state per `compiler/ori_arc/src/aims/
    contract/mod.rs:269` retaining `pub may_escape: bool` as a stored struct
    field. Every manifest rule emits `divergent-pending(blocked-by:
    unified-locality-dimension)` under this probe tuple.
    """
    result = mock_shipped_lattice_probe(4, True)
    assert result.returncode == 0, f"expected exit 0, got {result.returncode}: {result.stdout}"
    env = _parse_envelope(result.stdout)
    assert env["verdict"] == "divergent_pending"
    assert env["exit_reason"] == "locality_conformance_divergent_pending"
    assert env["autopilot_routable"] is True
    assert env["probe_tuple"]["lattice_variant_count"] == 4
    assert env["probe_tuple"]["may_escape_is_stored"] is True
    rule_ids_in_envelope = {r["rule_id"] for r in env["rule_verdicts"]}
    assert rule_ids_in_envelope == MANIFEST_RULE_IDS
    for row in env["rule_verdicts"]:
        assert row["current"].startswith("divergent-pending"), row
        assert "unified-locality-dimension" in row["current"], row


def test_fixture_4_variant_derived_emits_divergent_pending(mock_shipped_lattice_probe) -> None:
    """NEGATIVE (partial-migration intermediate): (4, False) — four Locality
    variants, may_escape DERIVED (not stored). Not current shipped state but
    a recognized partial-migration shape per the locality cross-walk SC7. All manifest rules emit
    `divergent-pending`, exit 0.
    """
    result = mock_shipped_lattice_probe(4, False)
    assert result.returncode == 0, f"expected exit 0, got {result.returncode}: {result.stdout}"
    env = _parse_envelope(result.stdout)
    assert env["verdict"] == "divergent_pending"
    assert env["exit_reason"] == "locality_conformance_divergent_pending"
    assert env["autopilot_routable"] is True
    assert env["probe_tuple"]["lattice_variant_count"] == 4
    assert env["probe_tuple"]["may_escape_is_stored"] is False
    rule_ids_in_envelope = {r["rule_id"] for r in env["rule_verdicts"]}
    assert rule_ids_in_envelope == MANIFEST_RULE_IDS
    for row in env["rule_verdicts"]:
        assert row["current"].startswith("divergent-pending"), row
        assert "unified-locality-dimension" in row["current"], row


def test_fixture_5_variant_stored_emits_divergent_pending(mock_shipped_lattice_probe) -> None:
    """NEGATIVE: intermediate (5, True) -> divergent-pending, exit 0."""
    result = mock_shipped_lattice_probe(5, True)
    assert result.returncode == 0, f"expected exit 0, got {result.returncode}: {result.stdout}"
    env = _parse_envelope(result.stdout)
    assert env["verdict"] == "divergent_pending"
    assert env["exit_reason"] == "locality_conformance_divergent_pending"
    assert env["probe_tuple"]["lattice_variant_count"] == 5
    assert env["probe_tuple"]["may_escape_is_stored"] is True
    for row in env["rule_verdicts"]:
        assert row["current"].startswith("divergent-pending"), row


def test_fixture_5_variant_derived_emits_pass(mock_shipped_lattice_probe) -> None:
    """POSITIVE: post-locality-work target (5, False) -> pass, exit 0."""
    result = mock_shipped_lattice_probe(5, False)
    assert result.returncode == 0, f"expected exit 0, got {result.returncode}: {result.stdout}"
    env = _parse_envelope(result.stdout)
    assert env["verdict"] == "pass"
    assert env["exit_reason"] == "locality_conformance_pass"
    assert env["autopilot_routable"] is True
    assert env["probe_tuple"]["lattice_variant_count"] == 5
    assert env["probe_tuple"]["may_escape_is_stored"] is False
    for row in env["rule_verdicts"]:
        assert row["current"] == "pass", row


def test_fixture_manifest_free_divergence_emits_fatal(mock_shipped_lattice_probe) -> None:
    """NEGATIVE: probe tuple outside recognized shape -> fatal divergent, exit 2.

    Simulates rule path drift outside the manifest set (e.g., a new
    ParamContract mutation that produces a non-canonical (variants, stored)
    tuple). The cross-walk treats unrecognized shapes as fatal so the cross-walk
    amends the manifest or surfaces the divergence for manual fix.
    """
    # variant_count=6 is outside the recognized {4, 5} domain.
    result = mock_shipped_lattice_probe(6, False)
    assert result.returncode == 2, f"expected exit 2, got {result.returncode}: {result.stdout}"
    env = _parse_envelope(result.stdout)
    assert env["verdict"] == "fatal_divergent"
    assert env["exit_reason"] == "locality_conformance_fatal_divergent"
    assert env["autopilot_routable"] is False
    for row in env["rule_verdicts"]:
        assert row["current"] == "divergent", row


def test_fixture_5_variant_derived_passes_for_every_manifest_rule(
    mock_shipped_lattice_probe,
) -> None:
    """SC7 strengthening: every manifest rule flips to pass when target state lands."""
    result = mock_shipped_lattice_probe(5, False)
    env = _parse_envelope(result.stdout)
    rule_ids_in_envelope = {r["rule_id"] for r in env["rule_verdicts"]}
    assert rule_ids_in_envelope == MANIFEST_RULE_IDS, (
        f"rule coverage drift: missing {MANIFEST_RULE_IDS - rule_ids_in_envelope}, "
        f"extra {rule_ids_in_envelope - MANIFEST_RULE_IDS}"
    )
    for row in env["rule_verdicts"]:
        assert row["current"] == "pass", (
            f"rule {row['rule_id']} did not flip to pass: {row['current']}"
        )


def test_real_probe_matches_shipped_state(workspace_root: Path) -> None:
    """Tier-1 integration test per the locality cross-walk SC8 (cures claude-ds-F3): runs the
    locality-conformance binary WITHOUT env-var overrides, exercising the
    real build.rs syn-AST parse path against
    `compiler/ori_arc/src/aims/lattice/dimensions.rs` +
    `compiler/ori_arc/src/aims/contract/mod.rs`.

    Asserts exit 0 + verdict `divergent_pending` +
    `probe_tuple.lattice_variant_count == 4` +
    `probe_tuple.may_escape_is_stored == True` — actual current shipped state.

    Skips cleanly if the locality-conformance binary is not built.
    """
    import os
    import subprocess

    import pytest

    binary = workspace_root / "aims-proof" / "target" / "release" / "locality-conformance"
    if not binary.exists():
        pytest.skip(
            f"checker binary not built at {binary}; "
            f"run `cargo build --release -p aims-proof-checker --bin locality-conformance` first"
        )

    # Strip ORI_LOCALITY_PROBE_* env vars so the binary exercises the real
    # build.rs-emitted probe constants vs the mock-injection env-var path.
    env = {k: v for k, v in os.environ.items() if not k.startswith("ORI_LOCALITY_PROBE_")}
    result = subprocess.run(
        [str(binary)],
        cwd=str(workspace_root / "aims-proof"),
        env=env,
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert result.returncode == 0, (
        f"locality-conformance exited {result.returncode}; "
        f"stdout={result.stdout!r}; stderr={result.stderr!r}"
    )
    envelope = _parse_envelope(result.stdout)
    assert envelope["verdict"] == "divergent_pending", (
        f"real-probe verdict drift: expected divergent_pending, got {envelope['verdict']!r}; "
        f"full envelope: {envelope!r}"
    )
    assert envelope["probe_tuple"]["lattice_variant_count"] == 4, (
        f"real-probe lattice_variant_count drift: expected 4, got "
        f"{envelope['probe_tuple']['lattice_variant_count']!r}"
    )
    assert envelope["probe_tuple"]["may_escape_is_stored"] is True, (
        f"real-probe may_escape_is_stored drift: expected True, got "
        f"{envelope['probe_tuple']['may_escape_is_stored']!r}"
    )
