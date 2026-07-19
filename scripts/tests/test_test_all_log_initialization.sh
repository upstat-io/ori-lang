#!/bin/bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/test_all/logging.sh
source "$HERE/../test_all/logging.sh"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

target="$TMP_DIR/human.log"
log="$TMP_DIR/test-all.log"
printf '%s\n' 'runtime transcript' > "$target"

ln -s "human.log" "$log"
initialize_test_all_log "$log"
if [ -L "$log" ] || [ -s "$log" ]; then
    echo "FAIL: symlink destination was not replaced with an empty regular file"
    exit 1
fi
if [ "$(< "$target")" != "runtime transcript" ]; then
    echo "FAIL: symlink target was modified"
    exit 1
fi

printf '%s\n' 'runtime transcript' > "$target"
rm -f "$log"
ln "$target" "$log"
if [ ! "$log" -ef "$target" ]; then
    echo "FAIL: hardlink fixture does not share the target inode"
    exit 1
fi
initialize_test_all_log "$log"
if [ "$log" -ef "$target" ] || [ -s "$log" ]; then
    echo "FAIL: hardlink destination was not replaced with a distinct empty file"
    exit 1
fi
if [ "$(< "$target")" != "runtime transcript" ]; then
    echo "FAIL: hardlink target was modified"
    exit 1
fi

if initialize_test_all_log "$TMP_DIR/missing/test-all.log" 2>/dev/null; then
    echo "FAIL: initialization succeeded with a missing parent directory"
    exit 1
fi

echo "PASS: standalone log initialization preserves forwarded targets"
