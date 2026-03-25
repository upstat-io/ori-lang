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

    prompt = f"""You are writing release notes for **Ori** ({tag}), an alpha-stage statically-typed, expression-based programming language with HM type inference, ARC memory management, and capability-based effects. The audience is developers and language enthusiasts following the project.

Write detailed, informative release notes in markdown. Do NOT wrap in ```markdown fences.

## Format

Start with a 1-2 sentence summary blurb describing the theme of this release.

Then group changes into sections (omit empty ones):
- **Features** — new user-facing capabilities
- **Bug Fixes** — corrected behavior
- **Improvements** — enhancements to existing features, performance, error messages
- **Compiler Internals** — refactoring, architecture changes, code quality in the compiler itself (always include if applicable — compiler development IS the product at alpha stage)
- **Housekeeping** — CI/CD, build scripts, docs, repo maintenance, tooling, website changes — anything that isn't the compiler or language itself. Keep these brief (one line each).

For each bullet:
- **Bold title** followed by 1-2 sentences explaining what changed and why it matters
- Use past tense ("Added", "Fixed", "Improved")
- Reference affected areas (e.g., type checker, evaluator, LLVM codegen, parser)
- Housekeeping items can be shorter — just a bold title and one sentence is fine

## Rules
- The PR descriptions are your PRIMARY source — they contain human-written summaries of what changed and why
- The commit log is supplementary — use it to catch anything the PRs missed
- Never say "Internal improvements and maintenance" — every change gets a meaningful description
- Skip "nightly" automation PRs — focus on substantive changes
- Do not reproduce test plan checklists — focus on what changed, not how it was tested

## Input

Pull request descriptions (primary source):
{pr_bodies}

Commit log ({prev_tag or 'beginning'}..{tag}):
{commit_log}"""

    try:
        import asyncio

        async def _generate():
            from copilot import CopilotClient
            from copilot.session import PermissionHandler

            client = CopilotClient()
            await client.start()
            try:
                session = await client.create_session(
                    model="claude-sonnet-4.6",
                    streaming=False,
                    on_permission_request=PermissionHandler.approve_all,
                )
                done = asyncio.Event()
                result = []

                def on_event(event):
                    t = event.type.value if hasattr(event.type, "value") else str(event.type)
                    if t == "assistant.message":
                        content = event.data.content if hasattr(event.data, "content") else str(event.data)
                        result.append(content)
                    elif t == "session.idle":
                        done.set()

                session.on(on_event)
                await session.send({"prompt": prompt})
                await asyncio.wait_for(done.wait(), timeout=120)
                return result[-1] if result else None
            finally:
                await client.stop()

        return asyncio.run(_generate())
    except Exception as e:
        print(f"AI generation failed ({e}), using fallback", file=sys.stderr)
        return None


def generate_fallback_notes(tag, commit_log):
    """Categorize commits by conventional commit prefix."""
    sections = {
        "Features": [],
        "Bug Fixes": [],
        "Improvements": [],
        "Compiler Internals": [],
        "Housekeeping": [],
    }

    for line in commit_log.strip().split("\n"):
        line = line.strip()
        if not line or not line.startswith("- "):
            continue
        subject = line[2:]  # strip "- " prefix
        if subject.startswith("feat"):
            sections["Features"].append(line)
        elif subject.startswith("fix"):
            sections["Bug Fixes"].append(line)
        elif subject.startswith("perf"):
            sections["Improvements"].append(line)
        elif subject.startswith("refactor"):
            sections["Compiler Internals"].append(line)
        else:
            sections["Housekeeping"].append(line)

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
