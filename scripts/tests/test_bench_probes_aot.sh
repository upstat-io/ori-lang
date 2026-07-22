#!/usr/bin/env bash
# Pins the generic bench harness's AOT lane: the compiler binary and native
# runtime static archive are captured into every record, form the link-cache key
# (so a changed runtime forces a relink), and a stale, absent, or mid-run-drifted
# identity yields an INVALID RUN rather than a reading or a bench failure.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

mkdir -p "$WORK/scripts/bench_probes" "$WORK/bin"
for f in measure.sh report.py characterize.py provenance.py; do
    cp "$ROOT/scripts/bench_probes/$f" "$WORK/scripts/bench_probes/"
done
MEASURE="$WORK/scripts/bench_probes/measure.sh"
REGISTRY="$WORK/scripts/bench_probes/registry.json"

# Comparator stubs shared with the base harness self-test: `agree` reproduces the
# subject's output, `work_same` its work counters.
printf '#!/bin/sh\nprintf "540000\\n"\n' > "$WORK/bin/agree"
printf '#!/bin/sh\nprintf "work calls 500\\nwork toggles 241500\\n"\n' > "$WORK/bin/work_same"
chmod +x "$WORK/bin/agree" "$WORK/bin/work_same"
export PATH="$WORK/bin:$PATH"

printf '{"subjects":{},"noise_threshold_pct":5.0,"min_profile_distance":0.2}\n' > "$REGISTRY"

# 10. AOT lane: compiler + native-runtime archive identity is recorded in every
#     record, is the link-cache key (so a changed runtime forces a relink), and a
#     stale / absent / mid-run-drifted identity is an INVALID RUN (exit 4) --
#     never a reading and never a bench failure (exit 1).
AOT_TARGET="$WORK/target/release"
mkdir -p "$AOT_TARGET"
ARCHIVE="$AOT_TARGET/libori_rt.a"
LINK_LOG="$WORK/link.log"
: > "$LINK_LOG"
printf 'rt-v1\n' > "$ARCHIVE"
printf 'main\n' > "$WORK/prog.ori"
printf 'main\n' > "$WORK/drift.ori"

# The harness owns the AOT build; a stub cargo keeps the pin off a real toolchain.
printf '#!/bin/sh\nexit 0\n' > "$WORK/bin/cargo"
# Stub compiler: records each link and emits a runnable program. A `drift.ori`
# program mutates the runtime archive WHILE measured -- the concurrent-rebuild case.
cat > "$AOT_TARGET/ori" <<EOF
#!/bin/sh
echo "link \$*" >> "$LINK_LOG"
out=""; prev=""
for a in "\$@"; do
    [ "\$prev" = "-o" ] && out="\$a"
    prev="\$a"
done
case "\$2" in
    *drift.ori) printf '#!/bin/sh\nprintf "540000\\\\n"\nprintf "drifted" >> "%s"\n' "$ARCHIVE" > "\$out" ;;
    *) printf '#!/bin/sh\nprintf "540000\\\\n"\n' > "\$out" ;;
esac
chmod +x "\$out"
EOF
chmod +x "$WORK/bin/cargo" "$AOT_TARGET/ori"

python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
reg["subjects"]["aotprog"] = {
    "kind": "program-wallclock", "samples": 1,
    "aot": {"profile": "release", "source": "prog.ori"},
    "external_baselines": {
        "agreeing": {"role": "floor", "max_ratio": 1.0, "command": "agree",
                     "work_count": {"command": "work_same"}},
    },
}
reg["subjects"]["aotdrift"] = {
    "kind": "program-wallclock", "samples": 1,
    "aot": {"profile": "release", "source": "drift.ori"},
}
json.dump(reg, open(sys.argv[1], "w"))
PY

aot_key() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['aot']['cache_key'])" "$1"; }

# POSITIVE: identity captured into the record; first run links.
"$MEASURE" aotprog --samples-json "$WORK/aot1.json" >/dev/null 2>"$WORK/aot1.err" \
    || { cat "$WORK/aot1.err" >&2; fail "aot probe exited non-zero on a valid identity"; }
python3 - "$WORK/aot1.json" "$ARCHIVE" <<'PY' || fail "aot record missing runtime identity"
import hashlib, json, sys
rec = json.load(open(sys.argv[1]))
aot = rec["aot"]
assert aot["verdict"] == "valid", aot
assert aot["relinked"] is True, aot
runtime = aot["identity"]["runtime"]
assert runtime["present"] is True, runtime
assert runtime["path"].endswith("libori_rt.a"), runtime
assert runtime["sha256"] == hashlib.sha256(open(sys.argv[2], "rb").read()).hexdigest(), runtime
for part in ("compiler", "source"):
    assert aot["identity"][part]["sha256"], aot["identity"][part]
assert aot["cache_key"] and len(aot["cache_key"]) == 64, aot
PY
[ "$(wc -l < "$LINK_LOG")" = "1" ] || fail "first aot run should link exactly once"
KEY1="$(aot_key "$WORK/aot1.json")"

# POSITIVE: an unchanged identity reuses the cached link (no relink).
"$MEASURE" aotprog --samples-json "$WORK/aot2.json" >/dev/null 2>&1 \
    || fail "aot probe exited non-zero on a cached identity"
python3 -c "import json,sys; a=json.load(open(sys.argv[1]))['aot']; assert a['relinked'] is False, a" \
    "$WORK/aot2.json" || fail "unchanged identity must reuse the cached link"
[ "$(wc -l < "$LINK_LOG")" = "1" ] || fail "unchanged identity must not relink"
[ "$(aot_key "$WORK/aot2.json")" = "$KEY1" ] || fail "unchanged identity changed the cache key"

# FORCE RELINK: a changed runtime archive yields a new key and a fresh link.
printf 'rt-v2\n' > "$ARCHIVE"
"$MEASURE" aotprog --samples-json "$WORK/aot3.json" >/dev/null 2>&1 \
    || fail "aot probe exited non-zero after a runtime-archive change"
python3 -c "import json,sys; a=json.load(open(sys.argv[1]))['aot']; assert a['relinked'] is True, a" \
    "$WORK/aot3.json" || fail "a changed runtime archive must force a relink"
[ "$(wc -l < "$LINK_LOG")" = "2" ] || fail "a changed runtime archive must relink exactly once more"
KEY3="$(aot_key "$WORK/aot3.json")"
[ "$KEY3" != "$KEY1" ] || fail "a changed runtime archive must change the cache key"

# NEGATIVE (the teeth): a cached link whose recorded runtime identity disagrees
# with the archive on disk is an INVALID RUN, not a reading and not a failure.
CACHE_ENTRY="$WORK/build/bench-aot-cache/$KEY3"
python3 - "$CACHE_ENTRY/aot-identity.json" <<'PY'
import json, sys
rec = json.load(open(sys.argv[1]))
rec["runtime"]["sha256"] = "0" * 64
json.dump(rec, open(sys.argv[1], "w"))
PY
set +e
"$MEASURE" aotprog >/dev/null 2>"$WORK/stale.err"
STALE_RC=$?
set -e
[ "$STALE_RC" = "4" ] \
    || fail "a stale cached link must exit 4 (invalid run), got $STALE_RC"
grep -q "invalid run" "$WORK/stale.err" \
    || fail "stale-cache exit must name the invalid run: $(cat "$WORK/stale.err")"
grep -q "different runtime" "$WORK/stale.err" \
    || fail "stale-cache reason must name the runtime archive: $(cat "$WORK/stale.err")"

# MUTATION CHECK: neuter the staleness comparison and the negative pin must fail.
cp "$WORK/scripts/bench_probes/provenance.py" "$WORK/provenance.orig.py"
python3 - "$WORK/scripts/bench_probes/provenance.py" <<'PY'
import re, sys
path = sys.argv[1]
src = open(path).read()
start = src.index("def stale_cache_reason(")
end = src.index("\ndef ", start + 1) if "\ndef " in src[start + 1:] else len(src)
src = src[:start] + "def stale_cache_reason(recorded, current, executable):\n    return None\n" + src[end:]
open(path, "w").write(src)
PY
set +e
"$MEASURE" aotprog >/dev/null 2>&1
MUTANT_RC=$?
set -e
cp "$WORK/provenance.orig.py" "$WORK/scripts/bench_probes/provenance.py"
find "$WORK/scripts/bench_probes" -name '__pycache__' -type d -exec rm -rf {} + 2>/dev/null || true
[ "$MUTANT_RC" != "4" ] \
    || fail "mutation check: neutering stale_cache_reason still reported an invalid run"
set +e
"$MEASURE" aotprog >/dev/null 2>&1
RESTORED_RC=$?
set -e
[ "$RESTORED_RC" = "4" ] \
    || fail "mutation check: restoring stale_cache_reason lost the invalid-run verdict"

# NEGATIVE: an absent runtime archive is an INVALID RUN, never a pass.
rm -f "$CACHE_ENTRY/aot-identity.json"
mv "$ARCHIVE" "$WORK/archive.bak"
set +e
"$MEASURE" aotprog >/dev/null 2>"$WORK/absent.err"
ABSENT_RC=$?
set -e
mv "$WORK/archive.bak" "$ARCHIVE"
[ "$ABSENT_RC" = "4" ] || fail "an absent runtime archive must exit 4, got $ABSENT_RC"
grep -q "artifact absent" "$WORK/absent.err" \
    || fail "absent-archive reason must name the artifact: $(cat "$WORK/absent.err")"

# NEGATIVE: a runtime archive rewritten WHILE the program is measured (the
# concurrent-rebuild case) invalidates the run rather than yielding a reading.
set +e
"$MEASURE" aotdrift >/dev/null 2>"$WORK/drift.err"
DRIFT_RC=$?
set -e
printf 'rt-v2\n' > "$ARCHIVE"
[ "$DRIFT_RC" = "4" ] || fail "a mid-run runtime-archive change must exit 4, got $DRIFT_RC"
grep -q "identity changed during the run" "$WORK/drift.err" \
    || fail "mid-run drift must be reported as such: $(cat "$WORK/drift.err")"

# A row declaring `aot` alongside `command`/`build` is rejected: the harness owns
# the AOT build and link, so a second declaration would be silently ignored.
python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
reg["subjects"]["aot_conflict"] = {
    "kind": "program-wallclock", "samples": 1, "command": "agree",
    "aot": {"profile": "release", "source": "prog.ori"},
}
reg["subjects"]["aot_nosource"] = {
    "kind": "program-wallclock", "samples": 1, "aot": {"profile": "release"},
}
json.dump(reg, open(sys.argv[1], "w"))
PY
if "$MEASURE" aot_conflict >/dev/null 2>&1; then
    fail "a row declaring both aot and command must be rejected"
fi
if "$MEASURE" aot_nosource >/dev/null 2>&1; then
    fail "an aot block with no source must be rejected"
fi
python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
for name in ("aot_conflict", "aot_nosource"):
    reg["subjects"].pop(name, None)
json.dump(reg, open(sys.argv[1], "w"))
PY

# Both AOT profiles are addressed: the explicit runtime-archive build precedes
# the compiler build in each, and each resolves its own target directory.
python3 - "$WORK/scripts/bench_probes" <<'PY' || fail "aot profile handling did not hold"
import sys
sys.path.insert(0, sys.argv[1])
import provenance as p

debug, release = p.build_commands("debug"), p.build_commands("release")
for cmds in (debug, release):
    assert len(cmds) == 2, cmds
    assert "-p ori_rt" in cmds[0], cmds
    assert "-p oric" in cmds[1], cmds
assert all("--release" not in c for c in debug), debug
assert all("--release" in c for c in release), release
assert p.profile_dir("/repo", {"profile": "debug"}).as_posix() == "/repo/target/debug"
assert p.profile_dir("/repo", {"profile": "release"}).as_posix() == "/repo/target/release"
try:
    p.profile_of({"profile": "fast"})
except p.AotSpecError:
    pass
else:
    raise AssertionError("an unknown aot.profile must be rejected")
PY

# The report classifies an invalid run as INCONCLUSIVE with no ratios: neither a
# comparator pass nor a comparator failure.
mv "$ARCHIVE" "$WORK/archive.bak"
python3 "$WORK/scripts/bench_probes/report.py" --repo-root "$WORK" --subject aotprog --json \
    > "$WORK/aot-report.json" 2>/dev/null || fail "report.py exited non-zero on an invalid run"
mv "$WORK/archive.bak" "$ARCHIVE"
python3 - "$WORK/aot-report.json" <<'PY' || fail "report did not classify the invalid run"
import json, sys
subject = json.load(open(sys.argv[1]))["subjects"][0]
assert subject["invalid_run"], subject
assert "artifact absent" in subject["invalid_run"], subject
for label, entry in subject["comparators"].items():
    assert entry["verdict"] == "inconclusive", (label, entry)
    assert entry["invalid_run"], (label, entry)
    assert "ratios" not in entry, (label, entry)
PY

printf 'All AOT-lane checks passed.\n'
