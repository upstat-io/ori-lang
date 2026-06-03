#!/usr/bin/env bash
# burden-balance.sh — Measure VF-1 `verify_burden_balance` residual over a corpus.
#
# Runs `ori build`; the faithful Phase-5 burden emission is checked against the
# RL-2/RL-4/RL-5 net-zero obligation (no masking balancer — the burden path is
# the sole RC-emission path). Counts DISTINCT (file, var, exit_block)
# imbalances — the raw warn stream double-emits (Step 6 + Step 11 postprocess
# checkpoints), so a naive `grep -c` over-counts.
#
# Usage:
#   diagnostics/burden-balance.sh                 # whole tests/spec corpus, debug bin
#   diagnostics/burden-balance.sh tests/spec/x    # a path subset
#   diagnostics/burden-balance.sh --release file.ori
#   diagnostics/burden-balance.sh --files         # also list per-file counts
#   diagnostics/burden-balance.sh --raw file.ori  # dump distinct warn lines for one file
#
# Exit 0 always (measurement tool). The count is printed to stdout.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_DIR"

BUILD=debug
SHOW_FILES=0
RAW=0
TARGETS=()
for arg in "$@"; do
  case "$arg" in
    --release) BUILD=release ;;
    --files) SHOW_FILES=1 ;;
    --raw) RAW=1 ;;
    *) TARGETS+=("$arg") ;;
  esac
done

BIN="target/$BUILD/ori"
if [ ! -x "$BIN" ]; then
  echo "burden-balance: $BIN not found — run 'cargo build${BUILD:+ }${BUILD/debug/}' first" >&2
  echo "  (build the dev binary fresh BEFORE measuring; a stale binary yields false counts)" >&2
  exit 0
fi

if [ "${#TARGETS[@]}" -eq 0 ]; then
  TARGETS=(tests/spec)
fi

# Expand directory targets to .ori files; keep explicit .ori files as-is.
FILES=()
for t in "${TARGETS[@]}"; do
  if [ -d "$t" ]; then
    while IFS= read -r f; do FILES+=("$f"); done < <(find "$t" -name '*.ori')
  else
    FILES+=("$t")
  fi
done

# Distinct-imbalance extractor: pull `vN net=... at exit block M` tuples, dedup.
extract() {
  grep -oE 'burden imbalance \(VF-1\): v[0-9]+ net=[-+0-9]+ expected=0 at exit block [0-9]+' \
    | sort -u
}

if [ "$RAW" -eq 1 ]; then
  for f in "${FILES[@]}"; do
    lines=$(ORI_LOG=ori_arc::pipeline::aims_pipeline::postprocess=warn \
      "$BIN" build "$f" -o /tmp/burden_balance_out 2>&1 | extract)
    if [ -n "$lines" ]; then
      echo "=== $f ==="
      echo "$lines"
    fi
  done
  exit 0
fi

total=0
nfiles=0
declare -a FILE_LINES
for f in "${FILES[@]}"; do
  n=$(ORI_LOG=ori_arc::pipeline::aims_pipeline::postprocess=warn \
    "$BIN" build "$f" -o /tmp/burden_balance_out 2>&1 | extract | wc -l)
  if [ "$n" -gt 0 ]; then
    total=$((total + n))
    nfiles=$((nfiles + 1))
    FILE_LINES+=("$n	$f")
  fi
done

if [ "$SHOW_FILES" -eq 1 ]; then
  printf '%s\n' "${FILE_LINES[@]}" | sort -rn
  echo "---"
fi
echo "VF-1 imbalances: $total distinct across $nfiles files (build=$BUILD)"
