---
name: sync-docs
description: Sync ALL project documentation — CLAUDE.md, rules, design docs, README, canon, guide — against actual code and spec. Nightly-ready, fully automated, fact-bound.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, Skill
---

# Sync All Documentation

Reconcile EVERY document in the repository against the actual codebase and spec. This is a comprehensive nightly sync — it updates canon, all `.claude/rules/*.md` files, `CLAUDE.md`, design docs, guide docs, README files, and all other non-spec documentation to accurately reflect the current implementation.

**This command runs fully automated.** No pauses for user confirmation. Execute each phase to completion. Commit and report at the end.

**This is the comprehensive nightly sync.** `/sync-claude` remains a separate lightweight skill for delta syncs at subsection close-outs — it covers CLAUDE.md + rules affected by a specific code change. This command does the full audit.

## The One Rule: FACT-BOUND

**Every single change MUST be bound to a verifiable fact.** Before writing ANY documentation claim:

1. **Verify it** — read the actual source code, spec clause, or test file
2. **Cite it** — record which file/line/clause proves the claim
3. **Write only what you verified** — if you can't verify it, don't write it

**BANNED:**
- Exaggerating capabilities ("handles all edge cases" without proof)
- Hallucinating features (describing behavior that doesn't exist in code)
- Aspirational language ("will support", "planned to") — rules describe what IS, not what will be
- Inferring from enum definitions without grepping construction sites
- Copying claims between documents without re-verifying against code

**Rules files are FACTS about ideal behavior** — they describe how the system is SUPPOSED to work according to the spec. They are not plans, not bug reports, not wishlists. When implementation doesn't match spec, that's a bug to file via `/add-bug`, not something to paper over in the rules.

## The Second Rule: COMPACT WITHOUT INFORMATION LOSS

**Every document must be as compact as possible without losing information.** These files are loaded into Claude's context window — bloat wastes tokens, slows every interaction, and risks truncation. Compactness is not optional; it is a quality dimension alongside accuracy.

**Compaction techniques:**
- **Bullet points over prose** — "Arena: `ExprArena` + `ExprId`, not `Box<Expr>`" beats a paragraph
- **Tables over lists** — structured info (file→purpose, type→trait, etc.) goes in tables
- **Eliminate redundancy** — if two documents say the same thing, one should point to the other
- **No motivation in rules** — "why" belongs in design docs, not rules files. Rules say WHAT and HOW.
- **Remove "this means that"** — if the bullet already says X, don't follow with "in other words, X"
- **Size awareness** — rules files range from 16 lines (roadmap.md) to 1125 lines (typeck.md). Complex subsystems (type checker, AIMS, impl-hygiene) legitimately need hundreds of lines — never truncate these to hit an arbitrary target. But all files should be audited: every line must earn its place. If a section can be a table, make it a table. If a paragraph can be a bullet, make it a bullet. CLAUDE.md: every section must justify its byte count.
- **Condense, never delete** — when trimming, compress the information into fewer words, don't remove it. The goal is density, not omission.

**Test**: for every paragraph in a rules file, ask "could this be a bullet point?" If yes, make it one.

### Verification Ledger (MANDATORY)

Every documentation edit MUST be tracked in a verification ledger. At the end of the run, the commit message body includes a section listing each modified file with its verification sources:

```
Verification ledger:
  .claude/rules/parse.md — verified against compiler/ori_parse/src/lib.rs:1-50, spec §7
  .claude/rules/typeck.md — verified against compiler/ori_types/src/infer/, spec §8-§9
  docs/compiler/design/03-lexer/index.md — verified against compiler/ori_lexer/src/lib.rs
  ...
```

This makes fact-binding auditable after the fact. A reviewer can check whether the cited sources actually support the documentation claims.

## Scope

### IN SCOPE — Update these documents

| Surface | Path(s) | What it describes |
|---------|---------|-------------------|
| Canon (pipeline SSOT) | `.claude/rules/canon.md` | Pipeline phases, desugars, invariants |
| Rules files | `.claude/rules/*.md` (excl. canon.md, ori-syntax.md) | Per-subsystem fact-based rules |
| Ori syntax ref | `.claude/rules/ori-syntax.md` | Language quick reference |
| CLAUDE.md | `CLAUDE.md` | Project-level instructions, commands, key paths |
| Compiler design docs | `docs/compiler/design/**/*.md` | Compiler internals design |
| Guide docs | `docs/guide/**/*.md` | Language guide / tutorial |
| Formatter docs | `docs/tooling/formatter/**/*.md` | Formatter design + user docs |
| LSP docs | `docs/tooling/lsp/design/**/*.md` | LSP design |
| Development docs | `docs/development/*.md` | Developer guidelines, versioning |
| Module docs | `docs/ori_lang/v2026/modules/**/*.md` | Stdlib module docs |
| Language docs | `docs/ori_lang/*.md` (excl. spec/) | Language-level docs (README, versioning) |
| Command files | `.claude/commands/*.md` | Sync command and other command definitions |
| Skill docs | `.claude/skills/*/SKILL.md` | Skill documentation |
| README files | `**/*.md` matching `*README*` (excl. build/, .claude/worktrees/) | Project and crate READMEs |

### OFF LIMITS — Do NOT modify these (file bugs instead)

| Surface | Path(s) | Why |
|---------|---------|-----|
| Spec | `docs/ori_lang/v2026/spec/**` | Proposal gate required |
| Grammar | `docs/ori_lang/v2026/spec/grammar.ebnf` | Proposal gate required |
| Operator rules | `docs/ori_lang/v2026/spec/operator-rules.md` | Proposal gate required |
| Proposals | `docs/ori_lang/proposals/**/*.md` | Immutable records |
| Archived design | `docs/ori_lang/v2026/archived-design/**` | Historical record |
| Plans | `plans/**/*.md` | Active work artifacts |
| Build artifacts | `build/**` | Generated content |
| Worktrees | `.claude/worktrees/**` | Transient agent copies |

**When you find a spec/grammar issue:** File it via `/add-bug` (Skill tool) with subsystem `docs` and severity based on impact. Include the specific spec clause and what's wrong. Then continue syncing — do NOT stop.

## Phase 0: Discovery

Scan all document surfaces to build the work list. Use a repo-wide glob filtered by off-limits paths:

```
Glob: **/*.md (path: project root)
```

Then filter out off-limits paths (spec, proposals, archived-design, plans, build, worktrees). Group remaining files by surface type for phased processing.

## Phase 1: Canon (`canon.md`) — Pipeline SSOT

**Canon syncs FIRST because it is the Single Source of Truth for the pipeline.** All other documents (rules files, CLAUDE.md, design docs) derive from canon's phase map, desugar list, and invariant catalog. Syncing canon first ensures downstream phases operate against an accurate pipeline description.

### Verify each canon section:

1. **Pipeline table (§1)**: verify crate names, input/output types, authoritative homes against actual `compiler/*/Cargo.toml` and `src/lib.rs` files
2. **Canonical desugars (§2)**: verify each desugar against the actual parser/typeck code
3. **Pattern compilation (§3)**: verify against `compiler/ori_canon/src/patterns/`
4. **Per-phase output invariants (§4)**: verify each invariant is still enforced in code
5. **Phase purity rules (§5)**: verify no phase-bleeding exists
6. **SSOTs table (§6)**: verify each canonical home file path is accurate
7. **Non-negotiable invariants (§7)**: verify against current rules files

## Phase 2: Rules Files (`.claude/rules/*.md`, excluding canon.md and ori-syntax.md)

Rules files are loaded into every Claude conversation. Stale rules cause wrong behavior across ALL interactions.

### For each rules file:

1. **Read the rules file** — note its `paths:` pattern and subject area
2. **Read the corresponding source code** — the actual implementation files the rules describe
3. **Read the spec** — spec clauses relevant to this subsystem (use mapping table below)
4. **Compare** — for each claim in the rules file:
   - Is it still true? (verify against code)
   - Is it spec-accurate? (verify against spec)
   - Is anything missing? (new code not covered by rules)
   - Is anything stale? (rules describe removed/changed code)
5. **Update** — fix discrepancies. Record each edit in the verification ledger.

### Rules file principles:

- **Spec-driven**: rules describe how the system SHOULD work per the spec. Implementation is evidence for what currently exists, but spec is authoritative for what's correct. When implementation diverges from spec, file a bug via `/add-bug` — don't adjust the rules file to match the broken implementation.
- **Concise**: bullet format, tables for structured info. Every line must earn its place.
- **Present tense**: "The lexer produces X" not "The lexer was changed to produce X"
- **No plans**: rules are not roadmaps. Unimplemented features don't belong in rules.
- **No volatile metrics**: no test counts, no coverage percentages

### Mapping: rules file → source code → spec clauses

| Rules file | Source code | Spec clauses |
|------------|------------|--------------|
| `parse.md` | `compiler/ori_parse/src/` | §6 Source code, §7 Lexical elements |
| `typeck.md` | `compiler/ori_types/src/` | §8 Types, §9 Properties of types, §10 Declarations |
| `types.md` | `compiler/ori_types/src/` | §8 Types, §9 Properties of types |
| `eval.md` | `compiler/ori_eval/src/` | §14 Expressions, §23 Program execution |
| `patterns.md` | `compiler/ori_patterns/src/` | §15 Patterns |
| `aims-rules.md` | `compiler/ori_arc/src/` | §21 Memory model |
| `arc.md` | `compiler/ori_arc/src/` | §21 Memory model |
| `codegen-rules.md` | `compiler/ori_llvm/src/` | N/A (implementation) |
| `llvm.md` | `compiler/ori_llvm/src/` | N/A (implementation) |
| `runtime.md` | `compiler/ori_rt/src/` | §21 Memory model |
| `ir.md` | `compiler/ori_ir/src/` | N/A (implementation) |
| `diagnostic.md` | `compiler/ori_diagnostic/src/` | N/A (implementation) |
| `fmt.md` | `compiler/ori_fmt/src/` | Annex D Formatting |
| `repr.md` | `compiler/ori_repr/src/` | N/A (implementation) |
| `aot.md` | `compiler/ori_llvm/src/aot/` | §23 Program execution |
| `registry.md` | `compiler/ori_registry/src/` | §8 Types, §9 Properties, Annex C Built-ins |
| `tests.md` | `tests/`, test infrastructure | §19 Testing |
| `impl-hygiene.md` | Cross-cutting | N/A (process) |
| `compiler.md` | `compiler/` | N/A (architecture) |
| `canonicalization.md` | `compiler/ori_canon/src/` | N/A (implementation) |
| `cargo.md` | `Cargo.toml` files | N/A (build) |
| `spec.md` | `docs/ori_lang/v2026/spec/` | Meta (spec process) |
| `proposals.md` | `docs/ori_lang/proposals/` | Meta (proposal process) |
| `roadmap.md` | `plans/roadmap/` | Meta (planning process) |
| `intelligence.md` | `scripts/intel-query.sh` | N/A (tooling) |
| `ori-lang.md` | Language docs | N/A (doc standards) |

### Parallelization strategy:

Use `Agent` subagents (type: `Explore`) for independent information gathering:
- Batch 3-5 agents at a time
- Each agent reads one rules file + its corresponding source code
- Each agent reports: accurate claims, stale claims, missing info, incorrect info
- Main thread applies the fixes based on agent reports

## Phase 3: Ori Syntax Reference (`ori-syntax.md`)

Synced separately because it has a unique verification surface — it must match BOTH the spec AND the prelude/stdlib implementation.

1. **Read `ori-syntax.md`**
2. **Verify against spec**: `docs/ori_lang/v2026/spec/` — clauses §7-§27
3. **Verify against prelude**: `library/std/prelude.ori`
4. **Verify against stdlib**: `library/std/` (testing.ori, collections/, etc.)
5. **Update** discrepancies

## Phase 4: CLAUDE.md

CLAUDE.md is the project's top-level instruction set. Sync it against the current state:

1. **Commands section**: verify every command still works (check scripts exist, aliases valid)
2. **Key Paths section**: verify every path exists and description is accurate
3. **Feature Flags table**: verify against actual `Cargo.toml` feature definitions
4. **Compiler Coding Guidelines**: verify against `.claude/rules/impl-hygiene.md` and `compiler.md` (CLAUDE.md summarizes, rules files are authoritative)
5. **Ori Language section**: verify against spec and current implementation
6. **AIMS section**: verify against `.claude/rules/aims-rules.md` and `arc.md`
7. **Environment variables**: verify each `ORI_*` var exists in the codebase
8. **Reference Repos**: verify paths exist

## Phase 5: Design Docs (`docs/compiler/design/**/*.md`)

For each design doc:

1. **Read the design doc**
2. **Read the corresponding source code**
3. **Compare claims against implementation**
4. **Fix discrepancies**

## Phase 6: Guide Docs (`docs/guide/**/*.md`)

For each guide doc:

1. **Read the guide doc**
2. **Verify code examples** against actual compiler behavior
3. **Verify feature descriptions** against spec
4. **Fix discrepancies**

## Phase 7: Other Documentation

- `docs/tooling/formatter/**/*.md` — formatter design + user docs
- `docs/tooling/lsp/design/**/*.md` — LSP design docs
- `docs/development/*.md` — developer guidelines, versioning
- `docs/ori_lang/v2026/modules/**/*.md` — stdlib module docs
- `docs/ori_lang/*.md` (non-spec) — language-level docs
- All README files (`**/*README*`, excluding build/, worktrees/)

## Phase 8: Spec/Grammar Issue Collection

Throughout all phases, when you encounter a discrepancy where the SPEC appears wrong (not the implementation):

1. **DO NOT modify spec or grammar files**
2. **File via `/add-bug`** (Skill tool) with:
   - Subsystem: `docs` (section-08 in bug tracker)
   - Severity: based on impact
   - Description: which spec clause, what's wrong, what the correct behavior should be per implementation
   - Note: "Spec issue — requires proposal workflow to fix"

## Phase 9: Commit

After all phases complete, commit all documentation changes. Stage only the files this skill modified — do NOT use `git add -A` or `git add .`.

```bash
# Stage only the specific files that were modified
git add CLAUDE.md .claude/rules/*.md docs/ README.md compiler/*/README.md ...
# (list the actual modified files)

git commit -m "$(cat <<'EOF'
docs: nightly sync — reconcile all documentation against codebase

Synced surfaces: canon, rules, ori-syntax, CLAUDE.md, design docs,
guide docs, formatter docs, LSP docs, READMEs, module docs

Verification ledger:
  <file> — verified against <source>
  ...
EOF
)"

git push
```

## Writing Style Matrix

Each document surface has its own voice. Apply the correct style:

| Surface | Tense | Voice | Style |
|---------|-------|-------|-------|
| Canon (`canon.md`) | Present | Technical, declarative | Formal pipeline description. Cite phase rules. |
| Rules files (`.claude/rules/*.md`) | Present | Concise, imperative | Bullet points, tables. Quick reference, not tutorial. |
| Ori syntax (`ori-syntax.md`) | Present | Reference | Dense, terse, comprehensive. Spec-aligned. |
| CLAUDE.md | Present | Instructional | Direct commands. "Do X", "Never Y". |
| Design docs (`docs/compiler/design/`) | Present | Explanatory | Explain WHY and HOW. Design decisions, trade-offs. |
| Guide docs (`docs/guide/`) | Present | Tutorial | Teach. "You can...", examples, progressive complexity. |
| Formatter/LSP docs | Present | Mixed reference/tutorial | User-facing: tutorial. Design: explanatory. |
| Development docs | Present | Practical | Developer-facing instructions. |
| Module docs | Present | Reference | API reference style. |
| READMEs | Present | Introductory | What, why, how to start. Marketing-adjacent for root. |

**Common to ALL surfaces:**
- Present tense, factual descriptions
- No "was changed to", "previously", "now"
- No test counts or volatile metrics
- No progress updates or completion notes
- Write as if for someone who has never seen previous versions

## Automation Protocol

This command is designed to run nightly without human intervention:

1. **No `AskUserQuestion` calls** — make judgment calls and proceed
2. **No pauses between phases** — execute sequentially to completion
3. **File bugs, don't block** — when you find spec issues, file them and continue
4. **Stage only modified doc files** — never `git add -A`
5. **Include verification ledger** — in commit message body
6. **Report at the end** — summary of what was synced

## Output

After completion, report:

1. **Files synced**: list each modified file with a one-line summary of changes
2. **Verification ledger**: for each modified file, which source files/spec clauses verified it
3. **Bugs filed**: list any spec/grammar issues filed via `/add-bug`
4. **Discrepancies found**: summary of code-vs-doc mismatches fixed

## User Input

$ARGUMENTS
