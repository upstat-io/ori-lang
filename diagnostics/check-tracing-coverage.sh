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
#
# Exit codes:
#   0 = all checks pass
#   1 = one or more coverage issues found
#   2 = usage or repository-shape error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
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
        'src/lib.rs|parse'
        'src/lib.rs|parse_with_metadata'
        'src/lib.rs|parse_incremental'
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
