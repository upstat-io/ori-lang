# Create Plan Command

Create a new plan directory with index and section files using the standard template.

## Usage

```
/create-plan <name> [description]
```

- `name`: Directory name for the plan (kebab-case, e.g., `error-recovery`, `lsp-integration`)
- `description`: Optional one-line description of the plan's goal

## Workflow

### Step 1: Gather Information

If not provided via arguments, ask the user:

1. **Plan name** — kebab-case directory name
2. **Plan title** — Human-readable title (e.g., "Error Recovery System")
3. **Goal** — One-line description of what this plan accomplishes
4. **Sections** — List of major sections (at least 2-3)

Use AskUserQuestion if needed to clarify scope.

### Step 2: Read the Template

Read `plans/_template/plan.md` for the structure reference.

### Step 3: Create Directory Structure

Create the plan directory and files:

```
plans/{name}/
├── index.md           # Keyword index for discovery
├── 00-overview.md     # High-level goals and section summary
├── section-01-*.md    # First section
├── section-02-*.md    # Additional sections...
└── section-NN-*.md    # Final section
```

### Step 4: Generate index.md

Create the keyword index with:
- **Reroute frontmatter** (if this is a reroute plan — i.e., a parallel track alongside the main roadmap):
  ```yaml
  ---
  reroute: true
  name: "{Short Name}"
  full_name: "{Full Plan Name}"
  status: queued
  order: N
  ---
  ```
  The `name`, `full_name`, `status`, and `order` fields are the single source of truth for the website.
  `order` controls queue priority — lower value = promoted first (default 999 if omitted).
  `key` and `dir` are derived at load time from the directory name.
- Maintenance notice at the top
- How to use instructions
- Keyword cluster for each section (initially with placeholder keywords)
- Quick reference table

### Step 5: Generate 00-overview.md

Create overview with:
- Plan title and goal
- Section list with brief descriptions
- Dependencies (if any)
- Success criteria

### Step 6: Generate Section Files

For each section, create `section-{NN}-{name}.md` with:
- YAML frontmatter (section ID, title, status: not-started, goal)
- Section header with status emoji
- Placeholder subsections with `- [ ]` checkboxes
- Completion checklist at the end

### Step 7: Report Summary

Show the user:
- Files created
- Next steps (fill in details, add keywords to index)

---

## Example

**Input:** `/create-plan error-recovery "Improve compiler error messages and recovery"`

**Creates:**
```
plans/error-recovery/
├── index.md
├── 00-overview.md
├── section-01-error-types.md
├── section-02-recovery-strategies.md
└── section-03-user-facing-messages.md
```

---

## Section Naming Conventions

| Section Type | Naming Pattern |
|--------------|----------------|
| Setup/Infrastructure | `section-01-setup.md` |
| Core Implementation | `section-02-core.md` |
| Integration | `section-03-integration.md` |
| Testing | `section-04-testing.md` |
| Documentation | `section-05-docs.md` |

---

## After Creation

Remind the user to:
1. Fill in section details with specific tasks
2. Add relevant keywords to `index.md` clusters
3. Update `00-overview.md` with dependencies and success criteria
4. **If performance-sensitive** (lexer, parser, typeck, eval, codegen): Add `/benchmark` checkpoints to relevant sections

## Performance-Sensitive Plans

For plans touching hot paths, include a "Performance Validation" section in `index.md`:

```markdown
## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** [list specific sections]
**Skip benchmarks for:** [list non-perf sections]
```

See `plans/_template/plan.md` for full guidance.

---

## Template Reference

The command uses `plans/_template/plan.md` as the structure reference. See that file for:
- Complete index.md template
- Section file template
- Status conventions
- The roadmap (`plans/roadmap/`) as a working example
