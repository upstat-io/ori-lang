# Shared Reviewer Command File (reviewer-agnostic methodology)

This file contains the review methodology shared by all reviewers
(any model, any tool) across all review skills (`tpr-review`,
`review-work`, `review-plan`, `tp-help`). Reviewer-specific and
tool-specific content lives in the respective skill files that
reference this document.

This file is the canonical, single-source description of *what* a
review must do. It deliberately omits *how* a particular reviewer
launches commands, parses transcripts, or formats output — those are
transport, parser, and presentation concerns owned by the per-reviewer
skill files. When the per-reviewer skill files diverge on methodology,
this document is authoritative.

This is an independent, adversarial review methodology:

- trust current files, fresh command output, and git objects
- distrust summaries, checklists, status metadata, commit messages,
  and prior agent claims until verified
- review the real codebase and the real plan, not the story about them

## Scope Inputs

Every review starts by resolving scope from one of the following:

- a plan directory or section file
  - example: `plans/iter-rc-contract/`
  - example: `plans/iter-rc-contract/section-02-elem-dec-fn.md`
- a section id or topical keywords
  - example: `section-02`
  - example: `02`
  - example: `elem_dec_fn`
- a git range or commit selector
  - example: `HEAD~5..HEAD`
  - example: `abc123..def456`
  - example: `last commit`
  - example: `last 3 commits`
  - example: `abc123`
- uncommitted work selectors
  - `staged`
  - `unstaged`
  - `worktree`
  - `current branch`
- explicit files or directories
  - example: `compiler/ori_arc/src/lower/control_flow/`
  - example: `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs`

If no scope is explicitly named, default to a recent committed slice
plus any uncommitted work:

- committed changes from `HEAD~3..HEAD` (or the available local
  history when fewer than 3 commits exist)
- staged changes from `git diff --cached`
- unstaged changes from `git diff`

## Scope Resolution Order

Resolve the target in this order, taking the first match that yields
a coherent review surface:

1. Existing path from the user
2. Explicit git range or commit selector
3. Explicit uncommitted-work selector (`staged`, `unstaged`,
   `worktree`, `current branch`)
4. Plan match from `plans/*/index.md`, `plans/*/00-overview.md`, and
   `plans/*/section-*.md`
5. If nothing explicit was given, the default recent-committed slice
   plus uncommitted work described above

Broaden the scope if the initial slice is too narrow to be coherent.
Expand stepwise to `HEAD~5..HEAD`, then `HEAD~10..HEAD`, then
`git merge-base master HEAD..HEAD`, until the scope includes the full
implementation story for the touched code.

Treat the initial slice as too narrow when any of these are true:

- the commits are an obvious follow-up or fixup for code mostly
  changed just before the slice
- tests in the slice depend on behavior introduced just before the
  slice
- the owning plan section or nearby plan edits describe work spanning
  more than the slice
- the slice contains partial reverts, cleanup-only commits, or
  verification-only commits that do not show the primary behavior
  change
- file or module history shows the relevant implementation landing
  immediately before the slice

If even the broadened branch-local scope is huge and still not
coherent enough to review well, ask one concise question to narrow it
before proceeding. Otherwise continue.

Commits and diffs are scope *selectors*, not content *filters*. Once
a file or module is in scope, review it completely enough to judge
correctness, tests, rule compliance, and plan drift — not only the
diff hunks that selected it.

## Evidence Gathering

Build the review packet mechanically before forming any conclusions.
Skipping or thinning these inputs is the most common cause of weak
reviews.

### 1. Git Evidence

Collect whichever apply to the review surface:

- committed diff stat for the review range
- committed patch for the review range
- commit log for the review range
- staged diff: `git diff --cached`
- unstaged diff: `git diff`
- current status: `git status --short`

If reviewing committed work, inspect both:

- the cumulative diff for the whole range
- the individual commits in the range, to catch partial reverts,
  contradictory edits, or tests added after code changes

If reviewing staged or unstaged work, compare the right layers:

- staged work against `HEAD`
- unstaged work against the index

When committed, staged, and unstaged layers all exist, walk them
together — partial reverts and silent fixups frequently span layers.

### 2. File Inventory

From the git evidence, identify:

- all changed files
- the tests that should cover them
- adjacent callers, callees, helpers, and data definitions needed to
  understand behavior

READ THE FULL CHANGED FILES, not only the diff hunks. Expand into
neighboring files when needed to verify invariants and downstream
impact. A diff hunk shows what moved; the surrounding code shows
whether the move is consistent with the contract the rest of the
file enforces.

### 3. Standards Packet

The review is not complete until the work has been checked against
the repository standards.

Always read:

- `CLAUDE.md`
- `.claude/rules/tests.md`
- `.claude/rules/compiler.md`
- `.claude/rules/impl-hygiene.md`
- `.claude/rules/roadmap.md`

Also read every file under `.claude/rules/*.md` before finalizing
findings. Prioritize rules that match the changed paths or domain
first, but the final review must account for the full rule set,
marking non-applicable rules as such in your own reasoning rather
than silently skipping them.

### 4. Plan Context

Gather recently modified plans so the review can detect plan drift,
missing updates, and active known work:

1. `git diff --name-only HEAD -- plans/`
2. `git diff --name-only --cached -- plans/`
3. plan files changed inside the committed review range
4. if the review range is omitted, plan files changed in the last few
   local commits touching `plans/`
5. if the user named a plan or section, include that plan's
   `index.md`, `00-overview.md`, the target section, and any
   explicitly referenced neighboring sections

Read the discovered plan files directly. If a section is extremely
large, read its overview/index plus the specific headings tied to the
changed work.

Use plan context to answer:

- which section claims ownership of the code being changed
- whether cross-section edits were reflected in related plans
- whether completion claims still match the repository state
- whether discovered problems were already planned, partially
  addressed, or silently deferred

## Deep Investigation Standard

This is NOT a diff skim. A review fails this standard if any of the
following are skipped on in-scope code:

- read whole changed files
- read enough neighboring code to understand invariants, ownership,
  and boundary contracts
- trace data flow across function, module, and phase boundaries
  touched by the work
- inspect both tests that changed and tests that should have changed
  but did not
- use commit-by-commit history to catch partial reverts and
  contradictory edits hidden by the final tree
- prefer diagnostics, tracing, and repo-native verification tools
  over guesswork

When a change touches ARC, AOT, lowering, runtime, tests, spec, or
roadmap-owned areas, assume the failure surface is wider than the
diff and expand the review accordingly. These subsystems have
historical interaction patterns where a change in one phase silently
invalidates assumptions in another.

When verification is blocked (missing fixture, environment
limitation, irreproducible state), record the gap explicitly so the
consumer can calibrate trust — never paper over it with inference
dressed up as observation.

## Mandatory Standards Checks

Every review must explicitly test the work against these expectations
from `CLAUDE.md` and the rule set:

- bugs are fixed completely, not deferred behind comments or vague
  follow-up notes
- tests come with the fix, and bug fixes have matrix coverage plus at
  least one semantic pin when the changed path is shared
- debug and release verification requirements are respected when the
  rules require them
- plan boundaries are updated when a fix crosses section ownership
  lines
- no silent workarounds, dummy values, or hand-wavy "pre-existing"
  exemptions
- touched Rust files respect hygiene expectations:
  - test placement in sibling `tests.rs`
  - file-size and split expectations
  - tracing instead of `println!`
  - no dead code, unjustified lint suppression, or hidden invariants
- domain-specific rules under `.claude/rules/*.md` are satisfied for
  the relevant subsystems

If the work violates `CLAUDE.md` or a rule file, that IS a finding
even if the code "works." Rule violations are not stylistic; they
encode invariants the rest of the system depends on.

The "Zero Deferral" and "TPR Findings" sections of `CLAUDE.md` are
load-bearing for review judgment:

- a finding cannot be marked resolved with a scope note or
  rationalization
- "pre-existing", "out of scope", "architectural limitation",
  "future improvement", and "conservative/safe" are NOT valid
  resolution reasons — only a real fix or a concrete tracked plan
  item satisfies a finding
- findings may only be rejected when factually incorrect (the
  described issue does not actually exist)

## Finding Format (for envelopes)

Each finding contains the following fields. The authoritative schema
is `.claude/skills/dual-tpr/findings-schema.json`; the description
here exists so the methodology is self-contained.

- `ordinal`: integer, 1-based, independent per reviewer
- `severity`: one of `high`, `medium`, `low`
- `location`: repo-relative path:line matching the canonical regex
  `^[a-zA-Z0-9_./-]+:[0-9]+$` (see
  `.claude/skills/dual-tpr/envelope-format.md` for the full grammar
  including disallowed characters and resolution rules)
- `title`: imperative voice, sentence case, no markdown, no trailing
  punctuation, max 200 characters
- `evidence`: the specific mismatch, regression, or missing case —
  concrete enough that a separate implementer can reproduce the
  observation without re-running the review
- `impact`: why the work is incomplete, unsafe, or non-compliant —
  the consequence to the system, not a restatement of the evidence
- `required_plan_update`: what must be validated and integrated to
  resolve the finding — actionable language, not a wish
- `layer`: one of `committed`, `staged`, `unstaged` — which git
  layer the finding lives in
- `basis`: one of `fresh_verification`, `direct_file_inspection`,
  `git_history`, `inference` (see Verification Basis below)
- `confidence`: one of `high`, `medium`, `low` — calibrated to how
  much corroboration the basis provides
- `citations`: optional array of `{url, description}` for external
  sources (grounded research, specs, prior art) when the finding
  references material outside the repo

Stable IDs use the format `TPR-{section}-{ordinal}` when the section
number is known. One issue per item; combining unrelated issues
under one ID makes triage and selective resolution impossible.

When a prior finding turns out to be invalid or already addressed,
do not delete it. Mark it resolved with a short rationale that
references the validation date. Deletion erases the audit trail and
invites the same finding to be re-filed by the next review.

## Verification Basis

Every finding must declare its basis — exactly one of:

- `fresh_verification`: the reviewer actually ran the test, script,
  or diagnostic and observed the outcome themselves. This is the
  strongest basis. Cite the exact command and the relevant excerpt
  of its output.
- `direct_file_inspection`: the reviewer read the code in the current
  tree and reasoned about it without running it. Strong for
  invariants that are visible at the file level; weaker for
  cross-phase or cross-crate behavior.
- `git_history`: the reviewer inspected commits, patches, or blame
  to trace how the code reached its current state. Useful for
  partial-revert detection and for understanding intent recorded in
  commit messages — but commit messages are not proof of correctness.
- `inference`: the reviewer deduced the conclusion from context
  without direct observation. This is the weakest basis; use
  sparingly and only when stronger bases are infeasible.

Prefer fresh verification when feasible. When it is not, be explicit
about the weaker basis so the consumer can calibrate trust. A
finding with `basis: inference` and `confidence: high` is a yellow
flag — high confidence without observation usually means the
reviewer skipped a check they should have performed.

When a verification step is named in the plan or in commit
discussion, prefer rerunning that exact step over proxies. If a
cheaper command proves the same fact, say so explicitly. If the
verification cannot be reproduced in the current environment, record
the residual risk rather than dropping the check.

## Review Boundaries

Do NOT:

- accept "done" claims because a checklist is checked off
- treat commit messages as proof that the implementation is correct
- ignore staged or unstaged deltas when they materially change the
  reviewed work
- flag speculative issues without evidence
- mark findings resolved with scope notes or rationalizations
- convert findings straight into completed implementation checklist
  items without verifying the actual code change exists
- mark a section `complete` while unchecked third-party findings
  remain in its `Third Party Review Findings` block
- hide rule violations because they look stylistic
- broaden a finding beyond what the evidence supports — speculative
  follow-on issues should be filed separately, not bundled

Do:

- review committed, staged, and unstaged work together when relevant
- call out mismatches between branch history and current tree state
- surface `CLAUDE.md` and rule-file violations explicitly
- annotate when a finding is already covered by a recent plan or
  bug-tracker entry, with a citation to the existing item
- mention residual risk when verification was blocked, naming the
  blocker
- keep findings sharp enough for a later implementation pass to act
  on directly without re-doing the investigation
- reopen completed plan metadata when new third-party findings are
  added so the findings are not hidden behind a completed/resolved
  plan state
- preserve existing findings history when editing plan files; mark
  resolved entries `[x]` rather than removing them

The review is independent and adversarial by design. The goal is to
catch what the implementation pass missed — not to re-tell the
implementation story in a different voice. A review that only
restates what the commit messages already say is a transcription,
not a review.
