#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LINT="$HERE/../test-weakening-lint.py"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

comment_diff="$WORK/comment-only.diff"
cat > "$comment_diff" <<'EOF'
diff --git a/compiler/example.rs b/compiler/example.rs
index 111111111..222222222 100644
--- a/compiler/example.rs
+++ b/compiler/example.rs
@@ -1 +1 @@
-fn example() {}
+fn example() { work(); }
diff --git a/tests/spec/example.ori b/tests/spec/example.ori
index 333333333..444444444 100644
--- a/tests/spec/example.ori
+++ b/tests/spec/example.ori
@@ -1,7 +1 @@
-// ================================================================
-// Example group
-// ================================================================
-// Context one.
-// Context two.
-// Context three.
 @test_example tests _ () -> void = ();
EOF

python3 "$LINT" --diff-file "$comment_diff"

surface_diff="$WORK/surface-loss.diff"
cat > "$surface_diff" <<'EOF'
diff --git a/compiler/example.rs b/compiler/example.rs
index 111111111..222222222 100644
--- a/compiler/example.rs
+++ b/compiler/example.rs
@@ -1 +1 @@
-fn example() {}
+fn example() { work(); }
diff --git a/tests/spec/example.ori b/tests/spec/example.ori
index 333333333..444444444 100644
--- a/tests/spec/example.ori
+++ b/tests/spec/example.ori
@@ -1,6 +1 @@
-@case_one () -> int = 1;
-@case_two () -> int = 2;
-@case_three () -> int = 3;
-@case_four () -> int = 4;
-@case_five () -> int = 5;
 @test_example tests _ () -> void = ();
EOF

set +e
surface_output="$(python3 "$LINT" --diff-file "$surface_diff" 2>&1)"
surface_rc=$?
set -e
if [ "$surface_rc" -ne 1 ]; then
    printf 'FAIL: executable test loss exited %s instead of 1\n%s\n' "$surface_rc" "$surface_output"
    exit 1
fi
case "$surface_output" in
    *"P5: net-negative executable line count (-5, threshold 5)"*) ;;
    *)
        printf 'FAIL: executable test loss did not produce P5\n%s\n' "$surface_output"
        exit 1
        ;;
esac

echo "PASS: comment cleanup is ignored while executable test loss remains gated"
