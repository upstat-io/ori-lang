---
paths:
  - "**"
---

# Ori Quick Reference

**Spec is authoritative**: `docs/ori_lang/v2026/spec/` (Clauses 1–27, Annexes A–E; `grammar.ebnf` for syntax, `operator-rules.md` for semantics)

> **Pending**: `capability-unification-generics-proposal` (approved 2026-02-20, revised 2026-03-04) will change: `#derive(Trait)` → `type T: Trait = {...}`. Bound syntax (`T: Trait`, `where T: Trait`, `trait Foo: Bar`) is UNCHANGED. Syntax below reflects CURRENT compiler behavior until implementation.

## Declarations

**Functions**: `@name (p: T) -> R = expr;` | `@name (p: T) -> R = { ... }` (no `;`) | `pub @name` | `@name<T>` | `@name<T: Trait>` | `@name<T: A + B>` | `where T: Clone` | `uses Capability` | `(x: int = 10)` defaults
**Variadics**: `@sum (nums: ...int) -> int` | receives as `[T]` | call: `sum(1, 2, 3)` | spread: `sum(...list)` | trait objects: `...Printable` | empty calls need explicit type for generics
**Clauses**: `@f (0: int) -> int = 1` then `@f (n) = n * f(n-1)` | `if guard` | exhaustive, top-to-bottom
**Constants**: `let $name = value;` | `pub let $name` | module-level must be `$`
**Const Functions**: `$name (p: T) -> R = expr` — pure, comptime, limits: 1M steps/1000 depth/100MB/10s
**Types**: `type N = { f: T }` struct | `A | B | C(f: T)` sum | `type N = Existing` newtype | `type N<T>` | `#derive(Eq)` | `pub type`
**Newtypes**: `type UserId = int` | construct: `UserId(42)` | `.inner` (always public) | no trait/method inheritance | `#derive(Eq, Clone)` required | zero cost
**Traits**: `trait N { @m (self) -> T }` | `@m (self) -> T = default` | `type Item` assoc | `type Output = Self` default | `trait C: P` | `@m () -> Self` assoc fn | `trait N<T = Self>` default type param
**Impls**: `impl T { @m }` inherent | `impl T: Trait` | `impl<T: B> C<T>: Trait` | `self` (mutable in methods — mutations propagate to caller) / `Self`
**Associated Functions**: `impl T { @new () -> T }` — no `self` | call: `Type.method()` | `Self` in return | generics: `Option<int>.some(v:)`
**Default Impls**: `pub def impl Trait { @m }` — stateless, one per trait/module, auto-bound, override with `with`
**Extensions**: `extend Type { @m (self) -> T }` | `extend<T: Bound> [T]` | `extend T where T: Bound` | `pub extend` | no statics/fields/override
**Resolution**: Diamond=single impl; Inherent>Trait>Extension; qualified: `Trait.method(v)`, `Type::Trait::Assoc`; extensions: `module.Type.method(v)`
**Object Safety**: No `Self` return/param (except receiver), no generic methods; safe: `Printable`, `Debug`, `Hashable`; unsafe: `Clone`, `Eq`, `Iterator`
**Tests**: `@t tests @fn () -> void` | `tests _` floating | `tests @a tests @b` multi | `#skip("r")` | `#compile_fail("e")` | `#fail("e")`

## Conditional Compilation

**Target**: `#target(os: "linux")` | `arch:` | `family:` | `any_os:` | `not_os:` | file-level: `#!target(...)`
**Config**: `#cfg(debug)` | `release` | `feature:` | `any_feature:` | `not_debug` | `not_feature:`
**Constants**: `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release` — false branch not type-checked

## Types

**Primitives**: `int` (i64), `float` (f64), `bool`, `str` (UTF-8), `char`, `byte`, `void`, `Never`
**Special**: `Duration` (`100ns`/`us`/`ms`/`s`/`m`/`h`), `Size` (`100b`/`kb`/`mb`/`gb`/`tb`)
**Collections**: `[T]` list, `[T, max N]` fixed-capacity, `{K: V}` map, `Set<T>`
**Compound**: `(T, U)` tuple (access: `.0`, `.1`), `()` unit, `(T) -> U` fn, `Trait` object, `impl Trait` existential
**Generic**: `Option<T>`, `Result<T, E>`, `Range<T>`, `Ordering`
**Const Generics**: `$N: int` | `@f<T, $N: int>` | `$B: bool` | `where N > 0` | `where N > 0 && N <= 100`
**Const Bounds**: comparison (`==`/`!=`/`<`/`<=`/`>`/`>=`), logical (`&&`/`||`/`!`), arithmetic (`+`/`-`/`*`/`/`/`%`), bitwise (`&`/`|`/`^`/`<<`/`>>`) | multiple `where` = AND
**Channels**: `Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>` (`T: Sendable`)
**Concurrency**: `Nursery`, `NurseryErrorMode` (`CancelRemaining | CollectAll | FailFast`)
**FFI**: `CPtr` (C opaque), `JsValue` (JS handle), `JsPromise<T>` (JS async)
**Rules**: No implicit conversions; overflow panics; `str[i]` → single-codepoint `str`

### Duration & Size

**Duration**: 64-bit nanoseconds; suffixes `ns`/`us`/`ms`/`s`/`m`/`h`; decimal syntax (`0.5s`=500ms, `1.5s`=1500ms)
**Size**: 64-bit bytes (non-negative); suffixes `b`/`kb`/`mb`/`gb`/`tb`; SI units (1000-based); decimal syntax (`1.5kb`=1500 bytes)
**Decimal literals**: Compile-time sugar using integer arithmetic (no floats); must result in whole base unit; `1.5ns`/`0.5b` = error
**Arithmetic**: `+`/`-`/`*`/`/`/`%`, unary `-` (Duration only; Size `-` panics if negative, unary `-` = compile error)
**Methods**: `.nanoseconds()`/`.microseconds()`/`.milliseconds()`/`.seconds()`/`.minutes()`/`.hours()` | `.bytes()`/`.kilobytes()`/`.megabytes()`/`.gigabytes()`/`.terabytes()` → `int`
**Factory**: `Duration.from_nanoseconds(ns:)`... | `Size.from_bytes(b:)`...
**Traits**: `Eq`, `Comparable`, `Hashable`, `Clone`, `Debug`, `Printable`, `Default` (`0ns`/`0b`), `Sendable`

### Never

Bottom type (uninhabited); coerces to any `T`
**Producers**: `panic(msg:)`, `todo()`, `unreachable()`, `break`, `continue`, `expr?` on Err/None, infinite `loop`
**Generics**: `Result<Never, E>` = always Err | `Result<T, Never>` = always Ok | `Option<Never>` = always None
**Restrictions**: Cannot be struct field; may be sum variant payload (unconstructable)

### Fixed-Capacity Lists

`[T, max N]` — inline-allocated, compile-time max N, dynamic length 0..N | `[T, max N] <: [T]`
**Methods**: `.capacity()`, `.is_full()`, `.remaining()`, `.push()` (panics), `.try_push()` → `bool`, `.push_or_drop()`, `.push_or_oldest()`, `.to_dynamic()`
**Conversion**: `.to_fixed<$N>()` panics | `.try_to_fixed<$N>()` → `Option`

### Existential Types (`impl Trait`)

`impl Trait where Assoc == Type` — opaque return type; concrete type hidden from callers
**Position**: return only | argument position: use generics instead
**Syntax**: `@f () -> impl Iterator where Item == int` | `impl A + B` multi-trait
**Where clause**: type-local (constraints on associated types, not type params)
**Dispatch**: static (monomorphized) — no vtable overhead
**Rules**: all return paths must yield same concrete type
**vs Trait objects**: `impl Trait` (static/single type) vs `Trait` (dynamic/any type at runtime)

## Literals

`42`, `1_000_000`, `0xFF`, `0b1010` | `3.14`, `2.5e-8` | `"hello"` (escapes: `\\\"\n\t\r\0\xHH\u{H}`) | `` `{name}` `` | `'a'`, `'\x41'` (escapes: `\\\'\n\t\r\0\xHH\u{H}`, `\xHH` restricted to `\x00`–`\x7F`) | `b'x'`, `b'\xFF'` (byte literal, escapes: `\\\'\n\t\r\0\xHH`, full `\x00`–`\xFF`, no `\u{}` or `\"`) | `true`/`false` | duration/size literals | `[1, 2]`, `[...a, ...b]` | `{key: v}`, `{"key": v}`, `{[expr]: v}`, `{...a, ...b}` | `Point { x, y }`, `{ ...p, x: 10 }`

## Operators (precedence high→low)

1. `.` `[]` `()` `?` `as` `as?` — 2. `**` (right) — 3. `!` `-` `~` — 4. `*` `/` `%` `div` `@` — 5. `+` `-` — 6. `<<` `>>` — 7. `..` `..=` `by` — 8. `<` `>` `<=` `>=` — 9. `==` `!=` — 10. `&` — 11. `^` — 12. `|` — 13. `&&` — 14. `||` — 15. `??` — 16. `|>` (pipe)

**Unary**: `!` (Not), `-` (Neg), `~` (BitNot) | **Bitwise**: `&`/`|`/`^` (BitAnd/Or/Xor), `<<`/`>>` (Shl/Shr)
**Shift overflow**: negative count panics; count ≥ bit width panics; `1 << 63` panics
**Operator traits**: desugar to trait methods; user types implement for operator support
**Compound assignment**: `x op= y` desugars to `x = x op y` (parser-level) | `+=` `-=` `*=` `/=` `%=` `**=` `@=` `&=` `|=` `^=` `<<=` `>>=` `&&=` `||=` | statement, not expression | target must be mutable (no `$`) | `&&=`/`||=` preserve short-circuit
**Pipe**: `x |> f(a: v)` fills single unspecified param | `x |> .method()` calls method on piped value | `x |> (a -> expr)` lambda fallback | prec 16 (lowest) | left-assoc | desugars to let-binding + call in type checker | "unspecified" = no value AND no default

## Expressions

**Conditionals**: `if c then e else e` | `if c then e` (void)
**Bindings**: `let x = v` mutable | `let $x` immutable | `let x: T` | shadowing OK | `let { x, y }` | `let { x: px }` | `let (a, b)` | `let [$h, ..t]` | `let [$h, ..$t]` immutable rest
**Indexing**: `list[0]`, `list[# - 1]` (`#`=length, panics OOB) | `map["k"]` → `Option<V>`
**Index/Field Assignment**: `list[i] = x` → `list = list.updated(key: i, value: x)` | `state.field = x` → `state = { ...state, field: x }` | mixed chains: `state.items[i] = x`, `list[i].name = x` | compound: `list[i] += 1` | root must be mutable (non-`$`)
**Access**: `v.field`, `v.0` (tuple), `v.method(arg: v)` — named args required except: fn variables, single-param with inline lambda
**Argument Punning**: `f(x:)` = `f(x: x)` when variable matches param name | `f(x:, y: 42)` mixed | trailing `:` distinguishes from positional `f(x)`
**Lambdas**: `x -> x + 1` | `(a, b) -> a + b` | `() -> 42` | `(x: int) -> int = x * 2` — capture by value
**Ranges**: `0..10` excl | `0..=10` incl | `0..10 by 2` | descending: `10..0 by -1` | infinite: `0..`, `0.. by -1` | int only
**Blocks**: `{ let $x = 1; x + 2 }` — `;` terminates statements, last expression (no `;`) is value | all `;` = void block | `ori fmt` enforces blank line before result | empty `{ }` = empty map
**Loops**: `while c do e` | `for i in items do e` | `for x in items yield x * 2` | `for x in items if g yield x` | nested `for` | `loop { body }` + `break`/`continue` | `break value` | `continue value`
**While**: `while condition do body` — sugar for `loop { if !condition then break; body }` | type: `void` | no `while...yield` | `break value` error (E0860) | `continue value` error (E0861)
**Loop body**: block expression; `loop { a \n b \n c }` for sequences | type: `void` (break no value), inferred (break value), `Never` (no break) | `continue value` error (E0861)
**Yield control**: `continue` skips | `continue value` substitutes | `break` stops | `break value` adds final | `{K: V}` from `(K, V)` tuples
**Labels**: `loop:name` | `for:name` | `while:name` | `block:name` | `break:name` | `continue:name` | no shadowing | `continue:name value` in yield → outer
**Labeled blocks**: `block:name { body }` — early exit via `break:name value` | type = unified exit paths | bare `break` is loop-only (not block) | `continue:block_label` = error | transparent to `break:loop_label`/`continue:loop_label`
**Spread**: `[...a, ...b]` | `{...a, ...b}` | `P { ...orig, x: 10 }` — later wins, literal contexts only | `fn(...list)` into variadic only

## Block expressions

**Semicolons**: Rust-style — `;` terminates statements; last expression (no `;`) is block value; all `;` = void block
**Semicolon rule**: Ends with `}`? No `;`. Everything else: `;`. Applies to `use`, `let $`, functions, types, methods.
**Blocks**: `{ let $x = 1; let $y = 2; x + y }` — `;` on statements, no `;` on result
**Match**: `match expr { P1 -> e1, P2 -> e2 }` — scrutinee before block, comma-separated arms (trailing comma optional)
**Try**: `try { let $x = f()?; Ok(x) }` — error-propagating block
**Contracts**: `pre(condition)` | `pre(condition | "message")` | `post(r -> condition)` — on function declaration, between signature and `=`
**function_exp**: `recurse(condition:, base:, step:, memo:, parallel:)` | `parallel(tasks:, max_concurrent:, timeout:)` → `[Result]` | `spawn(tasks:, max_concurrent:)` → `void` | `timeout(op:, after:)` | `cache(key:, op:, ttl:)` | `with(acquire:, action:, release:)` | `for(over:, match:, default:)` | `catch(expr:)` → `Result<T, str>` | `nursery(body:, on_error:, timeout:)`
**Channels**: `channel<T>(buffer:)` → `(Producer, Consumer)` | `channel_in` | `channel_out` | `channel_all`
**Conversions**: `42 as float` infallible | `"42" as? int` fallible → `Option`
**Match patterns**: literal | `x` | `_` | `Some(x)` | `{ x, y }` | `[a, ..rest]` | `1..10` | `A | B` | `x @ pat` | `x if guard` | variant punning: `Circle(radius:)` = `Circle(radius: radius)`, `Some(value:)` = `Some(value: value)`
**Exhaustiveness**: match exhaustive; guards need `_`; `let` patterns irrefutable

## Imports

**Relative**: `use "./math" { add };` | `"../utils"` | `"./http/client"`
**Module**: `use std.math { sqrt };` | `use std.net.http as http;`
**Private**: `use "./m" { ::internal };` | **Alias**: `{ add as plus }` | **Re-export**: `pub use`
**Without default**: `use "m" { Trait without def };` — import without `def impl`
**Extensions**: `extension std.iter.extensions { Iterator.count }` — method-level, no wildcards | `pub extension`

## FFI

**Native (C)**: `extern "c" from "lib" { @_sin (x: float) -> float as "sin" }` | `from` specifies library | `as` maps name
**JavaScript**: `extern "js" { @_sin (x: float) -> float as "Math.sin" }` | `extern "js" from "./utils.js"`
**C Variadics**: `extern "c" { @printf (fmt: CPtr, ...) -> c_int }` — untyped, requires `unsafe`, platform va_list ABI
**Types**: `CPtr` opaque | `Option<CPtr>` nullable | `JsValue` handle | `JsPromise<T>` async
**C Types**: `c_char`, `c_short`, `c_int`, `c_long`, `c_longlong`, `c_float`, `c_double`, `c_size`
**Layout**: `#repr("c")` C-compatible | `#repr("packed")` no padding | `#repr("transparent")` same as single field | `#repr("aligned", N)` minimum alignment (power of two) | struct types only; newtypes implicitly transparent
**Unsafe**: `unsafe { ptr_read(...) }` | **Capability**: `uses Unsafe` (marker, like `Suspend` — cannot be bound via `with...in`)
**Async WASM**: `JsPromise<T>` implicitly resolved at binding sites | **Compile Error**: `compile_error("msg")`

### Deep FFI (opt-in annotations on extern blocks)

**Error Protocols**: `extern "c" from "lib" #error(errno) { ... }` — block-level; `#error(none)` per-function opt-out
**Error Variants**: `#error(errno | nonzero | null | negative | success: N | none)` — auto-generates `Result<T, FfiError>`
**FfiError**: `use std.ffi { FfiError }` — `{ code: int, message: str, source: str }`
**Out Params**: `@f (name: str, db: out CPtr) -> c_int` — `out` params folded into return type
**Ownership**: `owned` / `borrowed` on params/returns | str returns default to `borrowed` (copy, don't free)
**Free**: `#free(fn)` on block or per-function — auto-generates `Drop` impl for `owned CPtr`
**[byte] Elision**: `[byte]` in extern generates adjacent `(ptr, len)` C args | `mut [byte]` generates `(ptr, &len)`
**Parametric FFI**: `uses FFI("sqlite3")` per-library | `uses FFI` shorthand for all | each `from "lib"` is distinct capability
**Mocking**: `with FFI("lib") = handler { fn: (...) -> T = ..., } in { ... }` — handler-based mock; stateless is sugar for `handler(state: ())`

## Capabilities

**Declare**: `@f (...) -> T uses Http = ...` | `uses FileSystem, Suspend`
**Provide**: `with Http = RealHttp { } in expr` | `with Http = mock, Cache = mock in expr`
**Stateful Handlers**: `with Cap = handler(state: init) { op: (s) -> (s', val), ... } in expr` — state replaces `self`; returns `(S, R)` tuple; frame-local mutable state; `with...in` returns body type only
**Handler Rules**: context-sensitive keyword; single state value (compose via structs); all trait methods required (defaults used if omitted); no `self`; errors E1204-E1207
**Resolution**: with...in > imported `def impl` > module-local `def impl`
**Suspend**: `uses Suspend` = may suspend; no `uses` = sync; concurrency via `parallel(...)`
**Standard**: `Http`, `FileSystem`, `Clock`, `Random`, `Crypto`, `Cache`, `Print` (default), `Logger`, `Env`, `Intrinsics`, `Suspend`, `FFI`
**Intrinsics**: Generic SIMD/bit ops; `Intrinsics.simd_add(a:, b:)` (monomorphized by `[T, max N]`), `count_ones(value:)`, `cpu_has_feature(feature:)`; comparisons return `Mask<$N>` (methods: `bits`, `any`, `all`, `count`, `first_set`; operators: `&`, `|`, `~`)
**Capsets**: `capset Net = Http, Dns, Tls` — transparent alias, expanded in `uses` before type checking; `@f uses Net` expands to `@f uses Http, Dns, Tls`; capsets can include other capsets; not a trait (no `impl`, no `with`, no `def impl`)

## Comments

`// comment` — own line only | Doc: `// Desc` | `// * name:` | `// ! Error:` | `// > expr -> result`

## Formatting

4 spaces, 100 char limit, trailing commas multi-line only | `;` terminates statements in blocks, `use`, `let $`, expression-bodied declarations; block body `}` = no `;` | Space around: binary ops, arrows, colons/commas, `pub`, all braces `{ }`, `as`/`by`/`|`/`with`/`+`, `=` in `<T = Self>`, `??`, compound `+=` | No space: parens/brackets, `.`/`..`/`?`/`...`, empty delimiters, before `;`, labels `:`, punning `name:` | Break at 100; blocks 4-space indent; blank line before result in setup+result blocks; `match`/`try`/`recurse`/`parallel`/`spawn`/`nursery` always stacked; `timeout`/`cache`/`catch` width-based | Params/args/generics/where/fields/variants one-per-line; chains break at `.method()` (all-or-nothing); binary break before op; `if...then` together, `else` newline; chained `else if` each on own line; `for...yield`/`do` inline if fits | File order: file attrs → imports (stdlib→relative, sorted alpha) → constants → user-ordered rest | Attrs canonical order: `#target`/`#cfg` → `#repr` → `#derive` → `#skip`/`#compile_fail`/`#fail` | Traits: assoc types → required methods → defaults | Impls: assoc types → methods in trait order | Parens always preserved; never removed by formatter

## Keywords

**Reserved (35)**: `as break continue def div do else extend extension extern false for if impl in let loop match pub self Self suspend tests then trait true type unsafe use uses void where while with yield`
**Reserved (future)**: `asm inline static union view` (reserved for future low-level features)
**Context-sensitive**: `args block body buffer by cache catch default embed expr from handler has_embed map max nursery on_error over parallel pre post recurse spawn state timeout try without`
**Built-in names**: `int float str byte bool len is_empty is_some is_none is_ok is_err assert assert_eq assert_ne assert_some assert_none assert_ok assert_err assert_panics assert_panics_with compare min max print panic todo unreachable dbg compile_error embed has_embed`

## Prelude

**Types**: `Option<T>` (`Some`/`None`), `Result<T, E>` (`Ok`/`Err`), `Error`, `TraceEntry`, `Ordering`, `PanicInfo`, `CancellationError`, `CancellationReason`, `FormatSpec`, `Alignment`, `Sign`, `FormatType`
**Traits**: `Eq`, `Comparable`, `Hashable`, `Printable`, `Formattable`, `Debug`, `Clone`, `Default`, `Drop`, `Len`, `IsEmpty`, `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`, `Into`, `Traceable`, `Index`, `Sendable`, `Value`

**Built-ins**: `print(msg:)`, `len(collection:)`, `is_empty(collection:)`, `is_some/is_none(option:)`, `is_ok/is_err(result:)`, `assert(condition:)`, `assert_eq(actual:, expected:)`, `assert_ne(actual:, unexpected:)`, `assert_some/none/ok/err(...)`, `assert_panics(expr:)`, `assert_panics_with(expr:, message:)`, `panic(msg:)`→`Never`, `todo()`/`todo(reason:)`→`Never`, `unreachable()`/`unreachable(reason:)`→`Never`, `dbg(value:)`/`dbg(value:, label:)`→`T`, `compare(left:, right:)`→`Ordering`, `min/max(left:, right:)`, `hash_combine(seed:, value:)`→`int`, `repeat(value:)`→iter (`T: Clone`), `is_cancelled()`→`bool`, `compile_error(msg:)`, `drop_early(value:)`, `embed(path)`→type-driven (`str`/`[byte]`), `has_embed(path)`→`bool`

**Option**: `.map(transform:)`, `.unwrap_or(default:)`, `.ok_or(err:)`, `.and_then(then:)`, `.filter(predicate:)`
**Result**: `.map(transform:)`, `.map_err(transform:)`, `.unwrap_or(default:)`, `.ok()`, `.err()`, `.and_then(then:)`, `.context(msg:)`, `.trace()`→`str`, `.trace_entries()`→`[TraceEntry]`, `.has_trace()`
**Error**: `.trace()`, `.trace_entries()`, `.has_trace()`
**Ordering**: `Less | Equal | Greater` — `.is_less/equal/greater()`, `.is_less_or_equal/greater_or_equal()`, `.reverse()`, `.then(other:)`, `.then_with(f:)`; default `Equal`; order `Less < Equal < Greater`; impls Eq, Comparable, Clone, Debug, Printable, Hashable, Default

**Printable**: `@to_str (self) -> str` — required for `` `{x}` ``; all primitives impl
**Formattable**: `@format (self, spec: FormatSpec) -> str` — blanket for Printable; spec: `[[fill]align][sign][#][0][width][.precision][type]`; align `<>^`; sign `+ - `; types `bxXoeEf%`; `#` prefix; `0` pads
**Debug**: `@debug (self) -> str` — escaped strings, derivable | **Clone**: `@clone (self) -> Self` — all primitives/collections, derivable
**Iterator**: `type Item; @next (self) -> (Option<Self.Item>, Self)` — fused, copy elision, lazy
**DoubleEndedIterator**: `trait: Iterator { @next_back (self) -> (Option<Self.Item>, Self) }`
**Iterable**: `type Item; @iter (self) -> impl Iterator` | **Collect**: `@from_iter (iter: impl Iterator) -> Self`
**Iterator methods**: `.map`, `.filter`, `.fold`, `.find`, `.for_each`, `.collect`, `.count`, `.any`, `.all`, `.take`, `.skip`, `.enumerate`, `.zip`, `.chain`, `.flatten`, `.flat_map`, `.cycle`, `.join`
**DoubleEnded methods**: `.rev`, `.last`, `.rfind`, `.rfold`
**Infinite**: `repeat(value:)`, `(0..).iter()` — bound with `.take(count:)` before `.collect()`
**Into**: `@into (self) -> T` — lossless, explicit `.into()`, standard: str→Error, int→float, Set<T>→[T]; no identity/chaining
**Traceable**: `@with_trace`, `@trace`→`str`, `@trace_entries`→`[TraceEntry]`, `@has_trace`
**TraceEntry**: `{ function, file, line, column: int }` — `@` prefix; most recent first
**PanicInfo**: `{ message, location: TraceEntry, stack_trace: [TraceEntry], thread_id: Option<int> }`
**Drop**: `@drop (self) -> void` — refcount zero; not async; panic during unwind aborts
**Index**: `@index (self, key: Key) -> Value` — `x[k]`→`x.index(key: k)`; return `T`/`Option<T>`/`Result<T, E>`; `#` built-in only; multiple impls per type OK: `impl Index<int, V>` + `impl Index<str, V>` disambiguated by key type at compile time
**Eq**: `@equals (self, other: Self) -> bool` — reflexive/symmetric/transitive; derives `==`/`!=`
**Comparable**: `trait: Eq { @compare (self, other: Self) -> Ordering }` — total order; derives `<`/`<=`/`>`/`>=`; NaN > all; `None < Some`; `Ok < Err`
**Hashable**: `trait: Eq { @hash (self) -> int }` — `a == b` ⇒ same hash; +0.0/-0.0 same; use `hash_combine`
**Operator traits**: `Add`/`Sub`/`Mul`/`Div`/`FloorDiv`/`Rem`/`Pow<Rhs = Self>` — binary; `MatMul<Rhs = Self>` — matrix multiply (`@`); `Neg`/`Not`/`BitNot` — unary; `BitAnd`/`BitOr`/`BitXor<Rhs = Self>`, `Shl`/`Shr<Rhs = int>` — bitwise; `As<T>`/`TryAs<T>` — conversion (`as`/`as?`); all default `type Output = Self`
**Operator methods**: `add`/`subtract`/`multiply`/`divide`/`floor_divide`/`remainder`/`power` — arithmetic; `matrix_multiply` — matmul (`@`); `negate`/`not`/`bit_not` — unary; `bit_and`/`bit_or`/`bit_xor`/`shift_left`/`shift_right` — bitwise; `as`/`try_as` — conversion
**Sendable**: marker trait, auto-derived by compiler; all fields must be `Sendable`, no interior mutability, no non-Sendable captures; required for channel types `T: Sendable`; cannot be implemented manually
**Value**: `trait Value: Clone, Eq` — marker trait; inline storage, bitwise copy, no ARC, no Drop; all fields must be `Value`; cannot be implemented manually; auto-satisfies `Clone` + `Sendable`; warning >256 bytes, error >512 bytes; primitives (`int`/`float`/`bool`/`char`/`byte`/`void`/`Duration`/`Size`/`Ordering`) implicitly `Value`; `str`/`[T]`/`{K:V}`/`Set<T>` never `Value`; syntax: `type Point: Value, Eq = { x: float, y: float }`
**List methods**: `.map(transform:)`, `.filter(predicate:)`, `.fold(initial:, op:)`, `.find(where:)`, `.any(predicate:)`, `.all(predicate:)`, `.first()`, `.last()`, `.take(count:)`, `.skip(count:)`, `.slice(start, end)`, `.push(value)`, `.pop()`, `.insert(index, value)`, `.remove(index)`, `.reverse()`, `.sort()` (`T: Comparable`), `.contains(value:)` (`T: Eq`), `.updated(key:, value:)`, `.len()`, `.is_empty()`
**String methods**: `.split(sep:)`, `.trim()`, `.substring(start:, end:)`, `.upper()`, `.lower()`, `.starts_with(prefix:)`, `.ends_with(suffix:)`, `.contains(substr:)`, `.len()`, `.is_empty()`, `.as_bytes()`→`[byte]` (zero-copy), `.to_bytes()`→`[byte]` (copy), `.byte_len()`→`int`; assoc: `str.from_utf8(bytes:)`→`Result<str, Error>`, `str.from_utf8_unchecked(bytes:)`→`str` (unsafe)
**Char methods**: `.is_alphabetic()`, `.is_digit()`, `.is_alphanumeric()`, `.is_whitespace()`, `.is_uppercase()`, `.is_lowercase()`, `.is_ascii()`, `.is_control()` (Unicode); `.is_ascii_alphabetic()`, `.is_ascii_digit()`, `.is_ascii_alphanumeric()`, `.is_ascii_whitespace()`, `.is_ascii_uppercase()`, `.is_ascii_lowercase()`, `.is_ascii_hex_digit()`, `.is_ascii_punctuation()`, `.is_ascii_control()` (ASCII); `.to_ascii_uppercase()`, `.to_ascii_lowercase()`, `.to_digit(radix:)`→`Option<int>` (radix 2..=36, panic on invalid)
**Byte methods**: `.is_ascii()`, `.is_ascii_alpha()`/`.is_alpha()`, `.is_ascii_digit()`/`.is_digit()`, `.is_ascii_alphanumeric()`/`.is_alnum()`, `.is_ascii_whitespace()`/`.is_whitespace()`, `.is_ascii_uppercase()`/`.is_upper()`, `.is_ascii_lowercase()`/`.is_lower()`, `.is_ascii_hex_digit()`/`.is_hex_digit()`, `.is_ascii_punctuation()`, `.is_ascii_control()`; `.to_ascii_uppercase()`, `.to_ascii_lowercase()`, `.to_digit(radix:)`→`Option<int>`
**Reflect** (opt-in via `#derive(Reflect)`): `@type_info`→`TypeInfo`, `@field_count`→`int`, `@field_by_index`/`@field_by_name`→`Option<Unknown>`; `Unknown` for type-erased downcasting; all primitives impl; read-only, no method reflection
