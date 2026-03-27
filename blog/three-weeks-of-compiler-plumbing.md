---
title: "Three Weeks of Compiler Plumbing"
date: 2026-03-26
description: "AIMS went from a design to a working pipeline. Also: a new crate for representation optimization, and the kind of debugging that makes a compiler real."
---

The last blog was about cross-platform CI breaking in spectacular ways. Since then, about 300 commits have gone in. The headline is AIMS — the memory system went from "we have a plan" to "it works end-to-end." But there's also a new compiler crate, conditional compilation, and the usual parade of bugs that only surface when you test on real programs.

---

## AIMS: The Memory System That Defines Ori

AIMS — ARC Intelligent Memory System — is the reason Ori can promise "no garbage collector, no borrow checker" without caveats. It's not one optimization. It's a pipeline where each stage feeds the next, and the whole thing has to agree or nothing works.

The idea sounds simple: Ori has value semantics, so every variable conceptually owns its data. Under the hood, we use reference counting to avoid copying everything all the time. AIMS is the system that makes that fast — it figures out where to insert RC operations, where to skip them, where values can be mutated in place instead of copied, and where the compiler can prove a value is unique and skip the runtime check entirely.

Before these three weeks, the individual pieces existed but they weren't connected. The ARC insertion pass worked. COW worked for simple cases. The LLVM backend generated correct-ish IR. But there was no unified pipeline, no invariant checking between stages, and no way to verify that the whole system was coherent.

Now there is.

### What the pipeline actually does

The ARC pipeline has roughly eight layers. The first classifies every type — is it trivial (just integers and bools, no heap allocation) or does it need RC? Trivial types skip the entire pipeline. Then Perceus-style insertion places RC operations at last-use points. Borrow inference figures out which function parameters don't need RC at call sites. Reset/reuse converts drop-then-allocate sequences into in-place reuse. Static uniqueness analysis eliminates runtime COW checks when the compiler can prove a value has exactly one owner.

Each layer depends on the ones before it. If type classification is wrong, borrow inference makes bad decisions. If borrow inference is wrong, the LLVM backend generates unnecessary RC calls. If COW analysis is wrong, you get double-frees or memory leaks. The pipeline has to be *coherent* — every stage has to agree on the same facts about every value.

Getting that coherence is what most of the last three weeks was about.

### The bugs that made it real

The worst was in COW for slices. A slice is a view into a list — it points into the middle of someone else's allocation. When a COW mutation triggers on a slice, the runtime needs to find the original allocation's metadata (how many elements, how to clean them up). That metadata lives in a header before the start of the *original* buffer, not the slice's pointer.

The code was reading the header relative to the slice pointer. Which means it was reading whatever happened to be at that memory address — usually part of the data from a previous element. On small programs this happened to be zero, which is a valid "no cleanup needed" value. On real programs, it was garbage.

Two days of `ORI_TRACE_RC=1` output to find. Fifteen lines to fix.

I also had to implement V5 of the RC header — adding an element count field so slice cleanup knows how many elements to process. Before this, it used the buffer capacity, which is wrong for slices that are shorter than the backing allocation. That's a quiet leak — you decrement RC on elements that aren't yours.

The fat pointer ABI was another multi-day saga. Ori's collection iterators use fat pointers — a data pointer plus metadata. The ABI for passing these across function boundaries had to change when we unified how `elem_dec_fn` (the function pointer that knows how to clean up elements) propagates through the system. Getting the runtime, the codegen, and the ARC pipeline to agree on the same fat pointer layout took about a week.

### The scorecard

I run the full test suite through what I call "code journeys" — representative Ori programs that exercise every compiler phase. When AIMS started, the journeys were scoring around 6-7 out of 10. Mostly correct, with RC leaks or unnecessary operations.

After the pipeline work, all 13 journeys score 10.0. No leaks, no unnecessary RC, no missed optimizations. That doesn't mean AIMS is done — there are more optimization layers to add — but the foundation is correct and verified.

---

## A New Crate: Representation Optimization

The other big structural change is `ori_repr` — a new compiler crate for figuring out how to lay out types in memory.

Right now, every `int` in Ori is 64 bits. Every `bool` is 64 bits. Every enum discriminant is a full machine word. This works but it's wasteful. A `bool` should be 1 byte. An enum with 3 variants needs a single byte tag. An `int` that only holds values 0-255 could be a `u8`.

`ori_repr` has three analysis passes so far. Canonical layout handles struct field ordering, enum discriminant sizing, zero-sized type elimination, and `#repr` attributes for FFI. Triviality analysis determines which types can skip ARC entirely — an `int` doesn't need reference counting, and that chains transitively through structs. Range analysis tracks possible integer values through the program using abstract interpretation, so the compiler can narrow storage.

Range analysis has been the most bug-prone. It handles conditionals, loops, function calls across modules, and feedback through recursive call graphs. Every edge case I think I've covered reveals another — overflow in negation, division by a range containing zero, bit-shift by a negative count. Thirty-five fix rounds and counting.

---

## Everything Else

**Conditional compilation** was supposed to be small. Ori now has `#target(os: "linux")` and `#cfg(debug)` attributes that can gate functions, types, constants, impl blocks, and imports. The false branch isn't type-checked. Getting attributes to work correctly on every AST node type, through incremental parsing, and through the formatter, took longer than the AIMS pipeline work.

**The type registry** (`ori_registry`) centralized all built-in type information — methods, operators, signatures — into a single source of truth. Previously this was scattered across 1,900 lines of parallel allowlists in three crates. The migration surfaced a dozen inconsistencies nobody had noticed, like parameter names that differed between the type checker and evaluator.

**LLVM codegen quality** improved across the board: string constant deduplication, dead code elimination after noreturn calls, surgical struct field loading (only load the fields you use), and range loop specialization that generates a single comparison instead of the full iterator protocol.

---

## What's Next

`ori_repr` continues with enum niche filling and escape analysis. I also approved a compile-time reflection proposal — `fields_of(T)` and `$for` loops that expand at compile time, enough to write a JSON serializer in pure Ori without runtime reflection or FFI.

But mostly I'll keep doing what makes a compiler real: finding the bugs that only show up in programs that aren't trivial, fixing them, writing tests so they stay fixed.

---

*Follow along on [GitHub](https://github.com/upstat-io/ori-lang) or the [roadmap](/roadmap).*
