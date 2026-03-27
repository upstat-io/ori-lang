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

## Core Principle: Deliverables, Not Process

Release notes describe what was DELIVERED, not the process of delivery. Think of it like a code review: when a PR merges, the release notes say "Added feature X" — not "Added feature X, then fixed 8 things the reviewer caught." Review feedback is part of delivering X correctly.

Apply this to ALL process artifacts:
- **TPR (Third-Party Review) findings** — fold into the feature they harden. "Conditional compilation now works on all item types, with proper validation and no incremental parse leakage" — NOT 8 separate TPR bullets. Only surface a TPR as a standalone Bug Fix if it reveals broken behavior that existed in a PREVIOUS release and affected users.
- **Internal plan references** — strip section numbers, plan IDs, and internal tracking codes (no "§02.1", "TPR-01-062", "Phase A"). Use plain English: "representation optimization" not "repr-opt §02".
- **Iterative refinements** — hardening, edge case fixes, and polish done during development are part of the feature, not separate items.

**The test**: would an outside contributor who has never seen our plans, TPR system, or internal tracking understand every bullet? If not, rewrite it.

## Rules
- The PR descriptions are your PRIMARY source — they contain human-written summaries of what changed and why
- The commit log is supplementary — use it to catch anything the PRs missed
- Curate ruthlessly — 4 meaningful bullets beats 12 granular ones. Combine related work into single entries.
- Never dump the git log — every entry must be written for humans
- Never say "Internal improvements and maintenance" — if it matters, describe the impact; if it doesn't, omit it
- Skip version-bump commits and nightly automation PRs
- Do not reproduce test plan checklists
- Quantify performance improvements when data is available ("2.3x faster", "95 to 128 MiB/s")
- For diagnostic improvements, briefly describe the before/after experience
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
                reply = await session.send_and_wait(prompt, timeout=120.0)
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


def main():
    parser = argparse.ArgumentParser(description="Generate Ori release notes")
    parser.add_argument("--tag", required=True, help="Release tag (e.g., v2026.03.23.1-Alpha)")
    parser.add_argument("--prev", default=None, help="Previous tag (auto-detected if omitted)")
    parser.add_argument("-o", "--output", default=None, help="Output file (stdout if omitted)")
    args = parser.parse_args()

    prev_tag = args.prev or get_prev_tag(args.tag)
    commit_log = gather_commit_log(prev_tag, args.tag)
    pr_bodies = gather_pr_bodies(prev_tag)

    # Try AI, fall back to structured categorization
    notes = generate_ai_notes(args.tag, prev_tag, commit_log, pr_bodies)
    if not notes:
        notes = generate_fallback_notes(args.tag, commit_log)

    if args.output:
        with open(args.output, "w") as f:
            f.write(notes)
        print(f"Release notes written to {args.output}", file=sys.stderr)
    else:
        print(notes)


if __name__ == "__main__":
    main()
