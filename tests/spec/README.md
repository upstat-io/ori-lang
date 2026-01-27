# Specification Tests

**CRITICAL: Tests are the source of truth.**

These tests validate that the compiler conforms to the language specification
defined in `docs/ori_lang/0.1-alpha/spec/`.

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
│   ├── literals.ori
│   ├── identifiers.ori
│   ├── keywords.ori
│   └── operators.ori
├── types/            # 06-types.md
│   ├── primitives.ori
│   ├── collections.ori
│   ├── generics.ori
│   └── inference.ori
├── expressions/      # 09-expressions.md
│   ├── arithmetic.ori
│   ├── comparison.ori
│   ├── conditionals.ori
│   └── bindings.ori
└── patterns/         # 10-patterns.md
    ├── run.ori
    ├── try.ori
    ├── match.ori
    └── data.ori
```

## Running Spec Tests

```bash
# Run all spec tests
ori test tests/spec/

# Run specific category
ori test tests/spec/lexical/
```

## Adding New Tests

1. Identify the spec section being tested
2. Create test file in appropriate directory
3. Add comment referencing spec: `// Spec: 03-lexical-elements.md § Literals`
4. Write tests that validate the spec, not the current behavior
