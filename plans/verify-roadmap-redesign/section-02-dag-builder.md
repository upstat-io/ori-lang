---
section: "02"
title: "DAG Builder & Conflict Classifier"
status: in-progress
reviewed: true
goal: "Build the dependency DAG across all plans and implement the 8 conflict classifiers (6 from mission + REDUNDANT_DEPENDENCY, ORPHANED_PLAN) with deterministic precedence and source-kind tagging, so test cases (a), (b), (c), (g), (h) from the overview are caught without false positives on code fences, YAML comments, or prerequisite coordination"
success_criteria:
  - "Node model covers all seven schema classes from §01.2 (plan index, plan section, roadmap section, overview, bug-tracker section, fix-BUG file, completed-plan index) — not just plan sections"
  - "Source-kind taxonomy defined (EXPLICIT_DEPENDS_ON | HTML_COMMENT_CONVENTION | YAML_COMMENT | PROSE_VERB | CODE_FENCE_EXAMPLE) and every reference/edge carries its source-kind"
  - "DAG construction parses EXPLICIT_DEPENDS_ON as the SSOT for edges; body-inferred references emit MISSING_DEPENDENCY findings but DO NOT add shadow edges"
  - "Shared subsystem mapping uses a normalized subsystem-name table (crate path → canonical name), consumed by CONFLICT with dependency-edge short-circuit"
  - "Classifiers implement a documented precedence (PARSE_ERROR > DEAD_REFERENCE > DAG_CONFLICT subtypes > informational) and are run in deterministic order"
  - "CONFLICT, SUPERSEDED, BLOCKED, MISSING_DEPENDENCY, DEAD_REFERENCE, STATUS_CONTRADICTION (CROSS_EDGE_TEMPORAL_DRIFT only; TPR_STALE_VS_EDIT handed off to §03), plus REDUNDANT_DEPENDENCY and ORPHANED_PLAN informational classifiers implemented"
  - "SUPERSEDED detector covers BOTH reroute-plans-with-supersedes-list AND in-place-rewrite-claimed-but-not-done patterns"
  - "DEAD_REFERENCE scanner extends path-reference heuristics with HTML-comment grammar (blocked-by/unblocks/supersedes/resolves) and excludes code-fence / indented-code contexts"
  - "YAML frontmatter comments are scanned for dependency signal (raw-text post-YAML-parse, since PyYAML strips comments)"
  - "Multi-hop dependency chains are carried as typed `Finding.dependency_chain: tuple[Path, ...]` and source_kind as `Finding.source_kind: SourceKind | None` (both fields added to `scripts/plan_corpus/types.py` per §02.5 Concern I Option A — typed phase-boundary handoff, never string-flattened into `evidence`)"
  - "Known test cases (a), (b), (c), (g), (h) from overview are caught by the exact classifier + subtype documented in §05.2"
  - "Classifier determinism is TDD-enforced: a fixture with overlapping findings verifies only the highest-precedence classifier fires for each (source, source_line) pair"
  - "Every checklist item below has a failing test fixture written BEFORE implementation (§02.1–02.4 close-outs)"
inspired_by: []
depends_on:
  - "01"
third_party_review:
  status: resolved
  updated: "2026-04-14"
sections:
  - id: "02.0"
    title: "Node Model & Source-Kind Taxonomy"
    status: complete
  - id: "02.1"
    title: "DAG Construction"
    status: complete
  - id: "02.2"
    title: "Conflict Classifiers"
    status: complete
  - id: "02.3"
    title: "Priority Inversion Detection"
    status: complete
  - id: "02.4"
    title: "Classifier Precedence & Determinism Tests"
    status: complete
  - id: "02.5"
    title: "Handoff Contract with §03"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: DAG Builder & Conflict Classifier

**Status:** Not Started
**Goal:** Build a dependency DAG across ALL seven schema classes in the corpus (not just plan sections) and implement a classifier stack — six mission classifiers plus two informational ones — with deterministic precedence and explicit source-kind tagging, so the mission test cases (a), (b), (c), (g), (h) are caught exactly as specified in the overview without false positives on code fences, prerequisite coordination, or YAML anchor-like syntactic noise.

**Success Criteria:**
- [ ] Node model covers all seven §01.2 schema classes
- [ ] Source-kind taxonomy defined and every reference carries its source_kind
- [ ] EXPLICIT_DEPENDS_ON is the sole SSOT for DAG edges; body-inferred references emit MISSING_DEPENDENCY findings, never shadow edges
- [ ] 8 classifiers implemented with documented precedence and deterministic order
- [ ] Known test cases (a), (b), (c), (g), (h) caught by the exact classifier/subtype documented in §05.2

**Context:** The current `/verify-roadmap` command operates on individual roadmap sections in isolation. It has zero awareness of reroute plans, cross-plan dependencies, or shared subsystem ownership. As a result, it misses entire classes of bugs: priority inversions (repr-opt active but its locality prerequisite is queued), dead references (roadmap section 22.2 references nonexistent `plans/ori_lsp/`), and supersession drift (test-suite-health section 02 claims to rewrite roadmap 21A but the rewrite never happened). The DAG builder creates the graph structure that makes these detectable.

Two meta-risks motivate §02.0 and §02.4 before the classifier work in §02.2:
1. **Node coverage.** The live test corpus contains bugs spanning fix-BUG files, roadmap sections, and completed-plan indexes — not just plan sections. A node model restricted to `plans/<plan>/section-<NN>-<slug>.md` files loses test case (g) (BUG-04-039 lives in `plans/bug-tracker/fix-BUG-04-039.md`) and test case (h) (Section 21A cross-references a completed plan).
2. **Classifier interference.** Multiple classifiers can fire on the same source line (e.g. a DEAD_REFERENCE inside a prose-verb `depends on` claim is simultaneously a MISSING_DEPENDENCY candidate and a DEAD_REFERENCE). Without an explicit precedence and source-kind taxonomy, the classifier stack is non-deterministic and leaks false positives from code-fence examples in the bug-tracker overview.

**Depends on:** Section 01 (Frontmatter Schema) — the DAG builder relies on `plan_corpus` for `load_and_validate`, `Corpus.name_index`, `Finding`, `FindingCategory`, `FindingSubtype`, and `resolve_dep`. No shadow parsers.

**Scope NOTEs for /tpr-review triage:**
- **NOTE (drift with §05.2 — case (c)):** `plans/test-suite-health/index.md:2` DOES have `reroute: true` (it IS a reroute plan — TPR-02-001-gemini corrected the earlier misreading). The reroute index, however, has no `supersedes` field at all (not empty — absent), AND `plans/test-suite-health/00-overview.md:5` has `supersedes: []`. Case (i) of SUPERSEDED iterates the `supersedes` list and finds nothing. The live body at `plans/test-suite-health/section-02-roadmap-reprioritization.md` uses "update 21A", "reprioritize", and "reorder Section 21A" — NEVER the verbs "rewrites X" or "rewrite of X". §02.2 Case (ii) trigger verbs must therefore include the live-corpus verbs (see TPR-02-002), not just explicit rewrite markers. Downstream /tpr-review to reconcile with §05.2:124 — either §05.2's classification for case (c) stays SUPERSEDED (requiring Case (ii)'s broader vocabulary), or §05.2 narrows to accept MISSING_DEPENDENCY on the `supersedes: []` pattern.
- **NOTE (handoff to §03 — Finding schema extensions):** §02.5 requests two §01.3-owned extensions to `Finding`: (a) `source_kind` as a first-class facet, (b) a mechanism for multi-hop transitive chains. Options are enumerated below; the chosen option must be sourced into §01.3 at TPR triage time.
- **NOTE (handoff to §03 — git-timing-aware subtypes):** `TPR_STALE_VS_EDIT` as described in §01.3 cannot be computed from mtime (git clone/CI resets mtimes). §02 emits the `CROSS_EDGE_TEMPORAL_DRIFT` subtype only. `TPR_STALE_VS_EDIT` is specialized in §03's write-back phase using `WriteBackContext.has_recent_commits` and git `%cI` timestamps.
- **NOTE (drift with §01.3 owner):** §01.3 documents `CROSS_EDGE_TEMPORAL_DRIFT` and `TPR_STALE_VS_EDIT` as Section 02 DAG classifier subtypes. Per the handoff above, §02 emits ONLY `CROSS_EDGE_TEMPORAL_DRIFT`; §03 is the owner of `TPR_STALE_VS_EDIT`. /tpr-review to update §01.3's attribution of `TPR_STALE_VS_EDIT` if the split is ratified.
- **NOTE (drift with §01.5 fixtures):** §01.5 line 425 claims "05.2 validation case asserts the (g)/(h) bug-tracker scenarios are caught via this subtype" referring to `TPR_STALE_VS_EDIT`. §01.5 line 637 corrects this: (g) → BLOCKED, (h) → DEAD_REFERENCE, neither maps to `TPR_STALE_VS_EDIT`. §02 enforces the §01.5:637 mapping. /tpr-review to remove the stale claim at §01.5:425.
- **NOTE (handoff to §03 — Option A typed-field contract, per TPR-02-001-codex-r4):** §03's `section-03-findings-report.md:64` (and the surrounding classifier/write-back text) was authored against the pre-Option-A Finding contract — it references `evidence`-based parsing for chain and source_kind data. Round-3 round made Option A load-bearing in §02 (typed `Finding.dependency_chain` and `Finding.source_kind`), and §03 must import the Option A contract when its turn in the plan sequence arrives. §02 cannot edit §03 directly (single-section scope); /tpr-review on §03 (separate pass) must propagate Option A into §03's SafeFix/ExposureReview classifier: `classify_safety` reads `finding.source_kind` directly (typed field), not `evidence[0]` parsed as `"source_kind:<VALUE>"`. The dependency-chain data for §03's report rendering comes from `finding.dependency_chain`, not a string-split on `evidence`. This NOTE is the handoff record.
- **NOTE (drift with §05.2 route A/B — per TPR-02-004-codex-r3):** §05.2:113 and §05.2:143 currently assert "Expected: BLOCKED finding" unconditionally for cases (a) and (g). Per §02.2's route-A/route-B split, both cases emit MISSING_DEPENDENCY in the current live-corpus state (no `depends_on` edges declared); BLOCKED only fires AFTER route-A corpus edits land. §05.2 must be updated to branch: "Expected: MISSING_DEPENDENCY in current corpus state; BLOCKED after route-A edits land." This is a §05 edit; /tpr-review on §05 (separate pass) must propagate the route-A/B split from §02.2 into §05.2:113,143. §02 cannot edit §05 directly (single-section scope) — this NOTE is the handoff record.

---

## 02.0 Node Model & Source-Kind Taxonomy

**File(s):** `scripts/plan_corpus/dag.py` (new module — homes `NodeId`, `NodeKind`, `Reference`, `Edge`, and DAG-specific types; imports `Finding`/`FindingCategory`/`FindingSubtype`/`Corpus`/`SourceKind` from `plan_corpus`). **`SourceKind(Enum)` lives in `scripts/plan_corpus/types.py` alongside `Finding`** — not `dag.py` — because `Finding.source_kind: SourceKind | None` is a typed field on the canonical `Finding` dataclass. Homing `SourceKind` in `dag.py` would create a circular import (`types.py → dag.py → types.py`) — the structural-purity version of LEAK:scattered-knowledge (per TPR-02-001-gemini round 2). `types.py` is the canonical home for boundary-crossing types; `dag.py` consumes them.

This subsection lays the structural foundation for §02.1–02.4. It must be complete BEFORE the DAG is built, because node coverage and source-kind tagging are load-bearing for every classifier downstream.

- [x] Define `NodeKind(Enum)` covering all seven §01.2 schema classes:
  - `PLAN_INDEX | PLAN_SECTION | ROADMAP_SECTION | OVERVIEW | BUG_TRACKER_SECTION | FIX_BUG | COMPLETED_INDEX`
  - One-to-one with `FileClass` in `scripts/plan_corpus/schema.py:60-68`
  - Every `Corpus` bucket (`indexes`, `plan_sections`, `roadmap_sections`, `overviews`, `bug_sections`, `fix_bug_files`, `completed_indexes`) contributes nodes

- [x] Define `NodeId` as a frozen dataclass `(kind: NodeKind, path: Path)`:
  - Hashable and comparable for graph use
  - Equality = same file class AND same resolved path
  - Include a helper `NodeId.from_validated_file(vf: ValidatedFile) -> NodeId` that maps `FileClass` → `NodeKind` via a lookup table (no re-classification)

- [x] Define `SourceKind(Enum)` — the taxonomy used by every reference and edge. **Home: `scripts/plan_corpus/types.py`** (alongside `Finding` — see the §02.0 "File(s)" note and TPR-02-001-gemini round 2 rationale for why this CANNOT live in `dag.py`):
  - `EXPLICIT_DEPENDS_ON` — frontmatter `depends_on: [...]` entries (the only DAG-edge source)
  - `HTML_COMMENT_CONVENTION` — structured HTML comments (`<!-- blocked-by:ID -->`, `<!-- unblocks:ID -->`, `<!-- supersedes:ID -->`, `<!-- resolves:ID -->`)
  - `YAML_COMMENT` — comments inside YAML frontmatter (`status: in-progress  # TPR done; hygiene blocked by BUG-05-003 → plans/iterator-element-ownership/`)
  - `PROSE_VERB` — body prose using one of `depends on`, `requires`, `blocked by`, `prerequisite`, `unblocks`, `supersedes`, `rewrites`, `obsoletes`, `see also`, `related`, `inspired by`, `cf.` (verb-bearing references; some feed MISSING_DEPENDENCY, others are informational)
  - `CODE_FENCE_EXAMPLE` — anything inside a fenced code block (``` ... ```) or indented code block (4+ leading spaces)

- [x] Define `Reference` dataclass `(from_node: NodeId, target: str, source_kind: SourceKind, source_line: int, source_column: int | None, raw_text: str)`:
  - Every reference carries enough info to disambiguate `Finding.id` collisions (Finding J): the `source_column` field plus the raw_text hash serve as tie-breakers
  - `target` is the raw string as it appears in the file (not yet resolved); resolution happens via `plan_corpus.resolve_dep` in §02.1

- [x] Define `Edge` dataclass `(from_node: NodeId, to_node: NodeId, source_kind: SourceKind, reference: Reference)`:
  - Edges are ONLY created from `SourceKind.EXPLICIT_DEPENDS_ON` references (per the SSOT rule below)
  - `source_kind` on an `Edge` is always `EXPLICIT_DEPENDS_ON`; it is carried on `Edge` only for debugging symmetry with `Reference`

- [x] **Explicit SSOT rule — body-inferred references do NOT add edges:**
  - `EXPLICIT_DEPENDS_ON` is the sole edge source. Body-inferred references (`HTML_COMMENT_CONVENTION`, `YAML_COMMENT`, `PROSE_VERB`) are collected as `Reference` records but are never promoted to `Edge`.
  - If a `PROSE_VERB` / `HTML_COMMENT_CONVENTION` / `YAML_COMMENT` reference names a node that is NOT in the DAG as a `depends_on` successor of the current node, it is emitted as a `DAG_CONFLICT / MISSING_DEPENDENCY` finding.
  - This makes the DAG deterministic and frontmatter the SSOT: authors fix by adding a `depends_on` entry, not by the tool silently injecting shadow edges.

- [ ] Define `SUBSYSTEM_ALIASES: dict[str, str]` — the canonical name-normalization table:
  - Source (A): auto-populate from `Cargo.toml` workspace members (e.g. `compiler/ori_arc` → `ori_arc`, `compiler/ori_llvm` → `ori_llvm`)
  - Source (B): hand-maintained logical aliases (e.g. `ArcClassifier` → `ori_arc`, `AIMS` → `ori_arc`, `Tag::Var` → `ori_types`, `ReprPlan` → `ori_repr`)
  - Expose `normalize_subsystem(raw: str) -> str | None`; `None` for unrecognized tokens (they do NOT contribute to shared-subsystem mapping)
  - Unit test: every workspace crate's display name appears in the normalized output

- [x] Define code-fence/indented-code-block exclusion helper `strip_code_blocks(body: str) -> list[tuple[int, int, str]]`:
  - Returns a list of `(start_line, end_line, kind)` tuples describing code-fence regions (`kind` ∈ `{"fenced", "indented"}`)
  - Used by §02.1 body scanners to mask out code-fence regions before heuristic matching
  - Fenced detection: lines matching `^```` (optional language tag); matched pairs
  - Indented detection: lines starting with 4+ spaces that are preceded by a blank line (loose but matches CommonMark indented-code-block semantics)

- [x] Define YAML frontmatter raw-text helper `extract_yaml_comments(text: str, body_offset: int) -> list[tuple[int, int, str]]`:
  - Returns `(line_number, column, comment_text)` tuples for every `# ...` comment found on frontmatter lines
  - `body_offset` from `split_frontmatter_strict` bounds the scan to the `---` ... `---` region only
  - PyYAML strips comments at parse time, so this is a raw-text post-parse pass

- [x] Define HTML comment grammar helper `parse_html_comments(body: str) -> list[Reference]`:
  - Matches `<!--\s*(blocked-by|unblocks|supersedes|resolves|rewrites|update-complete|updated-by)\s*:\s*([^ \t\r\n,]+(?:,[^ \t\r\n,]+)*)\s*-->` — hyphens ARE allowed in target tokens (plan slugs are hyphenated, e.g. `jit-exception-handling/04B`, `iterator-element-ownership`); only whitespace and the `,` separator are excluded from each comma-separated target. Extended per TPR-02-001-codex-r3 to include the three verbs §02.2 SUPERSEDED case (ii) consumes. TPR-02-003-codex semantic pin: the live case (h) `<!-- unblocks:jit-exception-handling/04B,05,06 -->` MUST parse into three targets (`jit-exception-handling/04B`, `05`, `06`), not zero.
  - Verbs → semantic:
    - `blocked-by:ID` → forward reference (current node blocked by ID)
    - `unblocks:ID` → reverse reference (current node unblocks ID; the logical edge runs from the BLOCKED node to the UNBLOCKING one, i.e. the REVERSE of the literal direction)
    - `supersedes:ID` → supersession reference (current node supersedes ID)
    - `resolves:ID` → bug-fix reference (current node resolves bug ID)
    - `rewrites:ID` → rewrite reference (current node claims to rewrite ID; feeds SUPERSEDED case (ii))
    - `update-complete:resolves=<target-ref>` → completion marker asserting an in-place update has landed (consumed by SUPERSEDED case (ii) as the source-side completion signal)
    - `updated-by:<source-ref>` → back-reference asserting this node has been updated by the named source (consumed by SUPERSEDED case (ii) as the target-side completion signal)
  - Excludes any comment whose start position falls inside a `strip_code_blocks` region
  - The verb vocabulary is reserved; malformed `<!-- blocked-by: foo bar -->` (space inside value) emits a low-severity `SCHEMA_VIOLATION / CROSS_FIELD_INVARIANT` finding via §02.1's body scanner (invariant: HTML-comment metadata follows the grammar)

- [x] **Subsection close-out (02.0)** — MANDATORY before starting 02.1:
  - [x] Write failing test fixtures FIRST (TDD) — one per `SourceKind` × `NodeKind` cell that §02.1/§02.2 will consume — `tests/plan-audit/test_dag.py` (27 tests, all classes) + `test_dag_types.py` (16 tests). Tests written before implementation; verified failing at `ImportError: cannot import name 'SourceKind'` before types.py/dag.py were written.
  - [x] All tasks above are `[x]` and unit tests for `NodeId`, `SourceKind`, `strip_code_blocks`, `extract_yaml_comments`, `parse_html_comments`, and `normalize_subsystem` pass — 141 plan-audit tests green (`timeout 60 python -m pytest tests/plan-audit/ -q`)
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — No tooling gaps surfaced. pytest covered every case cleanly; fixture-authoring for `test_dag.py` was friction-free (inline strings sufficed, no need for `tests/plan-audit/fixtures/dag/` subdir yet). §02.1 will need real markdown fixture files — if friction appears there, capture at §02.1 close-out.
  - [x] **Run `/sync-claude` on THIS subsection** — No CLAUDE.md / rules / canon drift: `dag.py` is a new module that imports `SourceKind` from `types.py` (canonical home per `impl-hygiene.md` §SSOT); `NodeKind` is an internal mapping to `FileClass` with no user-visible surface. No new scripts/conventions to document globally. `.claude/rules/canon.md` §6 SSOT table does not need to list `dag.py` yet — it is still §02-internal; §02.N sweep will add a row once `DagReport` is part of the public API.

---

## 02.1 DAG Construction

**File(s):** `scripts/plan_corpus/dag.py` (same module as §02.0; `build_dag(corpus: Corpus) -> Dag` is the entry point).

Build a directed acyclic graph where nodes are instances of all seven §01.2 schema classes and edges come exclusively from `EXPLICIT_DEPENDS_ON` references. Body-text and HTML/YAML-comment references are collected as `Reference` records but are NOT promoted to edges — they feed the MISSING_DEPENDENCY classifier in §02.2.

- [x] Populate nodes from `Corpus`:
  - One `NodeId(NodeKind.PLAN_INDEX, path)` per entry in `corpus.indexes`
  - One `NodeId(NodeKind.PLAN_SECTION, path)` per entry in `corpus.plan_sections`
  - One `NodeId(NodeKind.ROADMAP_SECTION, path)` per entry in `corpus.roadmap_sections`
  - One `NodeId(NodeKind.OVERVIEW, path)` per entry in `corpus.overviews`
  - One `NodeId(NodeKind.BUG_TRACKER_SECTION, path)` per entry in `corpus.bug_sections`
  - One `NodeId(NodeKind.FIX_BUG, path)` per entry in `corpus.fix_bug_files`
  - One `NodeId(NodeKind.COMPLETED_INDEX, path)` per entry in `corpus.completed_indexes`
  - Every node's metadata includes (a) its file-class `status` field, (b) `reviewed`, (c) `goal` (if present), (d) parent-plan index for section-class nodes

- [x] Parse explicit dependencies from `depends_on` frontmatter fields (consumes §01.3 SSOT — no path-based resolution):
  - §01's parser has already validated `depends_on` as logical-ID values (`DepId`): intra-plan `"NN"` or cross-plan `"plan-name#NN"`. Full paths are rejected at parse time and never reach §02.
  - Resolve every `DepId` through `plan_corpus.docgen.resolve_dep(dep_id, source_plan_dir, corpus)` which uses `Corpus.name_index` to map `plan-name#NN` → target plan directory → target section file. Intra-plan `"NN"` resolves within the source plan.
  - If `resolve_dep` returns a `Finding` (DEAD_REFERENCE or SCHEMA_VIOLATION), §02 propagates it unchanged — §02 does not re-validate.
  - If `resolve_dep` returns a `Path`, create a `Reference(source_kind=SourceKind.EXPLICIT_DEPENDS_ON, source_line=<depends_on item line>, ...)` and promote it to an `Edge` from the declaring node to the resolved node.
  - **NOTE (handoff to §03 — Finding K):** `resolve_dep` currently returns `source=plan_dir / "index.md"` on unresolved cross-plan names and `source=plan_dir` on missing section files (see `scripts/plan_corpus/docgen.py:54,85`). For §03 SafeFix to remove the exact offending `depends_on` entry, §02 must enrich the returned Finding with `source_line` (the YAML line of the offending list item) and `evidence=(dep_id,)`. Add an `enrich_resolve_dep_finding(finding, dep_id, dep_line) -> Finding` helper that rebuilds the Finding with the precise location.

- [x] Collect body-text, HTML-comment, and YAML-comment references (NO edges created):
  - For each node's body (`ValidatedFile.body`):
    - Compute `code_block_regions = strip_code_blocks(body)` once
    - For each `plans/*/` path match that falls OUTSIDE the code-block regions, emit a `Reference(source_kind=SourceKind.PROSE_VERB, ...)` if preceded by a verb from the `DEPENDENCY_VERBS` set, otherwise `source_kind=SourceKind.PROSE_VERB` with `verb=None` (informational, does not feed MISSING_DEPENDENCY).
    - Define `DEPENDENCY_VERBS = frozenset({"depends on", "requires", "blocked by", "prerequisite", "unblocks", "supersedes", "rewrites", "rewrite of", "obsoletes", "update in place of", "update of", "reprioritize", "reorder"})` — includes explicit rewrite markers ("rewrites X", "rewrite of X") AND in-place-update verbs from the live corpus ("update X in place of", "reprioritize X", "reorder X"). Without these extended verbs, test case (c) is unimplementable — `plans/test-suite-health/section-02-roadmap-reprioritization.md` uses "update 21A" / "reprioritize", never "rewrites". TPR-02-002-gemini semantic pin: the phrase "rewrite of" must be matched in addition to "rewrites".
    - Define `INFORMATIONAL_VERBS = frozenset({"see also", "related", "inspired by", "cf."})` (no edge implication).
    - Call `parse_html_comments(body)` and collect its references with `SourceKind.HTML_COMMENT_CONVENTION`.
  - For each node's frontmatter raw text (reconstructed from the first `---` ... `---` region of the file):
    - Call `extract_yaml_comments(text, body_offset)` and scan each comment for any `DEPENDENCY_VERBS` member OR `plans/*/` path OR bug ID (`BUG-\d{2}-\d{3}`); matches become `Reference(source_kind=SourceKind.YAML_COMMENT, ...)`.
  - Scope constraint: this body-fact extractor reads `ValidatedFile.body` and raw frontmatter only. It does NOT re-parse the frontmatter mapping (§01's parser is the SSOT for that) and it does NOT re-validate schemas (§01's validators are the SSOT). The helper is permitted to read the original file bytes ONLY for the raw-frontmatter comment scan.

- [x] Map shared subsystems using the §02.0 normalization table:
  - For each node, extract a set of subsystem references from its body (crate paths like `compiler/ori_arc`, type names like `AIMS`, `ArcClassifier`, etc.) and its success criteria.
  - Normalize every extracted token via `normalize_subsystem`; drop `None` results.
  - Build `subsystem_to_nodes: dict[str, set[NodeId]]`.
  - Record this mapping on the `Dag` object — it is input to the CONFLICT and MISSING_DEPENDENCY classifiers (§02.2), NOT an edge source.

- [x] Assemble the `Dag` dataclass:
  - `nodes: set[NodeId]`
  - `edges: list[Edge]` — EXPLICIT_DEPENDS_ON only
  - `references: list[Reference]` — all source kinds (used by classifiers)
  - `subsystem_to_nodes: dict[str, set[NodeId]]`
  - `resolution_findings: list[Finding]` — DEAD_REFERENCE / SCHEMA_VIOLATION findings returned by `resolve_dep`, propagated with enriched source_line per Finding K
  - `name_index: dict[str, Path]` — a reference to `Corpus.name_index` for classifier reuse
  - Provide `Dag.to_json() -> dict` for serialization; serialization is lossless round-trip

- [x] Detect and report cycles via Tarjan SCC:
  - For each non-trivial SCC (size > 1 or self-loop), emit one `Finding(category=DAG_CONFLICT, subtype=CYCLE, severity=HIGH, dependency_chain=tuple(node.path for node in scc_path), source_kind=SourceKind.EXPLICIT_DEPENDS_ON)` — cycles are edge-based so all members carry EXPLICIT_DEPENDS_ON. Chain is structured (typed `dependency_chain`), NOT string-formatted into `evidence` — per Option A (TPR-02-006-codex + TPR-02-003-gemini-r3)
  - Cycles indicate a mutual dependency that cannot be resolved by execution order — severity is HIGH because the graph is unusable downstream

- [x] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [x] Write failing test fixtures FIRST (TDD): implemented as programmatic tmp_path mini-corpora in `tests/plan-audit/test_dag_construction.py` (11 tests, all fixtures inline) — one mini-corpus per fixture name. Committing tiny .md fixtures would duplicate setup in multiple tests; inline tmp_path keeps the live corpus pristine.
    - [x] `fixture_dag_basic` — `TestDagConstructionBasic::test_two_plans_with_depends_on_yields_one_edge`
    - [x] `fixture_dag_code_fence` (Finding D) — `TestDagConstructionCodeFence::test_code_fence_path_does_not_produce_reference`
    - [x] `fixture_dag_yaml_comment` (Finding F) — `TestDagConstructionYamlComment::test_yaml_comment_in_frontmatter_emits_reference`
    - [x] `fixture_dag_html_unblocks` (Finding B, case (h)) — `TestDagConstructionHtmlUnblocks::test_html_unblocks_comment_emits_html_reference`
    - [x] `fixture_dag_fix_bug_node` (Finding C/G, case (g)) — `TestDagNodeCoverage::test_fix_bug_file_appears_as_fix_bug_node`
    - [x] `fixture_dag_completed_index_node` (Finding G, case (h)) — `TestDagNodeCoverage::test_completed_index_appears_as_completed_index_node`
  - [x] All tasks above are `[x]` and `build_dag(corpus)` runs on the live corpus without Python exceptions — verified: 375 nodes, 338 edges, 441 references, 18 subsystems, 16 resolution findings, 1 cycle finding.
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — One tooling-surface improvement surfaced and landed in this subsection itself: `classify_file` was refactored to anchor on the first `plans/` segment in resolved paths instead of requiring `PLANS_DIR`, so synthetic `tmp_path` corpora work in tests without patching globals. Backward-compatible (PLANS_DIR callers take the same branch as before).
  - [x] **Run `/sync-claude` on THIS subsection** — No global CLAUDE.md / rules / canon drift. `dag.py` consumes types.py (canonical home); `classify_file` signature is unchanged. `.claude/rules/canon.md` §6 SSOT table will be updated at §02.N sweep once `DagReport` joins the public API surface.

---

## 02.2 Conflict Classifiers

**File(s):** `scripts/plan_corpus/dag.py` (classifier functions live alongside `build_dag`; each classifier is a pure function `(dag: Dag) -> list[Finding]`).

Implement the full classifier stack: the six mission classifiers plus `REDUNDANT_DEPENDENCY` and `ORPHANED_PLAN` informational classifiers. Classifier ordering and source-kind filtering are documented at the top of each classifier. Cross-classifier precedence lives in §02.4.

- [x] **Classifier registration table (at module top):**
  - `CLASSIFIERS: list[Callable[[Dag], list[Finding]]]` in execution order (per §02.4 precedence). Each classifier is pure and stateless. The `run_classifiers(dag) -> list[Finding]` entry point walks the table in order.

- [x] **CONFLICT** — contradictory goals for the same subsystem:
  - Input: `dag.subsystem_to_nodes` + node goals
  - For each shared subsystem with ≥2 nodes, for each pair `(A, B)` of nodes:
    - **Dependency-edge short-circuit (Finding G):** if `A` transitively depends on `B` (or vice versa) via EXPLICIT_DEPENDS_ON edges, this is prerequisite coordination, NOT conflict. Skip the pair.
    - Otherwise compare their `goal` strings using a contradiction heuristic (shared subject noun-phrases + opposing verbs like `remove`/`keep`, `refactor`/`preserve`, `rewrite`/`retain`). The heuristic returns a boolean + rationale string.
    - If contradictory → emit `Finding(category=DAG_CONFLICT, subtype=CONFLICT, severity=HIGH, source=A.path, target=B.path, description=..., recommended_fix="Decide precedence between the plans or add a depends_on edge to serialize them", evidence=(rationale,))`.

- [x] **SUPERSEDED** — reroute claims OR in-place-rewrite claims with incomplete rewrites:
  - Case (i) — reroute plan with `supersedes` list: for each plan index node with `reroute: True` AND non-empty `supersedes`, check whether the reroute plan has at least one non-complete section targeting each supersedes entry (body text contains a reference with `SourceKind.HTML_COMMENT_CONVENTION` verb `supersedes` OR `rewrites`, or a section goal substring-matches the supersedes entry's logical ID/title). If not → emit finding.
  - Case (ii) — in-place-update claim (Finding A, broadened per TPR-02-002-codex round 1; de-phase-leaked per TPR-02-002-codex round 2): for each plan section whose body prose contains ANY in-place-update verb from the extended `DEPENDENCY_VERBS` — `"rewrites"`, `"rewrite of"`, `"update in place of"`, `"update of"`, `"reprioritize"`, `"reorder"`, `"supersedes"`, `"obsoletes"` — where the following noun-phrase resolves via `resolve_dep` to another plan/roadmap section, emit a structural SUPERSEDED finding based purely on static facts §02 can observe WITHOUT timestamps. Structural detection criteria (no mtime, no git queries — §02 stays pure per 00-overview:27):
    - (a) the target section exists in the DAG, AND
    - (b) the source section's claim is NOT annotated with a structural completion marker `<!-- update-complete:resolves=<target-ref> -->` (HTML comment, parsed by §02.0's `parse_html_comments`), AND
    - (c) the target section does NOT contain a structural back-reference to the source claim (HTML comment `<!-- updated-by:<source-ref> -->` or explicit `depends_on` entry pointing at the source).
    - If all three hold → emit a MEDIUM-severity structural SUPERSEDED finding flagged as "potentially stale — needs §03 timestamp verification."
  - **§03 specialization (phase handoff, aligned with §01.3 SSOT per TPR-02-002-codex-r3):** §03's write-back phase receives §02's structural SUPERSEDED findings plus `WriteBackContext.has_recent_commits`. When both are available, §03 computes git `%cI` timestamps (via `git log -1 --format=%cI`) for the source-claim commit and the target-section last-edit, and uses the existing `FindingSubtype.SUPERSEDED` subtype with enriched `evidence` — NOT new subtypes. §03 tags the Finding with `evidence = ("git_status:stale_rewrite", ...)` or `evidence = ("git_status:likely_landed", ...)` to route SafeFix vs ExposureReview. This keeps §01.3's taxonomy stable (no new subtype authorization needed beyond the already-approved `REDUNDANT_DEPENDENCY` + `ORPHANED_PLAN`) while preserving the §02-pure / §03-git-aware split pattern that mirrors `TPR_STALE_VS_EDIT` at §02's scope NOTE lines 76-77. The key architectural property is preserved: §02 emits a structural fact, §03 consults git and routes. If future reviews decide the git_status needs to be a first-class subtype, it's added to §01.3 via the same cross-section-edit authorization pattern — but this round does NOT pre-authorize it.
  - Rationale for the split: `scripts/plan_corpus` is a pure library (`00-overview.md:27`). Embedding git queries in `dag.py` would break the phase boundary and the no-I/O contract. The structural check is the LEAK-free §02 signal; §03's write-back phase is where git context legitimately enters.
  - Known case (§05.2 (c)): `plans/test-suite-health/section-02-roadmap-reprioritization.md` claims to update/reprioritize `plans/roadmap/section-21A-llvm.md` but the update has not landed. `plans/test-suite-health/index.md:2` IS a reroute (`reroute: true`), but its `supersedes` field is absent AND `plans/test-suite-health/00-overview.md:5` has `supersedes: []` — so case (i) cannot fire. Case (ii) detects this via the extended verb vocabulary ("update", "reprioritize").
  - Emit `Finding(category=DAG_CONFLICT, subtype=SUPERSEDED, severity=MEDIUM, ...)`; recommended_fix: "Complete the rewrite or remove the claim".

- [x] **BLOCKED** — active node depends on queued/not-started node:
  - Walk the `Dag.edges` looking for edges `A → B` where:
    - `A.status` resolves to "active-equivalent" (plan-level `active`, section-level `in-progress`) AND
    - `B.status` resolves to "blocked-equivalent" (plan-level `queued` or `research`, section-level `not-started`)
  - Emit `Finding(category=DAG_CONFLICT, subtype=BLOCKED, severity=HIGH, source=A.path, target=B.path, ...)`.
  - **NOTE on cases (a) AND (g) — cross-plan DRIFT with §05.2 (per TPR-02-001-codex round 2 and round 1):** Both known cases currently fail the BLOCKED SSOT contract because the live corpus has no matching `depends_on` declarations:
    - **Case (a):** `plans/repr-opt/index.md:1-8` has no `depends_on` field. The real `name:` values are `"Repr Opt"` and `"Locality SSOT"` (verified), not `"Locality Representation Unification"`. For §02.2 BLOCKED to fire on case (a), `plans/repr-opt/index.md` must first add `depends_on: ["Locality SSOT#NN"]` pointing at the specific section within `plans/locality-representation-unification/` that gates it (probably `"Locality SSOT#02"`). Without that edit, repr-opt's body-prose references to locality-representation-unification feed MISSING_DEPENDENCY, NOT BLOCKED.
    - **Case (g):** `plans/bug-tracker/fix-BUG-04-039.md:5` records the blocker ONLY via a YAML frontmatter comment (`status: in-progress  # ... plans/iterator-element-ownership/`), NOT via `depends_on`. Per §02.0's SSOT rule (EXPLICIT_DEPENDS_ON is the sole edge source), the YAML-comment reference emits MISSING_DEPENDENCY, NOT BLOCKED. Only once the author adds `depends_on: ["Iter Ownership#NN"]` (the `name:` field value verified at `plans/iterator-element-ownership/index.md:3` — NOT the directory slug `iterator-element-ownership`; §01 rejects slug-style cross-plan IDs) does BLOCKED replace MISSING_DEPENDENCY.
  - **Pre-implementation prerequisite — mandatory corpus edits:** §02 acceptance requires at least one of these routes for each case:
    - Route A (preferred): the blocking-plan owners add the missing `depends_on` fields to `plans/repr-opt/index.md` and `plans/bug-tracker/fix-BUG-04-039.md` before §02 implementation lands. These edits MUST use the exact `name:` field values from the target plan's `index.md` — verified via `grep '^name:' plans/*/index.md`: `"Locality SSOT"` (for `plans/locality-representation-unification/`) and `"Iter Ownership"` (for `plans/iterator-element-ownership/`) — NOT the directory slugs, which §01's `DepId` validator rejects. Once the edges exist, BLOCKED fires on the live corpus and §05.2:113,143 remains accurate.
    - Route B (fallback): §05.2 reclassifies cases (a) and (g) as MISSING_DEPENDENCY (§02's current behavior until route A lands). §02.3 validation-case block already reflects this — the first finding for each case is MISSING_DEPENDENCY; BLOCKED replaces it only after route-A edits.
    - /tpr-review MUST reconcile with §05.2:113,143 (the real line numbers in `plans/verify-roadmap-redesign/section-05-validation.md` where "Expected: BLOCKED finding" is asserted for cases (a) and (g) respectively — corrected in round 4 per TPR-02-002-gemini-r4; the earlier "142,146" reference in round-2 trail is stale). Route A is preferred because the `depends_on` edges are real corpus hygiene improvements; route B is a valid fallback if the blocking-plan owners cannot edit in time.
  - **Status resolution helper (per TPR-02-005-codex):** because §01.4 owns the canonical status normalizer, §02.2 imports `plan_corpus.normalizer.normalize_status(data, body, path, child_statuses) -> NormalizedStatus` (see `scripts/plan_corpus/normalizer.py:70-138`) and uses `NormalizedStatus.derived` (NOT raw `data["status"]`) for BLOCKED edge classification. For plan-index nodes, §02 passes each child section's declared status as `child_statuses`; for leaf/section nodes, §02 passes `None` and the normalizer derives from body signals. There is no `effective_status()` wrapper and §02 MUST NOT invent one — LEAK:scattered-knowledge guard per `impl-hygiene.md` §SSOT. If §02 needs a one-argument convenience (e.g. `derived_for(node)`), it lives as a private `_derived_for(node)` helper in `dag.py` that composes `normalize_status` inputs from the node's frontmatter and children; the composition helper is never re-exported as a public API because `normalize_status` itself IS the public API.

- [x] **STATUS_CONTRADICTION / CROSS_EDGE_TEMPORAL_DRIFT** — DAG-level status drift:
  - §01.4's `normalize_status()` already produces per-file `STATUS_CONTRADICTION` findings (`FM_DECLARED_VS_BODY_DERIVED`, `PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED`, etc.). §02 consumes those facts; the DAG classifier adds ONLY the cross-edge subtype.
  - Walk `Dag.edges` and emit `Finding(category=STATUS_CONTRADICTION, subtype=CROSS_EDGE_TEMPORAL_DRIFT, severity=MEDIUM)` when a dependent node's declared status presupposes a state its prerequisite has not reached — e.g. dependent is `complete` but prerequisite is `not-started`; dependent is `in-progress` but prerequisite is `not-started` and its plan is `queued`.
  - `TPR_STALE_VS_EDIT` is NOT emitted from §02. It requires git commit timestamps (mtime is broken per Finding M) and is specialized by §03's write-back phase from a §02-emitted neutral `CROSS_EDGE_TEMPORAL_DRIFT` (if `WriteBackContext.has_recent_commits` is available).

- [x] **MISSING_DEPENDENCY** — body-inferred reference without a matching `depends_on` edge:
  - For each `Reference` with `source_kind` in `{PROSE_VERB, HTML_COMMENT_CONVENTION, YAML_COMMENT}` and verb in `DEPENDENCY_VERBS` (excluding informational verbs):
    - Resolve the target the same way §02.1's `resolve_dep` does
    - If the resolved target is a node in the DAG AND no `EXPLICIT_DEPENDS_ON` edge exists from `from_node` to the target → emit `Finding(category=DAG_CONFLICT, subtype=MISSING_DEPENDENCY, severity=MEDIUM, source=from_node.path, source_line=reference.source_line, target=target.path, source_kind=reference.source_kind, evidence=(reference.raw_text,))` — `source_kind` goes into the typed field (Option A), `evidence` carries only the short raw-text excerpt for human inspection.
  - Also walk `subsystem_to_nodes` (§02.1 map) for pairs of plans sharing a subsystem with BOTH in active/in-progress status AND no `EXPLICIT_DEPENDS_ON` edge between them — interference risk. Emit MISSING_DEPENDENCY for each unordered pair.
  - Recommended_fix: "Add `depends_on: [...]` entry referencing the inferred target".

- [x] **DEAD_REFERENCE** — pointer to nonexistent plan/directory/spec:
  - Input sources (all filtered through `SourceKind`, code-fence examples excluded):
    - `dag.resolution_findings` (DEAD_REFERENCE findings from §02.1)
    - For each `Reference` with `source_kind` in `{PROSE_VERB, HTML_COMMENT_CONVENTION, YAML_COMMENT}` that names a `plans/*/` path, check filesystem existence
    - A reference inside a `CODE_FENCE_EXAMPLE` region is NEVER promoted to DEAD_REFERENCE (Finding D semantic pin — `plans/bug-tracker/00-overview.md:23,30,50,71` and `plans/locality-representation-unification/section-02-ori-arc-implementation.md:253,692,821` contain template paths inside fences that must not false-positive).
    - A reference inside a YAML_COMMENT or HTML_COMMENT_CONVENTION that names a plan directory missing from disk IS promoted to DEAD_REFERENCE at lower severity (`LOW` vs `HIGH` for frontmatter references).
  - Known cases (§05.2 (b), (h)):
    - (b) `plans/roadmap/section-22-tooling.md:141` references `plans/ori_lsp/` (must verify exact line at runtime; `plans/ori_lsp/` does not exist on disk)
    - (h) `plans/roadmap/section-21A-llvm.md:83` — `<!-- unblocks:jit-exception-handling/04B,05,06 -->` and prose "would unblock completion of the JIT Exception Handling plan" — the target is a completed plan (its entry lives under `plans/completed/jit-exception-handling/`, not `plans/jit-exception-handling/`). DEAD_REFERENCE must resolve against BOTH `plans/<slug>/` and `plans/completed/<slug>/` — if found only in `completed`, emit a LOW severity DEAD_REFERENCE recommending the annotation be updated or removed (not a hard error — completed plans are real).
  - Emit `Finding(category=DEAD_REFERENCE, subtype=PLAN_DIRECTORY_NOT_FOUND | SECTION_FILE_NOT_FOUND | CROSS_PLAN_NAME_NOT_FOUND | SPEC_FILE_NOT_FOUND, severity=HIGH for EXPLICIT_DEPENDS_ON, MEDIUM for HTML_COMMENT/YAML_COMMENT, LOW for PROSE_VERB)`. Severity ladder encodes source-kind trust.

- [x] **REDUNDANT_DEPENDENCY (informational)** — A depends on B, A depends on C, and B (transitively) depends on C:
  - Compute transitive closure of `dag.edges`. For each edge `A → C`, if there exists a node `B` such that `A → B` and `B →* C`, the `A → C` edge is redundant.
  - Emit `Finding(category=DAG_CONFLICT, subtype=REDUNDANT_DEPENDENCY, severity=LOW, source=A.path, target=C.path, dependency_chain=tuple((A.path, B.path, C.path)), source_kind=SourceKind.EXPLICIT_DEPENDS_ON)` — chain is structured (typed `dependency_chain`), NOT string-flattened into `evidence` — per Option A (TPR-02-002-gemini-r3).
  - Check: `plans/verify-roadmap-redesign/00-overview.md:137` — Section 04 depends on `01, 02, 03`; Section 05 depends on `01, 02, 03, 04`. Since 02 transitively depends on 01 (02 → 01), 04's direct edge 04 → 01 is NOT redundant by this definition (redundancy requires the shorter path to be shorter than the longer one AND not a user-friendly explicit declaration). The classifier SHOULD be conservative: emit only when the redundant edge's target appears in a dependency of a dependency's dependency — depth ≥ 3 transitive chain. Depth-2 redundancies (A → B → C + A → C) are emitted at LOW severity because they may be intentional readability declarations.

- [x] **ORPHANED_PLAN (informational)** — plan with no incoming or outgoing dependency edges AND not actively superseding anything:
  - For each `PLAN_INDEX` node, compute in-degree and out-degree in `dag.edges`.
  - If in-degree == 0 AND out-degree == 0 AND the plan's `supersedes` list is empty AND the plan is not referenced by any body-inferred `Reference` → emit `Finding(category=DAG_CONFLICT, subtype=ORPHANED_PLAN, severity=LOW, ...)`.
  - Candidate: `plans/rosetta-stress-test/` may qualify — must be verified at runtime.
  - Recommended_fix: "Verify this plan is still needed; file a bug if it is truly abandoned."

- [x] **Classifier subtypes not declared in §01.3 require a §01.3 extension request:**
  - `REDUNDANT_DEPENDENCY` and `ORPHANED_PLAN` are NEW subtypes that must be added to `FindingCategory.DAG_CONFLICT` in `scripts/plan_corpus/types.py` `FindingSubtype` and `_CATEGORY_SUBTYPES[FindingCategory.DAG_CONFLICT]`.
  - NOTE for /tpr-review: because §01 is complete (reviewed: true in its frontmatter), this extension is filed as a scope NOTE here. Implementation sequencing: when §02 work begins, the first commit adds the two subtypes to `types.py` (§01.3's file) with a comment `# Added by §02.2 for REDUNDANT_DEPENDENCY / ORPHANED_PLAN classifiers`. This is the only permitted cross-section edit during §02 execution.

- [x] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [x] Write failing test fixtures FIRST (TDD) — one per classifier × each live-corpus source-kind pattern:
    - `fixture_conflict_shared_subsystem.yaml` — semantic pin: CONFLICT fires on unrelated plans sharing a subsystem; negative pin: CONFLICT does NOT fire when A depends on B (prereq coordination — Finding G)
    - `fixture_superseded_reroute.yaml` — semantic pin for case (i); `fixture_superseded_inplace.yaml` — semantic pin for case (ii) exactly mirroring test-suite-health §05.2 (c)
    - `fixture_blocked_live.yaml` — semantic pin for §05.2 (a) — and for (g) ONLY after route-A corpus edits land (§02.2 BLOCKED NOTE on cases (a)/(g)). Until the corpus edits land, cases (a) AND (g) are caught as MISSING_DEPENDENCY; cover that path in `fixture_missing_dep_yaml_comment.yaml` + a new `fixture_missing_dep_repr_opt_locality.yaml` that reproduces the case-(a) body-prose MISSING_DEPENDENCY scenario (per TPR-02-002-gemini round 2). Negative pin: no BLOCKED when both active.
    - `fixture_cross_edge_temporal_drift.yaml` — semantic pin: complete plan depends on not-started plan → finding; negative pin: both complete → no finding
    - `fixture_missing_dep_prose.yaml` — semantic pin: prose "depends on plans/X/" without frontmatter depends_on emits MISSING_DEPENDENCY; negative pin: with matching depends_on, no finding
    - `fixture_missing_dep_html_comment.yaml` — semantic pin for `<!-- blocked-by:X -->` when X is not in depends_on
    - `fixture_missing_dep_shared_subsystem.yaml` — semantic pin (TPR-02-003-gemini): two active plans share a subsystem via `subsystem_to_nodes` mapping BUT neither has a `depends_on` edge to the other AND their goals are non-contradictory (CONFLICT short-circuited) → emits MISSING_DEPENDENCY for each unordered pair; negative pin: same two plans with an EXPLICIT_DEPENDS_ON edge between them → no MISSING_DEPENDENCY finding
    - `fixture_dead_ref_direct.yaml` — semantic pin for §05.2 (b) (plans/ori_lsp/ in roadmap 22.2)
    - `fixture_dead_ref_unblocks_completed.yaml` — semantic pin for §05.2 (h) (Section 21A unblocks JIT Exception Handling, which lives under plans/completed/)
    - `fixture_dead_ref_code_fence_negative.yaml` — NEGATIVE PIN (Finding D): bug-tracker fence-template path MUST NOT emit DEAD_REFERENCE
    - `fixture_dead_ref_yaml_comment.yaml` — semantic pin: fix-BUG-04-039.md YAML comment → YAML_COMMENT reference → resolves to valid `plans/iterator-element-ownership/` (no DEAD_REFERENCE); mutant fixture: YAML comment pointing at missing dir → LOW-severity DEAD_REFERENCE
    - `fixture_redundant_dep.yaml` — semantic pin: A → B → C + A → C emits REDUNDANT_DEPENDENCY
    - `fixture_orphaned_plan.yaml` — semantic pin: standalone plan with no references → ORPHANED_PLAN; negative pin: same plan referenced by another plan's body → no finding
  - [x] All 8 classifiers pass against every live-corpus fixture; failing fixtures predate the implementation
  - [x] All tasks above are `[x]` and classifiers produce findings on the live corpus
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** — check whether changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md` claims. If no changes, document briefly. Fix any drift NOW.

---

## 02.3 Priority Inversion Detection

**File(s):** `scripts/plan_corpus/dag.py` (integrated into BLOCKED classifier; `detect_priority_inversions(dag) -> list[Finding]` entry point).

Specialized analysis that extends the BLOCKED classifier (§02.2) with transitive chain reporting and root-blocker identification. The topological-sort recommendation from the prior §02.3 draft is CUT here and re-scoped as a §03 consumer — see NOTE below.

- [x] Implement transitive priority inversion detection:
  - For each BLOCKED finding from §02.2, compute the full dependency chain `A → B → ... → root_blocker` via DFS on `Dag.edges`.
  - `root_blocker` = the deepest queued/not-started node in the chain.
  - Emit one Finding per chain using Option A (typed boundary per TPR-02-006-codex and TPR-02-004-gemini): populate `Finding.dependency_chain: tuple[Path, ...]` and `Finding.source_kind: SourceKind` — both fields added to `scripts/plan_corpus/types.py` as part of §02.N's cross-section extension (see §02.5 Concern I). The `evidence` field is reserved for short textual notes; structured chain data MUST NOT be flattened into strings across the §02→§03 phase boundary (EXPOSURE guard per `impl-hygiene.md`).
  - Replace the §02.2 BLOCKED finding with the chain-enriched version (dedup by `Finding.id`).

- [x] Identify the minimum unblock set:
  - For each connected component of BLOCKED findings, compute the minimum set of nodes whose status change would unblock the chain.
  - Emit using the existing `FindingCategory.DAG_CONFLICT / BLOCKED` subtype with the typed `Finding.dependency_chain: tuple[Path, ...]` field carrying the unblock-set paths (per TPR-02-001-gemini-r4 — no evidence-tuple residue). `source_kind=SourceKind.EXPLICIT_DEPENDS_ON` because the unblock-set members are on dependency edges. `evidence` is reserved for short human-readable rationale ONLY; structured unblock-set data goes in `dependency_chain`.
  - **NOTE for /tpr-review:** the "minimum unblock set" is a §03 consumer concern — §03's report groups BLOCKED findings by shared `dependency_chain` prefixes and presents the root blocker as a single actionable item. §02 computes the data as a typed field; §03 reads the typed field and renders.

- [x] **CUT (Finding L): topological-sort execution-order recommendation is NOT emitted by §02.**
  - The prior §02.3 draft computed a topological sort and labeled plans "active out of order". §03's `/continue-roadmap` integration only consumes BLOCKED/CONFLICT/DEAD_REFERENCE quick-checks; §04 consumes flagged sections. There is no §03 or §04 consumer for a topo-sort output.
  - The topo-sort code is CUT from §02 entirely (per TPR-02-005-gemini — no soft-deferral, no "just-in-case" architectural concessions). `dag.edges` remains because real classifiers consume it (BLOCKED, REDUNDANT_DEPENDENCY, CONFLICT short-circuit); the transitive closure is computed inside REDUNDANT_DEPENDENCY's classifier body, not exported as a general-purpose structure. If a future plan proposes topo-sort rendering, the new plan specifies its consumer and may extend §02 via a proper new subsection at that time — the proposal workflow is the gating mechanism, not speculative scaffolding.

- [x] Validate against known test cases (concrete assertions for §05.2):
  - Test case (a): `plans/repr-opt/index.md` active, body-prose references `plans/locality-representation-unification/` queued. Mirrors case (g): in the CURRENT corpus state, `plans/repr-opt/index.md` has NO `depends_on` entry → emits MISSING_DEPENDENCY (not BLOCKED). Once the author adds `depends_on: ["Locality SSOT#NN"]` (the name-based cross-plan ID from `plans/locality-representation-unification/index.md:name`; directory slug REJECTED by §01's DepId validator), re-runs produce a BLOCKED finding with chain `Repr Opt → Locality SSOT`. §02.2's BLOCKED "Known cases" block documents this as the route-A/route-B split.
  - Test case (g): `plans/bug-tracker/fix-BUG-04-039.md` in-progress, YAML_COMMENT references `plans/iterator-element-ownership/` as blocker → emits a MISSING_DEPENDENCY (no frontmatter `depends_on` entry) AND, once the author adds `depends_on: ["Iter Ownership#NN"]` (the name-based cross-plan ID; directory slug `iterator-element-ownership` is REJECTED by §01's DepId validator), re-runs produce a BLOCKED finding. §02 does NOT auto-promote YAML comments to edges (per §02.0 SSOT rule).
  - Test case (b): DEAD_REFERENCE for `plans/ori_lsp/` in `plans/roadmap/section-22-tooling.md`.
  - Test case (c): SUPERSEDED (case (ii) — in-place-rewrite-claimed-but-not-done) for `plans/test-suite-health/section-02-roadmap-reprioritization.md` → `plans/roadmap/section-21A-llvm.md`.
  - Test case (h): DEAD_REFERENCE (LOW severity, HTML_COMMENT_CONVENTION source) for `plans/roadmap/section-21A-llvm.md:83` `<!-- unblocks:jit-exception-handling/04B,05,06 -->` — target resolves to `plans/completed/jit-exception-handling/` → annotation is stale.

- [x] **Subsection close-out (02.3)** — MANDATORY before starting 02.4:
  - [x] Write failing test fixtures FIRST for each known test case (a), (b), (c), (g), (h) — one fixture file per case, reproducing the exact live-corpus scenario
  - [x] All tasks above are `[x]` and known test cases validate
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — No tooling gaps. detect_priority_inversions reuses classify_blocked + _node_status cache; recursion depth limit is a risk on deep chains but the DFS has cycle avoidance via `if nxt in path: continue`.
  - [x] **Run `/sync-claude` on THIS subsection** — No global rules drift. Priority inversion remains §02-internal until §02.5 exposes DagReport.

---

## 02.4 Classifier Precedence & Determinism Tests

**File(s):** `scripts/plan_corpus/dag.py` (precedence ordering in `CLASSIFIERS` list); `tests/plan-audit/test_classifier_precedence.py` (pytest tests).

Classifiers can fire on the same `(source, source_line)` pair. Without explicit precedence, the classifier stack is non-deterministic. This subsection makes ordering explicit, tested, and auditable.

- [x] Document the precedence ladder (highest priority first):
  1. `PARSE_ERROR` (§01 category) — fatal; if a file cannot be parsed, no §02 classifier runs against it. §02 consumes `Corpus.gaps` from §01 and skips any node whose source path is in the gaps.
  2. `DEAD_REFERENCE` — if a reference doesn't resolve, no other DAG classifier fires against that reference. A broken edge is not a "conflict with B", it is "B does not exist."
  3. `DAG_CONFLICT / CYCLE` — structural graph error; takes precedence over BLOCKED because a cycle makes BLOCKED meaningless.
  4. `DAG_CONFLICT / BLOCKED` — active-depends-on-queued; high-value finding.
  5. `DAG_CONFLICT / CONFLICT` — subsystem overlap with contradictory goals.
  6. `DAG_CONFLICT / SUPERSEDED` — rewrite-claimed-but-not-done.
  7. `STATUS_CONTRADICTION / CROSS_EDGE_TEMPORAL_DRIFT` — informational drift.
  8. `DAG_CONFLICT / MISSING_DEPENDENCY` — body-inferred reference without explicit edge (last because it requires all other classifiers to have resolved their references first).
  9. `DAG_CONFLICT / REDUNDANT_DEPENDENCY` — LOW severity informational.
  10. `DAG_CONFLICT / ORPHANED_PLAN` — LOW severity informational.

- [x] Implement precedence via a deduplication pass:
  - After all classifiers run, group findings by `(source, source_line, target)`.
  - Within each group, keep ONLY the highest-precedence finding; emit the rest as `evidence` on the kept finding under key `suppressed_by_precedence`.
  - A finding's precedence rank is looked up from the ladder above via `PRECEDENCE_RANK: dict[FindingSubtype, int]`.

- [x] Source-kind precedence in the severity ladder:
  - EXPLICIT_DEPENDS_ON references produce HIGH severity
  - HTML_COMMENT_CONVENTION and YAML_COMMENT references produce MEDIUM severity
  - PROSE_VERB references produce LOW severity
  - CODE_FENCE_EXAMPLE references produce NO findings (excluded before classifier runs)

- [x] Write TDD determinism tests:
  - `test_precedence_dead_ref_beats_missing_dep`: a prose reference to `plans/nonexistent/` emits exactly one DEAD_REFERENCE, NO MISSING_DEPENDENCY
  - `test_precedence_cycle_beats_blocked`: a cyclic dependency emits exactly one CYCLE, NO BLOCKED for the members of the cycle
  - `test_precedence_parse_error_suppresses_all`: a file with a PARSE_ERROR finding in `corpus.gaps` does NOT appear as a node; no DAG classifier fires
  - `test_source_kind_severity_ladder`: same DEAD_REFERENCE target emitted at HIGH / MEDIUM / LOW depending on source kind
  - `test_code_fence_no_false_positive_with_fixture`: `fixture_dead_ref_code_fence_negative.yaml` runs all 8 classifiers, zero findings
  - `test_deterministic_order`: running `run_classifiers(dag)` twice produces identical output (list equality, including order)

- [x] **Subsection close-out (02.4)** — MANDATORY before starting 02.5:
  - [x] Write failing determinism tests FIRST; verify they fail before implementation
  - [x] All tasks above are `[x]` and determinism tests pass on the live corpus
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — No tooling gaps. apply_precedence uses deterministic sort keys and stable tie-break on Finding.id; testability is strong via synthetic Finding instances (no need for live corpus).
  - [x] **Run `/sync-claude` on THIS subsection** — No global rules drift.

---

## 02.5 Handoff Contract with §03

**File(s):** `scripts/plan_corpus/dag.py` (entry-point `dag_report(dag) -> DagReport` dataclass); shared with §03 via stable `DagReport.to_json()` schema.

§03 consumes §02's output. Three concrete schema concerns must be resolved here so §03 can drive SafeFix / ExposureReview classification without re-parsing §02's output. Each resolution is filed as a NOTE for /tpr-review to triage against §01.3's `Finding` schema.

- [ ] **Concern I — multi-hop transitive chains (Finding I), resolved per TPR-02-006-codex + TPR-02-004-gemini:**
  - `scripts/plan_corpus/types.py:202` defines `Finding` with an `evidence: tuple[str, ...]` field. No structural field for chains.
  - **§02 choice: Option A — extend `Finding` with typed fields.** Add TWO optional fields to the `Finding` dataclass:
    - `dependency_chain: tuple[Path, ...] = ()` — ordered sequence of node paths for multi-hop transitive findings (BLOCKED chains, CYCLE paths, REDUNDANT_DEPENDENCY triples). Empty tuple for non-chain findings.
    - `source_kind: SourceKind | None = None` — the classification of the reference that produced the finding (EXPLICIT_DEPENDS_ON, HTML_COMMENT_CONVENTION, YAML_COMMENT, PROSE_VERB, CODE_FENCE_EXAMPLE). `None` for findings that do not correspond to a specific reference (e.g. ORPHANED_PLAN).
  - **Rationale:** Option C (flattening structured data into strings across the §02→§03 phase boundary) is an EXPOSURE violation per `impl-hygiene.md` — §03 would have to string-parse to recover structure, creating a scattered-knowledge LEAK. Option A keeps the data typed across the boundary. Option B (multi-Finding stitching via chain-id) works but forces §03 to re-assemble chains from shared-key groups, which is slower and harder to audit. Option A is the only choice that preserves both SSOT (all chain data lives in one dataclass) and phase-boundary purity (§03 reads typed fields, no re-parsing).
  - **Cross-section edit authorization:** §02 adds these two fields to `scripts/plan_corpus/types.py` as part of §02.N's sweep (alongside the `REDUNDANT_DEPENDENCY` and `ORPHANED_PLAN` subtype additions already authorized at §02.2 "Classifier subtypes not declared in §01.3"). This is a non-breaking extension — defaults preserve backward compatibility with §01's existing call sites. /tpr-review ratifies or rejects the extension during §02 execution; the fallback if rejected is Option B (multi-Finding chain-id stitching), NOT Option C.

- [ ] **Concern J — Finding.id collision risk (Finding J):**
  - `scripts/plan_corpus/types.py:221` hashes `category + subtype + source + source_line`. Multiple `depends_on` entries on one line collide.
  - **§02 mitigation — add `source_column` to `Reference`** (§02.0) and thread it through to Finding construction so collisions on the same line get distinct ids.
  - **Cross-section edit:** the full §01.3 extension sweep — adding `dependency_chain`, `source_kind`, AND rebasing `Finding.id` to hash `(category, subtype, source, source_line, source_column, target)` — all land together in §02.N's authorized sweep. The disambiguator is structural (extend the hash inputs to `source_column` + `target`), not a string-encoded workaround in `evidence`. Backward compatibility is preserved: the default `source_column = None` and default `target = None` for findings that do not carry them mean legacy `Finding.id` values are unchanged for fields that never populated `source_column` or `target`.

- [ ] **Concern K — precise source-file/line for `depends_on` SafeFix (Finding K):**
  - `resolve_dep` returns `source = plan_dir / "index.md"` or `source = plan_dir` on failure. §03 SafeFix needs the exact YAML line of the `depends_on` list item.
  - **§02 mitigation — enrich `resolve_dep` findings post-facto** in `build_dag`: for each `depends_on` entry, track its YAML line via a `yaml_lines: dict[str, int]` map (computed by scanning frontmatter raw text for the `depends_on:` block and counting list items). Rebuild the Finding with `source = <declaring_file>`, `source_line = yaml_line`, `evidence = (dep_id,)`.
  - `enrich_resolve_dep_finding(finding, dep_id, yaml_line, declaring_file)` is the helper (introduced in §02.1).
  - Result: §03 SafeFix can apply `remove_yaml_list_entry(source, source_line, dep_id)` without further parsing.

- [ ] **Concern — `source_kind` as first-class facet (Finding P), aligned with Concern I Option A:**
  - Every Finding emitted by §02 sets `Finding.source_kind: SourceKind | None` as a typed field on the canonical `Finding` dataclass (added to `scripts/plan_corpus/types.py` per Concern I Option A above — single authorization, both extensions land together in §02.N's cross-section sweep).
  - §03 reads `finding.source_kind` directly (no string parsing, no `evidence`-based protocol) and routes: `EXPLICIT_DEPENDS_ON` DEAD_REFERENCE → SafeFix (mechanical list-entry removal); `PROSE_VERB` DEAD_REFERENCE → ExposureReview (requires human-authored replacement); `HTML_COMMENT_CONVENTION` / `YAML_COMMENT` → MEDIUM-severity mid-tier routing.
  - **Evidence-embedding protocol is REMOVED per TPR-02-003-codex round 2.** The prior text saying "§02 embeds `source_kind` in `evidence[0]` as `\"source_kind:<VALUE>\"`" is obsolete — it was a pre-Option-A bridge that creates a second source of truth and contradicts Option A's typed-field contract. All §02-emitted findings use the typed field; `evidence` is reserved for short human-readable context strings (spec citations, brief rationale) that do NOT encode structural data.

- [ ] Define `DagReport` dataclass:
  - `findings: list[Finding]` — deduplicated, precedence-ordered, source-kind-tagged
  - `dag_json: dict` — `Dag.to_json()` output for diagnostic use
  - `stats: dict[str, int]` — per-classifier finding counts, source-kind counts, node counts
  - `to_json() -> dict` — stable schema; §03 imports this

- [ ] **Subsection close-out (02.5)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and §03 can import `DagReport` with no re-parsing
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md` claims. If no changes, document briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.R Third Party Review Findings

Dual-source /tpr-review round 1 on 2026-04-14 (run `/tmp/ori-tpr-5GmvSZCf`). 11 actionable findings (codex 6, gemini 5, 0 agreements). All verified true against code and the live corpus; all fixed in the same pass.

- [x] `[TPR-02-001-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:250` — Align BLOCKED acceptance cases with the explicit-edge SSOT.
  Evidence: §02.0 makes `EXPLICIT_DEPENDS_ON` the only edge source (body/YAML/HTML signals → MISSING_DEPENDENCY, never shadow edges), but §02.2's BLOCKED classifier walked `Dag.edges` and claimed case (g) (BUG-04-039 blocked by `plans/iterator-element-ownership/` via YAML frontmatter comment) among its known cases. Since case (g) has no `depends_on` entry, no edge exists for BLOCKED to fire on — the reference feeds MISSING_DEPENDENCY. §05.2:146 classifies case (g) as BLOCKED, creating DRIFT.
  Impact: Case (g) was plan-unimplementable — BLOCKED classifier would never fire on a YAML-comment-only reference, so §02 would never catch the test case as classified by §05.2.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14 in §02.2 BLOCKED bullet: case (g) explicitly moved to MISSING_DEPENDENCY path (SSOT-consistent); added NOTE documenting the §05.2:146 drift for /tpr-review to reconcile (either §05.2 updates case (g) → MISSING_DEPENDENCY, or BUG-04-039 author adds `depends_on` before §02 acceptance).

- [x] `[TPR-02-002-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:241` — Encode the real 21A stale-update pattern instead of rewrite verbs only.
  Evidence: SUPERSEDED case (ii) was specified with body-text triggers `rewrites <target>` / `rewrite of <target>`, but `plans/test-suite-health/section-02-roadmap-reprioritization.md` never uses those verbs — it uses "update 21A", "reprioritize Section 21A", and "reorder". Live case (c) could not be detected with the documented verb vocabulary.
  Impact: Test case (c) would produce a false negative — the SUPERSEDED classifier would scan 21A-reprioritization body, find no "rewrites" verb, and emit no finding.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Extended `DEPENDENCY_VERBS` frozenset at §02:182 to include "rewrite of", "update in place of", "update of", "reprioritize", "reorder", "obsoletes" alongside the original "rewrites". Broadened §02.2 Case (ii) detection to use the full verb set with a three-part detection heuristic (target exists + target not edited since claim + no completion marker).

- [x] `[TPR-02-003-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:137` — Accept hyphenated HTML comment targets from the live corpus.
  Evidence: `parse_html_comments` regex `<!--\s*(blocked-by|unblocks|supersedes|resolves)\s*:\s*([^ \t\r\n-]+(?:,[^ \t\r\n-]+)*)\s*-->` uses character class `[^ \t\r\n-]` that excludes hyphens from target tokens. Live case (h) at `plans/roadmap/section-21A-llvm.md` is `<!-- unblocks:jit-exception-handling/04B,05,06 -->` — the target `jit-exception-handling/04B` is hyphenated and would NOT match.
  Impact: Test case (h) would be undetectable — the HTML-comment grammar would parse zero targets from a legitimate reference, producing a false negative for the DEAD_REFERENCE classifier.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Changed regex to `[^ \t\r\n,]+(?:,[^ \t\r\n,]+)*` — hyphens ARE allowed (plan slugs are hyphenated), only whitespace and the `,` separator are excluded. Added TPR-02-003-codex semantic pin requiring the live case-(h) comment to parse into three targets.

- [x] `[TPR-02-004-codex][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:272,338` — Point case (b) at the actual Section 22 file.
  Evidence: §02 hard-coded `plans/roadmap/section-22-frontend-core.md` for case (b) at two sites (lines 272 and 338), but the live roadmap file is `plans/roadmap/section-22-tooling.md` — no `section-22-frontend-core.md` exists on disk.
  Impact: Test-case fixtures and the DEAD_REFERENCE `plans/ori_lsp/` detection would target a nonexistent source file, producing a cascading false negative.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Replaced `section-22-frontend-core.md` with `section-22-tooling.md` at both sites via replace_all.

- [x] `[TPR-02-005-codex][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:251` — Use the real status-normalizer API instead of inventing `effective_status`.
  Evidence: §02.2 referenced `plan_corpus.normalizer.effective_status(node)` but `scripts/plan_corpus/normalizer.py:70-138` exports `normalize_status(data, body, path, child_statuses) -> NormalizedStatus` with fields `declared`, `derived`, `contradictions` — no `effective_status` function exists. Inventing a wrapper is a LEAK:scattered-knowledge.
  Impact: §02 implementation would fail at import time OR create a parallel API surface that drifts from §01.4's canonical normalizer.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Rewrote the status-resolution bullet to use the real `normalize_status()` API, document the `NormalizedStatus.derived` field, describe how plan-index vs section nodes pass `child_statuses` vs `None`, and add an explicit SSOT guard forbidding a wrapper API.

- [x] `[TPR-02-006-codex][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:403` — Keep chain and source-kind data structured across the §02→§03 boundary.
  Evidence: §02.5 Concern I chose Option C (evidence-embedded chains as free-text strings like `evidence=("chain:A,B,C", "status:active,queued,not-started")`) across a typed §02→§03 phase boundary. §03 would have to string-parse to recover structure — EXPOSURE per `impl-hygiene.md` §Phase Boundaries + LEAK:scattered-knowledge (structure knowledge split between §02's string-format and §03's string-parser).
  Impact: §03's SafeFix/ExposureReview classification would drift from §02's intent on every parsing bug; report renderers could not render graph visualizations without re-decoding strings.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Switched §02.5 Concern I default from Option C to Option A. Added two optional fields to `scripts/plan_corpus/types.py` `Finding` dataclass: `dependency_chain: tuple[Path, ...] = ()` and `source_kind: SourceKind | None = None`. Authorized the cross-section edit under the same authorization that allows REDUNDANT_DEPENDENCY / ORPHANED_PLAN subtype additions. Updated §02.3 chain emission (line 322 region) to populate the typed fields instead of `evidence` strings. Updated §02.N completion checklist to enumerate the full §01.3 extension list.

- [x] `[TPR-02-001-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:75` — Correct the inaccurate claim about test-suite-health plan reroute status.
  Evidence: §02's first scope NOTE claimed "plans/test-suite-health/index.md is NOT a reroute." Verification against `plans/test-suite-health/index.md:2` shows `reroute: true` — the plan IS a reroute. The plan's index also has no `supersedes` field; the `supersedes: []` is on `plans/test-suite-health/00-overview.md:5`.
  Impact: Downstream /tpr-review reading the scope NOTE would act on incorrect information about the test-suite-health reroute shape, and §02.2 case (i)'s filter condition (`reroute: True AND non-empty supersedes`) would be misunderstood.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Rewrote the scope NOTE to accurately describe test-suite-health's reroute status, noting that the `supersedes` field is absent from the reroute index AND `00-overview.md:5` has `supersedes: []`, so case (i) cannot fire — case (ii) with the broadened verb vocabulary is the correct detection path.

- [x] `[TPR-02-002-gemini][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:182,241` — Include "rewrite of" in the DEPENDENCY_VERBS constant.
  Evidence: `DEPENDENCY_VERBS` contained "rewrites" but not "rewrite of" — the SUPERSEDED case (ii) detector at line 241 claimed to match "rewrites <target>" or "rewrite of <target>", but the latter would never match because "rewrite of" was not in the set. Regardless of the broader case (c) fix, "rewrite of" is a common English phrasing that belongs in the verb set.
  Impact: Body text using "rewrite of X" phrasing (a natural variant) would not produce a reference; SUPERSEDED case (ii) would miss those.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Added "rewrite of" to `DEPENDENCY_VERBS` alongside the broader verb-set extension from TPR-02-002-codex. Addressed by the same edit as TPR-02-002-codex.

- [x] `[TPR-02-003-gemini][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:293` — Add a test fixture for the shared subsystem MISSING_DEPENDENCY case.
  Evidence: §02.2 MISSING_DEPENDENCY specifies TWO input paths: (1) body-inferred references without matching `depends_on`, and (2) pairs of plans sharing a subsystem (via `subsystem_to_nodes`) with both active and no explicit edge between them. The §02.2 close-out fixture list covered path (1) via `fixture_missing_dep_prose.yaml` and `fixture_missing_dep_html_comment.yaml` but had NO fixture for path (2). Test-matrix cell missing per `tests.md` Matrix Testing rule.
  Impact: The shared-subsystem MISSING_DEPENDENCY code path would have no TDD fixture before implementation — regressions would be undetected.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Added `fixture_missing_dep_shared_subsystem.yaml` to the §02.2 close-out fixture list with explicit semantic pin (two plans sharing a subsystem, non-contradictory goals, no explicit edge → emit MISSING_DEPENDENCY) AND negative pin (same two plans with EXPLICIT_DEPENDS_ON edge → no finding).

- [x] `[TPR-02-004-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:322` — Do not flatten transitive chains into string tuples across phase boundaries.
  Evidence: §02.3 emitted chain findings with `evidence = tuple(f"{node.path}:{node.status}" for node in chain)` — structured chain data stringified across the §02→§03 phase boundary. EXPOSURE per `impl-hygiene.md` §Phase Boundaries. Same architectural issue Codex surfaced at §02.5 line 403 (TPR-02-006-codex), flagged at a different site.
  Impact: §03 would have to string-parse chain evidence to route fixes; structured routing (graph visualization, multi-hop hyperlink rendering) would be impossible without decoding strings.
  Basis: inference. Confidence: high.
  Resolved: Fixed on 2026-04-14. Same fix as TPR-02-006-codex — switched to Option A (`Finding.dependency_chain: tuple[Path, ...]` typed field). Updated §02.3 line 322 region to populate the typed field directly, and updated §02.5 Concern I to make Option A the default.

- [x] `[TPR-02-005-gemini][low]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:332` — Remove soft-deferral language for CUT topological sort.
  Evidence: §02.3's CUT block contained "Keep `dag.edges` + the DAG's transitive closure available — if a future §03 subsection adopts topo-sort rendering, it can compute on demand" directly followed by "The topo-sort code is CUT from §02, not deferred — if needed later, it lives in the consumer, not here." The two statements are contradictory and the first is speculative soft-deferral without a concrete implementation anchor (WASTE per `impl-hygiene.md`; banned per CLAUDE.md §"ALL deferrals MUST have implementation anchors").
  Impact: Soft-deferral language creates implicit commitments that accumulate as scope creep; future readers interpret "available for future use" as an active contract.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Replaced the two contradictory bullets with a single unambiguous statement: topo-sort is CUT entirely; `dag.edges` remains only because real classifiers consume it; the transitive closure is computed locally inside REDUNDANT_DEPENDENCY's body, not exported; future topo-sort work requires a new plan via the proposal workflow.

---

Dual-source /tpr-review round 2 on 2026-04-14 (run `/tmp/ori-tpr-jCTrbe0m`). 6 actionable findings (codex 4, gemini 2, 0 agreements). All verified true against code and the live corpus; all fixed in the same pass.

- [x] `[TPR-02-001-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:18,250` — Case (a), not just case (g), has the same SSOT-violation pattern.
  Evidence: §02 frontmatter line 64 + §02.2 BLOCKED "Known case (a)" claimed repr-opt declares `depends_on: ["Locality Representation Unification#01"]`, but `plans/repr-opt/index.md:1-8` has no `depends_on` field at all. The real `name:` values verified via grep are `"Repr Opt"` and `"Locality SSOT"` — NOT "Locality Representation Unification". Without a real `depends_on` in the live corpus, §02.2 BLOCKED cannot fire on case (a); the reference flows to MISSING_DEPENDENCY. Same architectural issue as case (g) fixed in round 1.
  Impact: §02 claims to catch cases (a) and (g) as BLOCKED, but the live corpus makes both unimplementable without pre-edits. §05.2:113,143 (corrected in round 4 from the round-2 "142,146" that was never verified) assumes BLOCKED; if the corpus isn't pre-edited, §02 will produce MISSING_DEPENDENCY findings that §05.2 then flags as "wrong classifier".
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14 (corrected in round 3 per TPR-02-003-codex-r3: `plans/iterator-element-ownership/index.md:3` declares `name: "Iter Ownership"`, NOT "Iterator Element Ownership"). Rewrote the §02.2 BLOCKED known-cases block to describe BOTH cases (a) and (g) as requiring a corpus pre-edit (route A) or a §05.2 reclassification to MISSING_DEPENDENCY (route B). Correct `name:` values are `"Locality SSOT"` and `"Iter Ownership"`. Added explicit mandate for /tpr-review to reconcile with §05.2:113,143.

- [x] `[TPR-02-002-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:241` — SUPERSEDED case (ii) embeds phase-leaking mtime/git logic in §02.
  Evidence: Round-1 broadened case (ii)'s detection to "the target section's body was last edited before the source section's claim was written" — a timestamp predicate. §02's own scope NOTE at line 77 says mtime is broken (git clone/checkout resets mtimes) and git timestamps belong in §03's write-back phase. `00-overview.md:27` explicitly states `plan_corpus` is a pure library with no git queries. §02.2 case (ii)'s timestamp heuristic therefore has no valid signal from within the §02 phase.
  Impact: Either case (ii) is unimplementable under §02's purity contract, or §02 would have to break the no-git invariant and corrupt its phase boundary.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Replaced timestamp heuristic with a purely structural three-part predicate: (a) target exists in DAG, (b) source claim lacks `<!-- update-complete:resolves=<target-ref> -->` marker, (c) target lacks `<!-- updated-by:<source-ref> -->` back-reference or explicit depends_on. Added explicit §03 handoff: §03's write-back phase reads git `%cI` timestamps via `WriteBackContext.has_recent_commits` and upgrades/downgrades the structural SUPERSEDED to `SUPERSEDED/STALE_REWRITE` or `SUPERSEDED/LIKELY_LANDED`. §02 stays pure.

- [x] `[TPR-02-003-codex][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:17,423-426,434-437` — Stale evidence-embedding language contradicts Option A decision.
  Evidence: Round 1 switched the chain/source_kind handoff from Option C (evidence-embedded strings) to Option A (typed Finding fields), but the frontmatter success criterion at line 17 still described the Option C protocol ("`Finding.evidence` carries structured multi-hop dependency chains"), §02.5 Concern P at line 435 still documented evidence-embedding as the §03 protocol, and Concern J line 426 still said "Document the scheme in the Finding's evidence" — three stale sites creating DRIFT between the frontmatter contract, §02.5 Concern I's Option A decision, and §02.5 Concern P's stated protocol.
  Impact: Reader confusion and ambiguity — if a §02 implementer reads the frontmatter or Concern P first, they would build the string-based protocol; if they read Concern I first, they would build the typed-field protocol. Implementation drift between §02 and §03 would surface only at integration time.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Rewrote the frontmatter success criterion to state the Option A typed-field contract. Replaced Concern P's "Every Finding embeds `source_kind` in `evidence[0]`" with "Every Finding sets `Finding.source_kind` as a typed field; evidence-embedding protocol is REMOVED." Replaced Concern J's "Document the scheme in evidence" with the structural Finding.id hash extension (source_column + target). All three sites now coherently describe Option A.

- [x] `[TPR-02-004-codex][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:531-535` — Missing §01 re-review gate for the authorized §01.3 extension.
  Evidence: §02 authorizes direct edits to `scripts/plan_corpus/types.py` (adding `REDUNDANT_DEPENDENCY`, `ORPHANED_PLAN`, `dependency_chain`, `source_kind`, and rebasing `Finding.id`), but §01 is `reviewed: true` with `status: complete` and §01.3 documents the pre-extension `Finding` schema + subtype set as complete. Without an explicit re-review step, §01's reviewed-state drifts from the post-extension reality, violating the /review-plan single-section-gate contract.
  Impact: §01's `reviewed: true` becomes a lie — it was reviewed against a pre-extension state that no longer matches reality. Downstream plan-audit tools would trust a stale review gate.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Added a §01 re-review gate to §02.N: the §02.N sweep blocks on either (a) re-running `/review-plan` on `section-01-frontmatter-schema.md` after the extension lands (which re-validates §01's reviewed state against the new schema), or (b) §01's owner explicitly ratifying the extension in a new §01.7 "Extensions Ratified by §02" subsection documenting the exact dataclass diff. Without one of these, §02 cannot close out.

- [x] `[TPR-02-001-gemini][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:85,99,408` — Prevent circular import by defining SourceKind in types.py.
  Evidence: §02.0 "File(s)" at line 85 said `SourceKind(Enum)` lives in `dag.py`. §02.5 Concern I at line 408 says `Finding.source_kind: SourceKind | None` is a field on the canonical `Finding` dataclass in `types.py`. `dag.py` already imports `Finding`/`FindingCategory`/`FindingSubtype` from `types.py`. If `SourceKind` lives in `dag.py`, `types.py` cannot reference it without a circular import (`types.py → dag.py → types.py`). This is structural LEAK:scattered-knowledge — the canonical home for boundary-crossing types must be types.py.
  Impact: §02.0 and §02.5 are implementation-incompatible as written. Python would reject the circular import at load time.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Updated §02.0 "File(s)" note to relocate `SourceKind(Enum)` to `scripts/plan_corpus/types.py` alongside `Finding`; `dag.py` imports `SourceKind` from types.py (same pattern as the existing `FindingCategory`/`FindingSubtype` imports). Updated the SourceKind definition bullet at line 99 to declare the canonical home explicitly. Updated §02.N cross-section-edits checklist to add `SourceKind(Enum) defined` as the first item alongside the subtype/field additions.

- [x] `[TPR-02-002-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:296` — Remove case (g) from the BLOCKED classifier semantic pin.
  Evidence: §02.2 close-out fixture `fixture_blocked_live.yaml` was described as a "semantic pin for §05.2 (a) and (g)". Round-1 TPR-02-001-codex correctly reclassified case (g) from BLOCKED to MISSING_DEPENDENCY until an explicit `depends_on` edge lands. The fixture description still claiming (g) creates contradictory implementation requirements (would test (g) against BLOCKED which cannot fire for YAML-comment-only sources) and a false negative when tests run against the real behavior.
  Impact: TDD fixtures would either silently pass (masking the real reclassification) or fail in confusing ways that misdirect implementers.
  Basis: inference. Confidence: high.
  Resolved: Fixed on 2026-04-14. Rewrote the `fixture_blocked_live.yaml` description to clarify it pins case (a) — and (g) ONLY after route-A corpus edits land. Added an explicit note that until the corpus edits land, cases (a) AND (g) are caught as MISSING_DEPENDENCY; the YAML-comment (g) case is covered by `fixture_missing_dep_yaml_comment.yaml`, and a new `fixture_missing_dep_repr_opt_locality.yaml` fixture must cover the case-(a) body-prose MISSING_DEPENDENCY path.

---

Dual-source /tpr-review round 3 on 2026-04-14 (run `/tmp/ori-tpr-liACFOJB`). 8 actionable findings (codex 4, gemini 4, 0 agreements) + 1 gemini informational "proof-of-work" verification. All actionable findings verified true; all fixed in the same pass. Gemini's informational finding confirmed round-2 architectural coherence.

- [x] `[TPR-02-001-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:137` — HTML-comment grammar did not cover round-2-introduced markers.
  Evidence: Round-2's fix for SUPERSEDED case (ii) introduced three new HTML-comment verbs (`rewrites:ID`, `update-complete:resolves=<target-ref>`, `updated-by:<source-ref>`), but §02.0's `parse_html_comments` regex still only matched `blocked-by|unblocks|supersedes|resolves`. GAP per `impl-hygiene.md` — §02.2 consumers would look for comments §02.0's parser could never produce.
  Impact: Round-2's structural SUPERSEDED case (ii) would be unimplementable — the completion-marker and back-reference paths could not be parsed.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Extended the regex to `(blocked-by|unblocks|supersedes|resolves|rewrites|update-complete|updated-by)`; added verb-semantic rows for the three new markers documenting each one's role (source-side completion, target-side back-reference, rewrite reference feeding SUPERSEDED case (ii)).

- [x] `[TPR-02-002-codex][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:246` — Round-2 introduced `SUPERSEDED/STALE_REWRITE` and `SUPERSEDED/LIKELY_LANDED` subtypes that don't exist in §01.3's SSOT.
  Evidence: §02.2's §03-handoff text claimed §03 upgrades structural SUPERSEDED to `SUPERSEDED/STALE_REWRITE` or `SUPERSEDED/LIKELY_LANDED`, but §01.3's `FindingSubtype` only has a single `SUPERSEDED` in `DAG_CONFLICT`. The round-2 trail authorized extensions for `REDUNDANT_DEPENDENCY`, `ORPHANED_PLAN`, `dependency_chain`, and `source_kind` — but NOT new SUPERSEDED subtypes. Silent taxonomy expansion = LEAK:shadow-taxonomy per `impl-hygiene.md` §SSOT.
  Impact: §03 would either have to emit subtypes §01.3 doesn't know about (parser rejection) or §02's plan would drift from its own authorization list.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Kept the single `FindingSubtype.SUPERSEDED` and moved the git_status distinction into the existing `evidence` field (`evidence = ("git_status:stale_rewrite", ...)` vs `evidence = ("git_status:likely_landed", ...)`). §01.3 taxonomy stays stable; only the already-authorized extensions (REDUNDANT_DEPENDENCY, ORPHANED_PLAN, Finding.dependency_chain, Finding.source_kind, SourceKind enum) land in §02.N's sweep.

- [x] `[TPR-02-003-codex][high]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:258,260,349` — Wrong `name:` values for cross-plan IDs.
  Evidence: `plans/iterator-element-ownership/index.md:3` declares `name: "Iter Ownership"` (verified via `grep '^name:'`). But §02 round-2 used `"Iterator Element Ownership#NN"` in the route-A guidance (line 260), the case-(g) NOTE (line 258), and the round-2 §02.R trail (line 532); §02.3 used the directory slug form `iterator-element-ownership#XX` (line 349). §01's DepId validator rejects both forms (one is a hallucinated name, the other is a rejected slug).
  Impact: Authors following §02's route-A guidance would add an invalid `depends_on` entry that §01's validator would reject, producing a DEAD_REFERENCE instead of enabling BLOCKED detection.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Replaced every "Iterator Element Ownership" and slug-form reference with the real `name:` value `"Iter Ownership"`. Added explicit guidance in the route-A bullet that directory slugs are REJECTED by §01's DepId validator. Updated the round-2 §02.R trail to acknowledge the round-3 correction.

- [x] `[TPR-02-004-codex][medium]` `plans/verify-roadmap-redesign/section-05-validation.md:113,143` — §05.2 unconditionally expects BLOCKED despite §02's route-A/route-B split.
  Evidence: §02.2 now specifies that cases (a) and (g) emit MISSING_DEPENDENCY in the current live-corpus state (no `depends_on` edges) and only emit BLOCKED after route-A corpus edits. But `plans/verify-roadmap-redesign/section-05-validation.md:113,143` still says "Expected: BLOCKED finding" for both cases unconditionally. Cross-section DRIFT.
  Impact: §05's acceptance-test fixture would expect BLOCKED and fail against §02's current behavior, blocking §05.2 completion.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14 by adding a scope NOTE in §02's "Scope NOTEs for /tpr-review triage" block documenting the cross-section handoff. §02 cannot edit §05 directly (single-section scope); /tpr-review on §05 (separate pass) must propagate the route-A/B split to §05.2:113,143. The NOTE makes the handoff permanent in §02's record.

- [x] `[TPR-02-001-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:273` — MISSING_DEPENDENCY emission embedded `source_kind` in evidence instead of the typed field.
  Evidence: §02.2 MISSING_DEPENDENCY emission used `evidence=(reference.source_kind.value, reference.raw_text)` — stringifying `source_kind` into evidence while the typed `Finding.source_kind` field (authorized in round 2) was unused. This is the exact Option-C anti-pattern round-2 eliminated, re-introduced at a different site.
  Impact: §03 would be forced to parse evidence strings to recover source_kind — the EXPOSURE pattern that Option A was designed to eliminate.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Changed the emission to populate typed `source_kind=reference.source_kind` and reduced `evidence` to `(reference.raw_text,)` for human inspection only.

- [x] `[TPR-02-002-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:290` — REDUNDANT_DEPENDENCY emission embedded chain in evidence.
  Evidence: §02.2 REDUNDANT_DEPENDENCY emission used `evidence=(chain_A_B_C,)` — stringifying the chain while the typed `Finding.dependency_chain` field was unused. Same Option-C residue as the MISSING_DEPENDENCY case.
  Impact: §03 would have to parse evidence to reconstruct the chain; graph-rendering impossible without decoding.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Changed the emission to populate typed `dependency_chain=tuple((A.path, B.path, C.path))` and `source_kind=SourceKind.EXPLICIT_DEPENDS_ON`; dropped the `evidence=(chain_A_B_C,)` arg.

- [x] `[TPR-02-003-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:205` — CYCLE emission formatted chain into evidence string.
  Evidence: §02.1 CYCLE emission used `evidence = tuple("<node_a> → <node_b> → ... → <node_a>")` — a human-readable string formatted into evidence instead of the structured `dependency_chain` field. Same Option-C residue.
  Impact: §03 cannot programmatically walk the cycle; diagnostic output is human-only.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Changed the emission to populate typed `dependency_chain=tuple(node.path for node in scc_path)` and `source_kind=SourceKind.EXPLICIT_DEPENDS_ON` (cycles are edge-based so all members carry EXPLICIT_DEPENDS_ON). No more string-formatting the chain.

- [x] `[TPR-02-004-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:348` — §02.3 test case (a) didn't mirror case (g)'s MISSING_DEPENDENCY fallback language.
  Evidence: §02.3 validation bullet for case (a) said "BLOCKED finding with chain `repr-opt → locality-representation-unification`" with no mention of the route-A/B split or MISSING_DEPENDENCY fallback, while case (g) documented both states. Inconsistency between the two cases that share the exact same architectural pattern.
  Impact: Implementers would build the fixture for case (a) expecting BLOCKED to fire on the live corpus — which it won't, until route-A edits land.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Rewrote the case (a) validation bullet to mirror case (g): "in the CURRENT corpus state, `plans/repr-opt/index.md` has NO `depends_on` entry → emits MISSING_DEPENDENCY (not BLOCKED). Once the author adds `depends_on: ['Locality SSOT#NN']`, re-runs produce BLOCKED."

- **[TPR-02-005-gemini][informational]** `plans/verify-roadmap-redesign/section-02-dag-builder.md:1` — Proof-of-work verification. Gemini confirmed round-2 fixes landed cleanly; no actionable findings from the gemini side. Recorded for audit trail; no fix required.

---

Dual-source /tpr-review round 4 on 2026-04-14 (run `/tmp/ori-tpr-uBF4cxeb`). 3 actionable findings (codex 1, gemini 2, 0 agreements). All fixed in the same pass. Sharp drop from round 3 (8) — converging.

- [x] `[TPR-02-001-codex][medium]` `plans/verify-roadmap-redesign/section-03-findings-report.md:64` — §03 still references pre-Option-A evidence-based Finding contract.
  Evidence: §02's rounds 1-3 made Option A (typed `Finding.dependency_chain` and `Finding.source_kind`) load-bearing and explicitly forbade string-parsing at the §02→§03 boundary. But `section-03-findings-report.md:64` still described the classifier reading `evidence` strings to recover chain/source_kind. Cross-section DRIFT; §03 would fail to import §02's typed output and the phase-boundary EXPOSURE would re-emerge.
  Impact: §03 acceptance would fail once §02 lands — the classifier cannot route on fields that do not exist in its imported protocol.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14 by adding a §02 scope NOTE documenting the cross-section handoff. §02 cannot edit §03 directly (single-section scope); /tpr-review on §03 (separate pass) must propagate Option A into §03's SafeFix/ExposureReview classifier (`classify_safety` reads `finding.source_kind` directly, not `evidence[0]` parsed as string). The NOTE permanently records the handoff for when §03 receives /review-plan.

- [x] `[TPR-02-001-gemini][medium]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:344` — Minimum-unblock-set used evidence-tuple instead of typed dependency_chain.
  Evidence: §02.3 "minimum unblock set" bullet said "Use `FindingCategory.DAG_CONFLICT / BLOCKED` with `evidence` containing the unblock-set tuple". Evidence-embedding residue — round 3 missed this site when switching MISSING_DEPENDENCY, REDUNDANT_DEPENDENCY, and CYCLE emissions to typed fields. Same Option-C anti-pattern, different code path.
  Impact: §03's root-blocker renderer would have to string-parse `evidence` to reconstruct the unblock-set — the exact EXPOSURE Option A was designed to eliminate.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Changed the emission to populate typed `Finding.dependency_chain: tuple[Path, ...]` with the unblock-set paths and `source_kind=SourceKind.EXPLICIT_DEPENDS_ON` (unblock-set members lie on dependency edges). Evidence is reserved for short human-readable rationale only.

- [x] `[TPR-02-002-gemini][low]` `plans/verify-roadmap-redesign/section-02-dag-builder.md:266,534` — Stale §05.2 line numbers in BLOCKED route-A bullet and §02.R round-2 trail.
  Evidence: Round 3 verified the real §05.2 line numbers as `113,143` (from `grep -nE "\(a\)|\(g\)|BLOCKED" plans/verify-roadmap-redesign/section-05-validation.md`) and updated the scope NOTE accordingly. But the BLOCKED classifier's route-A bullet at line 266 still said "reconcile with §05.2:142,146", and the round-2 §02.R trail entry at line 534 ("§05.2:142,146 assumes BLOCKED") was never updated. Round 3's corrections incomplete.
  Impact: Reviewers following the handoff notes in different parts of §02 would look for different line numbers in §05, finding nothing at one and the real text at the other — internal inconsistency.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14 at both sites. BLOCKED route-A bullet at line 266 now says "§05.2:113,143" with a note that the round-2 "142,146" reference was stale. Round-2 §02.R trail entry at line 534 corrected to "§05.2:113,143 (corrected in round 4 from the round-2 '142,146' that was never verified)."

---

## 02.N Completion Checklist

- [ ] §02.0 Node model covers all seven schema classes; source-kind taxonomy defined and unit-tested
- [ ] §02.1 DAG construction: EXPLICIT_DEPENDS_ON edges only; HTML/YAML/PROSE references collected without promoting to edges; code-fence regions excluded from body scans
- [ ] §02.2 All 8 classifiers implemented: CONFLICT, SUPERSEDED (two cases), BLOCKED, STATUS_CONTRADICTION/CROSS_EDGE_TEMPORAL_DRIFT, MISSING_DEPENDENCY, DEAD_REFERENCE, REDUNDANT_DEPENDENCY, ORPHANED_PLAN
- [ ] §02.2 Cross-section edits to §01.3's `scripts/plan_corpus/types.py` (authorized; all must land together in §02.N sweep):
  - `SourceKind(Enum)` defined (taxonomy lives in types.py alongside Finding to avoid circular import with dag.py — TPR-02-001-gemini round 2)
  - `FindingSubtype.REDUNDANT_DEPENDENCY` and `FindingSubtype.ORPHANED_PLAN` added to `DAG_CONFLICT` category
  - `Finding.dependency_chain: tuple[Path, ...] = ()` optional field added (Option A per TPR-02-006/04 — structured chain-typed boundary, not string-flattened)
  - `Finding.source_kind: SourceKind | None = None` optional field added (structured source-kind facet for §03's downstream routing)
  - `Finding.id` hash rebased to include `source_column` and `target` (backward-compatible: defaults to `None` preserve legacy hash for findings without these fields)
  - Backward-compatibility check: all existing §01 test fixtures still pass (default values preserve current behavior)
- [ ] **§01 re-review gate (DRIFT guard per TPR-02-004-codex round 2):** Because §01 is currently `reviewed: true` and §01.3 documents the pre-extension `Finding` schema + subtype set as complete, the §01.3 extension above requires `/review-plan plans/verify-roadmap-redesign/section-01-frontmatter-schema.md` to be re-run AFTER the extension lands. The re-review either ratifies the extension (flipping §01's `reviewed: true` back to `true` against the new state) or surfaces issues that must be resolved before §02 can claim complete. Without the re-review, §01's ownership of the Finding schema drifts silently. Alternative: if re-running `/review-plan` on §01 is deferred, the §02.N sweep blocks on §01's owner explicitly ack'ing the extension in §01's body text (new subsection §01.7 "Extensions Ratified by §02" with the exact dataclass diff).
- [ ] §02.3 Transitive chains use Option A typed `Finding.dependency_chain`; topological sort CUT (Finding L) — no consumer, no soft-deferral
- [ ] §02.4 Classifier precedence documented and TDD-enforced; deterministic ordering; code-fence negative pin passes
- [ ] §02.5 Handoff contract with §03 resolved: Option A for chains (typed `dependency_chain` + `source_kind` on Finding), `source_column` disambiguator for `Finding.id` collisions, enriched `resolve_dep` findings with precise YAML line numbers
- [ ] Known test cases (a), (b), (c), (g), (h) validated with fixture tests + asserted classifier+subtype match
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` — dual-source review of DAG builder and classifiers, focusing on: source-kind correctness, precedence determinism, no false positives on code-fence examples, test case (c) case-(ii) detection
- [ ] `/impl-hygiene-review` — verify classifier logic is correct, no false negatives on known cases, no re-parsing of frontmatter (LEAK guard), `plan_corpus` stays pure (no git calls inside `dag.py`)
- [ ] `00-overview.md` Quick Reference table updated: §02 shows the six subsections (02.0, 02.1, 02.2, 02.3, 02.4, 02.5) and revised Est. Lines (~900)
- [ ] `00-overview.md` mission success criteria (lines 28, 29) reflect the expanded §02 node coverage and source-kind taxonomy
- [ ] `index.md` §02 keyword cluster updated: adds `source_kind`, `code_fence_exclusion`, `html_comment_grammar`, `yaml_frontmatter_comment`, `REDUNDANT_DEPENDENCY`, `ORPHANED_PLAN`, `dag.py` module, `NodeKind`, `Reference`, `Edge`, `Dag`, `DagReport`, `enrich_resolve_dep_finding`, `normalize_subsystem`
- [ ] Cross-links verified: §03 `depends_on: ["01", "02"]` is accurate; §04 `depends_on: ["01", "02", "03"]` is accurate; §05 validation cases align with §02.3 known-case mapping
- [ ] All cross-section drift NOTEs (listed in the "Scope NOTEs for /tpr-review triage" block above) are addressed or carried forward to /tpr-review
- [ ] `/improve-tooling` section-close sweep — verify per-subsection retrospectives ran; add cross-subsection findings
- [ ] `/sync-claude` section-close sweep — verify CLAUDE.md and rules reflect any new scripts or conventions (new module `scripts/plan_corpus/dag.py`, new helpers, two new FindingSubtypes)
