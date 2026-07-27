#!/usr/bin/env bash
# Contract pins for the shared scan helpers in `_common.sh`.
#
# The absence/negative-filter suites exercise `enumerate_corpus`,
# `corpus_files_into`, `scan_symbol_into`, `count_symbol_files`, and
# `list_symbol_files`. `scan_pattern_into` has no caller inside this repository,
# so a regression isolated to it passes both of those suites and reaches
# consumers unchallenged.
#
# This suite pins the helper's OWN contract, naming no consumer. The properties
# below are what a caller may rely on; a change that breaks one must fail here,
# in the repository that owns the file, without any external checkout existing.
set -uo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_common.sh
source "$SCRIPT_DIR/_common.sh"

PASS=0
FAIL=0

ok()   { echo "PASS $1"; PASS=$((PASS + 1)); }
bad()  { echo "FAIL $1"; FAIL=$((FAIL + 1)); }

WORK="$(mktemp -d)" || { echo "Error: mktemp -d failed" >&2; exit 2; }
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

printf 'alpha PredicateXStack beta\n' > "$WORK/a.txt"
printf 'no match here\n'              > "$WORK/b.txt"
printf 'GATED-PROBE marker\n'         > "$WORK/c.txt"

# 1. REGEX, not fixed-string. `scan_symbol_into` matches literally; the pattern
#    sibling must interpret alternation. A caller relying on a multi-term regex
#    silently matches nothing if this degrades to a literal search.
res=()
if scan_pattern_into res 'Predicate.Stack|gated.probe' "$WORK/a.txt" "$WORK/b.txt" "$WORK/c.txt"; then
    [[ ${#res[@]} -eq 2 ]] && ok "regex alternation matches both terms" \
                           || bad "regex alternation matched ${#res[@]}, expected 2"
else
    bad "regex scan returned an error"
fi

# 2. CASE-INSENSITIVE. `GATED-PROBE` must match a lowercase pattern.
res=()
if scan_pattern_into res 'gated.probe' "$WORK/c.txt"; then
    [[ ${#res[@]} -eq 1 ]] && ok "match is case-insensitive" \
                           || bad "case-insensitive match returned ${#res[@]}, expected 1"
else
    bad "case-insensitive scan returned an error"
fi

# 3. FIXED-STRING sibling stays literal. A regex metacharacter must NOT be
#    interpreted by `scan_symbol_into`, or a caller passing a literal symbol
#    containing `.` or `|` silently over-matches.
printf 'literal a.b here\n' > "$WORK/lit.txt"
printf 'literal axb here\n' > "$WORK/lit2.txt"
res=()
if scan_symbol_into res 'a.b' "$WORK/lit.txt" "$WORK/lit2.txt"; then
    [[ ${#res[@]} -eq 1 ]] && ok "fixed-string scan stays literal" \
                           || bad "fixed-string scan matched ${#res[@]}, expected 1"
else
    bad "fixed-string scan returned an error"
fi

# 4. NUL PRESERVED through a newline-bearing filename. One physical file must be
#    ONE array element whose bytes equal the path. Newline-delimited transport
#    splits it into several, inflating counts and emitting paths that do not
#    exist.
nl_name="$(printf 'part\nmid\nlast.txt')"
nl_path="$WORK/$nl_name"
printf 'PredicateXStack\n' > "$nl_path"
res=()
if scan_pattern_into res 'predicate.stack' "$nl_path"; then
    if [[ ${#res[@]} -eq 1 && "${res[0]}" == "$nl_path" ]]; then
        ok "newline-bearing filename is one unchanged element"
    else
        bad "newline filename yielded ${#res[@]} element(s), bytes intact: $([[ "${res[0]:-}" == "$nl_path" ]] && echo yes || echo no)"
    fi
else
    bad "newline-filename scan returned an error"
fi

# 5. EXPLICIT-PATH scanning, no directory walk. Passing a directory's FILES must
#    not pull in siblings the caller did not name -- corpus identity belongs to
#    the caller, never to the scanner's ignore-walker.
res=()
if scan_pattern_into res 'predicate.stack' "$WORK/b.txt"; then
    [[ ${#res[@]} -eq 0 ]] && ok "explicit-path scan adds no unnamed files" \
                           || bad "explicit-path scan returned ${#res[@]}, expected 0"
else
    bad "explicit-path scan returned an error"
fi

# 6. CALLER-ARRAY REPLACEMENT. The named array is REPLACED, never appended to:
#    a stale element surviving a second call silently inflates every later count.
res=("stale-entry")
if scan_pattern_into res 'predicate.stack' "$WORK/b.txt"; then
    [[ ${#res[@]} -eq 0 ]] && ok "caller array is replaced, not appended" \
                           || bad "caller array retained ${#res[@]} stale element(s)"
else
    bad "array-replacement scan returned an error"
fi

# 7. SCANNER ERROR IS NOT AN EMPTY SUCCESS. An `rg` exit > 1 must return 2, so a
#    query failure can never be read as "no matches" -- the admitting direction
#    that makes a protected floor satisfiable by a broken query.
res=()
scan_pattern_into res 'predicate.stack' "$WORK/does-not-exist-$$.txt" >/dev/null 2>&1
rc=$?
[[ $rc -eq 2 ]] && ok "scanner error returns 2, not empty success" \
                || bad "scanner error returned $rc, expected 2"

echo "---"
echo "contract self-test: $PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]] || exit 1
exit 0
