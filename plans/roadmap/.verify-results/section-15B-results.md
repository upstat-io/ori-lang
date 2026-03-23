# Section 15B Verification Results: Function Syntax

**Verified**: 2026-03-19
**Section status**: 0/270 (0%) -- not-started
**Sampling strategy**: Spot-checked 10 unchecked items; found partial implementations not tracked by plan

---

## Unchecked Items Sampled (confirming incomplete)

### 15B.1 Remove Dot Prefix from Named Arguments

| Item | Status | Evidence |
|------|--------|----------|
| Parser accepts `IDENTIFIER ':'` (dot removal done) | PLAN INACCURACY | Plan says "Done" in description but checkbox is unchecked. Parser does accept `name: value` syntax -- this is the current production syntax. Should be checked `[x]`. |
| Enforce named arguments for built-in functions | VERIFIED INCOMPLETE | No `tests/spec/expressions/builtin_named_args.ori` exists. Built-ins like `print` already use `msg:` in practice but enforcement (rejecting positional) is not systematic. |
| Update `print` to require `msg:` parameter | VERIFIED INCOMPLETE | No dedicated test file exists. |

### 15B.2 Default Parameter Values

| Item | Status | Evidence |
|------|--------|----------|
| Extend `param` production to accept `= expression` | VERIFIED INCOMPLETE (partial parser) | `ori_parse/src/grammar/item/function/mod.rs` has a `// = default_value (optional)` comment but full default parameter support is not complete. No `tests/spec/declarations/default_params.ori` exists. |

### 15B.3 Multiple Function Clauses

| Item | Status | Evidence |
|------|--------|----------|
| Allow `match_pattern` in parameter position | VERIFIED INCOMPLETE | References to "function clause" exist in 6 parser files (tests, error kinds, etc.) but no `tests/spec/declarations/function_clauses.ori` exists. Infrastructure is partial. |

### 15B.4 Positional Lambdas for Single-Parameter Functions

| Item | Status | Evidence |
|------|--------|----------|
| Check for lambda-literal positional argument exception | VERIFIED INCOMPLETE | No tests or implementation found for this specific exception rule. |

### 15B.5 Argument Punning (Call Arguments)

| Item | Status | Evidence |
|------|--------|----------|
| Parser: `name:` followed by `,` or `)` creates synthetic `Expr::Ident` | PLAN INACCURACY | This IS implemented in `ori_parse/src/grammar/expr/postfix.rs` (line 402: "Argument punning: `f(x:)` desugars to `f(x: x)`"). The code creates a synthetic `Expr::Ident` when colon is followed by comma or rparen. Should be checked `[x]`. |
| Mixed punned and explicit arguments | PLAN INACCURACY | Also implemented -- parser handles mixed cases naturally. No dedicated test file `tests/spec/expressions/argument_punning.ori` exists though. |
| Formatter: detect `name == value_ident` and emit `name:` form | VERIFIED INCOMPLETE | No formatter punning canonicalization found. |

---

## Summary

Plan status of 0% is inaccurate. At least 2-3 items have working implementations:
1. **Argument punning** (15B.5) -- parser implementation complete, no dedicated test file
2. **Dot prefix removal** (15B.1) -- current syntax already uses `name: value` without dots

The remaining items (default parameters, function clauses, positional lambdas, built-in named arg enforcement) are genuinely not implemented.

**Accuracy**: Section progress should be approximately 2-5% (a few parser items are done). Plan needs checkbox updates for items already implemented.

**Plan inaccuracies found**: 2 items described as done/partial in text but unchecked in checkboxes.
