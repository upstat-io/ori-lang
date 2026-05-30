"""Pytest configuration for §17 locality shipped-conformance tests.

Provides the mock_shipped_lattice_probe fixture per
the locality shipped-conformance cross-walk:
inject synthetic (lattice_variant_count, may_escape_is_stored) tuples into
the cross-walk via env-var overrides (ORI_LOCALITY_PROBE_VARIANT_COUNT +
ORI_LOCALITY_PROBE_MAY_ESCAPE_STORED), simulating shipped states (4-variant
stored, 4-variant derived, 5-variant stored, 5-variant derived) without
requiring locality-representation-unification to land first.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path
from typing import Callable, Iterator

import pytest


@pytest.fixture(scope="session")
def workspace_root() -> Path:
    here = Path(__file__).resolve()
    for parent in [here, *here.parents]:
        if (parent / "aims-proof").is_dir() and (parent / "compiler" / "ori_arc").is_dir():
            return parent
    raise RuntimeError("could not locate the repo root containing aims-proof/ and compiler/ori_arc/")


@pytest.fixture(scope="session")
def locality_conformance_bin(workspace_root: Path) -> Path:
    """Build (once) and return the path to the locality-conformance binary."""
    aims_proof = workspace_root / "aims-proof"
    bin_path = aims_proof / "target" / "release" / "locality-conformance"
    if not bin_path.exists():
        cargo = shutil.which("cargo")
        if cargo is None:
            pytest.skip("cargo not on PATH; cannot build locality-conformance")
        subprocess.run(
            [cargo, "build", "--release", "-p", "aims-proof-checker", "--bin", "locality-conformance"],
            cwd=str(aims_proof),
            check=True,
        )
    return bin_path


@pytest.fixture
def mock_shipped_lattice_probe(
    locality_conformance_bin: Path,
    workspace_root: Path,
) -> Iterator[Callable[[int, bool], subprocess.CompletedProcess]]:
    """Run the locality-conformance binary with mocked probe tuple.

    Returns a callable (variant_count, may_escape_is_stored) ->
    CompletedProcess. Sets ORI_LOCALITY_PROBE_* env vars to inject the
    synthetic shipped state.
    """

    def _run(variant_count: int, may_escape_is_stored: bool) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        env["ORI_LOCALITY_PROBE_VARIANT_COUNT"] = str(variant_count)
        env["ORI_LOCALITY_PROBE_MAY_ESCAPE_STORED"] = "1" if may_escape_is_stored else "0"
        return subprocess.run(
            [str(locality_conformance_bin)],
            cwd=str(workspace_root / "aims-proof"),
            env=env,
            capture_output=True,
            text=True,
        )

    yield _run
