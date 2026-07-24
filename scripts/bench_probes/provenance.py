#!/usr/bin/env python3
"""Run provenance and artifact identity for bench records.

Owns what a reading was produced BY: the host environment, the harness and
registry hashes, comparator executable resolution, and -- the load-bearing case
-- the compiler binary plus native-runtime static archive an AOT row links
against (BM-V01/BM-V02/BM-V08/BM-V10).

A `program-wallclock` registry row (subject or comparator) may declare an `aot`
block instead of a `command`:

    "aot": {"profile": "release",
            "source": "<repo-relative program>",
            "args": "<optional argv tail>",
            "cache_dir": "build/bench-aot-cache"}

The harness then owns the whole AOT lane for that row: it builds the runtime
static archive and the compiler binary explicitly, records each artifact's
canonical path and content hash, links the program into a cache directory keyed
by those hashes, and re-verifies the identity after the measured runs.

An identity that is missing, drifted, or disagrees with a cached link is an
INVALID RUN (`INVALID_RUN_EXIT`) -- never a measurement pass and never a backend
failure (BM-V01/BM-V02/BM-V10).

Public surface: `environment`, `sha256_file`, `executable_of`,
`resolve_executable`, `build_commands`, `capture`, `missing_reason`,
`drift_reason`, `cache_key`, `link_dir`, `executable_path`,
`read_recorded_identity`, `write_recorded_identity`, `stale_cache_reason`,
`invalid_run_reason`, `render_aot`, `INVALID_RUN_EXIT`.
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

#: Exit status for an invalid run. Distinct from 1 (real bench failure),
#: 2 (IO/parse error), and 3 (unregistered subject / bad mode).
INVALID_RUN_EXIT = 4

#: Cargo packages that must both be current before an AOT comparison. Building
#: the compiler alone leaves the runtime archive stale.
RUNTIME_PACKAGE = "ori_rt"
COMPILER_PACKAGE = "oric"
COMPILER_BIN = "ori"

PROFILES = ("debug", "release")
DEFAULT_PROFILE = "release"
DEFAULT_CACHE_DIR = "build/bench-aot-cache"

#: Sidecar file recording the identity a cached executable was linked from.
CACHE_IDENTITY_FILE = "aot-identity.json"

#: Prefix bound into the cache key; a change here invalidates every cache entry.
KEY_SCHEMA = "aot-identity-v1"

#: The artifacts whose identity forms the key and the staleness verdict.
IDENTITY_PARTS = ("compiler", "runtime", "source")


class AotSpecError(ValueError):
    """Raised for a malformed `aot` block."""


def profile_of(spec: dict) -> str:
    profile = spec.get("profile", DEFAULT_PROFILE)
    if profile not in PROFILES:
        raise AotSpecError(f"aot.profile must be one of {PROFILES}, got {profile!r}")
    return profile


def runtime_archive_name() -> str:
    return "ori_rt.lib" if os.name == "nt" else f"lib{RUNTIME_PACKAGE}.a"


def compiler_name() -> str:
    return f"{COMPILER_BIN}.exe" if os.name == "nt" else COMPILER_BIN


def build_commands(profile: str) -> list[str]:
    """The explicit builds an AOT comparison requires, runtime archive first."""
    release = " --release" if profile == "release" else ""
    return [
        f"cargo build{release} -q -p {RUNTIME_PACKAGE}",
        f"cargo build{release} -q -p {COMPILER_PACKAGE} --bin {COMPILER_BIN}",
    ]


def profile_dir(repo_root: Path, spec: dict) -> Path:
    return Path(repo_root) / "target" / profile_of(spec)


def artifact_identity(path: Path) -> dict:
    """Canonical path plus content hash and byte size of one artifact."""
    path = Path(path)
    record = {"path": str(path.resolve() if path.exists() else path.absolute())}
    try:
        payload = path.read_bytes()
    except OSError:
        record["present"] = False
        return record
    record["present"] = True
    record["sha256"] = hashlib.sha256(payload).hexdigest()
    record["size"] = len(payload)
    return record


def capture(repo_root: Path, spec: dict) -> dict:
    """Identity of the compiler binary, the runtime archive, and the program."""
    source = spec.get("source")
    if not source:
        raise AotSpecError("aot block declares no source")
    built = profile_dir(repo_root, spec)
    return {
        "profile": profile_of(spec),
        "compiler": artifact_identity(built / compiler_name()),
        "runtime": artifact_identity(built / runtime_archive_name()),
        "source": artifact_identity(Path(repo_root) / source),
    }


def missing_reason(identity: dict) -> str | None:
    """Why the captured identity is unusable, or None when every part is present."""
    absent = [part for part in IDENTITY_PARTS if not identity[part].get("present")]
    if not absent:
        return None
    paths = ", ".join(f"{part}={identity[part]['path']}" for part in absent)
    return f"artifact absent: {paths}"


def drift_reason(before: dict, after: dict) -> str | None:
    """Why the identity changed across the measured runs, or None when stable."""
    for part in IDENTITY_PARTS:
        old, new = before[part], after[part]
        if old.get("sha256") != new.get("sha256") or old.get("present") != new.get("present"):
            return (
                f"{part} identity changed during the run: {old['path']} "
                f"{old.get('sha256', 'absent')} -> {new.get('sha256', 'absent')}"
            )
    return None


def cache_key(identity: dict) -> str:
    """Link-cache key over the profile and every artifact hash."""
    parts = [KEY_SCHEMA, identity["profile"]]
    parts += [identity[part].get("sha256", "absent") for part in IDENTITY_PARTS]
    return hashlib.sha256("\n".join(parts).encode("utf-8")).hexdigest()


def link_dir(repo_root: Path, spec: dict, key: str) -> Path:
    return Path(repo_root) / spec.get("cache_dir", DEFAULT_CACHE_DIR) / key


def executable_path(repo_root: Path, spec: dict, key: str) -> Path:
    stem = Path(spec["source"]).stem or "program"
    return link_dir(repo_root, spec, key) / stem


def read_recorded_identity(directory: Path) -> dict | None:
    """The identity a cached executable was linked from, or None when unrecorded."""
    try:
        return json.loads((Path(directory) / CACHE_IDENTITY_FILE).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def write_recorded_identity(directory: Path, identity: dict) -> None:
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / CACHE_IDENTITY_FILE).write_text(
        json.dumps(identity, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def stale_cache_reason(recorded: dict, current: dict, executable: Path) -> str | None:
    """Why a cached link may not be reused, or None when it matches `current`."""
    if not Path(executable).exists():
        return f"cached executable missing: {executable}"
    if recorded.get("profile") != current["profile"]:
        return (
            f"cached link profile {recorded.get('profile')!r} != "
            f"current profile {current['profile']!r}"
        )
    for part in IDENTITY_PARTS:
        was = (recorded.get(part) or {}).get("sha256")
        now = current[part].get("sha256")
        if was != now:
            return (
                f"cached link was produced from a different {part}: "
                f"{was or 'unrecorded'} != {now}"
            )
    return None


def sha256_file(path: Path) -> str | None:
    """Content hash of a harness input file, or None when unreadable."""
    try:
        return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    except OSError:
        return None


def executable_of(command: str) -> str | None:
    parts = shlex.split(command)
    return parts[0] if parts else None


def resolve_executable(command: str) -> str | None:
    name = executable_of(command)
    return shutil.which(name) if name else None


def environment(repo_root: Path, harness: Path, registry: Path) -> dict:
    """Host, revision, and harness-input identities needed to reproduce a run."""

    def git(*args: str) -> str | None:
        try:
            out = subprocess.run(
                ["git", "-C", str(repo_root), *args], capture_output=True, text=True, check=False
            )
        except OSError:
            return None
        return out.stdout.strip() or None

    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "cpu_count": os.cpu_count(),
        "python": sys.version.split()[0],
        "repo_root": str(repo_root),
        "head_sha": git("rev-parse", "HEAD"),
        "tree_dirty": bool(git("status", "--porcelain")),
        "harness_sha256": sha256_file(harness),
        "registry_sha256": sha256_file(registry),
    }


def invalid_run_reason(probes: dict) -> str | None:
    """The first invalid-run reason across a probe set, or None when all ran validly.

    An invalid run means a stale, absent, or drifted compiler/runtime/artifact
    identity: the readings describe no admissible treatment, so they may become
    neither a met nor a missed verdict.
    """
    for _metric, probe in sorted(probes.items()):
        reason = probe.get("invalid_run")
        if reason:
            return reason
    return None


def render_aot(aot: dict | None, invalid_run: str | None) -> list[str]:
    """The AOT lane's artifact identities, or its invalid-run verdict."""
    if invalid_run:
        return [f"  aot   INVALID RUN: {invalid_run}"]
    if not aot:
        return []
    identity = aot.get("identity") or {}
    lines = [
        f"  aot   profile={identity.get('profile')} key={aot.get('cache_key')} "
        f"relinked={aot.get('relinked')}"
    ]
    for part in IDENTITY_PARTS:
        record = identity.get(part) or {}
        lines.append(f"    {part:<9} {record.get('sha256')} {record.get('path')}")
    return lines
