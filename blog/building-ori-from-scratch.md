---
title: "I'm Building a Programming Language From Scratch. Here's What That's Actually Like."
date: 2026-03-01
description: "745 commits. 360,000 lines of Rust. 17 compiler crates. One person. The story of where Ori is, why it exists, and what building a language from scratch actually looks like."
---

**745 commits. 360,000 lines of Rust. 17 compiler crates. One person.**

I started writing Ori on January 20th, 2026. Five weeks later, I have a working lexer, parser, type checker with full Hindley-Milner inference, a tree-walking interpreter, a Salsa-based incremental compilation pipeline, and an LLVM backend that can compile native binaries. I've written 241 spec conformance tests in the language itself. The language has iterators, pattern matching, algebraic data types, derived traits, for-yield comprehensions, and an ARC-based memory model.

And honestly? Most days it still feels like I'm building the foundation.

This is the story of where Ori is, why it exists, and what building a language from scratch actually looks like when you're not a PL researcher at a university. You're just a developer who got frustrated enough to do something about it.

---

## Why Another Language?

I'll be direct: I think we're designing languages for the wrong author.

Every mainstream language was designed for humans typing code. The syntax is optimized for brevity, the error messages assume a human is reading them, the tooling assumes a person navigating files. That made sense for 50 years.

But the center of gravity is shifting. AI is writing more and more production code. And AI has fundamentally different failure modes than humans:

- AI doesn't make typos. It makes **plausible but subtly wrong** implementations.
- AI doesn't forget semicolons. It forgets **edge cases**.
- AI doesn't struggle with syntax. It struggles with **correctness guarantees**.

So I asked: what if you designed a language where the primary author is AI, but the primary *reader* is still human? What trade-offs change?

Almost everything, it turns out.

---

## What Ori Looks Like

Before I get into the philosophy, here's actual Ori code that runs today:

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

A few things stand out if you're used to Rust/Go/TypeScript:

- **`@` prefix** marks top-level declarations: functions, tests, main
- **Expression-based.** No `return` keyword. The last expression is the value.
- **`tests @sum_tree`** declares which function the test targets. This isn't a comment. The compiler tracks the dependency.
- **Named arguments** everywhere: `assert_eq(actual: ..., expected: ...)` not `assert_eq(..., ...)`
- **`#derive(Eq, Clone, Printable)`** for trait derivation, similar to Rust

Iterators feel natural:

```ori
@evens_doubled () -> [int] =
    (0..10).iter()
        .filter(predicate: x -> x % 2 == 0)
        .map(transform: x -> x * 2)
        .collect()
// [0, 4, 8, 12, 16]
```

For loops desugar to iterators under the hood:

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

Ori makes three bets that most languages don't:

### 1. Tests Are Smart

The compiler understands your tests. It tracks which tests target which functions. If you change `sum_tree`, the compiler knows that `test_sum_tree` needs to run. This is built into the dependency graph, not bolted on as a CI step.

Test enforcement is configurable: `off` (default), `warn` (flags untested functions), or `error` (untested functions fail compilation). Even at the default level, the infrastructure is always there -- dependency tracking, automatic test discovery, and incremental execution.

For AI-generated code, this is the difference between "trust me, it works" and "here's proof it works." Turn enforcement to `error` and the compiler won't produce a binary until every function has a test.

### 2. No Garbage Collector, No Borrow Checker

Ori uses Automatic Reference Counting. This is what Swift does, what Python does under the hood, what Objective-C did before ARC was automatic.

The borrow checker is incredible engineering, but it's also the #1 reason developers bounce off Rust. Lifetime annotations, borrowing rules, fighting the compiler. It's a steep hill. For AI-authored code, the borrow checker is even worse: AI generates code that *looks* correct but violates ownership rules in ways that are hard to diagnose from error messages alone.

ARC is simpler. Objects are freed when their last reference dies. It's deterministic (unlike GC), predictable, and the mental model fits in one sentence. The trade-off is reference cycles, which Ori prevents at compile time.

Right now I'm deep in the weeds of making ARC *fast*. More on that below.

### 3. Effects Are Explicit

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

## What I'm Actually Working On Right Now

Here's where it gets real. The romantic version of building a language is "I'm designing beautiful syntax." The reality is: **I'm debugging double-frees in Copy-on-Write list operations.**

### The Value Semantics Problem

Ori has value semantics. When you write `let b = a`, you conceptually get an independent copy. This is great for reasoning about code (no spooky action at a distance), but naive value semantics means copying everything all the time, which is terrible for performance.

The solution is Copy-on-Write (COW): when you "copy" a list, you actually just bump a reference count. Both `a` and `b` point to the same memory. Only when one of them tries to *mutate* (like `push`), do you check: is the reference count 1? If yes, mutate in place. No one else is looking. If not, copy first, then mutate.

This sounds simple. It is not.

I have a custom runtime library (`ori_rt`) written in Rust that implements COW-aware operations for lists, maps, sets, and strings. I just spent the last 48 hours:

1. **Implementing Small String Optimization.** Strings under 24 bytes are stored inline, no heap allocation. This is the kind of optimization that makes real programs dramatically faster because most strings are short.

2. **Redesigning OriMap to a single-buffer layout.** Hash maps traditionally have separate allocations for entries and metadata. I'm packing everything into one contiguous buffer so COW only needs to copy one thing.

3. **Fixing a double-free in list COW.** When you concatenate two lists (`a + b`), and both are shared (refcount > 1), the COW path was freeing the original buffer and *then* trying to read from it. Classic use-after-free, except it manifested as a double-free because the freed memory got recycled. This one took hours of staring at `ORI_TRACE_RC=1` output.

4. **Adding element RC callbacks.** When a list is COW-copied, each element inside it needs its reference count incremented. This requires a callback function pointer that knows the element type. I had to thread this through all 8 list mutation primitives.

This is the stuff that makes or breaks a language runtime. Nobody sees it. It's not in any tutorial. But if `list.push(x)` is 10x slower than it should be, nobody will use your language.

### The LLVM Backend

I have a working LLVM backend. It can compile Ori programs to native binaries. It handles generic monomorphization, closures, sum types, pattern matching, the works.

It also has 28 known bugs.

I know this because I did something I call "code journeys." I took 12 representative Ori programs, ran them through both the interpreter and the LLVM backend, and compared outputs. The interpreter is correct for all 12. The LLVM backend gets 9 of 12 right. The other 3 have issues ranging from "closures mixed with sum types crash" to "equality comparison on enum payloads silently returns the wrong answer."

That last one is the scary kind. No crash, no error message, just the wrong answer. Silent miscompilation. I have a detailed plan to fix all 28 issues, organized by severity. The 4 critical ones (crashes and silent wrong results) are first.

### The Diagnostic Toolkit

One thing I'm proud of: I built an extensive diagnostic toolkit for debugging the compiler itself. When you're writing a compiler, you need tools to debug your tools. I have:

- **`diagnose-aot.sh`**: compiles a program, runs it, checks for leaks, dumps RC stats, generates LLVM IR, optionally runs Valgrind. All in one command.
- **`dual-exec-debug.sh`**: runs a program through both interpreter and AOT, compares outputs, auto-dumps everything on mismatch
- **`codegen-audit.sh`**: static analysis of generated LLVM IR. Checks RC balance, COW correctness, ABI conformance.
- **Phase dumps**: `ORI_DUMP_AFTER_PARSE=1`, `ORI_DUMP_AFTER_TYPECK=1`, `ORI_DUMP_AFTER_ARC=1`, `ORI_DUMP_AFTER_LLVM=1`. See the IR at every stage.

Building these tools felt like "yak shaving" at the time. They've saved me hundreds of hours since.

---

## The Architecture

The compiler is 17 Rust crates:

| Crate | What It Does |
|-------|-------------|
| `ori_lexer_core` | Raw scanner (~1 GiB/s throughput) |
| `ori_lexer` | Full tokenizer with keyword/literal handling |
| `ori_parse` | Recursive descent parser |
| `ori_ir` | Intermediate representation |
| `ori_types` | Type checker with HM inference |
| `ori_eval` | Tree-walking interpreter |
| `ori_arc` | ARC insertion via Perceus algorithm |
| `ori_llvm` | LLVM IR generation |
| `ori_rt` | Native runtime (COW collections, SSO strings) |
| `ori_fmt` | Code formatter |
| `ori_lsp` | Language server protocol |
| `oric` | CLI + Salsa incremental compilation |
| ... | + 5 support crates |

The crate dependency graph is strictly layered. No upward dependencies. IO only happens in the CLI crate. This isn't academic purity. It's survival. When you're moving fast and touching everything, strict boundaries are the only thing keeping the codebase from collapsing into a ball of circular dependencies.

I use [Salsa](https://github.com/salsa-rs/salsa) for incremental compilation, the same framework `rust-analyzer` uses. It means re-checking a file after a small edit only recomputes what changed. This matters a lot for the LSP (language server) experience.

---

## The Honest Struggles

Building a language alone is hard. Here's what's actually hard about it:

### Everything is connected

Change the parser? The type checker might break. Fix a type inference bug? The evaluator's assumptions might be wrong. Add a new runtime function? It needs to be registered in the type checker, the evaluator, AND the LLVM backend. I have checklists for adding an iterator method that are 11 steps across 6 files.

The upside: you understand the whole system. The downside: there is no "someone else's problem."

### You're writing tests for your test infrastructure

My test runner is part of the compiler. So when the test runner has a bug... how do you test that? I've written Rust-level tests that test the Ori-level test runner that runs Ori-level tests. It's turtles all the way down.

### Decisions compound

Every syntax choice I make now will live for years. Should pattern matching use `->` or `=>`? Is it `match x { ... }` or `when x is { ... }`? These feel trivial in isolation, but each one creates a precedent that every future feature has to respect. I've rewritten the grammar EBNF more times than I can count.

### LLVM is powerful and merciless

LLVM gives you incredible optimization for free, but it assumes you're generating correct IR. Generate incorrect IR and you get no error message. Your program just does something wrong, or segfaults, or (worst case) works fine on your machine and crashes on someone else's. I've spent more time debugging my LLVM IR generation than any other part of the compiler.

### The "last 20%" is 80% of the work

Getting a basic program to compile and run: a few weeks. Getting *every program* to compile and run correctly, with proper error messages, edge case handling, and reasonable performance: months. The distance between "works on happy path" and "actually reliable" is enormous.

---

## What's Next

The roadmap has 23 sections across 8 tiers. Here's what's immediately ahead:

1. **Finish Value Semantics Optimization.** Complete COW for maps and sets, implement zero-copy slices, add static uniqueness analysis so the compiler can skip COW checks when it can prove a value is unique.

2. **Fix all 28 LLVM codegen issues.** Get AOT correctness from 75% to 100% on code journeys.

3. **Type Strategy Registry.** Centralize all built-in type behavior (methods, operators, memory strategy) into a single data-driven registry. Right now this knowledge is scattered across ~1,900 lines of parallel allowlists. It's a maintainability nightmare.

4. **Representation Optimization.** Narrow integers (`int` -> `i32` when range allows), enum niche filling (like Rust's `Option<bool>` = 1 byte), escape analysis for stack promotion, ARC header compression.

5. **Capabilities system.** The `uses` clause for effect tracking. The type checker infrastructure exists but the full system isn't wired up yet.

6. **Standard library.** The modules exist (`std.io`, `std.json`, `std.net`, `std.crypto`, etc.) but most are stubs waiting for FFI to be complete.

---

## The Meta-Game: Building With AI (But Not Vibe Coding)

Here's the meta-irony: I'm building a language designed for AI-authored code, and I'm using AI to help build it.

I need to be clear about something: **this is not vibe coding.** I'm not prompting "build me a compiler" and hoping for the best. I've been writing software for 30 years. I've built production systems, led teams, debugged things at 3 AM. I know what good code looks like and I know what a house of cards looks like.

What I'm doing with Claude Code is applying the same rigorous practices I'd use with a team of senior engineers (TDD, code review, architectural planning, spec-driven development, reference implementation study) except my team happens to be an AI. The `CLAUDE.md` file in my repo is 200+ lines of engineering standards: no workarounds, no hacks, no shortcuts, fact-check everything, consult reference repos before deciding. Every bug gets tests *first*, then a fix. Every new feature gets a spec section, a plan, and conformance tests. The AI follows the same rules a human contributor would, or the code doesn't ship.

And that's the real thesis I'm trying to prove: **you can build deeply complex, production-quality software with AI tools, if you refuse to lower your standards.** The 52% test coverage ratio, the 16-section formal spec, the 12 code journeys, the diagnostic toolkit: that's not AI output. That's engineering discipline applied to AI output. The tool changed. The standards didn't.

Claude writes significant chunks of the compiler. The entire workflow is AI-accelerated: exploring reference compiler codebases (I have 10 cloned: Rust, Go, Zig, TypeScript, Gleam, Elm, Roc, Swift, Koka, Lean4), drafting implementation plans, writing code, writing tests.

But here's what I've learned: AI is great at implementing well-specified behavior and terrible at architectural decisions. It can write a perfect `eval_iter_next()` if you tell it exactly what the function should do. It cannot decide whether iterators should be a trait, an enum, or a protocol. Those decisions still need a human who understands the trade-offs, and 30 years of pattern recognition for what's going to bite you six months from now.

Ori is, in a sense, my answer to that observation. Make the language so explicit, so structured, so testable, that AI can work in it effectively, while keeping the architectural decisions in human hands.

---

## Numbers

Since I know devs love metrics:

- **745 commits** in 5 weeks
- **359,000 lines** of Rust across 17 compiler crates
- **187,000 lines** of tests (52% of the codebase)
- **241 spec conformance tests** written in Ori
- **16-section language spec** with formal grammar (EBNF)
- **Lexer throughput**: ~1 GiB/s (raw scanner)
- **10 reference compilers** studied for design decisions
- **12 code journeys** for systematic LLVM debugging
- **21 diagnostic scripts** for compiler debugging

---

## Try It? Not Yet.

Ori isn't ready for users. The compiler panics on programs it should reject gracefully. The error messages are sometimes wrong. The LLVM backend silently miscompiles 25% of non-trivial programs. The standard library is mostly stubs.

But it's getting there. Every day, more tests pass. Every day, the error messages get a little better. Every day, the distance between "works" and "works correctly" shrinks.

If you're interested in following along, check out the [roadmap](/roadmap/) or browse the [language spec](/docs/spec/index). If you're interested in language design, compiler architecture, or just watching someone build something ambitious and probably slightly insane, stick around.

Building a language is the hardest thing I've ever done in software. It's also the most fun.

---

*I'm Eric, and I'm building Ori, a statically-typed, expression-based language designed for the AI era. This is the first in what will hopefully be a series of posts about the journey.*
