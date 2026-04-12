---
section: "05"
title: "Code Graph: Parser Adapters"
status: not-started
reviewed: false
goal: "Set up tree-sitter parsing infrastructure for all 11 repos with per-language adapters, fallback ladder, and custom query files."
success_criteria:
  - "tree-sitter parses files from all 9 supported languages (Lean .lean excluded)"
  - "languages.yaml defines adapter capabilities per language"
  - "repos.yaml defines include/exclude roots per repo"
  - "Custom tags.scm queries exist for Zig, Haskell, Swift, Koka where official ones are missing"
  - "Full parse of all reference repos completes in <60 seconds"
  - "Parse error rate documented per language with known limitations"
depends_on: []
inspired_by:
  - "tree-sitter official grammars (rust, go, typescript, cpp)"
  - "alex-pinkus/tree-sitter-swift fork"
  - "Sourcegraph SCIP indexers for multi-language symbol extraction"
third_party_review:
  status: none
  updated: null
---

# 05 Code Graph: Parser Adapters

## 05.0 Goal

Set up the tree-sitter parsing infrastructure that all code graph work depends on. This section doesn't extract symbols or import into Neo4j — it ensures every repo can be parsed and that the adapter layer handles multi-language complexity.

## 05.1 Python Dependencies

**Location**: `~/projects/lang_intelligence/.venv/`

- [ ] Install tree-sitter core: `pip install tree-sitter>=0.25.0`
- [ ] Install pip-available grammars: `pip install tree-sitter-rust tree-sitter-go tree-sitter-zig tree-sitter-typescript tree-sitter-haskell tree-sitter-cpp`
- [ ] Build alex-pinkus Swift grammar from source: clone `alex-pinkus/tree-sitter-swift`, build shared library, register with tree-sitter
- [ ] Build Koka grammar from source: clone `koka-community/tree-sitter-koka`, build shared library
- [ ] Verify all grammars load: write a test script that instantiates `Language()` for each grammar
- [ ] Document: Lean .lean files are NOT supported (86% error rate). Lean4 repo is parsed via C++ grammar for runtime code only.
- [ ] Document: Ori uses its own Rust parser (no tree-sitter grammar). Ori code graph pipeline is a separate adapter (Section 09).

### Subsection 05.1 close-out
**`/improve-tooling` retrospective**: Were any grammars difficult to install? Should we add a `setup.sh` script to automate the full dependency installation?

---

## 05.2 Language Adapter Manifests

**File**: `~/projects/lang_intelligence/languages.yaml`

Define per-language capabilities:
```yaml
rust:
  grammar: tree-sitter-rust  # pip package name
  extensions: [".rs"]
  tags_scm: official  # official, custom, or none
  capabilities: [syntax, decls, imports, calls, types, impls]
  maturity: stable
  error_rate: 0.09

go:
  grammar: tree-sitter-go
  extensions: [".go"]
  tags_scm: official
  capabilities: [syntax, decls, imports, calls, types, impls]
  maturity: stable
  error_rate: 0.01

# ... etc for all languages
```

**File**: `~/projects/lang_intelligence/repos.yaml`

Define per-repo include/exclude roots:
```yaml
rust:
  source: ~/projects/reference_repos/lang_repos/rust
  languages: [rust]
  include:
    - compiler/
    - library/
  exclude:
    - tests/
    - vendor/
    - target/
    - "*.generated.*"

swift:
  source: ~/projects/reference_repos/lang_repos/swift
  languages: [swift, cpp]  # mixed-language repo
  include:
    - lib/SILOptimizer/
    - lib/SIL/
    - lib/Sema/
    - include/swift/AST/
  exclude:
    - test/
    - benchmark/
    - validation-test/
```

- [ ] Create `languages.yaml` with all 10 language configs (9 tree-sitter + Ori custom)
- [ ] Create `repos.yaml` with curated include/exclude roots for all 11 repos
- [ ] For each repo, select only compiler-relevant source roots (per Codex's scale constraint)
- [ ] For mixed-language repos (Gleam=Rust, Roc=Rust, Elm=Haskell, Koka=Haskell, Lean4=C++/Lean), list all applicable languages

### Subsection 05.2 close-out
**`/improve-tooling` retrospective**: Were the include/exclude patterns sufficient? Any repos where we included too much or too little?

---

## 05.3 Custom Query Files

For languages without official `tags.scm`, write custom tree-sitter query files:

**Files**: `~/projects/lang_intelligence/queries/{lang}/tags.scm`

- [ ] **Zig** (`queries/zig/tags.scm`): Extract `fn_decl`, `struct_decl`, `enum_decl`, `const_decl`, `call_expression`. Use Zig's `node-types.json` as reference for node type names.
- [ ] **Haskell** (`queries/haskell/tags.scm`): Extract `function`, `signature`, `type_alias`, `data_declaration`, `class_declaration`, `instance_declaration`. Reference: tree-sitter-haskell `src/node-types.json`.
- [ ] **Koka** (`queries/koka/tags.scm`): Extract `fun_decl`, `type_decl`, `effect_decl`, `val_decl`. If tree-sitter-koka grammar is too immature, use Haskell queries for .hs files instead.
- [ ] **Lean** — SKIP. C++ files use the official C++ tags.scm. .lean files are not parseable.
- [ ] Test each custom query against actual repo files to verify capture accuracy

### Subsection 05.3 close-out
**`/improve-tooling` retrospective**: Were the custom queries accurate? Any node types that were unexpectedly named? Should we add a query validation test that checks captures against known files?

---

## 05.4 Parse Validation Script

**File**: `~/projects/lang_intelligence/scripts/validate-parsers.py`

A test script that:
- [ ] Loads each grammar from `languages.yaml`
- [ ] Parses a sample of files from each repo per `repos.yaml`
- [ ] Reports: files parsed, error nodes found, error rate
- [ ] Compares against expected error rates from `languages.yaml`
- [ ] Fails if any language exceeds its expected error rate by >5%
- [ ] Reports total parse time and throughput (lines/sec)

Expected performance baseline (from research):
- All repos combined: ~40 seconds, ~289K lines/sec
- Per-repo parsing should be <5 seconds for small repos, <15 seconds for Rust/Swift

### Subsection 05.4 close-out
**`/improve-tooling` retrospective**: Is the validation script useful for ongoing maintenance? Should it be run as part of a CI-like check for the lang_intelligence repo?

---

## 05.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] All 9 tree-sitter grammars load successfully (7 pip + 2 built from source)
- [ ] `languages.yaml` and `repos.yaml` define all 11 repos
- [ ] Custom queries exist for Zig, Haskell, Koka (at minimum)
- [ ] `validate-parsers.py` passes with all repos within expected error rates
- [ ] Full parse of all repos completes in <60 seconds
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
