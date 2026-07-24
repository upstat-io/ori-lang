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
#   diagnostics/burden-balance.sh --lineage-net tests/spec   # per-lineage post-lowering RC-net imbalances
#   diagnostics/burden-balance.sh --lineage-net --raw file.ori  # per-lineage nets for one file
#
# VF-1 mode (default) counts per-var `verify_burden_balance` imbalances.
# `--lineage-net` mode surfaces the COMPLEMENTARY per-same-alloc-lineage RC-net
# (post-burden-lowering `fresh-alloc(+1) + RcInc - RcDec` per rep) — a cross-var
# signal VF-1's per-var count is blind to: a dup-alias lineage nets 0 per-var
# while the lineage nets +N (leak) / -N (double-free), per `arc.md §Debugging`
# alias-lineage net + AIMS RL-2 release-once-per-lineage.
#
# Exit 0 always (measurement tool). The count is printed to stdout.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_DIR"

BUILD=debug
SHOW_FILES=0
RAW=0
LINEAGE_NET=0
TARGETS=()
for arg in "$@"; do
  case "$arg" in
    --release) BUILD=release ;;
    --files) SHOW_FILES=1 ;;
    --raw) RAW=1 ;;
    --lineage-net) LINEAGE_NET=1 ;;
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

# Distinct-imbalance extractor: pull `vN net=... at exit block M` tuples
# (plus the attribution suffix `[def=<kind> exit=<kind> ops=iN/dN/pN/vN]`
# when the compiler emits it), dedup.
extract() {
  grep -oE 'burden imbalance \(VF-1\): v[0-9]+ net=[-+0-9]+ expected=0 at exit block [0-9]+( \[def=[a-z-]+ exit=[a-z-]+ repr=[a-z-]+ ops=i[0-9]+/d[0-9]+/p[0-9]+/v[0-9]+\])?' \
    | sort -u
}

# Per-lineage extractor: strip ANSI, pull the realize-trace
# `alias-lineage RC-net imbalance` line's `fn_name="..." rep=N net=N` tuple
# (emitted only for net != 0), dedup. The realize trace is verbose; the grep
# keeps only the imbalance signal.
lineage_extract() {
  sed 's/\x1b\[[0-9;]*m//g' \
    | grep 'alias-lineage RC-net imbalance' \
    | grep -oE 'fn_name="[^"]*" rep=[0-9]+ net=[-+0-9]+' \
    | sort -u
}

if [ "$LINEAGE_NET" -eq 1 ]; then
  if [ "$RAW" -eq 1 ]; then
    for f in "${FILES[@]}"; do
      lines=$(ORI_LOG=ori_arc::aims::realize=trace \
        "$BIN" build "$f" -o /tmp/burden_balance_out 2>&1 | lineage_extract)
      if [ -n "$lines" ]; then
        echo "=== $f ==="
        echo "$lines"
      fi
    done
    exit 0
  fi
  # net < 0 = over-release (unambiguous double-free, the §09.2 dup-alias root);
  # net > 0 = leak OR a legitimate transfer-out (returned/stored value the
  # caller owns) — investigate per-fixture with `--lineage-net --raw`.
  ln_total=0
  ln_df=0
  ln_nfiles=0
  declare -a LN_FILE_LINES
  for f in "${FILES[@]}"; do
    out=$(ORI_LOG=ori_arc::aims::realize=trace \
      "$BIN" build "$f" -o /tmp/burden_balance_out 2>&1 | lineage_extract)
    [ -z "$out" ] && continue
    n=$(printf '%s\n' "$out" | grep -c 'net=')
    df=$(printf '%s\n' "$out" | grep -cE 'net=-[0-9]+')
    ln_total=$((ln_total + n))
    ln_df=$((ln_df + df))
    ln_nfiles=$((ln_nfiles + 1))
    LN_FILE_LINES+=("$n	df=$df	$f")
  done
  if [ "$SHOW_FILES" -eq 1 ]; then
    printf '%s\n' "${LN_FILE_LINES[@]}" | sort -rn
    echo "---"
  fi
  echo "Per-lineage RC-net imbalances: $ln_total distinct ($ln_df net<0 double-free) across $ln_nfiles files (build=$BUILD)"
  exit 0
fi

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
