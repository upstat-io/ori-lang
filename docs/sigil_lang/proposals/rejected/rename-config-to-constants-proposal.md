# Proposal: Rename "Config Variables" to "Constants"

**Status:** Rejected
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Rejected:** 2026-01-25
**Reason:** Superseded by const-keyword-proposal.md — decided to replace `$` sigil entirely with `const` keyword for familiarity.
**Affects:** Documentation, specification, compiler backend

---

## Summary

Rename "config variables" to "constants" throughout the language documentation, specification, and compiler codebase. The `$` sigil syntax remains unchanged.

```sigil
// Before (terminology only)
$timeout = 30s  // "config variable"

// After (terminology only)
$timeout = 30s  // "constant"
```

---

## Motivation

### The Problem

The current terminology "config variable" is misleading:

1. **"Variable" implies mutability** — but these values cannot be reassigned
2. **"Config" implies runtime configuration** — but these are compile-time literals
3. **Cognitive dissonance** — they behave exactly like constants in every other language

What they actually are:
- Immutable
- Module-level scope
- Must be initialized with literals
- Evaluated at compile time

This is the definition of a constant.

### Why "Config Variable" Was Chosen

The original intent was to signal the *use case* (configuration values like timeouts, URLs, feature flags). However, the implementation is just compile-time constants, and the name causes confusion.

---

## Design

### Terminology Changes

| Current | Proposed |
|---------|----------|
| config variable | constant |
| config | constant |
| $name (a config) | $name (a constant) |

### Syntax (Unchanged)

The `$` sigil and all syntax remains exactly the same:

```sigil
$max_retries = 3
$timeout = 30s
$api_base = "https://api.example.com"
pub $default_limit = 100
```

### Grammar Production (Rename Only)

```ebnf
// Before
config = [ "pub" ] "$" identifier "=" literal .

// After
constant = [ "pub" ] "$" identifier "=" literal .
```

---

## Changes Required

### Documentation

1. **Spec files:**
   - `spec/04-constants.md` — rename "Config Variables" section to "Constants" (file name already correct)
   - Update all references to "config variable" → "constant"

2. **Design files:**
   - `design/02-syntax/01-basic-syntax.md` — rename section, update text
   - `design/02-syntax/index.md` — update references
   - `design/glossary.md` — update entry
   - All other files referencing "config"

3. **CLAUDE.md:**
   - Rename "Config Variables" section to "Constants"
   - Update description and examples

### Compiler Backend

Rename internal types and functions:

| Current (estimated) | Proposed |
|---------------------|----------|
| `ConfigVar` | `Constant` |
| `config_variables` | `constants` |
| `parse_config` | `parse_constant` |
| `ConfigDecl` | `ConstantDecl` |

*Note: Actual names depend on current compiler implementation.*

### Error Messages

Update any error messages that mention "config":

```
// Before
error: config variable must be initialized with a literal

// After
error: constant must be initialized with a literal
```

---

## Migration

This is a **documentation-only change** for users. No code changes required:

- Syntax unchanged
- Semantics unchanged
- No deprecation period needed

---

## Alternatives Considered

### 1. Remove the `$` Sigil Entirely

Use `const` keyword like other languages:

```sigil
const max_retries = 3
const timeout = 30s
```

**Rejected:** The `$` sigil provides visual distinction at usage sites. When you see `$timeout` in code, you immediately know it's a module-level constant, not a local variable. This aligns with Sigil's philosophy of explicit sigils (`@` for functions, `$` for constants).

### 2. Keep "Config" Name

Maintain current terminology despite the confusion.

**Rejected:** The term actively misleads users about the feature's nature.

### 3. Call Them "Module Constants"

More precise name indicating scope.

**Rejected:** Overly verbose. "Constant" is sufficient since the `$` sigil already implies module-level scope.

---

## Summary

- Rename "config variable" → "constant" in all docs and compiler code
- Keep `$` sigil syntax unchanged
- No user-facing code changes required
- Eliminates terminology confusion
