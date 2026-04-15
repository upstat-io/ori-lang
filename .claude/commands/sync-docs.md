---
name: sync-docs
description: Sync ALL project documentation via batch TPR verification — dual-source (Codex + Gemini) review of every doc surface against actual code and spec. Nightly-ready, fully automated, fact-bound.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, Skill, EnterWorktree, ExitWorktree
---

# Sync All Documentation (TPR-Verified)

Reconcile all non-spec documentation against the actual codebase and spec using **dual-source TPR verification per batch**. Each batch of related doc files is verified by launching `/tpr-review` with a custom objective — Codex and Gemini independently cross-check every claim against code/spec, then Claude fixes findings and loops until both reviewers report clean.

**This command runs fully automated.** No pauses for user confirmation. Execute each phase to completion. Commit and report at the end.

**This is the comprehensive nightly sync.** `/sync-claude` remains a separate lightweight skill for delta syncs at subsection close-outs.

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

**Every document must be as compact as possible without losing information.** These files are loaded into Claude's context window — bloat wastes tokens, slows every interaction, and risks truncation.

**Compaction techniques:**
- **Bullet points over prose** — "Arena: `ExprArena` + `ExprId`, not `Box<Expr>`" beats a paragraph
- **Tables over lists** — structured info goes in tables
- **Eliminate redundancy** — if two documents say the same thing, one should point to the other
- **No motivation in rules** — "why" belongs in design docs, not rules files
- **Condense, never delete** — compress into fewer words, don't remove information

## Scope

### IN SCOPE — Verify and update these documents

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
| Language docs | `docs/ori_lang/*.md` (excl. spec/) | Language-level docs |
| Command files | `.claude/commands/*.md` | Command definitions |
| Skill docs | `.claude/skills/*/SKILL.md` | Skill documentation |
| README files | `**/*.md` matching `*README*` (excl. build/, .claude/worktrees/) | Project and crate READMEs |
| Remaining `.md` | Any `.md` not in the above rows or off-limits | Error code docs, skill internals, etc. |

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

**When you find a spec/grammar issue:** File it via `/add-bug` with subsystem `docs` and severity based on impact. Then continue — do NOT stop.

## Phase 0: Worktree Isolation (MANDATORY)

**All sync work happens in an isolated git worktree.** This keeps the `dev` branch completely clean — if something goes wrong, the worktree can be discarded with zero impact.

1. **Enter worktree** via `EnterWorktree` tool with name `sync-docs-YYYY-MM-DD` (use today's date)
2. This creates a new branch based on HEAD inside `.claude/worktrees/`
3. All subsequent phases run inside the worktree
4. At the end (Phase 3), the worktree is kept for user review — the user decides when to merge

**Do NOT skip this step.** Proceeding without worktree isolation is banned — the sync touches too many files to risk polluting the working branch.

## Phase 0.5: Discovery & Triage

1. Glob `**/*.md` from project root
2. Filter out off-limits paths (spec, proposals, archived-design, plans, build, worktrees)
3. Group remaining files into the batches defined in Phase 1
4. Check `git diff --name-only HEAD~30 HEAD -- compiler/ library/ scripts/ tests/` to identify high-drift areas

## Phase 1: Batch TPR Verification

For each batch below, launch `/tpr-review` (Skill tool) with the custom objective specified. The TPR loop runs until BOTH reviewers (Codex + Gemini) report zero actionable findings. After the TPR loop converges for a batch, fix all findings, commit, then proceed to the next batch.

### Batch Definitions

Each batch groups semantically related files. The custom objective tells both reviewers exactly what to verify.

---

#### Batch 1: Canon (`canon.md`) — Pipeline SSOT

**Files:** `.claude/rules/canon.md`

**Custom objective for `/tpr-review`:**
```
Verify every claim in .claude/rules/canon.md against the actual compiler source code. Specifically:

1. Pipeline table (§1): verify each crate name exists in compiler/*/Cargo.toml, verify input/output types match actual src/lib.rs exports, verify authoritative home files exist
2. Canonical desugars (§2): verify each desugar against the actual parser (compiler/ori_parse/src/) and type checker (compiler/ori_types/src/) code — confirm the desugar still exists and the description matches
3. Pattern compilation (§3): verify against compiler/ori_canon/src/patterns/ — confirm algorithm description, input/output, and consumer list
4. Per-phase output invariants (§4): verify each invariant is still enforced in code (grep for debug_assert!, validation functions)
5. Phase purity rules (§5): verify no phase-bleeding exists in the dependency graph
6. SSOTs table (§6): verify every canonical home file path actually exists and the description matches
7. Non-negotiable invariants (§7): verify against current .claude/rules/ files

Flag: stale crate names, wrong input/output types, missing phases, incorrect desugar descriptions, stale file paths in SSOTs table, invariants described but not enforced.
```

---

#### Batch 2: Frontend Rules (parser, type checker, type system, canonicalization)

**Files:** `.claude/rules/parse.md`, `.claude/rules/typeck.md`, `.claude/rules/types.md`, `.claude/rules/canonicalization.md`

**Custom objective for `/tpr-review`:**
```
Verify these 4 rules files against their corresponding source code and the spec:
- parse.md ↔ compiler/ori_parse/src/ + compiler/ori_lexer/src/ + spec §6-§7
- typeck.md ↔ compiler/ori_types/src/ + spec §8-§10
- types.md ↔ compiler/ori_types/src/ + spec §8-§9
- canonicalization.md ↔ compiler/ori_canon/src/

For each claim in each rules file: (1) Is it still true? Verify against code. (2) Is it spec-accurate? Verify against docs/ori_lang/v2026/spec/. (3) Is anything missing? Check for new code not covered. (4) Is anything stale? Check for removed/renamed code.

Flag: stale rule anchors (e.g., §XX-N referencing removed code), incorrect type/function names, missing rules for new features, descriptions that don't match current implementation.
```

---

#### Batch 3: Backend Rules (AIMS, ARC, codegen, LLVM, repr, AOT, runtime)

**Files:** `.claude/rules/aims-rules.md`, `.claude/rules/arc.md`, `.claude/rules/codegen-rules.md`, `.claude/rules/llvm.md`, `.claude/rules/repr.md`, `.claude/rules/aot.md`, `.claude/rules/runtime.md`

**Custom objective for `/tpr-review`:**
```
Verify these 7 backend rules files against their corresponding source code:
- aims-rules.md + arc.md ↔ compiler/ori_arc/src/ + spec §21
- codegen-rules.md + llvm.md ↔ compiler/ori_llvm/src/
- repr.md ↔ compiler/ori_repr/src/
- aot.md ↔ compiler/ori_llvm/src/aot/
- runtime.md ↔ compiler/ori_rt/src/ + spec §21

For each claim: verify against actual source code. Check for stale descriptions of pipeline steps, wrong function/type names, missing lattice dimensions or realization steps, incorrect codegen rules, stale ABI descriptions.

Flag: stale AIMS lattice descriptions, wrong pipeline step ordering, incorrect runtime function signatures, missing codegen rules for new features, stale representation layout descriptions.
```

---

#### Batch 4: Infrastructure Rules (IR, diagnostics, compiler architecture, hygiene, cargo, tests, formatter, registry)

**Files:** `.claude/rules/ir.md`, `.claude/rules/diagnostic.md`, `.claude/rules/compiler.md`, `.claude/rules/impl-hygiene.md`, `.claude/rules/cargo.md`, `.claude/rules/tests.md`, `.claude/rules/fmt.md`, `.claude/rules/registry.md`

**Custom objective for `/tpr-review`:**
```
Verify these 8 infrastructure rules files against their corresponding source code:
- ir.md ↔ compiler/ori_ir/src/
- diagnostic.md ↔ compiler/ori_diagnostic/src/
- compiler.md ↔ compiler/ (architecture, crate dependencies)
- impl-hygiene.md ↔ cross-cutting (process rules — verify referenced functions/types still exist)
- cargo.md ↔ Cargo.toml files across all crates
- tests.md ↔ tests/ + test infrastructure
- fmt.md ↔ compiler/ori_fmt/src/ + spec Annex D
- registry.md ↔ compiler/ori_registry/src/ + spec §8-§9, Annex C

For each claim: verify against actual source code. Check for stale function/type references, incorrect crate dependency descriptions, removed test patterns, stale formatter rules.

Flag: stale DerivedTrait variants, incorrect crate dependency graph, missing test patterns, stale registry type descriptions, incorrect cross-phase invariant contracts in impl-hygiene.md.
```

---

#### Batch 5: Meta Rules + Intelligence

**Files:** `.claude/rules/spec.md`, `.claude/rules/proposals.md`, `.claude/rules/roadmap.md`, `.claude/rules/intelligence.md`, `.claude/rules/ori-lang.md`

**Custom objective for `/tpr-review`:**
```
Verify these 5 meta/process rules files against their corresponding artifacts:
- spec.md ↔ docs/ori_lang/v2026/spec/ (verify spec process descriptions match actual spec structure)
- proposals.md ↔ docs/ori_lang/proposals/ (verify proposal process descriptions)
- roadmap.md ↔ plans/roadmap/ (verify roadmap process descriptions)
- intelligence.md ↔ scripts/intel-query.sh + .claude/skills/dual-tpr/compose-intel-summary.md (verify query subcommands, consumer lists, subsystem mapping)
- ori-lang.md ↔ docs/ori_lang/ (verify doc standards)

For each claim: verify against actual files. Check for stale consumer lists in intelligence.md, incorrect spec clause numbering, stale proposal process descriptions.

Flag: stale intel-query subcommand documentation, wrong consumer counts, incorrect file paths, stale process descriptions.
```

---

#### Batch 6: Ori Syntax Reference

**Files:** `.claude/rules/ori-syntax.md`

**Custom objective for `/tpr-review`:**
```
Verify every claim in .claude/rules/ori-syntax.md against THREE sources:
1. The spec: docs/ori_lang/v2026/spec/ (clauses §7-§27, grammar.ebnf, operator-rules.md)
2. The prelude: library/std/prelude.ori
3. The stdlib: library/std/ (testing.ori, collections/, etc.)

This is a DENSE reference file. Check:
- Every type listed exists in the spec and prelude
- Every trait listed has the correct methods and signatures
- Every built-in function has the correct signature
- Every operator has the correct precedence and associativity
- Every keyword is correctly classified (reserved vs context-sensitive)
- Every collection method listed actually exists in the stdlib
- String/char/byte methods match actual implementations

Flag: missing types/traits/functions, wrong signatures, incorrect precedence, stale method lists, methods listed that don't exist, methods that exist but aren't listed.
```

---

#### Batch 7: CLAUDE.md

**Files:** `CLAUDE.md`

**Custom objective for `/tpr-review`:**
```
Verify every claim in the project CLAUDE.md against the actual codebase:

1. Commands section: verify every command/script still exists (check file paths), verify aliases are valid
2. Key Paths section: verify every path exists and description is accurate
3. Feature Flags table: verify against actual Cargo.toml feature definitions (grep for [features] in compiler/*/Cargo.toml)
4. Environment variables: verify each ORI_* variable is actually checked in the codebase (grep for each one)
5. Compiler Coding Guidelines: verify against .claude/rules/impl-hygiene.md and compiler.md
6. AIMS section: verify lattice dimension count and descriptions against .claude/rules/aims-rules.md and compiler/ori_arc/src/
7. Reference Repos: verify each path in ~/projects/reference_repos/lang_repos/ exists
8. CLI section: verify commands match actual ori binary capabilities
9. Versioning: verify BUILD_NUMBER exists, versioning scripts exist

Flag: missing/renamed scripts, stale paths, wrong feature flags, env vars not actually checked in code, incorrect lattice dimension counts, stale reference repo paths.
```

---

#### Batch 8: Design Docs

**Files:** `docs/compiler/design/**/*.md`

**Custom objective for `/tpr-review`:**
```
Verify the compiler design docs against the actual implementation. For each design doc:

1. Read the design doc and identify its claims about code structure, algorithms, data types
2. Verify those claims against the actual source files in compiler/
3. Check that referenced types, functions, and modules still exist and have the described behavior

Priority areas (most likely to drift):
- docs/compiler/design/01-architecture/ ↔ compiler crate structure
- docs/compiler/design/05-type-system/ ↔ compiler/ori_types/src/
- docs/compiler/design/09-aims/ ↔ compiler/ori_arc/src/
- docs/compiler/design/10-llvm-backend/ ↔ compiler/ori_llvm/src/
- docs/compiler/design/11-runtime/ ↔ compiler/ori_rt/src/

Flag: stale architecture descriptions, wrong type names, removed algorithms still described, missing new subsystems not documented.
```

---

#### Batch 9: Guide Docs + Other Docs

**Files:** `docs/guide/**/*.md`, `docs/tooling/**/*.md`, `docs/development/*.md`, `docs/ori_lang/v2026/modules/**/*.md`, `docs/ori_lang/*.md` (non-spec), all README files (excl. build/, worktrees/)

**Custom objective for `/tpr-review`:**
```
Verify guide docs, tooling docs, development docs, module docs, language docs, and README files against the spec and codebase:

1. Guide docs (docs/guide/): verify code examples compile and match current Ori syntax per spec, verify feature descriptions are accurate
2. Tooling docs (docs/tooling/): verify formatter and LSP descriptions match current implementations
3. Development docs (docs/development/): verify developer instructions are current
4. Module docs (docs/ori_lang/v2026/modules/): verify stdlib module descriptions match library/std/
5. Language docs (docs/ori_lang/*.md non-spec): verify language descriptions match spec
6. READMEs: verify project descriptions, build instructions, and getting-started guides are current

Flag: code examples with syntax that doesn't match current spec, stale feature descriptions, incorrect build instructions, wrong stdlib API descriptions.
```

---

### Batch Execution Protocol

For each batch (1 through 9), in order:

1. **Read all files in the batch** to understand current state
2. **Launch `/tpr-review`** (via Skill tool) with the batch's custom objective as ARGS
3. **The TPR loop handles everything**: reviewer launch, finding merge, thoroughness judgment, fix-and-rerun until clean
4. **After TPR converges (zero findings from both reviewers)**, record which files were modified and what was verified
5. **Commit batch changes** via `/commit-push` before moving to the next batch
6. **Proceed to next batch**

**Batches are sequential, not parallel.** Each batch's fixes may affect downstream batches (e.g., canon.md fixes inform rules file verification). Earlier batches cover higher-priority surfaces.

**If a batch's TPR finds spec/grammar issues:** File via `/add-bug` with subsystem `docs` and continue. Do NOT modify spec/grammar files.

## Phase 2: Spec/Grammar Issue Collection

Throughout all batches, when TPR reviewers or Claude's own fixes encounter a discrepancy where the SPEC appears wrong:

1. **DO NOT modify spec or grammar files**
2. **File via `/add-bug`** with subsystem `docs`, severity based on impact, the specific spec clause, and what's wrong

## Phase 3: Final Report & Worktree Handoff

After all batches converge:

1. Verify all changes are committed (each batch commits via `/commit-push`)
2. **Do NOT push from the worktree** — the worktree branch is local
3. **Exit the worktree** via `ExitWorktree` with `action: "keep"` — preserves the branch and all commits
4. Report the worktree branch name so the user can review and merge

**Report:**
1. **Worktree branch**: the branch name created in Phase 0 (user merges when ready)
2. **Batches completed**: list each batch with TPR iteration count and files modified
3. **Verification sources**: for each modified file, which source files/spec clauses the TPR reviewers verified against
4. **Bugs filed**: any spec/grammar issues filed via `/add-bug`
5. **Total TPR rounds**: sum across all batches

**Merge instruction for user:** After reviewing the worktree's commits, merge with:
```bash
git merge <worktree-branch-name>
# or cherry-pick specific commits
```

## Writing Style Matrix

| Surface | Tense | Voice | Style |
|---------|-------|-------|-------|
| Canon (`canon.md`) | Present | Technical, declarative | Formal pipeline description |
| Rules files | Present | Concise, imperative | Bullet points, tables |
| Ori syntax (`ori-syntax.md`) | Present | Reference | Dense, terse, comprehensive |
| CLAUDE.md | Present | Instructional | Direct commands |
| Design docs | Present | Explanatory | WHY and HOW, trade-offs |
| Guide docs | Present | Tutorial | "You can...", examples |
| Other docs | Present | Mixed | Per-surface appropriate |

**Common to ALL surfaces:** present tense, no "was changed to", no test counts, no progress updates, write as if for someone who has never seen previous versions.

## Automation Protocol

This command is designed to run nightly without human intervention:

1. **Worktree isolation first** — `EnterWorktree` before any work; `ExitWorktree(keep)` at the end
2. **No `AskUserQuestion` calls** — make judgment calls and proceed
3. **No pauses between batches** — execute sequentially to completion
4. **File bugs, don't block** — when you find spec issues, file them and continue
5. **Each batch commits separately** — via `/commit-push` after TPR convergence (commits stay on worktree branch)
6. **No push from worktree** — user merges the worktree branch when ready
7. **Report at the end** — summary of all batches + worktree branch name

## User Input

$ARGUMENTS
