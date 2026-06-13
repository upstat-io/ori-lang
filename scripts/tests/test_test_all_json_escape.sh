#!/usr/bin/env bash
# TDD pin: test-all.sh must JSON-escape control characters in failure-message /
# test_id strings so build/test-all-summary.json is valid JSON (RFC 8259 §7 —
# every U+0000..U+001F escaped or stripped). A literal tab in a failure message
# (the compile-fail escaping test t_escaping) otherwise writes a raw 0x09 into a
# JSON string value, making the summary unparseable.
#
# Pins: (a) the shared json_escape_string() helper exists; (b) it escapes
# tab/newline/CR faithfully as \t/\n/\r and preserves backslash + double-quote;
# (c) it strips rare invisible C0 controls so EVERY input yields valid JSON;
# (d) BOTH parse_rust_failures + parse_ori_failures route their non-hardcoded
# string slots through the helper (the SSOT wiring).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTALL="$HERE/../../test-all.sh"

if [ ! -f "$TESTALL" ]; then
    echo "FAIL: test-all.sh not found at $TESTALL"
    exit 1
fi

# Extract just the json_escape_string() definition (test-all.sh has no
# BASH_SOURCE main-guard, so it cannot be sourced wholesale).
fn="$(sed -n '/^json_escape_string()/,/^}/p' "$TESTALL")"
if [ -z "$fn" ]; then
    echo "FAIL: json_escape_string() is not defined in test-all.sh (cure unshipped)"
    exit 1
fi
eval "$fn"

# (b) Faithful case: tab + LF + CR + backslash + double-quote MUST round-trip
# exactly (each control escaped, not lost). LF/CR exercise the line-oriented-sed
# hazard — a naive `s/\n/\\n/` in a basic sed chain will not match a newline in
# pattern space, so this case fails a broken newline-escaper.
faithful="$(printf 'a\tb\nc\rd\\e"f')"
escaped_faithful="$(json_escape_string "$faithful")"

# (c) Strip case: a rare invisible C0 control (0x01) MUST be removed so the
# output is valid JSON (the cure strips the rare-C0 range rather than \u-escaping).
rare="$(printf 'a\001b')"
escaped_rare="$(json_escape_string "$rare")"

python3 - "$escaped_faithful" "$escaped_rare" <<'PY'
import json, sys
escaped_faithful, escaped_rare = sys.argv[1], sys.argv[2]
# Faithful: parses + round-trips to the original bytes (controls preserved).
parsed = json.loads('"' + escaped_faithful + '"')
assert parsed == 'a\tb\nc\rd\\e"f', f"faithful round-trip mismatch: {parsed!r}"
for ch in ('\t', '\n', '\r'):
    assert ch in parsed, f"{ch!r} must be preserved (escaped), not stripped"
# Strip: parses + the rare 0x01 control is gone (valid JSON for every input).
parsed_rare = json.loads('"' + escaped_rare + '"')
assert parsed_rare == 'ab', f"rare C0 control must be stripped: {parsed_rare!r}"
print("OK: json_escape_string escapes tab/LF/CR faithfully + strips rare C0 + valid JSON")
PY

# (d) SSOT wiring: BOTH parse_rust_failures + parse_ori_failures MUST route
# their failure strings through the shared helper. Extract each body and assert
# it references json_escape_string — a regression leaving either site on the old
# inline sed escaping would slip past the helper-only test above.
for parser in parse_rust_failures parse_ori_failures; do
    body="$(sed -n "/^${parser}()/,/^}/p" "$TESTALL")"
    if ! printf '%s' "$body" | grep -q 'json_escape_string'; then
        echo "FAIL: ${parser}() does not route through json_escape_string (SSOT wiring missing)"
        exit 1
    fi
done
echo "OK: parse_rust_failures + parse_ori_failures both route through json_escape_string"

# (e) Integration pin: exercise the REAL parse_ori_failures `while IFS= read -r`
# loop end-to-end, not just json_escape_string with literal args. A helper that
# escapes literal args but corrupts output inside the read loop would pass
# (b)/(c)/(d) yet break the live summary. Extract parse_ori_failures (it calls
# json_escape_string, eval'd above), feed a tab+quote+backslash FAIL line, and
# assert json.load parses the emitted array.
parse_fn="$(sed -n '/^parse_ori_failures()/,/^}/p' "$TESTALL")"
if [ -z "$parse_fn" ]; then
    echo "FAIL: parse_ori_failures() is not defined in test-all.sh"
    exit 1
fi
eval "$parse_fn"
in_file="$(mktemp)"
out_file="$(mktemp)"
printf '[FAIL] t_escaping then\ttab and a "quote" and a \\backslash end\n' > "$in_file"
parse_ori_failures "$in_file" "ori_llvm" > "$out_file"
rm -f "$in_file"
python3 - "$out_file" <<'PY'
import json, sys
arr = json.loads(open(sys.argv[1]).read())  # raises on unfixed raw-tab corruption
assert len(arr) == 1, f"expected 1 failure, got {len(arr)}"
# The faithfully-escaped \t round-trips back to a real tab after JSON decode
# (escape, not strip — rejects the strip-all variant on the live path too).
assert '\t' in arr[0]["error_message"], "tab must survive (escaped then decoded) in error_message"
assert '\t' in arr[0]["test_id"], "tab must survive (escaped then decoded) in test_id"
print("OK: parse_ori_failures read-loop path emits parseable JSON with faithful tab")
PY
rm -f "$out_file"
