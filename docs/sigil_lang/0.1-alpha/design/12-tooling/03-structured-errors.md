# Structured Errors

Sigil's error output is designed for AI self-correction. Every error includes structured data, semantic addresses, and ready-to-apply fix suggestions.

---

## Design Principles

1. **Machine-parseable by default** - JSON output enables automated fix loops
2. **Semantic addresses included** - Errors link directly to the edit system
3. **Fix suggestions with confidence** - AI knows which fixes to apply automatically
4. **Deterministic output** - Same input always produces same error order
5. **Primary vs cascading** - Help AI fix root causes, not symptoms

---

## JSON Error Format

### Complete Error Structure

```json
{
  "errors": [
    {
      "id": "E0308",
      "severity": "error",
      "category": "type",
      "message": "mismatched types",
      "location": {
        "file": "src/main.si",
        "line": 15,
        "column": 10,
        "end_line": 15,
        "end_column": 25,
        "address": "@process.body"
      },
      "expected": "int",
      "found": "str",
      "context": {
        "expression": "x + \"hello\"",
        "surrounding_code": "    result = x + \"hello\""
      },
      "suggestions": [
        {
          "message": "convert int to str",
          "edit": {
            "op": "set",
            "address": "@process.body",
            "value": "str(x) + \"hello\""
          },
          "confidence": "medium"
        },
        {
          "message": "convert str to int",
          "edit": {
            "op": "set",
            "address": "@process.body",
            "value": "x + int(\"hello\")"
          },
          "confidence": "low"
        }
      ],
      "related": [
        {
          "file": "src/main.si",
          "line": 10,
          "message": "x defined here as int"
        }
      ],
      "docs_url": "https://sigil-lang.org/errors/E0308",
      "is_primary": true
    }
  ],
  "warnings": [],
  "stats": {
    "error_count": 1,
    "warning_count": 0,
    "files_checked": 3,
    "time_ms": 45
  }
}
```

---

## Error Structure Fields

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique error code (e.g., "E0308") |
| `severity` | enum | `error`, `warning`, `info` |
| `category` | enum | Error category for routing |
| `message` | string | Short, actionable description |
| `location` | object | Where the error occurred |

### Location Object

```json
{
  "location": {
    "file": "src/api.si",
    "line": 15,
    "column": 10,
    "end_line": 15,
    "end_column": 25,
    "address": "@api.fetch_data.op"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `file` | string | Relative file path |
| `line` | int | Line number (1-indexed) |
| `column` | int | Column number (1-indexed) |
| `end_line` | int | End line for spans |
| `end_column` | int | End column for spans |
| `address` | string | Semantic address (see [Semantic Addressing](01-semantic-addressing.md)) |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `expected` | string | What was expected (for type errors) |
| `found` | string | What was found (for type errors) |
| `context` | object | Expression and surrounding code |
| `suggestions` | array | Possible fixes with edits |
| `related` | array | Related locations |
| `docs_url` | string | Link to error documentation |
| `is_primary` | bool | True if this is a root cause |
| `caused_by` | string | Error ID that caused this (cascading errors) |

---

## Error Categories

Errors are categorized to help AI route them to appropriate handlers.

| Category | Examples | Typical AI Action |
|----------|----------|-------------------|
| `syntax` | Missing paren, invalid token | Clear fix from suggestion |
| `type` | Type mismatch, missing field | May need design decision |
| `reference` | Unknown identifier, missing import | Usually clear fix |
| `pattern` | Invalid pattern usage, missing property | Check pattern docs |
| `test` | Test failure, assertion error | Examine test logic |
| `coverage` | Untested function | Generate test |
| `import` | Circular dependency, missing module | Restructure imports |

### Category-Specific Fields

**Type errors include:**
```json
{
  "category": "type",
  "expected": "int",
  "found": "str",
  "type_context": {
    "in_expression": "a + b",
    "operand_types": ["int", "str"]
  }
}
```

**Reference errors include:**
```json
{
  "category": "reference",
  "unknown_name": "fetchData",
  "similar_names": ["fetch_data", "fetch_user"],
  "available_imports": ["std.http { fetch }"]
}
```

**Pattern errors include:**
```json
{
  "category": "pattern",
  "pattern_name": "retry",
  "missing_properties": [".op"],
  "invalid_properties": [".max_tries"],
  "valid_properties": [".op", ".attempts", ".backoff", ".on", ".jitter"]
}
```

---

## Semantic Addresses in Errors

Every error includes a semantic address, enabling direct use with the edit system.

### Address Benefits

```json
{
  "location": {
    "file": "src/api.si",
    "line": 15,
    "column": 10,
    "address": "@api.fetch_data.op"
  }
}
```

**AI workflow:**
1. Receive error with address `@api.fetch_data.op`
2. Generate edit: `{ "op": "set", "address": "@api.fetch_data.op", "value": "..." }`
3. No need to parse line/column or figure out context

### Address Granularity

Addresses are as specific as possible:

```json
// Error in pattern property
"address": "@api.fetch_data.attempts"

// Error in function parameter
"address": "@api.fetch_data.params.url"

// Error in specific validation rule
"address": "@validate_user.rules[2]"

// Error in struct field
"address": "type User.email"
```

---

## Fix Suggestions

Suggestions include ready-to-apply edit commands.

### Suggestion Structure

```json
{
  "suggestions": [
    {
      "message": "add missing field 'email'",
      "edit": {
        "op": "add",
        "address": "type User",
        "value": "email: str"
      },
      "confidence": "high"
    }
  ]
}
```

### Confidence Levels

| Level | Meaning | AI Action |
|-------|---------|-----------|
| `high` | Almost certainly correct | Apply automatically |
| `medium` | Likely correct, verify | Review before applying |
| `low` | One possibility among several | Present options to user |

### High-Confidence Examples

**Missing import:**
```json
{
  "id": "E0100",
  "message": "unknown identifier 'parse'",
  "suggestions": [{
    "message": "add import for std.json",
    "edit": {
      "op": "add",
      "address": "@api.imports",
      "value": "use std.json { parse }"
    },
    "confidence": "high"
  }]
}
```

**Typo in identifier:**
```json
{
  "id": "E0100",
  "message": "unknown identifier 'fecth_data'",
  "suggestions": [{
    "message": "did you mean 'fetch_data'?",
    "edit": {
      "op": "set",
      "address": "@process.body",
      "value": "fetch_data(url)"
    },
    "confidence": "high"
  }]
}
```

### Medium-Confidence Examples

**Type conversion:**
```json
{
  "id": "E0308",
  "message": "expected int, found str",
  "suggestions": [
    {
      "message": "convert str to int",
      "edit": {
        "op": "set",
        "address": "@calc.body",
        "value": "int(x) + y"
      },
      "confidence": "medium"
    }
  ]
}
```

### Low-Confidence Examples

**Multiple valid fixes:**
```json
{
  "id": "E0308",
  "message": "type mismatch in return",
  "suggestions": [
    {
      "message": "change return type to str",
      "edit": {
        "op": "set",
        "address": "@process.return_type",
        "value": "str"
      },
      "confidence": "low"
    },
    {
      "message": "convert result to int",
      "edit": {
        "op": "set",
        "address": "@process.body",
        "value": "int(result)"
      },
      "confidence": "low"
    }
  ]
}
```

### No Suggestion

Some errors cannot have automatic fixes:

```json
{
  "id": "E0500",
  "message": "circular dependency between modules",
  "suggestions": [],
  "note": "Restructure modules to break the cycle"
}
```

---

## Test Failure Output

Test failures include rich debugging information.

### Test Failure Structure

```json
{
  "id": "T0001",
  "category": "test",
  "severity": "error",
  "message": "assertion failed",
  "test": "@test_add",
  "target": "@add",
  "location": {
    "file": "src/_test/math.test.si",
    "line": 8,
    "address": "@test_add.assertions[0]"
  },
  "assertion": {
    "type": "assert_eq",
    "expected": "5",
    "expected_type": "int",
    "actual": "4",
    "actual_type": "int",
    "expression": "add(2, 2)",
    "diff": {
      "expected_repr": "5",
      "actual_repr": "4"
    }
  },
  "suggestions": [
    {
      "message": "fix expected value if test is wrong",
      "edit": {
        "op": "set",
        "address": "@test_add.assertions[0].expected",
        "value": "4"
      },
      "confidence": "low"
    },
    {
      "message": "check @add implementation",
      "edit": null,
      "confidence": "medium"
    }
  ]
}
```

### Assertion Types

**assert_eq:**
```json
{
  "type": "assert_eq",
  "expected": "[1, 2, 3]",
  "actual": "[1, 2]",
  "diff": {
    "missing": [3],
    "extra": []
  }
}
```

**assert:**
```json
{
  "type": "assert",
  "expression": "x > 0",
  "evaluated_to": false,
  "bindings": {
    "x": -5
  }
}
```

**assert_panics:**
```json
{
  "type": "assert_panics",
  "expression": "get([], 0)",
  "expected_panic": true,
  "did_panic": false,
  "returned": "0"
}
```

---

## Multiple Errors

### Primary vs Cascading

```json
{
  "errors": [
    {
      "id": "E0100",
      "message": "unknown type 'User'",
      "is_primary": true,
      "location": {
        "address": "@api.fetch_user.return_type"
      }
    },
    {
      "id": "E0308",
      "message": "cannot access field 'name' on unknown type",
      "is_primary": false,
      "caused_by": "E0100",
      "location": {
        "address": "@api.fetch_user.body"
      }
    },
    {
      "id": "E0308",
      "message": "cannot access field 'email' on unknown type",
      "is_primary": false,
      "caused_by": "E0100",
      "location": {
        "address": "@api.fetch_user.body"
      }
    }
  ]
}
```

**AI strategy:**
1. Filter to `is_primary: true` errors
2. Fix primary errors first
3. Re-run check - cascading errors often resolve
4. Address remaining errors

### Error Grouping

Related errors can be grouped:

```json
{
  "error_groups": [
    {
      "primary": {
        "id": "E0100",
        "message": "unknown type 'User'"
      },
      "cascading": [
        { "id": "E0308", "message": "cannot access field 'name'" },
        { "id": "E0308", "message": "cannot access field 'email'" }
      ],
      "suggestion": "Define type User or import it"
    }
  ]
}
```

---

## Deterministic Output

Errors are always sorted deterministically.

### Sort Order

1. By file path (alphabetically)
2. By line number (ascending)
3. By column number (ascending)
4. By error ID (alphabetically)

### Rationale

- Same input always produces same output
- AI can reliably compare before/after
- Enables automated testing of error handling
- No random ordering based on internal state

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success, no errors or warnings |
| 1 | Errors present |
| 2 | Warnings present (no errors) |
| 3 | Internal compiler error |
| 4 | Invalid arguments / usage error |

### Usage in Scripts

```bash
sigil check src/
if [ $? -eq 1 ]; then
    echo "Errors found, running fix loop"
    # AI fix loop here
fi
```

---

## Human-Readable Output

While JSON is the default for tooling, human-readable output is available.

### Enable Human Mode

```bash
sigil check src/ --human
```

### Human Format

```
error[E0308]: mismatched types
  --> src/main.si:15:10
   |
15 |     result = x + "hello"
   |              ^^^^^^^^^^^ expected int, found str
   |
   = help: convert int to str with str(x)
   = see: https://sigil-lang.org/errors/E0308

error[E0100]: unknown identifier
  --> src/main.si:20:5
   |
20 |     fetchData(url)
   |     ^^^^^^^^^ did you mean 'fetch_data'?
   |

error: 2 errors emitted
```

---

## AI Self-Correction Loop

### Complete Workflow

```bash
# 1. AI runs check
sigil check src/ > errors.json

# 2. AI parses errors, finds high-confidence fixes
cat errors.json | jq '.errors[] | select(.suggestions[0].confidence == "high")'

# 3. AI builds edit batch from suggestions
{
  "edits": [
    { "op": "add", "address": "@api.imports", "value": "use std.json { parse }" },
    { "op": "set", "address": "@process.body", "value": "fetch_data(url)" }
  ]
}

# 4. AI applies edits
sigil edit apply-json < edits.json

# 5. AI re-runs check
sigil check src/ > errors.json

# 6. Repeat until no errors or only low-confidence suggestions remain
```

### Iteration Limit

AI should limit fix iterations to prevent infinite loops:

```python
max_iterations = 10
for i in range(max_iterations):
    errors = run_check()
    if not errors:
        break
    high_confidence = [e for e in errors if has_high_confidence_fix(e)]
    if not high_confidence:
        # Only medium/low confidence fixes remain
        # Present to user for decision
        break
    apply_fixes(high_confidence)
```

---

## Coverage Errors

Missing test coverage produces structured errors.

### Coverage Error

```json
{
  "id": "C0001",
  "category": "coverage",
  "severity": "error",
  "message": "function @helper has no tests",
  "location": {
    "file": "src/utils.si",
    "line": 15,
    "address": "@utils.helper"
  },
  "function": {
    "name": "@helper",
    "signature": "(x: int) -> int",
    "complexity": "low"
  },
  "suggestions": [
    {
      "message": "generate test skeleton",
      "edit": {
        "op": "add",
        "address": "@utils._test.tests",
        "value": "@test_helper tests @helper () -> void = run(\n    assert_eq(helper(0), 0)\n)"
      },
      "confidence": "medium"
    }
  ]
}
```

---

## Batch Error Summary

For large projects, a summary helps prioritize.

```json
{
  "summary": {
    "total_errors": 15,
    "total_warnings": 3,
    "by_category": {
      "type": 8,
      "reference": 4,
      "coverage": 2,
      "syntax": 1
    },
    "by_file": {
      "src/api.si": 5,
      "src/handlers.si": 4,
      "src/utils.si": 3,
      "src/main.si": 3
    },
    "fixable": {
      "high_confidence": 6,
      "medium_confidence": 5,
      "low_confidence": 4
    }
  },
  "errors": [...]
}
```

---

## Error Documentation

Each error code has documentation explaining:
- What causes the error
- Common fixes
- Examples of incorrect and correct code

### Docs URL

```json
{
  "id": "E0308",
  "docs_url": "https://sigil-lang.org/errors/E0308"
}
```

### Error Index

```bash
sigil explain E0308
```

```
E0308: mismatched types

This error occurs when the type of an expression doesn't match what's expected.

Common causes:
- Arithmetic on mixed types (int + str)
- Wrong return type
- Pattern property has wrong type

Example (incorrect):
    @add (a: int, b: str) -> int = a + b
                                   ^^^^^ str is not int

Example (correct):
    @add (a: int, b: int) -> int = a + b

See also: Type system docs, Type conversion
```

---

## Summary

| Field | Purpose |
|-------|---------|
| `id` | Unique error code |
| `severity` | error / warning / info |
| `category` | Route to appropriate handler |
| `message` | Short description |
| `location` | File, line, column, and semantic address |
| `expected` / `found` | For type errors |
| `context` | Expression and surrounding code |
| `suggestions` | Ready-to-apply fixes with confidence |
| `related` | Connected locations |
| `is_primary` | True if root cause |
| `caused_by` | What caused this (for cascading) |

| Feature | Benefit |
|---------|---------|
| JSON format | Machine-parseable |
| Semantic addresses | Direct link to edit system |
| Confidence levels | AI knows what to auto-fix |
| Primary/cascading | Fix root causes first |
| Deterministic order | Reliable comparisons |
| Exit codes | Scriptable workflows |

---

## See Also

- [Semantic Addressing](01-semantic-addressing.md) - Address syntax in errors
- [Edit Operations](02-edit-operations.md) - Applying fix suggestions
- [Testing](../11-testing/index.md) - Test failure details
- [Refactoring API](07-refactoring-api.md) - Complex fixes
