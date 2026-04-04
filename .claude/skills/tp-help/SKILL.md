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

### Step 3: Call Codex via Agent

**CRITICAL: Use an Agent, NOT direct Bash.** The Bash tool has a 120-second default timeout that kills codex before it finishes. An Agent has no timeout and will wait for codex to complete.

Spawn an Agent with this pattern:

```
Launch Agent with prompt:
  "Run the following codex command and return the full output:
   
   codex exec '{prompt}' --full-auto --json 2>/dev/null | tail -200
   
   Then parse the JSONL output to extract agent_message items:
   
   cat <output> | python3 -c \"
   import sys, json
   for line in sys.stdin:
       line = line.strip()
       if not line: continue
       try:
           obj = json.loads(line)
           if obj.get('type') == 'item.completed' and obj.get('item', {}).get('type') == 'agent_message':
               print(obj['item']['text'])
       except json.JSONDecodeError: pass
   \" | tail -3000
   
   Return the extracted messages."
```

**DO NOT:**
- Run `codex exec` directly via Bash tool (will timeout or auto-background)
- Set `run_in_background: true` on the Agent
- Set any timeout on the Bash call inside the Agent

### Step 4: Apply the Answer

- Evaluate Codex's response against CLAUDE.md rules before applying
- You have full project context that Codex doesn't — use your judgment to filter
- If Codex disagrees with your approach, present both perspectives to the user

### Step 5: Brief the User

Tell the user:
- What you asked Codex
- What Codex said (brief summary)
- How you're applying it (or why you're not)
