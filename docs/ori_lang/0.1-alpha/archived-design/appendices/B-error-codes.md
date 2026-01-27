# Appendix B: Error Codes

This appendix catalogs all error codes produced by the Ori compiler.

---

## Error Code Format

All Ori errors follow a structured format:

```
E[Category][Number]
```

- **Category**: Single letter indicating error type
- **Number**: Three-digit identifier within category

---

## Categories

| Prefix | Category | Description |
|--------|----------|-------------|
| `ET` | Type | Type system errors |
| `EP` | Parse | Syntax and parsing errors |
| `EN` | Name | Name resolution errors |
| `EM` | Match | Pattern matching errors |
| `EF` | Function | Function-related errors |
| `EC` | Config | Configuration errors |
| `EI` | Import | Module and import errors |
| `EA` | Async | Async capability errors |
| `ES` | Test | Testing errors |
| `EW` | Warning | Warnings (non-fatal) |

---

## Type Errors (ET)

### ET001: Type Mismatch

**Description:** Expected one type but found another.

```ori
@add (left: int, right: int) -> int = left + right
// ET001: expected int, found str
add(
    .left: "hello",
    .right: 5,
)
```

**Fix:** Ensure the value matches the expected type.

---

### ET002: Unknown Type

**Description:** Referenced type does not exist.

```ori
// ET002: unknown type 'Foo'
@process (input: Foo) -> void = ...
```

**Fix:** Define the type or import it.

---

### ET003: Generic Arity Mismatch

**Description:** Wrong number of type arguments for generic type.

```ori
type Pair<A, B> = { first: A, second: B }
// ET003: expected 2 type arguments, found 1
x: Pair<int> = ...
```

**Fix:** Provide the correct number of type arguments.

---

### ET004: Cannot Infer Type

**Description:** Type cannot be determined from context.

```ori
// ET004: cannot infer element type of empty list
x = []
```

**Fix:** Add type annotation: `x: [int] = []`

---

### ET005: Trait Bound Not Satisfied

**Description:** Type doesn't implement required trait.

```ori
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
// ET005: User does not implement Comparable
sort(
    .items: [User{}, User{}],
)
```

**Fix:** Implement the trait for the type.

---

### ET006: Incompatible Types in Binary Operation

**Description:** Operator cannot be applied to given types.

```ori
// ET006: cannot subtract int from str
"hello" - 5
```

**Fix:** Use compatible types for the operator.

---

### ET007: Missing Field

**Description:** Struct literal missing required field.

```ori
type Point = { x: int, y: int }
// ET007: missing field 'y'
p = Point { x: 10 }
```

**Fix:** Provide all required fields.

---

### ET008: Unknown Field

**Description:** Referenced field doesn't exist on type.

```ori
type Point = { x: int, y: int }
// ET008: no field 'z' on type Point
p.z
```

**Fix:** Use a field that exists on the type.

---

### ET009: Duplicate Field

**Description:** Field specified multiple times in struct literal.

```ori
// ET009: duplicate field 'x'
Point { x: 1, x: 2, y: 3 }
```

**Fix:** Remove the duplicate field.

---

### ET010: Return Type Mismatch

**Description:** Function body type doesn't match declared return type.

```ori
@greet (name: str) -> int = "Hello, " + name
// ET010: expected int, found str
```

**Fix:** Ensure body matches return type.

---

### ET011: Variant Not Found

**Description:** Unknown variant for sum type.

```ori
type Status = Pending | Done
// ET011: no variant 'Running' on type Status
s = Status.Running
```

**Fix:** Use an existing variant.

---

### ET012: Object Safety Violation

**Description:** Trait cannot be used with `dyn` due to object safety rules.

```ori
trait Clonable {
    // Returns Self
    @clone (self) -> Self
}
// ET012: trait Clonable is not object-safe
x: dyn Clonable = ...
```

**Fix:** Modify trait to be object-safe or use generics instead.

---

### ET013: Recursive Type Without Indirection

**Description:** Type directly contains itself without pointer indirection.

```ori
// ET013: infinite size type
type Node = { value: int, next: Node }
```

**Fix:** Use `Option<Node>` or similar.

---

### ET014: Associated Type Not Specified

**Description:** Missing associated type for trait object.

```ori
// ET014: missing associated type 'Item'
items: [dyn Iterator]
```

**Fix:** Specify the associated type: `[dyn Iterator<Item = int>]`

---

## Parse Errors (EP)

### EP001: Unexpected Token

**Description:** Parser encountered unexpected token.

```ori
// EP001: unexpected end of expression
@foo () -> int = 5 +
```

**Fix:** Complete the expression correctly.

---

### EP002: Expected Token

**Description:** Parser expected a specific token.

```ori
// EP002: expected ')' after parameters
@foo (value: int -> int = value
```

**Fix:** Add the missing token.

---

### EP003: Invalid Number Literal

**Description:** Number literal is malformed.

```ori
// EP003: invalid number literal
x = 123abc
```

**Fix:** Use valid number syntax.

---

### EP004: Unterminated String

**Description:** String literal not closed.

```ori
// EP004: unterminated string literal
s = "hello
```

**Fix:** Close the string with `"`.

---

### EP005: Invalid Escape Sequence

**Description:** Unknown escape sequence in string.

```ori
// EP005: invalid escape sequence '\q'
s = "hello\q"
```

**Fix:** Use valid escapes: `\n`, `\t`, `\r`, `\\`, `\"`.

---

### EP006: Expected Expression

**Description:** Expression expected but not found.

```ori
// EP006: expected expression after 'then'
@foo () -> int = if x then else 5
```

**Fix:** Provide the missing expression.

---

### EP007: Invalid Pattern Syntax

**Description:** Pattern expression is malformed.

```ori
// EP007: fold requires .op argument
fold(
    .over: items,
    .initial: 0,
)
```

**Fix:** Use correct pattern syntax.

---

### EP008: Reserved Keyword

**Description:** Reserved word used as identifier.

```ori
// EP008: 'match' is a reserved keyword
@match (value: int) -> int = value
```

**Fix:** Use a different identifier.

---

### EP009: Invalid Duration Unit

**Description:** Unknown duration unit.

```ori
// EP009: invalid duration unit 'd', expected ms/s/m/h
timeout = 5d
```

**Fix:** Use valid units: `ms`, `s`, `m`, `h`.

---

### EP010: Invalid Operator

**Description:** Unknown or invalid operator.

```ori
// EP010: invalid operator '<>'
x = 5 <> 3
```

**Fix:** Use a valid operator.

---

### EP011: Reserved Built-in Name

**Description:** Function name conflicts with a reserved built-in function.

```ori
@is_empty (items: [int]) -> bool = len(
    .collection: items,
) == 0
// EP011: 'is_empty' is a reserved built-in function name
```

**Fix:** Use a different, more descriptive name.

```ori
@queue_is_empty (queue: [int]) -> bool = len(
    .collection: queue,
) == 0
```

Reserved built-in names include: `int`, `float`, `str`, `byte`, `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`, `assert_panics`, `assert_panics_with`, `compare`, `min`, `max`, `print`, `panic`.

---

## Name Errors (EN)

### EN001: Undefined Variable

**Description:** Variable not found in scope.

```ori
// EN001: undefined variable 'bar'
@foo () -> int = bar
```

**Fix:** Define the variable or check spelling.

---

### EN002: Undefined Function

**Description:** Function not found.

```ori
// EN002: undefined function 'unknown_fn'
result = unknown_fn(
    .value: 5,
)
```

**Fix:** Define or import the function.

---

### EN003: Duplicate Definition

**Description:** Name already defined in scope.

```ori
@foo () -> int = 1
// EN003: duplicate definition of 'foo'
@foo () -> int = 2
```

**Fix:** Use unique names.

---

### EN004: Shadowing in Same Block

**Description:** Variable shadowed in same binding block.

```ori
run(
    let x = 5,
    // EN004: 'x' already bound in this block
    let x = 10,
)
```

**Fix:** Use different names or separate blocks.

---

### EN005: Private Item

**Description:** Accessing non-public item from another module.

```ori
// EN005: 'internal_fn' is private
use other_module { internal_fn }
```

**Fix:** Use `pub` on the item or don't import it.

---

### EN006: Module Not Found

**Description:** Referenced module doesn't exist.

```ori
// EN006: module 'nonexistent' not found
use nonexistent { foo }
```

**Fix:** Check module path and file location.

---

### EN007: Circular Import

**Description:** Modules import each other.

```ori
// a.ori: use b { foo }
// b.ori: use a { bar }
// EN007: circular import between 'a' and 'b'
```

**Fix:** Refactor to break the cycle.

---

## Match Errors (EM)

### EM001: Non-Exhaustive Match

**Description:** Match doesn't cover all cases.

```ori
match(opt,
    // EM001: non-exhaustive, missing: None
    Some(value) -> value,
)
```

**Fix:** Add missing patterns or use wildcard `_`.

---

### EM002: Unreachable Pattern

**Description:** Pattern can never match.

```ori
match(value,
    _ -> "any",
    // EM002: unreachable pattern
    5 -> "five",
)
```

**Fix:** Reorder patterns or remove unreachable ones.

---

### EM003: Refutable Pattern in Binding

**Description:** Pattern might not match in context requiring success.

```ori
// EM003: pattern might not match
Some(value) = get_optional()
```

**Fix:** Use `match` for refutable patterns.

---

### EM004: Duplicate Pattern Binding

**Description:** Same variable bound multiple times in pattern.

```ori
// EM004: duplicate binding 'first'
(first, first) = pair
```

**Fix:** Use unique names for each binding.

---

### EM005: Invalid Guard Type

**Description:** Guard expression is not boolean.

```ori
match(value,
    // EM005: guard must be bool, found int
    number.match(number) -> "yes",
)
```

**Fix:** Use boolean expression in guard.

---

### EM006: Wrong Variant Arity

**Description:** Wrong number of fields in variant pattern.

```ori
type Pair = Two(first: int, second: int)
match(pair,
    // EM006: expected 2 fields, found 1
    Two(value) -> value,
)
```

**Fix:** Match all fields or use `..` to ignore rest.

---

## Function Errors (EF)

### EF001: Wrong Argument Count

**Description:** Function called with wrong number of arguments.

```ori
@add (left: int, right: int) -> int = left + right
// EF001: expected 2 arguments, found 1
add(
    .left: 1,
)
```

**Fix:** Provide correct number of arguments.

---

### EF002: Missing Return Type

**Description:** Function missing explicit return type.

```ori
// EF002: missing return type
@foo () = 5
```

**Fix:** Add return type: `@foo () -> int = 5`

---

### EF003: Recursive Without self

**Description:** Direct recursion without using `self`.

```ori
@factorial (number: int) -> int =
    if number <= 1 then 1 else number * factorial(
        .number: number - 1,
    )
// EF003: use 'self' for recursion in recurse pattern
```

**Fix:** Use `recurse` pattern with `self`.

---

### EF004: self Outside Recursion

**Description:** `self` used outside recursive context.

```ori
// EF004: 'self' only valid in recurse pattern
@foo () -> int = self(
    .value: 5,
)
```

**Fix:** Use `recurse` pattern or call function by name.

---

### EF005: Missing Async Capability

**Description:** Function calls code that may suspend without declaring `uses Async`.

```ori
@fetch (url: str) -> Result<str, Error> uses Http, Async = ...
// EF005: missing Async capability
@use_fetch () -> str = fetch(
    .url: "...",
)
```

**Fix:** Add `uses Async` to function signature, or provide capability with `with`.

---

### EF006: Capability Not Provided

**Description:** Required capability not in scope.

```ori
// EF006: Http not provided
@process () -> Result<str, Error> uses Http = Http.get("/data")
```

**Fix:** Provide capability with `with...in` or propagate with `uses`.

---

## Config Errors (EC)

### EC001: Non-Constant Config

**Description:** Config value is not a compile-time constant.

```ori
// EC001: config must be constant
$timeout = calculate_timeout()
```

**Fix:** Use literal value.

---

### EC002: Config Type Not Allowed

**Description:** Config uses unsupported type.

```ori
// EC002: functions not allowed in config
$callback = item -> item + 1
```

**Fix:** Use supported types: `int`, `float`, `str`, `bool`, duration.

---

## Import Errors (EI)

### EI001: Duplicate Import

**Description:** Same item imported twice.

```ori
// EI001: duplicate import 'sqrt'
use math { sqrt, sqrt }
```

**Fix:** Remove duplicate.

---

### EI002: Import Conflict

**Description:** Imported name conflicts with local definition.

```ori
use math { add }
// EI002: 'add' already imported
@add (left: int, right: int) -> int = left + right
```

**Fix:** Use `as` to rename import.

---

### EI003: Self Import

**Description:** Module imports itself.

```ori
// In foo.ori:
// EI003: module cannot import itself
use foo { bar }
```

**Fix:** Remove self-import.

---

## Async Errors (EA)

### EA001: Blocking in Async

**Description:** Blocking operation in async context.

```ori
// EA001: blocking call in async function
async @fetch () -> str =
    sleep_blocking(
        .duration: 1000,
    )
```

**Fix:** Use async-compatible operations.

---

### EA002: Detached Task

**Description:** Task not awaited or joined.

```ori
// EA002: detached task not allowed
async @start () -> void =
    spawn(
        .tasks: [background_work],
    )
```

**Fix:** Use structured concurrency with `parallel`.

---

## Test Errors (ES)

### ES001: Missing Tests

**Description:** Function has no tests.

```ori
// ES001: no tests for 'helper'
@helper (value: int) -> int = value + 1
```

**Fix:** Add at least one test for the function.

---

### ES002: Invalid Test Target

**Description:** Test targets non-existent function.

```ori
@test_foo tests @nonexistent () -> void = ...
// ES002: function 'nonexistent' not found
```

**Fix:** Target an existing function.

---

### ES003: Test Assertion Failed

**Description:** Test assertion did not pass.

```ori
// ES003: assertion failed: 4 != 5
assert_eq(
    .actual: 2 + 2,
    .expected: 5,
)
```

**Fix:** Fix the code or correct the expected value.

---

### ES004: Multiple Test Files

**Description:** Tests found in both locations.

```ori
// Tests in foo.ori AND tests/_test/foo.test.ori
// ES004: ambiguous test location
```

**Fix:** Use only one test location.

---

## Warnings (EW)

### EW001: Unused Variable

**Description:** Variable defined but never used.

```ori
run(
    // EW001: unused variable 'x'
    let x = 5,
    10,
)
```

**Fix:** Use the variable or prefix with `_`.

---

### EW002: Unused Import

**Description:** Imported item never used.

```ori
// EW002: unused import 'sin'
use math { sqrt, sin }
```

**Fix:** Remove unused import.

---

### EW003: Redundant Pattern

**Description:** Pattern always matches (after previous patterns).

```ori
match(value,
    Some(_) -> "some",
    // EW003: redundant pattern
    Some(5) -> "five",
)
```

**Fix:** Reorder or remove redundant pattern.

---

### EW004: Deprecated Item

**Description:** Using deprecated function or type.

```ori
// EW004: 'old_api' is deprecated, use 'new_api'
old_api()
```

**Fix:** Use the suggested replacement.

---

## See Also

- [Structured Errors](../12-tooling/03-structured-errors.md)
- [Testing](../11-testing/index.md)
- [Type System](../03-type-system/index.md)
