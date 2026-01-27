# Ori Language Specification

Version 0.1-alpha

## Design Principle

**Lean Core, Rich Libraries.** The language core defines only constructs requiring special syntax or static analysis. Data transformation and utilities are standard library methods.

| Core (compiler) | Library (stdlib) |
|-----------------|------------------|
| `run`, `try`, `match`, `recurse` | `map`, `filter`, `fold`, `find` |
| `parallel`, `spawn`, `timeout` | `retry`, `validate` |
| `cache`, `with` | Collection methods |

See [Patterns](10-patterns.md) for core constructs. See [Built-in Functions](11-built-in-functions.md) for library methods.

## Contents

1. [Notation](01-notation.md)
2. [Source Code](02-source-code.md)
3. [Lexical Elements](03-lexical-elements.md)
4. [Constants](04-constants.md)
5. [Variables](05-variables.md)
6. [Types](06-types.md)
7. [Properties of Types](07-properties-of-types.md)
8. [Declarations](08-declarations.md)
9. [Expressions](09-expressions.md)
10. [Patterns](10-patterns.md)
11. [Built-in Functions](11-built-in-functions.md)
12. [Modules](12-modules.md)
13. [Testing](13-testing.md)
14. [Capabilities](14-capabilities.md)
15. [Memory Model](15-memory-model.md)
16. [Formatting](16-formatting.md)
17. [Blocks and Scope](17-blocks-and-scope.md)
18. [Program Execution](18-program-execution.md)
19. [Control Flow](19-control-flow.md)
20. [Errors and Panics](20-errors-and-panics.md)
21. [Constant Expressions](21-constant-expressions.md)
22. [System Considerations](22-system-considerations.md)

## Status

Alpha. Breaking changes expected.

## Conformance

Implementations must:
- Accept conforming programs
- Reject non-conforming programs with diagnostics
- Produce specified behavior

Extensions must not alter conforming program behavior.
