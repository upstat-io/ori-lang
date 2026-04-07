# Dual-TPR Envelope Format V1

This document is the canonical reference for the **Dual-TPR Findings Envelope V1**
format that Codex CLI and Gemini CLI both emit when invoked as third-party
reviewers via the `dual-tpr` skill family.

> **SSOT pointer:** the JSON Schema for the envelope lives at
> `.claude/skills/dual-tpr/findings-schema.json`. Anywhere this document quotes
> a schema constraint (regex, length cap, enum), the schema file is the
> authoritative source — keep this document in sync with that file.

---

## Overview

Two reviewers run in parallel for every dual-tpr review round:

- **Codex** (`codex exec --full-auto --output-schema findings-schema.json ...`)
  emits a JSON envelope **directly** as the final `agent_message.text`. No
  sentinels are required because Codex's `--output-schema` flag forces the
  output to conform to the schema at the CLI boundary.

- **Gemini** (`gemini --approval-mode yolo --output-format stream-json ...`)
  has no `--output-schema` equivalent. Its prompt instructs it to wrap the
  envelope between two HTML-comment sentinels at the END of the reviewer's
  free-form prose response. The transport layer (Section 02) extracts the
  envelope post-hoc and validates it against the schema after extraction.

This is the **asymmetric rigor pattern**: Codex is strict at the CLI boundary,
Gemini is lenient at the CLI boundary but strict post-extraction. Both produce
the same `FindingsEnvelope` shape downstream, so the merge layer never has to
care which reviewer produced a given envelope.

---

## Sentinel format (Gemini only)

The two sentinels are HTML comments that survive markdown rendering invisibly
and are easy to grep:

| Marker | Literal value |
|---|---|
| BEGIN  | `<!-- BEGIN-ORI-DUAL-TPR-V1 -->` |
| END    | `<!-- END-ORI-DUAL-TPR-V1 -->`   |

### Why two sentinels (BEGIN AND END)

A single sentinel cannot distinguish "the reviewer produced a clean envelope"
from "the reviewer's response was truncated mid-envelope". With both sentinels,
the parser can detect three distinct cases:

1. **Clean envelope** — BEGIN found, END found, JSON between them parses and
   validates → schema-compliant envelope returned to caller.
2. **Truncated response** — BEGIN found, END missing → caller returns
   `failed_partial` round (the reviewer started writing the envelope but the
   process was cut off; this is recoverable via infra retry).
3. **No envelope at all** — BEGIN never appears → caller returns
   `failed_partial` (the reviewer never reached the envelope-writing phase).

### Why the `V1` suffix

The version suffix is the schema versioning hook. When the envelope schema is
revised (added required fields, changed enum values, etc.), the new version
introduces fresh sentinels (e.g. `BEGIN-ORI-DUAL-TPR-V2` /
`END-ORI-DUAL-TPR-V2`) that coexist with V1 during transition. The transport
layer can support both simultaneously by trying each sentinel pair in order
of preference (newest first, fall back to older versions).

---

## Sentinel placement

The reviewer prompt instructs Gemini to place the envelope at the END of its
response, not in the middle:

- All free-form prose (analysis, scope expansion notes, evidence summaries)
  goes ABOVE the envelope.
- Exactly one blank line above the BEGIN sentinel.
- BEGIN sentinel on its own line.
- Then a fenced JSON code block (` ```json ... ``` `).
- Then END sentinel on its own line.
- Exactly one blank line below the END sentinel (or end of file).

### Concrete example

```text
Free text from the reviewer about what they investigated, why,
where they expanded scope, how they verified findings, what citations
they consulted. Multiple paragraphs are allowed and encouraged.

The reviewer can continue writing as much prose as it wants here —
none of it is parsed. Only the JSON between the sentinels is consumed
by the orchestrator.

<!-- BEGIN-ORI-DUAL-TPR-V1 -->
` ``json
{ ...envelope... }
` ``
<!-- END-ORI-DUAL-TPR-V1 -->
```

(In the example above, the literal triple-backticks of the inner fence are
shown with a space inserted to avoid breaking out of this document's outer
fence. In an actual envelope, the inner fence is the standard
` ```json ... ``` ` markdown code block with no inserted space.)

---

## Codex case (no sentinels needed)

When Codex is invoked with `--output-schema findings-schema.json`, the CLI
forces the final `agent_message.text` to be a JSON document conforming to the
schema. The transport extractor reads `$RUN/codex.jsonl`, locates the final
`item.completed` event whose item is an `agent_message`, and parses its `text`
field with `json.loads()` directly:

```python
import json

def parse_codex(jsonl_path: str) -> dict:
    with open(jsonl_path) as f:
        for line in f:
            event = json.loads(line)
            if event.get("type") == "item.completed":
                item = event.get("item", {})
                if item.get("type") == "agent_message":
                    return json.loads(item["text"])  # already schema-conformant
    raise RuntimeError("no agent_message found in codex output")
```

No sentinel extraction step. No fenced-block search. No prose-stripping. The
asymmetric rigor pattern means Codex is trusted at the CLI boundary because
`--output-schema` makes the trust load-bearing.

---

## Gemini case (sentinels required)

Gemini has no `--output-schema` flag, so its envelope is extracted post-hoc.
The transport extractor runs the following pipeline:

1. **Read** `$RUN/gemini.jsonl`. This file contains the stream-json output:
   one JSON object per line, including delta chunks, assistant messages,
   and a terminal `{"type":"result","status":"success"}` event.

2. **Concatenate** all `{"type":"message","role":"assistant",...,"delta":true}`
   message fragments in arrival order. Per Codex Step 6B's catch, gemini
   streams assistant content in chunks; the parser must reassemble them into
   a single contiguous string before searching for sentinels. Out-of-order
   reassembly is a parser bug — events arrive in chronological order in the
   JSONL stream, and the concatenator must preserve that order.

3. **Wait** for the terminal `{"type":"result","status":"success"}` event. If
   the file ends without this event (or with `status:"failed"`), the
   reviewer round is `failed_partial` regardless of what is in the
   concatenated text.

4. **Search** the concatenated text for the BEGIN sentinel:
   `<!-- BEGIN-ORI-DUAL-TPR-V1 -->`. If not found → `failed_partial`.

5. **Extract** the fenced JSON block immediately following the BEGIN sentinel.
   The block starts at the next ` ```json ` opener and ends at the next
   ` ``` ` closer. If either delimiter is missing → `failed_partial`.

6. **Verify** the END sentinel `<!-- END-ORI-DUAL-TPR-V1 -->` appears after
   the fenced block. If missing → `failed_partial` (truncation detected).

7. **Validate** the extracted JSON against `findings-schema.json` using the
   same JSON Schema validator the rest of the system uses. Any validation
   error → `failed_partial`.

8. **Any failure** at any step above → return a `failed_partial` reviewer
   round to the caller. The caller's infra retry logic (Section 02.4) is
   responsible for deciding whether to re-invoke gemini or surface the
   failure to the user. Validation failures are NOT retried (they indicate
   a prompt-discipline problem, not a transient infra issue).

### Reference Python sketch

```python
import json
import re

BEGIN = "<!-- BEGIN-ORI-DUAL-TPR-V1 -->"
END = "<!-- END-ORI-DUAL-TPR-V1 -->"
FENCE_RE = re.compile(r"```json\s*\n(.*?)\n```", re.DOTALL)

def parse_gemini(jsonl_path: str) -> dict:
    fragments = []
    saw_terminal_success = False
    with open(jsonl_path) as f:
        for line in f:
            event = json.loads(line)
            etype = event.get("type")
            if etype == "message" and event.get("role") == "assistant" and event.get("delta"):
                fragments.append(event.get("content", ""))
            elif etype == "result":
                if event.get("status") == "success":
                    saw_terminal_success = True
                break
    if not saw_terminal_success:
        raise RuntimeError("gemini did not emit terminal success event")
    text = "".join(fragments)
    begin_idx = text.find(BEGIN)
    if begin_idx < 0:
        raise RuntimeError("BEGIN sentinel not found in gemini response")
    after_begin = text[begin_idx + len(BEGIN):]
    match = FENCE_RE.search(after_begin)
    if not match:
        raise RuntimeError("no fenced JSON block after BEGIN sentinel")
    after_fence = after_begin[match.end():]
    if END not in after_fence:
        raise RuntimeError("END sentinel not found after fenced JSON block")
    return json.loads(match.group(1))
```

---

## Canonical location format

**Regex:** `^(?!/)(?!\./)[a-zA-Z0-9_./-]+:[0-9]+$`

> **Authoritative source:** `.claude/skills/dual-tpr/findings-schema.json` field
> `findings.items.properties.location.pattern`. This document quotes the regex
> for human reference; if the two ever disagree, the schema file wins and this
> document must be updated.

### Format breakdown

- `<repo-relative path>` — the file path relative to the repo root. The two
  leading negative lookaheads `(?!/)(?!\./)` reject leading `/` (absolute
  paths) and leading `./` (current-dir prefix). Dotfiles like `.gitignore`
  and dot-directories like `.cargo/` ARE allowed because their leading `.`
  is not followed by `/`.
- `:` — single colon separator between path and line number.
- `<line number>` — single integer in `[1, ∞)`. No ranges, no commas, no
  alternatives.

### Valid examples

| Location | Notes |
|---|---|
| `compiler/ori_arc/src/lower/control_flow/mod.rs:123` | Deep repo-relative path |
| `library/std/iter.ori` (not standalone — needs `:N`) | Path only, must add `:line` |
| `library/std/iter.ori:45`                  | Standard valid form |
| `tests/spec/collections/cow/test.ori:1`    | Test file |
| `Cargo.toml:5`                             | Repo-root file |
| `.gitignore:3`                             | Dotfile (allowed) |
| `.cargo/config.toml:1`                     | Dot-rooted directory (allowed) |

### Invalid examples

| Location | Why rejected |
|---|---|
| `/home/eric/projects/ori_lang/file.rs:1` | Absolute path — leading `/` is rejected |
| `./file.rs:1`                            | Leading `./` is rejected |
| `file.rs`                                | Missing `:line` separator |
| `file.rs:`                               | Empty line number |
| `file.rs:1-10`                           | Line range — `-` is in path-char class but a hyphen after `:` doesn't match `[0-9]+` |
| `file.rs:abc`                            | Non-numeric line |
| `file.rs:1,2,3`                          | Multi-line list — comma is not allowed in either path or line position |
| `file with spaces.rs:1`                  | Space is not in `[a-zA-Z0-9_./-]` |

### Rationale

Exact-match agreement detection between Codex and Gemini requires
**byte-identical** location strings. Repo-relative is the only canonical form
because absolute paths and `./` prefixes encode environment-specific
information (the absolute path differs per developer / CI runner; `./` vs no
`./` is a stylistic choice with no semantic difference). Without canonical
form, the merge layer would have to normalize before comparing, which would
require an additional consensus point and a third potential bias source.

---

## Canonical title style

The schema enforces only the **structural** constraints (length cap, type =
string). The remaining style constraints are enforced by **prompt
instructions** in the reviewer SKILL.md files because they are
natural-language properties that JSON Schema cannot express.

### Schema-enforced (in `findings-schema.json`)

- **Type:** string (no nesting, no objects)
- **Maximum length:** 200 characters

### Prompt-enforced (in reviewer SKILL.md instructions)

- **Imperative voice (verb-first):** "Add", "Fix", "Replace", "Remove",
  "Move", "Rename", "Insert", "Drop", "Tighten", "Loosen". The first word
  is a verb in the imperative mood, never a gerund (`-ing`) or past tense
  (`-ed`).
- **Sentence case:** capitalize the first word and proper nouns; lowercase
  everything else. Not Title Case, not lowercase.
- **No markdown formatting:** ` `code` `, `**bold**`, `_italic_`,
  `[link](url)` are all forbidden in titles. Use plain text only.
- **No trailing punctuation:** no period, no exclamation mark, no question
  mark.
- **No interrogative form:** "Why is X not detected?" is not a finding, it
  is a discussion question. Findings are statements of what to do.

### Valid examples

| Title | Notes |
|---|---|
| `Add dec on early-exit branch in lower_branch` | Verb + object + scope |
| `Fix off-by-one in range_len for empty ranges` | Verb + bug + context |
| `Replace println with tracing::debug in eval/iterator` | Verb + old + new + scope |
| `Remove dead match arm in resolve_iterator_method` | Verb + target + scope |
| `Tighten location regex to reject absolute paths` | Verb + target + condition |

### Invalid examples (caught by prompt instructions, not schema)

| Title | Violation |
|---|---|
| `Adding a dec.`                                        | Gerund + trailing period |
| `**Add dec**`                                          | Markdown bold |
| `add dec on early-exit branch`                         | Not sentence case (lowercase first word) |
| `Why is this not detected?`                            | Interrogative + question mark |
| `fix bug in foo`                                       | Not sentence case |
| `Add dec on early-exit branch and also fix the issue with the lowering of nested control flow constructs in the case where multiple loops are nested with break-with-value...` | Exceeds 200 characters |

### Rationale

Same as the location rationale — exact-match agreement detection requires
byte-identical title strings across both reviewers. The schema enforces what
it can (length); the prompt enforces what it cannot (style). The combination
gives reliable agreement detection without requiring a fuzzy-matching layer
in the merge step.

---

## Example envelopes

The two examples below illustrate the full envelope shape for each reviewer.
Both validate cleanly against `findings-schema.json` (verified via
`validate-envelopes.sh`).

### Gemini case — multi-finding with grounded citations

This is what a complete gemini reviewer round looks like in the wire format.
Note the surrounding free-form prose, the BEGIN sentinel, the fenced JSON
block, and the END sentinel:

```text
I reviewed the changes in compiler/ori_runtime/src/sync/atomic.rs and
compiler/ori_arc/src/refcount/mod.rs against the Rust standard library
documentation for atomic ordering. I expanded scope beyond the starting
packet to include refcount/mod.rs because the atomic.rs change has a direct
caller in that file. I verified each finding against the cited Rust std
documentation and against the C++ memory_order reference for cross-language
context.

<!-- BEGIN-ORI-DUAL-TPR-V1 -->
` ``json
{
  "schema_version": "1.0",
  "status": "complete",
  "reviewer": "gemini",
  "skill": "tpr-review",
  "scope_actually_reviewed": {
    "git_range": "HEAD~1..HEAD",
    "files_read": [
      "compiler/ori_runtime/src/sync/atomic.rs",
      "compiler/ori_arc/src/refcount/mod.rs"
    ],
    "rules_consulted": [".claude/rules/runtime.md"],
    "specs_consulted": [],
    "plans_consulted": [],
    "expanded_beyond_packet": true,
    "expansion_reason": "atomic.rs change has a direct caller in refcount/mod.rs that needed inspection to confirm the ordering propagates correctly."
  },
  "findings": [
    {
      "ordinal": 1,
      "severity": "medium",
      "location": "compiler/ori_runtime/src/sync/atomic.rs:42",
      "title": "Use Acquire ordering instead of Relaxed for refcount load",
      "evidence": "Line 42 uses Ordering::Relaxed when loading the refcount before the dec branch, but the Rust std documentation states refcount drops must use Acquire to synchronize with prior Release stores from other threads.",
      "impact": "On weakly-ordered architectures (ARM, RISC-V) the relaxed load can return a stale refcount value, leading to a missed deallocation or premature free under contended drop paths.",
      "required_plan_update": "Update plans/runtime-correctness/atomic-orderings.md to reference Rust std atomic guidance and add a TSan test exercising the refcount drop path under contention.",
      "basis": "fresh_verification",
      "layer": "committed",
      "confidence": "high",
      "citations": [
        {
          "url": "https://doc.rust-lang.org/std/sync/atomic/",
          "description": "Rust atomic ordering reference — Acquire/Release semantics for refcount drops"
        },
        {
          "url": "https://en.cppreference.com/w/cpp/atomic/memory_order",
          "description": "C++ memory_order reference — the underlying model Rust inherits"
        }
      ]
    },
    {
      "ordinal": 2,
      "severity": "high",
      "location": "compiler/ori_arc/src/refcount/mod.rs:87",
      "title": "Add Release fence before final refcount decrement in drop_arc",
      "evidence": "drop_arc decrements the refcount with Acquire ordering at line 87, but the prior writes to the boxed value (line 80-86) need a Release fence before the dec to prevent a thread that observes refcount=0 from racing on the now-freeable storage.",
      "impact": "Use-after-free under specific weak-memory thread interleavings — observable on aarch64 with TSan, not observable on x86_64 due to TSO.",
      "required_plan_update": "Add a SeqCst or Release fence in drop_arc and a TSan test under aarch64 emulation.",
      "basis": "fresh_verification",
      "layer": "committed",
      "confidence": "high",
      "citations": [
        {
          "url": "https://doc.rust-lang.org/nomicon/arc-mutex/arc-drop.html",
          "description": "Rustonomicon — Arc Drop fence pattern"
        }
      ]
    }
  ],
  "verification": {
    "tests_rerun": [],
    "diagnostics_run": [],
    "verification_gaps": [
      "No TSan test currently exercises the refcount drop path under aarch64 weak ordering"
    ]
  },
  "no_findings": false
}
` ``
<!-- END-ORI-DUAL-TPR-V1 -->
```

(The space inserted between the backticks of the inner fence is a documentation
artifact to avoid breaking out of this document's outer fence — actual gemini
output uses the standard ` ```json ... ``` ` markdown fence with no inserted
space.)

### Codex case — raw JSON, no sentinels

This is what a complete codex reviewer round looks like in the wire format.
Note that there is NO surrounding prose, NO sentinels, and NO fenced block —
the entire `agent_message.text` field IS the JSON envelope, because
`--output-schema findings-schema.json` forces this shape at the CLI boundary:

```json
{
  "schema_version": "1.0",
  "status": "complete",
  "reviewer": "codex",
  "skill": "tpr-review",
  "scope_actually_reviewed": {
    "git_range": "HEAD~3..HEAD",
    "files_read": [
      "compiler/ori_arc/src/lower/control_flow/mod.rs",
      "compiler/ori_arc/src/lower/control_flow/branch.rs",
      "compiler/ori_arc/src/aims/realize.rs",
      "plans/repr-opt/section-04-control-flow.md"
    ],
    "rules_consulted": [
      ".claude/rules/arc.md",
      ".claude/rules/impl-hygiene.md"
    ],
    "specs_consulted": [
      "docs/ori_lang/v2026/spec/operator-rules.md"
    ],
    "plans_consulted": [
      "plans/repr-opt/section-04-control-flow.md"
    ],
    "expanded_beyond_packet": true,
    "expansion_reason": "The starting packet referenced lower_branch but the actual RC imbalance traced through three additional files in the lowering pipeline; expanded into branch.rs and realize.rs to confirm the lineage."
  },
  "findings": [
    {
      "ordinal": 1,
      "severity": "high",
      "location": "compiler/ori_arc/src/lower/control_flow/mod.rs:123",
      "title": "Add dec on early-exit branch in lower_branch",
      "evidence": "lower_branch generates an inc on entry at line 118 but the early-exit path at line 138 returns without emitting the matching dec, leaving the refcount imbalanced by +1 per traversal of the break path.",
      "impact": "Memory leak on every loop body that contains a break or continue against an arc-counted value; breaks the AIMS Certified contract for any function containing such control flow.",
      "required_plan_update": "Add a regression test to plans/repr-opt/section-04-control-flow.md exercising for+break with an arc-counted body type, plus a debug_assert in lower_branch that catches the imbalance at compile time.",
      "basis": "direct_file_inspection",
      "layer": "committed",
      "confidence": "high"
    },
    {
      "ordinal": 2,
      "severity": "high",
      "location": "compiler/ori_arc/src/lower/control_flow/branch.rs:87",
      "title": "Insert phantom marker for moved capture in nested closure",
      "evidence": "The nested closure lowering at branch.rs:87 takes ownership of an outer capture without inserting the phantom marker that the AIMS realize pass relies on for lineage tracking, causing realize.rs:204 to misclassify the move as a borrow.",
      "impact": "AIMS Certified contract is incorrectly granted to functions that perform a real ownership transfer, which can produce double-free or use-after-free in release builds where FastISel skips the redundant inc/dec elision.",
      "required_plan_update": "Add a phantom-marker check to the AIMS contract verification step in plans/repr-opt/section-05-realize.md and a TDD matrix entry covering nested-closure capture patterns.",
      "basis": "fresh_verification",
      "layer": "committed",
      "confidence": "high"
    }
  ],
  "verification": {
    "tests_rerun": [
      "cargo test -p ori_arc lowering::control_flow",
      "ORI_CHECK_LEAKS=1 ./target/debug/ori run tests/spec/control_flow/break_in_for.ori"
    ],
    "diagnostics_run": [
      "diagnostics/arc-dump.sh tests/spec/control_flow/break_in_for.ori",
      "diagnostics/rc-stats.sh tests/spec/control_flow/break_in_for.ori"
    ],
    "verification_gaps": []
  },
  "no_findings": false
}
```

Note that the codex envelope above has no `citations` arrays on its findings —
codex does not perform web search. This is the structural complement to
gemini's `citations`-bearing findings: same envelope shape, different
content patterns.

---

## How agreement is detected

When both reviewers run on the same review round, the merge layer (Section
02.5) compares findings across the two envelopes to detect **agreement** and
**disagreement** cases. The detection rule is intentionally strict:

> Two findings — one from each reviewer — are considered an **agreement** if
> and only if their `(location, title)` pair is **byte-identical**. Anything
> less is treated as two distinct findings.

### Why byte-identical (and not fuzzy)

The strict-match policy is deliberate, per Codex Step 6B Q7. Three reasons:

1. **Avoid introducing a third bias source.** The whole point of running two
   reviewers in parallel is to reduce bias by getting independent perspectives.
   A fuzzy matcher (Levenshtein distance, normalized whitespace, lowercased
   titles, etc.) is itself a third opinion that decides which pairs of
   findings "really mean the same thing". That third opinion is a new bias
   source that can mask real disagreements.

2. **Force canonical formatting upstream.** The canonical `(location, title)`
   format defined in this document exists precisely because exact-match
   agreement detection is the policy. Both reviewers are instructed to emit
   identical canonical strings; if they don't, the prompt is at fault and
   needs strengthening, not the matcher.

3. **Surface disagreements as a feature, not noise.** When the two reviewers
   produce findings that "look similar" but are not byte-identical, that
   difference is informative — it means at least one reviewer is being sloppy
   about canonical form, OR the two reviewers are pointing at subtly
   different things. Both cases deserve human attention. A fuzzy matcher
   would silently merge them, hiding the disagreement.

### Output of the merge step

The merge layer produces three categories of findings, all returned to the
caller (Claude) so they can be written to the plan:

| Category | Definition | Tag suffix |
|---|---|---|
| **Agreement** | Both reviewers emitted byte-identical `(location, title)`. | `[TPR-NN-NNN-both]` (caller may also retain per-reviewer tags) |
| **Codex-only** | Codex emitted a finding with no matching gemini finding. | `[TPR-NN-NNN-codex]` |
| **Gemini-only** | Gemini emitted a finding with no matching codex finding. | `[TPR-NN-NNN-gemini]` |

The reviewer-tag ID format and the ordinal allocation rules are defined in
the next section.

---

## Reviewer-tag ID format

When Claude writes findings into a plan file's `## NN.R Third Party Review
Findings` block, each finding is prefixed with a reviewer-tagged identifier.
The format is:

```
[TPR-{section}-{ordinal}-{reviewer}]
```

| Component | Format | Description |
|---|---|---|
| `{section}` | two-digit zero-padded | The owning plan section number (e.g., `02`, `03`, `15`) |
| `{ordinal}` | three-digit zero-padded | A counter, INDEPENDENT per reviewer — codex's first finding for section 02 is `001`, gemini's first finding for section 02 is also `001`. The ordinals do NOT share a namespace |
| `{reviewer}` | literal | Either `codex` or `gemini` |

### Examples

| Tag | Meaning |
|---|---|
| `[TPR-02-001-codex]`   | Codex's 1st finding for section 02 |
| `[TPR-02-001-gemini]`  | Gemini's 1st finding for section 02 — NOT necessarily the same finding as codex's 1st. Whether they refer to the same issue is decided by `(location, title)` byte-identical match at presentation time, not by ordinal coincidence |
| `[TPR-04-007-codex]`   | Codex's 7th finding for section 04 |
| `[TPR-08-012-gemini]`  | Gemini's 12th finding for section 08 |

### Writing into the plan file

When the merge layer presents findings to Claude for writing into the plan,
each reviewer's findings appear with its own ordinal sequence:

- **Each reviewer's findings are written with its own ordinal sequence.**
  Codex's findings are numbered 001, 002, 003... in `[TPR-NN-NNN-codex]`
  form. Gemini's findings are numbered 001, 002, 003... in
  `[TPR-NN-NNN-gemini]` form. The ordinals are not shared.
- **Agreements (same `(location, title)` from both reviewers) appear as TWO
  entries** in the TPR block — both visible to the human reader, both with
  the same `(file:line, title)` pair, but with different `-codex` and
  `-gemini` suffixes. The human reads both adjacent entries and recognizes
  the agreement immediately.
- **Disagreements (one reviewer flagged, the other didn't) appear as one
  entry** with one tag.

### Why independent ordinals

The decision to keep ordinal namespaces independent per reviewer is
deliberate, per Codex Step 6B Q4. The alternative — sharing a base ID like
`[TPR-02-001-codex]` paired with `[TPR-02-001-gemini]` to represent "the same
finding from two reviewers" — would bake an equivalence claim into ID
assignment. Whether two findings are "the same" is a judgment call that
should NOT be implicit in the IDs.

The cleaner design: each reviewer numbers independently in its own namespace,
and the human reader recognizes equivalence by reading two adjacent entries
that share `(location, title)`. This separates the **merging** concern (done
by the merge layer at write time, byte-exact only) from the **identification**
concern (done by ordinal allocation, per-reviewer independent), and prevents
the orchestrator from silently resolving disagreements by ID convention.

---

## Per-run scratch directory conventions

All reviewer rounds use a per-run scratch directory created at the start of
the round:

```bash
RUN=$(mktemp -d -t ori-tpr-XXXXXXXX)
```

The `XXXXXXXX` template generates an 8-character random suffix; the directory
is created under `$TMPDIR` (typically `/tmp` on Linux) with a name like
`/tmp/ori-tpr-A1B2C3D4`. Each round gets its own `$RUN`, so concurrent
invocations never race.

### File layout inside `$RUN`

| File | Contents |
|---|---|
| `$RUN/codex.prompt.md`     | The prompt sent to codex (preserved for postmortem inspection) |
| `$RUN/codex.jsonl`         | Codex's stdout — `item.completed` JSONL stream |
| `$RUN/gemini.prompt.md`    | The prompt sent to gemini |
| `$RUN/gemini.jsonl`        | Gemini's stdout — `stream-json` JSONL stream |
| `$RUN/codex.envelope.json` | Extracted+validated codex envelope (cached after parse for downstream reuse) |
| `$RUN/gemini.envelope.json`| Extracted+validated gemini envelope (cached after parse for downstream reuse) |
| `$RUN/worktree-before.txt` | `git status --porcelain` snapshot taken before reviewer launches |
| `$RUN/worktree-after.txt`  | `git status --porcelain` snapshot taken after both reviewers complete |
| `$RUN/round.log`           | Orchestration log: which reviewer started when, infra retry counts, failure reasons |

The two `worktree-*.txt` snapshots feed the dirty-worktree guard: if the
two snapshots differ, at least one reviewer modified tracked source files
(violating the prompt-discipline contract that reviewers must not write to
the source tree). The diff is surfaced to the user in the failure message.

### Cleanup policy

- **Successful round** — both reviewers returned valid envelopes, the
  dirty-worktree guard passed, the schema validated both envelopes, and
  the merge layer produced findings. After the findings are written to the
  plan file: `rm -rf "$RUN"`.
- **Failed round** — any infra failure (timeout, nonzero exit, missing
  terminal success event), parse failure (missing sentinel, malformed JSON),
  schema validation failure, OR dirty-worktree guard rejection. **Retain
  `$RUN`** for postmortem inspection. Print the path to the user as part
  of the failure message: `Round failed; postmortem dir retained at $RUN`.
- **Multi-iteration loops** — within a multi-iteration TPR loop (e.g.,
  10 semantic iterations of `/tpr-review`), each iteration gets its own
  `$RUN` directory. Successful intermediate iterations are cleaned up;
  failed intermediate iterations are retained for postmortem.

### Why per-run instead of fixed paths

The existing single-source wrappers use fixed paths like
`/tmp/tpr-iter.jsonl`, `/tmp/review-work.jsonl`, `/tmp/tp-help.jsonl`. These
paths have a latent race-condition bug: if two review wrappers run
concurrently (or two iterations of the same wrapper overlap), they clobber
each other's stdout. The bug has not been observed in practice because users
rarely run two reviews simultaneously, but the dual-source plan exposes it
in three ways:

1. **Two reviewers per round** — `dual-invoke.sh` launches both codex and
   gemini in parallel; if both wrote to `/tmp/review.jsonl`, the second one
   to write would overwrite the first.
2. **Concurrent invocations** — if a user runs `/tpr-review` in two terminal
   windows simultaneously (e.g., on two different feature branches), the
   fixed-path bug would cause cross-contamination of reviewer outputs.
3. **No postmortem isolation** — fixed paths get overwritten on each round,
   so postmortem inspection of a failed round becomes impossible after the
   next round runs.

The per-run scratch directory pattern fixes all three:
- Distinct `$TMPDIR` per round → no concurrent races.
- Postmortem retention on failure → debugging real problems.
- No cross-iteration contamination within multi-iteration loops.

This is the only behavioral change in Section 01 (everything else is
contracts and documentation). The `mktemp -d -t ori-tpr-XXXXXXXX` invocation
is the load-bearing primitive that Section 02's `dual-invoke.sh` will source
when constructing each round's environment.
