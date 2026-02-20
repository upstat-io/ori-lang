#!/usr/bin/env python3
"""Convert run() blocks to { } blocks in AOT test files.

Handles nested run() inside loop(), and run() inside run().
Converts comma-separated statements to semicolons.

IMPORTANT: Processes innermost run() first to avoid corrupting
nested blocks with incorrect semicolons.
"""

import sys


def find_matching_paren(s: str, start: int) -> int:
    """Find the closing paren matching the opening paren at `start`."""
    depth = 0
    i = start
    while i < len(s):
        c = s[i]
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
            if depth == 0:
                return i
        elif c == '"':
            # Skip string literals
            i += 1
            while i < len(s) and s[i] != '"':
                if s[i] == '\\':
                    i += 1  # skip escaped char
                i += 1
        elif c == "'":
            # Skip char literals (e.g., 'a')
            i += 1
            if i < len(s) and s[i] == '\\':
                i += 1  # skip escaped char
            i += 1  # skip the char
            # Don't skip closing quote — loop will advance
        i += 1
    return -1


def find_matching_brace(s: str, start: int) -> int:
    """Find the closing brace matching the opening brace at `start`."""
    depth = 0
    i = start
    while i < len(s):
        c = s[i]
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                return i
        elif c == '"':
            i += 1
            while i < len(s) and s[i] != '"':
                if s[i] == '\\':
                    i += 1
                i += 1
        i += 1
    return -1


def convert_run_body(body: str) -> str:
    """Convert the body of a run() block: commas to semicolons.

    The last non-empty line's trailing comma is REMOVED (it's the result expression).
    All other trailing commas become semicolons.
    """
    lines = body.split('\n')

    # Find last non-empty line
    last_nonempty = -1
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].strip():
            last_nonempty = i
            break

    result = []
    for i, line in enumerate(lines):
        stripped = line.rstrip()
        if i == last_nonempty and stripped.endswith(','):
            # Last statement: remove trailing comma (it's the result expression)
            result.append(stripped[:-1])
        elif stripped.endswith(',') and i < last_nonempty:
            # Non-last statement: comma -> semicolon
            result.append(stripped[:-1] + ';')
        else:
            result.append(stripped if stripped else '')

    return '\n'.join(result)


def find_innermost_run(content: str) -> tuple[int, int] | None:
    """Find the innermost run() — one whose body contains no other run().

    Returns (start_of_run, end_of_closing_paren) or None.
    """
    pos = 0
    candidates = []

    while pos < len(content):
        idx = content.find('run(', pos)
        if idx == -1:
            break

        # Check it's not part of a larger word
        if idx > 0 and (content[idx - 1].isalnum() or content[idx - 1] == '_'):
            pos = idx + 4
            continue

        paren_start = idx + 3
        paren_end = find_matching_paren(content, paren_start)
        if paren_end == -1:
            pos = idx + 4
            continue

        body = content[paren_start + 1:paren_end]
        candidates.append((idx, paren_end, body))
        pos = idx + 4

    # Find the innermost: one whose body doesn't contain another run(
    for idx, paren_end, body in candidates:
        # Check if body contains another run( (that isn't part of a word)
        has_nested = False
        bpos = 0
        while bpos < len(body):
            bidx = body.find('run(', bpos)
            if bidx == -1:
                break
            if bidx > 0 and (body[bidx - 1].isalnum() or body[bidx - 1] == '_'):
                bpos = bidx + 4
                continue
            has_nested = True
            break

        if not has_nested:
            return (idx, paren_end)

    return None


def get_indent(content: str, idx: int) -> str:
    """Get the indentation of the line containing position `idx`."""
    line_start = content.rfind('\n', 0, idx)
    if line_start == -1:
        line_start = -1
    indent = ''
    for c in content[line_start + 1:idx]:
        if c in ' \t':
            indent += c
        else:
            break
    return indent


def process_content(content: str) -> str:
    """Process content, converting all run() blocks to {} blocks.

    Processes innermost run() first to avoid corrupting nested blocks.
    """
    result = content

    while True:
        match = find_innermost_run(result)
        if match is None:
            break

        idx, paren_end = match
        paren_start = idx + 3
        body = result[paren_start + 1:paren_end]

        # Convert body: commas -> semicolons
        converted_body = convert_run_body(body)

        # Build replacement
        indent = get_indent(result, idx)

        if converted_body.startswith('\n'):
            replacement = '{'
        else:
            replacement = '{ '
        replacement += converted_body
        if not converted_body.endswith('\n'):
            replacement += '\n'
        replacement += indent + '}'

        result = result[:idx] + replacement + result[paren_end + 1:]

    # Handle loop(expr) -> loop expr patterns
    # After run() conversion, we may have loop({ ... }) patterns
    # The loop() paren form was removed, so loop(break 42) -> loop break 42
    # and loop({ ... }) -> loop { ... }
    import re

    # loop(break ...) -> loop break ...
    pos = 0
    while pos < len(result):
        m = re.search(r'loop\(', result[pos:])
        if m is None:
            break
        loop_idx = pos + m.start()
        paren_start = loop_idx + 4
        paren_end = find_matching_paren(result, paren_start)
        if paren_end == -1:
            pos = loop_idx + 5
            continue

        inner = result[paren_start + 1:paren_end]

        # Check if it's loop({ ... }) — remove outer parens
        stripped = inner.strip()
        if stripped.startswith('{') and stripped.endswith('}'):
            # loop({ ... }) -> loop { ... }
            result = result[:loop_idx] + 'loop ' + stripped + result[paren_end + 1:]
        else:
            # loop(expr) -> loop expr (simple case like loop(break 42))
            result = result[:loop_idx] + 'loop ' + inner.strip() + result[paren_end + 1:]

        pos = loop_idx + 5

    return result


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 convert_aot_run.py <file>...")
        sys.exit(1)

    for path in sys.argv[1:]:
        with open(path) as f:
            content = f.read()

        converted = process_content(content)

        if content != converted:
            with open(path, 'w') as f:
                f.write(converted)
            # Count changes
            old_count = content.count('= run(') + content.count(' run(')
            new_count = converted.count('= run(') + converted.count(' run(')
            print(f"{path}: converted {old_count - new_count} run()/loop() blocks")
        else:
            print(f"{path}: no changes")


if __name__ == '__main__':
    main()
