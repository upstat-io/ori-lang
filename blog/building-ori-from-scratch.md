---
title: "Building a Programming Language From Scratch"
date: 2026-03-01
description: "Where Ori is, why it exists, and what building a language alone actually looks like."
---

I started writing Ori on January 20th. Five weeks later, I have a working compiler with type inference, an interpreter, an LLVM backend, and a growing spec test suite. The language has iterators, pattern matching, algebraic data types, and ARC-based memory management.

Most days it still feels like I'm building the foundation.

---

## Why Another Language?

I think we're designing languages for the wrong author.

Every mainstream language was designed for humans typing code. The syntax is optimized for brevity, the error messages assume a human is reading them. That made sense for 50 years. But AI is writing more and more production code, and AI has fundamentally different failure modes. It doesn't make typos — it makes plausible but subtly wrong implementations. It doesn't forget semicolons — it forgets edge cases.

So I asked: what if you designed a language where the primary author is AI, but the primary *reader* is still human? What trade-offs change?

Almost everything, it turns out.

---

## What Ori Looks Like

Here's actual Ori code that runs today:

```ori
use std.testing { assert_eq }

#derive(Eq, Clone, Printable)
type Tree = Leaf(value: int) | Node(left: Tree, right: Tree);

@sum_tree (t: Tree) -> int = match t {
    Leaf(v) -> v,
    Node(l, r) -> sum_tree(l) + sum_tree(r)
}

@test_sum_tree tests @sum_tree () -> void = {
    let tree = Node(
        left: Leaf(value: 1),
        right: Node(left: Leaf(value: 2), right: Leaf(value: 3))
    );
    assert_eq(actual: sum_tree(tree), expected: 6)
}
```

A few things if you're used to Rust or TypeScript:

- **`@` prefix** marks top-level declarations — functions, tests, main
- **Expression-based.** No `return` keyword. The last expression is the value.
- **`tests @sum_tree`** tells the compiler which function this test targets. It tracks the dependency.
- **Named arguments** everywhere: `assert_eq(actual: ..., expected: ...)`

Iterators:

```ori
@evens_doubled () -> [int] =
    (0..10).iter()
        .filter(predicate: x -> x % 2 == 0)
        .map(transform: x -> x * 2)
        .collect()
// [0, 4, 8, 12, 16]
```

For loops desugar to iterators:

```ori
@squares () -> [int] = for x in 1..6 yield x * x;
// [1, 4, 9, 16, 25]
```

Pattern matching with sum types:

```ori
type Status = Pending | Running(progress: int) | Done;

@describe (s: Status) -> str = match s {
    Pending -> "waiting",
    Running(p) -> str(p) + "%",
    Done -> "complete"
}
```

---

## The Three Bets

### Tests Are Smart

The compiler understands your tests. It tracks which tests target which functions. Change `sum_tree`, and it knows `test_sum_tree` needs to run. This isn't bolted on as a CI step — it's in the dependency graph.

Test enforcement is configurable: `off` (default), `warn`, or `error`. At `error`, the compiler won't produce a binary until every function has a test. For AI-generated code, this is the difference between "trust me" and "here's proof."

### No Garbage Collector, No Borrow Checker

Ori uses Automatic Reference Counting. The borrow checker is incredible engineering, but it's also the number one reason developers bounce off Rust. For AI-authored code it's even worse — AI generates code that *looks* correct but violates ownership rules in ways that are hard to diagnose.

ARC is simpler. Objects are freed when their last reference dies. Deterministic, predictable, and the mental model fits in one sentence. The trade-off is reference cycles, which Ori prevents at compile time.

### Effects Are Explicit

Functions that do I/O declare it:

```ori
@fetch (url: str) -> Result<str, Error> uses Http = ...
```

That `uses Http` isn't decoration. In tests, you can replace it:

```ori
@test_fetch tests @fetch () -> void =
    with Http = MockHttp { ... } in
    assert_eq(actual: fetch("..."), expected: Ok("..."))
```

No dependency injection framework. No mocking library. It's a language feature.

---

## What I'm Actually Working On

The romantic version of building a language is "designing beautiful syntax." The reality is I'm debugging double-frees in Copy-on-Write list operations.

### The Value Semantics Problem

Ori has value semantics. `let b = a` gives you an independent copy. Great for reasoning, terrible for performance if you actually copy everything.

The solution is Copy-on-Write: when you "copy" a list, you bump a reference count. Both `a` and `b` point to the same memory. Only when one tries to *mutate* do you check: is the reference count 1? If yes, mutate in place. If not, copy first, then mutate.

This sounds simple. It is not.

I have a custom runtime library in Rust that implements COW for lists, maps, sets, and strings. Recent work includes Small String Optimization (strings under 24 bytes stored inline, no heap allocation), redesigning the hash map to a single-buffer layout so COW only copies one thing, and fixing a double-free where concatenating two shared lists freed a buffer and then tried to read from it. That last one took hours of staring at `ORI_TRACE_RC=1` output.

This is the stuff that makes or breaks a language runtime. Nobody sees it. But if `list.push(x)` is 10x slower than it should be, nobody will use your language.

### The LLVM Backend

The LLVM backend can compile Ori to native binaries — generic monomorphization, closures, sum types, pattern matching, the works.

It also has known bugs. I found them by running representative programs through both the interpreter and the LLVM backend and comparing outputs. The interpreter gets them all right. The LLVM backend doesn't, yet. Issues range from "closures mixed with sum types crash" to "equality comparison on enum payloads silently returns the wrong answer." That last one is the scary kind — no crash, no error, just the wrong answer.

### The Diagnostic Toolkit

One thing I'm genuinely proud of: the diagnostic toolkit for debugging the compiler itself. `diagnose-aot.sh` compiles a program, runs it, checks for leaks, dumps RC stats, and generates LLVM IR in one command. `dual-exec-debug.sh` runs a program through both the interpreter and AOT, compares outputs, and auto-dumps everything on mismatch. Phase dumps let you see the IR at every stage of compilation.

Building these tools felt like yak shaving at the time. They've saved me hundreds of hours since.

---

## The Honest Struggles

### Everything is connected

Change the parser and the type checker might break. Fix a type inference bug and the evaluator's assumptions might be wrong. Add a new runtime function and it needs to be registered in the type checker, the evaluator, AND the LLVM backend. I have checklists for adding an iterator method that are 11 steps across 6 files.

The upside: you understand the whole system. The downside: there is no "someone else's problem."

### You're writing tests for your test infrastructure

My test runner is part of the compiler. When the test runner has a bug, how do you test that? I've written Rust-level tests that test the Ori-level test runner that runs Ori-level tests. Turtles all the way down.

### LLVM is powerful and merciless

LLVM gives you incredible optimization for free, but it assumes you're generating correct IR. Generate bad IR and you get no error message — your program just does something wrong, or segfaults, or works fine on your machine and crashes on someone else's. I've spent more time debugging LLVM IR generation than any other part of the compiler.

### The last 20% is 80% of the work

Getting a basic program to compile: a few weeks. Getting *every* program to compile correctly, with proper error messages and edge case handling: months. The distance between "works on happy path" and "actually reliable" is enormous.

---

## What's Next

Immediately: finish COW for maps and sets, implement zero-copy slices, add static uniqueness analysis so the compiler can skip COW checks when it proves a value is unique. Fix the remaining LLVM codegen bugs. Centralize all built-in type behavior into a single registry instead of the scattered allowlists I have now. Start wiring up the capabilities system for real.

Further out: representation optimization (narrowing integers, enum niche filling, escape analysis), and filling in the standard library once FFI is complete.

---

## Try It? Not Yet.

Ori isn't ready for users. The compiler panics on programs it should reject gracefully. The LLVM backend still miscompiles some non-trivial programs. The standard library is mostly stubs.

But every day, more tests pass. Every day, the error messages get better. Every day, the distance between "works" and "works correctly" shrinks.

If you're interested in following along, check out the [roadmap](/roadmap/) or browse the [language spec](/docs/spec/index).

Building a language is the hardest thing I've ever done in software. It's also the most fun.

---

*I'm Eric. I'm building Ori — a statically-typed, expression-based language designed for the AI era. More posts to come.*
