# Proposal: Always Stack Named Parameters

**Status:** Superseded
**Author:** Eric
**Created:** 2026-01-22
**Approved:** 2026-01-22
**Superseded:** 2026-01-24 by `remove-dot-prefix-proposal.md`

> **Note:** This proposal was superseded by the decision to use width-based formatting instead of always-stack. Named arguments now use `name: value` syntax (no dot prefix) and format inline when they fit within line width, stacking only when they exceed it.

---

## Summary

Change the formatter rule from "stack named parameters when 2+" to "always stack named parameters, even single parameters."

```sigil
// Before (single param inline allowed)
print(.msg: "Hello, world!")

// After (all params stacked)
print(
    .msg: "Hello, world!",
)
```

---

## Motivation

### Human Readability

The `.property:` sigil creates a visual rail when stacked:

```sigil
retry(
    .op: fetch(),
    .attempts: 3,
    .backoff: exponential(),
)
```

The dots align vertically — you can instantly see "there are 3 parameters" without reading. With inline formatting, you have to scan the text to count parameters.

### AI Modifiability

Stacked format makes each parameter an atomic, independent line:

```
    .timeout: 5s,
```

This enables surgical edits:

| Operation | Action |
|-----------|--------|
| Add param | Insert one line |
| Remove param | Delete one line |
| Modify param | Change one line |

No bracket matching. No comma fixups. No multi-edit operations. No reflowing.

**Example — adding a parameter:**

```sigil
// Before edit
retry(
    .op: fetch(),
    .attempts: 3,
)

// After edit (only line 4 inserted)
retry(
    .op: fetch(),
    .attempts: 3,
    .timeout: 5s,
)
```

The trailing comma convention means no line depends on its neighbors. Each line is self-contained.

### Consistency

The previous rule created two formats:
- 1 param: inline
- 2+ params: stacked

The new rule has one format:
- All params: stacked

One rule is simpler than two. No edge cases about when to stack.

---

## Design

### Formatter Rule

**Old rule:**
> Single property can be inline; 2+ properties are always stacked.

**New rule:**
> All named properties are always stacked vertically.

### Examples

```sigil
// Single parameter
print(
    .msg: "Hello, world!",
)

len(
    .of: items,
)

sqrt(
    .value: 16,
)

// Multiple parameters
map(
    .over: items,
    .transform: x -> x * 2,
)

retry(
    .op: http_get(
        .url: "/api/data",
    ),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
)

// Nested calls — each stacks independently
@process (items: [int]) -> int = fold(
    .over: filter(
        .over: items,
        .predicate: x -> x > 0,
    ),
    .init: 0,
    .op: +,
)
```

### Function Calls

Per [function-seq-exp-distinction.md](function-seq-exp-distinction.md), function calls now require named arguments for multi-parameter functions:

```sigil
// Single-param: positional OK, inline
print("hello")
len(items)
str(42)

// Multi-param: named required, stacked
add(
    .a: 1,
    .b: 2,
)

assert_eq(
    .actual: result,
    .expected: 42,
)

compare(
    .left: a,
    .right: b,
)
```

The stacking rule applies uniformly:
- **function_exp** (patterns): Always stacked
- **Function calls** (multi-param): Always stacked
- **Function calls** (single-param): Inline allowed

---

## Benefits

| Benefit | Description |
|---------|-------------|
| **Scanability** | Dots form vertical rail, params visible at a glance |
| **Atomic edits** | One line = one param, no dependencies |
| **Clean diffs** | Adding/removing param = single-line diff |
| **No bracket matching** | Structure is self-evident per line |
| **No comma fixups** | Trailing commas on every line |
| **Consistency** | One rule, no "1 vs 2+" distinction |
| **AI-friendly** | Mechanical modification without complex parsing |

---

## Tradeoffs

| Cost | Mitigation |
|------|------------|
| More vertical space | Whitespace aids readability |
| Simple calls look verbose | Consistency outweighs brevity |
| More lines of code | Lines are simpler, easier to process |

---

## Implementation

### Formatter Changes

Update the formatter to always emit stacked format when encountering `.name:` syntax inside a call expression.

### Files Updated

- `docs/sigil_lang/0.1-alpha/design/12-tooling/04-formatter.md` — Updated rule and rationale
- `CLAUDE.md` — Updated formatting rules summary

---

## Rationale

This change aligns with Sigil's core philosophy:

1. **Explicit over implicit** — Structure is visible, not hidden in dense inline text
2. **AI-first design** — Optimized for predictable, mechanical modification
3. **One way to do things** — Single format, no decisions
4. **Sigil philosophy** — The `.` sigil earns its place by creating scannable structure

The slight increase in verbosity is justified by the significant gains in readability and modifiability.

---

## Summary

Always stacking named arguments:
- Makes code easier for humans to scan (visual alignment)
- Makes code easier for AI to modify (atomic line edits)
- Simplifies the formatter rule (one rule, not two)
- Reinforces the value of the `.name:` sigil (creates visual structure)

This applies to:
- **function_exp** patterns (`map`, `filter`, `fold`, etc.) — always stacked
- **Function calls** with 2+ parameters — always stacked
- **Function calls** with 1 parameter — inline (no ambiguity)

See also: [function-seq-exp-distinction.md](function-seq-exp-distinction.md)

Approved for implementation.
