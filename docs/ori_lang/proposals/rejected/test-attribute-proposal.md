# Proposal: Test Attribute Syntax

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-22
**Related:** [Simplified Attribute Syntax](simplified-attributes-proposal.md)

---

## Summary

Replace the `tests` keyword syntax with a `#test` attribute for declaring test functions. This improves grepability, aligns with the attribute system, and simplifies the grammar.

```ori
// Before (keyword syntax)
@test_try_basic tests @safe_divide tests @try_basic () -> void = run(
    assert_eq(try_basic(), Ok(5)),
)

// After (attribute syntax)
#test(@safe_divide, @try_basic)
@test_try_basic () -> void = run(
    assert_eq(try_basic(), Ok(5)),
)
```

---

## Motivation

### The Problem

The current `tests` keyword syntax has several issues:

1. **Hard to grep** — Searching for tests requires complex patterns:
   ```bash
   # Current: awkward regex needed
   grep "tests @" *.ori

   # Proposed: simple prefix search
   grep "^#test" *.ori
   ```

2. **Buried in function signature** — The `tests` keyword appears mid-declaration, making it easy to miss:
   ```ori
   @test_complex_name tests @target1 tests @target2 () -> void = ...
   //                 ^^^^^^^^^^^^^ ^^^^^^^^^^^^^ easy to overlook
   ```

3. **Inconsistent with other metadata** — Attributes like `#skip` and `#derive` are prefixed, but test declarations use inline keywords.

4. **Grammar complexity** — The `tests` keyword requires special parsing rules and can chain indefinitely.

5. **Tooling friction** — IDEs and linters need special logic to identify test functions; an attribute is immediately recognizable.

### Why an Attribute?

Test declaration is metadata about a function:
- "This function is a test"
- "It tests these targets"

This is exactly what attributes express. Other languages use similar patterns:

| Language | Test Declaration |
|----------|-----------------|
| Rust | `#[test]` |
| Python | `@pytest.mark.test` or naming convention |
| Go | `func TestFoo(t *testing.T)` |
| Java | `@Test` |

An attribute-based approach aligns Ori with industry conventions.

---

## Design

### Syntax

```
TestAttribute = "#test" "(" TargetList ")"
TargetList = Target { "," Target }
Target = "@" Identifier
```

The `#test` attribute takes one or more function references as arguments:

```ori
// Single target
#test(@calculate_sum)
@test_sum () -> void = run(
    assert_eq(calculate_sum(1, 2), 3),
)

// Multiple targets
#test(@parse_int, @validate_input)
@test_parsing () -> void = run(
    assert_eq(parse_int("42"), Ok(42)),
    assert(validate_input("hello")),
)
```

### Semantic Rules

1. **At least one target required** — Every test must specify what it tests:
   ```ori
   // Error: #test requires at least one target
   #test()
   @orphan_test () -> void = ...
   ```

2. **Targets must exist** — Referenced functions must be defined in scope:
   ```ori
   // Error: @nonexistent is not defined
   #test(@nonexistent)
   @bad_test () -> void = ...
   ```

3. **No circular testing** — A function cannot test itself:
   ```ori
   // Error: @test_self cannot test itself
   #test(@test_self)
   @test_self () -> void = ...
   ```

4. **Return type must be void** — Test functions don't return values:
   ```ori
   // Error: test functions must return void
   #test(@foo)
   @test_foo () -> int = 42
   ```

### Combining with Other Attributes

Attributes stack naturally:

```ori
#skip("flaky on CI")
#test(@network_fetch)
@test_network () -> void = run(
    let result = network_fetch("https://example.com"),
    assert(is_ok(result)),
)

#skip("not yet implemented")
#test(@future_feature)
@test_future () -> void = run(
    assert(false),
)
```

Order doesn't matter, but convention is `#skip` before `#test`:

```ori
// Preferred
#skip("reason")
#test(@target)

// Also valid
#test(@target)
#skip("reason")
```

---

## Migration

### Automatic Migration

The `ori fmt` tool will automatically convert old syntax to new:

```ori
// Input (old syntax)
@test_foo tests @bar tests @baz () -> void = run(...)

// Output (new syntax)
#test(@bar, @baz)
@test_foo () -> void = run(...)
```

### Migration Path

1. **Phase 1:** Accept both syntaxes, emit deprecation warning for old
2. **Phase 2:** `ori fmt` auto-converts old to new
3. **Phase 3:** Remove old syntax support

### Regex for Finding Old Syntax

```bash
# Find all uses of old tests keyword
grep -E "@[a-z_]+ tests @" **/*.ori
```

---

## Examples

### Before and After

**Simple test:**
```ori
// Before
@test_add tests @add () -> void = run(
    assert_eq(add(1, 2), 3),
)

// After
#test(@add)
@test_add () -> void = run(
    assert_eq(add(1, 2), 3),
)
```

**Multiple targets:**
```ori
// Before
@test_math tests @add tests @subtract tests @multiply () -> void = run(
    assert_eq(add(1, 2), 3),
    assert_eq(subtract(5, 3), 2),
    assert_eq(multiply(2, 3), 6),
)

// After
#test(@add, @subtract, @multiply)
@test_math () -> void = run(
    assert_eq(add(1, 2), 3),
    assert_eq(subtract(5, 3), 2),
    assert_eq(multiply(2, 3), 6),
)
```

**With skip:**
```ori
// Before
#skip("waiting on parser fix")
@test_parser tests @parse () -> void = run(...)

// After
#skip("waiting on parser fix")
#test(@parse)
@test_parser () -> void = run(...)
```

**Real-world example (from try.ori):**
```ori
// Before
@test_try_returns_final tests @parse_int tests @try_multi () -> void = run(
    assert_eq(try_multi(), Ok(4)),
)

// After
#test(@parse_int, @try_multi)
@test_try_returns_final () -> void = run(
    assert_eq(try_multi(), Ok(4)),
)
```

---

## Implementation

### Parser Changes

Remove the `tests` keyword handling from function parsing. Instead, `#test` is parsed as a regular attribute with the attribute parser from the simplified-attributes proposal.

**Before (function parser):**
```rust
fn parse_function(&mut self) -> Function {
    let attrs = self.parse_attributes();
    self.expect(Token::At);
    let name = self.parse_identifier();

    // Special tests parsing
    let mut targets = vec![];
    while self.current() == Token::Tests {
        self.advance();
        self.expect(Token::At);
        targets.push(self.parse_identifier());
    }

    // ... rest of function
}
```

**After (function parser):**
```rust
fn parse_function(&mut self) -> Function {
    let attrs = self.parse_attributes();  // #test handled here
    self.expect(Token::At);
    let name = self.parse_identifier();
    // No special tests parsing needed
    // ... rest of function
}
```

### AST Changes

**Before:**
```rust
struct Function {
    name: Identifier,
    test_targets: Vec<Identifier>,  // Special field
    // ...
}
```

**After:**
```rust
struct Function {
    name: Identifier,
    attributes: Vec<Attribute>,  // #test is just an attribute
    // ...
}

// Test targets extracted from attributes when needed
fn get_test_targets(func: &Function) -> Option<Vec<Identifier>> {
    func.attributes.iter()
        .find(|a| a.name == "test")
        .map(|a| a.args.clone())
}
```

### Semantic Analysis

The type checker validates `#test` attributes:

```rust
fn check_test_attribute(&mut self, func: &Function, attr: &Attribute) {
    // Must have at least one target
    if attr.args.is_empty() {
        self.error("#test requires at least one target function");
    }

    // All targets must be function references
    for arg in &attr.args {
        match arg {
            Expr::FunctionRef(name) => {
                if !self.scope.has_function(name) {
                    self.error(format!("@{} is not defined", name));
                }
            }
            _ => self.error("#test arguments must be function references (@name)"),
        }
    }

    // Function must return void
    if func.return_type != Type::Void {
        self.error("test functions must return void");
    }
}
```

### Test Runner Changes

The test runner finds tests by attribute instead of AST field:

```rust
fn find_tests(module: &Module) -> Vec<&Function> {
    module.functions.iter()
        .filter(|f| f.attributes.iter().any(|a| a.name == "test"))
        .collect()
}

fn get_coverage_targets(test: &Function) -> Vec<Identifier> {
    test.attributes.iter()
        .find(|a| a.name == "test")
        .map(|a| extract_function_refs(&a.args))
        .unwrap_or_default()
}
```

---

## Grammar Changes

### Remove

```
FunctionDecl = ... [ TestClause ] ...
TestClause = "tests" "@" Identifier { "tests" "@" Identifier }
```

### Add

The `#test` attribute follows standard attribute grammar:

```
Attribute = "#" Identifier [ "(" ArgumentList ")" ]
// #test specifically:
TestAttribute = "#test" "(" FunctionRefList ")"
FunctionRefList = FunctionRef { "," FunctionRef }
FunctionRef = "@" Identifier
```

---

## Comparison

| Aspect | Old Syntax | New Syntax |
|--------|-----------|------------|
| Grepability | `grep "tests @"` (false positives) | `grep "^#test"` (precise) |
| Visual | Buried in signature | Clearly prefixed |
| Grammar | Special `tests` keyword | Standard attribute |
| Tooling | Custom detection | Attribute detection |
| Multi-target | Chain `tests @a tests @b` | List `@a, @b` |
| Alignment | Unique to Ori | Similar to Rust, Java |

---

## Design Rationale

### Why Not `#[test]` (Rust-style)?

Per the [simplified attributes proposal](simplified-attributes-proposal.md), Ori uses `#name()` not `#[name()]`. The brackets add noise without value.

### Why Require Targets?

Ori's philosophy is explicit testing. Every test must declare what it tests. This enables:
- Coverage analysis
- Dead code detection
- Documentation of test intent

An empty `#test()` would defeat this purpose.

### Why `@name` References?

The `@` ori identifies functions. Using `#test(@foo)` is consistent with how functions are referenced elsewhere:
- `@foo()` — call function
- `#test(@foo)` — test function
- `use './mod' { @foo }` — import function

### Why Not Named Arguments?

We considered:
```ori
#test(.targets: [@foo, @bar])
```

Rejected because:
1. More verbose for common case
2. Single unnamed argument list is clearer
3. No other metadata to name (just targets)

---

## Summary

Replacing `tests` keyword with `#test` attribute:

1. **Improves grepability** — `grep "#test"` finds all tests
2. **Cleaner syntax** — Targets listed once, not chained
3. **Consistent** — Follows attribute conventions
4. **Simpler grammar** — No special keyword handling
5. **Better tooling** — Standard attribute detection
6. **Maintains semantics** — Same test coverage requirements

The change is syntactic sugar with identical runtime behavior.
