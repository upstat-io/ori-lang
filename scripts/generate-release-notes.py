#!/usr/bin/env python3
"""Generate release notes for Ori releases.

Gathers commit log and PR descriptions, then generates structured release
notes using AI (Copilot SDK with Claude Sonnet 4.6) when available, falling
back to conventional-commit categorization.

Usage:
    # From git tag range (CI or local):
    ./scripts/generate-release-notes.py --tag v2026.03.23.1-Alpha

    # With explicit previous tag:
    ./scripts/generate-release-notes.py --tag v2026.03.23.1-Alpha --prev v2026.03.22.1-Alpha

    # Output to file instead of stdout:
    ./scripts/generate-release-notes.py --tag v2026.03.23.1-Alpha -o /tmp/notes.md

Environment:
    COPILOT_GITHUB_TOKEN  — enables AI generation (CI only)
    GH_TOKEN / GITHUB_TOKEN — needed for PR body fetching via `gh`
"""

import argparse
import os
import subprocess
import sys


def run(cmd, check=True):
    """Run a shell command and return stdout."""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if check and result.returncode != 0:
        return ""
    return result.stdout.strip()


def get_prev_tag(current_tag):
    """Find the most recent tag before current_tag."""
    tags = run("git tag --sort=-creatordate | grep '^v'")
    if not tags:
        return ""
    for tag in tags.split("\n"):
        tag = tag.strip()
        if tag and tag != current_tag:
            return tag
    return ""


def gather_commit_log(prev_tag, current_tag):
    """Get commit log between two tags."""
    if prev_tag:
        return run(f'git log "{prev_tag}..HEAD" --pretty=format:"- %s (%h)" --no-merges')
    return run('git log --pretty=format:"- %s (%h)" --no-merges -20')


def gather_pr_bodies(prev_tag):
    """Fetch merged PR descriptions via gh CLI."""
    if prev_tag:
        prev_date = run(f'git log -1 --format=%aI "{prev_tag}"')
        if prev_date:
            return run(
                f'gh pr list --state merged --base master --limit 20 '
                f'--json number,title,body,mergedAt '
                f'--jq \'[.[] | select(.mergedAt >= "{prev_date}")] | .[] | '
                f'"## PR #\\(.number): \\(.title)\\n\\(.body // "(no description)")\\n"\''
            )
    return run(
        'gh pr list --state merged --base master --limit 5 '
        '--json number,title,body '
        '--jq \'.[] | "## PR #\\(.number): \\(.title)\\n\\(.body // "(no description)")\\n"\''
    )


def generate_ai_notes(tag, prev_tag, commit_log, pr_bodies):
    """Try AI generation via Copilot SDK. Returns notes or None on failure."""
    if not os.environ.get("COPILOT_GITHUB_TOKEN"):
        return None

    prompt = f"""You are writing release notes for **Ori** ({tag}), an alpha-stage statically-typed, expression-based programming language with HM type inference, ARC memory management, and capability-based effects. The audience is developers and language enthusiasts following an open-source compiler project.

Write clear, curated release notes in markdown. Do NOT wrap in ```markdown fences.

## CRITICAL OUTPUT CONTRACT — READ FIRST

Your entire response becomes the release notes verbatim. There is no post-processing step that will "clean up" your output. Therefore:

- **DO NOT** begin with meta-commentary, preamble, or "thinking aloud" text. Banned openings include (but are not limited to): "Now I have enough context...", "Here are the release notes...", "Here is the...", "Let me write...", "I'll summarize...", "Based on the...", "Looking at the...", "After reviewing...", "Okay, I can see...", "Sure,...", "Got it,...".
- **DO NOT** emit a bare horizontal rule (`---`) before the first heading. The release body must begin with content, not a separator.
- **DO NOT** restate the task, describe your approach, or explain what you're about to do.
- **DO NOT** add any trailing commentary after the last section ("I hope this helps", "Let me know if...").
- **The very first character** of your response must be either `#` (a heading) or the first letter of the summary paragraph. Nothing else.
- If you have no information to summarize, still output valid release notes — do NOT narrate that fact.

Violating this contract ships broken release notes to real users on GitHub. There is no second chance.

## Analysis Process — Be Thorough, Not Hurried

Before writing a single bullet, do the following analysis in full. Take the time it takes — release notes are a permanent public record, and rushing the analysis ships an incomplete log.

1. Read every PR description in full. For each PR, identify the distinct user-facing changes it delivers AND the supporting work (refactors, hardening, internal cleanup) it absorbs into those changes.
2. Walk the commit log in full. For every commit that does not trace back to an already-categorized PR, classify whether it represents a distinct user-visible change. If unclear, lean toward including it — surfacing a real change as a small bullet beats dropping it.
3. Cross-reference PRs and commits. When a PR mentions a feature, confirm the commits exist; when a commit subject hints at a feature, confirm the PR explains it. Discrepancies often surface work the PR forgot to document.
4. Enumerate every distinct feature, behavior change, bug fix, performance improvement, diagnostic refinement, stdlib addition, and tooling change.
5. For each change, classify into a Format section below.
6. Within each section, sequence bullets by user impact — most impactful first, smaller refinements after.
7. Translate every internal identifier (bug IDs, plan refs, phase names, review-cycle codes) into user-facing language per the Anonymization section before writing the bullet.

Do not skim. Do not stop early. If the input is large, the analysis is large.

## Format

Start with a 1-3 sentence summary describing the theme of this release.

Then group changes into sections (omit empty ones):
- **Language** — syntax or semantics changes visible to Ori programmers
- **Compiler** — type checker, codegen, diagnostics, performance improvements (frame through user impact: "type errors now point to the correct location", not "refactored inference engine")
- **Standard Library** — new or changed functions, types, traits in the stdlib
- **Tooling** — CLI, formatter, LSP, test runner changes
- **Bug Fixes** — corrected behavior that was broken in a previous release
- **Breaking Changes** — anything that changes existing behavior (with migration guidance)

End with a brief **What's Next** section — 2-3 bullet points on near-term focus areas, derived from the commit log and PR descriptions.

For each bullet:
- **Bold title** followed by 1-2 sentences explaining what changed and why it matters to users
- Use past tense ("Added", "Fixed", "Improved")
- Frame everything through user impact — if a change has no user-visible effect, omit it entirely
- For new syntax, include a tiny inline code example showing the user-facing shape
- For breaking changes, include a brief migration note (before -> after)
- For performance changes with data, include the numbers ("2.3x faster", "95 to 128 MiB/s")
- For diagnostic improvements, briefly contrast the before/after experience

## Core Principle: Deliverables, Not Process

Release notes describe what was DELIVERED, not the process of delivery. Think of it like a code review: when a PR merges, the release notes say "Added feature X" — not "Added feature X, then fixed 8 things the reviewer caught." Review feedback is part of delivering X correctly.

Apply this to process artifacts:
- **TPR (Third-Party Review) findings** — fold into the feature they harden. "Conditional compilation now works on all item types, with proper validation and no incremental parse leakage" — NOT 8 separate TPR bullets. Only surface a TPR as a standalone Bug Fix if it reveals broken behavior that existed in a PREVIOUS release and affected users.
- **Iterative refinements** — hardening, edge case fixes, and polish done during development are part of the feature, not separate items.

**The test**: would an outside contributor who has never seen our plans, TPR system, or internal tracking understand every bullet? If not, rewrite it.

## Anonymization — Strip All Internal Identifiers

Release notes are public-facing. The following internal identifiers MUST NOT appear anywhere in the output — not in headings, not in bullets, not in the summary, not in the "What's Next" section:

- **Bug tracker IDs of any shape**: `BUG-XX-NNN`, `BUG-04-077`, `BUG-07-100`, internal tracker codes, etc.
- **Plan directory names**: `plans/<feature>/`, `bug-tracker/plans/BUG-XX-NNN/`, any path under `plans/`
- **Plan section and subsection references**: `§02.1`, `§04A`, `Section 5`, `Subsection 04.2`, `section-NN-<name>.md`
- **Phase references**: `Phase A`, `Phase 1.5`, `Phase 4`, `Phase 9`
- **Review cycle IDs**: `TPR-01-062`, `Round 3`, `Cycle 2`, `Iteration N`
- **Internal SSOT or rule citations**: `routing.md §1`, `impl-hygiene.md §SSOT`, any reference to `.claude/rules/`
- **Internal proposal codes**: `<name>-proposal.md` paths, internal proposal IDs
- **Sprint, milestone, or iteration codes**

**Refer to bugs by what they did to users — by name, by behavior — never by ID.**
Wrong: "Fixed BUG-04-077"
Wrong: "Fixed the issue tracked in BUG-04-077"
Right: "Fixed a crash when destructuring a map with a missing key"

**Refer to features by their user-facing name or title — never by plan, proposal, or section code.**
Wrong: "Implemented byte-literal-proposal §3"
Wrong: "Completed Phase 4 of repr-opt §02"
Right: "Added byte literal syntax (`b'x'`)"
Right: "Layout decisions now reuse a single computed plan across the codegen pipeline"

If a commit message or PR description contains internal tracking codes, translate them into user-facing language for the bullet. Internal codes are scaffolding for the development process — they have no business appearing in a public release announcement.

## Length and Completeness — No Truncation

If the analysis surfaces 40 distinct user-visible changes, ship 40 bullets. If it surfaces 100, ship 100. If it surfaces 12, ship 12. The list length is determined by the work delivered, not by an editorial budget. There is no "too long" — readers scan headings, skim sections, and read the bullets they care about. A complete release log serves both the developer celebrating shipped work and the researcher tracking language evolution.

The ONLY consolidation permitted is folding **non-user-facing process artifacts** (TPR review cycles, internal refactor commits, lint fixes done during a feature implementation, plan-section housekeeping) into the feature they harden. A distinct feature plus its TPR hardening is ONE bullet describing the delivered feature. Two distinct features are TWO bullets, even when they shipped in adjacent commits or touched the same file.

**Banned shortcuts** — these phrases admit defeat on the analysis and ship a worse release log:
- "Many small bug fixes" — name each fix and what it broke
- "Various improvements to X" — describe each improvement
- "Multiple performance optimizations" — list each one with its impact
- "Several diagnostic improvements" — describe each diagnostic that improved
- "Additional polish and stability work" — describe each piece of polish
- "Cleaned up internal X" — if it has no user impact, omit; if it does, describe the impact
- "And more" / "among other changes" — name the others or remove the phrase
- Any ellipsis indicating omitted items

If two distinct features happen to land in the same area, they get two bullets. If the same feature lands across 8 commits with iterative refinement, it gets one bullet that describes the delivered feature.

## Rules
- The PR descriptions are your PRIMARY source — they contain human-written summaries of what changed and why
- The commit log is supplementary — use it to catch anything the PRs missed
- Every distinct user-visible change gets its own bullet — do NOT aggregate distinct work to shorten the list
- Every entry must be written for humans — no raw git log dumps, no commit hashes, no PR numbers in the body
- Never say "Internal improvements and maintenance" — if it matters, describe the impact; if it doesn't, omit it
- Skip version-bump commits and nightly automation PRs entirely
- Do not reproduce test plan checklists
- If a change has no user-visible effect (pure refactoring, internal code movement), omit it entirely

## Input

Pull request descriptions (primary source):
{pr_bodies}

Commit log ({prev_tag or 'beginning'}..{tag}):
{commit_log}"""

    try:
        import asyncio

        async def _generate():
            from copilot import CopilotClient
            from copilot.session import Kind, PermissionRequestResult

            def approve_all(_request, _metadata):
                return PermissionRequestResult(kind=Kind.APPROVED)

            client = CopilotClient()
            await client.start()
            try:
                session = await client.create_session(
                    model="claude-sonnet-4.6",
                    streaming=False,
                    on_permission_request=approve_all,
                )
                reply = await session.send_and_wait(prompt, timeout=600.0)
                if reply is None:
                    return None
                text = reply.data.content
                if not text or len(text) < 50:
                    print(f"AI returned suspiciously short response ({text!r}), using fallback", file=sys.stderr)
                    return None
                return text
            finally:
                await client.stop()

        return asyncio.run(_generate())
    except Exception as e:
        print(f"AI generation failed ({e}), using fallback", file=sys.stderr)
        return None


# Phrases that signal the model is "thinking aloud" instead of producing
# release notes. Matched case-insensitively against the stripped start of
# the first paragraph. Keep this list conservative — false positives silently
# delete real content, which is worse than the original bug.
_META_PREFIXES = (
    "now i ",
    "now that i",
    "here is",
    "here are",
    "here's",
    "let me ",
    "i'll ",
    "i will ",
    "i have ",
    "i've ",
    "i can ",
    "based on ",
    "looking at ",
    "after reviewing",
    "after analyzing",
    "ok,",
    "okay,",
    "alright,",
    "got it",
    "sure,",
    "certainly,",
    "of course",
)


def _looks_like_meta(paragraph):
    """Return True if a paragraph looks like AI 'thinking aloud' preamble."""
    lower = paragraph.strip().lower()
    if not lower:
        return False
    if lower == "---":
        return True
    return any(lower.startswith(p) for p in _META_PREFIXES)


def strip_preamble(text):
    """Strip AI meta-commentary and bare leading separators from release notes.

    The Copilot SDK + Claude pipeline occasionally emits preamble like
    "Now I have enough context to write the release notes..." or a bare
    leading horizontal rule (`---`) before the real content. Both patterns
    have shipped to GitHub releases in the wild (v2026.04.08.1-alpha and
    v2026.04.07.1-alpha respectively). This function is a defensive
    post-processor that runs on every AI-generated notes string.

    Strategy:
    1. Drop a leading bare `---` separator if present.
    2. If the first paragraph begins with a known meta-commentary prefix,
       drop it and recurse (a leaked preamble is often followed by another
       `---` separator that also needs removal).
    3. If stripping would leave a suspiciously short result (< 50 chars),
       bail out and return the original text — better to ship preamble than
       to ship empty notes.
    """
    if not text:
        return text

    original = text
    text = text.lstrip()

    # Case 1: leading bare horizontal rule.
    if text.startswith("---\n") or text.startswith("---\r\n"):
        after_hr = text.split("\n", 1)[1] if "\n" in text else ""
        text = after_hr.lstrip()

    # Case 2: first paragraph is meta-commentary.
    parts = text.split("\n\n", 1)
    if len(parts) == 2:
        first_para, rest = parts
        if _looks_like_meta(first_para):
            # Recurse on the rest — a leaked preamble is often followed by
            # its own `---` separator which we also want to drop.
            stripped_rest = strip_preamble(rest)
            if len(stripped_rest.strip()) >= 50:
                return stripped_rest
            # Stripping emptied the notes — fall through to the bailout.

    # Final safety net: never return suspiciously short output.
    if len(text.strip()) < 50:
        return original
    return text


def generate_fallback_notes(tag, commit_log):
    """Categorize commits by conventional commit prefix."""
    sections = {
        "Language": [],
        "Compiler": [],
        "Standard Library": [],
        "Tooling": [],
        "Bug Fixes": [],
    }

    for line in commit_log.strip().split("\n"):
        line = line.strip()
        if not line or not line.startswith("- "):
            continue
        subject = line[2:]  # strip "- " prefix
        # Skip version bumps and automation
        if subject.startswith("chore: release") or subject.startswith("chore(release)"):
            continue
        if subject.startswith("feat"):
            sections["Language"].append(line)
        elif subject.startswith("fix"):
            sections["Bug Fixes"].append(line)
        elif subject.startswith(("perf", "refactor")):
            sections["Compiler"].append(line)
        elif subject.startswith("docs"):
            continue  # omit docs-only changes
        else:
            sections["Compiler"].append(line)

    body = ""
    for section, items in sections.items():
        if items:
            body += f"## {section}\n\n" + "\n".join(items) + "\n\n"

    if not body.strip():
        body = f"## Changes\n\n{commit_log}"

    return body.strip()


def _run_self_test():
    """Regression tests for strip_preamble(). Run via `--self-test`.

    Fixtures are lifted from real broken releases on GitHub:
      - v2026.04.08.1-alpha: "Now I have enough context..." paragraph leak
      - v2026.04.07.1-alpha: bare leading `---` separator leak
    """
    # Real v2026.04.08.1-alpha leak — the bug that triggered this fix.
    case_paragraph_leak = (
        "Now I have enough context to write the release notes. This release "
        "is almost entirely internal infrastructure — the only user-facing "
        "fix is the cross-compilation sysroot environment variable bug on "
        "macOS with versioned Darwin targets.\n"
        "\n"
        "---\n"
        "\n"
        "# Ori v2026.04.08.1-Alpha — Release Notes\n"
        "\n"
        "This release is a targeted cross-compilation fix for macOS users "
        "combined with internal developer tooling hardening."
    )
    out = strip_preamble(case_paragraph_leak)
    assert out.startswith("# Ori v2026.04.08.1-Alpha"), (
        f"paragraph-leak fixture: preamble not stripped. Got: {out[:80]!r}"
    )
    assert "Now I have enough context" not in out, (
        "paragraph-leak fixture: meta-commentary survived"
    )

    # Real v2026.04.07.1-alpha leak — bare leading horizontal rule.
    case_bare_hr = (
        "---\n"
        "\n"
        "## v2026.04.07.1-alpha\n"
        "\n"
        "This release focuses on AOT codegen correctness and reliability."
    )
    out = strip_preamble(case_bare_hr)
    assert out.startswith("## v2026.04.07.1-alpha"), (
        f"bare-HR fixture: separator not stripped. Got: {out[:80]!r}"
    )

    # Preamble followed by HR followed by heading (combined pattern).
    case_combined = (
        "Here are the release notes for this version.\n"
        "\n"
        "---\n"
        "\n"
        "# Title\n"
        "\n"
        "Some actual summary paragraph that is long enough to pass the "
        "length safety check comfortably."
    )
    out = strip_preamble(case_combined)
    assert out.startswith("# Title"), (
        f"combined fixture: combined preamble not stripped. Got: {out[:80]!r}"
    )
    assert "Here are the release notes" not in out

    # Clean input must pass through unchanged (no false positives).
    case_clean = (
        "# Ori v2026.04.06.1-Alpha — Release Notes\n"
        "\n"
        "This release is entirely focused on ARC correctness for closures. "
        "Indirect closure calls now have sound ownership tracking."
    )
    out = strip_preamble(case_clean)
    assert out == case_clean, (
        f"clean fixture: input was mutated. Got: {out[:80]!r}"
    )

    # Empty string must not crash.
    assert strip_preamble("") == ""
    assert strip_preamble(None) is None

    # Safety net: if stripping would leave <50 chars, return original.
    case_too_short_after_strip = "Now I have enough context.\n\n---\n\nTiny."
    out = strip_preamble(case_too_short_after_strip)
    assert out == case_too_short_after_strip, (
        "safety-net fixture: should have returned original to avoid "
        f"shipping empty notes. Got: {out!r}"
    )

    # Summary-first (no leading title heading) must pass through — the
    # prompt technically allows this shape and we must not eat the summary.
    case_summary_first = (
        "This release focuses on tooling hardening and minor bug fixes in "
        "the LLVM backend. No language-visible changes land here.\n"
        "\n"
        "## Bug Fixes\n"
        "\n"
        "- Fixed something important."
    )
    out = strip_preamble(case_summary_first)
    assert out == case_summary_first, (
        f"summary-first fixture: summary paragraph was eaten. Got: {out[:80]!r}"
    )

    print("strip_preamble() self-test: all assertions passed", file=sys.stderr)
    return 0


def main():
    parser = argparse.ArgumentParser(description="Generate Ori release notes")
    parser.add_argument("--tag", help="Release tag (e.g., v2026.03.23.1-Alpha)")
    parser.add_argument("--prev", default=None, help="Previous tag (auto-detected if omitted)")
    parser.add_argument("-o", "--output", default=None, help="Output file (stdout if omitted)")
    parser.add_argument("--self-test", action="store_true", help="Run strip_preamble() regression tests and exit")
    args = parser.parse_args()

    if args.self_test:
        return _run_self_test()

    if not args.tag:
        parser.error("--tag is required unless --self-test is used")

    prev_tag = args.prev or get_prev_tag(args.tag)
    commit_log = gather_commit_log(prev_tag, args.tag)
    pr_bodies = gather_pr_bodies(prev_tag)

    # Try AI, fall back to structured categorization
    notes = generate_ai_notes(args.tag, prev_tag, commit_log, pr_bodies)
    if notes:
        # Defense-in-depth: strip any AI meta-commentary/preamble that
        # slipped past the prompt-level instructions. See strip_preamble().
        notes = strip_preamble(notes)
    if not notes:
        notes = generate_fallback_notes(args.tag, commit_log)

    if args.output:
        with open(args.output, "w") as f:
            f.write(notes)
        print(f"Release notes written to {args.output}", file=sys.stderr)
    else:
        print(notes)


if __name__ == "__main__":
    sys.exit(main() or 0)
