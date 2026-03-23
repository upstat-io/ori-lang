#!/usr/bin/env python3
"""Internal parsing helpers for ir_parser.py.

Contains regex patterns, constant sets, and low-level string parsing
functions. This module has no dependency on ir_parser data classes.

Not intended for direct import by metric extractors -- use ir_parser.py.
"""

from __future__ import annotations

import re


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

LINKAGES = frozenset({
    "private", "internal", "external", "linkonce_odr", "weak_odr",
    "linkonce", "weak", "appending", "common", "available_externally",
})
STORAGE = frozenset({
    "dso_local", "dso_preemptable", "unnamed_addr", "local_unnamed_addr",
})
CC = frozenset({
    "fastcc", "ccc", "coldcc", "x86_fastcallcc", "x86_thiscallcc",
    "arm_apcscc", "arm_aapcscc", "win64cc", "preserve_allcc",
})
RETURN_ATTRS = frozenset({
    "noundef", "signext", "zeroext", "nonnull", "noalias", "inreg",
})
PARAM_ATTRS = frozenset({
    "noundef", "signext", "zeroext", "nonnull", "noalias", "inreg",
    "nocapture", "readonly", "writeonly", "byval", "sret", "nest",
})

# Known function-level attributes that may appear inline after the param list
FUNC_ATTRS = frozenset({
    "nounwind", "uwtable", "cold", "noreturn", "willreturn",
    "nosync", "nofree", "nocallback", "speculatable",
    "noundef", "noalias", "readonly", "readnone",
    "mustprogress", "nonlazybind",
})


# ---------------------------------------------------------------------------
# Regex Patterns
# ---------------------------------------------------------------------------

# Handles both bare @name( and quoted @"name"(
FUNC_NAME_RE = re.compile(r'@(?:"([^"]+)"|([\w.$]+))\s*\(')
BLOCK_LABEL_RE = re.compile(r'^([\w.$]+):')
GLOBAL_RE = re.compile(r'^@[\w.$]+\s*=')
ATTR_GROUP_RE = re.compile(r'^attributes\s+#(\d+)\s*=\s*\{([^}]*)\}')
ATTR_REF_RE = re.compile(r'#(\d+)')
ASSIGN_RE = re.compile(r'^(%[\w.$]+)\s*=\s*(.*)')
METADATA_RE = re.compile(r',?\s*![\w.]+\s+!\d+')
INVOKE_TARGETS_RE = re.compile(
    r'to\s+label\s+%(\S+)\s+unwind\s+label\s+%(\S+)'
)


# ---------------------------------------------------------------------------
# Parsing Helpers (no data-class dependencies)
# ---------------------------------------------------------------------------

def parse_attr_set(text: str) -> set[str]:
    """Parse 'nounwind uwtable' or 'cold noreturn' into a set.

    Handles attrs with parens like memory(none) or memory(argmem: readwrite).
    """
    attrs: set[str] = set()
    i = 0
    n = len(text)
    while i < n:
        while i < n and text[i].isspace():
            i += 1
        if i >= n:
            break
        start = i
        depth = 0
        while i < n and (not text[i].isspace() or depth > 0):
            if text[i] == '(':
                depth += 1
            elif text[i] == ')':
                depth -= 1
            i += 1
        token = text[start:i]
        if token:
            attrs.add(token)
    return attrs


def parse_function_header(line: str) -> tuple[
    bool, str, str, str, str, list[str], list[set[str]], list[int], set[str]
] | None:
    """Parse a define/declare line.

    Returns (is_def, name, raw_name, cc, return_type, param_types,
             param_attrs, group_refs, inline_attrs) or None.

    ``name`` is the clean identifier (no ``@`` or quotes).
    ``raw_name`` is the LLVM-level name (with ``@`` and quotes if present).
    """
    stripped = line.strip().rstrip('{').strip()

    is_def = stripped.startswith('define ')
    keyword = 'define' if is_def else 'declare'

    name_match = FUNC_NAME_RE.search(stripped)
    if not name_match:
        return None

    # Group 1 = quoted name, Group 2 = bare name
    func_name = name_match.group(1) or name_match.group(2)
    raw_name = f'@"{func_name}"' if name_match.group(1) else f"@{func_name}"
    name_start = name_match.start()

    # --- Prefix: keyword ... return_type @name ---
    prefix = stripped[len(keyword):name_start].strip()
    cc = "ccc"
    remaining: list[str] = []
    for word in prefix.split():
        if word in LINKAGES or word in STORAGE:
            continue
        if word in CC:
            cc = word
            continue
        remaining.append(word)

    return_type = _extract_return_type(remaining)
    inline_attrs = {w for w in remaining if w in RETURN_ATTRS}

    # --- Params ---
    paren_start = stripped.index('(', name_start)
    paren_end = _find_matching_paren(stripped, paren_start)
    if paren_end == -1:
        return None

    params_text = stripped[paren_start + 1:paren_end]
    param_types, param_attrs = _parse_params(params_text)

    # --- Suffix: attr refs, inline attrs ---
    suffix = stripped[paren_end + 1:].strip()
    group_refs: list[int] = [
        int(m.group(1)) for m in ATTR_REF_RE.finditer(suffix)
    ]
    for word in suffix.split():
        if word.startswith('#') or word in (
            '{', '}', 'unnamed_addr', 'local_unnamed_addr',
        ):
            continue
        if word.startswith(('personality', 'section', 'comdat')):
            break
        if word.startswith('!') or word.startswith('align'):
            continue
        if word in FUNC_ATTRS:
            inline_attrs.add(word)

    return (is_def, func_name, raw_name, cc, return_type, param_types,
            param_attrs, group_refs, inline_attrs)


def _extract_return_type(words: list[str]) -> str:
    """Extract return type from prefix words (after stripping linkage/cc)."""
    type_words = [w for w in words if w not in RETURN_ATTRS]
    if not type_words:
        return "void"
    text = ' '.join(type_words)
    if '{' in text:
        brace_start = text.index('{')
        brace_end = text.rindex('}')
        return text[brace_start:brace_end + 1]
    return type_words[-1]


def _find_matching_paren(text: str, start: int) -> int:
    """Find matching ) for ( at start, handling nesting."""
    depth = 0
    for i in range(start, len(text)):
        if text[i] == '(':
            depth += 1
        elif text[i] == ')':
            depth -= 1
            if depth == 0:
                return i
    return -1


def _parse_params(text: str) -> tuple[list[str], list[set[str]]]:
    """Parse parameter list like 'i64 noundef %0, i64 noundef %1'."""
    text = text.strip()
    if not text:
        return [], []

    segments = _split_on_commas(text)
    param_types: list[str] = []
    param_attrs: list[set[str]] = []

    for seg in segments:
        seg = seg.strip()
        if seg == '...' or not seg:
            continue

        words = seg.split()
        if not words:
            continue

        # Struct type: { i64, i1 }
        if words[0] == '{':
            close = seg.find('}')
            if close >= 0:
                param_types.append(seg[:close + 1].strip())
                rest = seg[close + 1:].strip()
                param_attrs.append({w for w in rest.split() if w in PARAM_ATTRS})
            continue

        param_types.append(words[0])
        # Handle parameterized attributes like sret({ i64, ptr }) and
        # dereferenceable(N) — extract the base attribute name before '('
        attrs = set()
        for w in words[1:]:
            if w in PARAM_ATTRS:
                attrs.add(w)
            elif '(' in w:
                base = w[:w.index('(')]
                if base in PARAM_ATTRS:
                    attrs.add(base)
        param_attrs.append(attrs)

    return param_types, param_attrs


def _split_on_commas(text: str) -> list[str]:
    """Split text by commas, respecting { } nesting."""
    segments: list[str] = []
    depth = 0
    current: list[str] = []
    for ch in text:
        if ch == '{':
            depth += 1
        elif ch == '}':
            depth -= 1
        elif ch == ',' and depth == 0:
            segments.append(''.join(current))
            current = []
            continue
        current.append(ch)
    if current:
        segments.append(''.join(current))
    return segments
