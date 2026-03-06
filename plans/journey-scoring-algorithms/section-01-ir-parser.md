---
section: "01"
title: "LLVM IR Parser"
status: complete
goal: "Parse LLVM IR text into structured per-function data usable by all metric extractors"
depends_on: []
sections:
  - id: "01.1"
    title: "IR Text Parser"
    status: complete
  - id: "01.2"
    title: "Function Classification"
    status: not-started
  - id: "01.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: LLVM IR Parser

**Status:** Not Started
**Goal:** Parse the textual LLVM IR dumped by `ORI_DUMP_AFTER_LLVM=1` into a structured representation that all metric extractors can query: function definitions with their attributes, basic blocks, and instructions.

**Context:** All 4 IR-based metric extractors (instruction efficiency, ARC, attributes, control flow) need to inspect the same LLVM IR output. Rather than each extractor parsing IR independently (duplicated, fragile), a shared parser produces a structured intermediate representation that extractors consume. (The "IR Quality" scoring dimension is derived from the instruction efficiency and control flow extractors, not a separate module. The binary metrics extractor does not parse IR at all.)

LLVM IR is well-structured text with a consistent grammar. We don't need a full LLVM IR parser — only enough to extract: function definitions (name, attributes, calling convention), basic blocks (label, instructions), and instructions (opcode, operands as raw text). Declarations (`declare`) are also needed for attribute checking on runtime functions.

---

## 01.1 IR Text Parser

**File(s):** `.claude/skills/code-journey/ir_parser.py`

The parser reads LLVM IR text and extracts function-level structure. It doesn't need to understand LLVM IR semantics — just its syntactic structure.

**Key patterns to recognize:**

```text
; Function header comment
define [linkage] [cc] [ret_type] @name([params]) [attrs] {
block_label:                    ; preds = ...
  %reg = instruction operands
  ...
}

declare [ret_type] @name([params]) [attrs]

attributes #N = { attr1 attr2 ... }
```

- [ ] Parse `define` lines: extract function name, calling convention (`fastcc`, default), return type, parameter types, attribute group references (`#0`, `#1`), individual attributes (`noundef`)
- [ ] Parse `declare` lines: same fields, no body
- [ ] Parse basic blocks: label (or implicit `entry`/`bb0`), list of instructions
- [ ] Parse `attributes #N = { ... }` groups and resolve references
- [ ] Parse instructions: extract opcode (first word after `%reg =` or standalone), full text. No deep operand parsing needed — extractors pattern-match on instruction text.
- [ ] Handle LLVM IR comments (lines starting with `;`, inline `; comment`)
- [ ] Handle global constants (`@name = private unnamed_addr constant ...`)
- [ ] Handle empty IR input (compilation failed): `parse_module("")` returns an empty `Module` with a `parse_errors` list containing a descriptive error, NOT a crash
- [ ] Handle truncated/malformed IR (partial output from crashed compiler): parser must not raise unhandled exceptions — collect parse errors and return partial results
- [ ] Handle IR with no `define` blocks (e.g., only `declare` and `attributes`): valid case, return Module with empty user functions

**Parsing strategy:** Line-oriented with regex. LLVM IR is line-structured — one instruction per line, block labels on their own line, function boundaries at `define`/`}`. No need for a grammar parser.

> **WARNING (regex fragility):** LLVM IR is "well-structured" but has many syntactic variations the simple regex approach may not handle:
> - **Metadata annotations:** `!dbg !42`, `!tbaa !5` at end of instructions — must be stripped before opcode extraction
> - **Alignment/addrspace:** `load i64, ptr %x, align 8` — `align` is not an opcode
> - **Comdat groups:** `$_ori_foo = comdat any` — looks like a function but isn't
> - **Global aliases:** `@alias = alias i64, ptr @original`
> - **Multi-line instructions:** rare in Ori's output, but LLVM `phi` with many predecessors can span lines
> - **Vector types:** `<4 x i32>` — angle brackets could confuse attribute group parsing
>
> **Mitigation:** Start with Journey 1's known IR (simple, no edge cases). Add edge-case handling incrementally as more journeys expose new patterns. Each new LLVM IR pattern encountered should become a test case.

```python
import re
from dataclasses import dataclass, field

@dataclass
class Instruction:
    text: str           # Full instruction text (stripped)
    opcode: str         # First keyword: "call", "br", "ret", "add", etc.
    result: str | None  # %register name if assignment, else None

@dataclass
class BasicBlock:
    label: str
    instructions: list[Instruction]

@dataclass
class Function:
    name: str                       # Without @_ori_ prefix
    raw_name: str                   # Full LLVM name (e.g., @_ori_add)
    is_definition: bool             # define vs declare
    calling_convention: str         # "fastcc", "ccc" (default), etc.
    return_type: str                # "i64", "void", etc.
    attributes: set[str]            # Resolved: {"nounwind", "uwtable", ...}
    attribute_group_refs: list[int]  # Raw group references: [0, 2] for "#0 #2"
    blocks: list[BasicBlock]        # Empty for declarations
    param_attributes: list[set[str]]  # Per-parameter attributes
    param_types: list[str]           # ["i64", "i64", ...] — needed for ARC scalar RC detection

@dataclass
class Module:
    functions: dict[str, Function]  # Keyed by raw_name
    globals: list[str]              # Global constant definitions
    attribute_groups: dict[int, set[str]]  # #N -> {attr1, attr2, ...}
    parse_errors: list[str]         # Non-fatal parse issues (malformed lines, etc.)
```


- [ ] Write `parse_module(ir_text: str) -> Module` entry point
- [ ] **File size check:** If `ir_parser.py` approaches 400 lines, split into `ir_parser.py` (data model: `Instruction`, `BasicBlock`, `Function`, `Module` dataclasses + classification methods) and `ir_parser_core.py` (regex parsing: `parse_module()`, `_parse_function()`, `_parse_block()`, `_parse_instruction()`). The data model is the stable API; the parsing is the volatile implementation detail.
- [ ] Unit tests in `tests/test_ir_parser.py` with Journey 1's known IR (hardcoded as test fixture)

---

## 01.2 Function Classification

**File(s):** `.claude/skills/code-journey/ir_parser.py`

Classify functions into categories needed by metric extractors:

- [ ] **User functions**: names matching `@_ori_*` that are `define` (not `declare`)
- [ ] **Runtime declarations**: `@ori_panic_cstr`, `@ori_panic`, `@ori_rc_inc`, `@ori_rc_dec`, `@ori_buffer_rc_dec`, `@ori_list_rc_inc`, etc.
- [ ] **LLVM intrinsics**: `@llvm.sadd.with.overflow.*`, `@llvm.smul.with.overflow.*`, etc.
- [ ] **Entry wrapper**: `@main` (C ABI wrapper)

The `Function` dataclass needs the following computed properties for use by metric extractors:

```python
@property
def is_user_function(self) -> bool:
    """User-defined function (subject to scoring)."""
    return self.is_definition and self.raw_name.startswith("@_ori_")

@property
def is_runtime_decl(self) -> bool:
    """Runtime function declaration (ori_* without _ori_ prefix)."""
    return not self.is_definition and self.raw_name.startswith("@ori_")

@property
def is_entry_called(self) -> bool:
    """@_ori_main — called from C main wrapper, uses default cc."""
    return self.raw_name == "@_ori_main"
```

```python
def user_functions(self) -> list[Function]:
    """Functions defined by user code (subject to scoring)."""
    return [f for f in self.functions.values()
            if f.is_definition and f.raw_name.startswith("@_ori_")]

def is_entry_wrapper(self, f: Function) -> bool:
    """The C main() wrapper that calls @_ori_main."""
    return f.raw_name == "@main" and f.is_definition

def runtime_declarations(self) -> list[Function]:
    """Runtime function declarations (ori_*, not user code)."""
    return [f for f in self.functions.values()
            if not f.is_definition and f.raw_name.startswith("@ori_")]

def llvm_intrinsics(self) -> list[Function]:
    """LLVM intrinsic declarations (@llvm.*)."""
    return [f for f in self.functions.values()
            if f.raw_name.startswith("@llvm.")]
```

- [ ] Unit tests: classify Journey 1's functions correctly
- [ ] Unit test: `user_functions()` excludes `@main` wrapper
- [ ] Unit test: `runtime_declarations()` returns `@ori_panic_cstr` but not `@_ori_add`
- [ ] Unit test: `llvm_intrinsics()` returns `@llvm.sadd.with.overflow.i64`

---

## 01.3 Completion Checklist

- [ ] `parse_module()` correctly parses Journey 1's LLVM IR dump
- [ ] All functions, blocks, and instructions extracted with correct counts
- [ ] Attribute groups resolved to per-function attribute sets
- [ ] Function classification identifies user/runtime/intrinsic/wrapper
- [ ] Parser handles edge cases: empty blocks, multi-line instructions (none expected from Ori but defensive)
- [ ] Parser handles empty input (returns Module with empty functions and a parse error)
- [ ] Parser handles IR with only declarations (no definitions) — valid result, not an error
- [ ] Parser handles malformed lines gracefully (skips with warning in `parse_errors`, does not crash)
- [ ] `python3 -m pytest tests/test_ir_parser.py` passes

**Exit Criteria:** `parse_module(journey_1_ir)` returns a `Module` with the correct set of definitions and declarations from Journey 1's IR dump. Verify against the actual output of `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/01-arithmetic.ori` at implementation time. At minimum, expect: `@_ori_add` (definition, 7 instructions including overflow check), `@_ori_main` (definition, call + ret), `@main` (definition, 3-instruction ABI wrapper), `@ori_panic_cstr` (declaration), and at least one `@llvm.sadd.with.overflow.i64` (declaration). Attribute groups must be correctly resolved (e.g., `#0 = { nounwind uwtable }`).
