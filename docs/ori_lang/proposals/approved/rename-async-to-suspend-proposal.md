# Proposal: Rename Async Capability to Suspend

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Spec, standard library, error messages

---

## Summary

Rename the `Async` marker capability to `Suspend` to better reflect its semantics and avoid confusion with async/await patterns from other languages.

---

## Problem Statement

The current `Async` capability name has several issues:

1. **Misleading association**: Developers familiar with async/await (JavaScript, Rust, C#, Python) expect `.await` syntax. Ori has no `.await` — the name sets incorrect expectations.

2. **Describes paradigm, not behavior**: Other capabilities describe what a function *does* or *accesses*: `Http`, `FileSystem`, `Clock`. `Async` describes a programming paradigm, not behavior.

3. **The actual semantics**: The capability indicates a function may *suspend* execution. The word "async" doesn't convey this directly.

4. **Teaching burden**: Documentation must repeatedly clarify "Async doesn't mean async/await" — a sign the name is fighting intuition.

---

## Approved Changes

### Capability Name

Rename `Async` to `Suspend` (noun form for grammatical consistency with `uses Http`, `uses Clock`).

```ori
// Before
@fetch (url: str) -> Result<Data, Error> uses Http, Async = ...

// After
@fetch (url: str) -> Result<Data, Error> uses Http, Suspend = ...
```

### Terminology Changes

| Before | After |
|--------|-------|
| `Async` capability | `Suspend` capability |
| `uses Async` | `uses Suspend` |
| "async context" | "suspending context" |

### Standard Capabilities Table

Rename column from "Suspends" to "May Suspend" to avoid collision with capability name:

| Capability | Purpose | May Suspend |
|------------|---------|-------------|
| `Http` | HTTP client | Yes |
| `FileSystem` | File I/O | Yes |
| `Suspend` | Suspension marker | Yes |

---

## Rationale

### Semantic Accuracy

`Suspend` describes exactly what the capability grants: the ability to suspend execution.

### Grammatical Consistency

`uses Suspend` follows the noun pattern of other capabilities:
- `uses Http` (noun)
- `uses Clock` (noun)
- `uses Suspend` (noun — the ability/act of suspension)

### No Async/Await Confusion

Developers won't expect `.await` when they see `Suspend`. The name communicates:
- This function may pause and resume
- No special syntax needed to call it
- Concurrency via `parallel(...)`, not `await`

---

## Alternatives Considered

### Keep `Async`

**Pros**: Familiar terminology
**Cons**: Misleading, doesn't describe behavior, requires constant clarification

### Use `Suspends` (verb form)

**Pros**: Describes action
**Cons**: Grammatically awkward with `uses` (verb after verb)

### Use `Concurrent`

**Pros**: Implies non-blocking
**Cons**: Inaccurate — a function can suspend without being concurrent; concurrency is via `parallel`

---

## Migration

### For Users

Search and replace `uses Async` with `uses Suspend` in all `.ori` files.

### For Documentation

- Update all spec files with new terminology
- Update CLAUDE.md
- Update "async context" to "suspending context"

### For Compiler

1. Update lexer/parser to recognize `Suspend` keyword
2. Deprecation period: Accept both `Async` and `Suspend`, warn on `Async`
3. Remove `Async` in next major version

---

## Spec Changes Required

### Update `14-capabilities.md`

- Rename "Async Capability" section to "Suspend Capability"
- Replace all `Async` references with `Suspend`
- Rename table column "Suspends" to "May Suspend"
- Update error code E1203 message

### Update `23-concurrency-model.md`

- Replace "async context" with "suspending context"
- Replace `uses Async` with `uses Suspend`

### Update `03-lexical-elements.md`

- Replace `async` with `suspend` in reserved keywords

### Update `CLAUDE.md`

- Replace `Async` with `Suspend` in Capabilities section

---

## Error Message Updates

### E1203

Before:
```
error[E1203]: `Async` capability cannot be explicitly bound
```

After:
```
error[E1203]: `Suspend` capability cannot be explicitly bound
```

### New Deprecation Warning

```
warning: `Async` is deprecated, use `Suspend`
  --> src/main.ori:5:40
   |
 5 | @fetch (url: str) -> Data uses Http, Async = ...
   |                                      ^^^^^ help: replace with `Suspend`
```

---

## Summary

| Aspect | Before | After |
|--------|--------|-------|
| Capability name | `Async` | `Suspend` |
| Declaration | `uses Async` | `uses Suspend` |
| Context term | "async context" | "suspending context" |
| Table column | "Suspends" | "May Suspend" |
| Semantics | Unchanged | Unchanged |

The rename improves clarity without changing any runtime behavior.
