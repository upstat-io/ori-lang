#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/diagnostics" "$WORK/compiler/ori_parse/src"
cp "$ROOT/diagnostics/check-tracing-coverage.sh" "$WORK/diagnostics/"

cat > "$WORK/compiler/ori_parse/Cargo.toml" <<'EOF'
[dependencies]
tracing.workspace = true
EOF

write_instrumented_file() {
    local relative_path="$1"
    shift
    mkdir -p "$(dirname "$WORK/compiler/ori_parse/$relative_path")"
    : > "$WORK/compiler/ori_parse/$relative_path"
    for symbol in "$@"; do
        cat >> "$WORK/compiler/ori_parse/$relative_path" <<EOF
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn $symbol() {}
EOF
    done
}

write_instrumented_file src/lib.rs parse parse_with_metadata parse_incremental
write_instrumented_file src/module_parse.rs parse_module parse_imports parse_module_incremental
write_instrumented_file src/dispatch.rs dispatch_declaration
write_instrumented_file src/grammar/expr/mod.rs \
    parse_expr parse_non_assign_expr parse_non_comparison_expr
write_instrumented_file src/grammar/expr/primary/mod.rs parse_primary
write_instrumented_file src/grammar/ty/mod.rs parse_type
write_instrumented_file src/grammar/item/function/mod.rs parse_function_or_test
write_instrumented_file src/grammar/item/trait_def.rs parse_trait
write_instrumented_file src/grammar/item/impl_def/mod.rs parse_impl parse_def_impl
write_instrumented_file src/grammar/item/type_decl.rs parse_type_decl

positive_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color)"
case "$positive_output" in
    *"all 17 parser boundaries are instrumented"*"All checks passed."*) ;;
    *)
        printf 'FAIL: complete parser instrumentation was rejected\n%s\n' "$positive_output"
        exit 1
        ;;
esac

sed -i '1d' "$WORK/compiler/ori_parse/src/grammar/expr/primary/mod.rs"
set +e
negative_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color 2>&1)"
negative_rc=$?
set -e
if [[ $negative_rc -ne 1 || "$negative_output" != *"parse_primary must have"* ]]; then
    printf 'FAIL: missing parser boundary was not diagnosed (exit %s)\n%s\n' \
        "$negative_rc" "$negative_output"
    exit 1
fi

write_instrumented_file src/grammar/expr/primary/mod.rs parse_primary
mkdir -p "$WORK/compiler/ori_dead/src"
cat > "$WORK/compiler/ori_dead/Cargo.toml" <<'EOF'
[dependencies]
tracing.workspace = true
EOF
cat > "$WORK/compiler/ori_dead/src/lib.rs" <<'EOF'
pub fn work() {}
EOF

set +e
dead_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color 2>&1)"
dead_rc=$?
set -e
if [[ $dead_rc -ne 1 || "$dead_output" != *"ori_dead declares tracing"* ]]; then
    printf 'FAIL: dead tracing dependency was not diagnosed (exit %s)\n%s\n' \
        "$dead_rc" "$dead_output"
    exit 1
fi

echo "PASS: tracing coverage accepts complete spans and rejects missing or dead instrumentation"
