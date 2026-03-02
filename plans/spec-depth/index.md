# Spec Depth Plan — Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Source Code
**File:** `section-01-source-code.md` | **Status:** Not Started

```
source code, UTF-8, BOM, encoding, characters, unicode
line endings, CRLF, newline, carriage return
source file, .ori, .test.ori, file naming
line continuation, implicit continuation
module mapping, file path, source root
NUL, control characters, valid characters
line number, column number, position, span
```

---

### Section 02: Lexical Elements
**File:** `section-02-lexical-elements.md` | **Status:** Not Started

```
token, identifier, keyword, literal, operator, delimiter
unicode identifier, letter, digit, underscore
integer literal, hex, octal, binary, digit separator
float literal, exponent, scientific notation, Inf, NaN
string literal, escape sequence, unicode escape
template string, interpolation, backtick, format spec
character literal, rune, char, unicode scalar
duration literal, size literal, decimal syntax
semicolon, automatic semicolon, statement terminator
trailing comma, comma-separated list
whitespace, indentation, tab, space
token boundary, lexer-parser contract, greater-than
disambiguation, struct literal, soft keyword
```

---

### Section 03: Bindings and Declarations
**File:** `section-03-bindings-declarations.md` | **Status:** Not Started

```
constant, immutable, $, let $, module-level
variable, mutable, let, binding, assignment
default value, zero value, initialization
drop, lifetime, drop ordering, destructor
declaration, function, type, trait, impl
predeclared identifier, built-in name, universe scope
exported identifier, pub, visibility, public, private
uniqueness, name conflict, redeclaration
blank identifier, wildcard, underscore
forward reference, mutual recursion
```

---

### Section 04: Blocks and Scope
**File:** `section-04-blocks-scope.md` | **Status:** Not Started

```
block, scope, lexical scope, nested scope
universe scope, predeclared, module scope, file scope
type parameter scope, Self, self
label scope, loop label, for label
forward reference, mutual recursion
shadowing, visibility, name resolution order
```

---

### Section 05: Expressions
**File:** `section-05-expressions.md` | **Status:** Not Started

```
expression, operand, primary expression
composite literal, list literal, map literal, struct literal
type inference, literal typing, contextual type
conversion, as, as?, Into, type cast
conversion table, legal conversion, lossy, lossless
method value, method expression, first-class method
string interpolation, template expression
type narrowing, refinement, smart cast
argument punning, named argument, positional argument
evaluation order, left-to-right, short-circuit
```

---

### Section 06: Control Flow
**File:** `section-06-control-flow.md` | **Status:** Not Started

```
statement, expression statement, statement termination
if then else, conditional, branch
match, pattern matching, exhaustiveness, arm
for do, for yield, loop, iteration
break, continue, label, labeled loop
terminating expression, Never, diverge, dead code
assignment, compound assignment, desugaring
empty expression, void, unit
```

---

### Section 07: Program Execution and Panics
**File:** `section-07-execution-panics.md` | **Status:** Not Started

```
program, executable, entry point, @main
initialization, module init, config variable
zero value, default initialization, uninitialized
termination, exit code, signal, cleanup
panic, runtime panic, unrecoverable
panic handler, @panic, PanicInfo
double panic, panic in Drop
stack trace, trace entry, backtrace
out of bounds, overflow, division by zero
index, shift, assertion failure
```

---

### Section 08: Built-in Functions
**File:** `section-08-builtins.md` | **Status:** Not Started

```
built-in, prelude, standard function
print, panic, todo, unreachable, dbg
assert, assert_eq, assert_ne, assert_panics
len, is_empty, is_some, is_none, is_ok, is_err
compare, min, max, hash_combine
repeat, drop_early, embed, has_embed
compile_error, is_cancelled
edge case, panic condition, type constraint
```

---

### Section 09: Verification
**File:** `section-09-verification.md` | **Status:** Not Started

```
consistency, cross-reference, internal audit
completeness, coverage, gap analysis
edge case inventory, panic catalog
terminology, "shall", "may", normative
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Source Code | `section-01-source-code.md` |
| 02 | Lexical Elements | `section-02-lexical-elements.md` |
| 03 | Bindings and Declarations | `section-03-bindings-declarations.md` |
| 04 | Blocks and Scope | `section-04-blocks-scope.md` |
| 05 | Expressions | `section-05-expressions.md` |
| 06 | Control Flow | `section-06-control-flow.md` |
| 07 | Program Execution and Panics | `section-07-execution-panics.md` |
| 08 | Built-in Functions | `section-08-builtins.md` |
| 09 | Verification | `section-09-verification.md` |
