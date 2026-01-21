# Edit Operations

Edit operations enable targeted modifications to Sigil code using semantic addresses. Instead of regenerating entire files, AI makes precise changes to specific elements.

---

## Design Principles

1. **Semantic, not textual** - Edit code by meaning, not character positions
2. **Atomic batches** - Multiple edits succeed or fail together
3. **Validated before applied** - Type-check and parse-check before modifying
4. **Reversible** - Every edit can be undone

---

## Operations Overview

| Operation | Purpose | Example |
|-----------|---------|---------|
| `set` | Replace a value | Change retry attempts from 3 to 5 |
| `add` | Add new element | Add field to struct |
| `remove` | Delete element | Remove unused parameter |
| `rename` | Change identifier | Rename function across codebase |
| `move` | Reorder or relocate | Move function to different module |

---

## Set Operation

Replace the value at a semantic address.

### Syntax

```json
{
  "op": "set",
  "address": "<semantic-address>",
  "value": "<new-value>"
}
```

### Examples

**Change a pattern property:**
```json
{
  "op": "set",
  "address": "@api.fetch_data.attempts",
  "value": "5"
}
```

Before:
```sigil
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

After:
```sigil
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 5,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

**Change a config value:**
```json
{
  "op": "set",
  "address": "$config.timeout",
  "value": "60"
}
```

**Change a function body:**
```json
{
  "op": "set",
  "address": "@math.add.body",
  "value": "a + b + 1"
}
```

**Change a type field:**
```json
{
  "op": "set",
  "address": "type User.email",
  "value": "email: Option<str>"
}
```

**Change a parameter type:**
```json
{
  "op": "set",
  "address": "@process.params.count",
  "value": "count: int"
}
```

### Validation

The set operation validates:
1. Address exists (unless using `create_if_missing: true`)
2. New value parses correctly
3. New value type-checks in context
4. Result compiles

---

## Add Operation

Add a new element to a collection or structure.

### Syntax

```json
{
  "op": "add",
  "address": "<parent-address>",
  "value": "<new-element>",
  "position": "<position>"  // optional: "first", "last", "before:<ref>", "after:<ref>"
}
```

### Adding Struct Fields

```json
{
  "op": "add",
  "address": "type User",
  "value": "verified: bool"
}
```

Before:
```sigil
type User = {
    id: int,
    name: str,
    email: str
}
```

After:
```sigil
type User = {
    id: int,
    name: str,
    email: str,
    verified: bool
}
```

### Adding Sum Type Variants

```json
{
  "op": "add",
  "address": "type Status",
  "value": "Cancelled(reason: str)"
}
```

Before:
```sigil
type Status = Pending | Running(progress: float) | Done
```

After:
```sigil
type Status = Pending | Running(progress: float) | Done | Cancelled(reason: str)
```

### Adding Function Parameters

```json
{
  "op": "add",
  "address": "@fetch_data.params",
  "value": "timeout: int = 30",
  "position": "last"
}
```

### Adding Import Items

```json
{
  "op": "add",
  "address": "@api.imports[\"std.http\"]",
  "value": "post"
}
```

Before:
```sigil
use std.http { get }
```

After:
```sigil
use std.http { get, post }
```

### Adding Validation Rules

```json
{
  "op": "add",
  "address": "@validate_user.rules",
  "value": "input.phone.len >= 10 | \"phone required\"",
  "position": "after:\"email\""
}
```

### Position Options

| Position | Meaning |
|----------|---------|
| `"first"` | Add at beginning |
| `"last"` | Add at end (default) |
| `"before:ref"` | Add before element matching ref |
| `"after:ref"` | Add after element matching ref |
| `"at:n"` | Add at specific index |

---

## Remove Operation

Delete an element from the code.

### Syntax

```json
{
  "op": "remove",
  "address": "<element-address>"
}
```

### Removing Struct Fields

```json
{
  "op": "remove",
  "address": "type User.legacy_id"
}
```

Before:
```sigil
type User = {
    id: int,
    legacy_id: int,
    name: str
}
```

After:
```sigil
type User = {
    id: int,
    name: str
}
```

### Removing Function Parameters

```json
{
  "op": "remove",
  "address": "@fetch_data.params.timeout"
}
```

### Removing Functions

```json
{
  "op": "remove",
  "address": "@deprecated_helper"
}
```

### Removing Pattern Properties

```json
{
  "op": "remove",
  "address": "@fetch_data.jitter"
}
```

### Validation

Remove operations validate:
1. Element exists
2. Removal does not break references (unless `force: true`)
3. Result still compiles

**Reference check failure:**
```json
{
  "error": "remove_would_break_references",
  "address": "@helper",
  "references": [
    { "file": "src/main.si", "line": 23, "expression": "helper(x)" },
    { "file": "src/utils.si", "line": 8, "expression": "helper(y)" }
  ],
  "suggestion": "Remove references first, or use force: true"
}
```

---

## Rename Operation

Rename an identifier across the entire codebase.

### Syntax

```json
{
  "op": "rename",
  "address": "<element-address>",
  "new_name": "<new-identifier>"
}
```

### Renaming Functions

```json
{
  "op": "rename",
  "address": "@api.fetch",
  "new_name": "fetch_user_data"
}
```

**Updates:**
- Function definition
- All call sites
- Test declarations (`tests @old` becomes `tests @new`)
- Documentation references
- Re-exports

### Renaming Types

```json
{
  "op": "rename",
  "address": "type User",
  "new_name": "UserAccount"
}
```

**Updates:**
- Type definition
- All type annotations
- Constructor calls
- Pattern matches

### Renaming Fields

```json
{
  "op": "rename",
  "address": "type User.email",
  "new_name": "email_address"
}
```

**Updates:**
- Field definition
- All field accesses
- Destructuring patterns
- Named property syntax

### Renaming Config

```json
{
  "op": "rename",
  "address": "$timeout",
  "new_name": "request_timeout"
}
```

### Rename Output

```json
{
  "operation": "rename",
  "old_name": "@fetch",
  "new_name": "@fetch_user_data",
  "changes": [
    { "file": "src/api.si", "line": 42, "type": "definition" },
    { "file": "src/handlers.si", "line": 23, "type": "call_site" },
    { "file": "src/handlers.si", "line": 45, "type": "call_site" },
    { "file": "src/_test/api.test.si", "line": 8, "type": "test_declaration" }
  ],
  "files_modified": 3,
  "locations_updated": 4
}
```

---

## Move Operation

Relocate elements within or between modules.

### Syntax

```json
{
  "op": "move",
  "address": "<element-address>",
  "to": "<destination>"
}
```

### Moving Functions Between Modules

```json
{
  "op": "move",
  "address": "@api.helper",
  "to": "utils"
}
```

**Handles:**
- Moves function definition
- Adds import at original location if still referenced
- Adds imports at all usage sites
- Updates re-exports

### Reordering Within Module

```json
{
  "op": "move",
  "address": "@helper",
  "position": "after:@main"
}
```

### Moving Types

```json
{
  "op": "move",
  "address": "type api.Response",
  "to": "types"
}
```

### Move Output

```json
{
  "operation": "move",
  "element": "@api.helper",
  "from": "src/api.si",
  "to": "src/utils.si",
  "changes": [
    { "file": "src/api.si", "type": "removed_definition" },
    { "file": "src/api.si", "type": "added_import", "import": "use utils { helper }" },
    { "file": "src/utils.si", "type": "added_definition" },
    { "file": "src/handlers.si", "type": "updated_import" }
  ]
}
```

---

## JSON Interface

The primary interface for edit operations is JSON, optimized for AI generation.

### Single Edit

```json
{
  "edit": {
    "op": "set",
    "address": "@fetch.attempts",
    "value": "5"
  }
}
```

### Batch Edits

```json
{
  "edits": [
    { "op": "set", "address": "@fetch.attempts", "value": "5" },
    { "op": "add", "address": "type User", "value": "verified: bool" },
    { "op": "rename", "address": "@old_func", "new_name": "new_func" }
  ],
  "transaction": true
}
```

### Request Options

```json
{
  "edits": [...],
  "options": {
    "dry_run": false,        // Preview without applying
    "format": true,          // Auto-format after edit
    "validate": true,        // Type-check result
    "transaction": true      // All-or-nothing
  }
}
```

---

## CLI Interface

For human use and scripting:

### Single Operations

```bash
# Set operation
sigil edit set @fetch.attempts 5

# Add operation
sigil edit add "type User" "verified: bool"

# Remove operation
sigil edit remove "type User.legacy_id"

# Rename operation
sigil edit rename @old_func @new_func

# Move operation
sigil edit move @helper --to utils
```

### From JSON File

```bash
sigil edit apply edits.json
```

### From Stdin

```bash
echo '{"op":"set","address":"@fetch.attempts","value":"5"}' | sigil edit apply -
```

### Options

```bash
sigil edit set @fetch.attempts 5 --dry-run    # Preview only
sigil edit set @fetch.attempts 5 --no-format  # Skip auto-format
sigil edit set @fetch.attempts 5 --force      # Skip validation
```

---

## Validation and Safety

### Pre-Edit Validation

Before applying any edit:

1. **Address resolution** - Verify address exists and is unambiguous
2. **Parse check** - New value must be syntactically valid
3. **Type check** - New value must type-check in context
4. **Reference check** - For remove/rename, check affected references

### Validation Errors

**Address not found:**
```json
{
  "error": "address_not_found",
  "address": "@api.fetch_data.retries",
  "suggestion": "Did you mean @api.fetch_data.attempts?"
}
```

**Parse error:**
```json
{
  "error": "parse_error",
  "address": "@fetch.body",
  "value": "a + + b",
  "message": "unexpected token '+' at position 4"
}
```

**Type error:**
```json
{
  "error": "type_error",
  "address": "@fetch.attempts",
  "value": "\"five\"",
  "expected": "int",
  "found": "str"
}
```

### Post-Edit Validation

After constructing the edit:

1. **Full parse** - Entire file must parse
2. **Full type-check** - Entire file must type-check
3. **Test discovery** - Tests must still be discoverable

---

## Atomic Batches

Multiple edits in a batch are transactional.

### Transaction Behavior

```json
{
  "edits": [
    { "op": "rename", "address": "@old", "new_name": "new" },
    { "op": "set", "address": "@new.attempts", "value": "5" }
  ],
  "transaction": true
}
```

**Guarantee:** Either all edits apply, or none do.

### Validation Order

1. All edits validated individually
2. Edits applied in order to temporary state
3. Final state validated
4. If all pass, write to disk
5. If any fail, no changes written

### Rollback on Failure

```json
{
  "status": "failed",
  "applied": 0,
  "failed_at": 2,
  "error": {
    "op": "set",
    "address": "@new.attempts",
    "message": "address not found after rename"
  },
  "rollback": "complete"
}
```

---

## Dry Run Mode

Preview edits without applying them.

### Request

```json
{
  "edits": [...],
  "options": { "dry_run": true }
}
```

### Response

```json
{
  "dry_run": true,
  "would_change": [
    {
      "file": "src/api.si",
      "changes": [
        {
          "line": 42,
          "before": ".attempts: 3,",
          "after": ".attempts: 5,"
        }
      ]
    }
  ],
  "validation": "passed",
  "safe_to_apply": true
}
```

### CLI

```bash
sigil edit set @fetch.attempts 5 --dry-run
```

---

## Auto-Format Integration

Edit operations automatically format affected code.

### Behavior

1. Edit applied to AST
2. Formatter runs on modified elements
3. Result written with canonical formatting

### Disable Formatting

```json
{
  "edits": [...],
  "options": { "format": false }
}
```

Or CLI:
```bash
sigil edit set @fetch.attempts 5 --no-format
```

### Rationale

- Edits always produce canonical code
- No separate format step needed
- AI's semantic edits are always well-formatted

See [Formatter](04-formatter.md) for formatting rules.

---

## Error Handling

### Error Response Format

```json
{
  "status": "error",
  "error": {
    "code": "E_EDIT_001",
    "type": "address_not_found",
    "message": "Cannot resolve address @api.fetch_data.retries",
    "address": "@api.fetch_data.retries",
    "suggestions": [
      {
        "message": "Did you mean attempts?",
        "address": "@api.fetch_data.attempts"
      }
    ]
  }
}
```

### Error Codes

| Code | Type | Description |
|------|------|-------------|
| `E_EDIT_001` | `address_not_found` | Address does not exist |
| `E_EDIT_002` | `ambiguous_address` | Multiple matches |
| `E_EDIT_003` | `parse_error` | Value syntax error |
| `E_EDIT_004` | `type_error` | Value type mismatch |
| `E_EDIT_005` | `reference_error` | Would break references |
| `E_EDIT_006` | `conflict` | Name already exists |
| `E_EDIT_007` | `invalid_position` | Position specifier invalid |
| `E_EDIT_008` | `read_only` | Element cannot be modified |

---

## AI Workflow Example

### Traditional Edit (Rewrite Whole Function)

AI regenerates entire function to change one property:

```sigil
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 5,  // changed from 3
    .backoff: exponential(base: 100ms, max: 5s)
)
```

**Problems:**
- Many tokens for small change
- Risk of errors in unchanged code
- Intent unclear in diff

### Semantic Edit

```json
{ "op": "set", "address": "@api.fetch_data.attempts", "value": "5" }
```

**Benefits:**
- One line, clear intent
- Cannot accidentally change other code
- Edit command is the diff
- Fewer tokens, lower cost

### Complete AI Workflow

```bash
# 1. AI runs check, gets error with suggestion
sigil check src/ --json > errors.json

# 2. Error includes ready-to-apply edit:
{
  "suggestions": [{
    "edit": { "op": "add", "address": "@api.imports", "value": "use std.json { parse }" },
    "confidence": "high"
  }]
}

# 3. AI applies high-confidence fix
sigil edit apply-json '{"op":"add","address":"@api.imports","value":"use std.json { parse }"}'

# 4. AI re-runs check
sigil check src/ --json > errors.json

# 5. Repeat until zero errors
```

---

## Summary

| Operation | Purpose | Key Options |
|-----------|---------|-------------|
| `set` | Replace value | `create_if_missing` |
| `add` | Add element | `position` |
| `remove` | Delete element | `force` |
| `rename` | Change name | Updates all references |
| `move` | Relocate | `position`, `to` |

| Feature | Description |
|---------|-------------|
| JSON interface | Primary format for AI |
| CLI interface | For humans and scripts |
| Dry run | Preview without applying |
| Atomic batches | All-or-nothing transactions |
| Auto-format | Canonical output always |
| Validation | Parse + type check before apply |

---

## See Also

- [Semantic Addressing](01-semantic-addressing.md) - Address syntax
- [Structured Errors](03-structured-errors.md) - Error format with edit suggestions
- [Formatter](04-formatter.md) - Auto-formatting rules
- [Refactoring API](07-refactoring-api.md) - Higher-level refactoring operations
