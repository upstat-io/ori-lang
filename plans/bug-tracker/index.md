---
parallel: true
name: "Bug Tracker"
full_name: "Ori Compiler Bug Tracker"
status: active
---

# Bug Tracker Index

> **Maintenance Notice:** This is a permanent parallel plan. It is always open and never archived. Bugs are organized by compiler subsystem. Update keyword clusters when adding bugs to new areas.

## How to Use

1. Search this file (Ctrl+F) for keywords related to the bug
2. Find the subsystem section
3. Open the section file to see open bugs
4. Use `/add-bug` to file new bugs, `/review-bugs` to triage

---

## Keyword Clusters by Section

### Section 01: Parser & Lexer
**File:** `section-01-parser-lexer.md` | **Status:** No Open Bugs

```
parser, lexer, tokenizer, syntax error
parse error, unexpected token, grammar
AST, token, precedence, EBNF
ori_parse, ori_lexer
```

---

### Section 02: Type Checker
**File:** `section-02-typeck.md` | **Status:** No Open Bugs

```
type checker, type inference, unification
type error, constraint, generics, bounds
trait resolution, method dispatch
ori_types, infer, check
```

---

### Section 03: Evaluator
**File:** `section-03-eval.md` | **Status:** No Open Bugs

```
evaluator, interpreter, runtime value
eval error, method dispatch, iterator
closure, pattern match, control flow
ori_eval, ori_patterns
```

---

### Section 04: Codegen & LLVM
**File:** `section-04-codegen-llvm.md` | **Status:** No Open Bugs

```
codegen, LLVM, IR, JIT, AOT
code generation, lowering, monomorphization
inkwell, basic block, phi node
ori_llvm, ori_arc, ARC pipeline
```

---

### Section 05: Runtime & ARC
**File:** `section-05-runtime-arc.md` | **Status:** No Open Bugs

```
runtime, ARC, reference counting, memory
COW, copy-on-write, slice, buffer
leak, double-free, use-after-free
ori_rt, ori_rc, AIMS
```

---

### Section 06: Stdlib
**File:** `section-06-stdlib.md` | **Status:** No Open Bugs

```
stdlib, standard library, prelude
collections, iterator, string
Option, Result, traits, derive
library/std, ori_registry
```

---

### Section 07: Tooling & CLI
**File:** `section-07-tooling-cli.md` | **Status:** No Open Bugs

```
CLI, tooling, formatter, test runner
ori run, ori check, ori test, ori fmt
diagnostic, error message, warning
oric, ori_fmt, ori_diagnostic
```

---

### Section 08: Spec & Docs
**File:** `section-08-spec-docs.md` | **Status:** No Open Bugs

```
spec, specification, grammar, EBNF
documentation, design doc, proposal
CLAUDE.md, rules, roadmap, plans
docs/ori_lang, .claude/rules
```

---

## Quick Reference

| ID | Subsystem | File |
|----|-----------|------|
| 01 | Parser & Lexer | `section-01-parser-lexer.md` |
| 02 | Type Checker | `section-02-typeck.md` |
| 03 | Evaluator | `section-03-eval.md` |
| 04 | Codegen & LLVM | `section-04-codegen-llvm.md` |
| 05 | Runtime & ARC | `section-05-runtime-arc.md` |
| 06 | Stdlib | `section-06-stdlib.md` |
| 07 | Tooling & CLI | `section-07-tooling-cli.md` |
| 08 | Spec & Docs | `section-08-spec-docs.md` |
