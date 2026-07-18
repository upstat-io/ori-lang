#!/usr/bin/env bash
# Validate that production tracing dependencies have live instrumentation.
#
# Usage:
#   diagnostics/check-tracing-coverage.sh [options]
#
# Options:
#   --color        Force color output (default: auto-detect terminal)
#   --no-color     Disable color output
#   -h, --help     Show this help
#
# Checks:
#   1. Every compiler crate with a direct `tracing` dependency uses it in
#      production source.
#   2. `ori_parse` retains spans on its public, module, declaration, expression,
#      type, and item-parsing boundaries. Every required span uses `skip_all` so
#      enabling parser traces does not eagerly format parser state.
#   3. Every explicit tracing target and span-name literal is declared in the
#      central tracing registry.
#
# Exit codes:
#   0 = all checks pass
#   1 = one or more coverage issues found
#   2 = usage or repository-shape error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TRACING_REGISTRY="$SCRIPT_DIR/tracing-registry.txt"
USE_COLOR=auto

while [[ $# -gt 0 ]]; do
    case $1 in
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        *)
            echo "Error: unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
    esac
done

if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then USE_COLOR=yes; else USE_COLOR=no; fi
fi

if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_BOLD='\033[1m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_BOLD="" C_NC=""
fi

COMPILER_DIR="$ROOT_DIR/compiler"
if [[ ! -d "$COMPILER_DIR" ]]; then
    echo "Error: compiler/ not found at $COMPILER_DIR" >&2
    exit 2
fi

check_tracing_registry() {
    python3 - "$COMPILER_DIR" "$TRACING_REGISTRY" <<'PY'
from __future__ import annotations

import pathlib
import re
import sys

compiler_dir = pathlib.Path(sys.argv[1])
registry_path = pathlib.Path(sys.argv[2])

if not registry_path.is_file():
    print(f"  MISSING: tracing registry not found at {registry_path}")
    raise SystemExit(1)

registered: dict[str, set[tuple[str, str]]] = {"target": set(), "span": set()}
errors: list[str] = []
for line_number, raw_line in enumerate(
    registry_path.read_text(encoding="utf-8").splitlines(), start=1
):
    line = raw_line.strip()
    if not line or line.startswith("#"):
        continue
    parts = [part.strip() for part in line.split(" | ")]
    if len(parts) != 3:
        errors.append(
            f"  INVALID: {registry_path}:{line_number} must be `kind | name | compiler/path.rs`"
        )
        continue
    kind, value, consumer = parts
    if kind not in registered or not value or not consumer.startswith("compiler/"):
        errors.append(
            f"  INVALID: {registry_path}:{line_number} must name a target/span and compiler source path"
        )
        continue
    entry = (value, consumer)
    if entry in registered[kind]:
        errors.append(f"  DUPLICATE: {kind} `{value}` in {consumer}")
        continue
    registered[kind].add(entry)

macro_names = (
    "enabled|trace|debug|info|warn|error|event|span|"
    "trace_span|debug_span|info_span|warn_span|error_span"
)
macro_pattern = re.compile(
    rf"(?<![A-Za-z0-9_:])(?:tracing::)?(?:{macro_names})!\s*\((.*?)\)",
    re.DOTALL,
)
target_pattern = re.compile(r'\btarget\s*:\s*"([^"]+)"')
level_span_pattern = re.compile(
    r'(?<![A-Za-z0-9_:])(?:tracing::)?(?:trace|debug|info|warn|error)_span!\s*\(\s*'
    r'(?:target\s*:\s*"[^"]+"\s*,\s*)?"([^"]+)"',
    re.DOTALL,
)
generic_span_pattern = re.compile(
    r'(?<![A-Za-z0-9_:])(?:tracing::)?span!\s*\(\s*'
    r'(?:target\s*:\s*"[^"]+"\s*,\s*)?[^,]+,\s*"([^"]+)"',
    re.DOTALL,
)
instrument_pattern = re.compile(
    r'#\[\s*(?:tracing::)?instrument\s*\((.*?)\)\s*\]',
    re.DOTALL,
)
instrument_name_pattern = re.compile(r'\bname\s*=\s*"([^"]+)"')
instrument_target_pattern = re.compile(r'\btarget\s*=\s*"([^"]+)"')
raw_string_prefix_pattern = re.compile(r'(?:br|r)(?P<hashes>#{0,255})"')
tracing_literal_candidate_pattern = re.compile(
    rf'(?:{macro_names})\s*!|(?:tracing::)?instrument'
)


def strip_rust_comments(source: str) -> tuple[str, list[tuple[int, int]]]:
    """Blank Rust comments and locate strings while preserving source positions."""
    output = list(source)
    string_ranges: list[tuple[int, int]] = []
    index = 0
    source_len = len(source)

    def blank(start: int, end: int) -> None:
        for position in range(start, end):
            if output[position] != "\n":
                output[position] = " "

    while index < source_len:
        raw_match = (
            raw_string_prefix_pattern.match(source, index)
            if source[index] in {"b", "r"}
            else None
        )
        if raw_match is not None:
            string_start = index
            terminator = '"' + raw_match.group("hashes")
            content_start = raw_match.end()
            terminator_start = source.find(terminator, content_start)
            index = source_len if terminator_start < 0 else terminator_start + len(terminator)
            string_ranges.append((string_start, index))
            continue

        if source[index] == '"':
            string_start = index
            index += 1
            while index < source_len:
                if source[index] == "\\":
                    index += 2
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            string_ranges.append((string_start, index))
            continue

        if source.startswith("//", index):
            comment_end = source.find("\n", index + 2)
            if comment_end < 0:
                comment_end = source_len
            blank(index, comment_end)
            index = comment_end
            continue

        if source.startswith("/*", index):
            depth = 1
            comment_end = index + 2
            while comment_end < source_len and depth > 0:
                if source.startswith("/*", comment_end):
                    depth += 1
                    comment_end += 2
                elif source.startswith("*/", comment_end):
                    depth -= 1
                    comment_end += 2
                else:
                    comment_end += 1
            blank(index, comment_end)
            index = comment_end
            continue

        index += 1

    return "".join(output), string_ranges


def inside_string(position: int, string_ranges: list[tuple[int, int]]) -> bool:
    return any(start <= position < end for start, end in string_ranges)


discovered: dict[str, set[tuple[str, str]]] = {"target": set(), "span": set()}
for source_path in sorted(compiler_dir.glob("*/**/*.rs")):
    consumer = (pathlib.Path("compiler") / source_path.relative_to(compiler_dir)).as_posix()
    raw_source = source_path.read_text(encoding="utf-8")
    if tracing_literal_candidate_pattern.search(raw_source) is None:
        continue
    source, string_ranges = strip_rust_comments(raw_source)
    for macro_match in macro_pattern.finditer(source):
        if inside_string(macro_match.start(), string_ranges):
            continue
        target_match = target_pattern.search(macro_match.group(1))
        target_position = (
            macro_match.start(1) + target_match.start() if target_match is not None else -1
        )
        if target_match is not None and not inside_string(target_position, string_ranges):
            discovered["target"].add((target_match.group(1), consumer))
    for span_match in level_span_pattern.finditer(source):
        if not inside_string(span_match.start(), string_ranges):
            discovered["span"].add((span_match.group(1), consumer))
    for span_match in generic_span_pattern.finditer(source):
        if not inside_string(span_match.start(), string_ranges):
            discovered["span"].add((span_match.group(1), consumer))
    for instrument_match in instrument_pattern.finditer(source):
        if inside_string(instrument_match.start(), string_ranges):
            continue
        name_match = instrument_name_pattern.search(instrument_match.group(1))
        name_position = (
            instrument_match.start(1) + name_match.start() if name_match is not None else -1
        )
        if name_match is not None and not inside_string(name_position, string_ranges):
            discovered["span"].add((name_match.group(1), consumer))
        target_match = instrument_target_pattern.search(instrument_match.group(1))
        target_position = (
            instrument_match.start(1) + target_match.start() if target_match is not None else -1
        )
        if target_match is not None and not inside_string(target_position, string_ranges):
            discovered["target"].add((target_match.group(1), consumer))

for kind in ("target", "span"):
    for value, consumer in sorted(discovered[kind] - registered[kind]):
        errors.append(f"  UNREGISTERED: {kind} `{value}` in {consumer}")
    for value, consumer in sorted(registered[kind] - discovered[kind]):
        errors.append(f"  STALE: {kind} `{value}` is not present in {consumer}")

if errors:
    print("\n".join(errors))
    raise SystemExit(1)

print(
    "  OK: "
    f"{len(discovered['target'])} target sites and {len(discovered['span'])} span-name sites are registered"
)
PY
}

mapfile -t TRACING_MANIFESTS < <(
    grep -El '^[[:space:]]*tracing([.]workspace)?[[:space:]]*=' \
        "$COMPILER_DIR"/*/Cargo.toml 2>/dev/null | sort
)
if [[ ${#TRACING_MANIFESTS[@]} -eq 0 ]]; then
    echo "Error: no direct tracing dependencies found under compiler/" >&2
    exit 2
fi

issues=0
printf "${C_BOLD}Checking production tracing use in %d compiler crates${C_NC}\n" \
    "${#TRACING_MANIFESTS[@]}"

for manifest in "${TRACING_MANIFESTS[@]}"; do
    crate_dir="${manifest%/Cargo.toml}"
    crate="${crate_dir##*/}"
    source_dir="$crate_dir/src"

    if [[ ! -d "$source_dir" ]]; then
        printf "  ${C_RED}MISSING${C_NC}: %s has no src/ directory\n" "$crate"
        issues=$((issues + 1))
        continue
    fi

    if grep -R -qE 'tracing::|use[[:space:]]+tracing([[:space:]:;,{])' \
        --include='*.rs' --exclude='tests.rs' --exclude-dir='tests' "$source_dir"; then
        printf "  ${C_GREEN}OK${C_NC}: %s\n" "$crate"
    else
        printf "  ${C_RED}DEAD${C_NC}: %s declares tracing but has no production instrumentation\n" \
            "$crate"
        issues=$((issues + 1))
    fi
done

printf '\n%bChecking explicit tracing target and span-name registry%b\n' "$C_BOLD" "$C_NC"
set +e
registry_output="$(check_tracing_registry 2>&1)"
registry_rc=$?
set -e
printf '%s\n' "$registry_output"
if [[ $registry_rc -ne 0 ]]; then
    issues=$((issues + 1))
fi

has_instrumented_symbol() {
    local path="$1"
    local symbol="$2"
    python3 - "$path" "$symbol" <<'PY'
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
symbol = sys.argv[2]
if not path.is_file():
    raise SystemExit(1)

source = path.read_text(encoding="utf-8")
visibility = r"(?:pub(?:\([^)]*\))?\s+)?"
pattern = re.compile(
    r"#\[\s*tracing::instrument\s*\([^\]]*\bskip_all\b[^\]]*\)\s*\]\s+"
    + visibility
    + r"fn\s+"
    + re.escape(symbol)
    + r"\b",
    re.DOTALL,
)
raise SystemExit(0 if pattern.search(source) else 1)
PY
}

ORI_PARSE_DIR="$COMPILER_DIR/ori_parse"
if grep -qE '^[[:space:]]*tracing([.]workspace)?[[:space:]]*=' \
    "$ORI_PARSE_DIR/Cargo.toml" 2>/dev/null; then
    printf '\n%bChecking required ori_parse span boundaries%b\n' "$C_BOLD" "$C_NC"
    PARSER_ANCHORS=(
        'src/api.rs|parse'
        'src/api.rs|parse_with_metadata'
        'src/api.rs|parse_incremental'
        'src/module_parse.rs|parse_module'
        'src/module_parse.rs|parse_imports'
        'src/module_parse.rs|parse_module_incremental'
        'src/dispatch.rs|dispatch_declaration'
        'src/grammar/expr/mod.rs|parse_expr'
        'src/grammar/expr/mod.rs|parse_non_assign_expr'
        'src/grammar/expr/mod.rs|parse_non_comparison_expr'
        'src/grammar/expr/primary/mod.rs|parse_primary'
        'src/grammar/ty/mod.rs|parse_type'
        'src/grammar/item/function/mod.rs|parse_function_or_test'
        'src/grammar/item/trait_def.rs|parse_trait'
        'src/grammar/item/impl_def/mod.rs|parse_impl'
        'src/grammar/item/impl_def/mod.rs|parse_def_impl'
        'src/grammar/item/type_decl.rs|parse_type_decl'
    )

    parser_issues=0
    for anchor in "${PARSER_ANCHORS[@]}"; do
        relative_path="${anchor%%|*}"
        symbol="${anchor##*|}"
        if ! has_instrumented_symbol "$ORI_PARSE_DIR/$relative_path" "$symbol"; then
            printf "  ${C_RED}MISSING${C_NC}: %s must have a tracing::instrument span with skip_all (%s)\n" \
                "$symbol" "$relative_path"
            parser_issues=$((parser_issues + 1))
            issues=$((issues + 1))
        fi
    done
    if [[ $parser_issues -eq 0 ]]; then
        printf "  ${C_GREEN}OK${C_NC}: all %d parser boundaries are instrumented\n" \
            "${#PARSER_ANCHORS[@]}"
    fi
fi

printf '\n%bSummary:%b\n' "$C_BOLD" "$C_NC"
printf "  Direct tracing dependencies: %d | Issues: %d\n" \
    "${#TRACING_MANIFESTS[@]}" "$issues"

if [[ $issues -eq 0 ]]; then
    printf '\n%b%bAll checks passed.%b\n' "$C_GREEN" "$C_BOLD" "$C_NC"
    exit 0
fi

printf '\n%b%b%d issue(s) found.%b\n' "$C_RED" "$C_BOLD" "$issues" "$C_NC"
exit 1
