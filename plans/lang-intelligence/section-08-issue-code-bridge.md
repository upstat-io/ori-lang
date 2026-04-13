---
section: "08"
title: "Issue-to-Code Bridge"
status: not-started
reviewed: false
goal: "Extract code references from issue/PR/comment bodies and link them to code symbols via CodeReference intermediary nodes with confidence scoring."
success_criteria:
  - "CodeReference nodes created from file paths, backticked symbols, and qualified names in issue bodies"
  - "MENTIONS_CODE relationships link Issue/Comment/Review to CodeReference"
  - "RESOLVES_TO relationships link CodeReference to File/Symbol (when unambiguous)"
  - "Confidence scores reflect extraction quality (regex path match > backtick > heuristic)"
  - "Stale references preserved (not deleted) with staleness metadata"
depends_on: ["07"]
third_party_review:
  status: none
  updated: null
---

# 08 Issue-to-Code Bridge

## 08.0 Goal

This is the bridge layer that connects the issue graph to the code graph. Without it, issues and code live in separate universes within the same Neo4j instance. The bridge enables the killer queries: "find issues that reference code implementing the same concept as Ori's exhaustiveness checker."

## 08.1 Code Reference Extraction

**File**: `~/projects/lang_intelligence/neo4j/extract_code_refs.py`

Extract code mentions from issue/comment/review bodies using regex patterns:

**Pattern types** (ordered by confidence):
1. **File paths** (confidence: high): `compiler/rustc_parse/src/parser/expr.rs`, `src/Sema/TypeChecker.cpp`
2. **Backticked identifiers** (confidence: medium): `` `check_exhaustiveness` ``, `` `PatternColumn` ``
3. **Qualified names** (confidence: medium): `rustc_pattern_analysis::usefulness::compute_exhaustiveness`
4. **Line references** (confidence: medium-high): `expr.rs:42`, `L123-L156`
5. **Fenced code blocks** (confidence: low): Code snippets that might contain function/type names

- [ ] Implement regex extractors for each pattern type
- [ ] Assign confidence scores per extraction type
- [ ] Deduplicate references within the same issue (same symbol mentioned 5 times = 1 CodeReference)
- [ ] Output JSONL records: `{"issue_repo": "rust", "issue_number": 12345, "raw_text": "check_exhaustiveness", "mention_kind": "backtick", "file_hint": null, "symbol_hint": "check_exhaustiveness", "confidence": 0.7}`

### Subsection 08.1 close-out
**`/improve-tooling` retrospective**: Were the regex patterns accurate? High false positive rate? Any common patterns missed?

---

## 08.2 Reference Resolution

**File**: `~/projects/lang_intelligence/neo4j/resolve_code_refs.py`

Match extracted references to actual code symbols in Neo4j:

- [ ] **File path resolution**: Match extracted path against `File.path` nodes in the same repo
- [ ] **Symbol resolution**: Match extracted identifier against `Symbol.name` or `Symbol.qualified_name`
- [ ] **Ambiguity handling**: If a backticked name matches 5+ symbols, mark as `ambiguous` rather than creating 5 RESOLVES_TO edges
- [ ] **Cross-repo awareness**: An issue in `rust-lang/rust` referencing `compiler/rustc_parse/src/parser/expr.rs` resolves within the `rust` repo
- [ ] Create `CodeReference` nodes with: raw_text, mention_kind, confidence, extracted_from (issue/comment/review ID)
- [ ] Create `MENTIONS_CODE` edges from Issue/Comment/Review to CodeReference
- [ ] Create `RESOLVES_TO` edges from CodeReference to File/Symbol (when confidence >= threshold)
- [ ] Unresolved references: keep the CodeReference node without RESOLVES_TO (for future resolution when more code is indexed)
- [ ] **Module-level source resolution**: Create synthetic module-level Symbol nodes for files that emit relationships but have zero structural symbols from decls.scm (e.g., Haskell modules, C/C++ headers). These files produce IMPORTS/CALLS records with `source_qualified_name` that can't resolve to any Symbol node at import time — Section 07 tracks them as `source_unresolved`. Fix here or in Section 06's extract_symbols.py by emitting a file-scope symbol record when relationships exist but no declaration symbols do. <!-- unblocks:07.2 source_unresolved gap -->

### Subsection 08.2 close-out
**`/improve-tooling` retrospective**: What's the resolution success rate? What fraction of references resolve unambiguously? Should we lower/raise the confidence threshold?

---

## 08.3 Ontology Seeding

This is where the Concept, FailureMode, and CompilerPhase taxonomy nodes get created.

**File**: `~/projects/lang_intelligence/neo4j/seed_ontology.py`

**Start narrow** — 5 core concepts, 5 compiler phases, 10 failure modes:

**Concepts** (per ChatGPT + TPR consensus):
- pattern_matching, type_inference, reference_counting, effect_handling, diagnostics

**Compiler phases**:
- parser, typechecker, lowering, codegen, diagnostics

**Failure modes**:
- soundness_hole, inference_ambiguity, diagnostic_confusion, compile_time_blowup, pattern_incompleteness, coherence_conflict, monomorphization_explosion, ir_mismatch, codegen_regression, parser_ambiguity

- [ ] Create Concept nodes with aliases/synonyms (e.g., "pattern_matching" aliases: "match", "switch", "case", "exhaustiveness", "usefulness")
- [ ] Create CompilerPhase nodes
- [ ] Create FailureMode nodes with descriptions
- [ ] Auto-tag Symbols with Concepts based on: file path patterns, symbol names, module names
- [ ] Auto-tag Issues with FailureModes based on: labels, title keywords, body keywords
- [ ] Create TAGGED_AS edges (Symbol→Concept), INTRODUCES_FAILURE_MODE edges (Issue→FailureMode)
- [ ] Test: `MATCH (c:Concept {name: 'pattern_matching'})<-[:TAGGED_AS]-(s:Symbol) RETURN count(s)` returns non-zero for repos that have pattern matching code

### Subsection 08.3 close-out
**`/improve-tooling` retrospective**: Were the auto-tagging heuristics accurate? Too many false tags? Need manual override mechanism?

---

## 08.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] Code references extracted from issue/comment/review bodies
- [ ] CodeReference nodes created with confidence scores
- [ ] RESOLVES_TO edges link references to File/Symbol nodes
- [ ] Ontology seeded with Concept, FailureMode, CompilerPhase nodes
- [ ] Auto-tagging produces meaningful TAGGED_AS and INTRODUCES_FAILURE_MODE edges
- [ ] Bridge queries work: `MATCH (i:Issue)-[:MENTIONS_CODE]->(cr)-[:RESOLVES_TO]->(s:Symbol) RETURN count(i)`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
