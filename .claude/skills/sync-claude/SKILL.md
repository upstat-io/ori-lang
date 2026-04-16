---
name: sync-claude
description: "Check whether code changes affect CLAUDE.md, .claude/rules/*.md, canon.md, or other Claude artifacts, and update them. TRIGGER at every subsection close-out (after /improve-tooling), section completion (after /improve-tooling sweep), and after any cross-cutting change (new phase, new crate, new enum variant, new command, renamed API). AUTO-TRIGGER when: (1) a new crate, module, or phase is added, (2) an enum variant, trait, or public API is added/renamed/removed, (3) a command, alias, or env var is added/changed, (4) a pipeline phase boundary changes, (5) a desugar or canonicalization step is added/modified, (6) the spec is updated, (7) any .claude/rules/*.md file's domain is touched by code changes."
---

# Sync Claude Artifacts

**PURPOSE:** After code changes, verify that all Claude artifacts (CLAUDE.md, .claude/rules/*.md, canon.md, ori-syntax.md) still accurately describe the codebase. Code changes silently invalidate docs — this skill closes that loop.

**SCOPE:** This skill syncs Claude-loaded artifacts only — files that are injected into conversation context and drive Claude's understanding of the codebase. It does NOT cover design docs (`docs/compiler/design/`), which are handled by `/sync-docs`. It does NOT cover spec files (`docs/ori_lang/v2026/spec/`), which are handled by `/sync-spec`.

**ABSOLUTE RULE: If you changed code, check the docs. If the docs are wrong, fix them NOW.**

## What Gets Checked

| Artifact | Path | What to verify |
|----------|------|----------------|
| **CLAUDE.md** | `CLAUDE.md` | Commands, env vars, key paths, feature flags, CLI usage, versioning, entry points |
| **Rules files** | `.claude/rules/*.md` | Phase rules, type system rules, codegen rules, patterns, registry, etc. |
| **Canon** | `.claude/rules/canon.md` | Pipeline overview, phase table, desugars, output invariants, SSOTs, cross-refs |
| **Ori syntax** | `.claude/rules/ori-syntax.md` | Language syntax quick reference — types, operators, keywords, prelude |
| **Skill files** | `.claude/skills/*/SKILL.md` | Skill trigger conditions, workflows, referenced paths/commands |
| **Memory index** | User's MEMORY.md | Stale project memories (architecture decisions, patterns that changed) |

## Trigger Conditions

This skill fires at two mandatory workflow points and auto-triggers on specific code patterns:

### Mandatory Workflow Points

1. **Per-subsection close-out** — after `/improve-tooling` retrospective completes, before the `---` separator. Ask: "Did the code I wrote in this subsection invalidate any claims in CLAUDE.md or rules files?"
2. **Section completion checklist** — after `/improve-tooling` section-close sweep, before repo hygiene check. Broader scope: look across all subsections for cumulative doc drift.

### Auto-Trigger Patterns

Fire this skill proactively when ANY of these occur during normal work:

| Change type | Affected artifacts |
|-------------|-------------------|
| New crate or module added | `CLAUDE.md` §Key Paths, `canon.md` §1 Pipeline, relevant rules file |
| Pipeline phase added/reordered | `canon.md` §1 + §4 + §5, `compiler.md` |
| New enum variant (DerivedTrait, CanExpr, ArcInstr, etc.) | `ir.md`, `arc.md`, `canon.md` §4, CLAUDE.md §Adding a Derived Trait |
| Public API added/renamed/removed | Relevant rules file (typeck.md, types.md, eval.md, etc.) |
| New command, script, or env var | `CLAUDE.md` §Commands, `diagnostic.md` if in diagnostics/ |
| New desugar or canonicalization step | `canon.md` §2 or §4.3, `typeck.md` or `parse.md` |
| Operator semantics changed | `canon.md` §2, `ori-syntax.md` §Operators, spec `operator-rules.md` |
| New prelude type/trait/function | `ori-syntax.md` §Prelude, `CLAUDE.md` if it's a built-in |
| New feature flag | `CLAUDE.md` §Feature Flags |
| Registry change (new TypeDef, method) | `registry.md` |
| Spec clause added/modified | `ori-syntax.md`, `canon.md` §6 SSOTs |
| New error code range | `diagnostic.md` |
| ARC/AIMS lattice dimension change | `arc.md`, `aims-rules.md`, `canon.md` §4.4–§4.5, CLAUDE.md §AIMS |
| Runtime FFI function added/changed | `runtime.md` |
| Formatter rule changed | `fmt.md` |
| Test pattern/convention changed | `tests.md` |
| New keyword (reserved or context-sensitive) | `ori-syntax.md` §Keywords |

## Workflow

### Step 1: Identify What Changed

Determine the scope of code changes since the last sync point. Use:

```bash
# If mid-subsection: changes since last commit
git diff --name-only HEAD

# If at section close: all changes in the section
git diff --name-only <section-start-commit>..HEAD

# Quick: what crates were touched?
git diff --name-only HEAD | grep -oP 'compiler/\w+' | sort -u
```

### Step 1.5: Intelligence-Graph Symbol Inventory (MANDATORY when graph available)

After identifying changed crates in Step 1, query the intelligence graph to
get a complete symbol inventory for each changed crate. This reveals symbols
that may have been added, renamed, or removed — and that doc artifacts might
not reflect.

@.claude/skills/query-intel/compose-intel-summary.md

Specifically: for each crate touched in the diff, run
`scripts/intel-query.sh --human file-symbols "<crate-path-fragment>" --repo ori`
to get the current symbol surface. Compare against the rules file's documented
symbols (Step 2's mapping). Symbols present in the graph but absent from the
rules file are likely new additions that need documentation. Symbols in the
rules file but absent from the graph may have been removed or renamed.

This step does NOT check whether a file's references to the intelligence
graph are current — that is a different concern. This step checks whether
the CODE SYMBOLS documented in rules files match the actual codebase.

### Step 2: Map Changes to Artifacts

For each changed file/crate, consult the trigger table above. Build a checklist of artifacts to verify.

**Mapping heuristic by crate:**

| Crate touched | Primary rules file | Also check |
|---------------|-------------------|------------|
| `ori_lexer` | `parse.md` | `canon.md` §4.0 |
| `ori_parse` | `parse.md` | `canon.md` §2, §4.1 |
| `ori_types` | `typeck.md`, `types.md` | `canon.md` §4.2, `registry.md` |
| `ori_canon` | `canonicalization.md` | `canon.md` §3, §4.3 |
| `ori_eval` | `eval.md` | `patterns.md` |
| `ori_arc` | `arc.md`, `aims-rules.md` | `canon.md` §4.4–§4.5, CLAUDE.md §AIMS |
| `ori_llvm` | `llvm.md`, `codegen-rules.md` | `canon.md` §4.6, `aot.md` |
| `ori_ir` | `ir.md` | CLAUDE.md §Adding a Derived Trait |
| `ori_diagnostic` | `diagnostic.md` | — |
| `ori_rt` | `runtime.md` | — |
| `ori_registry` | `registry.md` | — |
| `ori_fmt` | `fmt.md` | — |
| `oric` | `compiler.md` | CLAUDE.md §Commands |
| `library/std/` | `ori-syntax.md` §Prelude | — |
| `tests/` | `tests.md` | CLAUDE.md §Commands |
| `diagnostics/` | `diagnostic.md` | CLAUDE.md §Commands |
| `scripts/` | CLAUDE.md §Commands | — |

### Step 3: Verify Each Artifact

For each artifact identified in Step 2:

1. **Read the artifact** — focus on sections related to the changed code
2. **Compare against current code** — are file paths still valid? Are enum variants listed? Are commands accurate? Are invariants still true?
3. **Check for stale claims** — does the doc reference removed functions, old phase names, deprecated env vars?
4. **Check for missing entries** — did the code add something the doc doesn't mention yet?

### Step 4: Fix Drift

Update each artifact that has drifted. Follow these rules:

- **CLAUDE.md**: Keep entries concise. Match the existing style (tables, bullet lists, pipe-separated commands).
- **Rules files**: Concise bullets, tables for structured data. Every line must earn its place. Complex subsystems (typeck, AIMS) legitimately need hundreds of lines.
- **canon.md**: Maintain the existing section structure. Update tables and invariant lists precisely. Cross-reference §numbers must stay consistent.
- **ori-syntax.md**: Ultra-dense reference format. No prose — keywords, operators, types, syntax forms.

**ABSOLUTE: Rules files describe IDEAL SPEC-DRIVEN BEHAVIOR — not current code state.**

Rules files (`.claude/rules/*.md`) describe how the system SHOULD work — the ideal, spec-driven target behavior. They are NOT a mirror of current code. This distinction is load-bearing:

- **When code is fixed to match the spec**: REMOVE any "shipped divergence" annotation that described the old broken behavior. The spec rules already describe the correct behavior — once code matches, the annotation is stale. Do NOT replace a "code diverges" annotation with a "code now matches" annotation — that's documentation of a bug fix, which belongs in the bug tracker, not in the rules.
- **When code still diverges from spec**: the preamble "partially-shipped" annotations in files like `aims-rules.md` may note the divergence factually ("Shipped: X. Spec: Y."). These are status annotations, NOT rules. They exist so reviewers don't re-flag known implementation gaps as spec issues.
- **NEVER add bug IDs, fix dates, commit hashes, or tracking references to rules files.** Bug provenance belongs in `plans/bug-tracker/`, commit messages, and fix-section files — never in `.claude/rules/*.md`.
- **NEVER add "not yet implemented" or "tracked as BUG-XX-NNN" to rules files.** Unimplemented spec features are just the spec — the rules already describe the ideal. Implementation gaps are tracked in the bug tracker.
- **The test**: after your edit, read the rules file as if you've never seen the codebase. Does every statement describe how the system SHOULD work? Or does it describe what's broken, what was fixed, or what's planned? If the latter, it doesn't belong in a rules file.

**ABSOLUTE: Before removing ANY rule, anchor, or named element from a rules file, search for references.**

Rules files contain named anchors (DP-9, RL-13, CN-3, TR-2, etc.), section references (§4.5, §1.9), and named concepts (e.g., `is_cow_aware_unique`, `cross_dim_evidence_total`) that may be referenced from:
- **Code** — comments citing spec rules (`// Spec: DP-6 + RL-11`), `#[expect]` reason strings, doc comments
- **Other rules files** — cross-references between `aims-rules.md`, `arc.md`, `canon.md`, `codegen-rules.md`, etc.
- **Plan files** — `plans/` sections referencing spec rules in their analysis
- **Test files** — test doc comments citing the rule being tested
- **CLAUDE.md** — top-level rule summaries

Before removing or renaming any named element:
1. `grep -rn "ELEMENT_NAME" compiler/ .claude/rules/ plans/ CLAUDE.md tests/` (or use `rg`)
2. For each hit: is it a reference that would become dangling? If yes, update the reference first.
3. Only then remove the element from the rules file.

Removing a rule anchor without checking references creates invisible broken cross-refs that mislead future readers and reviewers. This applies even when removing a "shipped divergence" annotation — the annotation itself may be cross-referenced.

### Step 5: Verify Fixes

After updating, re-read each modified artifact to confirm:
- No broken table formatting
- No stale cross-references (§numbers, file paths)
- No contradictions with other artifacts
- The change is self-consistent
- No dangling references to removed elements (run the grep from the removal rule above)

## What This Skill Does NOT Do

- **Does not sync design docs** — use `/sync-docs` for `docs/compiler/design/`
- **Does not sync spec** — use `/sync-spec` for `docs/ori_lang/v2026/spec/`
- **Does not sync grammar** — use `/sync-grammar` for `grammar.ebnf`
- **Does not audit rules file size/format** — use `/sync-docs` nightly for comprehensive audits
- **Does not save memories** — if a project fact changed, update the memory file directly

## Retrospective Integration

### Per-Subsection Close-Out

At subsection close-out, after `/improve-tooling`, ask these three questions:

1. **Did I add/rename/remove any public API, type, variant, or function?** → Check the relevant rules file
2. **Did I add/change any command, env var, or script?** → Check CLAUDE.md §Commands
3. **Did I change any pipeline phase behavior or output invariant?** → Check `canon.md`

If all three are "no" and the subsection was purely internal implementation with no API surface changes, document briefly: "Claude artifact sync: no API/command/phase changes — artifacts current." Do not silently skip.

### Section Completion

At section close, after `/improve-tooling` sweep, do a broader pass:

1. Run Step 1–2 across ALL commits in the section
2. Verify each mapped artifact
3. Look for cumulative drift invisible at per-subsection scope (e.g., three subsections each added one prelude function, but `ori-syntax.md` §Prelude was only updated for the first)

## Anti-Patterns (BANNED)

- **"Docs are someone else's job"** — if you changed the code, the doc sync is YOUR work
- **"It's a minor change"** — minor code changes cause major doc drift when accumulated
- **"I'll update docs at the end"** — by section close, you've forgotten what you changed in subsection .1
- **"The rules file is already too long"** — that's a `/sync-docs` nightly concern; this skill's job is accuracy, not size
- **"CLAUDE.md doesn't need this level of detail"** — if it's in CLAUDE.md already, it must be accurate; if it shouldn't be there, remove it cleanly
- **Silently skipping the check** — every subsection close-out must either update artifacts or document "no changes needed"

## Commit Convention

Claude artifact sync commits use standard conventional-commit types:

```
docs(rules): update typeck.md — new desugar added in §04.2
docs(claude): add ORI_NEW_FLAG to CLAUDE.md §Commands
docs(rules): update canon.md §4.3 — new CanExpr variant
```

Use `docs` type with the appropriate scope (`rules`, `claude`, `canon`, `syntax`). Commit separately from code changes — doc syncs are their own atomic commits.
