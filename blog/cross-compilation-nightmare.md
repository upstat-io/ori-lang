---
title: "Everything Was Fine Until We Added Landing Pads"
date: 2026-03-03
description: "Cross-platform CI was green. Then we added exception handling for ARC cleanup, and macOS and Windows both broke. The two-day spiral that ended with upgrading LLVM from 17 to 21."
---

**Everything was working. All three platforms. Green CI.**

**Then we added landing pads, and everything fell apart.**

This is the story of the last two days: how a feature that worked perfectly on Linux simultaneously broke macOS and Windows in completely different ways, and the slow realization that the only fix was upgrading our entire LLVM toolchain by four major versions.

---

## The Setup: Cross-Platform Was Already Done

Ori has compiled on all three platforms since day one. That was a deliberate choice — cross-platform CI from the first commit, not something you bolt on later. Ubuntu, macOS, Windows. We dealt with the platform bootstrapping early: a [custom LLVM package for Windows](https://github.com/upstat-io/llvm-package-windows), path separators, `.exe` suffixes, the usual. Boring, solved, green.

Then, in the third week of February, we started implementing ARC optimizations in the LLVM backend. Ori uses Automatic Reference Counting for memory management, and to make that work correctly with exceptions, you need *landing pads* — LLVM's mechanism for cleaning up resources when a function unwinds. If a function allocates some RC'd objects and then calls another function that panics, you need a landing pad to decrement those reference counts before the stack unwinds. Without them, you leak memory on every panic.

We also implemented `catch(expr:)` — Ori's try/catch construct — which wraps an expression in an `invoke`/`landingpad` pair so panics can be caught and converted to `Result` values.

Both features worked perfectly on Linux. All tests passed. We merged.

Then CI ran on macOS and Windows.

---

## Act 1: Two Platforms, Two Different Failures

macOS failed fast. The compiler itself was crashing — stack overflow during LLVM's instruction selection pass for the aarch64 (ARM) backend. This was new. The same code compiled fine before we added landing pads, because landing pads add extra basic blocks, which means more IR, which means deeper recursion in LLVM's code generation internals.

Windows failed differently. The `catch(expr:)` construct was generating Itanium-style exception handling IR (`landingpad` instructions), but Windows MSVC uses a completely different exception model: SEH (Structured Exception Handling). The Itanium model uses `landingpad` instructions. SEH uses `catchpad`, `catchswitch`, and `catchret`. They're not just different names for the same thing — they're fundamentally different IR constructs with different semantics.

On Linux, Rust panics unwind using the Itanium model. Our `landingpad` catches the unwind, runs cleanup, done. On Windows MSVC, Rust panics use `_CxxThrowException` — C++ exceptions, which go through SEH. Our Itanium `landingpad` either doesn't catch them at all, or when it does, Rust detects that its panic was caught by non-Rust code and aborts:

```
Rust panics must be rethrown
```

Two platforms. Two completely different breakages. One shared root cause: we were generating exception handling IR that only worked on Linux.

---

## Act 2: The Two-Day Troubleshooting Sprint

On March 2nd, I started trying to fix this within LLVM 17.

The macOS stack overflow seemed fixable — just bump the thread stack size. But fixing it properly meant understanding *why* the aarch64 backend needed more stack than x86_64. The answer: LLVM's ARM instruction selector does deeper recursion through C++ FFI during code generation. This is inside LLVM's C++ code, called through FFI — Rust's stack probes can't help. The default 8 MiB thread stack on macOS wasn't enough.

I stole `rustc`'s pattern: spawn the entire compiler on a 32 MiB thread:

```rust
const STACK_SIZE: usize = 32 * 1024 * 1024; // 32 MiB

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

Plus `ensure_sufficient_stack` guards on the recursive codegen functions — drop function generation, RC value traversal, type resolution. Defense in depth.

But while debugging macOS, I uncovered a worse problem: the linker was hanging. Not crashing — *hanging*. No output, no error, the CI job would just sit there until the 10-minute timeout killed it.

### The Infinite Loop Nobody Saw

My AOT linker has platform detection to choose the right flags. On Linux: `-Bstatic` and `-Bdynamic`. On macOS: `-search_paths_first`. The detection checked the OS component of the target triple:

```rust
fn is_macos(target: &str) -> bool {
    // Split "aarch64-apple-darwin" on "-", check os == "darwin"
    parts.get(2).map_or(false, |os| os == "darwin")
}
```

When you query LLVM for the *native* triple on macOS, it returns `aarch64-apple-darwin25.2.0`. With a version number. My function was comparing `"darwin25.2.0" == "darwin"`. False. So the linker thought it was on Linux and passed `-Bstatic` to macOS's `ld`. Which failed.

Which triggered the retry logic. Which called `link()` again. Which detected the same "Linux" platform. Which passed the same bad flags. Which failed. Which triggered retry. Forever.

```
retry_link() → link() → execute_with_retry() → retry_link() → ∞
```

No retry limit. No recursion depth check. An infinite loop that only manifested on macOS because Linux's target triple doesn't have a version number. The AOT linker had been running on macOS CI all along — but until we added landing pads, the generated code was simple enough that the wrong linker flags happened to not matter.

Fixed with `starts_with("darwin")` and a one-retry cap. But this was hours of staring at CI logs that just... stopped.

---

## Act 3: The LLVM 17 Wall

With macOS unblocked, I turned to Windows. And hit a wall.

The Itanium exception model doesn't work on Windows MSVC. Period. You need SEH. SEH requires `catchpad`, `catchswitch`, and `catchret` instructions. These instructions exist in LLVM — they've existed since LLVM 3.8. But `inkwell` (the Rust LLVM bindings we use) at version `llvm17-0` didn't expose them.

I spent hours looking for workarounds:

- **Could we emit the SEH instructions manually through the C API?** Technically yes, but without `inkwell` wrappers, we'd be writing raw `LLVMBuildXxx` calls with `*mut LLVMValue` pointers. Unsafe, unergonomic, and completely outside our abstraction layer.
- **Could we use a different exception mechanism on Windows?** Itanium landing pads simply don't work for catching Rust panics on MSVC. That's not a limitation of our code — it's how Rust's panic runtime works on Windows.
- **Could we disable `catch(expr:)` on Windows?** We could, but landing pads for ARC cleanup *also* need to work, and those have the same fundamental problem.

Late on March 2nd, the conclusion became inescapable: we had to upgrade LLVM. Not a minor version bump. LLVM 17 to LLVM 21. Four major versions. The latest `inkwell` release — `llvm21-1` — exposes the SEH instructions we needed.

### The Upgrade

52 files changed. `inkwell` from `llvm17-0` to `llvm21-1`. `llvm-sys` from `170` to `211`.

The mechanical parts were straightforward: LLVM 19 renamed the debug info builder APIs (the "DbgRecord migration"), so `LLVMDIBuilderInsertDbgValueAtEnd` became `LLVMDIBuilderInsertDbgValueRecordAtEnd`. Find-and-replace.

The hard part was implementing SEH properly. On the Itanium side, a try/catch is:

```
invoke @function() to %continue unwind %landing_pad

landing_pad:
  %ex = landingpad { ptr, i32 }
  ; cleanup + handle
```

On SEH:

```
invoke @function() to %continue unwind %dispatch

dispatch:
  %cs = catchswitch within none [%handler] unwind to caller

handler:
  %cp = catchpad within %cs [ptr null, i32 64, ptr null]
  ; every call here needs a "funclet bundle"
  catchret from %cp to %continue
```

The funclet bundle is the killer. Every single call instruction inside a `catchpad` scope needs an operand bundle that says "I belong to this exception handling scope." Miss one, and LLVM's verifier rejects your IR. I had to thread a `current_funclet_pad` field through the entire code generation pipeline so that `emit_rt_call()` — the function that emits runtime calls for RC operations, COW, string allocation, everything — automatically attaches the right bundle when we're inside a catch block.

And then it *still* didn't work. Even with SEH properly implemented, catching a Rust panic via `catchpad` on Windows MSVC triggers the "Rust panics must be rethrown" abort. Rust's panic runtime detects that its exception was caught by non-Rust code and terminates the process.

The fix: don't use `catchpad` for `catch(expr:)` on Windows at all. Instead, generate a thunk — a small closure-like function that captures the expression — and pass it to `ori_try_call`, a runtime function that wraps `std::panic::catch_unwind`. The catch happens in Rust, not in LLVM IR. Different codegen path, same language semantics.

```
// Linux: invoke + landingpad (Itanium)
// Windows: thunk + ori_try_call (catch_unwind wrapper)
// User code: catch(expr:) — identical on both
```

Platform-specific implementation hiding behind a platform-agnostic language feature. This is what "cross-platform" actually means.

---

## Act 4: The Parade of Smaller Nightmares

With the big problems solved, the smaller ones came out of the woodwork.

### `c_char` Is Not `i8` on ARM

```
error[E0308]: mismatched types
  expected `*const i8`, found `*const u8`
```

C's `char` type is signed on x86_64 and unsigned on aarch64. I had hardcoded `*const i8` in runtime FFI functions. Fix: replace with `std::ffi::c_char` everywhere. The bug compiled and ran perfectly on Intel for a month.

### Windows `link.exe` Sends Errors to stdout

Every Unix linker sends errors to stderr. MSVC's `link.exe` sends them to stdout. My `LinkerError` struct didn't even have a `stdout` field. Windows link failures were silently swallowed — "linking failed" with an empty error message. Hours of "why is linking failing with no error?" before I thought to capture both streams.

### Flaky Tests Were Concurrency

With all platforms compiling, I turned on the full test suite. Tests started passing, then failing, then passing again. The runtime has a global `RC_LIVE_COUNT` atomic counter for leak detection. Five test modules were reading/writing it concurrently. Linux happened to schedule them in a lucky order most of the time. macOS didn't.

Fix: a process-global mutex. Fifty-seven tests across five modules, each acquiring `lock_rc()` before touching RC state. Not pretty. Correct.

### Rust 1.93 Showed Up Uninvited

Mid-debugging, GitHub Actions updated their `stable` Rust toolchain to 1.93, which added a `function-casts-as-integer` lint. All 165 JIT runtime symbol mappings broke:

```rust
// Before (Rust 1.92): fine
("ori_list_push", ori_list_push as usize)

// After (Rust 1.93): denied
("ori_list_push", ori_list_push as *const () as usize)
```

One hundred and sixty-five two-step casts. I also pinned the toolchain to `1.93.1` across all workflows. Surprise toolchain upgrades during a cross-platform debugging marathon are not fun.

### Dynamic Library Discovery

Hardcoded per-platform library lists (`-lm -lpthread` on Linux, `-framework CoreFoundation` on macOS, `msvcrt.lib` on Windows) kept breaking as we hit edge cases. Replaced the entire thing with a `build.rs` that calls `rustc --print native-static-libs` and passes the result via an environment variable. Let Rust tell us what it needs. Should have done this from the start.

---

## The Timeline

Here's what two days of cross-platform debugging actually looks like:

| Time | What |
|------|------|
| Mar 2, 14:00 | Start fixing cross-platform CI, `c_char` fix for ARM |
| Mar 2, 15:30 | Windows EXE suffix, path separator fixes |
| Mar 2, 16:00 | Windows linker: UNC paths, CRT libraries, MSVC detection |
| Mar 2, 16:30 | Exclude LLVM AOT tests from cross-platform (band-aid) |
| Mar 2, 17:00 | Discover link.exe stdout issue, add stdout capture |
| Mar 2, 18:00 | Dynamic library discovery via `build.rs` |
| Mar 2, 18:45 | Discover MSVC link.exe via VS installation, not PATH |
| Mar 2, 22:44 | **Realize LLVM 17 can't do SEH. Upgrade to LLVM 21.** |
| Mar 3, 07:00 | SEH catch trampoline for Windows, enable JIT symbols |
| Mar 3, 07:20 | Serialize RC-touching tests to fix flakiness |
| Mar 3, 10:18 | macOS CI hangs. Discover the infinite linker retry loop. |
| Mar 3, 10:29 | Add continue-on-error while debugging |
| Mar 3, 12:10 | Troubleshoot macOS ARM64 stack overflow |
| Mar 3, 12:49 | Fix: `starts_with("darwin")`, retry cap, 32 MiB stack |
| Mar 3, 14:41 | Remove band-aids. Full AOT test suite on all platforms. Green. |

The LLVM 21 upgrade commit — the one that changed 52 files — landed at 10:44 PM on a Sunday. Not because I planned it that way. Because that's when I accepted there was no other path.

---

## What I Learned

**Features interact across platforms in ways you can't predict.** Landing pads worked perfectly on Linux. On macOS they blew the stack. On Windows they used the wrong exception model. Same feature, three platforms, three completely different failure modes. You don't find these bugs in design review. You find them at 10 PM on a Sunday.

**"Fix it later" has a compound interest rate.** The linker retry logic with no limit? Wrote it weeks ago, it was a quick hack, worked fine on Linux. The hardcoded `*const i8`? Worked fine on Intel — the ARM CI didn't exercise that code path until now. The `RC_LIVE_COUNT` without a mutex? Worked fine with Linux's default thread scheduling. Every shortcut came due at the same time.

**LLVM version upgrades are both terrifying and fine.** I put off the LLVM 21 upgrade for weeks because it felt like a huge risk. When I finally did it, the mechanical migration took a few hours. The *new* code (SEH implementation) was the hard part, and I would have needed to write that regardless. The fear of the upgrade was worse than the upgrade.

**Cross-platform isn't a feature you add.** It's a constraint that reveals every assumption you made. What sign is a `char`? Where do error messages go? How deep does the stack need to be? What exception model does the OS use? Every platform answers differently, and every answer is correct for that platform. Your code has to be correct for all of them.

---

## Where We Are Now

All three platforms are green. Not smoke tests — the full AOT test suite. Compile, link, run, verify output. On Ubuntu x86_64, macOS ARM64, and Windows x86_64.

Linux ARM64 is confirmed working — we validated it on a GCP `t2a-standard-2` ARM instance running Ubuntu 22.04 with LLVM 21. It's still commented out in the release workflow because we need CI infrastructure for building release binaries on ARM, but the compiler runs correctly on aarch64.

The cost of these two days: 52 files changed for the LLVM upgrade, a new exception handling model, a new runtime function, a new linker architecture, 57 tests refactored, 165 function pointer casts updated, and exactly one infinite loop that only triggered on Apple Silicon.

The thing about cross-platform is that it's never one big problem. It's thirty small problems that each think they're the only one. And each one is invisible until you run the code on a machine you didn't develop on.

That's why CI exists. And that's why it runs on three operating systems and two architectures.

---

*I'm Eric, and I'm building Ori, a statically-typed, expression-based language designed for the AI era. This is the second post in a series about the journey. The first one is [here](/blog/building-ori-from-scratch).*
