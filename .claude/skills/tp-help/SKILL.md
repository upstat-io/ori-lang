---
name: tp-help
description: "Get third-party help from Codex CLI. Use this proactively when you are stuck on a problem, unsure about an implementation approach, want a second opinion on code you just wrote, need help debugging a failing test, want someone to verify your reasoning, or need help understanding unfamiliar code."
---

# Third Party Help (Codex)

Get collaborative help from Codex CLI on whatever you're currently working on.

## Usage

```
/tp-help [question]
```

## Workflow

### Step 1: Build Context Package

Gather the relevant context for the question. Be specific.

**Always include:**
- The specific question or problem
- The file(s) involved (read them and include key sections)

**Include when relevant:**
- Error messages or test failure output
- What you've already tried
- The two approaches you're deciding between
- The spec section that defines expected behavior

### Step 2: Format the Prompt

Build a prompt that gives Codex everything it needs in one shot:

```
You are helping with the Ori compiler (Rust codebase, LLVM backend, ARC memory management).

## Question
{The specific question or problem}

## Context
{Key file contents, error messages, diffs}

## What I've Tried
{If applicable}

## Constraints
{Any rules from CLAUDE.md or .claude/rules/ that apply}
```

### Step 3: Call Codex via Bash in background

Run codex directly via the Bash tool with `run_in_background: true`. The
`.claude/hooks/block-banned-commands.sh` hook allows background execution
on codex commands specifically; the 2-minute foreground default cap does
not apply to background tasks. You will receive a completion notification
when codex finishes (typically 5-15 minutes).

Write a prompt file first so heredocs/quoting don't fight shell escaping:

```
Write '/tmp/tp-help-prompt.md' with the full question + context package.
```

Then launch codex in the background:

```
Bash (run_in_background: true):
  rm -f /tmp/tp-help.jsonl /tmp/tp-help.done
  codex exec "$(cat /tmp/tp-help-prompt.md)" --full-auto --json 2>/dev/null > /tmp/tp-help.jsonl
  ec=$?
  touch /tmp/tp-help.done
  echo "exit=$ec"
```

Continue working or wait idle. When the completion notification arrives,
parse the JSONL output for `agent_message` items:

```
Bash:
  python3 -c "
  import json
  with open('/tmp/tp-help.jsonl') as f:
      for line in f:
          line = line.strip()
          if not line:
              continue
          try:
              obj = json.loads(line)
              if obj.get('type') == 'item.completed' and obj.get('item', {}).get('type') == 'agent_message':
                  print(obj['item']['text'])
                  print()
          except json.JSONDecodeError:
              pass
  "
```

**DO NOT:**
- Run `codex exec` in the Bash foreground (will hit the 2-minute default
  timeout or get auto-backgrounded; either way output may be truncated).
- Wrap codex in an Agent subagent — the Agent adds no value over direct
  background Bash, costs an extra process, and the Agent cannot be
  `run_in_background: true` so it can't wait longer than the harness cap.
- Set a `timeout:` parameter on the Bash call (the hook blocks timeouts
  under 5 minutes on codex; backgrounding is the preferred path).
- Inline the full prompt in the Bash command — shell escaping of multi-
  line markdown is fragile; write to a file and `cat` it instead.

### Step 4: Apply the Answer

- Evaluate Codex's response against CLAUDE.md rules before applying
- You have full project context that Codex doesn't — use your judgment to filter
- If Codex disagrees with your approach, present both perspectives to the user

### Step 5: Brief the User

Tell the user:
- What you asked Codex
- What Codex said (brief summary)
- How you're applying it (or why you're not)
