---
section: "06"
title: "Code Graph: Symbol Extraction"
status: not-started
reviewed: false
goal: "Extract structural symbols (Module, Function, Struct, Trait, Method) and relationships (CALLS, IMPORTS, IMPLEMENTS) from tree-sitter ASTs into a normalized intermediate format."
success_criteria:
  - "extract_symbols.py produces JSON symbol records from any supported language"
  - "Symbol records contain: qualified_name, kind, language, language_kind, file, line, signature_hash"
  - "Relationship records contain: source_symbol, target_symbol, kind (CALLS/IMPORTS/IMPLEMENTS)"
  - "Cross-language normalization: Rust traits and Go interfaces both produce kind=trait_like"
  - "No expression or statement-level nodes — structural only"
depends_on: ["05"]
third_party_review:
  status: none
  updated: null
---

# 06 Code Graph: Symbol Extraction

## 06.0 Goal

Transform tree-sitter ASTs into normalized symbol and relationship records. This is the bridge between raw parsing (Section 05) and Neo4j import (Section 07). The output is language-neutral JSON — the same format regardless of whether the input is Rust, Go, or Haskell.

## 06.1 Symbol Schema

**Normalized symbol kinds** (closed set, cross-language):

| Kind | Rust | Go | Zig | TypeScript | Haskell | Swift |
|------|------|-----|-----|------------|---------|-------|
| `module` | `mod` | package | - | module | module | - |
| `function` | `fn` | `func` | `fn` | `function` | top-level fn | `func` |
| `method` | `fn` in impl | method | method | method | - | method |
| `type` | `struct` | `struct` | `struct` | `class`/`interface` | `data` | `class`/`struct` |
| `sum_type` | `enum` | - | `union` | `enum` | `data` variants | `enum` |
| `trait_like` | `trait` | `interface` | - | `interface` | `class` (typeclass) | `protocol` |
| `impl_block` | `impl T: Trait` | implicit | - | `implements` | `instance` | `extension` conformance |
| `field` | struct field | struct field | field | property | - | property |
| `variant` | enum variant | - | union field | enum case | constructor | case |
| `const` | `const`/`static` | `const`/`var` | `const` | `const` | - | `let` |
| `type_alias` | `type X = Y` | `type X = Y` | `const type` | `type` | `type` | `typealias` |

- [ ] Define the symbol kind enum in `extract_symbols.py`
- [ ] Map each language's tree-sitter node types to the normalized kinds
- [ ] Preserve `language_kind` as the original language-specific kind name

## 06.2 Extraction Script

**File**: `~/projects/lang_intelligence/neo4j/extract_symbols.py`

**Contract**:
```
Usage: python3 neo4j/extract_symbols.py <repo_name> [--output symbols.jsonl]
Consumes: parser_adapter.parse_repo() -> Iterator[ParseResult]
  (ParseResult includes: source_bytes, tree, query_handles for decls/calls/imports/impls)
Outputs: JSONL with one record per symbol or relationship
```

**Note:** This script does NOT read `repos.yaml`, `languages.yaml`, or query `.scm` files directly. All grammar loading, file walking, query compilation, and error handling is the responsibility of Section 05's `parser_adapter.py`. This script operates on `ParseResult` objects — it extracts symbols from pre-parsed trees using pre-compiled query handles.

**Symbol record format**:
```json
{"type": "symbol", "repo": "rust", "file": "compiler/rustc_parse/src/parser/expr.rs",
 "name": "parse_expr", "qualified_name": "rustc_parse::parser::expr::parse_expr",
 "kind": "function", "language": "rust", "language_kind": "fn_item",
 "line": 42, "end_line": 120, "visibility": "pub",
 "signature_hash": "a1b2c3d4"}
```

**Relationship record format**:
```json
{"type": "relationship", "kind": "CALLS",
 "source": "rustc_parse::parser::expr::parse_expr",
 "target": "rustc_parse::parser::item::parse_item",
 "repo": "rust", "file": "compiler/rustc_parse/src/parser/expr.rs", "line": 67}
```

- [ ] Implement per-language extractors that consume `ParseResult.query_handles` from `parser_adapter.py`
- [ ] Use `query_handles["decls"]` for declaration extraction (all languages have this family)
- [ ] Use `query_handles["calls"]` for CALLS relationships (official captures for Rust/Go; custom for others)
- [ ] Use `query_handles["imports"]` for IMPORTS relationships
- [ ] Use `query_handles["impls"]` for IMPLEMENTS relationships (empty stubs for Go/Zig/C++ return zero captures — this is expected)
- [ ] Fall back to programmatic tree walking against `ParseResult.tree` only when a query family is intentionally stubbed/partial or the adapter is native/custom (Ori)
- [ ] Compute qualified_name by walking parent module/namespace chain
- [ ] Compute signature_hash as a stable fingerprint for change detection (used by live sync)
- [ ] Output JSONL (streaming, not buffered) for memory efficiency on large repos

### Subsection 06.2 close-out
**`/improve-tooling` retrospective**: Was the extraction accurate? Any language-specific quirks that need special handling? Any false positives in CALLS extraction (e.g., matching variable names that happen to look like function calls)?

---

## 06.3 Cross-Language Normalization Tests

- [ ] Test: Rust `trait Foo` and Go `interface Foo` both produce `kind: trait_like`
- [ ] Test: Rust `impl Bar: Foo` and Go method set both produce `kind: impl_block` with IMPLEMENTS edge
- [ ] Test: Function calls in all languages produce CALLS relationships
- [ ] Test: Import statements in all languages produce IMPORTS relationships
- [ ] Test: qualified_name is deterministic (same file → same qualified_name on re-parse)
- [ ] Test: signature_hash changes when function signature changes but not when body changes

### Subsection 06.3 close-out
**`/improve-tooling` retrospective**: Were the normalization tests sufficient? Any edge cases in qualified_name computation (e.g., anonymous modules, re-exports)?

---

## 06.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] `extract_symbols.py` produces JSONL for all 9 supported languages
- [ ] Symbol kinds normalized across languages per the mapping table
- [ ] CALLS, IMPORTS, IMPLEMENTS relationships extracted
- [ ] qualified_name and signature_hash computed correctly
- [ ] Normalization tests pass
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
