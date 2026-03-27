---
title: "Everything Was Fine Until We Added Landing Pads"
date: 2026-03-03
description: "How adding exception handling broke macOS and Windows in completely different ways, and why we had to upgrade LLVM by four major versions."
---

Everything was working. All three platforms. Green CI.

Then we added landing pads, and everything fell apart.

---

## The Setup

Ori has compiled on all three platforms since day one — cross-platform CI from the first commit, not bolted on later. Ubuntu, macOS, Windows. We dealt with the platform bootstrapping early: custom LLVM packages, path separators, `.exe` suffixes. Boring, solved, green.

Then we started implementing ARC optimizations in the LLVM backend. Ori uses Automatic Reference Counting, and to make that work with exceptions, you need *landing pads* — LLVM's mechanism for cleaning up when a function unwinds. If a function allocates some RC'd objects and then calls something that panics, a landing pad decrements those reference counts before the stack unwinds. Without them, you leak memory on every panic.

We also implemented `catch(expr:)` — Ori's try/catch — which wraps an expression in an `invoke`/`landingpad` pair so panics become `Result` values.

Both worked perfectly on Linux. All tests passed. We merged.

Then CI ran on macOS and Windows.

---

## Two Platforms, Two Different Failures

macOS failed fast. Stack overflow during LLVM's instruction selection for aarch64. Landing pads add extra basic blocks, which means more IR, which means deeper recursion in LLVM's code generation. The default 8 MiB stack wasn't enough.

Windows failed differently. We were generating Itanium-style exception handling (`landingpad` instructions), but Windows MSVC uses SEH — `catchpad`, `catchswitch`, `catchret`. These aren't different names for the same thing. They're fundamentally different IR constructs with different semantics. Our Itanium `landingpad` either didn't catch Rust panics at all, or when it did, Rust detected that its panic was caught by non-Rust code and aborted:

```
Rust panics must be rethrown
```

Same feature. Three platforms. Three completely different failure modes.

---

## The macOS Spiral

The stack overflow seemed fixable — just bump the thread stack size. I stole `rustc`'s pattern: spawn the compiler on a 32 MiB thread:

```rust
const STACK_SIZE: usize = 32 * 1024 * 1024;

fn main() {
    let builder = std::thread::Builder::new()
        .name("ori-main".into())
        .stack_size(STACK_SIZE);
    let handle = builder.spawn(real_main).unwrap_or_else(|e| {
        eprintln!("error: failed to spawn main thread: {e}");
        std::process::exit(1);
    });
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}
```

But while debugging that, I found a worse problem: the linker was hanging. Not crashing — *hanging*. No output, no error, the CI job would sit there until the timeout killed it.

### The Infinite Loop Nobody Saw

My AOT linker has platform detection for choosing flags. It checked the OS component of the target triple:

```rust
fn is_macos(target: &str) -> bool {
    parts.get(2).map_or(false, |os| os == "darwin")
}
```

LLVM's native triple on macOS returns `aarch64-apple-darwin25.2.0`. With a version number. My function was comparing `"darwin25.2.0" == "darwin"`. False. So the linker thought it was on Linux and passed `-Bstatic` to macOS's `ld`. Which failed. Which triggered the retry logic. Which called `link()` again. Which detected "Linux" again. Which passed the same bad flags. Forever.

No retry limit. No recursion depth check. An infinite loop that only manifested on macOS because Linux's triple doesn't have a version suffix. The linker had been running on macOS CI all along, but until landing pads, the generated code was simple enough that the wrong flags didn't matter.

Fixed with `starts_with("darwin")` and a one-retry cap. But this was hours of staring at CI logs that just... stopped.

---

## The LLVM 17 Wall

With macOS unblocked, I turned to Windows. And hit a wall.

The Itanium exception model doesn't work on Windows MSVC. Period. You need SEH. SEH requires `catchpad`, `catchswitch`, and `catchret`. These exist in LLVM, but `inkwell` (our Rust LLVM bindings) at `llvm17-0` didn't expose them.

I spent hours looking for workarounds. Raw C API calls? Unsafe and outside our abstraction layer. Different exception mechanism? Itanium landing pads simply don't catch Rust panics on MSVC. Disable `catch(expr:)` on Windows? Landing pads for ARC cleanup have the same fundamental problem.

Late on a Sunday night, the conclusion became inescapable: upgrade LLVM. Not a minor bump. LLVM 17 to LLVM 21. Four major versions.

The mechanical parts were fine — LLVM 19 renamed some debug info APIs, find-and-replace. The hard part was implementing SEH properly. The Itanium model is one instruction (`landingpad`). SEH is three (`catchswitch`, `catchpad`, `catchret`), and every call instruction inside a `catchpad` scope needs a funclet bundle saying "I belong to this exception handling scope." Miss one call, and LLVM's verifier rejects your IR. I had to thread a `current_funclet_pad` through the entire codegen pipeline.

And then it *still* didn't work. Even with SEH implemented correctly, catching a Rust panic via `catchpad` on Windows triggers the "Rust panics must be rethrown" abort.

The fix: don't use `catchpad` for `catch(expr:)` on Windows at all. Generate a thunk, pass it to `ori_try_call` — a runtime function that wraps `std::panic::catch_unwind`. The catch happens in Rust, not in LLVM IR. Different codegen path, same language semantics.

```
// Linux: invoke + landingpad (Itanium)
// Windows: thunk + ori_try_call (catch_unwind wrapper)
// User code: catch(expr:) — identical on both
```

Platform-specific implementation hiding behind a platform-agnostic language feature. This is what "cross-platform" actually means.

---

## The Parade of Smaller Nightmares

With the big problems solved, the smaller ones came out.

**`c_char` is not `i8` on ARM.** C's `char` is signed on x86_64 and unsigned on aarch64. I had hardcoded `*const i8` in runtime FFI functions. Compiled and ran perfectly on Intel for a month.

**Windows `link.exe` sends errors to stdout.** Every Unix linker uses stderr. My `LinkerError` struct didn't have a `stdout` field. Windows link failures were silently swallowed — "linking failed" with an empty error message. Hours of debugging before I thought to capture both streams.

**Flaky tests were concurrency.** The runtime has a global `RC_LIVE_COUNT` atomic counter for leak detection. Five test modules were reading and writing it concurrently. Linux happened to schedule them in a lucky order. macOS didn't. Fix: a process-global mutex across 57 tests. Not pretty. Correct.

**Rust 1.93 showed up uninvited.** Mid-debugging, GitHub Actions updated their Rust toolchain, adding a `function-casts-as-integer` lint. Every JIT runtime symbol mapping broke. One hundred and sixty-five two-step casts. I also pinned the toolchain after that.

**Dynamic library discovery.** Hardcoded per-platform library lists kept breaking. Replaced the whole thing with a `build.rs` that calls `rustc --print native-static-libs`. Let Rust tell us what it needs. Should have done this from the start.

---

## Where We Are Now

All three platforms are green. Not smoke tests — the full AOT test suite. Compile, link, run, verify output on Ubuntu x86_64, macOS ARM64, and Windows x86_64.

The thing about cross-platform is that it's never one big problem. It's thirty small problems that each think they're the only one. And each one is invisible until you run the code on a machine you didn't develop on.

Every shortcut came due at the same time. The linker retry with no limit. The hardcoded `*const i8`. The `RC_LIVE_COUNT` without a mutex. Every one of them worked fine for weeks. They all broke in the same two days.

That's why CI exists. And that's why it runs on three operating systems.

---

*I'm Eric, building Ori. This is the second post — the first is [here](/blog/building-ori-from-scratch).*
