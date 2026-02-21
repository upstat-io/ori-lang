#!/usr/bin/env python3
"""Fix inline Ori source strings in Rust test files.

Adds trailing `;` after expression-body item declarations in Ori source
strings embedded in Rust code. Handles:
  - Regular strings: "..."
  - Raw strings: r"...", r#"..."#

Only modifies strings that look like Ori source (contain @-declarations,
type declarations, etc.).
"""

import re
import sys
from pathlib import Path

# Pattern for Ori item declarations with expression bodies
# These are the kinds that go through eat_optional_item_semicolon():
#   - @name (...) -> type = expr
#   - type Name = Body
#   - impl methods inside impl blocks

# Characters that indicate the expression body ended with a block (no ; needed)
BLOCK_ENDINGS = {'}'}

# Patterns that indicate continuation onto the next line
CONTINUATION_ENDINGS = [
    ' yield', ' then', ' else', ' do', ' in', ' ->', ' =',
    ' +', ' -', ' *', ' /', ' %', ' &&', ' ||', ' |>', ' ==', ' !=',
    ' <', ' >', ' <=', ' >=', ' <<', ' >>', ' &', ' |', ' ^',
    ' and', ' or', ',',
]

CONTINUATION_STARTS = ['.', '|>']


def needs_semicolon(line: str) -> bool:
    """Check if an Ori source line is an item declaration that needs ;."""
    stripped = line.rstrip()
    if not stripped or stripped.startswith('//'):
        return False
    # Already has semicolon
    if stripped.endswith(';'):
        return False
    # Block body - no semicolon needed
    if stripped.endswith('}'):
        return False
    # Check if line ends with a continuation
    for cont in CONTINUATION_ENDINGS:
        if stripped.endswith(cont):
            return False
    return True


def is_item_declaration_line(line: str) -> bool:
    """Check if a line starts an Ori item declaration."""
    stripped = line.lstrip()
    # Function/test: @name or pub @name
    if stripped.startswith('@') or stripped.startswith('pub @'):
        return True
    # Type declaration: type Name =
    if stripped.startswith('type ') or stripped.startswith('pub type '):
        return True
    return False


def next_line_is_continuation(lines: list, idx: int) -> bool:
    """Check if the next non-empty line continues the current expression."""
    for i in range(idx + 1, min(idx + 5, len(lines))):
        next_stripped = lines[i].strip()
        if not next_stripped or next_stripped.startswith('//'):
            continue
        for start in CONTINUATION_STARTS:
            if next_stripped.startswith(start):
                return True
        return False
    return False


def process_ori_source(source: str) -> str:
    """Process Ori source code and add semicolons where needed."""
    lines = source.split('\n')
    result_lines = []

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.rstrip()

        # Track whether we're inside an item declaration
        # Simple heuristic: if we see @name or type, this starts a declaration
        # that might span multiple lines until we find the body expression

        # For simple single-line declarations, just check and add ;
        if is_item_declaration_line(line) and '=' in line:
            # This line has the = sign, so body starts here
            # Check if it's a single-line expression body
            eq_idx = line.index('=')
            body_part = line[eq_idx+1:].strip()

            if body_part and needs_semicolon(line) and not next_line_is_continuation(lines, i):
                result_lines.append(stripped + ';')
                i += 1
                continue

        # For multi-line declarations, we need to track the body
        # The body ends when we see a line that doesn't continue
        # This is handled by the continuation detection

        # Check non-declaration lines that might be the end of a multi-line body
        # These are harder to detect generically, so we leave them for now

        result_lines.append(line)
        i += 1

    return '\n'.join(result_lines)


def extract_and_fix_rust_strings(content: str) -> str:
    """Find and fix Ori source strings in Rust code."""
    result = []
    i = 0

    while i < len(content):
        # Look for raw strings: r"..." or r#"..."#
        if content[i:i+2] == 'r"' or content[i:i+3] == 'r#"':
            # Find the raw string
            if content[i:i+3] == 'r#"':
                start = i
                i += 3
                end_marker = '"#'
                end_idx = content.find(end_marker, i)
                if end_idx == -1:
                    result.append(content[start:])
                    break
                string_content = content[i:end_idx]

                # Check if this looks like Ori source
                if looks_like_ori(string_content):
                    fixed = fix_ori_in_string(string_content)
                    result.append(content[start:start+3])
                    result.append(fixed)
                    result.append(end_marker)
                else:
                    result.append(content[start:end_idx + len(end_marker)])

                i = end_idx + len(end_marker)
            else:
                # r"..."
                start = i
                i += 2
                end_idx = find_unescaped_quote(content, i)
                if end_idx == -1:
                    result.append(content[start:])
                    break
                string_content = content[i:end_idx]

                if looks_like_ori(string_content):
                    fixed = fix_ori_in_string(string_content)
                    result.append('r"')
                    result.append(fixed)
                    result.append('"')
                else:
                    result.append(content[start:end_idx + 1])

                i = end_idx + 1

        # Look for regular strings: "..."
        elif content[i] == '"':
            start = i
            i += 1
            end_idx = find_unescaped_quote(content, i)
            if end_idx == -1:
                result.append(content[start:])
                break
            string_content = content[i:end_idx]

            if looks_like_ori_single_line(string_content):
                fixed = fix_single_line_ori(string_content)
                result.append('"')
                result.append(fixed)
                result.append('"')
            else:
                result.append(content[start:end_idx + 1])

            i = end_idx + 1
        else:
            result.append(content[i])
            i += 1

    return ''.join(result)


def find_unescaped_quote(content: str, start: int) -> int:
    """Find the next unescaped double quote."""
    i = start
    while i < len(content):
        if content[i] == '\\':
            i += 2  # Skip escaped character
        elif content[i] == '"':
            return i
        else:
            i += 1
    return -1


def looks_like_ori(s: str) -> bool:
    """Check if a string looks like Ori source code."""
    # Must contain function declarations, type declarations, or similar
    return bool(re.search(r'@\w+\s*[\(<]|^type\s+\w|^\s*@\w+\s*[\(<]|pub\s+@\w+', s, re.MULTILINE))


def looks_like_ori_single_line(s: str) -> bool:
    """Check if a single-line string looks like an Ori declaration."""
    s = s.strip()
    # Function declaration: @name (...) -> type = expr
    if re.match(r'@\w+', s) and '=' in s:
        return True
    # Type declaration: type Name = ...
    if re.match(r'(pub\s+)?type\s+\w+', s) and '=' in s:
        return True
    # Constant: let $name = ...
    if re.match(r'(pub\s+)?let\s+\$\w+', s) and '=' in s:
        return True
    # Constant shorthand: $name = ...
    if re.match(r'(pub\s+)?\$\w+\s*=', s):
        return True
    return False


def fix_single_line_ori(s: str) -> str:
    """Fix a single-line Ori source string by adding ; if needed."""
    stripped = s.rstrip()
    if not stripped:
        return s

    # Already has ;
    if stripped.endswith(';'):
        return s

    # Block body doesn't need ;
    if stripped.endswith('}'):
        return s

    # Multi-declaration strings (contain newlines)
    if '\\n' in s:
        return fix_multi_decl_string(s)

    # Check if this is a declaration with a body
    # Patterns: @name (...) -> type = expr
    #           type Name = Body
    #           let $name = expr
    #           $name = expr
    if '=' in stripped:
        # Find the = that's part of the declaration (not == or !=)
        # Simple check: if it has @name or type or let $
        if re.match(r'@\w+', stripped) or re.match(r'(pub\s+)?type\s+', stripped) or \
           re.match(r'(pub\s+)?let\s+\$', stripped) or re.match(r'(pub\s+)?\$\w+\s*=', stripped):
            return stripped + ';'

    return s


def fix_multi_decl_string(s: str) -> str:
    """Fix a string with \\n-separated declarations."""
    parts = s.split('\\n')
    result = []
    for j, part in enumerate(parts):
        stripped = part.rstrip()
        if stripped and not stripped.startswith('//') and not stripped.endswith(';') and \
           not stripped.endswith('}') and '=' in stripped:
            # Check if it's a declaration
            trimmed = stripped.lstrip()
            if re.match(r'@\w+', trimmed) or re.match(r'(pub\s+)?type\s+', trimmed) or \
               re.match(r'(pub\s+)?let\s+\$', trimmed) or re.match(r'(pub\s+)?\$\w+\s*=', trimmed):
                # Check if next part is a continuation
                if j + 1 < len(parts):
                    next_part = parts[j+1].strip()
                    if next_part and (next_part.startswith('.') or next_part.startswith('|>')):
                        result.append(part)
                        continue
                result.append(stripped + ';')
                continue
        result.append(part)
    return '\\n'.join(result)


def fix_ori_in_string(s: str) -> str:
    """Fix Ori source in a multi-line raw string."""
    lines = s.split('\n')
    result = []

    for i, line in enumerate(lines):
        stripped = line.rstrip()

        # Skip empty lines and comments
        if not stripped or stripped.lstrip().startswith('//'):
            result.append(line)
            continue

        # Already has ;
        if stripped.endswith(';'):
            result.append(line)
            continue

        # Block body
        if stripped.endswith('}'):
            result.append(line)
            continue

        # Check if this line ends a declaration body
        trimmed = stripped.lstrip()

        # Is this a single-line declaration with body?
        if '=' in trimmed:
            is_decl = False
            if re.match(r'@\w+', trimmed):
                is_decl = True
            elif re.match(r'(pub\s+)?@\w+', trimmed):
                is_decl = True
            elif re.match(r'(pub\s+)?type\s+\w+', trimmed):
                is_decl = True
            elif re.match(r'(pub\s+)?let\s+\$', trimmed):
                is_decl = True
            elif re.match(r'(pub\s+)?\$\w+\s*=', trimmed):
                is_decl = True

            if is_decl:
                # Check for continuation
                has_continuation = False
                for cont in CONTINUATION_ENDINGS:
                    if stripped.endswith(cont):
                        has_continuation = True
                        break

                if not has_continuation:
                    # Check next non-empty line
                    for j in range(i + 1, min(i + 5, len(lines))):
                        next_line = lines[j].strip()
                        if not next_line or next_line.startswith('//'):
                            continue
                        for start in CONTINUATION_STARTS:
                            if next_line.startswith(start):
                                has_continuation = True
                                break
                        break

                if not has_continuation:
                    result.append(stripped + ';')
                    continue

        # Check if this is the last line of a multi-line declaration body
        # Heuristic: if previous lines had a declaration and this line
        # is indented (continuation of body), it might need ;
        # This is harder to detect generically...

        result.append(line)

    return '\n'.join(result)


def process_file(filepath: Path) -> bool:
    """Process a single Rust file. Returns True if modified."""
    content = filepath.read_text()
    new_content = extract_and_fix_rust_strings(content)

    if new_content != content:
        filepath.write_text(new_content)
        return True
    return False


def main():
    """Process all Rust test files in the compiler directory."""
    compiler_dir = Path(__file__).parent.parent / 'compiler'

    # Find all Rust test files
    test_files = []
    for rs_file in compiler_dir.rglob('*.rs'):
        # Only process test files
        name = rs_file.name
        path_str = str(rs_file)
        if 'test' in name or 'test' in path_str.split('/')[-2:]:
            test_files.append(rs_file)

    # Also check for integration test directories
    for test_dir in compiler_dir.rglob('tests'):
        if test_dir.is_dir():
            for rs_file in test_dir.rglob('*.rs'):
                if rs_file not in test_files:
                    test_files.append(rs_file)

    modified = 0
    for filepath in sorted(test_files):
        if process_file(filepath):
            print(f"  Fixed: {filepath.relative_to(compiler_dir.parent)}")
            modified += 1

    print(f"\nProcessed {len(test_files)} files, modified {modified}")


if __name__ == '__main__':
    main()
