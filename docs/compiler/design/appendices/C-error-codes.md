---
title: "Appendix C: Error Codes"
description: "Ori Compiler Design — Appendix C: Error Codes"
order: 9003
section: "Appendices"
---

# Appendix C: Error Codes

Every diagnostic the Ori compiler emits carries a unique, permanent error code. This appendix is the canonical reference for all codes across every compiler phase. It covers the code format and numbering scheme, the complete table of all 119 codes, representative diagnostic output from each major phase, the `ori --explain` documentation system, and the procedure for adding new codes.

## How to Use This Reference

The fastest way to understand any error is the `--explain` flag:

```bash
ori --explain E2001
```

This prints the full documentation for the code: a title, an explanation of what went wrong, annotated Ori code examples that trigger the error, concrete solutions, and cross-references to related codes. Every documented code follows this format.

### Error code format

Codes follow the pattern `<prefix><phase><sequence>`:

- **Prefix**: `E` for errors, `W` for warnings.
- **Phase digit**: the first digit after the prefix identifies the compiler phase that emits the diagnostic.
- **Sequence**: a three-digit number, allocated sequentially within each phase range. Gaps in the sequence (such as the absence of E0007) indicate codes that were reserved during development but never assigned.

### Stability guarantee

Once a code is assigned, its meaning never changes. A code may be deprecated (no longer emitted by the compiler), but it will never be reassigned to a different diagnostic. This guarantee means that external tooling, CI scripts, and documentation can refer to error codes by number and trust that `E2001` will always mean "type mismatch."

## Error Code Ranges

Each compiler phase owns a contiguous range. The phase digit makes it possible to identify the origin of any diagnostic at a glance.

| Range | Phase | Description |
|-------|-------|-------------|
| E0xxx | Lexer | Tokenization errors: malformed literals, invalid characters, cross-language habits |
| E1xxx | Parser | Syntax errors: unexpected tokens, unclosed delimiters, invalid declarations |
| E2xxx | Type Checker | Type errors: mismatches, missing bounds, inference failures, capability violations |
| E3xxx | Patterns | Pattern system errors: unknown patterns, invalid arguments, type mismatches |
| E4xxx | ARC Analysis | ARC IR lowering errors: unsupported constructs, FBIP violations |
| E5xxx | Codegen / LLVM | Code generation errors: verification failures, linker errors, target issues |
| E6xxx | Runtime / Eval | Evaluator runtime errors: arithmetic faults, lookup failures, control flow |
| E9xxx | Internal | Compiler bugs and infrastructure limits |
| W1xxx | Parser Warnings | Non-fatal parser diagnostics |
| W2xxx | Type Warnings | Non-fatal type checker diagnostics |

Import and module resolution errors fall under the E2xxx range because they are detected during type checking, not during a separate import resolution phase. See the [Import and Module Errors](#import-and-module-errors) section for details.

## Complete Error Code Table

The table below lists every code defined in the `define_error_codes!` macro. The **Docs** column indicates whether the code has a detailed documentation file accessible through `ori --explain`.

### Lexer Errors (E0xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E0001 | Unterminated String | String literal not closed before end of line or file | yes |
| E0002 | Invalid Character | Character not valid in Ori source text | yes |
| E0003 | Invalid Number | Malformed numeric literal (e.g., multiple decimal points) | yes |
| E0004 | Unterminated Char | Character literal not closed | yes |
| E0005 | Invalid Escape | Unrecognized escape sequence in string or character literal | yes |
| E0006 | Unterminated Template | Template literal (backtick string) not closed | -- |
| E0008 | Triple-Equals | Cross-language habit: `===` is not valid in Ori | -- |
| E0009 | Single-Quote String | Cross-language habit: single-quoted strings are not valid in Ori | -- |
| E0010 | Increment/Decrement | Cross-language habit: `++`/`--` operators do not exist in Ori | -- |
| E0011 | Unicode Confusable | A Unicode character that visually resembles an ASCII operator or letter | -- |
| E0012 | Detached Doc Comment | Doc comment not attached to any declaration | -- |
| E0013 | Standalone Backslash | Backslash outside of a string or character literal | -- |
| E0014 | Decimal Not Representable | Decimal duration/size literal cannot be represented as whole base units | -- |
| E0015 | Reserved-Future Keyword | A future-reserved keyword (`asm`, `inline`, `static`, `union`, `view`) used as identifier | -- |
| E0911 | Float Duration/Size | Floating-point syntax used for a duration or size literal, which requires integer syntax | -- |

### Parser Errors (E1xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E1001 | Unexpected Token | Parser encountered a token it did not expect in the current context | yes |
| E1002 | Expected Expression | An expression was expected but not found | yes |
| E1003 | Unclosed Delimiter | Missing closing `)`, `]`, or `}` | yes |
| E1004 | Expected Identifier | An identifier was expected (e.g., after `let`, in a function name position) | yes |
| E1005 | Expected Type | A type annotation was expected but not found | yes |
| E1006 | Invalid Function | Function definition is syntactically invalid | yes |
| E1007 | Missing Function Body | Function declaration has no body expression | yes |
| E1008 | Invalid Pattern Syntax | Malformed pattern in a match arm or let binding | yes |
| E1009 | Missing Pattern Arg | A required argument is missing from a pattern invocation | yes |
| E1010 | Unknown Pattern Arg | An argument name not recognized by the target pattern | yes |
| E1011 | Named Args Required | Multi-argument function call requires named arguments | yes |
| E1012 | Invalid Block Expression | Syntax error inside a block expression (`function_seq`) | yes |
| E1013 | Named Properties Required | Control flow expression (`function_exp`) requires named properties | yes |
| E1014 | Reserved Name | Attempt to define a function with a reserved built-in name | yes |
| E1015 | Unsupported Keyword | A keyword from another language (e.g., `return`) that is not valid in Ori | yes |
| E1016 | Expected Semicolon | Expected `;` after an item declaration | -- |

### Type Checker Errors (E2xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E2001 | Type Mismatch | Expression type does not match expected type | yes |
| E2002 | Unknown Type | Type name not defined in the current scope | yes |
| E2003 | Unknown Identifier | Name not found in the current scope (variables, functions, imports) | yes |
| E2004 | Arg Count Mismatch | Wrong number of arguments supplied to a function call | yes |
| E2005 | Cannot Infer | Type inference failed; explicit annotation required | yes |
| E2006 | Duplicate Definition | A name is defined more than once in the same scope | yes |
| E2007 | Closure Self-Ref | A closure captures itself, creating an infinite type | yes |
| E2008 | Cyclic Type | Type definition contains a cycle (e.g., `type T = [T]`) | yes |
| E2009 | Missing Trait Bound | A required trait is not implemented for the given type | yes |
| E2010 | Coherence Violation | Conflicting trait implementations for the same type | yes |
| E2011 | Named Args Required | Named arguments are required for this function call | yes |
| E2012 | Unknown Capability | Capability name not recognized | yes |
| E2013 | Provider Mismatch | Provider value does not implement the expected capability trait | yes |
| E2014 | Missing Capability | Function uses a capability that is not declared in its `uses` clause | yes |
| E2015 | Type Param Order | Non-default type parameter appears after a default type parameter | yes |
| E2016 | Missing Type Arg | Required type argument not supplied and no default exists | yes |
| E2017 | Too Many Type Args | More type arguments supplied than the type accepts | yes |
| E2018 | Missing Assoc Type | Implementation is missing a required associated type | yes |
| E2019 | Never Type Field | `Never` used as a struct field type, which is uninhabited | yes |
| E2020 | Unsupported Operator | Operator not supported for the operand type | yes |
| E2021 | Overlapping Impls | Two implementations overlap with equal specificity | yes |
| E2022 | Conflicting Defaults | Multiple super-traits provide conflicting default methods | yes |
| E2023 | Ambiguous Method | Method call could resolve to multiple implementations | yes |
| E2024 | Not Object-Safe | Trait cannot be used as a trait object | yes |
| E2025 | Missing Index Impl | Type does not implement the `Index` trait | yes |
| E2026 | Wrong Index Key | Key type does not match any `Index` implementation for the target type | yes |
| E2027 | Ambiguous Index Key | Multiple `Index` implementations match the key type | yes |
| E2028 | Default Sum Type | Cannot derive `Default` for a sum type (no obvious default variant) | yes |
| E2029 | Hashable Needs Eq | Cannot derive `Hashable` without also deriving or implementing `Eq` | yes |
| E2030 | Hash Invariant | `Hashable` implementation may violate the hash consistency invariant | yes |
| E2031 | Non-Hashable Key | Type used as a map key does not implement `Hashable` | yes |
| E2032 | Derive Field Missing | A field type does not implement a trait required by a `#derive` annotation | yes |
| E2033 | Non-Derivable Trait | The named trait cannot be automatically derived | yes |
| E2034 | Invalid Format Spec | Format specification in a template string is syntactically invalid | yes |
| E2035 | Format Type Unsupported | Format type specifier not supported for the expression's type | yes |
| E2036 | Missing Into | Type does not implement `Into<T>` for the target type | yes |
| E2037 | Ambiguous Into | Multiple `Into` implementations could apply | yes |
| E2038 | Missing Printable | Type does not implement `Printable`, required for template interpolation | yes |
| E2039 | Immutable Assignment | Cannot assign to an immutable binding (declared with `let $`) | -- |
| E2040 | Feature Not Supported | Language feature used that is not yet supported by the compiler | -- |

### Pattern Errors (E3xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E3001 | Unknown Pattern | Pattern name not recognized by the pattern registry | yes |
| E3002 | Invalid Pattern Args | Arguments to a pattern are invalid (wrong names, types, or count) | yes |
| E3003 | Pattern Type Error | Pattern does not match the type of the scrutinee | yes |

### ARC Analysis Errors (E4xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E4001 | Unsupported ARC Expr | Expression form cannot be lowered to ARC intermediate representation | -- |
| E4002 | Unsupported ARC Pattern | Pattern form cannot be lowered to ARC intermediate representation | -- |
| E4003 | ARC Internal Error | Invariant violation in the ARC analysis pass (compiler bug) | -- |
| E4004 | FBIP Enforcement | FBIP (Functional But In-Place) constraint violated in ARC IR | -- |

### Codegen / LLVM Errors (E5xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E5001 | LLVM Verification | LLVM module verification failed (internal compiler error) | -- |
| E5002 | Optimization Failed | LLVM optimization pipeline encountered a fatal error | -- |
| E5003 | Emission Failed | Object file, assembly, or bitcode emission failed | -- |
| E5004 | Target Not Supported | Requested compilation target is not available or misconfigured | -- |
| E5005 | Runtime Not Found | Runtime library (`libori_rt.a`) not found at the expected path | -- |
| E5006 | Linker Failed | System linker returned a non-zero exit code | -- |
| E5007 | Debug Info Failed | DWARF or other debug info could not be generated | -- |
| E5008 | WASM Error | Error specific to the WebAssembly compilation target | -- |
| E5009 | Module Target Error | Module-level target configuration could not be applied | -- |

### Runtime / Eval Errors (E6xxx)

Runtime errors are organized into subcategories by the nature of the fault. These codes are emitted by the interpreter (evaluator) and correspond to structured `EvalErrorKind` variants.

**Arithmetic errors (E6001--E6006)**

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E6001 | Division By Zero | Integer or float division where the divisor is zero | -- |
| E6002 | Modulo By Zero | Modulo operation where the divisor is zero | -- |
| E6003 | Integer Overflow | Arithmetic operation overflowed the `int` (i64) range | -- |
| E6004 | Size Subtraction | `Size` subtraction would produce a negative value | -- |
| E6005 | Size Multiply | `Size` multiplied by a negative value | -- |
| E6006 | Size Divide | `Size` divided by a negative value | -- |

**Type errors at runtime (E6010--E6012)**

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E6010 | Runtime Type Mismatch | Value has wrong type at runtime (should not occur after type checking) | -- |
| E6011 | Invalid Binary Op | Binary operator not valid for the operand types | -- |
| E6012 | Binary Type Mismatch | Left and right operands of a binary operator have different types | -- |

**Lookup errors (E6020--E6027)**

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E6020 | Undefined Variable | Variable name not found in the current environment | -- |
| E6021 | Undefined Function | Function name not found in the current environment | -- |
| E6022 | Undefined Constant | Constant name not found in the current environment | -- |
| E6023 | Undefined Field | Field access on a struct or record with no such field | -- |
| E6024 | Undefined Method | Method call on a type that does not have the named method | -- |
| E6025 | Index Out Of Bounds | List index is negative or greater than or equal to the list length | -- |
| E6026 | Key Not Found | Map key lookup returned no entry | -- |
| E6027 | Immutable Binding | Attempt to mutate a binding declared with `let $` | -- |

**Call errors (E6030--E6032)**

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E6030 | Arity Mismatch | Function called with wrong number of arguments | -- |
| E6031 | Stack Overflow | Recursion depth exceeded the interpreter's limit | -- |
| E6032 | Not Callable | Attempted to call a value that is not a function | -- |

**Control flow errors (E6040--E6051)**

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E6040 | Non-Exhaustive Match | Match expression did not cover the actual value | -- |
| E6050 | Assertion Failed | An `assert`, `assert_eq`, or related assertion function failed | -- |
| E6051 | Panic Called | Explicit `panic(msg:)` was invoked | -- |

**Other runtime errors (E6060--E6099)**

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E6060 | Missing Capability | A capability required at runtime was not provided via `with...in` | -- |
| E6070 | Const-Eval Budget | Constant evaluation exceeded its computational budget | -- |
| E6080 | Not Implemented | A language feature reached at runtime that is not yet implemented | -- |
| E6099 | Custom Runtime Error | A runtime error that does not fit any other category | -- |

### Internal Errors (E9xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| E9001 | Internal Error | An internal compiler error (ICE); indicates a bug in the compiler itself | yes |
| E9002 | Too Many Errors | The error limit was reached; remaining diagnostics suppressed | yes |

### Parser Warnings (W1xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| W1001 | Detached Doc Comment | Doc comment is not attached to any declaration | -- |
| W1002 | Unknown Calling Conv | Unrecognized calling convention in an `extern` block | -- |

### Type Checker Warnings (W2xxx)

| Code | Name | Description | Docs |
|------|------|-------------|:----:|
| W2001 | Infinite Iterator | Infinite iterator (e.g., `repeat`, unbounded range) consumed without `.take()` | yes |

## Example Diagnostics

This section shows representative diagnostic output from each major compiler phase. All diagnostics follow a consistent format: the severity and code on the first line, a source snippet with annotated spans, and optional notes or help text.

### Lexer: E0001 -- Unterminated String

```
error[E0001]: unterminated string literal
 --> src/main.ori:3:19
  |
3 |     let name = "hello
  |                 ^ string literal never closed
  |
  = help: add a closing `"` to terminate the string
```

The lexer detects that a double-quote was opened but never closed before the end of the line. This is one of the most common errors for programmers new to any language. The span points to the opening quote, and the help text states the fix directly.

### Parser: E1001 -- Unexpected Token

```
error[E1001]: unexpected token
 --> src/main.ori:5:11
  |
5 |     let x + = 1
  |           ^ expected `=` or `:`, found `+`
```

The parser expected either an `=` (for a binding) or a `:` (for a type annotation) after the identifier `x`, but found `+` instead. The label tells the programmer what was expected and what was found, which is usually enough to identify the typo.

### Type Checker: E2001 -- Type Mismatch

```
error[E2001]: type mismatch
 --> src/main.ori:3:18
  |
3 |     let x: int = "hello"
  |            ---   ^^^^^^^ expected `int`, found `str`
  |            |
  |            expected due to this annotation
```

This is the most frequently encountered type error. The primary label points to the expression that has the wrong type. A secondary label points to the annotation or context that established the expectation. Ori performs no implicit conversions, so the programmer must use an explicit conversion function or change the annotation.

### Patterns: E3002 -- Invalid Pattern Arguments

```
error[E3002]: missing required argument `transform` for pattern `map`
 --> src/main.ori:4:5
  |
4 |     map(over: items)
  |     ^^^^^^^^^^^^^^^^ missing `transform`
  |
  = help: valid arguments are: `over`, `transform`
```

Pattern expressions in Ori use named arguments. When a required argument is missing, the diagnostic names it explicitly and lists all valid arguments in the help text.

### Codegen: E5005 -- Runtime Not Found

```
error[E5005]: runtime library `libori_rt.a` not found
  |
  = note: searched at: /home/user/.local/lib/ori/libori_rt.a
  = help: build the runtime with `cargo bl` or `cargo blr`
```

This error appears when attempting AOT compilation without having built the Ori runtime library first. Unlike most diagnostics, it has no source span because it is not triggered by any particular line of user code. The help text gives the exact command to resolve the issue.

### Runtime: E6001 -- Division By Zero

```
error[E6001]: division by zero
 --> src/main.ori:5:15
  |
5 |     let r = x / 0
  |               ^ division by zero
```

Ori integers use checked arithmetic. Division by zero is a runtime error (not undefined behavior). The evaluator traps the operation and reports the exact location.

### Runtime: E6025 -- Index Out Of Bounds

```
error[E6025]: index out of bounds
 --> src/main.ori:4:13
  |
4 |     items[10]
  |          ^^^^ index 10 is out of range for list of length 3
```

List indexing in Ori is bounds-checked. The diagnostic includes both the index value and the list length, which are the two pieces of information needed to understand why the access failed.

### Type Checker: E2009 -- Missing Trait Bound

```
error[E2009]: missing trait bound
 --> src/main.ori:3:22
  |
3 |     let sorted = items.sort()
  |                        ^^^^ `sort` requires `Comparable`, but `MyStruct` does not implement it
  |
  = help: add `#derive(Comparable)` to `MyStruct`, or implement `Comparable` manually
```

When a method requires a trait that the receiver type does not implement, the diagnostic names both the method and the missing trait. If the trait is derivable, the help text suggests `#derive`.

## Error Documentation System

### The `ori --explain` command

Any error code can be queried from the command line:

```bash
ori --explain E2009
```

This prints the full markdown documentation for E2009 to the terminal. The command accepts codes case-insensitively (`e2009` and `E2009` both work) and returns a non-zero exit code if the code is unknown or undocumented.

### How documentation is embedded

Each documented error code has a markdown file in the `compiler/ori_diagnostic/src/errors/` directory, named after the code (e.g., `E2001.md`). These files are compiled into the binary at build time using Rust's `include_str!` macro. The `ErrorDocs` struct provides O(1) lookup through a lazily-initialized `HashMap`:

```rust
// In compiler/ori_diagnostic/src/errors/mod.rs
static DOCS: &[(ErrorCode, &str)] = &[
    (ErrorCode::E0001, include_str!("E0001.md")),
    (ErrorCode::E2001, include_str!("E2001.md")),
    // ... one entry per documented code
];
```

This approach means documentation is always in sync with the binary that ships it. There is no separate documentation artifact to keep up to date and no risk of version skew between the compiler and its error explanations.

### Standard documentation format

Every error documentation file follows a consistent structure:

1. **Title**: `# EXXXX: Short Name`
2. **Opening paragraph**: one or two sentences describing the error in plain language.
3. **Example**: an annotated Ori code snippet that triggers the error.
4. **Explanation**: a longer discussion of why the error occurs and what the compiler is checking.
5. **Common Causes**: a numbered list of typical programmer mistakes that lead to this error.
6. **Solutions**: concrete code examples showing how to fix each common cause.
7. **See Also**: cross-references to related error codes.

Not every file includes every section (some errors are simple enough that "Common Causes" and "Solutions" collapse into the explanation), but the ordering is always consistent.

### Documentation coverage

Of the 119 error codes currently defined, 64 have full documentation accessible through `ori --explain`. Coverage varies by phase:

| Phase | Total Codes | Documented | Coverage |
|-------|:-----------:|:----------:|:--------:|
| Lexer (E0xxx) | 15 | 5 | 33% |
| Parser (E1xxx) | 16 | 15 | 94% |
| Type Checker (E2xxx) | 40 | 38 | 95% |
| Patterns (E3xxx) | 3 | 3 | 100% |
| ARC (E4xxx) | 4 | 0 | 0% |
| Codegen (E5xxx) | 9 | 0 | 0% |
| Runtime (E6xxx) | 24 | 0 | 0% |
| Internal (E9xxx) | 2 | 2 | 100% |
| Warnings (Wxxx) | 3 | 1 | 33% |
| **Total** | **119** | **64** | **54%** |

The parser and type checker, which produce the errors programmers encounter most frequently, have near-complete documentation. The ARC, codegen, and runtime phases are infrastructure-facing and produce errors that are either internal (ARC, codegen) or have short self-explanatory messages (runtime arithmetic faults). Documentation for these phases is a lower priority but is planned.

## Adding a New Error Code

When a new diagnostic is needed, follow these steps in order:

### 1. Choose the correct range

Identify which compiler phase will emit the diagnostic. Use the range table at the top of this appendix. If the error could plausibly belong to two phases, assign it to the phase that detects it. For example, import resolution errors belong to E2xxx because they are detected during type checking, even though they concern the module system.

### 2. Allocate the next sequence number

Within the chosen range, find the highest currently allocated code and increment by one. Gaps in the sequence are acceptable and should not be backfilled. The `define_error_codes!` macro in `compiler/ori_diagnostic/src/error_code/mod.rs` is the single source of truth.

### 3. Add the code to the macro

Add a new line to the `define_error_codes!` invocation:

```rust
define_error_codes! {
    // ...existing codes...
    E2041, "Description of the new error";
}
```

The macro generates the enum variant, its `as_str()` representation, and its `description()`. Update the `COUNT` assertion in the test file to reflect the new total.

### 4. Write a documentation file

Create `compiler/ori_diagnostic/src/errors/E2041.md` following the standard format described in the previous section. Then register it in `compiler/ori_diagnostic/src/errors/mod.rs`:

```rust
static DOCS: &[(ErrorCode, &str)] = &[
    // ...existing entries...
    (ErrorCode::E2041, include_str!("E2041.md")),
];
```

### 5. Use the code in the emitting phase

Each compiler phase has its own problem type or error factory. The new code is used through the diagnostic builder:

```rust
Diagnostic::error(ErrorCode::E2041)
    .with_message("description of what went wrong")
    .with_label(span, "explanation of this location")
```

### 6. Verify

Run `cargo t -p ori_diagnostic` to check that all tests pass, including the exhaustive classification test (`test_all_variants_classified`) and the count test (`test_all_is_complete`). The classification test ensures every code is matched by exactly one `is_*` predicate; the count test catches unintentional additions or removals.

## Import and Module Errors

Import and module resolution errors are reported as E2xxx type checker errors because they are detected during the type checking phase, not during a separate resolution pass. This design reflects Ori's architecture: the parser produces an AST with unresolved `use` declarations, and the type checker resolves them as part of building the typed IR.

The most common import-related diagnostics are:

- **Module not found**: reported through the diagnostic system when a `use` path does not resolve to any known module. The error includes the full path that was attempted and, when possible, suggests similarly-named modules.

- **Item not exported**: reported as E2003 (unknown identifier) when the target module exists but does not export the requested name. The diagnostic context indicates that the name exists in the module but is private (not marked `pub`).

- **Circular imports**: detected during module resolution when two modules depend on each other. The diagnostic traces the cycle to help the programmer understand the dependency chain and decide where to break it.

These are unified under the E2xxx range rather than given their own range because, from the programmer's perspective, the symptoms are identical to other name-resolution failures: "I used a name and the compiler does not know what it refers to." Splitting them into a separate range would add complexity without improving the programmer's ability to fix the error.
