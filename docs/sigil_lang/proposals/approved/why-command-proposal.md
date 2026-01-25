# Proposal: Causality Tracking (`sigil impact` and `sigil why`)

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-24
**Approved:** 2026-01-25

---

## Summary

Expose Salsa's dependency tracking to users via two commands:

- `sigil impact <target>` — What will be affected if I change this? (before)
- `sigil why <target>` — Why did this break? (after)

```bash
# Before: know the blast radius
$ sigil impact @parse
If @parse changes:
  @compile        → uses @parse directly
  @run_program    → uses @compile
  12 functions affected

# After: trace to the source
$ sigil why @compile
@compile broke because:
  → @parse changed (src/parser.si:42)
```

**Know what breaks before you break it. Know why it broke after.**

---

## Motivation

### The Problem: Two Questions Developers Always Ask

**Before changing code:**
> "If I change this, what else will break?"

**After something breaks:**
> "Why did this break? I didn't touch that code."

Traditional workflow:
1. Make a change, hope for the best
2. Something unrelated breaks
3. Manually trace dependencies
4. Eventually find the root cause
5. Time wasted, frustration high

### The Opportunity

Sigil already knows the answers. Salsa tracks:
- Which functions depend on which inputs
- The full chain of causality through the codebase
- What changed and when

This information exists internally for incremental compilation. Exposing it gives developers superpowers.

### Why No One Does This

Most build systems treat dependency tracking as internal bookkeeping:
- Make: Timestamps, no semantic understanding
- Bazel: Action graph, but not exposed meaningfully
- Cargo: Recompilation hints, but no causality chain
- IDEs: "Find usages" but no impact analysis

Sigil, with Salsa's fine-grained tracking, can expose the full causality chain.

---

## Design

### Two Commands

| Command | Question | When |
|---------|----------|------|
| `sigil impact <target>` | What breaks if I change this? | Before changing |
| `sigil why <target>` | Why did this break? | After failure |

### `sigil impact` — Before You Change

Shows the blast radius of a potential change:

```bash
$ sigil impact @parse
If @parse changes:
  Direct dependents:
    @compile        (src/compiler.si:23)
    @format         (src/formatter.si:15)

  Transitive dependents:
    @run_program    → via @compile
    @build          → via @compile
    @check          → via @compile

  Summary: 6 functions affected
```

#### Detailed Impact

```bash
$ sigil impact @parse --verbose
If @parse changes:
  @compile (src/compiler.si:23)
    └── calls parse() at line 31
    └── calls parse() at line 47

  @format (src/formatter.si:15)
    └── calls parse() at line 22
```

#### Impact on Types

```bash
$ sigil impact @Ast
If @Ast type changes:
  Functions using Ast: 23
  Functions returning Ast: 8
  Functions accepting Ast: 15

  This is a high-impact change.
```

### `sigil why` — After Something Breaks

Traces a failure back to its source:

```bash
$ sigil why @compile
@compile broke because:
  → @parse changed (src/parser.si:42)
    - line 42: return type changed from Ast to Result<Ast, Error>
```

#### Multiple Causes

```bash
$ sigil why @build
@build broke because:
  → @compile changed
    → @parse changed (src/parser.si:42)
    → @optimize changed (src/optimizer.si:17)
```

#### With Diff

```bash
$ sigil why @compile --diff
@compile broke because:
  → @parse changed (src/parser.si:42)

  @@ -42,7 +42,7 @@
  -    Ast { nodes: nodes }
  +    Ok(Ast { nodes: nodes })
```

### Integration with Test Output

When tests fail, suggest the command:

```
$ sigil test
FAIL: @test_compile (0.003s)
  assertion failed: expected Ok, got Err

Hint: This test is dirty due to changes in @parse.
      Run `sigil why @test_compile` for details.
```

### Verbosity Levels

```bash
# Concise (default)
$ sigil why @test_compile
@test_compile dirty: @parse changed (src/parser.si:42)

# Detailed
$ sigil why @test_compile --verbose
@test_compile is dirty because:
  → calls @compile (src/compiler.si:15)
    → calls @parse (src/compiler.si:23)
      → @parse body changed
        - src/parser.si:42: added new match arm
        - src/parser.si:47: changed return type

# Graph view
$ sigil why @test_compile --graph
@test_compile
└── @compile
    ├── @parse ← CHANGED
    └── @optimize
```

---

## Implementation

### Salsa Already Tracks This

Salsa's incremental computation model tracks:
- Which queries depend on which inputs
- Which inputs have changed
- The memo/recomputation status of each query

The `why` command just formats this information.

### Core Algorithm

```rust
fn explain_why_dirty(db: &dyn Database, target: QueryId) -> CausalityChain {
    let mut chain = CausalityChain::new(target);

    // Get the inputs that changed
    let changed_inputs = db.changed_inputs_for(target);

    // For each changed input, find the path from target to input
    for input in changed_inputs {
        let path = find_dependency_path(db, target, input);
        chain.add_path(path);
    }

    chain
}

fn find_dependency_path(db: &dyn Database, from: QueryId, to: QueryId) -> Vec<QueryId> {
    // BFS/DFS through dependency graph
    // Return path from 'from' to 'to'
}
```

### Integration Points

1. **CLI:** Add `impact` and `why` subcommands
2. **Salsa wrapper:** Expose dependency information
3. **Formatter:** Pretty-print causality chains
4. **Test runner:** Suggest `why` on failures

---

## Examples

### Debugging Test Failures

```bash
$ sigil test
FAIL: @test_user_service

$ sigil why @test_user_service
@test_user_service is dirty because:
  → calls @user_service
    → calls @validate_email
      → @validate_email body changed (src/validation.si:28)

# Now you know: check the email validation change
```

### Understanding Build Times

```bash
$ sigil build --time
Build complete (12.3s)
Slowest recompilations:
  @compile: 4.2s
  @type_check: 3.1s

$ sigil why --recompile @compile
@compile recompiled because:
  → @Ast type definition changed (src/ast.si:15)

# The AST type change cascaded through the compiler
```

### Impact Analysis Before Committing

```bash
$ sigil impact @parse
If @parse changes:
  Functions invalidated: 12
  Tests that will run: 8
  Estimated test time: 0.3s

$ sigil impact @Ast
If @Ast changes:
  Functions invalidated: 47
  Tests that will run: 31
  Estimated test time: 2.1s

# Maybe refactor to reduce Ast coupling...
```

### CI Integration

```yaml
# .github/workflows/test.yml
- name: Run tests
  run: sigil test

- name: Explain failures
  if: failure()
  run: |
    sigil test --list-failed | while read test; do
      echo "=== $test ==="
      sigil why "$test"
    done
```

---

## Future Extensions

### Blame Integration

```bash
$ sigil why @test_compile --blame
@test_compile is dirty because:
  → @parse body changed (src/parser.si:42)
    Author: alice@example.com
    Commit: abc123 "Add new syntax for lambdas"
    Date: 2 hours ago
```

### Time Travel

```bash
# Why was this test dirty in a previous run?
$ sigil why @test_compile --at=2024-01-20T10:30:00
```

### Visualization

```bash
$ sigil why @test_compile --format=dot | dot -Tpng > causality.png
```

Generate a visual graph of the causality chain.

### IDE Integration

Hover over a failing test in VS Code:
> "This test is dirty because @parse changed (src/parser.si:42)"
> [Click to see full causality chain]

---

## Benefits

| Benefit | Description |
|---------|-------------|
| **Debugging** | Instantly understand why tests are running/failing |
| **Impact analysis** | See what a change will affect before making it |
| **Education** | Learn how your codebase is connected |
| **CI clarity** | Explain failures in automated pipelines |
| **Novel** | No other language/tool exposes this |

---

## Tradeoffs

| Cost | Mitigation |
|------|------------|
| CLI complexity | Single command with clear purpose |
| Output can be verbose | Multiple verbosity levels |
| Depends on Salsa internals | Stable interface over internal queries |

---

## Summary

The `sigil impact` and `sigil why` commands expose Salsa's dependency tracking to users:

- **Why is this dirty?** — Trace from test to changed input
- **What if I change this?** — Impact analysis before changes
- **Causality, not execution** — Understand *why*, not just *what*

This is novel. No other language exposes the causality chain of incremental computation to users. Sigil's "Code that proves itself" philosophy extends to "Code that explains itself."

The implementation is straightforward — the information already exists in Salsa. We're just formatting it.
