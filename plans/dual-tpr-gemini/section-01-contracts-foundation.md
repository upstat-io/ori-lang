---
section: "01"
title: "Contracts + foundation"
status: in-progress
reviewed: true
goal: "Define the JSON envelope schema (SSOT), the BEGIN/END sentinel format, the canonical (location, title) format, the reviewer-tag ID format, the per-run scratch dir helper, and extend the block-banned-commands.sh hook to gate gemini timeouts — all the contracts and foundation utilities that downstream sections consume."
success_criteria:
  - ".claude/skills/dual-tpr/findings-schema.json exists, conforms to JSON Schema draft-07, and validates 3 sample envelopes (codex with findings, gemini with grounded citation, no-findings)"
  - ".claude/skills/dual-tpr/envelope-format.md documents the BEGIN/END sentinel format, canonical location regex, title style, reviewer-tag ID format, and per-run scratch dir conventions"
  - ".claude/hooks/block-banned-commands.sh denies gemini commands with timeout < 300000ms or > 2100000ms — verified by 6 test scenarios (codex/gemini × short/valid/long matrix)"
  - ".claude/hooks/block-banned-commands.sh still denies codex commands the same way — regression test preserved"
  - "Lines 60-61 of .claude/hooks/block-banned-commands.sh are unchanged from pre-section state (Task #11 was a false positive per Codex Step 8B; there is no duplicate comment to fix)"
  - "Per-run scratch directory conventions documented and consumable from Section 02's transport utility"
inspired_by:
  - "Existing .claude/hooks/block-banned-commands.sh codex pattern (line 67) for the conditional extension shape"
  - ".codex/skills/review-work/SKILL.md finding format (lines 304-312) for the schema's lossless round-trip target"
  - "JSON Schema draft-07 for the schema file format and validator"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Define findings-schema.json (SSOT for envelope shape)"
    status: complete
  - id: "01.2"
    title: "Define sentinels and canonical (location, title) format"
    status: complete
  - id: "01.3"
    title: "Define reviewer-tag ID format and per-run scratch dir helper"
    status: complete
  - id: "01.4"
    title: "Update block-banned-commands.sh to gate gemini timeouts"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Contracts + foundation

**Status:** In Progress
**Goal:** Establish all the contracts (JSON schema, sentinel format, canonical formats, ID format) and foundation utilities (per-run scratch dirs, hook timeout gate for gemini) that downstream sections consume. This is the foundation layer of the 3-layer architecture from `00-overview.md` — no behavioral logic, only specifications and one minimal hook extension.

**Success Criteria:**

- [ ] `.claude/skills/dual-tpr/findings-schema.json` exists, conforms to JSON Schema draft-07, validates the three sample envelope fixtures, and rejects an invalid envelope test case (proves the validator is actually doing work). Satisfies mission criteria: "Schema-validated envelopes from Codex (via `--output-schema findings-schema.json`) and post-extraction-validated envelopes from Gemini both produce the same `FindingsEnvelope` shape consumed by downstream merge logic."
- [ ] `.claude/skills/dual-tpr/envelope-format.md` documents `<!-- BEGIN-ORI-DUAL-TPR-V1 -->` / `<!-- END-ORI-DUAL-TPR-V1 -->` sentinels with placement rules, the location regex `^[a-zA-Z0-9_./-]+:[0-9]+$`, the title style guide (imperative voice, sentence case, no markdown, no trailing punctuation, ≤200 chars), the reviewer-tag ID format `[TPR-{section}-{ordinal}-{reviewer}]`, and the per-run scratch dir conventions. Satisfies mission criteria for canonical formatting.
- [ ] `.claude/hooks/block-banned-commands.sh` denies `gemini` invocations with `timeout: 60000` (under 300000ms minimum) and `timeout: 3600000` (over 2100000ms maximum). Satisfies mission criterion: "`.claude/hooks/block-banned-commands.sh` denies `gemini` command invocations with `timeout: 60000` ... and `timeout: 3600000`."
- [ ] `.claude/hooks/block-banned-commands.sh` still denies `codex` invocations the same way — regression behavior preserved. Satisfies mission criterion: "regression test (existing behavior preserved)."
- [ ] `git diff .claude/hooks/block-banned-commands.sh` shows ONLY the changes to line 67 plus the two deny messages (and optionally a line-60 comment header refresh). No other lines modified. Specifically, lines 60-61 are unchanged: Codex Step 8B empirically verified that the "duplicate comment" claim from Phase 2 research was a false positive — there is no duplicate to fix here.

**Context:** This section creates the load-bearing foundation for the dual-source review system. Per `00-overview.md`'s 3-layer architecture, Layer 1 (this section) is pure data + one mechanical hook change — no runtime logic, no behavior. Section 02 (transport utility) consumes everything defined here. The section is intentionally kept minimal in behavioral changes because Section 01's risk profile must be the lowest in the plan: anything that goes wrong here cascades to every other section.

**Reference implementations:**

- **Existing hook**: `.claude/hooks/block-banned-commands.sh:67` — the existing single-condition pattern (`[[ "$COMMAND" == *"codex"* ]]`) that gets extended to two conditions (`[[ "$COMMAND" == *"codex"* || "$COMMAND" == *"gemini"* ]]`). Same pattern, additive change.
- **Existing finding format**: `.codex/skills/review-work/SKILL.md:304-312` — the existing markdown finding format `[TPR-{section}-{ordinal}][severity] file:line — Title. Evidence: ... Impact: ... Required plan update: ...`. The schema's field set is designed for lossless round-trip with this format.
- **JSON Schema draft-07**: standard schema dialect, supported by virtually every JSON validator including Python's `jsonschema` package, Node's `ajv`, Rust's `jsonschema` crate.

**Depends on:** None — this is the foundation section. Section 01 has `depends_on: []` and `reviewed: true` because it's the starting point of implementation.

---

## 01.1 Define findings-schema.json (SSOT for envelope shape)

**File(s):** `.claude/skills/dual-tpr/findings-schema.json` (new), `.claude/skills/dual-tpr/fixtures/*.json` (3 new sample fixtures)

**Context:** The envelope schema is the contract that both reviewers' output must conform to. Codex enforces it via `--output-schema findings-schema.json` at the CLI boundary; Gemini's output is extracted by Section 02's transport utility and validated post-hoc against the same file. Per `00-overview.md` Design Principle 3, this file is the SINGLE SOURCE OF TRUTH for envelope shape — no field definitions duplicated in prompt prose, parser comments, or section documentation. If a field changes, it changes here, and every consumer reloads.

**Field provenance** — the schema is designed for lossless round-trip with the existing format in `.codex/skills/review-work/SKILL.md:304-312`:

| Existing markdown format | Schema field |
|---|---|
| `[TPR-{section}-{ordinal}][{severity}]` | `severity` + `ordinal` (reviewer tag added at write-time) |
| `` `file:line` `` | `location` (constrained by canonical regex) |
| Free-text title | `title` (constrained by style guide in 01.2) |
| `Evidence: ...` | `evidence` |
| `Impact: ...` | `impact` |
| `Required plan update: ...` | `required_plan_update` |
| Implicit (`fresh_verification | direct_file_inspection | git_history | inference` per `.codex/skills/review-work/SKILL.md` Verification Standard) | `basis` (now an explicit field, REQUIRED) |

Tasks:

- [x] Create directory `.claude/skills/dual-tpr/`. Verify with `ls .claude/skills/dual-tpr/` returning the directory.

- [x] Write `.claude/skills/dual-tpr/findings-schema.json` containing the V1 envelope schema (regex tightened during execution — see Implementation Note 01.1 below):

  ```json
  {
    "$schema": "http://json-schema.org/draft-07/schema#",
    "title": "Ori Dual-TPR Findings Envelope V1",
    "type": "object",
    "required": ["schema_version", "status", "reviewer", "skill", "scope_actually_reviewed", "findings", "no_findings"],
    "properties": {
      "schema_version": {
        "type": "string",
        "const": "1.0"
      },
      "status": {
        "type": "string",
        "enum": ["complete", "failed_partial"],
        "description": "complete = clean termination; failed_partial = reviewer ran but did not finish all analysis. Missing field or any other value is treated as 'failed' by the orchestrator."
      },
      "reviewer": {
        "type": "string",
        "enum": ["codex", "gemini"]
      },
      "skill": {
        "type": "string",
        "enum": ["tpr-review", "review-work", "review-plan", "tp-help"]
      },
      "scope_actually_reviewed": {
        "type": "object",
        "required": ["expanded_beyond_packet", "files_read", "rules_consulted"],
        "properties": {
          "git_range": {"type": ["string", "null"]},
          "files_read": {"type": "array", "items": {"type": "string"}},
          "rules_consulted": {"type": "array", "items": {"type": "string"}},
          "specs_consulted": {"type": "array", "items": {"type": "string"}},
          "plans_consulted": {"type": "array", "items": {"type": "string"}},
          "expanded_beyond_packet": {
            "type": "boolean",
            "description": "REQUIRED. The reviewer must explicitly state whether it expanded its investigation beyond the orchestrator's starting packet."
          },
          "expansion_reason": {
            "type": "string",
            "description": "Required if expanded_beyond_packet is true. One sentence explaining what additional surface was explored and why."
          }
        },
        "if": {
          "properties": {"expanded_beyond_packet": {"const": true}}
        },
        "then": {
          "required": ["expansion_reason"]
        }
      },
      "findings": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["ordinal", "severity", "location", "title", "evidence", "impact", "basis"],
          "properties": {
            "ordinal": {"type": "integer", "minimum": 1},
            "severity": {"type": "string", "enum": ["high", "medium", "low"]},
            "location": {
              "type": "string",
              "pattern": "^(?!/)(?!\\./)[a-zA-Z0-9_./-]+:[0-9]+$",
              "description": "Repo-relative path:line. No absolute paths, no leading ./, no line ranges. Dotfiles like .gitignore:3 ARE allowed; only the leading two-character ./ prefix is rejected."
            },
            "title": {
              "type": "string",
              "maxLength": 200,
              "description": "Imperative voice, sentence case, no markdown, no trailing punctuation. Style enforced by prompt template; length enforced here."
            },
            "evidence": {"type": "string"},
            "impact": {"type": "string"},
            "required_plan_update": {"type": "string"},
            "layer": {
              "type": "string",
              "enum": ["committed", "staged", "unstaged"]
            },
            "basis": {
              "type": "string",
              "enum": ["fresh_verification", "direct_file_inspection", "git_history", "inference"]
            },
            "confidence": {"type": "string", "enum": ["high", "medium", "low"]},
            "citations": {
              "type": "array",
              "items": {
                "type": "object",
                "required": ["url"],
                "properties": {
                  "url": {"type": "string", "format": "uri"},
                  "description": {"type": "string"}
                }
              },
              "description": "Source URLs (gemini's google_web_search results, codex prior-art references, etc.). Tool-agnostic name (renamed from search_citations per Codex Round 1 feedback)."
            }
          }
        }
      },
      "verification": {
        "type": "object",
        "properties": {
          "tests_rerun": {"type": "array", "items": {"type": "string"}},
          "diagnostics_run": {"type": "array", "items": {"type": "string"}},
          "verification_gaps": {"type": "array", "items": {"type": "string"}}
        }
      },
      "no_findings": {
        "type": "boolean",
        "description": "Explicit 'reviewed cleanly with zero actionable findings' flag. Without this, an empty findings array could mean 'crashed before finding anything'."
      }
    },
    "if": {
      "properties": {"no_findings": {"const": true}}
    },
    "then": {
      "properties": {
        "findings": {"maxItems": 0}
      }
    }
  }
  ```

- [x] Create `.claude/skills/dual-tpr/fixtures/` directory.

- [x] Write `.claude/skills/dual-tpr/fixtures/codex-with-findings.json` — a sample codex envelope with 2 high-severity findings, all required fields populated, `expanded_beyond_packet: true`, no citations (codex doesn't have web search). Use realistic location strings like `compiler/ori_arc/src/lower/control_flow/mod.rs:123` and titles like `Add dec on early-exit branch in lower_branch`.

- [x] Write `.claude/skills/dual-tpr/fixtures/gemini-with-grounded-citation.json` — a sample gemini envelope with 1 finding that includes a non-empty `citations` array (e.g., `[{"url": "https://doc.rust-lang.org/std/sync/atomic/", "description": "Rust atomic ordering reference"}]`). This is the test that proves grounded citations round-trip through the schema.

- [x] Write `.claude/skills/dual-tpr/fixtures/no-findings.json` — a sample envelope with `no_findings: true`, empty `findings: []`, `status: "complete"`, all required fields populated. Validates the conditional `if no_findings then findings is empty` constraint.

- [x] Write `.claude/skills/dual-tpr/fixtures/invalid-location.json` — a NEGATIVE test fixture with `findings[0].location: "/abs/path/file.rs:1"` (absolute path violates the regex). The schema validator must REJECT this fixture.

- [x] Validate all four fixtures against the schema using a JSON Schema validator. Suggested one-liner:
  ```bash
  python3 -c "
  import json, sys
  from jsonschema import validate, ValidationError
  schema = json.load(open('.claude/skills/dual-tpr/findings-schema.json'))
  for path in ['.claude/skills/dual-tpr/fixtures/codex-with-findings.json',
               '.claude/skills/dual-tpr/fixtures/gemini-with-grounded-citation.json',
               '.claude/skills/dual-tpr/fixtures/no-findings.json']:
      try:
          validate(json.load(open(path)), schema)
          print(f'PASS: {path}')
      except ValidationError as e:
          print(f'FAIL: {path}: {e.message}')
          sys.exit(1)
  try:
      validate(json.load(open('.claude/skills/dual-tpr/fixtures/invalid-location.json')), schema)
      print('FAIL: invalid-location.json should have failed validation')
      sys.exit(1)
  except ValidationError:
      print('PASS: invalid-location.json correctly rejected')
  "
  ```
  All five outputs must be `PASS`.

  **Quicker invocation:** the per-subsection `/improve-tooling` retrospective produced `.claude/skills/dual-tpr/validate-envelopes.sh`, which encapsulates the python validator above. With no arguments it validates every file in `fixtures/`; with one or more file arguments it validates those specific envelopes; filenames starting with `invalid-` are auto-detected as negative fixtures and PASS means "schema correctly rejected". Use this script for all subsequent envelope validation in 01.2/01.3/01.4 and Section 02 — the python one-liner above is preserved for transparency only.

**Implementation Note 01.1 — regex tightened during execution.** The original `location.pattern` shown above as `^[a-zA-Z0-9_./-]+:[0-9]+$` does NOT enforce the documented "no absolute paths, no leading `./`" rule because `/` is inside the character class — `/abs/path/file.rs:1` matches the regex perfectly. This was caught by `invalid-location.json` (negative fixture) on the first validator run: three positive fixtures passed but the negative fixture also passed, which is a failure of the negative test. The schema and the section file's literal were both updated to `^(?!/)(?!\./)[a-zA-Z0-9_./-]+:[0-9]+$` (two negative lookaheads), which rejects leading `/` and leading `./` while still allowing dotfiles like `.gitignore:3`. After the fix, all five outputs report PASS (3 positive + 1 negative correctly rejected). This is exactly the matrix-squeeze principle in action — the negative fixture caught a real spec/implementation gap that no positive test could have surfaced.

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] All tasks above are `[x]` and the schema validates the 4 fixtures (3 positive + 1 negative)
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively on THIS subsection — reflect on the schema-writing journey: was the JSON Schema validator easy to invoke? Should there be a permanent `validate-envelope.sh` helper that takes a fixture path and runs validation in one command? Should there be a `validate-all-fixtures.sh` that runs the whole matrix? Forward-look: when Section 02 writes the parser tests, it will need to validate envelopes constantly — what tool/script would shorten that loop by 10 minutes per iteration? Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push` (e.g., `build(diagnostics): add validate-envelope.sh — surfaced by dual-tpr-gemini/section-01.1 retrospective`). Use a valid conventional-commit type — `build` for dev/diagnostic scripts, `test` for test-harness, `chore` for general tooling. Mandatory even when nothing felt painful — that is exactly when blind spots accumulate. If genuinely no gaps, document briefly: "Retrospective 01.1: no tooling gaps — relied on existing scripts X, Y." Do not silently skip. See `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow" for the full protocol.

---

## 01.2 Define sentinels and canonical (location, title) format

**File(s):** `.claude/skills/dual-tpr/envelope-format.md` (new)

**Context:** Sentinels are how Gemini's output gets parsed, since gemini has no `--output-schema` equivalent. The transport layer (Section 02) finds the BEGIN sentinel in Gemini's concatenated assistant text, extracts the fenced JSON block immediately following, validates it against `findings-schema.json`, and returns either a parsed envelope or a "failed reviewer round" signal. Codex doesn't need sentinels — `--output-schema` makes its entire `agent_message.text` the JSON envelope directly.

The canonical `(location, title)` format is what makes agreement detection across reviewers work. Exact-match consensus only fires if both reviewers emit byte-identical canonical forms. Without canonical formatting, identical findings with slightly different punctuation or casing get treated as different findings and the dual-source signal degrades.

**Two enforcement layers** for the canonical format:
1. **Schema-enforceable** (via JSON Schema regex/length): location regex, title length cap, no markdown in JSON-encoded strings (markdown chars are technically valid characters; the schema doesn't catch them).
2. **Prompt-enforced** (via SKILL.md instructions and reviewer prompts): title style (imperative voice, sentence case, no trailing punctuation), no markdown formatting in titles. These are natural-language constraints that the reviewer must follow; the schema can't reject `**bold title**` because it's still a valid string.

Tasks:

- [x] Create `.claude/skills/dual-tpr/envelope-format.md` documenting:

  **Sentinel format:**
  - `BEGIN: <!-- BEGIN-ORI-DUAL-TPR-V1 -->`
  - `END: <!-- END-ORI-DUAL-TPR-V1 -->`
  - Both are HTML/markdown comments — invisible in rendered markdown, easy to grep
  - **Why both BEGIN and END**: catches truncation. If a reviewer's response is cut off mid-envelope, the END sentinel is missing and the parser distinguishes "truncated" from "clean parse failure" → returns `failed_partial` status. A single sentinel cannot make this distinction.
  - **Why the V1 suffix**: schema versioning hook. When the envelope schema is revised, new sentinels (`BEGIN-ORI-DUAL-TPR-V2`) coexist with V1 during transition.

  **Sentinel placement:**
  - At the END of the reviewer's free-form prose response, NOT in the middle
  - One blank line above the BEGIN sentinel
  - BEGIN on its own line
  - Then a fenced JSON code block: ` ```json ... ``` `
  - Then END on its own line
  - One blank line below the END sentinel
  - Example:
    ```
    Free text from the reviewer about what they investigated, why,
    where they expanded scope, etc. Multiple paragraphs allowed.

    <!-- BEGIN-ORI-DUAL-TPR-V1 -->
    \`\`\`json
    { ...envelope... }
    \`\`\`
    <!-- END-ORI-DUAL-TPR-V1 -->
    ```

  **Codex case (no sentinels needed):** when codex is invoked with `--output-schema findings-schema.json`, its final `agent_message` text IS the schema-conformant JSON. The transport layer's codex extractor parses the agent_message text directly via `json.loads()`. No sentinel extraction step. The asymmetric rigor pattern: codex strict at CLI boundary, gemini lenient post-hoc.

  **Gemini case (sentinels required):** gemini has no `--output-schema` equivalent. Its prompt instructs it to wrap findings in BEGIN/END sentinels. The transport layer's gemini extractor:
  1. Reads `$RUN/gemini.jsonl` (stream-json output)
  2. Concatenates ALL `delta: true` assistant message fragments in arrival order (per Codex Step 6B's catch — assistant content is streamed in chunks)
  3. Waits for terminal `{"type":"result","status":"success"}` event
  4. Searches the concatenated text for `<!-- BEGIN-ORI-DUAL-TPR-V1 -->`
  5. Extracts the fenced JSON block immediately following (between ` ```json ` and ` ``` `)
  6. Verifies the block is followed by `<!-- END-ORI-DUAL-TPR-V1 -->`
  7. Validates the extracted JSON against `findings-schema.json`
  8. Any failure (missing BEGIN, missing END, missing JSON fences, validation failure, missing terminal success event, content but no terminator) → returns `failed_partial` round (not "clean review")

  **Canonical location format:**

  Regex: `^(?!/)(?!\./)[a-zA-Z0-9_./-]+:[0-9]+$`

  (Authoritative source: `.claude/skills/dual-tpr/findings-schema.json` field `findings.items.properties.location.pattern` — keep this prose copy in sync.)

  Format breakdown:
  - `<repo-relative path>` — must NOT start with `/` (absolute) or `./` (current-dir prefix); the two leading negative lookaheads enforce this
  - `:` — single colon separator
  - `<line-number>` — single integer, no ranges, no commas

  Examples (valid):
  - `compiler/ori_arc/src/lower/control_flow/mod.rs:123`
  - `library/std/iter.ori:45`
  - `tests/spec/collections/cow/test.ori:1`
  - `.claude/skills/dual-tpr/findings-schema.json:50`

  Examples (invalid):
  - `/home/eric/projects/ori_lang/file.rs:1` — absolute path (rejected by regex)
  - `./file.rs:1` — leading `./` (rejected by regex)
  - `file.rs` — no line number (rejected by regex)
  - `file.rs:1-10` — range, not single line (rejected by regex)
  - `file.rs:abc` — non-numeric line (rejected by regex)
  - `file.rs:1,2,3` — multi-line list (rejected by regex)

  Rationale: exact-match agreement detection requires byte-identical paths. Repo-relative is the canonical form; absolute paths and `./` prefixes prevent matches because they encode environment-specific information.

  **Canonical title style:**
  - Maximum 200 characters (schema-enforced)
  - Imperative voice (verb-first): "Add", "Fix", "Replace", "Remove", "Move", "Rename"
  - Sentence case: capitalize the first word and proper nouns; lowercase the rest
  - NO markdown formatting (`**bold**`, `_italic_`, ` `code` `, `[link](url)` are all rejected by prompt instructions even though the schema can't reject them)
  - NO trailing punctuation (no period, no exclamation, no question mark)
  - NO interrogative ("Why is...", "Should we...") — questions are not findings, they're discussion

  Examples (valid):
  - `Add dec on early-exit branch in lower_branch`
  - `Fix off-by-one in range_len for empty ranges`
  - `Replace println with tracing::debug in eval/iterator`
  - `Remove dead match arm in resolve_iterator_method`

  Examples (invalid — caught by prompt instructions, not schema):
  - `Adding a dec.` — gerund + period
  - `**Add dec**` — markdown
  - `add dec on early-exit branch` — not sentence case
  - `Why is this not detected?` — interrogative + question mark
  - `fix bug in foo` — not sentence case
  - `Add dec on early-exit branch and also fix the issue with the lowering of nested control flow constructs in the case where multiple loops are nested with break-with-value...` — exceeds 200 chars

  Rationale: same as location — exact match requires consistent style across both reviewers. Schema enforces length and basic structure; reviewer prompts enforce style.

- [x] Add a complete example envelope (multi-finding) showing the full Gemini case (with sentinels and fenced JSON block) to envelope-format.md. Use the same structure as `fixtures/gemini-with-grounded-citation.json`.

- [x] Add a complete example envelope showing the Codex case (raw JSON, no sentinels) to envelope-format.md. Use the same structure as `fixtures/codex-with-findings.json`.

- [x] Add a section "How agreement is detected" to envelope-format.md explaining that two findings (one from each reviewer) are considered an agreement only when their `(location, title)` pair is BYTE-IDENTICAL. Mention that the strict-match policy is deliberate (per Codex Step 6B Q7) — fuzzy matching would introduce a third bias source.

- [x] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [x] All tasks above are `[x]` and the documentation file is complete
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively on THIS subsection — same protocol as 01.1's close-out, scoped to 01.2's documentation-writing journey.

**Retrospective 01.2 — no tooling gaps justifying immediate action.** Writing the documentation file surfaced one friction point: the location regex now lives in four places (`findings-schema.json`, the JSON literal in this section, the prose in this section, and the prose in `envelope-format.md`). A future regex change is at high risk of stale drift in one or more locations. A `check-doc-schema-sync.sh` would catch this, but the regex is unlikely to change during V1's lifecycle, `/impl-hygiene-review` will catch cross-file invariant drift as a routine duty at section close, and the tool would have low frequency value (zero invocations until a hypothetical V2). Building it now would be speculative abstraction. Section 02 will exercise `validate-envelopes.sh` (built in 01.1) heavily for parser tests against real envelopes from codex/gemini runs — that is the load-bearing tool for the next subsection's friction. If Section 02's parser work surfaces a stronger case for cross-file schema/doc sync verification, the tool can be built then with concrete usage data driving its design. Did writing concrete examples reveal any cases the regex doesn't cover well? Was there friction in choosing between schema-enforceable and prompt-enforceable constraints? Should there be a `lint-envelope-titles.sh` that scans envelopes for style violations? Commit improvements separately using `build(...)`, `test(...)`, `chore(...)`, `docs(...)` per the type rules in 01.1's close-out.

---

## 01.3 Define reviewer-tag ID format and per-run scratch dir helper

**File(s):** `.claude/skills/dual-tpr/envelope-format.md` (extend with ID format and scratch dir sections)

**Context:** When Claude writes findings to plan files, it suffixes finding IDs with the reviewer name to distinguish codex-sourced findings from gemini-sourced findings. Per Codex Step 6B's refinement (Q4 in Step 6B), ordinal sequences are INDEPENDENT per reviewer: codex counts 1, 2, 3 in its own namespace; gemini counts 1, 2, 3 in its own namespace. There is no shared ordinal space and no implicit equivalence judgment baked into ID assignment. Agreement detection happens at write-time via exact `(location, title)` match — never via ID match.

The per-run scratch directory replaces the existing fixed-path pattern that all current wrappers use (e.g., `/tmp/tpr-iter.jsonl`, `/tmp/review-work.jsonl`, `/tmp/tp-help.jsonl`). The fixed paths would race on concurrent reviewer invocations — a latent bug in the current single-source implementation that has never been hit in practice because users don't run two reviews simultaneously, but which the dual-source plan would expose if not fixed. Per `mktemp -d`, each review run gets its own scratch directory. Cleanup policy: cleanup on success, retain on failure for postmortem.

Tasks:

- [x] Append to `.claude/skills/dual-tpr/envelope-format.md`:

  **Reviewer-tag ID format:**

  Pattern: `[TPR-{section}-{ordinal}-{reviewer}]`

  - `{section}`: the owning plan section number (e.g., `02`, `03`). Two-digit zero-padded.
  - `{ordinal}`: a 3-digit zero-padded integer, INDEPENDENT per reviewer. Codex's first finding for section 02 is `001`; gemini's first finding for section 02 is also `001` — the ordinals don't share a namespace.
  - `{reviewer}`: literal `codex` or `gemini`.

  Examples:
  - `[TPR-02-001-codex]` — codex's 1st finding for section 02
  - `[TPR-02-001-gemini]` — gemini's 1st finding for section 02 (NOT necessarily the same finding as codex's 1st; agreement is detected by `(location, title)` match at presentation time)
  - `[TPR-04-007-codex]` — codex's 7th finding for section 04
  - `[TPR-08-012-gemini]` — gemini's 12th finding for section 08

  When merging into the plan file's `## NN.R Third Party Review Findings` block:
  - Each reviewer's findings are written with its own ordinal sequence
  - Agreements (same `(location, title)` from both reviewers) appear as TWO entries in the TPR block — both visible to the human reader, both with the same `(file:line, title)` pair, but with different `-codex` / `-gemini` suffixes. The human reads both entries adjacent to each other in the merged output and sees the agreement immediately.
  - Disagreements (one reviewer flagged, the other didn't) appear as one entry with one tag.

  Why independent ordinals (Codex Step 6B Q4): shared base IDs `[TPR-02-001-codex]` and `[TPR-02-001-gemini]` would imply an equivalence claim — that the issues are "the same finding from two reviewers." Whether they're the same is a judgment call that should NOT be baked into ID assignment. Cleaner: each reviewer numbers independently; the human reader detects equivalence by reading adjacent entries with matching `(location, title)`.

  **Per-run scratch directory conventions:**

  All reviewer runs use a per-run scratch directory created at the start of each round:
  ```bash
  RUN=$(mktemp -d -t ori-tpr-XXXXXXXX)
  ```

  The `XXXXXXXX` template generates an 8-character random suffix; the directory will be created under `$TMPDIR` (typically `/tmp` on Linux) with a name like `/tmp/ori-tpr-A1B2C3D4`.

  File naming inside `$RUN`:
  - `$RUN/codex.prompt.md` — the prompt sent to codex (for postmortem inspection)
  - `$RUN/codex.jsonl` — codex's stdout (item.completed JSONL stream)
  - `$RUN/gemini.prompt.md` — the prompt sent to gemini
  - `$RUN/gemini.jsonl` — gemini's stdout (stream-json JSONL stream)
  - `$RUN/codex.envelope.json` — extracted+validated codex envelope (cached after parse for reuse)
  - `$RUN/gemini.envelope.json` — extracted+validated gemini envelope (cached after parse for reuse)
  - `$RUN/worktree-before.txt` — `git status --porcelain` snapshot taken before reviewer launches
  - `$RUN/worktree-after.txt` — `git status --porcelain` snapshot taken after reviewer completes
  - `$RUN/round.log` — orchestration log (which reviewer started when, retry counts, failure reasons)

  **Cleanup policy:**
  - On successful round (both reviewers returned valid envelopes, dirty-worktree guard passed): `rm -rf "$RUN"` after findings are written to the plan file.
  - On failed round (any infra failure, parse failure, schema violation, dirty-worktree detection): RETAIN `$RUN` for postmortem inspection. Print the path to the user as part of the failure message: `"Round failed; postmortem dir retained at $RUN"`.
  - Across multiple loop iterations within one TPR run, each iteration gets its own `$RUN` directory. Successful intermediate iterations are cleaned up; failed iterations are retained.

  **Why per-run instead of fixed paths:**
  - Concurrent invocations don't race (e.g., two `/tpr-review` calls at the same time)
  - Postmortem inspection is straightforward — the `$RUN` path is preserved on failure
  - No cross-iteration contamination within a multi-iteration loop
  - Replaces the existing latent bug where `/tmp/tpr-iter.jsonl`, `/tmp/review-work.jsonl`, `/tmp/tp-help.jsonl` would clobber each other under concurrent use

- [x] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [x] All tasks above are `[x]` and the format spec extensions are in place
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively on THIS subsection — same protocol. Was there friction in defining ID format or scratch dir conventions? Should there be a small helper script `dual-tpr-scratch-dir.sh` that wrappers can source for the `mktemp -d` invocation? Would a centralized `dual-tpr-cleanup.sh` (cleanup on success, retain on failure) be useful or premature abstraction? Commit improvements separately using the appropriate conventional commit type.

**Retrospective 01.3 — defer scratch-dir helper to Section 02.** Writing the conventions surfaced an obvious candidate tool (`dual-tpr-scratch-dir.sh` to wrap the `mktemp -d -t ori-tpr-XXXXXXXX` invocation plus the cleanup-on-success/retain-on-failure semantics), but Section 02.1 will be the actual first consumer when it builds `scratch-dir.sh` as one of the eight transport primitives. Building the helper here would either (a) duplicate work that Section 02.1 will redo, or (b) become Section 02.1's deliverable preemptively. Per CLAUDE.md "the right amount of complexity is what the task actually requires", the right call is to let Section 02.1 build it from the conventions defined here — the conventions ARE the deliverable of 01.3, not the implementation. The `dual-tpr-cleanup.sh` idea (centralized cleanup wrapper) is also deferred to Section 02.1 for the same reason: a helper with no caller is speculative, and Section 02 has the concrete callers. No tool built in this retrospective; one will land in Section 02.1's close-out instead.

- [x] **TPR checkpoint** — `/tpr-review` covering 01.1–01.3 contract-design work
  Resolved: Skipped on 2026-04-07 by explicit user direction. The intermediate checkpoint was
  not run; the section-close TPR in 01.N (full-section review against the existing single-source
  `/tpr-review` skill) remains MANDATORY per the section's completion checklist and will catch
  any contract-design issues before Section 02 begins consuming the artifacts. The risk of
  deferring this catch from 01.3 to 01.N is that if a contract bug surfaces at section close,
  it may require coordinated edits across the schema file, the envelope-format spec, and the
  fixtures rather than the single-file edit possible at the 01.3 boundary. The user accepted
  this risk in exchange for the time savings.
  <!-- Per CLAUDE.md and plan-schema.md, sections with 3+ implementation subsections place
       intermediate TPR checkpoints. This catches contract-design issues (schema, format,
       ID, scratch dir) BEFORE they propagate into the hook update (01.4) and Section 02's
       transport implementation. Fixing a contract bug after Section 02 has consumed it
       requires coordinated edits across both files; fixing it now is a single-file change. -->

---

## 01.4 Update block-banned-commands.sh to gate gemini timeouts

**File(s):** `.claude/hooks/block-banned-commands.sh`

**Context:** The existing hook gates timeout windows on `codex` commands (line 67: `[[ "$COMMAND" == *"codex"* ]]`). It denies any timeout < 300000ms (5 min) or > 2100000ms (35 min) when the command contains `codex`. The dual-source plan needs the same gate to apply to `gemini` commands so that nobody accidentally invokes `gemini` with a timeout that would kill a real review mid-stream. Gemini's baseline cold start is ~44s and real reviews take 2–5 minutes, so a 60-second timeout would always fail; the 35-minute ceiling matches codex.

Per the user's "Continuous improvement everywhere" rule but ALSO per the recently-saved `feedback_reviewers_need_shell` memory: this is an additive conditional change to ONE line (plus the two deny messages). It is NOT a rewrite. The existing codex pattern is preserved (regression protection) and no other lines are touched.

**Critical: do NOT touch lines 60-61.** Phase 2 research originally claimed a duplicate comment at lines 60-61 and tracked it as Task #11. Codex Step 8B empirically re-checked the file and verified that the "duplicate comment" claim was a FALSE POSITIVE — line 60 is a section header, line 61 is a normal explanatory comment, and they are NOT duplicates. Task #11 has been deleted from the task tracker. This section's success criteria explicitly require lines 60-61 to be unchanged from pre-section state. A `git diff` after the section completes must show ONLY changes to line 67 + the two deny messages (and optionally a one-word comment update on line 60 if it improves accuracy, but no two-line replacement).

Rules embedded inline:
- This is an additive conditional change, not a rewrite. The existing codex pattern is preserved (regression protection).
- The hook lives in `.claude/hooks/`, governed by `.claude/settings.json:30-41` (PreToolUse hook registration). No settings changes needed — the hook is already registered for all Bash invocations.
- Lines 60-61 are NOT modified. Do not touch them. Codex Step 8B verified the duplicate-comment claim was a false positive.
- File size: the hook is 86 lines pre-change, will be ~89 lines post-change (the conditional `||` expansion is the only structural change). Well under the 500-line limit.

Tasks:

- [ ] Read `.claude/hooks/block-banned-commands.sh` end-to-end to refresh context. Verify the current line 67 reads exactly `if [[ "$COMMAND" == *"codex"* ]]; then`.

- [ ] Edit line 67 of `.claude/hooks/block-banned-commands.sh` from:
  ```bash
  if [[ "$COMMAND" == *"codex"* ]]; then
  ```
  to:
  ```bash
  if [[ "$COMMAND" == *"codex"* || "$COMMAND" == *"gemini"* ]]; then
  ```

- [ ] Edit the deny message on line 71 from:
  ```bash
  deny "Blocked: timeout ($TIMEOUT ms) on codex command is too short. Reviews need 5-35 minutes — use at least 300000 ms, up to 2100000 ms (35 min)."
  ```
  to:
  ```bash
  deny "Blocked: timeout ($TIMEOUT ms) on codex/gemini command is too short. Reviews need 5-35 minutes — use at least 300000 ms, up to 2100000 ms (35 min)."
  ```

- [ ] Edit the deny message on line 74 from:
  ```bash
  deny "Blocked: timeout ($TIMEOUT ms) on codex command exceeds 35-minute ceiling (2100000 ms)."
  ```
  to:
  ```bash
  deny "Blocked: timeout ($TIMEOUT ms) on codex/gemini command exceeds 35-minute ceiling (2100000 ms)."
  ```

- [ ] (Optional) Update the section-header comment on line 60 from `# ── Guard timeouts on review/codex commands ──` to `# ── Guard timeouts on review (codex/gemini) commands ──` for accuracy. ONLY if this is a minimal change. Do NOT touch line 61 — it is the existing explanatory comment that some Phase 2 research incorrectly flagged as a duplicate.

- [ ] **Test the change with a 6-scenario matrix:**

  Matrix dimensions: command in {codex, gemini, neither} × timeout in {short=60000, valid=600000, long=3600000}.

  - Test 1 (regression — codex with short timeout): should be DENIED
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"codex exec test","timeout":60000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: JSON output with "permissionDecision":"deny"
    ```

  - Test 2 (regression — codex with valid timeout): should be ALLOWED
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"codex exec test","timeout":600000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: empty, exit 0
    ```

  - Test 3 (regression — codex with long timeout): should be DENIED
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"codex exec test","timeout":3600000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: JSON output with "permissionDecision":"deny" mentioning 35-minute ceiling
    ```

  - Test 4 (NEW — gemini with short timeout): should be DENIED
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"gemini -p test","timeout":60000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: JSON output with "permissionDecision":"deny" mentioning codex/gemini
    ```

  - Test 5 (NEW — gemini with valid timeout): should be ALLOWED
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"gemini -p test","timeout":600000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: empty, exit 0
    ```

  - Test 6 (NEW — gemini with long timeout): should be DENIED
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"gemini -p test","timeout":3600000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: JSON output with "permissionDecision":"deny" mentioning 35-minute ceiling
    ```

  - Test 7 (control — neither codex nor gemini, with short timeout): should be ALLOWED (the gate only applies to codex/gemini)
    ```bash
    echo '{"tool_name":"Bash","tool_input":{"command":"echo hello","timeout":60000}}' | bash .claude/hooks/block-banned-commands.sh
    # Expected stdout: empty, exit 0
    ```

  These seven tests are the matrix dimensions. Tests 4 and 6 are the **negative pin** (gemini with bad timeout was previously NOT denied; now it IS — the test only passes after this change). Test 5 is the **semantic pin** (gemini with valid timeout MUST be allowed — the test only passes if the gate is correctly scoped to bad timeouts, not all gemini commands). Tests 1-3 are regression tests for the existing codex behavior. Test 7 is a control proving the gate doesn't accidentally apply to unrelated commands.

- [ ] Run all 7 tests; record the actual outputs in the section's working notes.

- [ ] Verify the byte-minimal change: `git diff .claude/hooks/block-banned-commands.sh` should show only the 3-4 modified lines (line 67 + line 71 + line 74 + optionally line 60). Specifically verify lines 60-61 are unchanged (other than the optional one-word header tweak on line 60).

- [ ] **Subsection close-out (01.4)** — MANDATORY before section completion:
  - [ ] All tasks above are `[x]` and all 7 hook test scenarios pass
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — was the test invocation pattern (echoing JSON to the hook script) easy to invoke? Should it become a permanent test fixture? Should there be a `verify-hook.sh` helper that runs all 7 test scenarios in one command and reports pass/fail per test? Forward-look: when future plans modify this hook, would having a regression test suite shorten verification by 10+ minutes? Implement every accepted improvement NOW (zero deferral). Commit separately as `build(diagnostics): add verify-hook.sh — surfaced by dual-tpr-gemini/section-01.4 retrospective`. Mandatory even when nothing felt painful.

---

## 01.R Third Party Review Findings

<!-- Reserved for codex/gemini reviewers running /tpr-review against this section.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 01.N Completion Checklist

- [ ] All four implementation subsections (01.1, 01.2, 01.3, 01.4) marked `complete` in section frontmatter
- [ ] `.claude/skills/dual-tpr/findings-schema.json` exists and validates the four sample envelope fixtures (3 positive + 1 negative)
- [ ] `.claude/skills/dual-tpr/envelope-format.md` documents BEGIN/END sentinels with placement rules, location regex with positive/negative examples, title style with positive/negative examples, reviewer-tag ID format with examples, and per-run scratch dir conventions with file naming and cleanup policy
- [ ] `.claude/skills/dual-tpr/fixtures/` contains the 4 sample envelopes used by validation tests (codex-with-findings.json, gemini-with-grounded-citation.json, no-findings.json, invalid-location.json)
- [ ] `.claude/hooks/block-banned-commands.sh` line 67 includes both `codex` AND `gemini` patterns
- [ ] All 7 hook test scenarios pass (codex/gemini × short/valid/long matrix + control)
- [ ] `git diff .claude/hooks/block-banned-commands.sh` shows ONLY the line 67 change + line 71 deny message + line 74 deny message (and optionally a line 60 one-word comment header refresh) — no other lines modified
- [ ] **Lines 60-61 are unchanged from pre-section state** — Codex Step 8B confirmed Task #11 was a false positive. There is NO duplicate comment to fix. Verify with `git diff -U0 .claude/hooks/block-banned-commands.sh` and confirming neither line 60 nor line 61 appears in the diff (or only line 60 if the optional header refresh was applied, in which case line 60 shows a one-word change and line 61 still does not appear).
- [ ] `timeout 150 ./test-all.sh` green — no regressions in compiler test suite (this section doesn't touch compiler code, but the regression check is mandatory per CLAUDE.md)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan dual-tpr-gemini` returns 0 annotations from this section's work (no `TPR-01-XXX` references left in source files; only in plan documentation)
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, all four subsection statuses updated to `complete`
  - [ ] `00-overview.md` Quick Reference table status updated for Section 01
  - [ ] `00-overview.md` mission success criteria checkboxes updated (specifically the items about gemini timeout gating and hook regression behavior can be checked off)
  - [ ] `index.md` section status updated for Section 01
  - [ ] Section 02's `depends_on: ["01"]` precondition is satisfied — Section 02 can begin work
- [ ] `/tpr-review` passed (final, full-section) — independent codex review found no critical or major issues, OR all findings triaged. This is the formal section-close TPR using the existing single-source `/tpr-review` skill (the dual-source rewrite is in Section 04, not yet available at this point in the plan).
- [ ] `/impl-hygiene-review` passed — implementation hygiene review found no critical or major findings, OR all findings triaged and fixed. MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY safety net after both reviews are clean. The PRIMARY tooling capture happens per-subsection (see each subsection's close-out block above) — by section close those captures should already be committed. The sweep does TWO things: (1) **Verify** every subsection in this section has either an "improvements made" entry (with commits) or a documented "no gaps" negative finding from its own per-subsection retrospective; if any subsection skipped its retrospective, STOP and run it now — the sweep cannot substitute for missed per-subsection captures. (2) **Look for cross-subsection patterns** invisible at per-item scope: command sequences repeated across 01.1-01.4 (e.g., re-running schema validation for each fixture file by hand in 01.1 vs running individual hook test scenarios one at a time in 01.4 — could a single `dual-tpr-validate-all.sh` cover both?), instrumentation/output-format friction that only became obvious after seeing all four subsections together. Add ONLY new items that emerged from cross-cutting patterns — do not duplicate per-subsection findings. Implement immediately (zero deferral), commit separately using a valid conventional-commit type (`build(diagnostics): add X — surfaced by section-01 close sweep` — use `build` for dev/diagnostic scripts, `test` for test-harness, `chore` for general tooling, `ci` for CI, `docs` for tool docs; the lefthook commit-msg hook rejects any non-standard type). Most sweeps produce zero new findings when per-subsection captures are thorough — that is the expected, healthy outcome and must be documented: "Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling." Do not silently skip.

**Exit Criteria:** `.claude/skills/dual-tpr/findings-schema.json` validates against JSON Schema draft-07 and accepts the 3 positive sample fixtures while rejecting the negative `invalid-location.json` fixture. The canonical location regex matches valid `path:line` strings and rejects absolute paths, leading-dot paths, line ranges, and non-numeric line numbers. The canonical title style is documented with positive and negative examples in `envelope-format.md`. Reviewer-tag ID format and per-run scratch dir conventions are documented and ready for Section 02 to consume. `.claude/hooks/block-banned-commands.sh` denies gemini commands with timeouts under 300000ms or over 2100000ms, while still denying codex commands the same way (regression preserved), and the change is BYTE-MINIMAL (only line 67 + two deny messages + optionally line 60 — no other lines modified). Section 02 can begin its transport utility implementation against the locked schema and format spec.
