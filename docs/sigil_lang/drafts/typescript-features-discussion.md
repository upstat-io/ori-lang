# TypeScript Features Discussion for Sigil

**Purpose**: Evaluate TypeScript language concepts for potential adoption in Sigil.
**Process**: We will discuss each feature interactively. After discussion, I'll update this document with our decision and rationale.

---

## Feature Categories

### 1. Type System Features

#### 1.1 Discriminated Unions (Tagged Unions)
**What it is**: Union types with a common "discriminant" property that TypeScript uses for automatic type narrowing in switch/if statements.
```typescript
type Shape =
  | { kind: "circle"; radius: number }
  | { kind: "square"; side: number };

function area(s: Shape) {
  switch (s.kind) {
    case "circle": return Math.PI * s.radius ** 2;  // TypeScript knows s.radius exists
    case "square": return s.side ** 2;              // TypeScript knows s.side exists
  }
}
```
**Sigil relevance**: Sigil already has sum types. Question is whether the auto-narrowing behavior is interesting.

**Discussion**: Sigil's sum types + `match` already provide this capability. The key question was whether exhaustive matching should be enforced. Given Sigil's explicit philosophy (similar to requiring `Result<T, E>` handling), exhaustive matching aligns perfectly. If you add a new variant, every `match` becomes a compile error until updated. The `_` wildcard remains available for intentional catch-alls.

**Decision**: **No new syntax needed.** Sigil's existing sum types + `match` are sufficient. Compiler must enforce exhaustive matching (error if cases are missing). This is consistent with Sigil's explicit, fail-at-compile-time philosophy.

---

#### 1.2 Conditional Types
**What it is**: Types that choose between options based on a type-level condition.
```typescript
type IsArray<T> = T extends any[] ? true : false;
type NonNullable<T> = T extends null | undefined ? never : T;
```
**Sigil relevance**: Enables type-level programming. Could be useful for generic constraints.

**Discussion**: Adds significant complexity to the type system. TypeScript needed this partly due to JavaScript's dynamic nature. Sigil's stricter foundation reduces the need. Can make error messages confusing and leads toward Turing-complete types (a footgun). Most users would never need this.

**Decision**: **Rejected.** Too complex for the value it provides. Sigil's trait bounds and explicit generics are sufficient.

---

#### 1.3 Mapped Types
**What it is**: Create new types by transforming properties of existing types.
```typescript
type Readonly<T> = { readonly [K in keyof T]: T[K] };
type Partial<T> = { [K in keyof T]?: T[K] };
type Nullable<T> = { [K in keyof T]: T[K] | null };
```
**Sigil relevance**: Powerful for creating type utilities. Related to reflection/metaprogramming.

**Discussion**: Mapped types are deeply connected to conditional types and require `keyof` and type-level iteration. Without those foundations, mapped types don't work. They add metaprogramming complexity that doesn't fit Sigil's explicit philosophy. If needed, derive macros (`#[derive(Partial)]`) could address similar use cases more explicitly.

**Decision**: **Rejected.** Too coupled to conditional types and type-level programming. Derive macros are a better fit for Sigil if this need arises.

---

#### 1.4 Template Literal Types
**What it is**: Type-level string manipulation using template syntax.
```typescript
type EventName<T extends string> = `on${Capitalize<T>}`;
type ClickEvent = EventName<"click">;  // "onClick"

type CssValue = `${number}px` | `${number}em`;
```
**Sigil relevance**: Unique to TypeScript. Enables typed string DSLs, API route typing.

**Discussion**: Very TypeScript-specific, designed for the JavaScript ecosystem (event handlers, CSS, routing). Sigil isn't primarily a web language. Adds significant type checker complexity for niche use cases. If typed string patterns are needed, newtypes with validation or compile-time regex would be simpler and more explicit.

**Decision**: **Rejected.** Too niche and web-ecosystem-specific. Newtypes with validation are a better fit for Sigil.

---

#### 1.5 Index Signatures & Symbol Keys
**What it is**: Allow arbitrary keys in types, including symbol and template pattern keys.
```typescript
interface Dict<T> { [key: string]: T }
interface SymbolDict { [key: symbol]: string }
type EventHandlers = { [K in `on${string}`]: () => void }
```
**Sigil relevance**: Sigil has map types `{K: V}`. Question is whether more flexible key typing is useful.

**Discussion**: Sigil's clean separation between structs (static keys) and maps (dynamic keys) is a strength. Index signatures blur this distinction. If you need both, explicit composition is clearer: `type Config = { name: str, extra: {str: unknown} }`. Symbol keys add complexity without clear benefit in Sigil's context.

**Decision**: **Rejected.** Keep the clean struct/map separation. Explicit composition handles mixed cases better.

---

#### 1.6 Branded/Nominal Types
**What it is**: Pattern for creating nominally distinct types in a structural type system.
```typescript
type UserId = number & { readonly __brand: unique symbol };
type OrderId = number & { readonly __brand: unique symbol };
// UserId and OrderId are not assignable to each other
```
**Sigil relevance**: Sigil uses structural typing. Branded types add nominal safety for IDs, currencies, etc.

**Discussion**: Checked spec and design docs. **Sigil already has this!** The spec (06-types.md:432) states: "Newtypes are nominally distinct from their underlying type." Syntax: `type UserId = str` creates a nominal newtype, not a structural alias. `UserId` and `Email` (both wrapping `str`) are distinct types - passing one where the other is expected is a compile error. Construction via `UserId("...")`, unwrapping via `.unwrap()`.

**Decision**: **Already implemented.** No new syntax needed. Consider clarifying in CLAUDE.md that newtypes are nominal (not just aliases).

---

#### 1.7 Variance Annotations (`in`/`out`)
**What it is**: Explicit covariance/contravariance declarations on generic type parameters.
```typescript
interface Producer<out T> { produce(): T }      // covariant
interface Consumer<in T> { consume(x: T): void } // contravariant
interface Processor<in I, out O> { process(i: I): O }
```
**Sigil relevance**: Makes generic type relationships explicit. Catches errors at declaration site.

**Discussion**: Considered three options: (1) inferred variance like Rust, (2) required annotations like Scala's `+`/`-`, (3) optional annotations like TypeScript's `in`/`out`. Explicit annotations add syntax complexity and require users to understand variance theory. Most users won't need this. Rust's approach works well in practice - the compiler infers variance from usage.

**Decision**: **Use Rust-style inferred variance.** Compiler determines variance from how type parameters are used. No explicit annotations needed. Simpler for users, less syntax to learn.

---

#### 1.8 `infer` Keyword (Type Inference in Conditionals)
**What it is**: Extract/infer types within conditional type expressions.
```typescript
type ReturnType<T> = T extends (...args: any) => infer R ? R : never;
type UnwrapPromise<T> = T extends Promise<infer U> ? U : T;
type FirstArg<T> = T extends (x: infer A, ...args: any) => any ? A : never;
```
**Sigil relevance**: Powerful type-level pattern matching. Enables extracting types from complex structures.

**Discussion**: The `infer` keyword only exists within conditional type expressions (`T extends X ? Y : Z`). Since conditional types were rejected, there's no context for `infer` to operate in.

**Decision**: **Rejected.** Depends on conditional types, which were rejected.

---

### 2. Type Narrowing & Control Flow

#### 2.1 Type Guards (User-Defined)
**What it is**: Functions that narrow types via a special return type predicate.
```typescript
function isString(x: unknown): x is string {
  return typeof x === "string";
}
if (isString(value)) {
  // value is string here
}
```
**Sigil relevance**: Enables custom narrowing logic. Useful for validation, parsing.

**Discussion**: Sigil's `match` with type patterns already provides narrowing for sum types. Sigil doesn't have `unknown`/`any`, so the primary use case (narrowing dynamic types) doesn't apply. For validation of external data (JSON parsing), returning `Result<T, ParseError>` is more explicit and fits Sigil's error handling model better than a type predicate.

**Decision**: **Rejected.** Use `match` for sum type narrowing, `Result<T, E>` for validation/parsing.

---

#### 2.2 Assertion Functions (`asserts`)
**What it is**: Functions that assert a condition holds, affecting control flow analysis.
```typescript
function assertIsString(x: unknown): asserts x is string {
  if (typeof x !== "string") throw new Error();
}
assertIsString(value);
// value is string after this point
```
**Sigil relevance**: Integrates assertions with type system. Narrowing without if-blocks.

**Discussion**: Sigil handles this through returning unwrapped values directly (e.g., `@assert_some<T> (opt: Option<T>) -> T`) or using `panic()`. This is cleaner than TypeScript's approach of mutating the type environment in-place - no "spooky action at a distance." The `?` operator handles error propagation for `Result` types.

**Decision**: **Rejected.** Sigil's approach (return unwrapped value or panic) is more explicit and achieves the same goal.

---

#### 2.3 `never` Type & Exhaustiveness Checking
**What it is**: Type representing impossible values. Used to ensure all cases are handled.
```typescript
function assertNever(x: never): never {
  throw new Error("Unexpected: " + x);
}
switch (shape.kind) {
  case "circle": ...
  case "square": ...
  default: assertNever(shape); // Error if new variant added but not handled
}
```
**Sigil relevance**: Sigil has `Never` type. Question is whether exhaustiveness checking pattern is useful.

**Discussion**: Sigil already has the `Never` type and we decided (Feature 1.1) that `match` must enforce exhaustiveness. The `assertNever` pattern only exists in TypeScript because switches aren't exhaustive by default. In Sigil, the compiler enforces this directly - no runtime helper needed.

**Decision**: **Already implemented.** `Never` type exists, exhaustive `match` enforced by compiler. No additional work needed.

---

#### 2.4 Control Flow Analysis (Aliased Conditions)
**What it is**: TypeScript tracks narrowing through variable assignments and aliased conditions.
```typescript
const isValid = value !== null;
if (isValid) {
  // value is narrowed here even though check was aliased
}
```
**Sigil relevance**: More sophisticated narrowing. Reduces need for repeated checks.

**Discussion**: This is a compiler quality-of-life improvement, not a syntax change. The compiler should be smart enough to track narrowing through variable aliases (e.g., `let is_valid = x != None` should narrow `x` when `is_valid` is checked). Makes code more natural and DRY.

**Decision**: **Compiler goal.** The Sigil compiler should track aliased conditions for type narrowing. No syntax change required - this is an implementation enhancement.

---

### 3. Object & Class Features

#### 3.1 Declaration Merging
**What it is**: Multiple declarations of same name merge into one definition.
```typescript
interface User { name: string }
interface User { age: number }
// User now has both name and age
```
**Sigil relevance**: Controversial feature. Enables extension but can cause confusion.

**Discussion**: Widely considered a footgun in TypeScript. Makes code harder to understand - a type's shape is scattered across files. Directly violates Sigil's explicitness philosophy. Sigil has `extend` for adding methods to traits, but extending data (fields) should require explicit composition.

**Decision**: **Rejected.** Violates explicitness. Type definitions must be complete in one place.

---

#### 3.2 Module Augmentation
**What it is**: Extend types from other modules without modifying original source.
```typescript
declare module "express" {
  interface Request { user?: User }
}
```
**Sigil relevance**: Useful for adding types to third-party code. Extension mechanism.

**Discussion**: Same problems as declaration merging - type definitions scattered across the codebase. Alternatives are more explicit: wrapper types (`type MyRequest = { base: Request, user: User }`) or generics (`Request<UserContext>`).

**Decision**: **Rejected.** Same reasoning as declaration merging. Use wrapper types or generics instead.

---

#### 3.3 Abstract Classes
**What it is**: Classes that can't be instantiated directly; provide partial implementation.
```typescript
abstract class Animal {
  abstract makeSound(): void;
  move() { console.log("moving"); }
}
```
**Sigil relevance**: Sigil uses traits. Question is whether abstract classes add value.

**Discussion**: Sigil's traits already provide everything abstract classes offer: required methods (no body), default implementations, and multiple inheritance (better than single class inheritance). Adding abstract classes would create two ways to do the same thing, violating Sigil's "one way to do it" philosophy.

**Decision**: **Rejected.** Traits fully cover this use case.

---

#### 3.4 Mixins (via Class Expressions)
**What it is**: Compose classes by wrapping/extending dynamically.
```typescript
type Constructor<T = {}> = new (...args: any[]) => T;
function Timestamped<T extends Constructor>(Base: T) {
  return class extends Base { timestamp = Date.now(); };
}
```
**Sigil relevance**: Alternative to traits for horizontal code reuse.

**Discussion**: TypeScript mixins are a workaround for single class inheritance. Sigil's traits with multiple bounds (`T: A + B`) provide the same composition capability without the complexity of dynamically constructed classes.

**Decision**: **Rejected.** Traits with multiple bounds cover this use case.

---

#### 3.5 Auto-Accessors (`accessor` keyword)
**What it is**: Shorthand that creates getter/setter with private backing field.
```typescript
class Person {
  accessor name: string = "";  // creates get/set + #__name
}
```
**Sigil relevance**: Syntactic sugar for common pattern. Useful with decorators.

**Discussion**: This is class/OOP sugar that doesn't fit Sigil's struct + impl model. Sigil uses direct field access on structs, with methods for computed/validated access. Sigil's immutable-by-default approach means "setters" return new values anyway.

**Decision**: **Rejected.** Doesn't fit Sigil's data model.

---

#### 3.6 Private Fields (`#field`)
**What it is**: True runtime-private fields using `#` prefix (not just type-level).
```typescript
class Counter {
  #count = 0;
  increment() { this.#count++; }
}
```
**Sigil relevance**: Sigil has `::` for private. Question is runtime vs compile-time privacy.

**Discussion**: Runtime privacy matters in JavaScript because code runs in untrusted environments and dynamic property access (`obj[key]`) can bypass `private`. Sigil is compiled and doesn't have dynamic property access, so compile-time privacy is sufficient.

**Decision**: **Compile-time privacy is sufficient.** Sigil's `::` prefix provides module-level privacy enforced at compile time. No runtime enforcement needed.

---

### 4. Function Features

#### 4.1 Function Overloading
**What it is**: Multiple type signatures for same function implementation.
```typescript
function parse(x: string): number;
function parse(x: number): string;
function parse(x: string | number) { ... }
```
**Sigil relevance**: Enables different return types based on input types.

**Discussion**: Function overloading complicates type inference and hides what a function actually does. Sigil's explicitness philosophy favors: (1) separate functions with clear names (`parse_str`, `parse_int`), or (2) generics with associated types for related operations. Both are more explicit than overloading.

**Decision**: **Rejected.** Use separate named functions or generics instead.

---

#### 4.2 `this` Parameter Typing
**What it is**: Declare the type of `this` in function signatures.
```typescript
function onClick(this: HTMLElement, e: Event) {
  this.classList.add("clicked");
}
```
**Sigil relevance**: Useful for callback patterns, method binding.

**Discussion**: Sigil already has explicit `self` for methods and `Self` for the implementing type. There's no implicit `this` binding like JavaScript, so no need to type it. Callbacks use explicit closures that capture what they need.

**Decision**: **Rejected.** `self`/`Self` already cover this use case.

---

#### 4.3 Variadic Tuple Types
**What it is**: Spread operations at the type level for function arguments.
```typescript
type Concat<T extends any[], U extends any[]> = [...T, ...U];
function concat<T extends any[], U extends any[]>(a: T, b: U): [...T, ...U]
```
**Sigil relevance**: Enables typed variadic functions, tuple manipulation.

**Discussion**: Variadic tuples are powerful but niche, mainly useful for function wrappers and type-safe `apply`/`call`. Fixed-arity generics or lists cover most practical cases. Adds significant type system complexity for limited benefit.

**Decision**: **Rejected.** Use fixed-arity generics or lists.

---

### 5. Utility & Convenience Features

#### 5.1 `satisfies` Operator
**What it is**: Validate a value matches a type while keeping narrower inference.
```typescript
const colors = {
  red: "#ff0000",
  green: "#00ff00",
} satisfies Record<string, string>;
// colors.red is "#ff0000", not just string
```
**Sigil relevance**: Best of both worlds: validation + narrow types.

**Discussion**: `satisfies` requires literal types (strings/ints as types). While useful in TypeScript for retrofitting safety onto stringly-typed JavaScript, Sigil's sum types provide a cleaner solution. Sum types are properly distinct from their string representations, require explicit parsing at boundaries, and integrate with exhaustive matching. Example: `type HttpMethod = Get | Post | Put | Delete` is more explicit than `"GET" | "POST" | "PUT" | "DELETE"`.

**Decision**: **Rejected.** Literal types not needed - use sum types. `satisfies` becomes unnecessary without literal types. Use structs for known-field validation.

---

#### 5.2 `as const` Assertions
**What it is**: Make literal types immutable and preserve literal values.
```typescript
const routes = ["home", "about", "contact"] as const;
// type is readonly ["home", "about", "contact"], not string[]
```
**Sigil relevance**: Useful for enum-like patterns, configuration objects.

**Discussion**: Depends on literal types (which were rejected). Sigil is already immutable by default (`let` vs `let mut`), and sum types replace string literal enums. No need for a special assertion to preserve literals.

**Decision**: **Rejected.** Depends on literal types. Sigil's immutability and sum types cover these use cases.

---

#### 5.3 Utility Types (`Partial`, `Omit`, `Pick`, etc.)
**What it is**: Built-in type transformers.
```typescript
type Partial<T> = { [K in keyof T]?: T[K] };
type Pick<T, K extends keyof T> = { [P in K]: T[P] };
type Omit<T, K extends keyof any> = Pick<T, Exclude<keyof T, K>>;
type Required<T> = { [K in keyof T]-?: T[K] };
```
**Sigil relevance**: Standard library types. Question is which are useful for Sigil.

**Discussion**: Utility types depend on mapped types and `keyof` (both rejected). Without those foundations, they can't be expressed generically. Sigil's alternative is explicit type variants (`UserCreate`, `UserUpdate`, `UserPreview`). More verbose but each type's shape is visible at definition.

**Decision**: **Rejected.** Depends on mapped types. Use explicit type definitions instead.

---

#### 5.4 `keyof` and `typeof` Operators
**What it is**: Get keys of a type as union, or get type of a value.
```typescript
type Keys = keyof { a: 1; b: 2 };  // "a" | "b"
const x = { a: 1 };
type T = typeof x;  // { a: number }
```
**Sigil relevance**: Foundation for mapped types, dynamic key access.

**Discussion**: `keyof` produces literal union types (rejected). `typeof` infers type from value - Sigil uses explicit annotations. Dynamic property access `obj[key]` isn't idiomatic in Sigil; use direct field access for structs, maps for dynamic keys.

**Decision**: **Rejected.** Depends on literal types. Use explicit field access and maps.

---

#### 5.5 `using` Keyword (Explicit Resource Management)
**What it is**: Automatic cleanup of resources when scope exits.
```typescript
async function read() {
  using file = await openFile("data.txt");
  // file automatically closed when scope exits
}
```
**Sigil relevance**: Sigil has `with` pattern. Similar concept, different syntax.

**Discussion**: Both achieve RAII-style cleanup. Sigil's `with` pattern is more explicit - you see the acquire/use/release. TypeScript's `using` relies on a Symbol protocol. Sigil's approach fits its explicit philosophy better.

**Decision**: **Already implemented.** Sigil's `with` pattern covers this use case.

---

### 6. Enum Features

#### 6.1 Numeric Enums
**What it is**: Named constants that map to numbers.
```typescript
enum Direction { Up = 1, Down, Left, Right }
// Direction.Down === 2
```
**Sigil relevance**: Sigil uses sum types. Numeric enums are a simpler alternative.

**Discussion**: Sigil's sum types are more powerful and consistent. Numeric enums add a separate concept that overlaps with sum types. For cases needing numeric values, use a function: `@to_int (d: Direction) -> int = match(d, Up -> 1, Down -> 2, ...)`.

**Decision**: **Rejected.** Sum types cover this. Use match for numeric mapping.

---

#### 6.2 String Enums
**What it is**: Named constants that map to strings.
```typescript
enum Status { Active = "ACTIVE", Pending = "PENDING" }
```
**Sigil relevance**: More explicit, no reverse mapping issues.

**Discussion**: Same as numeric enums - sum types are the Sigil way. For string serialization, use a function: `@to_str (s: Status) -> str = match(s, Active -> "ACTIVE", Pending -> "PENDING")`.

**Decision**: **Rejected.** Sum types cover this. Use match for string mapping.

---

#### 6.3 `const enum`
**What it is**: Enums that are fully inlined at compile time.
```typescript
const enum Direction { Up, Down }
// Direction.Up becomes 0 in output, no runtime object
```
**Sigil relevance**: Performance optimization. No runtime overhead.

**Discussion**: `const enum` is a TypeScript compile-time optimization. Sigil's compiler can inline sum type variants as an optimization without special syntax. This is an implementation detail, not a language feature.

**Decision**: **Rejected.** Compiler optimization, not language feature. Sum type inlining is an implementation concern.

---

### 7. Module & Namespace Features

#### 7.1 Namespaces
**What it is**: Organize code into named scopes (older pattern).
```typescript
namespace Validation {
  export function validate(s: string) { ... }
}
```
**Sigil relevance**: Considered legacy in TS. ES modules preferred.

**Discussion**: Namespaces are a legacy TypeScript feature from before ES modules. Even TypeScript recommends ES modules now. Sigil uses file-based modules with explicit imports.

**Decision**: **Rejected.** Legacy feature. Sigil uses file-based modules.

---

#### 7.2 Triple-Slash Directives
**What it is**: Special comments for compiler configuration.
```typescript
/// <reference types="node" />
/// <reference path="./types.d.ts" />
```
**Sigil relevance**: Build/tooling configuration. May not be relevant.

**Discussion**: Triple-slash directives are TypeScript-specific build configuration for type references. Sigil's module system and build tooling don't need this pattern.

**Decision**: **Rejected.** TypeScript-specific build tooling. Not applicable to Sigil.

---

### 8. Decorator Features

#### 8.1 Class Decorators
**What it is**: Functions that modify or annotate classes.
```typescript
@sealed
class Greeter { ... }
```
**Sigil relevance**: Metaprogramming, annotations. Sigil has `#[attr]` syntax.

**Discussion**: TypeScript decorators are runtime functions that can modify behavior - this adds hidden magic that's hard to trace. Sigil's `#[attr]` attributes are compile-time metadata (derive, skip, deprecated) that inform the compiler/tooling without runtime behavior modification. This fits Sigil's explicit philosophy.

**Decision**: **Rejected.** Keep attributes compile-time only. No runtime decorators.

---

#### 8.2 Method/Property Decorators
**What it is**: Decorators on class members.
```typescript
class Example {
  @log
  method() { ... }
}
```
**Sigil relevance**: AOP patterns, validation, logging.

**Discussion**: Same reasoning as class decorators. Runtime method interception adds hidden behavior. For logging/validation, use explicit wrapper functions or the `with` pattern.

**Decision**: **Rejected.** Use explicit wrappers instead of hidden interception.

---

#### 8.3 Parameter Decorators
**What it is**: Decorators on function parameters.
```typescript
class Example {
  method(@required param: string) { ... }
}
```
**Sigil relevance**: Dependency injection, validation metadata.

**Discussion**: Parameter decorators are primarily used for dependency injection frameworks. Sigil's capability system (`uses Http`, `uses FileSystem`) provides explicit dependency declaration without hidden injection magic.

**Decision**: **Rejected.** Sigil's capability system handles dependency injection explicitly.

---

## Summary Table

| Feature | Category | Sigil Has Similar? | Priority |
|---------|----------|-------------------|----------|
| Discriminated Unions | Types | Yes (sum types) | **No change needed** |
| Conditional Types | Types | No | **Rejected** |
| Mapped Types | Types | No | **Rejected** |
| Template Literal Types | Types | No | **Rejected** |
| Branded Types | Types | **Yes (newtypes)** | **Already implemented** |
| Variance Annotations | Types | No | **Inferred (Rust-style)** |
| `infer` Keyword | Types | No | **Rejected** |
| User-Defined Type Guards | Narrowing | Has `match` | **Rejected** |
| Assertion Functions | Narrowing | Has `panic`/`?` | **Rejected** |
| `never` Exhaustiveness | Narrowing | Has `Never` | **Already implemented** |
| Declaration Merging | Objects | No | **Rejected** |
| Module Augmentation | Objects | No | **Rejected** |
| Abstract Classes | Objects | Has traits | **Rejected** |
| Mixins | Objects | Has traits | **Rejected** |
| Function Overloading | Functions | No | **Rejected** |
| Variadic Tuples | Functions | No | **Rejected** |
| `satisfies` | Utility | No | **Rejected** |
| `as const` | Utility | No | **Rejected** |
| Utility Types | Utility | Some | **Rejected** |
| `keyof`/`typeof` | Utility | No | **Rejected** |
| `using` (RAII) | Utility | Has `with` | **Already implemented** |
| Enums | Enums | Has sum types | **Rejected** |
| Decorators | Meta | Has attributes | **Rejected** |

---

## Discussion Log

### 2026-01-22: Complete Review

All 23 TypeScript features reviewed. Summary of outcomes:

**Already Implemented (4):**
- Discriminated Unions → Sigil sum types + exhaustive match
- Branded/Nominal Types → Sigil newtypes are nominal
- `never` Exhaustiveness → Sigil `Never` + exhaustive match
- `using` (RAII) → Sigil `with` pattern

**Adopted as Compiler Goal (2):**
- Variance Annotations → Inferred (Rust-style), no explicit syntax
- Control Flow Analysis → Compiler should track aliased conditions

**Rejected (17):**
- Conditional Types, Mapped Types, Template Literal Types, `infer` → Too complex, type-level programming not a goal
- Index Signatures → Keep clean struct/map separation
- Type Guards, Assertion Functions → Use `match` and `Result<T, E>`
- Declaration Merging, Module Augmentation → Violates explicitness
- Abstract Classes, Mixins → Traits cover this
- Auto-Accessors, Private Fields (runtime) → Compile-time privacy sufficient
- Function Overloading → Use separate named functions or generics
- `this` Parameter, Variadic Tuples → `self`/`Self` and fixed-arity cover this
- `satisfies`, `as const`, Utility Types, `keyof`/`typeof` → Depend on rejected literal types
- Enums (all 3) → Sum types are the Sigil way
- Namespaces, Triple-Slash → Legacy/TS-specific
- Decorators (all 3) → Keep attributes compile-time only

**Key Principle:** Sigil's explicitness philosophy guided most rejections. Features that add hidden behavior, scattered definitions, or type-level complexity were rejected in favor of explicit alternatives (sum types, traits, `Result`, `with` pattern).

---

## Sources

- [TypeScript Handbook](https://www.typescriptlang.org/docs/handbook/)
- [TypeScript 3.7 - Assertion Functions](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-7.html)
- [TypeScript 4.7 - Variance Annotations](https://www.typescript-training.com/course/intermediate-v2/11-covariance-contravariance/)
- [TypeScript 4.9 - satisfies Operator](https://www.totaltypescript.com/how-to-use-satisfies-operator)
- [Template Literal Types](https://www.typescriptlang.org/docs/handbook/2/template-literal-types.html)
- [Mapped Types](https://www.typescriptlang.org/docs/handbook/2/mapped-types.html)
- [Branded Types in TypeScript](https://nanamanu.com/posts/branded-types-typescript/)
