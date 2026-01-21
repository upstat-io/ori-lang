# Appendix B: Error Codes

This appendix catalogs all error codes produced by the Sigil compiler.

---

## Error Code Format

All Sigil errors follow a structured format:

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
| `EA` | Async | Async/await errors |
| `ES` | Test | Testing errors |
| `EW` | Warning | Warnings (non-fatal) |

---

## Type Errors (ET)

### ET001: Type Mismatch

**Description:** Expected one type but found another.

```sigil
@add (a: int, b: int) -> int = a + b
add("hello", 5)  // ET001: expected int, found str
```

**Fix:** Ensure the value matches the expected type.

---

### ET002: Unknown Type

**Description:** Referenced type does not exist.

```sigil
@process (x: Foo) -> void = ...  // ET002: unknown type 'Foo'
```

**Fix:** Define the type or import it.

---

### ET003: Generic Arity Mismatch

**Description:** Wrong number of type arguments for generic type.

```sigil
type Pair<A, B> = { first: A, second: B }
x: Pair<int> = ...  // ET003: expected 2 type arguments, found 1
```

**Fix:** Provide the correct number of type arguments.

---

### ET004: Cannot Infer Type

**Description:** Type cannot be determined from context.

```sigil
x = []  // ET004: cannot infer element type of empty list
```

**Fix:** Add type annotation: `x: [int] = []`

---

### ET005: Trait Bound Not Satisfied

**Description:** Type doesn't implement required trait.

```sigil
@sort<T> (list: [T]) -> [T] where T: Comparable = ...
sort([User{}, User{}])  // ET005: User does not implement Comparable
```

**Fix:** Implement the trait for the type.

---

### ET006: Incompatible Types in Binary Operation

**Description:** Operator cannot be applied to given types.

```sigil
"hello" - 5  // ET006: cannot subtract int from str
```

**Fix:** Use compatible types for the operator.

---

### ET007: Missing Field

**Description:** Struct literal missing required field.

```sigil
type Point = { x: int, y: int }
p = Point { x: 10 }  // ET007: missing field 'y'
```

**Fix:** Provide all required fields.

---

### ET008: Unknown Field

**Description:** Referenced field doesn't exist on type.

```sigil
type Point = { x: int, y: int }
p.z  // ET008: no field 'z' on type Point
```

**Fix:** Use a field that exists on the type.

---

### ET009: Duplicate Field

**Description:** Field specified multiple times in struct literal.

```sigil
Point { x: 1, x: 2, y: 3 }  // ET009: duplicate field 'x'
```

**Fix:** Remove the duplicate field.

---

### ET010: Return Type Mismatch

**Description:** Function body type doesn't match declared return type.

```sigil
@greet (name: str) -> int = "Hello, " + name
// ET010: expected int, found str
```

**Fix:** Ensure body matches return type.

---

### ET011: Variant Not Found

**Description:** Unknown variant for sum type.

```sigil
type Status = Pending | Done
s = Status.Running  // ET011: no variant 'Running' on type Status
```

**Fix:** Use an existing variant.

---

### ET012: Object Safety Violation

**Description:** Trait cannot be used with `dyn` due to object safety rules.

```sigil
trait Clonable {
    @clone (self) -> Self  // Returns Self
}
x: dyn Clonable = ...  // ET012: trait Clonable is not object-safe
```

**Fix:** Modify trait to be object-safe or use generics instead.

---

### ET013: Recursive Type Without Indirection

**Description:** Type directly contains itself without pointer indirection.

```sigil
type Node = { value: int, next: Node }  // ET013: infinite size type
```

**Fix:** Use `Option<Node>` or similar.

---

### ET014: Associated Type Not Specified

**Description:** Missing associated type for trait object.

```sigil
items: [dyn Iterator]  // ET014: missing associated type 'Item'
```

**Fix:** Specify the associated type: `[dyn Iterator<Item = int>]`

---

## Parse Errors (EP)

### EP001: Unexpected Token

**Description:** Parser encountered unexpected token.

```sigil
@foo () -> int = 5 +  // EP001: unexpected end of expression
```

**Fix:** Complete the expression correctly.

---

### EP002: Expected Token

**Description:** Parser expected a specific token.

```sigil
@foo (x: int -> int = x  // EP002: expected ')' after parameters
```

**Fix:** Add the missing token.

---

### EP003: Invalid Number Literal

**Description:** Number literal is malformed.

```sigil
x = 123abc  // EP003: invalid number literal
```

**Fix:** Use valid number syntax.

---

### EP004: Unterminated String

**Description:** String literal not closed.

```sigil
s = "hello  // EP004: unterminated string literal
```

**Fix:** Close the string with `"`.

---

### EP005: Invalid Escape Sequence

**Description:** Unknown escape sequence in string.

```sigil
s = "hello\q"  // EP005: invalid escape sequence '\q'
```

**Fix:** Use valid escapes: `\n`, `\t`, `\r`, `\\`, `\"`.

---

### EP006: Expected Expression

**Description:** Expression expected but not found.

```sigil
@foo () -> int = if x then else 5  // EP006: expected expression after 'then'
```

**Fix:** Provide the missing expression.

---

### EP007: Invalid Pattern Syntax

**Description:** Pattern expression is malformed.

```sigil
fold(arr, 0)  // EP007: fold requires 3 arguments
```

**Fix:** Use correct pattern syntax.

---

### EP008: Reserved Keyword

**Description:** Reserved word used as identifier.

```sigil
@match (x: int) -> int = x  // EP008: 'match' is a reserved keyword
```

**Fix:** Use a different identifier.

---

### EP009: Invalid Duration Unit

**Description:** Unknown duration unit.

```sigil
timeout = 5d  // EP009: invalid duration unit 'd', expected ms/s/m/h
```

**Fix:** Use valid units: `ms`, `s`, `m`, `h`.

---

### EP010: Invalid Operator

**Description:** Unknown or invalid operator.

```sigil
x = 5 <> 3  // EP010: invalid operator '<>'
```

**Fix:** Use a valid operator.

---

## Name Errors (EN)

### EN001: Undefined Variable

**Description:** Variable not found in scope.

```sigil
@foo () -> int = bar  // EN001: undefined variable 'bar'
```

**Fix:** Define the variable or check spelling.

---

### EN002: Undefined Function

**Description:** Function not found.

```sigil
result = unknown_fn(5)  // EN002: undefined function 'unknown_fn'
```

**Fix:** Define or import the function.

---

### EN003: Duplicate Definition

**Description:** Name already defined in scope.

```sigil
@foo () -> int = 1
@foo () -> int = 2  // EN003: duplicate definition of 'foo'
```

**Fix:** Use unique names.

---

### EN004: Shadowing in Same Block

**Description:** Variable shadowed in same binding block.

```sigil
run(
    let x = 5,
    let x = 10,  // EN004: 'x' already bound in this block
)
```

**Fix:** Use different names or separate blocks.

---

### EN005: Private Item

**Description:** Accessing non-public item from another module.

```sigil
use other_module { internal_fn }  // EN005: 'internal_fn' is private
```

**Fix:** Use `pub` on the item or don't import it.

---

### EN006: Module Not Found

**Description:** Referenced module doesn't exist.

```sigil
use nonexistent { foo }  // EN006: module 'nonexistent' not found
```

**Fix:** Check module path and file location.

---

### EN007: Circular Import

**Description:** Modules import each other.

```sigil
// a.si: use b { foo }
// b.si: use a { bar }
// EN007: circular import between 'a' and 'b'
```

**Fix:** Refactor to break the cycle.

---

## Match Errors (EM)

### EM001: Non-Exhaustive Match

**Description:** Match doesn't cover all cases.

```sigil
match(opt,
    Some(x) -> x  // EM001: non-exhaustive, missing: None
)
```

**Fix:** Add missing patterns or use wildcard `_`.

---

### EM002: Unreachable Pattern

**Description:** Pattern can never match.

```sigil
match(x,
    _ -> "any",
    5 -> "five"  // EM002: unreachable pattern
)
```

**Fix:** Reorder patterns or remove unreachable ones.

---

### EM003: Refutable Pattern in Binding

**Description:** Pattern might not match in context requiring success.

```sigil
Some(x) = get_optional()  // EM003: pattern might not match
```

**Fix:** Use `match` for refutable patterns.

---

### EM004: Duplicate Pattern Binding

**Description:** Same variable bound multiple times in pattern.

```sigil
(x, x) = pair  // EM004: duplicate binding 'x'
```

**Fix:** Use unique names for each binding.

---

### EM005: Invalid Guard Type

**Description:** Guard expression is not boolean.

```sigil
match(x,
    n if n -> "yes"  // EM005: guard must be bool, found int
)
```

**Fix:** Use boolean expression in guard.

---

### EM006: Wrong Variant Arity

**Description:** Wrong number of fields in variant pattern.

```sigil
type Pair = Two(a: int, b: int)
match(p, Two(x) -> x)  // EM006: expected 2 fields, found 1
```

**Fix:** Match all fields or use `..` to ignore rest.

---

## Function Errors (EF)

### EF001: Wrong Argument Count

**Description:** Function called with wrong number of arguments.

```sigil
@add (a: int, b: int) -> int = a + b
add(1)  // EF001: expected 2 arguments, found 1
```

**Fix:** Provide correct number of arguments.

---

### EF002: Missing Return Type

**Description:** Function missing explicit return type.

```sigil
@foo () = 5  // EF002: missing return type
```

**Fix:** Add return type: `@foo () -> int = 5`

---

### EF003: Recursive Without self

**Description:** Direct recursion without using `self`.

```sigil
@factorial (n: int) -> int =
    if n <= 1 then 1 else n * factorial(n - 1)
// EF003: use 'self' for recursion in recurse pattern
```

**Fix:** Use `recurse` pattern with `self`.

---

### EF004: self Outside Recursion

**Description:** `self` used outside recursive context.

```sigil
@foo () -> int = self(5)  // EF004: 'self' only valid in recurse pattern
```

**Fix:** Use `recurse` pattern or call function by name.

---

### EF005: Async Without Await

**Description:** Async function result not awaited.

```sigil
@fetch () -> async str = ...
@use_fetch () -> str = fetch()  // EF005: async result must be awaited
```

**Fix:** Add `.await` to async call.

---

### EF006: Await Outside Async

**Description:** `.await` used in non-async function.

```sigil
@foo () -> str = fetch().await  // EF006: await outside async function
```

**Fix:** Mark function as `async`.

---

## Config Errors (EC)

### EC001: Non-Constant Config

**Description:** Config value is not a compile-time constant.

```sigil
$timeout = calculate_timeout()  // EC001: config must be constant
```

**Fix:** Use literal value.

---

### EC002: Config Type Not Allowed

**Description:** Config uses unsupported type.

```sigil
$callback = x -> x + 1  // EC002: functions not allowed in config
```

**Fix:** Use supported types: `int`, `float`, `str`, `bool`, duration.

---

## Import Errors (EI)

### EI001: Duplicate Import

**Description:** Same item imported twice.

```sigil
use math { sqrt, sqrt }  // EI001: duplicate import 'sqrt'
```

**Fix:** Remove duplicate.

---

### EI002: Import Conflict

**Description:** Imported name conflicts with local definition.

```sigil
use math { add }
@add (a: int, b: int) -> int = a + b  // EI002: 'add' already imported
```

**Fix:** Use `as` to rename import.

---

### EI003: Self Import

**Description:** Module imports itself.

```sigil
// In foo.si:
use foo { bar }  // EI003: module cannot import itself
```

**Fix:** Remove self-import.

---

## Async Errors (EA)

### EA001: Blocking in Async

**Description:** Blocking operation in async context.

```sigil
async @fetch () -> str =
    sleep_blocking(1000)  // EA001: blocking call in async function
```

**Fix:** Use async-compatible operations.

---

### EA002: Detached Task

**Description:** Task not awaited or joined.

```sigil
async @start () -> void =
    spawn(background_work)  // EA002: detached task not allowed
```

**Fix:** Use structured concurrency with `parallel`.

---

## Test Errors (ES)

### ES001: Missing Tests

**Description:** Function has no tests.

```sigil
@helper (x: int) -> int = x + 1  // ES001: no tests for 'helper'
```

**Fix:** Add at least one test for the function.

---

### ES002: Invalid Test Target

**Description:** Test targets non-existent function.

```sigil
@test_foo tests @nonexistent () -> void = ...
// ES002: function 'nonexistent' not found
```

**Fix:** Target an existing function.

---

### ES003: Test Assertion Failed

**Description:** Test assertion did not pass.

```sigil
assert_eq(2 + 2, 5)  // ES003: assertion failed: 4 != 5
```

**Fix:** Fix the code or correct the expected value.

---

### ES004: Multiple Test Files

**Description:** Tests found in both locations.

```sigil
// Tests in foo.si AND tests/_test/foo.test.si
// ES004: ambiguous test location
```

**Fix:** Use only one test location.

---

## Warnings (EW)

### EW001: Unused Variable

**Description:** Variable defined but never used.

```sigil
run(
    let x = 5,  // EW001: unused variable 'x'
    10,
)
```

**Fix:** Use the variable or prefix with `_`.

---

### EW002: Unused Import

**Description:** Imported item never used.

```sigil
use math { sqrt, sin }  // EW002: unused import 'sin'
```

**Fix:** Remove unused import.

---

### EW003: Redundant Pattern

**Description:** Pattern always matches (after previous patterns).

```sigil
match(x,
    Some(_) -> "some",
    Some(5) -> "five"  // EW003: redundant pattern
)
```

**Fix:** Reorder or remove redundant pattern.

---

### EW004: Deprecated Item

**Description:** Using deprecated function or type.

```sigil
old_api()  // EW004: 'old_api' is deprecated, use 'new_api'
```

**Fix:** Use the suggested replacement.

---

## See Also

- [Structured Errors](../12-tooling/03-structured-errors.md)
- [Testing](../11-testing/index.md)
- [Type System](../03-type-system/index.md)
