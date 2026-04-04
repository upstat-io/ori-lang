---
name: tp-help
description: Get third-party help from Codex CLI. Use this proactively when you are stuck on a problem, unsure about an implementation approach, want a second opinion on code you just wrote, need help debugging a failing test, want someone to verify your reasoning about a tricky issue, or want a fresh perspective on a design decision. This is collaborative help, not a formal review — pass context and ask a specific question.
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[question or context]"
---

# Third Party Help (Codex)

Get collaborative help from Codex CLI on whatever you're currently working on. This is not a formal review — it's asking a second brain for help with a specific problem.

## When to Use This

- You're stuck on a bug and can't figure out the root cause
- You're unsure which of two implementation approaches is better
- You just wrote something tricky and want a sanity check
- A test is failing and you can't see why
- You need help understanding unfamiliar code
- You want to validate your reasoning before committing to an approach
- You're about to make a significant architectural decision

## Usage

```
/tp-help [question]
```

Can also be invoked proactively by Claude when it determines outside help would be valuable.

## Workflow

### Step 1: Build Context Package

Gather the relevant context for the question. Be specific — Codex works best with concrete context, not vague requests.

**Always include:**
- The specific question or problem
- The file(s) involved (read them and include key sections)

**Include when relevant:**
- The error message or test failure output
- What you've already tried
- The two approaches you're deciding between
- The spec section that defines expected behavior
- Recent git diff showing what you changed

### Step 2: Format the Prompt

Build a prompt that gives Codex everything it needs in one shot:

```
You are helping with the Ori compiler (Rust codebase, LLVM backend, ARC memory management).

## Question
{The specific question or problem}

## Context
{Key file contents, error messages, diffs — whatever is relevant}

## What I've Tried
{If applicable — what approaches were attempted and why they didn't work}

## Constraints
{Any rules from CLAUDE.md or .claude/rules/ that apply — e.g., "no workarounds, must be architecturally correct"}
```

### Step 3: Call Codex

```bash
codex exec "{prompt}" --full-auto --json
```

**Important:** The prompt must be a single string. For multiline prompts, use a heredoc:

```bash
codex exec "$(cat <<'PROMPT'
You are helping with the Ori compiler (Rust codebase, LLVM backend, ARC memory management).

## Question
{question}

## Context
{context}
PROMPT
)" --full-auto --json
```

**NO TIMEOUT.** Do NOT set a Bash `timeout:` parameter or prefix with `timeout`. Codex reviews legitimately take 5-15 minutes. Let them run to completion.

### Step 4: Parse Response

Extract agent messages from the JSONL output (type: `item.completed`, item.type: `agent_message`). The last few messages contain the answer.

### Step 5: Apply the Answer

- If Codex provided a solution, evaluate it against CLAUDE.md rules before applying
- If Codex suggested an approach, consider it alongside your own analysis
- If Codex found something you missed, incorporate the insight
- If Codex disagrees with your approach, present both perspectives to the user

**Do NOT blindly apply Codex's suggestions.** You have full project context that Codex doesn't — use your judgment to filter and adapt.

### Step 6: Brief the User

Tell the user:
- What you asked Codex
- What Codex said (brief summary)
- How you're applying it (or why you're not)
