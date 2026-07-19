#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/diagnostics" "$WORK/compiler/ori_parse/src"
cp "$ROOT/diagnostics/check-tracing-coverage.sh" "$WORK/diagnostics/"
: > "$WORK/diagnostics/tracing-registry.txt"

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

write_instrumented_file src/api.rs parse parse_with_metadata parse_incremental
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
rm -rf "$WORK/compiler/ori_dead"

cat >> "$WORK/compiler/ori_parse/src/api.rs" <<'EOF'
pub fn emit_custom_trace() {
    tracing::debug!(target: "ori_parse::custom", "custom trace");
    let _span = tracing::debug_span!("custom_span").entered();
}
EOF
cat > "$WORK/compiler/ori_parse/src/secondary.rs" <<'EOF'
#[tracing::instrument(name = "custom_instrument", target = "ori_parse::instrument")]
pub fn emit_custom_trace() {
    tracing::debug!(target: "ori_parse::custom", "second custom trace");
}

// tracing::info_span!("commented_span");
/* tracing::debug!(target: "ori_parse::commented", "commented target"); */
const EXAMPLE: &str = r#"tracing::info_span!("string_span");
tracing::debug!(target: "ori_parse::string", "string target");"#;
mydebug_span!("suffix_span");
mydebug!(target: "ori_parse::suffix", "suffix target");
other::debug_span!("path_span");
log::debug!(target: "ori_parse::other_macro", "other macro target");
EOF

set +e
unregistered_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color 2>&1)"
unregistered_rc=$?
set -e
expected_api_target="UNREGISTERED: target \`ori_parse::custom\` in compiler/ori_parse/src/api.rs"
expected_secondary_target="UNREGISTERED: target \`ori_parse::custom\` in compiler/ori_parse/src/secondary.rs"
expected_instrument_target="UNREGISTERED: target \`ori_parse::instrument\` in compiler/ori_parse/src/secondary.rs"
expected_instrument_span="UNREGISTERED: span \`custom_instrument\` in compiler/ori_parse/src/secondary.rs"
expected_custom_span="UNREGISTERED: span \`custom_span\` in compiler/ori_parse/src/api.rs"
if [[ $unregistered_rc -ne 1 \
    || "$unregistered_output" != *"$expected_api_target"* \
    || "$unregistered_output" != *"$expected_secondary_target"* \
    || "$unregistered_output" != *"$expected_instrument_target"* \
    || "$unregistered_output" != *"$expected_instrument_span"* \
    || "$unregistered_output" != *"$expected_custom_span"* ]]; then
    printf 'FAIL: unregistered tracing literals were not diagnosed (exit %s)\n%s\n' \
        "$unregistered_rc" "$unregistered_output"
    exit 1
fi

{
    printf 'target | ori_parse::custom | compiler/ori_parse/src/api.rs\n'
    printf 'span | custom_span | compiler/ori_parse/src/api.rs\n'
} >> "$WORK/diagnostics/tracing-registry.txt"
set +e
partial_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color 2>&1)"
partial_rc=$?
set -e
if [[ $partial_rc -ne 1 || "$partial_output" != *"$expected_secondary_target"* ]]; then
    printf 'FAIL: registering one consumer concealed a second consumer (exit %s)\n%s\n' \
        "$partial_rc" "$partial_output"
    exit 1
fi

{
    printf 'target | ori_parse::custom | compiler/ori_parse/src/secondary.rs\n'
    printf 'target | ori_parse::instrument | compiler/ori_parse/src/secondary.rs\n'
    printf 'span | custom_instrument | compiler/ori_parse/src/secondary.rs\n'
} >> "$WORK/diagnostics/tracing-registry.txt"
registered_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color)"
if [[ "$registered_output" != *"target sites and"*"span-name sites are registered"* ]]; then
    printf 'FAIL: registered tracing target was rejected\n%s\n' "$registered_output"
    exit 1
fi

printf 'span | stale_span | compiler/ori_parse/src/api.rs\n' \
    >> "$WORK/diagnostics/tracing-registry.txt"
set +e
stale_output="$("$WORK/diagnostics/check-tracing-coverage.sh" --no-color 2>&1)"
stale_rc=$?
set -e
expected_stale_span="STALE: span \`stale_span\` is not present in compiler/ori_parse/src/api.rs"
if [[ $stale_rc -ne 1 || "$stale_output" != *"$expected_stale_span"* ]]; then
    printf 'FAIL: stale tracing literal was not diagnosed (exit %s)\n%s\n' \
        "$stale_rc" "$stale_output"
    exit 1
fi

echo "PASS: tracing coverage accepts complete spans and rejects missing, dead, unregistered, or stale instrumentation"
