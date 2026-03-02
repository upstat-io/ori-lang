# Tooling

This section covers Ori's tooling for AI-first development: semantic addressing, structured errors, formatter, LSP, REPL, and refactoring API.

---

## Documents

| Document | Description |
|----------|-------------|
| [Semantic Addressing](01-semantic-addressing.md) | Addressable code elements |
| [Edit Operations](02-edit-operations.md) | set, add, remove, rename, move |
| [Structured Errors](03-structured-errors.md) | JSON error format for AI |
| [Formatter](04-formatter.md) | Canonical formatting |
| [LSP](05-lsp.md) | Language server features |
| [REPL](06-repl.md) | Interactive evaluation |
| [Refactoring API](07-refactoring-api.md) | Programmatic refactoring |

---

## Overview

Ori's tooling is designed for AI integration:

### Semantic Addressing

Every code element is addressable:

```
// function
@function_name
// pattern property
@function_name.attempts
// config
$config_name
// struct field
type TypeName.field
```

### Structured Errors

JSON output for AI self-correction:

```json
{
  "errors": [{
    "id": "E0308",
    "message": "mismatched types",
    "location": {
      "address": "@process.body"
    },
    "suggestions": [{
      "edit": { "op": "set", "address": "...", "value": "..." },
      "confidence": "high"
    }]
  }]
}
```

### Edit Operations

Targeted modifications without regenerating files:

```json
{ "op": "set", "address": "@fetch.attempts", "value": "5" }
```

### Key Principles

1. **JSON-first** - Structured I/O for AI
2. **Semantic, not syntactic** - Operate on meaning, not text
3. **Canonical formatting** - One way to format code
4. **Integrated testing** - Tests visible everywhere

---

## See Also

- [Main Index](../00-index.md)
- [Testing](../11-testing/index.md)
