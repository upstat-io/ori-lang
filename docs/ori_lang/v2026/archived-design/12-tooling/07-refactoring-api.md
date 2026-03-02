# Refactoring API

Ori provides a first-class refactoring API that both editors and AI can invoke directly. Instead of regenerating entire files, AI can request precise transformations like "rename this" or "extract this function."

---

## Design Principles

1. **Standalone API** - Not buried in LSP, accessible to any tool
2. **JSON in, JSON out** - Structured interface for AI
3. **Validated before applied** - No partial or broken transformations
4. **Transactional batches** - Multiple operations succeed or fail together
5. **Dry-run support** - Preview changes before applying
6. **Undo support** - Every operation is reversible

---

## Architecture

```
+-------------+     +-----------------+     +-------------+
|   AI/CLI    |---->|  Refactor API   |<----|    LSP      |
+-------------+     +-----------------+     +-------------+
                           |
                           v
                    +-------------+
                    |  Compiler   |
                    |  (validate) |
                    +-------------+
```

The refactoring API is a standalone service that:
- Accepts refactoring requests (CLI or JSON)
- Validates transformations against the compiler
- Returns structured results
- Is consumed by both AI tools and the LSP

---

## CLI Interface

```bash
# Rename
ori refactor rename @old_name @new_name

# Extract function
ori refactor extract-function @module.func.body[3:5] --name @helper

# Inline function
ori refactor inline @helper

# Change signature
ori refactor change-signature @fetch_data --add-param "timeout: int = 30"

# Move to different module
ori refactor move @api.helper --to utils

# Generate test
ori refactor generate-test @new_function

# Dry run (preview)
ori refactor rename @old @new --dry-run

# Undo
ori refactor undo <checkpoint-id>
```

---

## JSON Interface

```json
{
  "operation": "rename",
  "target": "@api.fetch_data",
  "new_name": "fetch_user_data",
  "options": {
    "dry_run": false,
    "include_comments": true
  }
}
```

### Response Format

```json
{
  "status": "success",
  "operation": "rename",
  "checkpoint_id": "rf_20240115_143022",
  "changes": [
    {
      "file": "src/api.ori",
      "line": 42,
      "type": "definition",
      "before": "@fetch_data",
      "after": "@fetch_user_data"
    },
    {
      "file": "src/handlers.ori",
      "line": 23,
      "type": "call_site",
      "before": "fetch_data(url)",
      "after": "fetch_user_data(url)"
    }
  ],
  "files_modified": 3,
  "locations_updated": 12
}
```

---

## Core Operations

### Overview

| Operation | Description | AI Use Case |
|-----------|-------------|-------------|
| `rename` | Rename across codebase | Fix naming after generation |
| `extract-function` | Expression to function | Reduce duplication |
| `inline-function` | Replace calls with body | Simplify over-abstraction |
| `extract-variable` | Name a subexpression | Clarify complex expressions |
| `inline-variable` | Replace with value | Remove unnecessary bindings |
| `change-signature` | Modify parameters | Evolve APIs |
| `move` | Relocate to module | Fix organization |
| `convert-pattern` | Change pattern type | Optimize pattern choice |
| `generate-test` | Create test skeleton | Ensure coverage |

---

## Rename

Rename an identifier across the entire codebase.

### Request

```json
{
  "operation": "rename",
  "target": "@api.fetch_data",
  "new_name": "fetch_user_data"
}
```

### What Gets Updated

- Function definition
- All call sites
- Test declarations (`tests @old` becomes `tests @new`)
- Comments mentioning the name (optional)
- Re-exports
- Documentation references

### Response

```json
{
  "status": "success",
  "operation": "rename",
  "checkpoint_id": "rf_20240115_143022",
  "old_name": "@api.fetch_data",
  "new_name": "@api.fetch_user_data",
  "changes": [
    {"file": "src/api.ori", "line": 42, "type": "definition"},
    {"file": "src/handlers.ori", "line": 23, "type": "call_site"},
    {"file": "src/handlers.ori", "line": 45, "type": "call_site"},
    {"file": "src/_test/api.test.ori", "line": 8, "type": "test_declaration"},
    {"file": "src/_test/api.test.ori", "line": 21, "type": "test_declaration"}
  ],
  "files_modified": 3,
  "locations_updated": 5
}
```

### Options

```json
{
  "operation": "rename",
  "target": "@api.fetch_data",
  "new_name": "fetch_user_data",
  "options": {
    "include_comments": true,  // Update mentions in comments
    "include_strings": false   // Don't update strings (dangerous)
  }
}
```

---

## Extract Function

Extract an expression or statement range into a new function.

### Request

```json
{
  "operation": "extract-function",
  "target": "@process.body.doubled",
  "new_name": "@double_all"
}
```

### Before

```ori
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: (accumulator, value) -> accumulator + value,
    ),
)
```

### After

```ori
@double_all (items: [int]) -> [int] = map(
    .over: items,
    .transform: item -> item * 2,
)

@process (items: [int]) -> int = run(
    let doubled = double_all(
        .items: items,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: (accumulator, value) -> accumulator + value,
    ),
)
```

### Inference

The API automatically:
- Detects free variables (become parameters)
- Infers parameter types
- Infers return type
- Detects captured variables (become parameters)
- Places function appropriately

### Range Extraction

Extract multiple statements:

```json
{
  "operation": "extract-function",
  "target": "@process.body[0:2]",
  "new_name": "@prepare_data"
}
```

Extracts statements 0 and 1 into a new function.

---

## Inline Function

Replace all calls to a function with its body.

### Request

```json
{
  "operation": "inline-function",
  "target": "@double_all"
}
```

### Before

```ori
@double_all (items: [int]) -> [int] = map(
    .over: items,
    .transform: item -> item * 2,
)

@process (items: [int]) -> int = run(
    let doubled = double_all(
        .items: items,
    ),
    fold(
        .over: doubled,
        .initial: 0,
        .operation: (accumulator, value) -> accumulator + value,
    ),
)
```

### After

```ori
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    fold(
        .over: doubled,
        .initial: 0,
        .operation: (accumulator, value) -> accumulator + value,
    ),
)
```

### Options

```json
{
  "operation": "inline-function",
  "target": "@helper",
  "options": {
    "delete_definition": true,  // Remove the function after inlining
    "inline_single_use": true   // Only inline if used once
  }
}
```

---

## Extract Variable

Name a subexpression for clarity.

### Request

```json
{
  "operation": "extract-variable",
  "target": "@process.body.filter[1]",
  "new_name": "threshold"
}
```

### Before

```ori
@process (items: [int]) -> [int] = filter(
    .over: items,
    .predicate: item -> item > 10,
)
```

### After

```ori
@process (items: [int]) -> [int] = run(
    let threshold = 10,
    filter(
        .over: items,
        .predicate: item -> item > threshold,
    ),
)
```

---

## Inline Variable

Replace a variable with its value.

### Request

```json
{
  "operation": "inline-variable",
  "target": "@process.body.threshold"
}
```

### Before

```ori
@process (items: [int]) -> [int] = run(
    let threshold = 10,
    filter(
        .over: items,
        .predicate: item -> item > threshold,
    ),
)
```

### After

```ori
@process (items: [int]) -> [int] = filter(
    .over: items,
    .predicate: item -> item > 10,
)
```

---

## Change Signature

Modify function parameters.

### Add Parameter

```json
{
  "operation": "change-signature",
  "target": "@fetch_data",
  "add_param": {
    "name": "timeout",
    "type": "int",
    "default": "30",
    "position": "last"
  }
}
```

**Updates all call sites** with the default value or requires manual updates.

### Remove Parameter

```json
{
  "operation": "change-signature",
  "target": "@fetch_data",
  "remove_param": "timeout"
}
```

### Reorder Parameters

```json
{
  "operation": "change-signature",
  "target": "@fetch_data",
  "reorder_params": ["url", "headers", "timeout"]
}
```

### Change Parameter Type

```json
{
  "operation": "change-signature",
  "target": "@fetch_data",
  "change_param": {
    "name": "timeout",
    "new_type": "Duration"
  }
}
```

### Response

```json
{
  "status": "success",
  "operation": "change-signature",
  "changes": [
    {"file": "src/api.ori", "type": "definition_updated"},
    {"file": "src/handlers.ori", "line": 23, "type": "call_site_updated"},
    {"file": "src/handlers.ori", "line": 45, "type": "call_site_updated"}
  ],
  "manual_review_needed": [
    {"file": "src/dynamic.ori", "line": 12, "reason": "dynamic call, cannot verify"}
  ]
}
```

---

## Move

Move elements between modules.

### Request

```json
{
  "operation": "move",
  "target": "@api.helper",
  "to": "utils"
}
```

### What Gets Updated

- Function definition moved to target module
- Import added at original location (if still referenced there)
- Imports added at all usage sites
- Re-exports updated

### Response

```json
{
  "status": "success",
  "operation": "move",
  "element": "@api.helper",
  "from": "src/api.ori",
  "to": "src/utils.ori",
  "changes": [
    {"file": "src/api.ori", "type": "removed_definition"},
    {"file": "src/api.ori", "type": "added_import", "import": "use utils { helper }"},
    {"file": "src/utils.ori", "type": "added_definition"},
    {"file": "src/handlers.ori", "type": "updated_import"}
  ]
}
```

### Move Type

```json
{
  "operation": "move",
  "target": "type api.Response",
  "to": "types"
}
```

---

## Convert Pattern

Transform code to use a different pattern.

### Suggest Improvements

```json
{
  "operation": "suggest",
  "target": "@process"
}
```

### Response

```json
{
  "suggestions": [
    {
      "location": "@process.body",
      "current": "manual recursion",
      "suggested": "recurse pattern with .memo",
      "reason": "detected recursive structure with repeated subproblems",
      "command": {
        "operation": "convert-pattern",
        "target": "@process.body",
        "to": "recurse",
        "options": { "memo": true }
      },
      "confidence": "high"
    }
  ]
}
```

### Apply Conversion

```json
{
  "operation": "convert-pattern",
  "target": "@fib.body",
  "to": "recurse",
  "options": { "memo": true }
}
```

### Before

```ori
@fib (number: int) -> int =
    if number <= 1 then number
    else fib(number - 1) + fib(number - 2)
```

### After

```ori
@fib (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: number,
    .step: self(number - 1) + self(number - 2),
    .memo: true
)
```

---

## Generate Test

Create test skeletons based on function analysis.

### Request

```json
{
  "operation": "generate-test",
  "target": "@fetch_data"
}
```

### Response

```json
{
  "status": "success",
  "generated_tests": [
    {
      "name": "@test_fetch_data_success",
      "code": "@test_fetch_data_success tests @fetch_data () -> void = run(\n    let result = fetch_data(\n        .url: \"https://example.com\",\n    ),\n    assert_ok(\n        .result: result,\n    ),\n)"
    },
    {
      "name": "@test_fetch_data_error",
      "code": "@test_fetch_data_error tests @fetch_data () -> void = run(\n    let result = fetch_data(\n        .url: \"invalid-url\",\n    ),\n    assert_err(\n        .result: result,\n    ),\n)"
    }
  ],
  "analysis": {
    "return_type": "Result<Data, Error>",
    "pattern": "retry",
    "suggested_cases": ["success", "error", "retry_exhausted"]
  }
}
```

### Pattern-Aware Generation

For functions using patterns, generates pattern-specific tests:

**Retry pattern:**
```ori
@test_fetch_retry_exhausted tests @fetch_data () -> void = run(
    // Mock to always fail
    let result = fetch_data(
        .url: "always-fail",
    ),
    assert_err(
        .result: result,
    ),
    assert_eq(
        .actual: error_retries(
            .result: result,
        ),
        .expected: 3,
    ),
)
```

**Validate pattern:**
```ori
@test_validate_all_rules tests @validate_user () -> void = run(
    // Test all validation rules are checked
    let result = validate_user(
        .user: { name: "", age: -1, email: "bad" },
    ),
    assert_err(
        .result: result,
    ),
    assert_eq(
        .actual: len(
            .collection: errors(
                .result: result,
            ),
        ),
        .expected: 3,
    ),
)
```

---

## Batch Operations

Execute multiple refactoring operations as a transaction.

### Request

```json
{
  "operations": [
    {"op": "extract-function", "target": "@process.body[0]", "new_name": "@helper"},
    {"op": "move", "target": "@helper", "to": "utils"},
    {"op": "rename", "target": "@helper", "new_name": "@format_data"}
  ],
  "transaction": true
}
```

### Behavior

- All operations validated before any applied
- If any fails, none apply
- Returns complete diff of all changes
- Single checkpoint for undo

### Response

```json
{
  "status": "success",
  "checkpoint_id": "rf_20240115_143022",
  "operations_completed": 3,
  "total_changes": [
    {"file": "src/process.ori", "changes": 2},
    {"file": "src/utils.ori", "changes": 1}
  ]
}
```

---

## Dry Run and Preview

Preview changes without applying them.

### Request

```json
{
  "operation": "rename",
  "target": "@old",
  "new_name": "@new",
  "options": { "dry_run": true }
}
```

### Response

```json
{
  "dry_run": true,
  "would_change": [
    {
      "file": "src/api.ori",
      "line": 42,
      "before": "@old (value: int) -> int = ...",
      "after": "@new (value: int) -> int = ..."
    },
    {
      "file": "src/main.ori",
      "line": 15,
      "before": "result = old(5)",
      "after": "result = new(5)"
    }
  ],
  "validation": "passed",
  "safe_to_apply": true
}
```

### AI Workflow

```bash
# 1. Generate refactoring command
# 2. Dry run to preview
ori refactor rename @old @new --dry-run

# 3. Verify changes look correct
# 4. Apply
ori refactor rename @old @new
```

---

## Undo Support

Every refactoring creates a checkpoint that can be undone.

### Checkpoint Creation

```json
{
  "status": "success",
  "checkpoint_id": "rf_20240115_143022",
  ...
}
```

### Undo Request

```bash
ori refactor undo rf_20240115_143022
```

Or JSON:
```json
{
  "operation": "undo",
  "checkpoint_id": "rf_20240115_143022"
}
```

### Undo Response

```json
{
  "status": "success",
  "operation": "undo",
  "checkpoint_id": "rf_20240115_143022",
  "restored_files": 3,
  "changes_reverted": 12
}
```

### List Checkpoints

```bash
ori refactor history
```

```json
{
  "checkpoints": [
    {
      "id": "rf_20240115_143022",
      "operation": "rename @old @new",
      "timestamp": "2024-01-15T14:30:22Z",
      "files_affected": 3
    },
    {
      "id": "rf_20240115_142815",
      "operation": "extract-function @process.body[0]",
      "timestamp": "2024-01-15T14:28:15Z",
      "files_affected": 1
    }
  ]
}
```

---

## Conflict Detection

Refactoring operations detect conflicts upfront.

### Name Conflict

```json
{
  "operation": "rename",
  "target": "@helper",
  "new_name": "@process"
}
```

**Response:**
```json
{
  "status": "error",
  "error": {
    "type": "conflict",
    "message": "@process already exists in module api",
    "conflicting_element": {
      "address": "@api.process",
      "file": "src/api.ori",
      "line": 25
    }
  },
  "suggestions": [
    "Use different name: @process_helper",
    "Move existing @process first",
    "Use fully qualified name"
  ]
}
```

### Reference Conflict

```json
{
  "operation": "move",
  "target": "@utils.helper",
  "to": "api"
}
```

**Response (circular dependency detected):**
```json
{
  "status": "error",
  "error": {
    "type": "circular_dependency",
    "message": "Moving @utils.helper to api would create circular dependency",
    "cycle": ["api", "handlers", "utils", "api"]
  },
  "suggestions": [
    "Create new module for shared code",
    "Inline the function instead"
  ]
}
```

---

## LSP Integration

The LSP's code actions invoke the refactoring API.

### Code Action Flow

1. User selects code and requests "Extract Function"
2. LSP translates selection to semantic address
3. LSP calls refactoring API with address
4. Refactoring API returns changes
5. LSP applies as workspace edit

### Same Operations

Editor users and AI get identical refactoring capabilities through the same API.

### Code Action Example

```
User: Selects "map(items, item -> item * 2)" in editor
Editor: Shows lightbulb with "Extract to function"
User: Clicks action
LSP: POST /refactor {
  "operation": "extract-function",
  "target": "@process.body.doubled",
  "new_name": "@double_all"
}
Refactoring API: Returns changes
LSP: Applies workspace edit
```

---

## Error Handling

### Validation Errors

```json
{
  "status": "error",
  "error": {
    "type": "validation",
    "message": "Cannot extract: expression contains side effects",
    "location": "@process.body[2]",
    "details": "Expression calls @mutate_state which has side effects"
  }
}
```

### Type Errors

```json
{
  "status": "error",
  "error": {
    "type": "type_error",
    "message": "Extracted function would have ambiguous return type",
    "inferred_types": ["int", "str"],
    "suggestion": "Add explicit type annotation"
  }
}
```

### Reference Errors

```json
{
  "status": "error",
  "error": {
    "type": "reference",
    "message": "Target address not found",
    "address": "@api.nonexistent",
    "similar": ["@api.fetch_data", "@api.process"]
  }
}
```

---

## Summary

| Operation | Purpose | Key Features |
|-----------|---------|--------------|
| `rename` | Change identifier | Cross-codebase, includes tests |
| `extract-function` | Create function from expression | Infers params, types |
| `inline-function` | Replace calls with body | Optionally delete definition |
| `extract-variable` | Name subexpression | Creates run block if needed |
| `inline-variable` | Replace with value | Simplifies bindings |
| `change-signature` | Modify parameters | Updates all call sites |
| `move` | Relocate element | Manages imports |
| `convert-pattern` | Change pattern usage | Suggests optimizations |
| `generate-test` | Create test skeleton | Pattern-aware |

| Feature | Description |
|---------|-------------|
| Standalone API | Used by CLI, LSP, and AI |
| JSON interface | Structured for AI |
| Dry run | Preview before applying |
| Transactions | Atomic batches |
| Undo | Checkpoint-based reversal |
| Conflict detection | Upfront validation |

---

## See Also

- [Semantic Addressing](01-semantic-addressing.md) - Target addresses
- [Edit Operations](02-edit-operations.md) - Lower-level edits
- [Structured Errors](03-structured-errors.md) - Error format
- [LSP](05-lsp.md) - Editor integration
- [Testing](../11-testing/index.md) - Generated tests
