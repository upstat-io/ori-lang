# Glossary

Definitions of terms used in Sigil documentation.

---

## A

### ARC (Automatic Reference Counting)
Memory management strategy where the runtime tracks how many references exist to each value. When the count reaches zero, the value is deallocated. Sigil uses ARC for heap-allocated values.

### Associated Type
A type defined within a trait that implementors must specify. For example, `Iterator` has an associated type `Item` representing what the iterator yields.

### Async
Keyword indicating a function can suspend and resume. Async functions return immediately with a future that must be awaited.

---

## B

### Binding
A name associated with a value. In Sigil, bindings are immutable by default. Created with `let name = value` inside `run`/`try` blocks or via function parameters.

### Bound
A constraint on a generic type parameter. Written with `where T: Trait`, specifying what capabilities `T` must have.

---

## C

### Closure
A function that captures variables from its enclosing scope. In Sigil, closures capture by value (copy).

### Config Variable
A global constant defined with `$` prefix. Used for configuration values that should be easily changeable.

### Compositional Model
Sigil's type system design where types are exact (no subtyping) and behavior is shared through traits (no inheritance).

---

## D

### Derive
Automatic generation of trait implementations based on a type's structure. Written as `#[derive(Trait1, Trait2)]` before a type definition.

### Destructuring
Extracting values from compound types by pattern. For example, `{ x, y } = point` extracts fields from a struct.

### Dynamic Dispatch
Method resolution at runtime via vtable. Used with `dyn Trait` for polymorphism when the concrete type isn't known at compile time.

---

## E

### Exhaustiveness
Property of pattern matching where all possible cases must be handled. The compiler enforces exhaustive matches.

### Expression
Code that produces a value. In Sigil, almost everything is an expression, including `if/else` and `match`.

---

## F

### First-Class Function
A function that can be treated as a value—stored in variables, passed as arguments, returned from functions.

### Function Type
The type of a function, written as `(ParamTypes) -> ReturnType`. For example, `(int, int) -> int`.

---

## G

### Generic
A type or function parameterized by type variables. Written with angle brackets: `Option<T>`, `@identity<T>`.

### Guard
An additional condition in a pattern match arm, written with `if`. For example, `x if x > 0 -> "positive"`.

---

## I

### Impl Block
A block that provides trait implementations for a type. Written as `impl Trait for Type { ... }`.

### Inference
The compiler's ability to determine types without explicit annotations. Sigil infers types within functions but requires explicit signatures at function boundaries.

---

## L

### Lambda
An anonymous function, written as `x -> expression` or `(x, y) -> expression`.

### Let
Keyword for creating bindings in `run`/`try` blocks. `let x = value` creates an immutable binding; `let mut x = value` creates a mutable binding. All bindings in sequential blocks must use `let`.

### Let Mut
Mutable binding syntax. `let mut x = value` creates a binding that can be reassigned with `x = new_value`. Use sparingly—prefer immutable bindings when possible.

---

## M

### Match
Pattern matching construct that tests a value against patterns and executes the matching arm.

### Monomorphization
Compilation strategy where generic functions are specialized for each type they're used with. Enables static dispatch with no runtime overhead.

---

## N

### Never Type
The type of expressions that never return (like `panic()`). Can coerce to any type since the code never actually produces a value.

### Newtype
A distinct type wrapping another type. For example, `type UserId = str` creates a new type that's not interchangeable with `str`.

### Nominal Typing
Type system where types are distinguished by name, not structure. Two types with identical fields are still different types if they have different names.

---

## O

### Option
Built-in type representing an optional value. Either `Some(value)` or `None`.

### Orphan Rule
Restriction that trait implementations require either the trait or the type to be defined in the current module. Prevents conflicting implementations.

---

## P

### Panic
An unrecoverable error that terminates the current execution. Used for programmer errors, not expected failure conditions.

### Pattern
A declarative construct like `recurse`, `map`, `fold`, `try` that captures common computational patterns.

### Prelude
Items automatically imported into every module without explicit `use`. Includes `Option`, `Result`, `Some`, `None`, `Ok`, `Err`.

---

## R

### Result
Built-in type representing success or failure. Either `Ok(value)` or `Err(error)`.

### Refutable Pattern
A pattern that might not match, such as `Some(x)`. Must be used in `match`, not in regular bindings.

---

## S

### Semantic Addressing
System for referring to code elements by their logical path rather than file location. For example, `@module.function.property`.

### Shadowing
Defining a new binding with the same name as an existing one in the same scope. The new binding "shadows" the old one.

### SSO (Small String Optimization)
Optimization where short strings are stored inline rather than heap-allocated.

### Structural Sharing
Data structure technique where modified versions share unchanged portions with the original. Enables efficient immutable collections.

### Sum Type
A type that can be one of several variants. Also called "tagged union" or "enum". For example, `type Status = Pending | Running | Done`.

---

## T

### Test
A function that verifies behavior, declared with `@test_name tests @target_function`. Mandatory in Sigil—every function requires at least one test.

### Trait
A collection of method signatures that types can implement. Sigil's mechanism for polymorphism without inheritance.

### Trait Object
A value of type `dyn Trait`, enabling dynamic dispatch. Contains a pointer to data and a pointer to the vtable.

### Try Pattern
Error propagation pattern that evaluates expressions in sequence, returning early on any `Err`. Use the `?` operator after an expression to propagate errors: `let data = fetch()?,` returns early if `fetch()` returns `Err`.

### Type Narrowing
Compiler's ability to track more specific types after type-checking operations. For example, after `is_some(opt)`, the compiler knows `opt` is `Some`.

---

## V

### Variant
One possibility of a sum type. For example, `Some` and `None` are variants of `Option`.

### Vtable
Virtual method table used for dynamic dispatch. Contains pointers to the actual method implementations for a specific type.

---

## W

### Where Clause
Syntax for specifying trait bounds on generic parameters. Written as `where T: Trait1 + Trait2`.

### Wildcard
The `_` pattern that matches anything without binding a name. Must be last in a match.

---

## See Also

- [Main Index](00-index.md)
- [Type System](03-type-system/index.md)
- [Traits](04-traits/index.md)
