# Roadmap Index

> **Maintenance Notice:** This index must be updated whenever roadmap items are added, removed, or reorganized. When modifying any `section-*.md` file, update the corresponding keyword cluster below to keep the index accurate and searchable.

Quick-reference keyword index for finding roadmap sections. Search for a term to locate the relevant section.

---

> **~~PRIORITY BLOCKER~~ RESOLVED (2026-02-19)**: Section 10.7 `catch(expr)` — implemented across all interpreter phases. `assert_panics` works via `library/std/testing.ori`. All 16 previously-skipped tests (11 in `integer_safety.ori`, 5 in `operators_bitwise.ori`) now pass. LLVM codegen for catch remains simplified (placeholder).

---

> **~~PRIORITY ESCALATION~~ RESOLVED (2026-02-20)**: Section 15D.3 `$` immutability enforcement — block expression syntax (`{ }` with `;`), old `run()`/`match()`/`try()`/`loop()` syntax removed, compile-time immutability enforcement (E2039), all 229+ test files migrated. Remaining 15D.3 items: `mut` keyword removal, `$x`/`x` scope conflicts, module-level immutability.

---

> **ACTIVE REROUTE — Block Unification (2026-02-20)**: `plans/block_unify/` — Eliminate `FunctionSeq::Run` / `ExprKind::Block` duality. Kill `SeqBinding`, unify to single `StmtKind`. Adopt Gleam pattern. 5 sections, touches ori_ir, ori_parse, ori_types, ori_canon, ori_eval, ori_llvm, ori_fmt. Remove this block when all 5 sections are complete.

---

## How to Use

1. **Search this file** (Ctrl+F / Cmd+F) for keywords related to what you're looking for
2. **Find the section ID** in the keyword cluster
3. **Open the section file**: `plans/roadmap/section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 00: Full Parser Support
**File:** `section-00-parser.md` | **Tier:** 0 | **Status:** In Progress

```
parser, parsing, syntax, grammar, EBNF
lexer, lexical, token, tokenize
AST, abstract syntax tree, parse tree
production, grammar rule, syntax rule
literal, identifier, keyword, operator, delimiter
expression, statement, declaration
pattern, binding pattern, match pattern
precedence, associativity, binary, unary, postfix
error recovery, parse error, syntax error
ori_parse, ori_lexer
```

---

### Section 01: Type System Foundation
**File:** `section-01-type-system.md` | **Tier:** 1 | **Status:** Complete

```
int, float, bool, str, char, byte, void, Never
primitive types, primitives, basic types
Duration, Size, time units, byte units
nanoseconds, microseconds, milliseconds, seconds, minutes, hours
bytes, kilobytes, megabytes, gigabytes, terabytes
type annotations, parameter types, lambda types, let binding types
Never type, bottom type, uninhabited, coercion
low-level, future-proofing, reserved, inline type, view type
lifetime, LifetimeId, borrowed, value category, ValueCategory
stack allocation, Inline, View, Boxed, architectural slot
```

---

### Section 02: Type Inference
**File:** `section-02-type-inference.md` | **Tier:** 1 | **Status:** Complete

```
type inference, HM, Hindley-Milner
unification, occurs check, substitution
generalization, let-polymorphism, polymorphism
generic inference, type argument inference
type errors, inference errors, type hints
expression inference, binding inference
```

---

### Section 03: Traits and Implementations
**File:** `section-03-traits.md` | **Tier:** 1 | **Status:** Partial

```
trait, traits, impl, implementation, implements
associated types, associated functions
trait bounds, where clause, constraints
derive, #derive, Eq, Clone, Hashable, Debug, Default, Printable
operator traits, Add, Sub, Mul, Div, Neg, Rem
BitAnd, BitOr, BitXor, Shl, Shr, BitNot
Comparable, Ordering, compare, less, greater, equal
Iterator, DoubleEndedIterator, Iterable, Collect
Into, conversion trait, type conversion
Index, subscript, indexing, custom index
object safety, trait object, dyn
default type parameters, default associated types
inherent impl, trait impl, generic impl
trait resolution, method resolution, dispatch
Formattable, format spec, padding, alignment
```

---

### Section 04: Module System
**File:** `section-04-modules.md` | **Tier:** 1 | **Status:** Partial

```
module, modules, mod, import, use
pub, public, private, visibility, ::
export, re-export, pub use
relative import, module import, path
dependency graph, cycle detection, circular
prelude, auto-import, implicit import
entry point, main, @main, library, binary
extend, extension, extension methods
orphan rules, coherence
diamond import, re-export chains
test module, private access in tests
```

---

### Section 05: Type Declarations
**File:** `section-05-type-declarations.md` | **Tier:** 1 | **Status:** Partial

```
struct, record, fields, field access
sum type, enum, variant, tagged union
newtype, wrapper type, .inner
generic type, type parameter, <T>
Option, Some, None, optional
Result, Ok, Err, error handling
Ordering, Less, Equal, Greater
List, Map, Set, collections
Tuple, tuple type, (T, U)
Range, range type, iterator
Function type, (T) -> U, callable
Channel, Producer, Consumer
associated function, Type.method, static method
derive, #derive, attribute
pub type, public type
```

---

### Section 06: Capabilities System
**File:** `section-06-capabilities.md` | **Tier:** 2 | **Status:** Partial

```
capability, capabilities, effect, effects
uses, uses clause, capability declaration
with, with...in, capability provision
Suspend, async, concurrency marker
Http, network, request, fetch
FileSystem, file, read, write, io
Cache, caching, memoization
Clock, time, now, instant
Random, random number, rng
Logger, logging, log
Env, environment, env var
Unsafe, unsafe block, raw pointer
FFI, foreign function, external
Intrinsics, SIMD, bit operations, CPU features
def impl, default implementation
capability propagation, transitive
mock, testing, test capability
```

---

### Section 07A: Core Built-ins
**File:** `section-07A-core-builtins.md` | **Tier:** 2 | **Status:** Not Started

```
as, as?, type cast, conversion
As trait, TryAs trait, fallible cast
assert, assert_eq, assert_ne, assertion
assert_some, assert_none, assert_ok, assert_err
assert_panics, assert_panics_with
print, println, output, stdout
panic, panic!, abort, crash
compare, min, max, ordering
NaN, float comparison, infinity
todo, todo!, unimplemented
unreachable, unreachable!
dbg, debug print, debug output
repeat, infinite iterator
compile_error, build error
embed, has_embed, file embedding, compile-time embedding
PanicInfo, panic handler, @panic
drop_early, early destruction
```

---

### Section 07B: Option & Result
**File:** `section-07B-option-result.md` | **Tier:** 2 | **Status:** Not Started

```
Option, Some, None, optional value
Result, Ok, Err, error result
is_some, is_none, option check
is_ok, is_err, result check
map, transform, functor
unwrap_or, default value, fallback
ok_or, option to result
and_then, flatmap, chain
filter, predicate, conditional
map_err, error transform
ok, err, extract
trace, stack trace, error trace
trace_entries, TraceEntry
has_trace, context, error context
```

---

### Section 07C: Collections & Iteration
**File:** `section-07C-collections.md` | **Tier:** 2 | **Status:** Not Started

```
len, length, size, count
is_empty, empty check
List, array, vector, dynamic array
map, transform, projection
filter, where, predicate
fold, reduce, aggregate
find, search, lookup
any, all, exists, forall
first, last, head, tail
take, skip, slice
reverse, sort, order
contains, includes, has
push, append, add
concat, join, merge
Range, range iteration
Iterator, next, iteration
DoubleEndedIterator, rev, reverse iteration
Iterable, iter, into iterator
Collect, from_iter, collect
zip, pair, combine
chain, concatenate
flatten, flat_map
cycle, repeat, infinite
enumerate, index, numbered
Debug, debug string, repr
```

---

### Section 07D: Stdlib Modules
**File:** `section-07D-stdlib-modules.md` | **Tier:** 2 | **Status:** Not Started

```
std.validate, validation, validator
std.resilience, retry, backoff
exponential backoff, linear backoff
std.math, saturating, wrapping, checked
overflow, underflow, bounds
INT_MAX, INT_MIN, FLOAT_MAX
std.testing, test utilities
std.time, Instant, DateTime, Date, Time
Timezone, Weekday, calendar
ISO 8601, time format, parse time
std.json, JsonValue, Json trait
parse json, serialize json, JSON
std.fs, Path, FileInfo, file system
read file, write file, directory
glob, pattern matching, file pattern
temp file, temporary
std.crypto, hash, SHA256, SHA512
encrypt, decrypt, AES, RSA
sign, verify, signature, Ed25519
key exchange, X25519, Diffie-Hellman
secure random, CSPRNG
key derivation, PBKDF2, Argon2
Duration stdlib, Size stdlib
```

---

### Section 08: Patterns
**File:** `section-08-patterns.md` | **Tier:** 3 | **Status:** Not Started

```
pattern, patterns, matching
destructure, destructuring
match pattern, case
exhaustive, exhaustiveness
guard, if guard, condition
irrefutable, refutable
wildcard, _, ignore
binding pattern, capture
literal pattern, constant
```

---

### Section 09: Match
**File:** `section-09-match.md` | **Tier:** 3 | **Status:** Not Started

```
match, match expression, switch
case, arm, branch
pattern matching, dispatch
exhaustiveness, complete
guard, when, if
default, fallback, _
or pattern, |, alternative
at pattern, @, capture
range pattern, ..
```

---

### Section 10: Control Flow
**File:** `section-10-control-flow.md` | **Tier:** 3 | **Status:** Not Started

```
if, then, else, conditional
for, for loop, iteration
for...in, for...do, for...yield
while, while loop
loop, infinite loop
break, exit loop
continue, skip iteration
label, loop label, :name
break:label, continue:label
yield, generator, produce
break value, loop expression
```

---

### Section 11: FFI
**File:** `section-11-ffi.md` | **Tier:** 4 | **Status:** Not Started

```
FFI, foreign function interface
extern, external, native
extern "c", C FFI, C binding
extern "js", JavaScript FFI, JS binding
CPtr, C pointer, opaque pointer
JsValue, JavaScript value, JS handle
JsPromise, async JS, promise
c_char, c_int, c_float, C types
c_long, c_double, c_size
variadic, ..., C varargs
repr, #repr("c"), C layout
unsafe, unsafe block, raw
callback, function pointer
library, linking, dynamic, static
```

---

### Section 12: Variadic Functions
**File:** `section-12-variadic-functions.md` | **Tier:** 4 | **Status:** Not Started

```
variadic, varargs, variable arguments
...T, rest parameter, spread
variadic trait object, ...Trait
spread operator, ..., unpack
empty variadic, explicit type
```

---

### Section 13: Conditional Compilation
**File:** `section-13-conditional-compilation.md` | **Tier:** 5 | **Status:** Not Started

```
conditional compilation, cfg
#target, target condition
#!target, file-level target
os, linux, macos, windows
arch, x86_64, aarch64, wasm32
family, unix, windows, wasm
#cfg, configuration
debug, release, build mode
feature, feature flag
$target_os, $target_arch
$debug, $release, compile-time
compile_error, unsupported
```

---

### Section 14: Testing
**File:** `section-14-testing.md` | **Tier:** 5 | **Status:** Not Started

```
test, @test, testing
tests, test attribute
attached test, target test
floating test, tests _
ori test, test runner
#skip, skip test
#compile_fail, compile error test
#fail, expected failure
private test, :: access
test module, _test directory
```

---

### Section 15A: Attributes & Comments
**File:** `section-15A-attributes-comments.md` | **Tier:** 5 | **Status:** Not Started

```
attribute, #, annotation
#derive, derive attribute
#skip, #fail, #compile_fail
#target, #cfg, conditional
#repr, representation
comment, //, line comment
doc comment, documentation
```

---

### Section 15B: Function Syntax
**File:** `section-15B-function-syntax.md` | **Tier:** 5 | **Status:** Not Started

```
function, @, declaration
parameter, argument, param
default parameter, optional arg
named argument, arg:
return type, -> T
generic function, <T>
where clause, constraint
pub, public function
visibility, export
```

---

### Section 15C: Literals & Operators
**File:** `section-15C-literals-operators.md` | **Tier:** 5 | **Status:** Not Started

```
literal, value, constant
int literal, integer, number
float literal, decimal, 3.14
hex, 0x, hexadecimal
binary, 0b, bits
string, "...", text
escape, \n, \t, \r, \\
template string, `...`, interpolation
char, '...', character
byte, b'...', byte literal
duration literal, 100ms, 1s
size literal, 1kb, 1mb
list literal, [...], array
map literal, {...}, dictionary
tuple literal, (...), pair
operator, +, -, *, /, %
precedence, order, priority
overload, operator trait
```

---

### Section 15D: Bindings & Types
**File:** `section-15D-bindings-types.md` | **Tier:** 1 | **Status:** Not Started (escalated from Tier 5)

```
let, binding, variable
let $, immutable, constant
mutable, mut, changeable
destructure, pattern binding
type annotation, : T
shadowing, rebind, override
index assignment, list[i] = x, IndexSet, updated
field assignment, state.field = x, struct spread
compound assignment, +=, -=, *=
assignment target, lvalue, lhs
copy-on-write, COW, ARC optimization
```

---

### Section 16: Async
**File:** `section-16-async.md` | **Tier:** 6 | **Status:** Not Started

```
async, asynchronous
Suspend, suspend capability
await, waiting (not a keyword)
JsPromise, JavaScript promise
promise resolution, implicit await
non-blocking, concurrent
```

---

### Section 17: Concurrency
**File:** `section-17-concurrency.md` | **Tier:** 6 | **Status:** Not Started

```
concurrency, concurrent, parallel
channel, Producer, Consumer
CloneableProducer, CloneableConsumer
send, receive, message passing
Nursery, structured concurrency
task, spawn, parallel
Sendable, thread-safe, cross-task
buffer, bounded, backpressure
```

---

### Section 18: Const Generics
**File:** `section-18-const-generics.md` | **Tier:** 7 | **Status:** Not Started

```
const generic, $N, compile-time
const parameter, $N: int
const bound, N > 0
const constraint, where N
const arithmetic, N + 1
fixed size, [T, max N]
```

---

### Section 19: Existential Types
**File:** `section-19-existential-types.md` | **Tier:** 7 | **Status:** Not Started

```
existential, impl Trait
opaque type, hidden type
return impl, abstract return
where Item ==, associated constraint
static dispatch, monomorphization
```

---

### Section 20: Reflection
**File:** `section-20-reflection.md` | **Tier:** 8 | **Status:** Not Started

```
reflection, introspection
runtime type, type info
type name, type id
```

---

### Section 21A: LLVM Backend
**File:** `section-21A-llvm.md` | **Tier:** 8 | **Status:** Partial

```
LLVM, llvm, codegen
JIT, just-in-time, runtime compile
code generation, IR, intermediate
inkwell, LLVM bindings
type lowering, type mapping
expression codegen, statement codegen
control flow, basic block, phi
pattern matching codegen
function codegen, call, invoke
closure, lambda, capture
ARC, reference counting, memory
Drop, destructor, cleanup
optimization, O0, O1, O2, O3
debug info, DWARF, source map
runtime, ori_rt, built-in functions
```

---

### Section 21B: AOT Compilation
**File:** `section-21B-aot.md` | **Tier:** 8 | **Status:** Not Started

```
AOT, ahead-of-time, static compile
native, executable, binary
target triple, cross-compile
x86_64, aarch64, platform
linux, macos, windows, darwin
wasm, wasm32, WebAssembly, WASI
object file, ELF, Mach-O, COFF
linking, linker, ld, lld
static link, dynamic link
symbol, mangle, demangle
debug info, DWARF, PDB, dSYM
optimization, LTO, thin LTO
incremental, cache, parallel build
ori build, build command
```

---

### Section 22: Tooling
**File:** `section-22-tooling.md` | **Tier:** 8 | **Status:** Partial

```
tooling, CLI, command line
ori check, type check
ori test, test runner
ori fmt, formatter, format
ori run, execute, interpreter
ori build, compile, AOT
diagnostic, error message
warning, lint, suggestion
LSP, language server, IDE
package, pkg, dependency, dependencies
oripk.toml, oripk.lock, manifest
ori install, ori add, ori remove
ori publish, registry, cache
```

---

## Section Quick Reference

| ID | Title | Tier | File |
|----|-------|------|------|
| 00 | Full Parser Support | 0 | `section-00-parser.md` |
| 01 | Type System Foundation | 1 | `section-01-type-system.md` |
| 02 | Type Inference | 1 | `section-02-type-inference.md` |
| 03 | Traits and Implementations | 1 | `section-03-traits.md` |
| 04 | Module System | 1 | `section-04-modules.md` |
| 05 | Type Declarations | 1 | `section-05-type-declarations.md` |
| 06 | Capabilities System | 2 | `section-06-capabilities.md` |
| 07A | Core Built-ins | 2 | `section-07A-core-builtins.md` |
| 07B | Option & Result | 2 | `section-07B-option-result.md` |
| 07C | Collections & Iteration | 2 | `section-07C-collections.md` |
| 07D | Stdlib Modules | 2 | `section-07D-stdlib-modules.md` |
| 08 | Patterns | 3 | `section-08-patterns.md` |
| 09 | Match | 3 | `section-09-match.md` |
| 10 | Control Flow | 3 | `section-10-control-flow.md` |
| 11 | FFI | 4 | `section-11-ffi.md` |
| 12 | Variadic Functions | 4 | `section-12-variadic-functions.md` |
| 13 | Conditional Compilation | 5 | `section-13-conditional-compilation.md` |
| 14 | Testing | 5 | `section-14-testing.md` |
| 15A | Attributes & Comments | 5 | `section-15A-attributes-comments.md` |
| 15B | Function Syntax | 5 | `section-15B-function-syntax.md` |
| 15C | Literals & Operators | 5 | `section-15C-literals-operators.md` |
| 15D | Bindings & Types | 1 | `section-15D-bindings-types.md` |
| 16 | Async | 6 | `section-16-async.md` |
| 17 | Concurrency | 6 | `section-17-concurrency.md` |
| 18 | Const Generics | 7 | `section-18-const-generics.md` |
| 19 | Existential Types | 7 | `section-19-existential-types.md` |
| 20 | Reflection | 8 | `section-20-reflection.md` |
| 21A | LLVM Backend | 8 | `section-21A-llvm.md` |
| 21B | AOT Compilation | 8 | `section-21B-aot.md` |
| 22 | Tooling | 8 | `section-22-tooling.md` |
| 23 | Full Evaluator Support | 0 | `section-23-evaluator.md` |

---

### Section 23: Full Evaluator Support
**File:** `section-23-evaluator.md` | **Tier:** 0 | **Status:** In Progress

```
evaluator, interpreter, eval, runtime
ori_eval, evaluate, execution
operator evaluation, binary op, unary op
null coalesce, ??, coalesce operator
primitive methods, to_str, clone, hash
trait methods, Printable, Clone, Hashable
indexing, map lookup, string index
Option, Result, Some, None, Ok, Err
derived traits, #derive, Eq, Clone
control flow, break, continue, loop
stdlib, Queue, Stack, string slice
```

---

## Maintenance Guidelines

When updating the roadmap:

1. **Adding items to a section**: Add relevant keywords to that section's cluster above
2. **Creating a new section**: Add a new keyword cluster block following the existing format
3. **Removing a section**: Remove the corresponding keyword cluster and table entry
4. **Renaming/reorganizing**: Update file references, IDs, and keywords accordingly

Keep keyword clusters:
- **Concise**: 3-8 lines of comma-separated terms
- **Searchable**: Include both formal names and common aliases
- **Current**: Reflect actual content of the section file
