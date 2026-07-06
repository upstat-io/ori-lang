# Shared ANSI helpers for shell scripts. Source this file; do not execute it.

# strip_ansi: stdin filter that removes SGR color sequences (ESC[...m) and
# preserves every other byte. Captured cargo/nextest output may carry color
# even when redirected to a file (FORCE_COLOR in the calling environment
# overrides isatty detection), so any exact-text matching against toolchain
# output must run on stripped text.
strip_ansi() {
    local esc
    esc=$(printf '\033')
    sed "s/${esc}\[[0-9;]*m//g"
}
