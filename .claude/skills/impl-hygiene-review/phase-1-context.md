# Phase 1 — Load Rules & Context

Read by a Sonnet sub-agent dispatched from `/impl-hygiene-review`. Not a registered skill. Loads CLAUDE.md, `.claude/rules/*.md`, and active-plan context into a structured context packet the Phase 3 Opus agent consumes.

Writes `/tmp/impl-hygiene-{run}/phase-1.json` summarizing which rules are in scope, the active plan (if any), and any rule frameworks (LEAK/DRIFT/GAP/WASTE/EXPOSURE/BLOAT/NOTE taxonomy) that apply to the review target.

---

### Phase 1: Load Rules & Context

#### 1a. Load Rules

The full rule set is embedded below (source of truth files — do not maintain separate copies):

**Hygiene Rules** (`.claude/rules/impl-hygiene.md`):
@.claude/rules/impl-hygiene.md

**Compiler Guidelines** (`.claude/rules/compiler.md`):
@.claude/rules/compiler.md

#### 1b. Load Plan Context

Gather context from active and recently-modified plan files so the review doesn't flag work that is already planned, in-progress, or intentionally deferred.

**Procedure:**
1. Run `git diff --name-only HEAD` and `git diff --name-only --cached` to find uncommitted modified files in `plans/`
2. Run `git diff --name-only HEAD~3..HEAD -- plans/` to find plan files changed in recent commits
3. Combine both lists (deduplicate) to get all recently-touched plan files
4. Read each discovered plan file (skip files > 1000 lines — read the `00-overview.md` or `index.md` instead)

**How to use plan context:**

Plan context does NOT suppress or deprioritize findings. Instead, it **annotates** them:

- If a finding falls within scope of an active plan, append `→ covered by plans/{plan}/` to the finding
- If a plan has an active reroute or suspension notice (e.g., "all work suspended until X"), note this in the review preamble so the user knows which areas are in flux
- If a plan explicitly describes a refactor that would resolve a finding, mark it as `[PLANNED]` instead of proposing a separate fix — but still list it so nothing falls through cracks
- Findings NOT covered by any plan are reported normally — these are the high-value discoveries

**Example annotation:**
```
3. **[DRIFT]** `compiler/ori_types/src/check/registration/mod.rs:142` — Missing sync for new `Serialize` variant
   → covered by plans/trait_arch/ (Section 3: Registration Overhaul)
```

This ensures the review adds value by distinguishing "known debt being addressed" from "unknown debt needing attention."

