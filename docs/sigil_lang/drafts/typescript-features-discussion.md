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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 1.8 `infer` Keyword (Type Inference in Conditionals)
**What it is**: Extract/infer types within conditional type expressions.
```typescript
type ReturnType<T> = T extends (...args: any) => infer R ? R : never;
type UnwrapPromise<T> = T extends Promise<infer U> ? U : T;
type FirstArg<T> = T extends (x: infer A, ...args: any) => any ? A : never;
```
**Sigil relevance**: Powerful type-level pattern matching. Enables extracting types from complex structures.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 3.2 Module Augmentation
**What it is**: Extend types from other modules without modifying original source.
```typescript
declare module "express" {
  interface Request { user?: User }
}
```
**Sigil relevance**: Useful for adding types to third-party code. Extension mechanism.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 3.5 Auto-Accessors (`accessor` keyword)
**What it is**: Shorthand that creates getter/setter with private backing field.
```typescript
class Person {
  accessor name: string = "";  // creates get/set + #__name
}
```
**Sigil relevance**: Syntactic sugar for common pattern. Useful with decorators.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 4.2 `this` Parameter Typing
**What it is**: Declare the type of `this` in function signatures.
```typescript
function onClick(this: HTMLElement, e: Event) {
  this.classList.add("clicked");
}
```
**Sigil relevance**: Useful for callback patterns, method binding.

**Discussion**: _pending_
**Decision**: _pending_

---

#### 4.3 Variadic Tuple Types
**What it is**: Spread operations at the type level for function arguments.
```typescript
type Concat<T extends any[], U extends any[]> = [...T, ...U];
function concat<T extends any[], U extends any[]>(a: T, b: U): [...T, ...U]
```
**Sigil relevance**: Enables typed variadic functions, tuple manipulation.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 5.2 `as const` Assertions
**What it is**: Make literal types immutable and preserve literal values.
```typescript
const routes = ["home", "about", "contact"] as const;
// type is readonly ["home", "about", "contact"], not string[]
```
**Sigil relevance**: Useful for enum-like patterns, configuration objects.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 5.4 `keyof` and `typeof` Operators
**What it is**: Get keys of a type as union, or get type of a value.
```typescript
type Keys = keyof { a: 1; b: 2 };  // "a" | "b"
const x = { a: 1 };
type T = typeof x;  // { a: number }
```
**Sigil relevance**: Foundation for mapped types, dynamic key access.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

### 6. Enum Features

#### 6.1 Numeric Enums
**What it is**: Named constants that map to numbers.
```typescript
enum Direction { Up = 1, Down, Left, Right }
// Direction.Down === 2
```
**Sigil relevance**: Sigil uses sum types. Numeric enums are a simpler alternative.

**Discussion**: _pending_
**Decision**: _pending_

---

#### 6.2 String Enums
**What it is**: Named constants that map to strings.
```typescript
enum Status { Active = "ACTIVE", Pending = "PENDING" }
```
**Sigil relevance**: More explicit, no reverse mapping issues.

**Discussion**: _pending_
**Decision**: _pending_

---

#### 6.3 `const enum`
**What it is**: Enums that are fully inlined at compile time.
```typescript
const enum Direction { Up, Down }
// Direction.Up becomes 0 in output, no runtime object
```
**Sigil relevance**: Performance optimization. No runtime overhead.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 7.2 Triple-Slash Directives
**What it is**: Special comments for compiler configuration.
```typescript
/// <reference types="node" />
/// <reference path="./types.d.ts" />
```
**Sigil relevance**: Build/tooling configuration. May not be relevant.

**Discussion**: _pending_
**Decision**: _pending_

---

### 8. Decorator Features

#### 8.1 Class Decorators
**What it is**: Functions that modify or annotate classes.
```typescript
@sealed
class Greeter { ... }
```
**Sigil relevance**: Metaprogramming, annotations. Sigil has `#[attr]` syntax.

**Discussion**: _pending_
**Decision**: _pending_

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

**Discussion**: _pending_
**Decision**: _pending_

---

#### 8.3 Parameter Decorators
**What it is**: Decorators on function parameters.
```typescript
class Example {
  method(@required param: string) { ... }
}
```
**Sigil relevance**: Dependency injection, validation metadata.

**Discussion**: _pending_
**Decision**: _pending_

---

## Summary Table

| Feature | Category | Sigil Has Similar? | Priority |
|---------|----------|-------------------|----------|
| Discriminated Unions | Types | Yes (sum types) | **No change needed** |
| Conditional Types | Types | No | **Rejected** |
| Mapped Types | Types | No | **Rejected** |
| Template Literal Types | Types | No | **Rejected** |
| Branded Types | Types | **Yes (newtypes)** | **Already implemented** |
| Variance Annotations | Types | No | _pending_ |
| `infer` Keyword | Types | No | _pending_ |
| User-Defined Type Guards | Narrowing | Partial | _pending_ |
| Assertion Functions | Narrowing | No | _pending_ |
| `never` Exhaustiveness | Narrowing | Has `Never` | _pending_ |
| Declaration Merging | Objects | No | _pending_ |
| Module Augmentation | Objects | No | _pending_ |
| Abstract Classes | Objects | Has traits | _pending_ |
| Mixins | Objects | Has traits | _pending_ |
| Function Overloading | Functions | No | _pending_ |
| Variadic Tuples | Functions | No | _pending_ |
| `satisfies` | Utility | No | _pending_ |
| `as const` | Utility | No | _pending_ |
| Utility Types | Utility | Some | _pending_ |
| `keyof`/`typeof` | Utility | No | _pending_ |
| `using` (RAII) | Utility | Has `with` | _pending_ |
| Enums | Enums | Has sum types | _pending_ |
| Decorators | Meta | Has attributes | _pending_ |

---

## Discussion Log

_Each feature discussion will be logged here with date and outcome._

---

## Sources

- [TypeScript Handbook](https://www.typescriptlang.org/docs/handbook/)
- [TypeScript 3.7 - Assertion Functions](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-7.html)
- [TypeScript 4.7 - Variance Annotations](https://www.typescript-training.com/course/intermediate-v2/11-covariance-contravariance/)
- [TypeScript 4.9 - satisfies Operator](https://www.totaltypescript.com/how-to-use-satisfies-operator)
- [Template Literal Types](https://www.typescriptlang.org/docs/handbook/2/template-literal-types.html)
- [Mapped Types](https://www.typescriptlang.org/docs/handbook/2/mapped-types.html)
- [Branded Types in TypeScript](https://nanamanu.com/posts/branded-types-typescript/)
