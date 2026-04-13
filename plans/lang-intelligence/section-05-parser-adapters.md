---
section: "05"
title: "Code Graph: Parser Adapters"
status: not-started
reviewed: false
goal: "Set up tree-sitter parsing infrastructure for all 11 repos with per-language adapters, query file families, adapter API contract, and validation matrix."
success_criteria:
  - "tree-sitter parses files from all 9 supported languages (Lean .lean excluded, coverage: partial)"
  - "languages.yaml defines adapter capabilities, coverage_status, and pinned grammar versions per language"
  - "repos.yaml defines include/exclude roots per repo with canonicalized repo_id vs source_root vs issue_root"
  - "Query file families (decls.scm, calls.scm, imports.scm, impls.scm) exist for ALL languages — even if some start with only decls"
  - "Parser adapter API exposes: repo_id, language_id, relative_path, source_bytes, byte_count, tree, had_error, error_node_count, query_handles, coverage_status, content_hash"
  - "Full parse of all reference repos completes in <60 seconds"
  - "Parse error rate documented per language with known limitations"
  - "Matrix validation passes: Language x (Valid/Malformed/Empty) x query family"
  - "Section 06 can consume adapter output without additional parsing or transformation"
depends_on: []
inspired_by:
  - "tree-sitter official grammars (rust, go, typescript, cpp)"
  - "alex-pinkus/tree-sitter-swift fork"
  - "Sourcegraph SCIP indexers for multi-language symbol extraction"
third_party_review:
  status: resolved
  updated: 2026-04-12
sections:
  - id: "05.1"
    title: "Python Dependencies & Version Compatibility"
    status: not-started
  - id: "05.2"
    title: "Language Adapter Manifests"
    status: not-started
  - id: "05.3"
    title: "Parser Adapter API Contract"
    status: not-started
  - id: "05.4"
    title: "Query File Families"
    status: not-started
  - id: "05.5"
    title: "Parse Validation & Matrix Testing"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# 05 Code Graph: Parser Adapters

## 05.0 Goal

Set up the tree-sitter parsing infrastructure that all code graph work depends on. This section delivers three things: (1) reliable grammar loading for all 9 supported languages, (2) a formal adapter API contract that Section 06 consumes, and (3) query file families (not just `tags.scm`) that prepare for relationship extraction. The section does NOT extract symbols or import into Neo4j — it ensures every repo can be parsed and that the adapter layer exposes everything downstream sections need.

**Success Criteria:**

- [ ] All 9 tree-sitter grammars load successfully with pinned, compatible versions
- [ ] Parser adapter API contract exposes `repo_id`, `language_id`, `relative_path`, `source_bytes`, `byte_count`, `tree`, `had_error`, `error_node_count`, `query_handles`, `coverage_status`, `content_hash`
- [ ] Query file families (`decls.scm`, `calls.scm`, `imports.scm`, `impls.scm`) exist for every supported language
- [ ] Matrix validation: Language x (Valid/Malformed/Empty) x query family all pass
- [ ] Full parse of all reference repos completes in <60 seconds
- [ ] Unblocks mission criteria: "tree-sitter parses all 9 supported languages" (parsing half — extraction is Section 06's deliverable)

**Context:** Section 06 (Symbol Extraction) needs more than just parse trees — it needs compiled query handles for declarations, calls, imports, and implementations. If Section 05 only delivers `tags.scm` parsing, Section 06 must reinvent query infrastructure. This section front-loads that work.

**Reference implementations:**
- **Sourcegraph SCIP**: Multi-language indexing with per-language adapter pattern
- **nvim-treesitter**: Query file organization (`queries/{lang}/{tags,highlights,locals}.scm`)

**Depends on:** None (independent pillar start).

---

## 05.1 Python Dependencies & Version Compatibility

**File(s):** `~/projects/lang_intelligence/.venv/`, `~/projects/lang_intelligence/requirements.txt`

Grammar packages pin different tree-sitter core versions. A blanket `pip install tree-sitter>=0.25.0` will fail or produce ABI mismatches. The correct approach: pin exact versions after a compatibility smoke test.

**Modern tree-sitter Python API (0.22+):** The `build_library()` / `Language()` pattern from pre-0.22 is deprecated. In tree-sitter 0.22+, grammar packages expose a `language()` function directly:

```python
# Modern API (tree-sitter >= 0.22)
import tree_sitter_rust
from tree_sitter import Language, Parser

RUST = Language(tree_sitter_rust.language())
parser = Parser(RUST)
tree = parser.parse(source_bytes)
```

There is NO shared library building step. Grammar packages are Python modules with compiled bindings.

- [ ] Create `requirements.txt` with exact pinned versions. Start with latest compatible set and run smoke test:
  ```
  tree-sitter==0.23.2        # Core — pick version compatible with ALL grammar packages
  tree-sitter-rust==0.23.3
  tree-sitter-go==0.23.4
  tree-sitter-zig==1.1.2
  tree-sitter-typescript==0.23.2
  tree-sitter-haskell==0.23.1
  tree-sitter-cpp==0.23.4
  ```
  **Version selection rule:** Start with the 0.23.x family where most grammars have releases. If a grammar only has 0.21.x, test whether it loads with core 0.23.x (ABI may be compatible). Document the result.
- [ ] Run compatibility smoke test: for each grammar package, `Language(mod.language())` must succeed, and `Parser(lang).parse(b"")` must return a tree without segfault
- [ ] Record the compatibility matrix in `requirements.txt` comments:
  ```
  # Compatibility matrix (verified YYYY-MM-DD):
  # tree-sitter-rust 0.23.3 + core 0.23.2: OK
  # tree-sitter-go   0.23.4 + core 0.23.2: OK
  # ...
  ```
- [ ] Swift grammar: Try `tree-sitter-swift==0.0.1` from PyPI first (exists on PyPI). If it loads successfully with the pinned core version, use it. Fall back to alex-pinkus source build only if the PyPI package fails the smoke test. Document which path was taken.
- [ ] Koka grammar: NOT on PyPI. Clone `koka-community/tree-sitter-koka` and install from source:
  ```bash
  KOKA_TMP=$(mktemp -d)
  git clone https://github.com/koka-community/tree-sitter-koka.git "$KOKA_TMP/tree-sitter-koka"
  cd "$KOKA_TMP/tree-sitter-koka" && pip install .
  rm -rf "$KOKA_TMP"
  ```
  If installation fails: (1) file via `/add-bug` with subsystem `lang-intelligence`, severity `medium`, and repro steps; (2) mark Koka coverage as `partial` in `languages.yaml` (use Haskell grammar for `.hs` files only).
- [ ] Verify all grammars load: create `scripts/validate-parsers.py` early (the permanent validation tool from 05.5) with at least `--smoke` mode that instantiates `Language()` for each grammar, runs `Parser(lang).parse(b"x")`, and reports success/failure. Do NOT create a separate `verify-grammars.py` — one tool, extended incrementally.
- [ ] Document: Lean `.lean` files have 86% parse error rate. Lean4 repo is parsed via C++ grammar for runtime code only. Coverage status: `partial` (not "unsupported" — some of the repo IS parseable via C++ grammar).
- [ ] Document: Ori uses its own Rust parser (no tree-sitter grammar). Ori adapter is implemented in Section 09.3. Ori MUST appear in `languages.yaml` with `grammar: native` and `coverage_status: custom`. Note: `validate-parsers.py` MUST skip `coverage_status: custom` entries (they use non-tree-sitter adapters implemented in other sections). No `blocked-by` annotation — Section 05 is complete without the Ori adapter; Section 09 builds on top.
- [ ] Create `scripts/setup-parsers.sh` that automates: venv creation, `pip install -r requirements.txt`, Koka source build (if needed), `validate-parsers.py --smoke` run. This script is the reproducible setup path for any new developer.

- [ ] **Subsection close-out (05.1)**
  - [ ] All tasks above are `[x]` and all grammars load via smoke test
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the dependency installation journey: were version conflicts hard to diagnose? Should `verify-grammars.py` report more detail (e.g., ABI version, core version detected)? Should `setup-parsers.sh` have a `--verbose` flag? Implement improvements, commit separately.

---

## 05.2 Language Adapter Manifests

**File(s):** `~/projects/lang_intelligence/languages.yaml`, `~/projects/lang_intelligence/repos.yaml`

These manifests are the single source of truth for the entire code graph pipeline. Every downstream script (Section 06 extraction, Section 07 import, Section 09 sync) reads them. Getting the schema right here prevents cascading fixes later.

**`languages.yaml` — per-language capabilities:**

```yaml
rust:
  grammar: tree-sitter-rust       # pip package name or "source" or "native"
  grammar_version: "0.23.3"       # pinned version (must match requirements.txt)
  extensions: [".rs"]
  query_families:                  # which .scm query files exist for this language
    - decls                        # declarations (functions, types, traits, etc.)
    - calls                        # call sites
    - imports                      # use/import statements
    - impls                        # impl/instance/conformance blocks
  coverage_status: full            # full | partial | custom
  maturity: stable
  expected_error_rate: 0.09
  notes: ""

# Ori — native parser, not tree-sitter
ori:
  grammar: native
  extensions: [".ori"]
  query_families: []               # N/A — uses Ori's own Rust parser via FFI
  coverage_status: custom
  maturity: stable
  expected_error_rate: 0.0
  notes: "Parsed by ori_parse (Rust). Adapter in Section 09."

lean:
  grammar: tree-sitter-cpp         # .lean files skipped; only C++ runtime parsed
  extensions: [".cpp", ".h"]       # NOT .lean
  query_families: [decls, calls, imports, impls]  # all C++ query families
  coverage_status: partial
  maturity: stable
  expected_error_rate: 0.02
  notes: ".lean files have 86% error rate — skipped. C++ runtime code only."
```

**`repos.yaml` — per-repo source mapping:**

The local corpus has both `go/` (issue tracker only) and `golang/` (source code). The manifest MUST canonicalize these:

```yaml
go:
  repo_id: go                                    # canonical ID used in Neo4j
  source_root: ${REFERENCE_REPOS_ROOT}/golang    # resolved at runtime by adapter
  issue_root: ${REFERENCE_REPOS_ROOT}/go         # resolved at runtime by adapter
  languages: [go]
  include:
    - cmd/compile/
    - go/types/
    - internal/types/
  exclude:
    - test/
    - vendor/
```

- [ ] Create `languages.yaml` with all 10 language configs (9 tree-sitter + Ori native), including:
  - `grammar` (pip package name, `"source"`, or `"native"`)
  - `grammar_version` (pinned, matching `requirements.txt`)
  - `extensions` (file extensions to match)
  - `query_families` (list of query family names this language has `.scm` files for)
  - `coverage_status` (`full` | `partial` | `custom`)
  - `maturity`, `expected_error_rate`, `notes`
- [ ] Create `repos.yaml` with curated include/exclude roots for all 11 repos, including:
  - `repo_id` (canonical Neo4j identifier — resolves the `go` vs `golang` duality)
  - `source_root` (env-var pattern: `${REFERENCE_REPOS_ROOT}/golang` — resolved at runtime by the adapter, NOT hardcoded absolute paths)
  - `issue_root` (env-var pattern: `${REFERENCE_REPOS_ROOT}/go` — resolved at runtime, for repos where issue tracker data is in a different directory)
  - `languages` (list of language IDs from `languages.yaml`)
  - `include` / `exclude` patterns
- [ ] Canonicalize the `go`/`golang` duality: `repo_id: go`, `source_root` pointing to `golang/`, `issue_root` pointing to `go/`
- [ ] For mixed-language repos, list all applicable languages:
  - Gleam: `[rust]` (compiler is Rust)
  - Roc: `[rust]` (compiler is Rust)
  - Elm: `[haskell]` (compiler is Haskell)
  - Koka: `[haskell]` (compiler is Haskell) — `.kk` files parsed separately if grammar works
  - Lean4: `[cpp]` (runtime only — `.lean` files skipped)
  - Swift: `[swift, cpp]` (mixed)
- [ ] Validate: every `languages:` entry in `repos.yaml` references a valid key in `languages.yaml`
- [ ] Validate: every `source_root` path exists on disk

- [ ] **Subsection close-out (05.2)**
  - [ ] All tasks above are `[x]` and both manifests validate
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — were include/exclude patterns sufficient? Any repos where scope was wrong? Should there be a `validate-manifests.py` script?

---

## 05.3 Parser Adapter API Contract

**File(s):** `~/projects/lang_intelligence/neo4j/parser_adapter.py`

The parser adapter is the boundary between "raw tree-sitter" and "everything downstream." Section 06 (extraction), Section 07 (import), and Section 09 (sync) all consume this API. The contract must be explicit, typed, and documented.

**Adapter output contract (per file):**

```python
@dataclass
class ParseResult:
    repo_id: str               # canonical repo identifier from repos.yaml
    language_id: str           # language key from languages.yaml
    relative_path: str         # path relative to source_root
    source_bytes: bytes        # raw file content (needed by Section 06 for qualified names, signature_hash)
    byte_count: int            # len(source_bytes)
    tree: Tree | None          # tree-sitter Tree (None on load failure)
    had_error: bool            # True if tree contains ERROR nodes
    error_node_count: int      # count of ERROR nodes in tree
    query_handles: dict[str, Query]  # compiled queries by family name
    coverage_status: str       # "full" | "partial" | "custom"
    content_hash: str          # SHA-256 of file content (for incremental sync)

class CoverageStatus(Enum):
    FULL = "full"              # grammar parses this language well
    PARTIAL = "partial"        # grammar has known gaps (e.g., Lean C++ only)
    CUSTOM = "custom"          # not tree-sitter (e.g., Ori native parser)
```

**Error handling policy:**
- Per-file parse failures (I/O error, encoding error): **soft** — skip file, log warning, continue. A single bad file must NOT abort the pipeline.
- Grammar load failures (missing package, ABI mismatch): **hard** — abort immediately with clear error message. A broken grammar affects ALL files for that language.
- Query compilation failures (malformed `.scm` file): **hard** — abort immediately. A broken query produces wrong extraction results silently.

- [ ] Implement `ParseResult` dataclass with all fields listed above
- [ ] Implement `CoverageStatus` enum
- [ ] Implement `parse_file(repo_config, lang_config, file_path) -> ParseResult` that:
  - Loads grammar from `languages.yaml` config
  - Reads file bytes (soft-fail on I/O/encoding errors)
  - Parses with tree-sitter
  - Counts ERROR nodes
  - Compiles and attaches query handles for all query families listed in `languages.yaml`
  - Computes SHA-256 content hash (for Section 09 incremental sync)
- [ ] Implement `parse_repo(repo_id) -> Iterator[ParseResult]` that:
  - Reads `repos.yaml` for include/exclude patterns
  - Walks the file tree, filtering by extensions from `languages.yaml`
  - Calls `parse_file` for each matching file
  - Logs per-file soft failures without aborting
- [ ] Implement hard error handling: grammar load and query compilation failures raise immediately with actionable error message (which package, which `.scm` file, what went wrong)
- [ ] Add `--parallel` flag using `ProcessPoolExecutor` for multi-repo or large-repo parsing. The <60s target for all repos may require parallelism — single-threaded Python is a bottleneck for ~500K+ lines across 11 repos. Default: sequential. Flag enables `max_workers=cpu_count()`.
- [ ] Implement `resolve_repo_path(template)` that expands `${REFERENCE_REPOS_ROOT}`, `${LANG_INTELLIGENCE_ROOT}`, and `${ORI_LANG_ROOT}` env vars in `repos.yaml` paths. Defaults: `REFERENCE_REPOS_ROOT` → `~/projects/reference_repos/lang_repos`, `ORI_LANG_ROOT` → `~/projects/ori_lang`, `LANG_INTELLIGENCE_ROOT` → `~/projects/lang_intelligence`. This is the canonical path resolver — downstream scripts must NOT hardcode absolute paths.
- [ ] Verify adapter output: write a smoke test that calls `parse_repo("rust")` on a small include path and asserts all `ParseResult` fields are populated

- [ ] **TPR checkpoint** — `/tpr-review` covering 05.1–05.3 implementation work
- [ ] **Subsection close-out (05.3)**
  - [ ] All tasks above are `[x]` and adapter API is documented and tested
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — was the adapter API sufficient for Section 06's needs? Any fields missing? Any error messages unhelpful during debugging? Implement improvements, commit separately.

---

## 05.4 Query File Families

**Files:** `~/projects/lang_intelligence/queries/{lang}/{family}.scm`

Official `tags.scm` files vary by language in what they capture. Some (Rust, Go) already include `@reference.call` and `@reference.implementation` captures alongside declarations. Others (TypeScript, Swift) primarily capture declarations. This subsection standardizes query file families for ALL languages, adapting existing upstream queries where possible and writing custom ones where needed.

**Query families:**
- `decls.scm` — declarations: functions, types, traits, methods, constants
- `calls.scm` — call expressions: function calls, method calls
- `imports.scm` — import/use/require statements
- `impls.scm` — impl blocks, interface conformance, instance declarations

**Per-language query file status:**

| Language | decls.scm | calls.scm | imports.scm | impls.scm | Source |
|----------|-----------|-----------|-------------|-----------|--------|
| Rust | Official (has decls) | Official (has @reference.call) | Custom | Official (has @reference.implementation) | tree-sitter-rust |
| Go | Official (has decls) | Official (has @reference.call) | Official (has package/import) | N/A (implicit) | tree-sitter-go |
| Zig | Custom (no official tags) | Custom | Custom | N/A | tree-sitter-zig |
| TypeScript | Official tags.scm adapted | Custom | Custom | Custom | tree-sitter-typescript |
| Haskell | Custom (no official tags) | Custom | Custom | Custom | tree-sitter-haskell |
| Swift | Official tags.scm adapted | Custom | Custom | Custom | tree-sitter-swift |
| C++ | Official tags.scm adapted | Custom | Custom | N/A | tree-sitter-cpp |
| Koka | Custom (if grammar works) | Custom | Custom | Custom | tree-sitter-koka |

**Implementation approach:**
1. For languages WITH official `tags.scm`: adapt/rename to `decls.scm`, then write `calls.scm`, `imports.scm`, `impls.scm` from scratch using each grammar's `node-types.json` as reference.
2. For languages WITHOUT official `tags.scm` (Zig, Haskell, Koka): write all four families from scratch.
3. Some families may be empty stubs for some languages (e.g., Go has no explicit `impl` blocks — `impls.scm` is empty). Empty stubs are valid — they return zero captures. The adapter contract handles this gracefully.

- [ ] **Rust** (`queries/rust/`): Adapt official tags.scm which already includes `@reference.call` and `@reference.implementation`. Split into `decls.scm` (declaration captures), `calls.scm` (call captures — already upstream), `impls.scm` (impl captures — already upstream). Write `imports.scm` (match `use_declaration` — not in upstream tags.scm).
- [ ] **Go** (`queries/go/`): Adapt official tags.scm which already includes `@reference.call` and package/import captures. Split into `decls.scm`, `calls.scm` (already upstream), `imports.scm` (already upstream). `impls.scm` is empty stub (Go interfaces are implicit).
- [ ] **Zig** (`queries/zig/`): Write all four from scratch using `node-types.json`. `decls.scm`: `fn_decl`, `struct_decl`, `enum_decl`, `const_decl`. `calls.scm`: `call_expression`. `imports.scm`: `@import` calls. `impls.scm`: empty stub.
- [ ] **TypeScript** (`queries/typescript/`): Create `decls.scm` from official tags.scm. Write `calls.scm`, `imports.scm` (match `import_statement`), `impls.scm` (match `implements_clause`).
- [ ] **Haskell** (`queries/haskell/`): Write all four from scratch. `decls.scm`: `function`, `signature`, `type_alias`, `data_declaration`, `class_declaration`. `calls.scm`: `function_application`. `imports.scm`: `import_declaration`. `impls.scm`: `instance_declaration`.
- [ ] **Swift** (`queries/swift/`): Create `decls.scm` from official tags.scm. Write `calls.scm`, `imports.scm`, `impls.scm` (match `protocol_conformance`).
- [ ] **C++** (`queries/cpp/`): Create `decls.scm` from official tags.scm. Write `calls.scm`, `imports.scm` (match `#include`). `impls.scm`: empty stub.
- [ ] **Koka** (`queries/koka/`): If tree-sitter-koka grammar loaded successfully in 05.1: write `decls.scm` (`fun_decl`, `type_decl`, `effect_decl`, `val_decl`), `calls.scm`, `imports.scm`, `impls.scm`. If grammar failed: use Haskell queries for `.hs` files and document the gap.
- [ ] Test each query file against at least one real file from its repo. Non-stub queries must compile without error and produce at least one capture on the test file. Declared stub queries (e.g., Go `impls.scm`, Zig `impls.scm`, C++ `impls.scm`) must compile without error and return zero captures as expected.
- [ ] Create golden file probes: for each language, pick one well-known file and record expected capture count. Example: `rustc_parse/src/parser/expr.rs` must yield at least 20 `decls` captures and at least 50 `calls` captures. Store in `tests/golden-probes.yaml`.

- [ ] **Subsection close-out (05.4)**
  - [ ] All tasks above are `[x]` and all query files compile (non-stubs produce captures, declared stubs return zero captures)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — were query files hard to write? Should there be a `test-queries.py` script that compiles all `.scm` files and reports errors? Any node types unexpectedly named? Implement improvements, commit separately.

---

## 05.5 Parse Validation & Matrix Testing

**File(s):** `~/projects/lang_intelligence/scripts/validate-parsers.py`

A comprehensive validation script that tests the full parser adapter stack: grammar loading, file parsing, query compilation, and capture accuracy.

**Matrix dimensions:**
1. **Language** (9 tree-sitter languages)
2. **File condition** (Valid source / Malformed source / Empty file)
3. **Query family** (decls / calls / imports / impls)

- [ ] Implement `validate-parsers.py` with the following test modes:
  - `--smoke`: Load each grammar, parse one file per language, verify no crash. Target: <5 seconds.
  - `--matrix`: Full matrix test — Language x Condition x Query Family. For each cell: parse, run query, report captures/errors. Target: <30 seconds.
  - `--full`: Parse all files from all repos per `repos.yaml`. Report per-language: files parsed, error nodes, error rate, total captures per query family. Target: <60 seconds.
  - `--golden`: Run golden file probes from `tests/golden-probes.yaml` and verify capture counts.
- [ ] Malformed file handling: For each language, create a deliberately malformed file (`tests/malformed/{lang}.{ext}`) with syntax errors. Verify parser produces a tree (with ERROR nodes) rather than crashing. Verify `had_error=True` and `error_node_count > 0` in the `ParseResult`.
- [ ] Empty file handling: For each language, verify parsing an empty file produces a tree with zero nodes and zero errors (not a crash or exception).
- [ ] Error rate validation: Compare actual error rates against `expected_error_rate` from `languages.yaml`. Fail if any language exceeds its expected rate by >5 percentage points.
- [ ] Query compilation validation: For each language x query family, verify the `.scm` file compiles without error. Report which families are stubs (zero captures expected).
- [ ] Performance reporting: Report parse throughput (files/sec, lines/sec, bytes/sec) for `--full` mode. Expected baseline: ~289K lines/sec aggregate, <5 seconds per small repo, <15 seconds for Rust/Swift.
- [ ] Golden file probes: At least one known file per language must yield specific capture counts (within 10% tolerance to account for grammar version changes). Probe failures are regressions.
- [ ] Incremental hashing verification: Verify `content_hash` in `ParseResult` is deterministic — same file content produces same hash on re-parse. This validates Section 09's skip-if-unchanged mechanism.
- [ ] Grammar update policy: Document in `scripts/validate-parsers.py --help` and `README.md`: who bumps grammar versions, how breakage is detected (run `--golden` before and after), CI gating strategy (run `--matrix` on every grammar version bump).

- [ ] **Subsection close-out (05.5)**
  - [ ] All tasks above are `[x]` and `validate-parsers.py --matrix` passes
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — was the validation script output clear? Should it produce JSON for CI consumption? Should `--golden` auto-bless on `BLESS=1`? Implement improvements, commit separately.

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001-codex][high]` `plans/lang-intelligence/section-06-symbol-extraction.md:53` — Close the LEAK between ParseResult and Section 06.
  Evidence: Section 06.2 reads repos.yaml/languages.yaml/tags.scm directly instead of consuming the ParseResult adapter.
  Resolved: Fixed on 2026-04-12. Added plan-sync item to 05.N requiring Section 06 update to consume adapter.
- [x] `[TPR-05-002-codex][high]` `plans/lang-intelligence/section-05-parser-adapters.md:176` — Remove LEAK of machine-local roots from repos.yaml.
  Evidence: repos.yaml hardcoded ~/projects/... paths. Changed to ${REFERENCE_REPOS_ROOT} env-var pattern with runtime resolver.
  Resolved: Fixed on 2026-04-12. Changed repos.yaml contract to env-var pattern, added resolve_repo_path() item to 05.3.
- [x] `[TPR-05-003-codex][high]` `plans/lang-intelligence/section-05-parser-adapters.md:237` — Fix GAP in ParseResult source payloads.
  Evidence: ParseResult had byte_count but not source_bytes — Section 06 needs source slices for qualified names.
  Resolved: Fixed on 2026-04-12. Added source_bytes field to ParseResult contract.
- [x] `[TPR-05-004-codex][medium]` `plans/lang-intelligence/section-05-parser-adapters.md:292` — Eliminate WASTE from tags.scm baseline.
  Evidence: Rust/Go official tags.scm already include call/impl references. Blanket "declarations only" claim was inaccurate.
  Resolved: Fixed on 2026-04-12. Nuanced per-language, updated table and implementation notes.
- [x] `[TPR-05-005-codex][medium]` `plans/lang-intelligence/00-overview.md:135` — Resolve DRIFT in overview language matrix.
  Evidence: Overview said Swift=source build, Lean=tree-sitter-lean. Section 05 says Swift=try PyPI first, Lean=C++ only.
  Resolved: Fixed on 2026-04-12. Synced overview matrix with Section 05 strategy.
- [x] `[TPR-05-001-gemini][high]` `plans/lang-intelligence/section-05-parser-adapters.md:125` — Change subsection close-out headers to checklist items.
  Evidence: Used ### headers instead of - [ ] checklist items per plan-schema.md.
  Resolved: Fixed on 2026-04-12. Converted all 5 close-out blocks to checklist item format.
- [x] `[TPR-05-002-gemini][medium]` `plans/lang-intelligence/section-05-parser-adapters.md:284` — Move TPR checkpoint above subsection close-out.
  Evidence: TPR checkpoint was placed after 05.3 close-out instead of before it.
  Resolved: Fixed on 2026-04-12. Moved TPR checkpoint to before close-out block.
- [x] `[TPR-05-003-gemini][high]` `plans/lang-intelligence/section-05-parser-adapters.md:393` — Add task to update Section 06 for query file rename.
  Evidence: Section 05 renames tags.scm to decls.scm but no plan-sync item to update Section 06.
  Resolved: Fixed on 2026-04-12. Added Section 06 update item to plan-sync block. (Overlaps with TPR-05-001-codex.)
- [x] `[TPR-05-001-codex][high]` (iter 2) `section-06-symbol-extraction.md:54` — Update Section 06 to consume adapter.
  Evidence: Section 06.2 still reads repos.yaml/tags.scm directly.
  Resolved: Fixed on 2026-04-12. Updated Section 06.2 contract to consume ParseResult/parse_repo().
- [x] `[TPR-05-002-codex][medium]` (iter 2) `section-05-parser-adapters.md:324` — Stub query validation contradiction.
  Evidence: Plan says stubs are valid (zero captures) but also requires all queries to produce captures.
  Resolved: Fixed on 2026-04-12. Qualified validation: non-stubs must produce captures, stubs must compile cleanly.
- [x] `[TPR-05-003-codex][medium]` (iter 2) `section-05-parser-adapters.md:62` — Success criteria overstates extraction.
  Evidence: Section 05 claims "extracts structural symbols" but extraction is Section 06's deliverable.
  Resolved: Fixed on 2026-04-12. Changed to "Unblocks mission criteria" (parsing half only).

---

## 05.N Completion Checklist

- [ ] All 9 tree-sitter grammars load with pinned versions (`requirements.txt` verified compatibility matrix)
- [ ] `languages.yaml` defines all 10 languages (9 tree-sitter + Ori `native`) with `coverage_status`, `grammar_version`, `query_families`; Lean is `partial`
- [ ] `repos.yaml` defines all 11 repos with canonicalized `repo_id` / `source_root` / `issue_root` (resolves `go`/`golang` duality)
- [ ] Parser adapter API (`parser_adapter.py`) exposes `ParseResult` with all contract fields; error handling: soft per-file, hard grammar/query
- [ ] Query file families (`decls.scm`, `calls.scm`, `imports.scm`, `impls.scm`) exist for all 9 languages (stubs where appropriate)
- [ ] `validate-parsers.py --matrix` passes (Language x Condition x Query Family), `--golden` probes pass, `--full` completes in <60 seconds
- [ ] `setup-parsers.sh` automates full environment setup; `--parallel` flag works for large-repo parsing
- [ ] Content hashing deterministic (same file = same hash); grammar update policy documented
- [ ] Plan annotation cleanup: no stale plan references in code
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` -> `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated (check off any now satisfied)
  - [ ] `index.md` section status updated
  - [ ] Next section's (`06`) `depends_on` verified — no stale assumptions
  - [x] **Update Section 06 plan** — already updated during plan review (TPR iter-2): Section 06.2 contract now consumes `ParseResult`/`parse_repo()` and uses query family handles.
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify every subsection ran its retrospective (no skips). Look for cross-subsection patterns: command sequences repeated across subsections, integration points with worse error messages than within-subsection failures, manual cross-referencing no tool combined. Implement new items, commit separately. Document negative finding if no cross-cutting gaps.

**Exit Criteria:** `validate-parsers.py --full --golden` passes with all 9 languages within expected error rates, <60 seconds total parse time, all golden probes within tolerance, and `parser_adapter.py` API contract verified by Section 06's extraction script importing and using it without modification.
