# Specification Tests

**CRITICAL: Tests are the source of truth.**

These tests validate that the compiler conforms to the language specification
defined in `docs/sigil_lang/0.1-alpha/spec/`.

## Philosophy

- Tests are derived from the spec, not from the implementation
- If a test fails, the **code is wrong**, not the test
- Never modify tests to match broken behavior
- Each test references the spec section it validates
- Each test must have a comment linking it to it's spec file and section reference (not line number)
- Each test must have a comment linking it to it's design file and section reference (not line number)


## Organization

```
tests/spec/
├── lexical/          # 03-lexical-elements.md
│   ├── literals.si
│   ├── identifiers.si
│   ├── keywords.si
│   └── operators.si
├── types/            # 06-types.md
│   ├── primitives.si
│   ├── collections.si
│   ├── generics.si
│   └── inference.si
├── expressions/      # 09-expressions.md
│   ├── arithmetic.si
│   ├── comparison.si
│   ├── conditionals.si
│   └── bindings.si
└── patterns/         # 10-patterns.md
    ├── run.si
    ├── try.si
    ├── match.si
    └── data.si
```

## Running Spec Tests

```bash
# Run all spec tests
sigil test tests/spec/

# Run specific category
sigil test tests/spec/lexical/
```

## Adding New Tests

1. Identify the spec section being tested
2. Create test file in appropriate directory
3. Add comment referencing spec: `// Spec: 03-lexical-elements.md § Literals`
4. Write tests that validate the spec, not the current behavior
