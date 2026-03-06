---
section: "04"
title: "Attribute Metrics"
status: complete
goal: "Check attribute presence/correctness per function against a deterministic checklist"
depends_on: ["01"]
sections:
  - id: "04.1"
    title: "Attribute Checklist Algorithm"
    status: complete
  - id: "04.2"
    title: "Wrong Attribute Detection"
    status: not-started
  - id: "04.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Attribute Metrics

**Status:** Not Started
**Goal:** Deterministically check which LLVM attributes are applicable to each function and whether they are correctly applied, producing the compliance percentage for `score.py`.

**Context:** Attributes like `nounwind`, `fastcc`, `noreturn`, `cold` are objectively checkable — either the attribute is present on the function or it isn't. The only judgment call is "is this attribute applicable?" — which we can also make algorithmic based on function properties.

**Depends on:** Section 01 (IR parser provides function definitions with resolved attributes).

---

## 04.1 Attribute Checklist Algorithm

**File(s):** `.claude/skills/code-journey/attribute_metrics.py`

For each function, determine which attributes are applicable and whether they're present:

```python
ATTRIBUTE_RULES = [
    # (attr_name, applicable_when, description)
    ("fastcc",    lambda f: f.is_user_function and not f.is_entry_called,
                  "Internal user function not called from C ABI (@_ori_main excluded)"),
    # WARNING: this rule assumes all Ori user functions are nounwind. This is true today
    # (panic paths end in unreachable), but may change if Ori adds structured error handling
    # that unwinds through user functions. If that happens, update this rule to check whether
    # the function contains any unwinding calls.
    ("nounwind",  lambda f: f.is_definition,
                  "All defined functions — user functions are nounwind because panic "
                  "paths end in unreachable (the function itself never propagates an "
                  "exception). The @main wrapper is also nounwind."),
    # uwtable is on user functions (#0) but NOT on @main wrapper (#3)
    ("uwtable",   lambda f: f.is_definition and f.raw_name != "@main",
                  "User-defined functions (Linux x86_64 stack unwinding). "
                  "The @main wrapper does NOT get uwtable."),
    # noundef appears on both return type and parameters in actual IR:
    # define fastcc noundef i64 @_ori_add(i64 noundef %0, i64 noundef %1)
    ("noundef",   lambda f: f.is_definition and f.return_type != "void",
                  "Non-void return AND integer/float parameters (Ori values are always defined)"),
    ("noreturn",  lambda f: f.is_runtime_decl and f.name in NORETURN_FUNCTIONS,
                  "Functions that never return (panic, abort)"),
    ("cold",      lambda f: f.is_runtime_decl and f.name in COLD_FUNCTIONS,
                  "Error/panic path functions"),
    ("memory(argmem: readwrite)",
                  lambda f: f.is_runtime_decl and f.name in RC_MEMORY_FUNCTIONS,
                  "RC functions that only touch argument memory (ori_rc_inc, ori_rc_dec)"),
    ("noalias",   lambda f: f.is_runtime_decl and f.name in NOALIAS_RETURN_FUNCTIONS,
                  "Functions that return newly-allocated (non-aliased) pointers"),
]

# Verified against compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
# ori_panic_format does NOT exist in the runtime
NORETURN_FUNCTIONS = {"ori_panic_cstr", "ori_panic"}
COLD_FUNCTIONS = {"ori_panic_cstr", "ori_panic"}
RC_MEMORY_FUNCTIONS = {"ori_rc_inc", "ori_rc_dec"}
NOALIAS_RETURN_FUNCTIONS = {"ori_rc_alloc"}
```

**Attribute detail from actual IR (verified via `ORI_DUMP_AFTER_LLVM=1`):**
- User functions (`@_ori_add`, `@_ori_main`) get `nounwind uwtable` via attribute group `#0`
- The `@main` wrapper gets `nounwind` only (no `uwtable`)
- Runtime declarations: `ori_panic_cstr` gets `cold noreturn` but explicitly NOT `nounwind` (must unwind for RC cleanup — see `runtime_functions.rs` test: "ori_panic_cstr must NOT be nounwind")
- However, user functions that CALL `ori_panic_cstr` ARE still `nounwind` — because the call is followed by `unreachable`, so the function itself never propagates an exception past its frame

- [ ] For each function, iterate `ATTRIBUTE_RULES` to determine applicable attributes
- [ ] Check presence in the function's resolved attribute set
- [ ] Count `applicable` and `correct` (present when applicable)
- [ ] Compute compliance: `correct / applicable * 100` (handle `applicable == 0` as 100%)

```python
@dataclass
class FunctionAttributeMetrics:
    name: str
    checks: list[tuple[str, bool, bool]]  # (attr, applicable, present)

@dataclass
class AttributeMetrics:
    per_function: list[FunctionAttributeMetrics]
    total_applicable: int
    total_correct: int
    compliance_pct: float
    has_wrong: bool          # Any wrong attribute applied
```

**How to determine `is_entry_called`:** A function is "entry-called" if it's `@_ori_main` (called from the C `@main` wrapper). `@_ori_main` uses the default calling convention (not `fastcc`), because the `@main` wrapper calls it without `fastcc`. In the actual IR: `define noundef i64 @_ori_main()` (no `fastcc` keyword).

- [ ] Implement applicability rules
- [ ] Implement presence checking
- [ ] Verify attribute group resolution covers ALL groups in the IR, not just `#0`. Journey 1 has at least `#0 = { nounwind uwtable }`, `#2 = { cold noreturn }`, `#3 = { nounwind }`. The parser (section 01) resolves group references to per-function attribute sets, but the attribute checker must verify that the resolved attributes are correct for EACH group a function references.
- [ ] Handle attribute groups that combine individual attrs and group refs (e.g., `define ... noundef ... #0` where `noundef` is inline and `#0` is a group ref) — both must be in the resolved set
- [ ] Handle `noundef` on both return type and parameters separately. In actual IR: `define fastcc noundef i64 @_ori_add(i64 noundef %0, i64 noundef %1) #0` — the function-level `noundef` applies to the return type, per-parameter `noundef` applies to each param. Both should count toward compliance.
- [ ] Unit test: Journey 1 with `noreturn` fixed = 100% compliance
- [ ] Unit test: Journey 1 without `noreturn` = <100% compliance
- [ ] Unit test: attribute rules for runtime declarations (e.g., `ori_panic_cstr` must have `noreturn` + `cold` but NOT `nounwind`)
- [ ] Unit test: function with no applicable attributes (e.g., a void-returning function with no special properties) → compliance = 100% (0/0 = 100%)

---

## 04.2 Wrong Attribute Detection

**File(s):** `.claude/skills/code-journey/attribute_metrics.py`

A "wrong" attribute is one that is present but shouldn't be — e.g., `nounwind` on a function that can unwind, or `noreturn` on a function that returns.

- [ ] Check for `noreturn` on functions that contain `ret` instructions
- [ ] Check for `readonly`/`readnone` on functions that call mutating runtime functions
- [ ] Check for `nounwind` on runtime declarations in `MUST_NOT_BE_NOUNWIND` set (e.g., `ori_panic_cstr`, `ori_panic` -- they must unwind for RC cleanup). This is a critical safety check from `runtime_functions.rs` tests.
- [ ] Set `has_wrong = True` if any wrong attribute detected (triggers gate in `score.py`)

**Note:** For Ori's current codegen, wrong attributes are rare. The check is defensive — it guards against future regressions.

---

## 04.3 Completion Checklist

- [ ] Journey 1 (current, with `noreturn` fix) scores `attr_applicable == attr_correct` (100% compliance), `has_wrong=false`. Exact count depends on how many applicable rules fire across all functions in Journey 1's IR — verify against exit criteria below.
- [ ] Attribute rules cover all attributes from SKILL.md checklist
- [ ] Wrong attribute detection catches `noreturn` on a returning function
- [ ] Output matches `score.py` format (`--attr-applicable`, `--attr-correct`, `--attr-has-wrong`)

**Exit Criteria:** `compute_attribute_metrics()` on Journey 1's current IR returns 100% compliance (all applicable attributes present, none wrongly applied). Specifically:
- `@_ori_add`: `fastcc`=YES, `nounwind`=YES, `uwtable`=YES, `noundef`=YES (return + 2 params)
- `@_ori_main`: `fastcc`=NO (entry-called), `nounwind`=YES, `uwtable`=YES, `noundef`=YES (return)
- `@main` wrapper: `nounwind`=YES, `fastcc`=NO (C ABI), `uwtable`=NO
- `@ori_panic_cstr` declaration: `noreturn`=YES, `cold`=YES, `nounwind`=NO (correct — must unwind)

On IR with a deliberately missing `nounwind` on `@_ori_add`, compliance drops from 100% to the correct value. On IR with `nounwind` wrongly added to `@ori_panic_cstr`, `has_wrong` is set to `true`.
