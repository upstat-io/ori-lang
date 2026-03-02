---
title: "Terms and definitions"
description: "Ori Language Specification — Clause 3: Terms and definitions"
order: 3
section: "Preliminary"
---

# 3 Terms and definitions

For the purposes of this document, the following terms and definitions apply.

ISO/IEC 2382 and IEEE 754 define terms used in this document that are not defined here.

## 3.1 program

complete set of modules forming a self-contained unit of compilation and execution (Clause 23)

NOTE  A conforming program has exactly one entry point.

## 3.2 module

unit of source code organization containing declarations, identified by a file path or package-qualified name (Clause 18)

## 3.3 package

collection of related modules distributed as a unit (Clause 18)

## 3.4 entry point

function designated as the starting point of program execution, declared with the `@main` attribute (Clause 23)

## 3.5 scope

region of program text within which a binding is visible (Clause 11)

## 3.6 block

delimited sequence of statements and an optional trailing expression that produces a value (Clause 11)

NOTE  The value of a block is determined by its last expression, if present. A block in which all statements are terminated by `;` has type `void`.

## 3.7 shadowing

introduction of a new binding with the same name as an existing binding in an enclosing scope, rendering the outer binding inaccessible within the inner scope (Clause 11)

## 3.8 type

classification determining the set of values and applicable operations for an entity (Clause 8)

## 3.9 primitive type

type built into the language whose representation is defined by the implementation (Clause 8)

NOTE  The primitive types are `int`, `float`, `bool`, `str`, `char`, `byte`, and `void`.

## 3.10 bottom type

type that has no values, denoted `Never`, which is a subtype of every other type (Clause 8)

NOTE  Expressions that never produce a value, such as `panic()`, `todo()`, and infinite loops without `break`, have the bottom type.

## 3.11 collection type

type representing an ordered or keyed group of elements (Clause 8)

NOTE  The collection types are list (`[T]`), fixed-capacity list (`[T, max N]`), map (`{K: V}`), and `Set<T>`.

## 3.12 generic type

type parameterized by one or more type parameters, enabling polymorphic definitions (Clause 8)

## 3.13 type parameter

placeholder for a type within a generic definition, resolved to a concrete type at each use site (Clause 8)

## 3.14 trait object

dynamically dispatched value whose concrete type is erased, retaining only the interface of a specified trait (Clause 9)

## 3.15 associated type

type declared within a trait that implementing types shall define concretely (Clause 10)

## 3.16 newtype

distinct type defined as a wrapper around an existing type, providing type safety with zero runtime cost (Clause 8)

NOTE  A newtype does not inherit traits or methods from its inner type.

## 3.17 existential type

opaque return type declared as `impl Trait`, hiding the concrete type from callers while guaranteeing static dispatch (Clause 8)

## 3.18 value

element of a type's value set, produced by evaluating an expression (Clause 14)

## 3.19 binding

association between a name and a value within a scope (Clause 13)

## 3.20 immutable binding

binding declared with the `$` sigil whose value cannot be reassigned after initialization (Clause 13)

## 3.21 mutable binding

binding declared with `let` (without `$`) whose value may be reassigned (Clause 13)

## 3.22 constant

module-level immutable binding declared with `let $name` whose value is determined at compile time (Clause 12)

## 3.23 literal

syntactic representation of a fixed value of a primitive or collection type (Clause 7)

## 3.24 constant expression

expression whose value can be fully determined at compile time without executing the program (Clause 24)

## 3.25 function

named callable entity with typed parameters and a return type, defined using the `@` sigil (Clause 10)

## 3.26 clause

one of multiple definitions of a function that are distinguished by their parameter patterns and evaluated top-to-bottom (Clause 10)

NOTE  Clauses enable pattern-based dispatch, similar to function clauses in Erlang or Elixir.

## 3.27 parameter

named, typed input to a function, declared in the function signature (Clause 10)

## 3.28 guard

boolean condition attached to a function clause or match arm that shall evaluate to `true` for the clause or arm to be selected (Clause 10)

## 3.29 variadic parameter

parameter declared with the `...` prefix, accepting zero or more arguments that are received as a list (Clause 10)

## 3.30 method

function defined within an `impl` block or trait that takes `self` as its first parameter (Clause 10)

## 3.31 associated function

function defined within an `impl` block that does not take `self`, called on the type rather than an instance (Clause 10)

## 3.32 default implementation

method body provided in a trait definition, used when an implementing type does not override it (Clause 10)

## 3.33 trait

named interface declaring a set of methods and associated types that types may implement (Clause 9)

## 3.34 implementation

block of method definitions that provides concrete behavior for a trait on a specific type, or inherent methods on a type (Clause 10)

## 3.35 trait bound

constraint requiring a type parameter to implement one or more specified traits (Clause 9)

## 3.36 coherence

property ensuring that for any given type and trait, at most one implementation exists in the program (Clause 9)

## 3.37 derived trait

trait whose implementation is automatically generated by the compiler based on the structure of the type (Clause 9)

NOTE  The `#derive` attribute requests derivation. Derivable traits include `Eq`, `Clone`, `Debug`, `Comparable`, and `Hashable`.

## 3.38 object safety

property of a trait that determines whether it can be used as a trait object for dynamic dispatch (Clause 9)

NOTE  A trait is object-safe if none of its methods return `Self`, take `Self` as a non-receiver parameter, or are generic.

## 3.39 pattern

syntactic construct used to test the structure of a value and optionally bind components to variables (Clause 15)

## 3.40 match arm

component of a `match` expression consisting of a pattern, an optional guard, and a body expression (Clause 15)

## 3.41 exhaustive match

`match` expression in which the set of arms covers all possible values of the scrutinee type (Clause 15)

## 3.42 scrutinee

expression whose value is tested against the patterns of a `match` expression (Clause 15)

## 3.43 automatic reference counting

memory management strategy in which each value maintains a count of references to it, and is deallocated when the count reaches zero (Clause 21)

## 3.44 reference count

non-negative integer associated with a heap-allocated value, tracking the number of live references to that value (Clause 21)

## 3.45 closure

function value that captures bindings from its enclosing scope by value (Clause 14)

## 3.46 value capture

mechanism by which a closure obtains copies of bindings from its enclosing scope at the point of closure creation (Clause 14)

NOTE  Ori captures all bindings by value. There is no capture by reference.

## 3.47 capability

named effect that a function declares it requires, enabling the caller to provide or mock the implementation (Clause 20)

## 3.48 handler

value provided in a `with...in` expression that satisfies a required capability for the duration of the enclosed expression (Clause 20)

## 3.49 suspend capability

the `Suspend` capability, which indicates that a function may suspend its execution to participate in cooperative concurrency (Clause 20)

## 3.50 test

function annotated with the `tests` attribute that verifies the behavior of one or more target functions (Clause 19)

## 3.51 attached test

test declared with `tests @target`, establishing a dependency relationship with the target function (Clause 19)

## 3.52 floating test

test declared with `tests _`, not attached to any specific function (Clause 19)

## 3.53 contract

precondition or postcondition declared on a function using `pre()` or `post()`, checked at run time (Clause 19)

## 3.54 panic

unrecoverable error condition that terminates execution of the current task (Clause 17)

NOTE  Panics are not exceptions and cannot be caught. They produce a diagnostic and abort.

## 3.55 recoverable error

error condition represented as a value of type `Result<T, E>`, propagable via the `?` operator (Clause 17)

## 3.56 error propagation

mechanism by which the `?` operator returns early from a function with an `Err` value when applied to a `Result` or `None` when applied to an `Option` (Clause 17)

## 3.57 task

unit of concurrent execution within a nursery, representing a suspendable computation (Clause 22)

## 3.58 suspending context

execution environment in which `Suspend` capability is available, permitting cooperative task switching (Clause 22)

## 3.59 suspension point

point in a task's execution at which it may yield control to the scheduler, occurring at operations that `use Suspend` (Clause 22)
