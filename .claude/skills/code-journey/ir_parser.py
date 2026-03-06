#!/usr/bin/env python3
"""LLVM IR parser for Code Journey metric extraction.

Parses textual LLVM IR (from ORI_DUMP_AFTER_LLVM=1) into structured
per-function data usable by all metric extractors.

Requires Python 3.10+ for union type syntax.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field


# ---------------------------------------------------------------------------
# Data Model
# ---------------------------------------------------------------------------

@dataclass
class Instruction:
    """A single LLVM IR instruction."""
    text: str           # Full instruction text (whitespace-stripped, no comments)
    opcode: str         # First keyword: "call", "br", "ret", "extractvalue", etc.
    result: str | None  # %register name if assignment, else None


@dataclass
class BasicBlock:
    """A basic block within a function."""
    label: str
    instructions: list[Instruction] = field(default_factory=list)


@dataclass
class Function:
    """An LLVM IR function (definition or declaration)."""
    name: str                        # Without @ (e.g., "_ori_add", "ori_panic_cstr")
    raw_name: str                    # Full LLVM name with @ (e.g., "@_ori_add")
    is_definition: bool              # define vs declare
    calling_convention: str          # "fastcc", "ccc" (default), etc.
    return_type: str                 # "i64", "void", "{ i64, i1 }", etc.
    attributes: set[str]             # Resolved: {"nounwind", "uwtable", "noundef", ...}
    attribute_group_refs: list[int]  # Raw refs: [0, 2] for #0 #2
    blocks: list[BasicBlock]         # Empty for declarations
    param_attributes: list[set[str]]  # Per-parameter attributes
    param_types: list[str]           # ["i64", "ptr", ...]

    @property
    def is_user_function(self) -> bool:
        """User-defined function (subject to scoring)."""
        return self.is_definition and self.raw_name.startswith("@_ori_")

    @property
    def is_runtime_decl(self) -> bool:
        """Runtime function declaration (ori_* without _ori_ prefix)."""
        return (not self.is_definition
                and self.raw_name.startswith("@ori_")
                and not self.raw_name.startswith("@_ori_"))

    @property
    def is_entry_called(self) -> bool:
        """@_ori_main — called from C main wrapper, uses default cc."""
        return self.raw_name == "@_ori_main"

    @property
    def is_llvm_intrinsic(self) -> bool:
        """LLVM intrinsic (@llvm.*)."""
        return self.raw_name.startswith("@llvm.")

    @property
    def instruction_count(self) -> int:
        """Total instructions across all basic blocks."""
        return sum(len(b.instructions) for b in self.blocks)


@dataclass
class Module:
    """Parsed LLVM IR module."""
    functions: dict[str, Function] = field(default_factory=dict)
    globals: list[str] = field(default_factory=list)
    attribute_groups: dict[int, set[str]] = field(default_factory=dict)
    parse_errors: list[str] = field(default_factory=list)

    def user_functions(self) -> list[Function]:
        """Functions defined by user code (subject to scoring)."""
        return [f for f in self.functions.values() if f.is_user_function]

    def runtime_declarations(self) -> list[Function]:
        """Runtime function declarations (ori_*, not user code)."""
        return [f for f in self.functions.values() if f.is_runtime_decl]

    def llvm_intrinsics(self) -> list[Function]:
        """LLVM intrinsic declarations (@llvm.*)."""
        return [f for f in self.functions.values() if f.is_llvm_intrinsic]

    def is_entry_wrapper(self, f: Function) -> bool:
        """The C main() wrapper that calls @_ori_main."""
        return f.raw_name == "@main" and f.is_definition


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_LINKAGES = frozenset({
    "private", "internal", "external", "linkonce_odr", "weak_odr",
    "linkonce", "weak", "appending", "common", "available_externally",
})
_STORAGE = frozenset({"dso_local", "dso_preemptable", "unnamed_addr", "local_unnamed_addr"})
_CC = frozenset({
    "fastcc", "ccc", "coldcc", "x86_fastcallcc", "x86_thiscallcc",
    "arm_apcscc", "arm_aapcscc", "win64cc", "preserve_allcc",
})
_RETURN_ATTRS = frozenset({
    "noundef", "signext", "zeroext", "nonnull", "noalias", "inreg",
})
_PARAM_ATTRS = frozenset({
    "noundef", "signext", "zeroext", "nonnull", "noalias", "inreg",
    "nocapture", "readonly", "writeonly", "byval", "sret", "nest",
})

_FUNC_NAME_RE = re.compile(r'@([\w.$]+)\s*\(')
_BLOCK_LABEL_RE = re.compile(r'^([\w.$]+):')
_GLOBAL_RE = re.compile(r'^@[\w.$]+\s*=')
_ATTR_GROUP_RE = re.compile(r'^attributes\s+#(\d+)\s*=\s*\{([^}]*)\}')
_ATTR_REF_RE = re.compile(r'#(\d+)')
_ASSIGN_RE = re.compile(r'^(%[\w.$]+)\s*=\s*(.*)')
_METADATA_RE = re.compile(r',?\s*![\w.]+\s+!\d+')


# ---------------------------------------------------------------------------
# Parsing
# ---------------------------------------------------------------------------

def parse_module(ir_text: str) -> Module:
    """Parse LLVM IR text into a Module.

    Handles empty input, malformed lines, and partial output gracefully.
    """
    module = Module()
    if not ir_text or not ir_text.strip():
        module.parse_errors.append("Empty IR input")
        return module

    lines = ir_text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        if not stripped or stripped.startswith(';'):
            i += 1
            continue

        if stripped.startswith('source_filename') or stripped.startswith('target'):
            i += 1
            continue

        attr_match = _ATTR_GROUP_RE.match(stripped)
        if attr_match:
            gid = int(attr_match.group(1))
            module.attribute_groups[gid] = _parse_attr_set(attr_match.group(2).strip())
            i += 1
            continue

        if stripped.startswith('define '):
            func, end_idx = _parse_function_def(lines, i, module)
            if func:
                module.functions[func.raw_name] = func
            i = end_idx + 1
            continue

        if stripped.startswith('declare '):
            func = _parse_function_decl(stripped, module)
            if func:
                module.functions[func.raw_name] = func
            i += 1
            continue

        if _GLOBAL_RE.match(stripped):
            module.globals.append(stripped)
            i += 1
            continue

        # Comdat groups: $name = comdat ...
        if stripped.startswith('$'):
            i += 1
            continue

        module.parse_errors.append(f"Line {i + 1}: unrecognized: {stripped[:80]}")
        i += 1

    _resolve_attributes(module)
    return module


def _parse_attr_set(text: str) -> set[str]:
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


def _parse_function_header(line: str) -> tuple[
    bool, str, str, str, list[str], list[set[str]], list[int], set[str]
] | None:
    """Parse a define/declare line.

    Returns (is_def, raw_name, cc, return_type, param_types,
             param_attrs, group_refs, inline_attrs) or None.
    """
    stripped = line.strip().rstrip('{').strip()

    is_def = stripped.startswith('define ')
    keyword = 'define' if is_def else 'declare'

    name_match = _FUNC_NAME_RE.search(stripped)
    if not name_match:
        return None

    func_name = name_match.group(1)
    raw_name = f"@{func_name}"
    name_start = name_match.start()

    # --- Prefix: keyword ... return_type @name ---
    prefix = stripped[len(keyword):name_start].strip()
    cc = "ccc"
    remaining: list[str] = []
    for word in prefix.split():
        if word in _LINKAGES or word in _STORAGE:
            continue
        if word in _CC:
            cc = word
            continue
        remaining.append(word)

    return_type = _extract_return_type(remaining)
    inline_attrs = {w for w in remaining if w in _RETURN_ATTRS}

    # --- Params ---
    paren_start = stripped.index('(', name_start)
    paren_end = _find_matching_paren(stripped, paren_start)
    if paren_end == -1:
        return None

    params_text = stripped[paren_start + 1:paren_end]
    param_types, param_attrs = _parse_params(params_text)

    # --- Suffix: attr refs, inline attrs ---
    suffix = stripped[paren_end + 1:].strip()
    group_refs: list[int] = [int(m.group(1)) for m in _ATTR_REF_RE.finditer(suffix)]
    for word in suffix.split():
        if word.startswith('#') or word in ('{', '}', 'unnamed_addr', 'local_unnamed_addr'):
            continue
        if word.startswith('personality') or word.startswith('section') or word.startswith('comdat'):
            break
        if word.startswith('!') or word.startswith('align'):
            continue
        # Known function-level attrs that may appear inline
        if word in ('nounwind', 'uwtable', 'cold', 'noreturn', 'willreturn',
                     'nosync', 'nofree', 'nocallback', 'speculatable',
                     'noundef', 'noalias', 'readonly', 'readnone',
                     'mustprogress', 'nonlazybind'):
            inline_attrs.add(word)

    return (is_def, raw_name, cc, return_type, param_types, param_attrs,
            group_refs, inline_attrs)


def _extract_return_type(words: list[str]) -> str:
    """Extract return type from prefix words (after stripping linkage/cc)."""
    type_words = [w for w in words if w not in _RETURN_ATTRS]
    if not type_words:
        return "void"
    # Struct types: { i64, i1 }
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
                param_attrs.append({w for w in rest.split() if w in _PARAM_ATTRS})
            continue

        param_types.append(words[0])
        param_attrs.append({w for w in words[1:] if w in _PARAM_ATTRS})

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


def _parse_function_def(
    lines: list[str], start: int, module: Module
) -> tuple[Function | None, int]:
    """Parse a function definition (define ... { ... })."""
    header_line = lines[start].strip()

    parsed = _parse_function_header(header_line)
    if not parsed:
        module.parse_errors.append(f"Line {start + 1}: failed to parse function header")
        i = start + 1
        while i < len(lines) and lines[i].strip() != '}':
            i += 1
        return None, i

    is_def, raw_name, cc, ret_type, p_types, p_attrs, g_refs, inl_attrs = parsed

    blocks: list[BasicBlock] = []
    current_block: BasicBlock | None = None
    i = start + 1

    while i < len(lines):
        stripped = lines[i].strip()

        if stripped == '}':
            if current_block:
                blocks.append(current_block)
            break

        if not stripped or stripped.startswith(';'):
            i += 1
            continue

        label_match = _BLOCK_LABEL_RE.match(stripped)
        if label_match and not stripped.startswith('%'):
            if current_block:
                blocks.append(current_block)
            current_block = BasicBlock(label=label_match.group(1))
            i += 1
            continue

        if current_block is None:
            current_block = BasicBlock(label="entry")

        # Multi-line instructions: switch ... [ ... ] spans multiple lines
        if stripped.startswith('switch') and '[' in stripped and ']' not in stripped:
            # Collect continuation lines until closing ]
            full_text = stripped
            i += 1
            while i < len(lines):
                cont = lines[i].strip()
                if not cont or cont.startswith(';'):
                    i += 1
                    continue
                full_text += ' ' + cont
                if ']' in cont:
                    i += 1
                    break
                i += 1
            current_block.instructions.append(_parse_instruction(full_text))
            continue

        current_block.instructions.append(_parse_instruction(stripped))
        i += 1

    return Function(
        name=raw_name.lstrip('@'),
        raw_name=raw_name,
        is_definition=True,
        calling_convention=cc,
        return_type=ret_type,
        attributes=set(inl_attrs),
        attribute_group_refs=g_refs,
        blocks=blocks,
        param_attributes=p_attrs,
        param_types=p_types,
    ), i


def _parse_function_decl(line: str, module: Module) -> Function | None:
    """Parse a function declaration (declare ...)."""
    parsed = _parse_function_header(line)
    if not parsed:
        module.parse_errors.append(f"Failed to parse declaration: {line[:80]}")
        return None

    _, raw_name, cc, ret_type, p_types, p_attrs, g_refs, inl_attrs = parsed
    return Function(
        name=raw_name.lstrip('@'),
        raw_name=raw_name,
        is_definition=False,
        calling_convention=cc,
        return_type=ret_type,
        attributes=set(inl_attrs),
        attribute_group_refs=g_refs,
        blocks=[],
        param_attributes=p_attrs,
        param_types=p_types,
    )


def _parse_instruction(text: str) -> Instruction:
    """Parse a single instruction line."""
    stripped = text.strip()

    # Strip inline comments
    semi = stripped.find(';')
    if semi >= 0:
        stripped = stripped[:semi].rstrip()

    # Strip trailing metadata (!dbg !42, !tbaa !5)
    stripped = _METADATA_RE.sub('', stripped).rstrip(', ').rstrip()

    result = None
    opcode_text = stripped

    assign_match = _ASSIGN_RE.match(stripped)
    if assign_match:
        result = assign_match.group(1)
        opcode_text = assign_match.group(2).strip()

    words = opcode_text.split()
    opcode = words[0] if words else ""

    return Instruction(text=stripped, opcode=opcode, result=result)


def _resolve_attributes(module: Module) -> None:
    """Resolve attribute group references to per-function attribute sets."""
    for func in module.functions.values():
        for ref in func.attribute_group_refs:
            if ref in module.attribute_groups:
                func.attributes |= module.attribute_groups[ref]
