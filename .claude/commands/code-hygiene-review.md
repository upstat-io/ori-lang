---
name: code-hygiene-review
description: Review code for hygiene violations and generate a fix plan. NOT refactoring — purely surface cleanliness.
allowed-tools: Read, Grep, Glob, Bash, Task, Edit, Write
---

# Code Hygiene Review

Review code for hygiene violations against `.claude/rules/impl-hygiene.md` and fix them.

**Hygiene is NOT refactoring.** No behavior changes, no API changes, no moving things between modules. Just making existing code clean:
- Dead code, unused imports, stale comments
- Naming inconsistencies
- File/method ordering violations
- Comment style violations (banners, restating code)
- Derive vs manual impl misuse
- Dead public surface
- Import organization

## Target

`$ARGUMENTS` specifies the target (crate, directory, or file):
- `/code-hygiene-review compiler/ori_lexer` — review a crate
- `/code-hygiene-review compiler/ori_types/src/infer/` — review a directory
- `/code-hygiene-review compiler/ori_parse/src/lib.rs` — review a single file

If no argument: ask the user what to review.

## Execution

### Step 1: Load Rules

Read `.claude/rules/impl-hygiene.md` to have the full rule set in context. You will embed these rules verbatim into each agent's prompt (agents cannot read rule files themselves).

### Step 2: Build File Inventory

Use Glob to find all `*.rs` files in the target path. This is the complete file list. Print the total count for the user.

### Step 3: Distribute Files Across 10 Agents

Divide the file list into 10 roughly equal groups using round-robin distribution (file 1 → agent 1, file 2 → agent 2, ... file 11 → agent 1, etc.). This balances work across agents regardless of file size ordering.

### Step 4: Spawn 10 Agents in Parallel

Launch exactly 10 Task agents **in a single message** (all 10 tool calls in one response) using `subagent_type: "general-purpose"` and `model: "sonnet"`. Each agent runs in the background (`run_in_background: true`).

Each agent's prompt MUST include:

1. **The complete hygiene rules** (embed the full content of `impl-hygiene.md` directly — agents cannot read rule files)
2. **The exact list of files** this agent is responsible for
3. **The processing instructions** (see below)
4. **The audit checklist** (see below)

#### Agent Instructions Template

```
You are a code hygiene fixer. You will process a list of files ONE AT A TIME, in order.

## CRITICAL: One File At A Time

You MUST process files sequentially. For EACH file in your list:
1. Read the file (ONE file only — never read multiple files at once)
2. Audit it against every hygiene rule below
3. Apply ALL fixes to that file using Edit/Write tools
4. Confirm the file is complete and clean
5. ONLY THEN move to the next file

DO NOT read ahead. DO NOT read files that are not in your list.
DO NOT read your next file until you have finished fixing the current one.
Each file is completely independent — you do not need context from any other file for this task.
Code cleanup is internal to each file.

## Your Files (process in this exact order)

{numbered list of files for this agent}

## Hygiene Rules

{full content of impl-hygiene.md}

## Audit Checklist

For each file, check and fix:

**File Organization:**
- Module doc comment present?
- Sections in correct order? (mods → imports → types → impls → fns → tests)
- Imports in 3 groups with blank-line separators?
- No inline test modules? (must be `#[cfg(test)] mod tests;` declaration only — if you find `#[cfg(test)] mod tests { ... }` with an inline body, extract it to a sibling `tests.rs` file)

**Impl Block Ordering:**
- Constructors first?
- Accessors before predicates before operations before helpers?
- pub before private within groups?

**Naming:**
- Function names follow verb-prefix conventions?
- Variable names scope-scaled? (no `token_kind_result` in a 3-line scope, no `t` in a 50-line function)
- Standard abbreviations used consistently?

**Struct/Enum Fields:**
- Primary data first, flags last?
- Inline comments where purpose isn't obvious?

**Comments:**
- No decorative banners? (`───`, `===`, `***`, `---`)
- No comments restating code?
- No commented-out code?
- `///` on all pub items?
- WHY comments, not WHAT?

**Derive vs Manual:**
- Any manual trait impls that duplicate derive behavior?

**Visibility:**
- Any dead pub items? (pub but unused outside crate)
- Any dead code? (unused functions, imports, variants)

**File Size:**
- Source file (excluding `tests.rs`) over 500 lines? Flag it — should be split into submodules

**Style:**
- All `#[allow(clippy)]` have `reason`?
- Functions under 50 lines? (note: dispatch tables exempt)
- No dead/commented-out code?

## Fixing Rules

- **No behavior changes** — Hygiene is purely cosmetic/organizational
- **No refactoring** — Don't extract functions, move modules, or change APIs
- **Test code is relaxed** — Section banners (`// === Section ===`) are OK in `tests.rs` files
- **Dispatch tables exempt** — Large match statements mapping tokens/tags are not "long functions"
- **Be precise with edits** — Use Edit tool with exact old_string matches. Do not rewrite entire files unless necessary.
- **If unsure, skip it** — When something is a judgment call and the current code is reasonable, leave it alone

## Inline Test Extraction

If you find `#[cfg(test)] mod tests { ... }` with an inline body, extract it:

1. **Determine the sibling path:**
   - `foo.rs` → `foo/tests.rs` (mkdir `foo/` if needed)
   - `bar/mod.rs` → `bar/tests.rs`
   - `lib.rs` / `main.rs` → `tests.rs` in the same directory
2. **Skip** if the sibling `tests.rs` already exists
3. **Extract** the inner content (everything between the outer `{` and `}`), dedent by one level (4 spaces)
4. **Write** the extracted content to the sibling `tests.rs`
5. **Replace** the inline block with just the declaration (preserve any attributes like `#[allow(...)]`):
   ```rust
   #[cfg(test)]
   mod tests;
   ```
6. **Alternatively**, run `python3 scripts/extract_tests.py --file <path> --apply` which handles all of this automatically

## Output Format

After processing ALL your files, return a summary of what you did:

### {filepath}
- Fixed: {brief description of each fix applied}
- OR: CLEAN (no changes needed)

Report the total count: N files processed, M files modified, K files clean.
```

### Step 5: Monitor and Collect Results

Wait for all 10 agents to complete by reading each agent's output file. Compile a summary of all changes made across all agents.

### Step 6: Verify No Regressions

After all agents have finished:
1. Run `./clippy-all.sh` to verify no regressions
2. Run `./test-all.sh` to verify no behavior changes
3. If any failures, investigate and fix them

### Step 7: Report

Present the user with a consolidated summary:

```
## Hygiene Review: {target}

**Scope:** N files reviewed across 10 agents
**Results:** M files modified, K files already clean

### Changes by file
{merged summaries from all agents, listing only modified files}

### Verification
- clippy: PASS/FAIL
- tests: PASS/FAIL
```
