---
section: "04"
title: "IR Parser Hardening"
status: not-started
goal: "Fix known parsing failures in ir_parser.py so all 12 journeys produce complete, correct parse results"
depends_on: []
sections:
  - id: "04.0"
    title: "Prerequisite: Split ir_parser.py"
    status: not-started
  - id: "04.1"
    title: "Quoted Function Names"
    status: not-started
  - id: "04.2"
    title: "Multi-line Instruction Handling"
    status: not-started
  - id: "04.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: IR Parser Hardening

**Status:** Not Started
**Goal:** Fix known parsing failures so all 12 journeys produce complete, accurate parse results with zero `parse_errors`. After this section, J8 (generics) parses all monomorphized functions.

**Context:** The `ir_parser.py` regex `_FUNC_NAME_RE = re.compile(r'@([\w.$]+)\s*\(')` cannot parse quoted LLVM function names like `@"_ori_first$24m$24int_int"` produced by monomorphized generics. J8 works around this (enough unquoted functions exist to compute scores), but it's a known gap documented in `plans/code-journeys/overview.md`.

> **WARNING (file size):** `ir_parser.py` is already **503 lines** -- at the limit.
> Adding quoted name support (+20 lines) and invoke handling (+30 lines) will
> push it to ~550+ lines. As a prerequisite step for this section, split
> `ir_parser.py` into:
> - `ir_parser.py` (~300 lines) — Module/Function/Block data classes + `parse_module()` entry point
> - `ir_parser_internal.py` (~250 lines) — regex patterns, line-level parsing helpers, instruction classification
>
> This split also makes it easier to test individual parsing functions in isolation.

**Depends on:** Nothing (independent fix).

---

## 04.0 Prerequisite: Split ir_parser.py

**File(s):** `.claude/skills/code-journey/ir_parser.py` (split into two files)

- [ ] Split `ir_parser.py` into `ir_parser.py` (data classes + public API) and `ir_parser_internal.py` (regex patterns + line-level helpers)
- [ ] Update all imports in `arc_metrics.py`, `attribute_metrics.py`, `control_flow_metrics.py`, `instruction_metrics.py`, `extract-metrics.py` — these import from `ir_parser`, which should remain the public API module
- [ ] Verify `python3 -m pytest tests/test_ir_parser.py` passes after split

---

## 04.1 Quoted Function Names

**File(s):** `.claude/skills/code-journey/ir_parser.py`

LLVM uses quoted names when the identifier contains characters not valid in bare identifiers. Ori's mangling uses `$` which is valid, but some backends quote names with special characters.

- [ ] Update `_FUNC_NAME_RE` to handle both bare and quoted names:
  ```python
  # Before:
  _FUNC_NAME_RE = re.compile(r'@([\w.$]+)\s*\(')

  # After: handles @name( and @"name"(
  # Note: use [\w.$]+ for bare names (not \S+? which would match a single char)
  _FUNC_NAME_RE = re.compile(r'@(?:"([^"]+)"|([\w.$]+))\s*\(')
  ```

- [ ] Update `_parse_function_header()` to extract from the correct capture group:
  ```python
  name_match = _FUNC_NAME_RE.search(stripped)
  if not name_match:
      return None
  # Group 1 = quoted name, Group 2 = bare name
  func_name = name_match.group(1) or name_match.group(2)
  raw_name = f'@"{func_name}"' if name_match.group(1) else f"@{func_name}"
  ```

- [ ] **Fix `name` field for quoted functions**: The `Function.name` field (without `@`) must strip quotes too. With the proposed regex, `group(1)` already gives the unquoted name. But `raw_name.lstrip('@')` in `_parse_function_decl` produces `'"name"'` (with quotes) for quoted names. Use `func_name` (from the regex group) directly instead of stripping from `raw_name`. This affects both `_parse_function_def` and `_parse_function_decl` since they share `_parse_function_header`.

- [ ] **Fix property predicates for quoted names**: Properties that use `raw_name.startswith(...)` break with quoted names (e.g., `@"_ori_..."` does not start with `@_ori_`). Fix each to use `self.name` (without `@` or quotes):
  - `is_user_function`: `self.raw_name.startswith("@_ori_")` — fails for `@"_ori_..."`. Fix: use `self.name.startswith("_ori_")`
  - `is_runtime_decl`: `self.raw_name.startswith("@ori_")` — fails for `@"ori_..."`. Fix: use `self.name.startswith("ori_")`
  - `is_entry_called`: `self.raw_name == "@_ori_main"` — OK (main is never quoted)
  - `is_llvm_intrinsic`: `self.raw_name.startswith("@llvm.")` — OK (intrinsics never quoted)

- [ ] Add test with quoted name IR:
  ```python
  def test_quoted_function_name():
      ir = 'define fastcc i64 @"_ori_first$24m$24int_int"(i64 %0) {\nentry:\n  ret i64 %0\n}\n'
      module = parse_module(ir)
      assert '@"_ori_first$24m$24int_int"' in module.functions
      func = module.functions['@"_ori_first$24m$24int_int"']
      assert func.name == '_ori_first$24m$24int_int'  # No quotes in .name
      assert func.is_user_function  # Starts with _ori_
  ```

---

## 04.2 Multi-line Instruction Handling

**File(s):** `.claude/skills/code-journey/ir_parser.py`

The multi-line `switch` fix (already done) should be generalized for other multi-line constructs.

- [ ] Audit for other multi-line patterns in LLVM IR:
  - `phi` with many incoming values (wraps across lines)
  - `landingpad` with multiple `catch`/`filter` clauses
  - `invoke` with long `to`/`unwind` labels that may wrap

- [ ] If any are found in journey IR, add continuation-line joining (same pattern as switch fix)

- [ ] **Parse `invoke` as a first-class instruction**: `invoke` has a different syntax from `call` and is currently not handled by the RC counting regexes (`_RC_INC_RE`/`_RC_DEC_RE` only match `call`, not `invoke`; `_RC_INVOKE_RE` exists but is never used for balance counting):
  ```
  %result = invoke fastcc i64 @func(i64 %0) to label %normal unwind label %cleanup
  ```
  The parser must:
  1. Recognize `invoke` as an opcode (currently it does extract it as opcode, but the downstream consumers ignore it)
  2. Extract the callee name from `invoke` instructions (same as `call`)
  3. Extract the `to label %X unwind label %Y` targets for CFG construction
  ```python
  _INVOKE_RE = re.compile(
      r'invoke\b.*@(?:"([^"]+)"|(\S+?))\s*\('  # callee name
  )
  _INVOKE_TARGETS_RE = re.compile(
      r'to\s+label\s+%(\S+)\s+unwind\s+label\s+%(\S+)'
  )
  ```

- [ ] **Update `arc_metrics.py`**: The `_RC_INVOKE_RE` already exists but is NEVER used in `_count_rc_ops()`. Either:
  - Extend `_RC_INC_RE` / `_RC_DEC_RE` to also match `invoke.*@ori_rc_inc` etc., OR
  - Add a separate count for invoke-based RC operations
  - **Verify**: Does Ori's codegen ever emit `invoke @ori_rc_inc`? If not (which is likely — RC functions don't unwind), this is defensive hardening only.

- [ ] **Update `extract_branch_targets()` in `ir_utils.py`**: Currently only handles `br` and `switch`. Must also handle `invoke` targets for correct CFG construction in control_flow_metrics.py and the new rc_state.py (Section 02).

---

## 04.3 Completion Checklist

- [ ] `ir_parser.py` split into `ir_parser.py` + `ir_parser_internal.py` (each <=400 lines)
- [ ] `_FUNC_NAME_RE` handles both `@name` and `@"name"` patterns
- [ ] `invoke` instructions parsed with callee extraction and target extraction
- [ ] `extract_branch_targets()` handles invoke `to`/`unwind` targets
- [ ] `arc_metrics.py` counts RC ops in both `call` and `invoke` instructions
- [ ] J8 (generics): all monomorphized functions parsed (0 parse errors)
- [ ] All 12 journeys: 0 `parse_errors` in output
- [ ] Tests cover: bare names, quoted names, names with `$`, empty module, invoke instructions
- [ ] `python3 -m pytest tests/test_ir_parser.py` passes

**Exit Criteria:** `parse_module()` on J8's IR returns a `Module` with zero `parse_errors` and includes all monomorphized function definitions (verified by count matching `grep -c '^define' ir.txt`).
