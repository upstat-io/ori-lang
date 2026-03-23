# Deep Safety: Research

Research into Linux kernel Rust `unsafe` usage patterns and Ori's capability-based alternatives.

---

## Status Note (2026-03-22)

This document has been re-audited against the live Ori repository, the current spec/proposal set,
and externally-available primary sources. The result is more conservative than earlier revisions.

### Repo-grounded feasibility assessment

Deep Safety is directionally plausible for Ori, but it is not an implementation-ready project on
the current compiler baseline.

As of 2026-03-22:

- Basic positive capabilities (`uses`, `with...in`) exist in the parser/type checker/evaluator,
  but capability provision to called functions is still skipped in
  `tests/spec/expressions/with_expr.ori`, stateful handlers are not implemented, and LLVM/AOT
  support is missing.
- `unsafe { ... }` exists syntactically and passes spec tests, but it is currently a transparent
  expression wrapper; the low-level unsafe operations it is meant to gate (raw pointer
  dereference, transmute, mutable statics, inline assembly) are not implemented yet.
- `extern` blocks parse, but there is no type-checker, evaluator, LLVM, runtime, or stdlib FFI
  stack behind them yet. `CPtr`, Deep FFI ownership/error protocols, callbacks, `out`
  parameters, and `#free` remain spec/proposal work rather than end-to-end compiler behavior.
- Concurrency patterns remain interpreter stubs, channels are stubbed, and the `Sendable`/task
  model is still roadmap work rather than shipped compiler/runtime behavior.
- None of the Deep Safety-specific low-level capabilities (`InterruptCtx`, `VolatileIO`, `DMA`,
  `Synchronization`, `RCU`, etc.), typed address wrappers, or negative-effect semantics exist in
  the compiler today.

### Practical conclusion

Ori can plausibly grow into Deep Safety, but only after finishing foundational capability, FFI,
concurrency, and AOT work. Any schedule that treats `without` plus four kernel capabilities as
the immediate next implementation step is not grounded in repository reality.

---

## Part 1: Why Rust Drowns in `unsafe` for Kernel Development

Rust's safety model is binary: code is either fully safe or `unsafe`. The moment you write `unsafe { }`, all compiler guarantees vanish inside that block. In kernel code:

- MMIO register access -> `unsafe`
- Inline assembly -> `unsafe`
- Calling C functions -> `unsafe`
- Global mutable state -> `unsafe`
- Pointer arithmetic -> `unsafe`
- Volatile reads -> `unsafe`
- Custom allocators -> `unsafe`

In the Linux kernel Rust bindings, `unsafe` appears in nearly every function. At that point it's not a safety boundary — it's noise. Developers stop reading it, reviewers stop scrutinizing it, and the entire model collapses into "we hope the comments are accurate."

The deeper issue: Rust treats `unsafe` as a trust boundary ("I, the programmer, assert this is correct"), but offers no way to verify that assertion or narrow what's being trusted.

---

## Part 2: Comprehensive Taxonomy of `unsafe` in Linux Kernel Rust Code

Sources:
- Linux Kernel Rust Coding Guidelines (docs.kernel.org/rust/coding-guidelines.html)
- Standards for use of unsafe Rust in the kernel (LWN Articles/982868)
- Banning unsafe in Rust for Linux device drivers (LWN Articles/985848)
- An Empirical Study of Rust-for-Linux (USENIX ATC 2024)
- Rust for Linux: Understanding the Security Impact (ACSAC 2024)
- Rust for Linux kernel crate API docs
- CVE-2025-68260 Analysis (first Rust CVE in Linux kernel)

### The Five Fundamental Unsafe Superpowers

These are the five operations Rust's `unsafe` keyword unlocks. Every kernel `unsafe` block ultimately bottoms out in one or more of these.

#### 1. Dereferencing Raw Pointers (`*const T` / `*mut T`)

**Why unsafe:** Raw pointers can be null, dangling, misaligned, or alias other mutable references. The compiler cannot verify any of these properties. Unlike references (`&T` / `&mut T`), raw pointers carry no lifetime information and no aliasing guarantees.

**Kernel prevalence:** Extremely high. Nearly every interaction with C kernel data structures involves raw pointers. The kernel's C API passes pointers freely — `struct device *`, `struct file *`, `struct inode *`, etc. — and Rust must dereference them to access data.

#### 2. Calling Unsafe Functions or Methods

**Why unsafe:** The function declares a safety contract (preconditions) that the compiler cannot enforce. The caller must manually verify these preconditions hold.

**Kernel prevalence:** Extremely high. ALL C functions called via FFI are implicitly `unsafe`. Since the kernel crate wraps hundreds of C functions (`krealloc`, `kfree`, `ioremap`, `iounmap`, `copy_from_user`, `dma_alloc_coherent`, etc.), virtually every safe Rust wrapper internally calls an unsafe function.

#### 3. Accessing or Modifying Mutable Static Variables

**Why unsafe:** Global mutable state is inherently unsynchronized. Multiple threads (or interrupt handlers) can race on reads/writes, causing undefined behavior. Rust cannot verify absence of data races at compile time for global mutable state.

**Kernel prevalence:** Moderate. Kernel modules use global state for per-module data, driver registrations, counters, and configuration. The `module!` macro generates static initialization structures.

#### 4. Implementing Unsafe Traits

**Why unsafe:** The trait declares invariants that implementations must uphold but that the compiler cannot check. Other code (`unsafe` or not) relies on these invariants being true.

**Kernel prevalence:** High. `Send`, `Sync`, `GlobalAlloc`, `ForeignOwnable`, `AlwaysRefCounted`, and various driver traits are all `unsafe` traits that kernel code must implement.

#### 5. Accessing Fields of a Union

**Why unsafe:** A union stores multiple types in the same memory, but only one is valid at any time. The compiler cannot track which field was last written, so reading the wrong field produces garbage or undefined behavior.

**Kernel prevalence:** Moderate. C kernel headers use unions extensively (e.g., socket address unions, ioctl parameter unions). Rust bindings generated by `bindgen` expose these as Rust unions.

---

### Kernel-Specific Unsafe Categories

#### 6. FFI / C Interop (Bindgen-Generated Bindings)

Every call from Rust into a C kernel function goes through `bindings::*`, which are auto-generated by `bindgen` from kernel headers. All such functions are `extern "C"` and implicitly `unsafe`.

**Why unsafe:** The Rust compiler knows nothing about the C function's implementation. It cannot verify that the function won't dereference a null pointer, won't cause a use-after-free, or won't violate any other invariant.

**Examples:**
- `bindings::krealloc(ptr, size, flags)` — memory allocation
- `bindings::kfree(ptr)` — memory deallocation
- `bindings::ioremap(offset, size)` — mapping I/O memory
- `bindings::copy_from_user(to, from, n)` — copying data from user space
- `bindings::dma_alloc_coherent(dev, size, dma_handle, flags)` — DMA allocation

**Abstraction pattern:** The `rust/kernel/` crate wraps each C function in a safe Rust API, confining the `unsafe` to a small, auditable boundary.

#### 7. C Helper Functions (`rust/helpers/*.c`)

Some C kernel APIs are inline functions or macros that `bindgen` cannot process. Rust-for-Linux writes thin C wrapper functions in `rust/helpers/` that call the actual API.

**Why unsafe:** Same as FFI — the Rust compiler cannot verify the C helper's implementation.

#### 8. Memory-Mapped I/O (MMIO)

Reading from or writing to device registers mapped into the CPU's address space via `ioremap()`. The `io_mem` module provides safe wrappers around `readb`/`readw`/`readl`/`writeb`/`writew`/`writel`.

**Why unsafe:**
- `ptr::read_volatile` / `ptr::write_volatile` are unsafe because they dereference raw pointers
- The pointer itself comes from `ioremap`, which returns a raw kernel virtual address
- Hardware registers may have alignment requirements the compiler cannot enforce
- Side effects on read (clearing interrupt status, advancing FIFO pointers)

#### 9. DMA (Direct Memory Access) Operations

Allocating memory regions that both the CPU and a device can access. Requires `dma_alloc_coherent()` / `dma_map_single()` and managing physical/bus addresses.

**Why unsafe:**
- Raw physical addresses that have no Rust type-system representation
- Memory modified asynchronously by hardware (no Rust borrow checker coverage)
- IOMMU translations invisible to Rust
- Cache coherency requirements
- Lifetime constraints spanning Rust/C boundaries

#### 10. Inline Assembly (`asm!` / `global_asm!`)

Embedding raw machine instructions for architecture-specific operations: interrupt enable/disable, memory barriers, TLB flushes, context switches, special register access (CR3, MSR, etc.).

**Why unsafe:**
- Assembly can corrupt any register, including the stack pointer
- Violates Rust's aliasing rules by accessing memory directly
- Breaks calling conventions
- Modifies control flow in ways invisible to the compiler
- Interacts with hardware state (interrupt flags, page tables)

#### 11. Synchronization Primitives (Mutex, SpinLock, RwSemaphore, CondVar)

The `kernel::sync` module provides `Mutex<T>`, `SpinLock<T>`, `RawSpinLock`, `RwSemaphore`, and `CondVar`, all wrapping their C counterparts.

**Why unsafe:**
- **Initialization:** Kernel mutexes/spinlocks require C-level initialization that must happen before first use — the pinned-initialization problem
- **Lock/unlock matching:** The C functions have no type-level association
- **Interrupt safety:** Wrong variant (irq-safe vs. non-irq-safe) in wrong context is a latent bug
- **`Send`/`Sync` impls:** Getting these wrong enables data races

#### 12. The `Opaque<T>` Type

A wrapper for C structs that Rust should never interpret directly. Foundation of most kernel type abstractions.

**Why unsafe:**
- Allows uninitialized values
- Permits mutation through shared references (violates Rust's fundamental aliasing rule)
- Lacks uniqueness guarantees for mutable references

#### 13. Reference Counting (`ARef<T>`, `AlwaysRefCounted`)

Kernel objects use C-level reference counting (`kref`, `refcount_t`). The `ARef<T>` smart pointer provides RAII semantics.

**Why unsafe:**
- `AlwaysRefCounted` is an unsafe trait — implementors must guarantee correct C refcount behavior
- Constructing `ARef` from a raw pointer requires trusting pointer validity
- The refcount is managed by C code, so Rust cannot verify it's never zero when accessed

#### 14. `ForeignOwnable` Trait (Rust-to-C Ownership Transfer)

Enables passing Rust objects to C code (as `void *`) and reclaiming them later. Used for driver private data, work items, timer callbacks, etc.

**Why unsafe:**
- `into_foreign()` erases all type information
- `from_foreign()` reconstructs from raw pointer — wrong/stale/double-freed pointer is UB
- C code may hold the pointer across arbitrary time spans

#### 15. Callback / VTable Registration

C subsystems call into Rust via function pointer tables (vtables). The `#[vtable]` attribute macro generates these tables from Rust trait implementations.

**Why unsafe:**
- Generated vtable contains raw function pointers called directly by C
- Each callback receives raw pointers that must be unsafely cast
- Lifetime management across the callback boundary is invisible to Rust
- C can call the callback after the Rust object has been dropped

#### 16. User-Space Pointer Access (`user_ptr`)

`UserSlicePtr` wraps `copy_from_user()` and `copy_to_user()` for safely copying data between kernel and user space.

**Why unsafe internally:**
- The pointer comes from user space and can be anything
- The underlying C functions handle page faults specially
- TOCTOU concerns — user-space memory can be concurrently modified
- Race conditions on user memory are permitted by the kernel's memory model but would be UB under Rust's strict model

#### 17. `container_of!` Macro (Intrusive Data Structures)

Given a pointer to a struct field, computes a pointer to the containing struct. Fundamental to Linux's intrusive linked lists, red-black trees, etc.

**Why unsafe:**
- Raw pointer arithmetic (`offset_of!` to compute field offset, then subtraction)
- Violates Rust's Stacked Borrows / Tree Borrows model
- Computed pointer's validity depends on original allocation being live

#### 18. Pinned Initialization (`pin-init` Framework)

Many kernel objects (mutexes, condition variables, device structures) must not be moved after initialization because C code holds raw pointers to them.

**Why unsafe:**
- `MaybeUninit<T>` used before the object is fully initialized
- `pin_init_from_closure()` is unsafe
- Pin projections require `unsafe` `map_unchecked_mut()`
- Rust's "return by value" model is incompatible with pinned, in-place initialization

#### 19. Custom Allocator (`GlobalAlloc` for `krealloc`/`kfree`)

The kernel cannot use the standard library's allocator. A custom `GlobalAlloc` implementation delegates to the kernel's `krealloc()` and `kfree()`.

**Why unsafe:**
- `GlobalAlloc` is an unsafe trait
- Panicking from an allocator is UB
- GFP flags determine allocation context — wrong flag can deadlock
- `krealloc` returning null means allocation failure (fallible allocation)

#### 20. `Send` and `Sync` Marker Trait Implementations

Marking types as safe to transfer between threads (`Send`) or to share via references (`Sync`).

**Why unsafe to implement:**
- Compiler-trusted traits — all concurrent code assumes they're correct
- Kernel types wrapping C structures often need manual impls
- Incorrect implementation enables data races (instant UB)

#### 21. Interrupt Handling and Atomic Context

Registering interrupt handlers, managing IRQ lines, disabling/enabling preemption.

**Why unsafe:**
- Interrupt handlers run in atomic context — cannot sleep, allocate with `GFP_KERNEL`, or acquire sleeping locks
- IRQ registration passes a function pointer that C calls asynchronously
- `local_irq_save()`/`local_irq_restore()` modify CPU interrupt flags directly

#### 22. RCU (Read-Copy-Update)

Lock-free concurrent data structure access.

**Why unsafe:**
- `rcu_dereference()` and `rcu_assign_pointer()` involve intentional data races
- RCU critical sections have invisible correctness constraints
- The grace period guarantee is a temporal invariant Rust cannot express
- LKMM relies on compiler behaviors the Rust memory model does not adopt

#### 23. Workqueue and Deferred Work

Scheduling work to be executed later in process context.

**Why unsafe:**
- Work items contain function pointers the kernel calls asynchronously
- Type erasure — C workqueue passes raw `struct work_struct *`
- Lifetime management: work item must outlive the workqueue submission

#### 24. Per-CPU Variables

Variables that have one instance per CPU core.

**Why unsafe:**
- Accessing per-CPU data requires disabling preemption first
- Address computed at runtime using CPU-specific offsets (raw pointer arithmetic)
- No Rust type can express "valid only while preemption is disabled on the current CPU"

#### 25. Module Initialization and Registration

The `module!` macro generates C-compatible `__init` and `__exit` functions.

**Why unsafe:**
- Generates `extern "C"` functions callable by the C module loader
- Error handling during init must properly unwind partial initialization
- `ThisModule` wraps a raw pointer managed by C code

#### 26. Transmutation and Type Reinterpretation

Reinterpreting one type's bit pattern as another via `mem::transmute`, `ptr::read`, pointer casts, or similar.

**Why unsafe:**
- Compiler cannot verify bit pattern is valid for target type
- Layout compatibility not guaranteed without `#[repr(C)]`
- Transmuting between types with different alignment requirements causes UB

#### 27. Uninitialized Memory (`MaybeUninit<T>`)

Allocating memory without initializing it, then filling it in later.

**Why unsafe:**
- Reading uninitialized memory is UB
- `MaybeUninit::assume_init()` is unsafe — caller asserts initialization occurred
- Kernel pattern: allocate, pass pointer to C function that fills it in, then `assume_init()`

#### 28. Lifetime and Variance Circumvention

Extending or fabricating lifetimes using `transmute`, `&*(ptr as *const T)` patterns, or `PhantomData`.

**Why unsafe:**
- Fabricating a lifetime tells the compiler "this reference is valid" — if it isn't, UB
- C kernel objects have lifetimes managed by reference counting or global registries, not Rust's stack-based lifetime model

#### 29. `#[repr(C)]` and Layout-Sensitive Code

Ensuring Rust structs have C-compatible memory layout.

**Why unsafe (indirectly):**
- Accessing a `#[repr(C)]` struct through a raw pointer is unsafe
- Without `#[repr(C)]`, Rust can reorder fields, add padding, change layout between versions

#### 30. Error Code / Errno Handling

Converting between C `int` error codes and Rust's `Result` type.

**Why unsafe (indirectly):**
- Constructing an `Error` from arbitrary `c_int` requires trusting the value is a valid negative errno

#### 31. Revocable / Conditional Access Patterns

The `Revocable<T>` type allows access to data that may be "revoked" when a device is removed while still referenced.

**Why unsafe:**
- Atomic operations for the revocation mechanism
- Data behind the wrapper may be freed by C code after revocation

#### 32. Page-Level Memory Management

The `pages` module provides abstractions for kernel page allocation and manipulation.

**Why unsafe:**
- Page allocation returns raw physical memory addresses
- Pages must be mapped before access, unmapped after use, freed exactly once
- Pages can be mapped into user space (creating aliasing)

---

### Cross-Cutting Concerns

#### 33. The Linux Kernel Memory Model (LKMM) vs. Rust Memory Model

The Linux kernel relies on a custom memory model (LKMM) that permits certain operations the C/C++ standard (and Rust) considers UB — for example, treating volatile accesses on certain types as atomic.

Certain lock-free patterns that are correct under LKMM are technically UB under Rust's model. The kernel community's solution is to use `unsafe` for these patterns and manually verify correctness against LKMM.

#### 34. No Standard Library (`#![no_std]`)

Kernel code cannot use Rust's standard library. Many safe abstractions depend on a working standard library. In the kernel, allocation can fail, so all collection types must be fallible — reimplementing core types with `unsafe` internals.

#### 35. Kernel Preemption and Scheduling Constraints

Certain code paths must not be preempted (spinlock critical sections, interrupt handlers, RCU read-side critical sections). Rust's type system cannot express "this function must not sleep" or "preemption must be disabled while this value exists."

---

## Part 3: Ori's Capability-Based Alternatives

### Ori's Existing Advantages

Ori's existing architecture provides the foundation for solving most kernel `unsafe` patterns:

1. **Capabilities are granular and trackable** — `uses Http, FileSystem` already declares exactly what effects a function performs. The compiler propagates these, verifies them, and allows mocking.

2. **Contracts provide proof obligations** — `pre()` and `post()` already let functions declare their invariants. Runtime-checked today but infrastructure exists for static verification later.

3. **The `Value` trait** — types marked `Value` are inline, bitwise-copyable, no ARC, no heap. This is the kernel-friendly subset of the type system.

4. **`Unsafe` is already a marker capability** — it propagates through the call chain and is discharged by `unsafe { }` blocks.

5. **`#repr("c")` and Deep FFI** — layout control, ownership annotations, error protocols, parametric FFI capability.

6. **`Sendable` auto-derived** — cannot be manually implemented, eliminating incorrect `Send`/`Sync` impls.

### Proposed Graduated Capability Decomposition

Instead of one `Unsafe` capability, decompose into specific low-level capabilities:

| Capability | What it gates | Proof obligation |
|---|---|---|
| `VolatileIO` | Memory-mapped register access | Address must be in declared range |
| `RawMemory` | Pointer arithmetic, raw allocation | Alignment, bounds, lifetime |
| `InlineAsm` | CPU instructions | Platform-specific |
| `StaticMut` | Mutable static state | Synchronization proof |
| `Transmute` | Type reinterpretation | Layout compatibility |
| `InterruptCtx` | Interrupt handler context | No allocation, no suspension |
| `DMA` | DMA buffer management | Cache coherency, alignment |
| `Allocator` | Custom memory allocation | Size/alignment invariants |
| `PerCpuAccess` | Per-CPU variable access | Preemption disabled |
| `RCU` | Read-Copy-Update operations | No sleeping in read side |
| `DeferredWork` | Workqueue/timer scheduling | Sendable closure |
| `LKMM` | Operations using Linux Kernel Memory Model | Acknowledges non-standard model |
| `Synchronization` | Lock/unlock operations | Matched acquire/release |
| `PageMgmt` | Page allocation and mapping | Alignment, map/unmap matching |

### Key Design Ideas

#### Typed Address Newtypes

Replace raw pointers with typed address newtypes:

```ori
type PhysAddr: Value, Eq, Comparable = int
type VirtAddr: Value, Eq, Comparable = int
type BusAddr: Value, Eq, Comparable = int
type UserPtr: Value = int

type MmioRegion: Value = { base: PhysAddr, size: Size }
type Register<T: Value>: Value = { offset: int }
```

Compiler prevents mixing address types. Contracts verify bounds.

#### Capability Composition Enforces Context Rules

```
InterruptCtx  -->  prohibits Allocator, Suspend
PerCpuAccess  -->  prohibits Suspend
RCU           -->  prohibits Suspend (in read-side)
```

These are compile-time checks, not documentation.

#### Scoped Resource Patterns (Lock Guards)

Ori's value semantics mean RAII guards don't work. Scoped APIs replace them:

```ori
@with_lock<T, R> (mutex: KMutex<T>, body: (T) -> R) -> R
    uses Synchronization
= { ... }
```

This is actually safer than RAII guards — impossible to forget the guard or hold it too long.

#### Deep FFI Already Handles Most C Interop

```ori
extern "c" from "kernel" #error(errno) {
    @krealloc (ptr: owned CPtr, size: c_size, flags: GfpFlags) -> owned CPtr
    @copy_from_user (to: mut [byte], from: UserPtr, n: c_size) -> c_size
}
```

No `unsafe` block needed. Error checking, ownership tracking, and `Result` wrapping are automatic.

#### `out` Parameters Eliminate Uninitialized Memory

```ori
extern "c" from "kernel" {
    @get_device_info (dev: borrowed CPtr, info: out DeviceInfo) -> c_int
}
// `out` parameter: compiler manages MaybeUninit internally
```

#### Kernel Compilation Profile

A subset of Ori for kernel development:
- All types must be `Value` (no ARC in hot paths)
- Allocation is fallible (returns `Result`)
- No standard library heap types
- `InterruptCtx` contexts enforce no-alloc, no-sleep

---

## Part 4: Scorecard — Can Ori Handle Each Category Safely?

> **Audit note (2026-03-22):** This scorecard was revised per the finding in Part 6. The original version presented the `SAFE` / `CAPABILITY-SAFE` ratings as validated architectural conclusions. They are not. Most mechanisms cited here are **proposed future work**, not capabilities that the current Ori spec or compiler supports. The "Evidence" column makes this explicit.

**Evidence key:**

| Code | Meaning |
|------|---------|
| **IN SPEC** | Mechanism exists in the current Ori spec (`docs/ori_lang/v2026/spec/`) or approved proposals |
| **PROPOSED** | Mechanism is defined only in this research note — requires new spec/proposal/implementation |
| **RUNTIME** | Safety relies on runtime `pre()`/`post()` contracts, not static proof |
| **UNRESOLVED** | No concrete mechanism designed yet |

| # | Category | Projected Status | Mechanism | Evidence |
|---|----------|-----------------|-----------|----------|
| 1 | Raw pointer dereference | **CAPABILITY-SAFE** | Typed address newtypes + bounds contracts | **PROPOSED** + **RUNTIME** — `PhysAddr`/`VirtAddr` not in spec; bounds are runtime contracts |
| 2 | Calling unsafe functions | **SAFE** | `uses FFI("lib")` + Deep FFI error protocols + ownership | **IN SPEC** — parametric FFI, `#error()`, `owned`/`borrowed` in approved Deep FFI proposal |
| 3 | Mutable statics | **CAPABILITY-SAFE** | `uses StaticMut` + synchronization contracts | **PROPOSED** + **RUNTIME** — `StaticMut` not in spec; synchronization proof is runtime contract |
| 4 | Unsafe trait impls | **SAFE** | `Sendable` auto-derived, capabilities replace unsafe traits | **IN SPEC** — `Sendable` auto-derived, manual impl forbidden (spec §8.14.2) |
| 5 | Union field access | **SAFE** | Sum types with `#repr("c")` | **IN SPEC** — sum types (spec §8.6.2), `#repr("c")` (spec §26.4.9) |
| 6 | FFI / C interop | **SAFE** | Deep FFI with error protocols, ownership annotations | **IN SPEC** — Deep FFI proposal approved |
| 7 | C helper functions | **SAFE** | Same Deep FFI mechanism | **IN SPEC** — same as #6 |
| 8 | MMIO | **CAPABILITY-SAFE** | `uses VolatileIO` + typed registers + bounds contracts | **PROPOSED** + **RUNTIME** — `VolatileIO`, `Register<T>`, `MmioRegion` not in spec |
| 9 | DMA | **CAPABILITY-SAFE** | `DmaBuffer<T>` newtype + `uses DMA` + Value+Sendable bound | **PROPOSED** — `DMA` capability and `DmaBuffer<T>` not in spec |
| 10 | Inline assembly | **CAPABILITY-SAFE** | `uses InlineAsm` + typed inputs/outputs | **PROPOSED** — `asm` keyword reserved (spec §7.3.2) but no design exists |
| 11 | Synchronization | **CAPABILITY-SAFE** | Scoped lock API + `uses Synchronization` | **PROPOSED** + **UNRESOLVED** — scoped lock API not designed; lock ordering unsolved |
| 12 | `Opaque<T>` | **SAFE** | `CPtr` is opaque by design | **IN SPEC** — `CPtr` defined (spec §26.4.4) |
| 13 | Reference counting | **SAFE** | Ori's native ARC + Deep FFI `#free` for C refcounts | **IN SPEC** — ARC (spec §21), `#free` in Deep FFI proposal |
| 14 | `ForeignOwnable` | **SAFE** | Deep FFI ownership + type-tracked `CPtr` round-trips | **IN SPEC** (partial) — `owned`/`borrowed` in Deep FFI; type-tracked round-trip is **PROPOSED** |
| 15 | Callback/vtable | **CAPABILITY-SAFE** | Trait-based vtable generation + `Sendable` bound | **PROPOSED** — no vtable-generation mechanism for C callbacks in spec |
| 16 | User-space pointers | **SAFE** | `UserPtr` newtype prevents mixing + bounds contracts | **PROPOSED** + **RUNTIME** — `UserPtr` not in spec; bounds are runtime contracts |
| 17 | `container_of!` | **SAFE** | Eliminated — index-based data structures, or FFI operation | **IN SPEC** (structural) — value semantics eliminate need; no intrusive containers |
| 18 | Pinned initialization | **SAFE** | Eliminated — `CPtr` handles point to C-allocated memory | **IN SPEC** (structural) — value semantics + `CPtr` eliminate the pin problem |
| 19 | Custom allocators | **CAPABILITY-SAFE** | `uses Allocator` + GFP flag tracking via capability | **PROPOSED** — `Allocator` as low-level capability not in spec; GFP tracking not designed |
| 20 | `Send`/`Sync` | **SAFE** | `Sendable` auto-derived, cannot be manual | **IN SPEC** — spec §8.14.2 |
| 21 | Interrupt handling | **CAPABILITY-SAFE** | `uses InterruptCtx` prohibits alloc/sleep at compile time | **PROPOSED** — `InterruptCtx` not in spec; **negative-effect system not designed** |
| 22 | RCU | **CAPABILITY-SAFE** | `uses RCU` prohibits sleeping in read-side | **PROPOSED** — `RCU` capability not in spec; negative-effect system required |
| 23 | Workqueue | **CAPABILITY-SAFE** | `uses DeferredWork` + `Sendable` closure | **PROPOSED** — `DeferredWork` not in spec |
| 24 | Per-CPU variables | **CAPABILITY-SAFE** | Scoped API + `uses PerCpuAccess` | **PROPOSED** — `PerCpuAccess` not in spec; scoped API not designed |
| 25 | Module init | **SAFE** | `@main` entry point with capabilities | **IN SPEC** — `@main` (spec §23.1), capability propagation (spec §20.7) |
| 26 | Transmutation | **RESIDUAL** | `uses Transmute` + size/alignment contracts (narrowed but not eliminated) | **PROPOSED** + **RUNTIME** — `Transmute` not in spec; contracts are runtime checks |
| 27 | Uninitialized memory | **SAFE** | Deep FFI `out` parameters handle this | **IN SPEC** — `out` in Deep FFI proposal |
| 28 | Lifetime circumvention | **SAFE** | Eliminated — value semantics, no lifetimes | **IN SPEC** (structural) — value semantics (spec §13.6, §21) |
| 29 | `#repr(C)` layout | **SAFE** | `#repr("c")` already exists | **IN SPEC** — spec §26.4.9 |
| 30 | Error code handling | **SAFE** | Deep FFI `#error(errno)` | **IN SPEC** — Deep FFI proposal |
| 31 | Revocable access | **CAPABILITY-SAFE** | Capability scoping + contracts | **PROPOSED** + **RUNTIME** — no revocable-access design exists |
| 32 | Page management | **CAPABILITY-SAFE** | `PhysAddr`/`VirtAddr` newtypes + `uses PageMgmt` | **PROPOSED** — `PageMgmt` not in spec |
| 33 | LKMM divergence | **RESIDUAL** | `uses LKMM` acknowledges different model | **UNRESOLVED** — no design for how LKMM interacts with Ori's memory model |
| 34 | No `std` | **SAFE** | Kernel compilation profile | **PROPOSED** — no kernel profile mechanism in spec |
| 35 | Preemption constraints | **CAPABILITY-SAFE** | Capability system enforces no-suspend, no-alloc | **PROPOSED** — requires negative-effect system not in spec |

### Summary

| Status | Count | Percentage |
|--------|-------|-----------|
| **SAFE** (fully eliminated) | 17 | 49% |
| **CAPABILITY-SAFE** (tracked + contracted) | 15 | 43% |
| **RESIDUAL** (programmer assertion needed) | 3 | 8% |

**However**, the evidence breakdown tells a different story:

| Evidence Level | Count | Categories |
|---------------|-------|-----------|
| **IN SPEC** (mechanism exists today) | 15 | #2, #4, #5, #6, #7, #12, #13, #17, #18, #20, #25, #27, #28, #29, #30 |
| **IN SPEC (partial)** | 1 | #14 |
| **PROPOSED** (requires new spec work) | 18 | #1, #3, #8, #9, #10, #11, #15, #16, #19, #21, #22, #23, #24, #26, #31, #32, #34, #35 |
| **UNRESOLVED** (no design exists) | 2 | #11 (lock ordering), #33 (LKMM) |
| **RUNTIME only** (contracts, not static proof) | 6 | #1, #3, #8, #16, #26, #31 (bounds/alignment/sync contracts are runtime `pre()`/`post()`) |

**Honest summary:** At the spec/proposal level, 15 categories have a mechanism in the current
Ori design surface, 18 require new capabilities, types, or enforcement mechanisms that exist only
as research directions in this note, and 2 still have no concrete design. 6 of the projected
ratings depend on runtime contracts rather than static proofs.

The projected scorecard (17 safe / 15 capability-safe / 3 residual) represents the **ceiling** —
what could be achieved if all proposed mechanisms are designed, specified, and implemented
correctly. The current **spec-level** floor is materially lower. The current **repository-level**
floor is lower again, because FFI, `Sendable`, stateful handlers, and AOT capability support are
not yet implemented end to end.

### Critical Missing Infrastructure

The largest gaps between the projected scorecard and the current Ori design surface:

1. **Negative-effect system** — The current capability model expresses *required* effects (`uses X`). Deep Safety requires *forbidden* effects (`InterruptCtx` prohibits `Allocator`). This is the single largest unresolved design question and affects categories #21, #22, #24, #35.

2. **Low-level capability taxonomy** — None of `VolatileIO`, `RawMemory`, `StaticMut`, `InterruptCtx`, `DMA`, `Allocator`, `PerCpuAccess`, `RCU`, `DeferredWork`, `LKMM`, `Synchronization`, `PageMgmt` exist in the spec. These need individual proposals.

3. **Typed address/pointer abstractions** — `PhysAddr`, `VirtAddr`, `BusAddr`, `UserPtr`, `MmioRegion`, `DmaBuffer<T>`, `Register<T>` are defined only here. Need stdlib design.

4. **Kernel compilation profile** — No mechanism to enforce "this module uses only Value types / fallible allocation / no ARC."

5. **Scoped resource APIs** — Lock guards, per-CPU access, RCU critical sections all need scoped API design that works with value semantics.

6. **Static contract verification** — Contracts are runtime checks today. Many scorecard ratings implicitly assume future static verification. This is a research-grade problem.

---

## Part 5: Design Questions — Provisional Design Directions

> **Status (2026-03-22):** The research now selects a provisional direction for each original
> design question. These are not yet validated compiler designs. The highest-risk areas remain:
> negative-effect inference at Ori scale, lock/context composability, and the interaction between
> LKMM-style concurrency patterns and Ori's ARC/memory model.

### 5.1 Lock Guard Pattern — RESOLVED

**Solution: Scoped APIs built on Ori's existing `with()` built-in.**

RAII guards require move semantics that Ori's value semantics don't provide. The scoped API pattern is actually *safer* than RAII — guaranteed release, no escape, reverse-order unwinding:

```ori
@with_lock<T, R> (mutex: Mutex<T>, body: (T) -> R) -> R
    uses Synchronization = {
    with(acquire: mutex.lock(), action: guard -> body(guard.data()), release: guard -> guard.unlock())
}
```

**Lock ordering**: Type-level lock levels via `LockBefore<L>` trait (from Fuchsia's `lock_ordering` crate), enforced at compile time. Multi-lock acquisition via `with_locks(a, b, body:)` that sorts by identity.

**Evidence**: Koka's effect handlers with `finally` clauses, Vault's "focus" pattern, Java's `synchronized(obj)`, and Ori's existing `with(acquire:, action:, release:)` all validate the scoped approach. Linux kernel code typically holds 1-3 locks; 96-deep limit exists for pathological paths only.

See `01-lock-and-zerocopy-research.md` for full analysis.

### 5.2 Zero-Copy Without Lifetimes — RESOLVED

**Solution: Three-layer approach — seamless slices + callback-scoped views + second-class borrows.**

1. **Seamless slices** (existing): Application-level zero-copy for strings and lists. Works today.
2. **Callback-scoped views**: For DMA, MMIO, mmap. View types cannot escape the callback scope. Built on `with()`. Enforced by: view type not implementing `Clone`, callback return type constrained to `Value`.
3. **Second-class borrowed parameters** (future): `view T` parameter mode — no RC inc/dec, cannot be stored or returned. Modeled after Lean 4's `@&`, Hylo's `let` subscript, Swift's `borrowing`.

**What is NOT achievable without lifetimes**: Returning views from functions, storing views in struct fields. These are rare in kernel code and can use owned copies at the boundary.

**Evidence**: Hylo/Val's mutable value semantics (Racordon et al., 2022) proves value semantics + zero-copy is achievable with second-class references. All kernel zero-copy patterns (DMA, SKB, MMIO, mmap) follow acquire-use-release, which maps directly to callback scoping.

See `01-lock-and-zerocopy-research.md` for full analysis.

### 5.3 LKMM Divergence — PARTIALLY RESOLVED

**Solution: `uses LKMM` as an explicitly-residual capability that acknowledges non-standard memory model semantics.**

The Linux Kernel Memory Model permits operations that are UB under standard memory models (e.g., treating volatile accesses as atomic). `uses LKMM` marks code that relies on these semantics. This is one of the three **residual** categories — Ori cannot verify LKMM correctness, only track that the code operates under non-standard rules.

**Open work**: The interaction between `LKMM` and Ori's ARC atomicity guarantees (spec §21.2.1) needs formal analysis. LKMM's `READ_ONCE`/`WRITE_ONCE` patterns need explicit Ori equivalents.

### 5.4 Capability Granularity — RESOLVED

**Solution: 14 low-level capabilities organized into 4 tiers, validated against 12 failure case studies.**

The Java checked exceptions failure (ICFP/OOPSLA studies: 2000+ catch-throw blocks per project, every post-Java JVM language rejected them) establishes the critical constraint: **capabilities must compose without viral propagation**. The Pony failure (6 rcaps + consume + recover = 8 concepts, learning cliff killed adoption) establishes: **max 3-4 concepts per domain**.

The proposed 14 capabilities group into 4 domains a developer encounters one at a time:
- **Memory** (3): `VolatileIO`, `RawMemory`, `DMA`
- **Context** (3): `InterruptCtx`, `PerCpuAccess`, `RCU`
- **Resources** (4): `Allocator`, `Synchronization`, `DeferredWork`, `PageMgmt`
- **Low-level** (3): `InlineAsm`, `StaticMut`, `Transmute`
- **Model** (1): `LKMM`

A typical driver function uses 1-2 capabilities. The existing `Unsafe` remains as the blanket escape hatch for truly unclassifiable operations.

**Evidence**: Swift SE-0458's 13 unsafe categories received "strongly positive" community reception. SPARK/Ada's 5-level graduated verification has 25+ years of production evidence that graduated models work. The 12 failure studies confirm the annotation burden must stay <1% of code.

See `failed-approaches.md` for the 12 design principles extracted from failures.

### 5.5 Capability Discharge — RESOLVED

**Solution: Three discharge mechanisms, matching capability type.**

| Capability Type | Discharge Mechanism | Example |
|----------------|-------------------|---------|
| **Environmental** (normal) | `with Cap = impl in expr` | `with Http = MockHttp in test()` |
| **Marker** (track-only) | The operation itself | `unsafe { ptr_read(...) }` |
| **Contextual** (negative) | Context exit | Exiting interrupt handler removes `InterruptCtx` denial |

The `without` clause (see Part 10) introduces a fourth concept: capabilities that are *denied* cannot be discharged within the denial scope. `with Allocator = impl in expr` inside a `without Allocator` context is a compile error.

### 5.6 Kernel Compilation Profile — RESOLVED

**Solution: Capability-based enforcement, not a separate compilation mode.**

Instead of a special "kernel mode," capabilities naturally restrict what code can do:
- `Value` trait bound on all types → no ARC in hot paths
- `uses Allocator` marks functions that allocate → `without Allocator` contexts enforce no-alloc
- Fallible allocation via `Result<T, AllocError>` return types
- `without Suspend` → no sleeping, no yielding

**Evidence**: Asterinas's OSTD framework achieves this through API design, not compiler modes. The "kernel profile" is simply code that operates under specific capability constraints, not a separate language subset.

### 5.7 ARC in Kernel Context — RESOLVED

**Solution: `Value` trait + capability denial + DMA-specific types.**

- **Interrupt handlers and DMA paths**: All types must be `Value` (inline, zero-ARC, bitwise-copyable). Enforced by `without Allocator` which transitively prohibits ARC operations.
- **Complex kernel objects**: Use ARC normally for driver state, configuration, file descriptors.
- **DMA buffers**: `DmaBuffer<T>` newtype where `T: Value` — guaranteed no ARC inside the buffer. Constructor contracts enforce alignment and DMA-safe memory source.

**Evidence**: Ori's existing `Value` trait (spec §8.14.3) already provides the right semantic — "inline storage, bitwise copy, no ARC, no Drop." The capability system ensures that Value-only contexts are enforced, not just documented.

---

## Part 6: Third-Party Audit

This research is ambitious enough that internal review is not sufficient. If Ori is going to claim that it can make low-level and kernel-adjacent programming *safer than Rust's binary `unsafe` model*, that claim should be tested by people who are not invested in Ori's design premises.

The right standard is not "does the idea sound plausible?" but:

- Does the design actually reduce the trusted computing base?
- Are the claimed guarantees mechanically enforceable by the compiler?
- Where the compiler cannot prove safety, is the residual trust boundary narrow and explicit?
- Does the model survive hostile review from kernel, compiler, memory-model, and FFI experts?

### Why External Audit Is Necessary

This plan crosses several domains where language designers routinely overestimate what a type system can guarantee:

- FFI ownership and marshalling
- raw memory and MMIO access
- interrupt and scheduling constraints
- DMA and cache coherency
- lock-free concurrency and memory models
- kernel-specific execution rules that diverge from standard language models

These are exactly the areas where "looks principled" often collapses under adversarial scrutiny.

### Audit Goals

A third-party audit should answer the following questions:

1. **Capability decomposition validity** — Is the proposed split (`VolatileIO`, `RawMemory`, `InlineAsm`, `StaticMut`, `Transmute`, `InterruptCtx`, `DMA`, `Allocator`, `PerCpuAccess`, `RCU`, `DeferredWork`, `LKMM`, `PageMgmt`, etc.) the right granularity, or does it merely rename one large `unsafe` bucket into several smaller but still under-specified ones?

2. **Proof boundary honesty** — For each category marked **SAFE** or **CAPABILITY-SAFE**, what is actually:
   - statically proven,
   - runtime-checked via contracts,
   - delegated to code review,
   - delegated to external tooling,
   - still effectively a programmer assertion?

3. **Contract sufficiency** — Are `pre()` / `post()` conditions strong enough to express the safety obligations being claimed, especially for:
   - bounds and alignment,
   - lock ordering,
   - no-sleep / no-alloc contexts,
   - callback validity,
   - DMA coherency,
   - memory ordering constraints?

4. **Memory-model soundness** — Is `uses LKMM` a coherent extension point, or does Linux-kernel-style memory ordering require semantics that fundamentally conflict with the rest of Ori's model?

5. **Zero-copy feasibility** — Can Ori support the necessary non-owning views for kernel and driver work without importing a lifetime system through the back door?

6. **Value-semantics interaction** — Do scoped APIs such as `with_lock(...)` and capability-gated low-level operations compose cleanly with Ori's value semantics, ARC rules, and `Value` trait?

7. **Trusted computing base reduction** — Compared to Rust-for-Linux, is the set of operations requiring unchecked trust actually smaller, or merely redistributed?

### Required Auditor Profiles

This should not be a single generalist review. It needs at least four perspectives:

- **Kernel engineer** — somebody with direct Linux kernel, driver, interrupt, DMA, and RCU experience.
- **Programming languages / type systems expert** — somebody who can distinguish "good API design" from actual static guarantees.
- **Memory-model / concurrency expert** — especially for LKMM, atomics, RCU, interrupt/preemption constraints, and lock-free claims.
- **FFI / systems safety reviewer** — somebody who has designed or reviewed ownership-aware FFI systems and can attack the Deep FFI assumptions.

Ideally, at least one reviewer should be skeptical of the thesis on entry. A friendly reviewer is useful; a hostile but technically serious reviewer is more valuable.

### Audit Inputs

An external audit should not review only this research note. It should review the full connected design surface:

- `plans/deep-safety/00-overview.md`
- `plans/deep-safety/research.md`
- `docs/ori_lang/v2026/spec/20-capabilities.md`
- `docs/ori_lang/v2026/spec/21-memory-model.md`
- `docs/ori_lang/v2026/spec/26-ffi.md`
- `docs/ori_lang/proposals/approved/deep-ffi-proposal.md`
- `docs/ori_lang/proposals/approved/unsafe-semantics-proposal.md`

If a prototype exists, the audit should review both the spec/design text and the implementation. Claims about "compiler-checked" behavior should not be accepted without checking how the compiler actually enforces them.

### Claims That Must Be Explicitly Challenged

The following claims are the highest-risk and should be attacked directly:

- **"32 out of 35 categories can be eliminated or converted to compiler-verified capabilities."**
  This is the headline claim in this document. Auditors should require category-by-category evidence and downgrade any item that depends primarily on review discipline rather than compiler enforcement.

- **"Contracts replace `// SAFETY:` comments."**
  Auditors should distinguish between runtime-checked contracts and statically discharged proof obligations. These are not equivalent.

- **"Typed newtypes replace raw pointers."**
  Auditors should test whether typed address wrappers genuinely reduce misuse or merely wrap integers without sufficient provenance, lifetime, aliasing, and mapping guarantees.

- **"`InterruptCtx` / `PerCpuAccess` / `RCU` can prohibit invalid operations at compile time."**
  Auditors should inspect whether the capability system can express *forbidden* effects, not just required ones.

- **"Kernel profile: no-heap, no-ARC subset."**
  Auditors should require a concrete enforcement mechanism, not just a guideline or linter.

- **"`uses LKMM` can model Linux-kernel memory behavior coherently."**
  Auditors should assume this is false until shown otherwise.

### Expected Failure Modes

The audit should specifically look for the following failure modes:

- Capabilities that are too coarse, so one capability silently smuggles multiple trust assumptions.
- Capabilities that are too fine, producing annotation noise without meaningful verification gain.
- "Static" claims that are really dynamic checks.
- Safety obligations that are expressed in prose but not in types, contracts, or compiler rules.
- Hidden reintroduction of lifetimes/borrows without admitting it.
- Areas where Ori's existing semantics (value capture, ARC, no shared mutable references) conflict with kernel realities.
- Places where the design relies on wrappers that are themselves effectively `unsafe` islands with no meaningful reduction in trust.

### Audit Deliverables

The external review should produce:

1. **Category-by-category scorecard** — each of the 35 categories rated as:
   - compiler-proven,
   - runtime-checked,
   - review-only,
   - unresolved,
   - impossible under current model.

2. **Trusted boundary map** — a concrete map of which operations still require programmer assertion.

3. **Semantic contradiction list** — any places where the plan conflicts with the existing Ori spec or with itself.

4. **Implementation prerequisites** — what must exist in the compiler/runtime before the safety claim is credible.

5. **No-go findings** — any categories where the current thesis should be withdrawn or substantially narrowed.

### Recommended Audit Timing

The audit should happen in two passes:

- **Pass 1: pre-implementation design audit**
  Goal: challenge the core thesis before large implementation cost is sunk.

- **Pass 2: post-prototype implementation audit**
  Goal: verify that the compiler and runtime actually enforce the promised boundaries.

The first pass is mandatory. Without it, there is a serious risk of building a large amount of infrastructure around claims that do not survive expert scrutiny.

### Potential Proof of Concept

This research needs a concrete proof artifact. Without one, the thesis remains architectural speculation.

The strongest first proof is **not** immediate support for physical NIC hardware. It is a **VM NIC driver port** that exercises the same low-level boundaries in a controlled environment.

#### Recommended Demo Target

- **Primary target:** a small VM NIC driver such as `virtio-net` in QEMU
- **Fallback target:** a similarly well-understood virtual device such as `e1000`
- **Do not start with:** a full production Linux driver port or hardware-specific bring-up on physical NICs

#### Porting Strategy

The right approach is to port the **core logic** of a small, existing driver design, not an entire existing driver stack.

- Reuse a pre-existing driver's queue layout, descriptor handling, register protocol, and interrupt/deferred-work flow as the behavioral reference
- Rebuild the boundary layer in Ori around explicit capabilities, typed addresses, contracts, and Deep FFI where needed
- Keep a direct mapping from each source-driver `unsafe` boundary to the corresponding Ori capability or checked abstraction

This keeps the proof grounded in real device logic while still testing Ori's own safety model instead of Linux framework glue.

#### What the Proof Must Demonstrate

- MMIO register access through typed register and address abstractions
- DMA descriptor-ring setup and update paths
- Interrupt handler constraints plus deferred work / bottom-half style processing
- FFI interaction where required without letting blanket `Unsafe` infect the public driver-facing API
- Compile-fail examples showing that invalid low-level operations are rejected when the model claims compile-time enforcement

#### Minimum Success Criteria

- The VM boots and the device initializes through the Ori driver
- The driver can successfully transmit and receive packets in the VM environment
- The proof includes adversarial negative cases:
  - invalid register-range access
  - interrupt-context allocation or suspension
  - invalid DMA buffer/alignment usage
  - misuse of typed address spaces
- The proof clearly distinguishes:
  - statically enforced guarantees
  - runtime-checked contracts
  - residual trusted boundaries

#### Follow-On Stage

If the VM NIC proof succeeds, a later second-stage demo on real hardware becomes credible. Physical hardware should validate that the model survives real timing, platform, and device-quirk pressure. It should not be the first proof obligation.

### Bottom Line

If this project wants to claim that Ori can be a better low-level safety story than Rust, it should invite the hardest external review it can get.

Anything less turns "deep safety" into branding.

### Third-Party Audit Findings

#### 2026-03-22 — HIGH — Scorecard overstates what the current Ori design surface actually proves

**Finding.** Part 4 currently presents the `SAFE` / `CAPABILITY-SAFE` scorecard as if the cited mechanisms are already grounded in the referenced Ori spec and approved proposals. That is not true today. The document's headline claim — "32 out of 35 categories can be eliminated or converted to compiler-verified capabilities" — is materially ahead of the current design surface.

**Why this is a finding.**

- The scorecard relies on capabilities and kernel-facing types that do not appear in the cited spec/proposal inputs at all: `VolatileIO`, `RawMemory`, `StaticMut`, `InterruptCtx`, `DMA`, `Allocator`, `PerCpuAccess`, `RCU`, `DeferredWork`, `LKMM`, `Synchronization`, `PageMgmt`, `PhysAddr`, `VirtAddr`, `BusAddr`, `UserPtr`, and `MmioRegion` are defined only in this research note, not in `docs/ori_lang/v2026/spec/` or the approved proposals listed in Part 6.
- The current capability spec only standardizes `Http`, `FileSystem`, `Cache`, `Clock`, `Random`, `Crypto`, `Print`, `Logger`, `Env`, `Intrinsics`, `FFI`, `Suspend`, and `Unsafe` ([docs/ori_lang/v2026/spec/20-capabilities.md](/home/eric/projects/ori_lang/docs/ori_lang/v2026/spec/20-capabilities.md#L269)).
- The current marker-capability model says marker capabilities "track what the code does" and propagate or discharge, but it does not define a negative-effect system capable of expressing claims like "`InterruptCtx` prohibits `Allocator` and `Suspend`" or "`RCU` prohibits sleeping" ([docs/ori_lang/v2026/spec/20-capabilities.md](/home/eric/projects/ori_lang/docs/ori_lang/v2026/spec/20-capabilities.md#L491)).
- Contracts are explicitly runtime checks, not static proofs ([docs/ori_lang/v2026/spec/15-patterns.md](/home/eric/projects/ori_lang/docs/ori_lang/v2026/spec/15-patterns.md#L63), [docs/ori_lang/v2026/spec/03-terms-and-definitions.md](/home/eric/projects/ori_lang/docs/ori_lang/v2026/spec/03-terms-and-definitions.md#L242)). That means claims such as "compiler-verified capabilities" and "contracts verify bounds/alignment" are overstated unless the document distinguishes runtime checking from static enforcement category by category.
- The current FFI spec still centers `Unsafe` as the active trust boundary for raw pointer dereference, pointer arithmetic, mutable statics, and transmute ([docs/ori_lang/v2026/spec/26-ffi.md](/home/eric/projects/ori_lang/docs/ori_lang/v2026/spec/26-ffi.md#L312)). The research note's decomposition in [plans/deep-safety/research.md](/home/eric/projects/ori_lang/plans/deep-safety/research.md#L364) is therefore a proposed future replacement, not something the current Ori model already substantiates.

**Impact.** As written, the scorecard reads like a validated architectural conclusion, but the evidence currently supports only a research thesis plus a list of required future mechanisms. That weakens the document's honesty at exactly the point where Part 6 says auditors should be most skeptical.

**Required change.** Downgrade the Part 4 scorecard from "what Ori can handle safely" to "what could become safe if new capabilities, kernel profile rules, and enforcement semantics are added", or add a second evidence column that distinguishes:

- already supported by current spec/proposals,
- plausible but unimplemented proposal work,
- runtime-checked only,
- unresolved / likely impossible under the current model.

**Resolution (2026-03-22).** Evidence column added to Part 4 scorecard. See revised table and honest summary.

---

## Part 7: Prior Art — Languages with Graduated Safety

No existing mainstream language does what Ori is proposing. The combination of value semantics + effect/capability tracking + low-level/kernel programming + narrower replacement for blanket `unsafe` is novel. However, the individual pieces have strong precedents.

### Primary References

#### Koka — Effect Typing and Handlers

**What it does:** Row-polymorphic effect types. Every function signature includes an effect row (e.g., `fun f() : <exn,io> int`) that tracks which effects it performs. Effects compose via row extension; handlers interpret effects. The type system distinguishes `total` (pure), `div` (divergent), `exn` (exceptions), `io` (full I/O), and user-defined effects.

**Systems relevance:** Koka's FIP/FBIP system (Fully In-Place / Fully But-In-Place) tracks allocation budgets statically:
- `fip` functions must prove zero net allocation
- `fbip` allows bounded allocation with borrowing
- `AllocAtMost Int | AllocFinitely | AllocUnlimited` — graduated allocation tracking

This is the closest any effect system comes to systems-level resource tracking. Koka also uses Perceus (optimized RC), which is architecturally identical to Ori's ARC pipeline.

**Limitation:** Koka's effect system tracks *functional* effects (state, exceptions, I/O), not *memory-safety* concerns. You cannot write `uses RawPointer` in Koka. The language deliberately avoids exposing unsafe memory operations. For kernel code, Koka would need a C FFI layer that sidesteps the effect system.

**What Ori should study:** Effect inference (Koka infers effects without annotation in most cases), effect polymorphism (generic functions inherit their arguments' effects), and the FIP/FBIP graduated allocation discipline.

**Source:** [Koka docs](https://koka-lang.github.io/koka/doc/book.html), `~/projects/reference_repos/lang_repos/koka/src/Core/CheckFBIP.hs`

#### F\* / Low\* / KaRaMeL / Vale — Verified Low-Level Code

**What it does:** Dependent types + multi-monadic effects. Programmers choose effect granularity; the compiler computes weakest preconditions via predicate transformers and discharges proof obligations using Z3 (SMT solver). Low\* is an embedded DSL for C-like code within F\* — no GC, structured memory model, compiles to C via KaRaMeL. Microsoft's Vale verifies assembly code with the same proof infrastructure.

**Systems relevance:** This is the strongest existing precedent for "unsafe territory, but with proof obligations instead of trust alone." Real-world output includes:
- HACL\* — verified crypto library used in Firefox, Linux kernel, and other systems
- EverCrypt — verified TLS 1.2/1.3 (Project Everest)
- Vale — verified high-performance assembly (AES-GCM, SHA-256, Poly1305)

Low\* programs generate C that is as fast as hand-written C. The proof overhead is front-loaded at development time and erased at runtime.

**Limitation:** Writing Low\* requires significant expertise. The SMT solver sometimes times out or produces unpredictable results. The cost is justified for high-assurance crypto and protocol code but impractical for application-level development.

**What Ori should study:** The pre/post contract model (F\*'s `requires`/`ensures` is what Ori's `pre()`/`post()` aspires to, but with static rather than dynamic checking), the graduated effect model (F\* programmers choose which effects to track), and KaRaMeL's approach to extracting verified low-level code to C.

**Source:** [F\*](https://fstar-lang.org/), [Low\* tutorial](https://fstar-lang.org/tutorial/old/tutorial.html), [Vale](https://www.microsoft.com/en-us/research/publication/vale-verifying-high-performance-cryptographic-assembly-code/)

#### ATS — Dependent Types for Systems Programming

**What it does:** Two-layer language: a *proof language* (inductive proofs encoded as total recursive functions, erased before execution) and a *programming language* (effectful, compiled to C). Proofs verify properties like pointer validity, array bounds, and reference count correctness *before* the program runs.

**Systems relevance:** ATS was used to implement parts of the Terrier RTOS (scheduler, memory management). It achieves C-level performance because proofs are erased. ATS2 was used to provide a fix for the Heartbleed bug. Dependent types can encode array length, pointer validity, resource state machines — exactly the invariants Ori's `pre()`/`post()` contracts express dynamically.

**Limitation:** Extremely steep learning curve. Fewer than 200 GitHub repositories contain ATS code. Error messages are notoriously opaque. The proof burden is too high for mainstream developers. ATS proves that dependent types *can* provide fine-grained safety for systems code, but at a DX cost that prevents adoption.

**What Ori should study:** The concept of proof erasure (proofs cost nothing at runtime), the encoding of low-level invariants in the type system (array bounds, pointer validity), and the lesson that *annotation burden kills adoption*. Ori's inference-first approach is the correct response to ATS's failure mode.

**Source:** [ATS overview](https://ats-lang.sourceforge.net/), [dependent types intro](https://ats-lang.sourceforge.net/DOCUMENT/INT2PROGINATS/HTML/c2243.html)

#### Pony — Deny Capabilities for Aliasing and Sharing

**What it does:** Reference capabilities (rcaps) based on *deny* properties. Six capability annotations: `iso` (isolated), `val` (immutable), `ref` (mutable), `box` (read-only), `trn` (transitional), `tag` (identity only). The key insight: capabilities describe what you *cannot* do with a reference, not what you can.

**Systems relevance:** The actor model + deny capabilities eliminate data races *at compile time* with zero runtime overhead. Pony compiles AOT to native code. The rcap system handles the most common source of systems bugs (data races) but does not address other unsafe operations (raw pointer arithmetic, transmutes).

**Limitation:** Six-capability system has a steep learning curve. Actor-model-only concurrency limits appeal for tight computational kernels. Limited adoption outside research and specialized financial systems.

**What Ori should study:** The deny-based mental model (capabilities express what is *forbidden*, not what is *permitted*). This is directly relevant to Ori's negative-effect problem — `InterruptCtx` needs to *deny* `Allocator` and `Suspend`. Pony proves this can work at the type level.

**Source:** [Pony reference capabilities](https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html), [Deny Capabilities paper](https://www.ponylang.io/media/papers/fast-cheap.pdf)

#### Austral — Linear Capabilities as Values

**What it does:** Linear types + capability-based security. Capabilities are linear values that represent permissions. To open a file, you need a `Filesystem` capability value; to allocate memory, you need a `Memory` capability value. Because capabilities are linear, they cannot be duplicated — you must explicitly pass them.

**Systems relevance:** Designed explicitly for systems programming. Linear types provide memory safety without GC. The capability system constrains what third-party code can do — code that does not receive a capability *literally cannot* perform the operation.

**Limitation:** Single-person project with limited ecosystem. Strict linearity makes some common patterns awkward. No `unsafe` escape hatch documented.

**What Ori should study:** Capabilities as *authority* (you can do X because you hold the capability value, not because the compiler says so). Austral's approach provides stronger supply-chain guarantees than Ori's annotation-based system. However, Ori's annotation approach (`uses X`) is less invasive than passing capability values through every call chain.

**Source:** [Austral spec](https://austral-lang.org/spec/spec.html), [Austral capability-based security](https://austral-lang.org/tutorial/capability-based-security)

#### SPARK/Ada — Graduated Proof Levels

**What it does:** Five levels of verification assurance, each building on the previous:

| Level | Name | What it proves |
|-------|------|---------------|
| 1 | Stone | Valid SPARK subset (no pointers, no dynamic dispatch, no goto) |
| 2 | Bronze | No uninitialized reads, no problematic aliasing |
| 3 | Silver | No runtime errors (no division by zero, no buffer overflow, no integer overflow) |
| 4 | Gold | Security-relevant properties (information flow, access control invariants) |
| 5 | Platinum | Full functional correctness (implementation matches spec) |

Each level is cumulative. Teams adopt incrementally. Stone and Bronze are "easy"; Silver requires modest effort; Gold/Platinum require proof expertise.

**Systems relevance:** Proven in production: Muen separation kernel, EwoK microkernel (~10K lines Ada, ~500 lines C/ASM), NVIDIA GPU firmware, Thales avionics systems.

**Limitation:** SPARK is a *subset* of Ada — dynamic dispatch, heap allocation, and access types are excluded. The GNATprove tool provides incremental feedback but requires Ada expertise.

**What Ori should study:** The graduated adoption model is SPARK's killer feature. Ori could define:
- **Level 0** (normal): Type-safe, ARC-managed, capability-tracked.
- **Level 1** (contracted): `pre()`/`post()` contracts present and runtime-checked.
- **Level 2** (verified): Contracts statically proven (requires SMT integration — future).
- **Level 3** (certified): Full functional correctness proofs (far future).

This lets teams adopt Deep Safety incrementally without requiring formal verification from day one.

**Source:** [SPARK overview](https://learn.adacore.com/courses/intro-to-spark/chapters/01_Overview.html), [SPARK contract guidance](https://learn.adacore.com/courses/Guidelines_for_Safe_and_Secure_Ada_SPARK/chapters/guidelines/robust_programming_practice/rpp09_use_precondition_and_postcondition_contracts.html)

### Secondary References

#### Swift SE-0458 — Fine-Grained Unsafe Tracking

Swift now tracks **13 distinct categories** of unsafe use (not binary safe/unsafe): `Override`, `Witness`, `TypeWitness`, `UnsafeConformance`, `UnownedUnsafe`, `ExclusivityUnchecked`, `NonisolatedUnsafe`, `ReferenceToUnsafe`, `ReferenceToUnsafeStorage`, `ReferenceToUnsafeThroughTypealias`, `CallToUnsafe`, `CallArgument`, `PreconcurrencyImport`. The `@unsafe` attribute marks declarations; the `unsafe` expression keyword marks usage sites (like `try`/`await`).

**Relevance:** Validates fine-grained unsafe categorization in a mainstream language. Ori could adopt similar granularity within its capability decomposition.

**Source:** [Swift SE-0458](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0458-strict-memory-safety.md), `~/projects/reference_repos/lang_repos/swift/include/swift/AST/UnsafeUse.h`

#### D Language — `@safe` / `@trusted` / `@system`

Three-level safety annotations:
- `@safe`: Memory-safe subset. Cannot use raw pointers, cannot call `@system` functions.
- `@trusted`: Safe interface wrapping unsafe internals. Programmer asserts correctness.
- `@system` (default): No restrictions.

**Cautionary lesson:** `@system` is the *default*, so most D code is unsafe. Wrong default killed adoption of the safety system. Ori correctly defaults to safe.

**Source:** [D memory-safe spec](https://dlang.org/spec/memory-safe-d.html)

#### Scala 3 Capture Checking

Compile-time tracking of which capabilities a value *captures*. Types annotated with capture sets: `(x: Tcp^{io}) -> Unit^{x}` means the function captures the `io` capability through `x`. Can express both positive (needs X) and negative (must not capture X) constraints.

**Relevance:** Goes further than Ori currently does — tracks capabilities through closures and generic data structures. Relevant for Ori's `with...in` scoping: if a closure escapes a scope, did it capture a capability it shouldn't have?

**Source:** [Scala 3 capture checking](https://docs.scala-lang.org/scala3/reference/experimental/cc.html)

#### Lean 4 — Verified ARC

Lean 4's RC insertion pass (`~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/RC.lean`) is architecturally identical to Ori's `ori_arc` pipeline. Both track parent-child derived values, borrowed parameters, and insert inc/dec instructions. Lean 4's `ExpandResetReuse.lean` corresponds to Ori's reset/reuse optimization. The key difference: Lean 4 *proves* RC correctness; Ori *tests* it.

**Relevance:** Demonstrates that Ori's ARC model can in principle be formally verified.

**Source:** [Lean 4](https://lean-lang.org/), `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/`

#### Vault (Microsoft Research, 2002) — Adoption and Focus

Two constructs bridging linear and non-linear types: *adoption* (alias a linear object by transferring responsibility to a parent) and *focus* (temporarily recover a linear view of an adopted object for the duration of a scope). This directly influenced Rust's borrow checker design.

**Relevance:** Vault's scoped focus pattern is conceptually similar to Ori's scoped resource APIs (`with_lock(mutex, body:)`). Both provide temporary exclusive access without full linear typing.

**Source:** [Vault paper](https://www.microsoft.com/en-us/research/publication/adoption-and-focus-practical-linear-types-for-imperative-programming/)

#### Dafny — Accessible Verification

Pre/postconditions with Z3-based static verification integrated into the compiler. More accessible than F\*. Failed proofs produce localized error messages.

**Relevance:** Ori's `pre()`/`post()` contracts are syntactically similar to Dafny's `requires`/`ensures`. Bridging the gap from runtime contracts to static verification would require SMT integration — Dafny shows this can be accessible.

**Source:** [Dafny](https://dafny.org/)

#### Liquid Haskell — Refinement Types

Ordinary Haskell types annotated with logical predicates (e.g., `{v:Int | v > 0}`). Z3 checks refinements at every use site. No runtime overhead.

**Relevance:** Refinement types could augment Ori's const generic bounds. Currently Ori supports `where N > 0` on const generics. Refinement types would extend this to runtime values: `@safe_index (list: [T], i: {v: int | v >= 0 && v < len(list)}) -> T` — proving array bounds statically.

**Source:** [Liquid Haskell tutorial](https://ucsd-progsys.github.io/liquidhaskell-tutorial/book.pdf)

### What Failed and Why — Quantitative Lessons for Ori

> **See `failed-approaches.md` for the full 925-line analysis of 12 failed/troubled approaches.**

| Language | What Failed | Quantitative Data | Lesson |
|----------|------------|-------------------|--------|
| Cyclone | Fat pointers, not annotation burden (~0.5% annot.) | LIFO region limits, no 64-bit support | Fat metadata kills layout compat; inference CAN keep burden low |
| ATS | Proof burden + dual mastery requirement | <200 GitHub repos; 1:1 to 2:1 proof ratio | Proofs must be optional, never required for common ops |
| CCured | Fat pointers (3 words) broke binary compat | 0-150% perf overhead; no separate compilation | Safety metadata must NOT change data layout |
| Java checked exceptions | Viral propagation, no polymorphism | 2000+ catch-throw blocks/project; 600+ coding errors/project | **THE #1 cautionary tale**: capabilities MUST compose without viral propagation |
| D `@safe` | Wrong default (`@system` = unsafe) | DIP1028 accepted then reversed; ecosystem can't migrate | Safe MUST be the default (Ori: correct) |
| Safe Haskell | Module-level granularity too coarse | Template Haskell bypasses entirely; rarely used in practice | Function-level tracking, not module-level |
| Pony | 6 rcaps + consume + recover = 8 concepts | Wallaroo (biggest user) migrated to Rust citing ecosystem | Max 3-4 safety concepts per domain |
| Vault | Linear types are infectious (upwardly viral) | Containing a linear field makes the container linear | Linearity must be opt-in scoped, not structural |
| Sing#/Singularity | Required rebuilding entire software stack | 4.5% overhead; >90% kernel safe — **WORKED technically** | Safety IS achievable; failure was ecosystem, not tech |
| Rust unsafe | Binary safe/unsafe too coarse | 29% of crates use unsafe; Rudra found 264 bugs in 6.5h | Graduated unsafe prevents both noise and escape |
| OCaml 5 effects | Shipped WITHOUT type tracking | Runtime-only; no production fully-typed effect system exists | Effect tracking MUST be compile-time enforced |
| E/Joe-E/Caja | Subsetting existing languages is intractable | Caja bypassed repeatedly via Unicode escapes, DOM clobbering | Build safety in from the start, don't retrofit |

### 12 Universal Design Principles (from failure analysis)

| # | Principle | Violated By | Ori Status |
|---|-----------|-------------|------------|
| 1 | Safe must be the default | D, Safe Haskell | **Correct** |
| 2 | Annotation burden <1% of code | Java (2000+ blocks), ATS | **Correct** (inference-first) |
| 3 | Safety metadata must not change data layout | CCured, Cyclone | **Correct** (`CPtr` opaque, capabilities are erased) |
| 4 | Escape hatches must be verifiable | D (`@trusted`), Rust (`// SAFETY:`) | **Correct** (`pre()`/`post()` are executable) |
| 5 | Capabilities must compose without viral propagation | Java checked exceptions, Vault | **CRITICAL RISK** — must validate |
| 6 | Function-level tracking, not module-level | Safe Haskell | **Correct** (`uses` per function) |
| 7 | Max 3-4 safety concepts per domain | Pony (8 concepts) | **Correct** (14 caps in 4 domains) |
| 8 | Must work with separate compilation | CCured | **Correct** (Salsa-based) |
| 9 | Must interop with existing ecosystems | Singularity, E | **Correct** (C FFI via Deep FFI) |
| 10 | Runtime safety overhead: target <5% | Singularity (4.5%), CCured (0-150%) | **TBD** |
| 11 | Effect tracking must be compile-time | OCaml 5 (runtime-only) | **Correct** (`uses` is compile-time) |
| 12 | Never require formal proofs for common ops | ATS, Vault | **Correct** (contracts are optional) |

**Principle #5 is the highest risk for Ori.** If adding a capability to a library function breaks all callers, developers will write `uses Unsafe` everywhere. The `with...in` discharge mechanism is the defense — intermediate callers don't accumulate capabilities they don't use.

### Architectural Mapping to Ori

| Concern | Primary reference | Secondary | Ori mechanism |
|---------|------------------|-----------|---------------|
| Effect/capability propagation | **Koka** | Scala 3 | `uses` clauses, capability variance |
| Real low-level verification | **F\*/Low\*/Vale** | SPARK/Ada | `pre()`/`post()` contracts (runtime today, static future) |
| Type-level low-level invariants | **ATS** | Liquid Haskell | Typed newtypes, const generics, const bounds |
| Capability restrictions on aliasing | **Pony** | Austral | Negative-effect system (PROPOSED) |
| Graduated safety levels | **SPARK/Ada** | Swift SE-0458, D | Capability decomposition + contract levels |
| Value semantics + ARC | **Lean 4** | Swift | `ori_arc` pipeline, `Value` trait |

### The Critical Discovery: Boolean Effect Algebra (ICFP 2023)

> **This is the most important finding in the entire research effort.**

Lutze, Madsen, Schuster, and Brachthäuser ("With or Without You: Programming with Effect Exclusion," ICFP 2023) formalized effect exclusion using Boolean algebra with union, intersection, and **complement** operations on effects. Implemented in the Flix programming language.

**Core mechanism:** Effects form a Boolean algebra:
- `ef1 + ef2` — union (has both effects)
- `ef1 & ef2` — intersection
- `~ef` — complement (any effect EXCEPT ef)
- `ef1 - ef2` — difference (equivalent to `ef1 & ~ef2`)

**Flix syntax:** `def handler(h: ErrMsg -> a \ (ef - Throw)): a \ ef` — the handler must not throw.

**Results:**
1. **Effect Safety Theorem** — proven: "No excluded effect is ever performed." This is a *non-standard* soundness property beyond progress/preservation.
2. **Principal types** — preserved modulo Boolean equivalence. Algorithm W + Boolean unification finds most general types.
3. **Boolean unification** — decidable (NP-hard in general, but effect sets are small in practice). Flix implementation demonstrates feasibility.
4. **59 real-world code fragments** validated against the system.

**This directly solves Ori's #1 unresolved problem** — expressing "InterruptCtx prohibits Allocator." The mapping is exact:

| Flix | Ori |
|------|-----|
| `ef - Block` | `uses (ef without Allocator)` |
| `~Block` | `without Allocator` |
| `handle` removes effect | `with X = impl in` discharges |

See `negative-effects-research.md` for the full 672-line analysis and proposed Ori design.

### Quantitative Benchmarks for Ori's Targets

| Metric | Best Known | Source | Ori Target |
|--------|-----------|--------|------------|
| Proof-to-code ratio | 7.5:1 | Atmosphere/Verus (SOSP 2025) | Not needed for common code |
| Annotation overhead | 14% avg | Prusti (Rust) | <1% for capabilities; optional for contracts |
| Runtime safety overhead | 4.5% | Singularity/Sing# | <5% |
| Static proof of runtime checks | 95-98% | SPARK Silver level | Future target for contracts |
| Verification effort | 2 person-years for microkernel | Atmosphere/Verus | N/A (not verification-first) |
| Memory safety bug prevention | 91% of safety CVEs | ACSAC 2024 (Rust in kernel) | Match or exceed via graduated caps |
| Ecosystem unsafe analysis speed | 43K packages in 6.5h | Rudra (SOSP 2021) | N/A (graduated caps prevent the class) |

### Bottom Line

Ori's proposed direction is unusual but not unprecedented in pieces. What is novel is the specific combination: value semantics + effect/capability tracking + low-level/kernel programming + narrower replacement for blanket `unsafe`. No existing language does this exact combination.

The closest architectural inspiration is **Koka + Flix (Boolean effects) + F\*/Low\* + Pony/Austral + SPARK**, not any single language.

**The research surface is now broad enough to choose a direction, but not broad enough to skip
prototype validation or prerequisite compiler work.** The remaining work is design specification,
baseline infrastructure, and proof-of-feasibility implementation — see Parts 10-14.

---

## Part 8: Empirical Research — Real-World Unsafe Patterns in Linux Kernel Rust Code

> **Date:** 2026-03-22
>
> **Purpose:** Ground the Deep Safety initiative in measured reality, not theory. Every claim about what Ori can improve must start from what actually exists in kernel Rust code today — how much `unsafe` there is, where it concentrates, what it does, and what goes wrong when it fails.

### 8.1 Rust-for-Linux Unsafe Usage Statistics

#### Scale of the Codebase

| Metric | Value | Source | Planning use |
|--------|-------|--------|--------------|
| Rust LoC in mainline (late 2022 snapshot) | ~13,000 | USENIX ATC 2024 | historical baseline |
| Rust LoC in mainline + staging (2024 snapshot) | ~131,000 (19K upstream + 112K staging) | USENIX ATC 2024 | shows growth and abstraction demand |

This document intentionally avoids anchoring on later news-site LoC estimates or later secondary
unsafe-line counts unless the primary source is cited directly. Those numbers may be directionally
useful, but they are not necessary to establish the planning conclusion: Rust-for-Linux has grown
enough that missing safe abstractions, not toy examples, are the real constraint.

#### Where Unsafe Concentrates

The unsafe code in Rust-for-Linux is **not uniformly distributed**. It concentrates in three layers:

1. **Bindgen-generated FFI bindings** (`rust/bindings/`): 100% unsafe by definition. Every C kernel function called from Rust goes through `extern "C"` bindings auto-generated by `bindgen`. This is the largest source of `unsafe` by line count.

2. **Safe abstraction layer** (`rust/kernel/`): concentrated unsafe. This is the hand-written
   layer that wraps unsafe FFI bindings into safe Rust APIs. The unsafe here is *structural* — it
   exists to create the safety boundary, not because the logic requires it. Exact percentages vary
   by snapshot and study; the concentration pattern is the point that matters.

3. **Drivers** (target: 0% unsafe): The stated goal is that driver code should contain zero `unsafe`. Drivers invoke the safe abstractions in `rust/kernel/` and should never need to touch raw pointers, FFI, or unsafe operations directly. As of 2025, this goal is achievable for drivers that have complete safe abstractions, but many subsystem abstractions are still missing.

#### Unsafe by Subsystem (from ACSAC 2024 vulnerability study)

The ACSAC 2024 paper ("Rust for Linux: Understanding the Security Impact of Rust in the Linux Kernel") studied 240 real driver vulnerabilities rather than counting unsafe blocks directly. Their classification:

| Vulnerability Class | Count | Percentage | Rust Prevention Rate |
|---------------------|-------|------------|---------------------|
| Safety (memory, type, bounds) | 113 | 47% | 91% (56% by Rust alone + 34% with developer discipline) |
| Protocol violations (API misuse, ordering) | 82 | 34% | Partial — Rust's type system helps but cannot enforce kernel-specific protocols |
| Semantic violations (logic errors) | 45 | 19% | Minimal — these are wrong-algorithm, not wrong-memory bugs |

**Key finding:** Rust can eliminate **91% of safety-class vulnerabilities** (the largest class), but protocol violations (34%) and semantic violations (19%) require enforcement mechanisms beyond what a type system provides — exactly the space where Ori's capability system and contracts could add value.

A separate analysis of 150 kernel CVEs from 2020-2024 found that **67% fell into memory safety categories** that Rust structurally prevents at compile time.

#### USENIX ATC 2024 Findings on Developer Experience

The USENIX ATC 2024 paper ("An Empirical Study of Rust-for-Linux: The Success, Dissatisfaction, and Compromise") found:

- **Bugs in merged RFL code:** 11 compilation bugs, 4 safe-abstraction-related bugs, 6 soundness bugs in the safe abstraction layer, 3 thread-safety violations
- **Developer dissatisfaction** centers on: the gap between safe abstractions needed and safe abstractions available; the difficulty of expressing kernel-specific patterns (pinned initialization, intrusive data structures) in Rust's type system; and the overhead of `unsafe` documentation
- **Compromise patterns**: Developers frequently work around missing abstractions by using `unsafe` directly in driver code, undermining the zero-unsafe-in-drivers goal

#### Broader Rust Ecosystem Context

- ~29% of Rust crates use `unsafe` code
- 80% of crates declare 3 or fewer unsafe contexts
- 44.6% of unsafe function definitions are FFI bindings — exactly the pattern that dominates RFL
- The kernel's abstraction layer has a materially higher unsafe concentration than a typical
  application crate because it is, by design, the FFI and kernel-invariant boundary layer

### 8.2 CVE-2025-68260 Analysis — The First Rust CVE in Linux

#### What Happened

CVE-2025-68260 is the first publicly disclosed vulnerability in Rust code within the Linux kernel. It was assigned in late 2025 and affects the Android Binder driver rewrite in Rust, introduced in Linux 6.18.

| Property | Value |
|----------|-------|
| Subsystem | Android Binder (IPC), Rust rewrite |
| Root cause | Race condition in intrusive linked-list manipulation |
| Impact | Denial of service (kernel crash), NOT remote code execution |
| Affected versions | Linux 6.18+ |
| Fixed in | 6.18.1, 6.19-rc1 |
| Severity | Stability issue, not exploitation primitive |

#### The Vulnerable Code Pattern

The bug was in `drivers/android/binder/node.rs`. The critical unsafe operation:

```rust
unsafe { node_inner.death_list.remove(self) };
```

The accompanying safety comment claimed: "NodeDeath is either in this list or in no list." This invariant was **incomplete** — it did not account for concurrent access.

#### Race Condition Mechanism

1. **Thread A** (`Node::release`) acquired a lock, moved death list items to a stack-allocated temporary list, then **released the lock** before finishing iteration
2. **Thread B** (`NodeDeath` cleanup) simultaneously called the unsafe `remove()` on what it believed was the original list
3. Both threads modified the same `prev/next` pointers without synchronization, corrupting memory

The NVD description: "touching `prev/next` pointers requires guaranteeing no other thread touches them in parallel; the foreign list case violates that guarantee."

#### What Rust's Type System Could and Could Not Prevent

**Could not prevent:**
- The memory corruption itself — the code used explicit `unsafe` blocks that bypass Rust's guarantees
- The logical error of releasing a lock prematurely — this is a concurrency design flaw, not a memory safety violation
- The incomplete invariant documentation — `// SAFETY:` comments are prose, not machine-checked

**Could have prevented (with proper abstractions):**
- A type-safe scoped-lock API (like Rust's `MutexGuard`) would have prevented the premature lock release — but the kernel Binder code bypassed this pattern
- A safe linked-list API that enforces mutual exclusion at the type level would have prevented the unguarded removal

#### Could Graduated Capabilities Have Prevented This?

**Yes, partially.** The specific failure pattern maps to capabilities Ori proposes:

1. **`uses Synchronization`** — The `remove()` operation on a shared intrusive list should require proving that the appropriate lock is held. In Ori's model, the scoped lock API (`with_lock(mutex, body:)`) would prevent the premature release pattern because the lock scope is the function body — you cannot release the lock and continue operating on the protected data.

2. **Contracts** — `pre(node_inner.lock.is_held())` on the `remove()` operation would have been a runtime check that catches the violation. Not a static proof, but better than a comment that nobody machine-checks.

3. **Value semantics** — Ori's value semantics help, but they do not automatically eliminate the
   intrusive linked-list problem in Linux-style environments. If Ori owns the abstraction boundary,
   it can prefer non-intrusive containers and scoped synchronization APIs, which would avoid this
   exact pattern in some subsystems. But Linux-compatible intrusive structures do not disappear by
   themselves; they still need either safe wrappers or confinement behind FFI/capability APIs.

**However:** The deeper issue — that a concurrency bug slipped through because `unsafe` disabled the compiler's ability to reason about the code — is exactly the problem Ori's graduated capabilities address. The `unsafe` block disabled *all* compiler checking. With graduated capabilities, only the specific capability (`Synchronization` + `RawMemory`) would be enabled, and the rest of the compiler's reasoning would remain active.

#### Comparative Context

On the same day CVE-2025-68260 was published, C code in the Linux kernel received **159 CVEs**. The Rust codebase, at less than 1% of the kernel, produced 1 CVE across ~3 years of integration — a denial-of-service, not a privilege escalation or remote code execution.

#### Detection: What Could Have Caught It Earlier

- **Loom** (concurrency testing framework): Exhaustively samples thread schedules. The article explicitly states: "The bug in CVE-2025-68260 would likely have been found before release if tested with Loom."
- **ThreadSanitizer (TSan)**: Detects data races at runtime
- **Klint**: If extended to track lock-hold contexts, could have flagged the unsafe operation outside the lock scope

### 8.3 Real virtio-net Driver Anatomy

#### virtio-drivers Crate (Rust userspace/embedded)

The `virtio-drivers` crate (rcore-os/virtio-drivers) provides a Rust implementation of VirtIO guest drivers. While not the in-kernel Linux driver, it exercises the same hardware protocol and provides the best available Rust-language source for analyzing unsafe patterns in VirtIO.

**Unsafe block count in `queue.rs`:** ~25 unsafe blocks in the core VirtQueue implementation.

**What operations require unsafe:**

| Operation | Why Unsafe | Count |
|-----------|-----------|-------|
| Descriptor table access | Raw pointer to DMA-allocated memory | ~8 |
| Available ring writes | Shared memory written by driver, read by device | ~4 |
| Used ring reads | Shared memory written by device, read by driver | ~3 |
| Atomic ordering (fences) | Memory barriers for device-driver synchronization | ~3 |
| HAL share/unshare | Platform-specific DMA buffer management | ~5 |
| Box from raw pointer | Recovering indirect descriptor allocations | ~2 |

**The VirtQueue structure:**

The virtqueue is a pair of ring buffers in shared memory:
- **Descriptor table**: Array of 16-byte descriptors, each pointing to a buffer (address, length, flags, next)
- **Available ring**: Driver writes buffer head indices here to submit work to device
- **Used ring**: Device writes completed buffer indices here

The driver maintains **shadow copies** (`desc_shadow`, `avail_idx`) because the device may modify shared memory at any time — the driver cannot trust reads from shared memory to be consistent.

**Ring buffer management pattern:**

```
1. Allocate descriptor(s) from free list
2. Fill descriptor with buffer physical address, length, flags
3. Copy shadow descriptor to device-visible descriptor table (unsafe: raw ptr write)
4. Write descriptor head to available ring (unsafe: raw ptr write + memory fence)
5. Write doorbell register (MMIO write) to notify device
6. [device processes request asynchronously]
7. Read used ring for completions (unsafe: raw ptr read + memory fence)
8. Reclaim descriptors to free list
```

Steps 3, 4, 5, 7 are all unsafe because they involve either raw pointer access to DMA memory or MMIO register writes.

**Non-blocking API safety requirements:**
- Buffer ownership must remain valid until completion callback
- Token matching: same token from start function must be passed to completion function
- Buffer consistency: same buffers passed to start must be passed to complete

These are exactly the kind of invariants that contracts (`pre()`/`post()`) can express.

#### The in-kernel virtio driver situation

The mainline Linux kernel does not yet have a Rust virtio-net driver. The C virtio-net driver (`drivers/net/virtio_net.c`) is approximately 4,500 lines. The Rust virtio abstractions are under development but not merged for networking.

#### Page allocator interaction

VirtIO drivers interact with the kernel page allocator through:
1. **DMA coherent allocation** (`dma_alloc_coherent()`) for descriptor rings — pages must be physically contiguous and accessible by device
2. **Streaming DMA** (`dma_map_single()` / `dma_map_page()`) for data buffers — mapped per-transfer, with cache synchronization
3. **Page allocation** (`alloc_pages()`) for receive buffers — must be DMA-capable (below device's DMA mask)

All of these are C functions accessed through unsafe FFI.

### 8.4 Real NVMe Driver Patterns

The Rust NVMe driver for Linux is led by Andreas Hindborg (Samsung, formerly Western Digital). It is a research/development vehicle, not production-ready, but exercises all the real NVMe protocol paths.

#### Admin Queue vs I/O Queue Setup

**Admin queue** (queue pair 0):
- Created during controller initialization
- Used for controller management commands: Identify, Create I/O Queue, Delete I/O Queue, Set Features, Get Log Page
- Single queue pair, typically depth 32
- Must be created before any I/O queues

**I/O queues** (queue pairs 1..N):
- Created via admin commands after controller initialization
- One pair per CPU core for lock-free submission (each core writes only to its own doorbell)
- Typical depth 1024
- Mapping: each submission queue has exactly one completion queue; multiple submission queues may share a completion queue

#### DMA Coherent Allocations

NVMe requires DMA-coherent memory for:
- **Submission queues**: 64-byte command entries, physically contiguous
- **Completion queues**: 16-byte completion entries, physically contiguous
- **PRP (Physical Region Page) lists**: For commands requiring more than 2 pages of data transfer

The driver allocates these via `dma_alloc_coherent()`, which returns both a CPU virtual address and a DMA bus address. The driver must track both and free them correctly on teardown.

**Alignment**: NVMe requires page-aligned buffers. The driver uses `max(Dma::page_size(), CAP.MPSMIN)` for alignment.

#### Command Submission/Completion Flow

```
Submission:
1. Build 64-byte NVMe command (opcode, namespace, PRP entries, command-specific fields)
2. Write command to submission queue at tail index (DMA memory write)
3. Increment tail index (modular)
4. Write new tail to submission queue tail doorbell register (MMIO write)

Completion:
1. Poll completion queue head for new entries (check phase bit flip)
2. Read 16-byte completion entry (status, command ID, submission queue head)
3. Match command ID to outstanding request
4. Process result (success/error)
5. Update completion queue head doorbell register (MMIO write)
6. Fire interrupt coalescing / process next
```

#### Interrupt Handling

NVMe supports MSI-X interrupts (one per I/O queue). The interrupt handler:
1. Acknowledges the interrupt
2. Processes all pending completion entries in the queue
3. Reclaims command slots

In the Rust driver, the interrupt handler calls into the completion processing path. The stated goal is to **remove all unsafe code from the driver** — this remains a work item as of 2025.

#### Performance

At 4 KiB block sizes, the Rust NVMe driver performs comparably to the C driver. At 512 B block sizes, the C driver outperforms Rust by up to 6% — attributed to higher per-operation overhead in the Rust driver for compute-limited (small-transfer) workloads.

### 8.5 The Rust Kernel Abstractions Layer (`rust/kernel/`)

#### `Opaque<T>` — How It Actually Works

`Opaque<T>` is the foundation for wrapping C kernel objects that Rust should never interpret directly.

**Structure:**
```rust
#[repr(transparent)]
pub struct Opaque<T> { /* MaybeUninit<UnsafeCell<T>> */ }
```

**Key properties:**
- `#[repr(transparent)]` — same memory layout as `T`
- Contains `MaybeUninit<UnsafeCell<T>>` — allows uninitialized values and interior mutability
- Implements `Send` (when `T: Send`) but NOT `Sync` — cannot be shared across threads without explicit synchronization
- Does NOT implement `RefUnwindSafe`

**Construction:**
- `Opaque::new(value)` — wrap an existing value (const fn)
- `Opaque::uninit()` — create uninitialized (const fn, for later C-side initialization)
- `.get() -> *mut T` — return raw pointer for C interop (unsafe to dereference)

**Why this matters for Ori:** `Opaque<T>` exists because Rust's safety model requires all values to be initialized and valid. Kernel C objects frequently violate this — they are allocated uninitialized, then initialized by C code, and contain interior mutability that violates Rust's aliasing rules. `Opaque<T>` is an escape hatch that says "Rust, don't look inside this value."

Ori's `CPtr` serves an analogous role but is simpler — it is a fully opaque handle with no generic parameter, no interior mutability concerns, and no initialization problem. This is a genuine design advantage of Ori's approach.

#### The pin-init Framework — Exact Mechanics

**The problem:** Kernel objects (mutexes, condition variables, device structures) must not move after initialization because C code holds raw pointers to them. Rust's "return by value" model moves objects, invalidating those pointers.

**The solution:** A two-phase initialization framework:

1. **Annotate struct with `#[pin_data]`** — marks which fields are structurally pinned (using `#[pin]`)
2. **Create initializer with `pin_init!` macro** — syntax similar to struct initializer but uses `<-` for in-place initialized fields
3. **Initialize in place via `InPlaceInit::pin_init()`** on a smart pointer (`Arc`, `Box`)

```rust
#[pin_data]
struct MyDevice {
    #[pin]
    mutex: Mutex<Data>,
    name: CString,
}

let dev = Arc::pin_init(pin_init!(MyDevice {
    mutex <- new_mutex!(Data::default()),
    name: CString::try_from_fmt(fmt!("my_device"))?,
}))?;
```

**Unsafe in pin-init:**
- `pin_init_from_closure()` is unsafe — caller must ensure the closure actually initializes all fields
- Pin projections require `unsafe` via `map_unchecked_mut()`
- `MaybeUninit::assume_init()` is unsafe — caller asserts initialization occurred

**Why this matters for Ori:** Ori's value semantics eliminate part of the pin-init problem for
pure Ori-owned values, because ordinary Ori values do not expose Rust-style move-after-pin
hazards. But C-owned objects, DMA-visible buffers, and other FFI-facing resources may still need
stable addresses and in-place initialization semantics. `CPtr` and Deep FFI can hide some of that
complexity from ordinary Ori code, but they do not make the underlying constraint disappear.

#### `ARef<T>` and `AlwaysRefCounted` — Real Usage

`ARef<T>` manages owned references to kernel objects that use C-level reference counting (`kref`, `refcount_t`).

**`AlwaysRefCounted` trait** (unsafe to implement):
- `inc_ref(&self)` — increment the C-side reference count
- `dec_ref(obj: NonNull<Self>)` — decrement; free when count reaches zero

**`ARef<T>` construction:**
- `ARef::from_raw(ptr: NonNull<T>)` — unsafe; caller must ensure the refcount was incremented and they are relinquishing one increment
- `From<&T>` — safe; increments refcount during conversion

**Key limitation:** `ARef<T>` is explicitly **not Send or Sync** — it cannot be transferred across threads or shared. This is conservative but safe.

**Why this matters for Ori:** Ori's native ARC handles reference counting transparently. For C kernel objects with C-side refcounts, Deep FFI's `#free` annotation + ownership tracking could manage the decrement on drop without requiring an unsafe trait implementation.

#### `ForeignOwnable` — How It Is Actually Used

`ForeignOwnable` transfers Rust object ownership to C code (as `void *`) and recovers it later.

**Trait definition:**
```rust
pub unsafe trait ForeignOwnable: Sized {
    const FOREIGN_ALIGN: usize;
    type Borrowed<'a>;
    type BorrowedMut<'a>;

    fn into_foreign(self) -> *mut c_void;
    unsafe fn from_foreign(ptr: *mut c_void) -> Self;
    unsafe fn borrow<'a>(ptr: *mut c_void) -> Self::Borrowed<'a>;
    unsafe fn borrow_mut<'a>(ptr: *mut c_void) -> Self::BorrowedMut<'a>;
}
```

**Implementations:** `()`, `Box<T>`, `Pin<Box<T>>`, `Arc<T>`

**Usage pattern:** A driver stores private data in a C structure's `void *` field:
1. `let ptr = my_data.into_foreign()` — erase type, transfer ownership to C
2. C code stores `ptr` in `dev_set_drvdata()` or similar
3. Later: `let data = unsafe { MyData::from_foreign(ptr) }` — recover ownership
4. Or: `let borrowed = unsafe { MyData::borrow(ptr) }` — temporary access

**Why `unsafe`:** The compiler cannot verify that the pointer is valid, hasn't been double-freed, and is being recovered at the correct type. The entire pattern relies on programmer discipline.

**Why this matters for Ori:** Ori's Deep FFI with typed `CPtr` round-trips could make this pattern safer by tracking the Rust type associated with the opaque pointer. The `ForeignOwnable` trait's `into_foreign`/`from_foreign` pair maps directly to `owned` annotations in Deep FFI.

#### The `#[vtable]` Macro — What It Generates

The `#[vtable]` macro bridges Rust traits and C function pointer tables.

**For each method in the trait, it generates:**
- A `HAS_<METHOD>` associated constant (`bool`) — `true` if the implementer overrides the method, `false` if using default
- A C-visible `unsafe extern "C"` callback wrapper that:
  1. Receives raw C types as parameters
  2. Converts them to safe Rust abstractions inside `unsafe` blocks
  3. Calls the user's safe Rust implementation
  4. Converts the result back to C types

**Optional vs required methods:**
- Optional methods must have a default implementation (but the default calls `build_error!()` to prevent accidental execution)
- When `HAS_<METHOD>` is false, a `NULL` pointer is installed in the C vtable
- The C subsystem checks for `NULL` before calling, matching the standard kernel pattern

**Example generated pattern:**
```rust
#[vtable]
impl Operations for MyDriver {
    fn foo(&self) -> Result<()> { /* ... */ }
    // bar not implemented — HAS_BAR = false, NULL in C vtable
}
assert_eq!(<MyDriver as Operations>::HAS_FOO, true);
assert_eq!(<MyDriver as Operations>::HAS_BAR, false);
```

**Why this matters for Ori:** The `#[vtable]` pattern shows that callback registration requires a thin unsafe layer to translate between C and Rust type representations. Ori's trait system + Deep FFI could generate equivalent bridging code — the `Sendable` bound on callbacks ensures thread safety, and the capability system tracks what the callback is permitted to do.

### 8.6 LWN Articles on Kernel Rust Safety

#### LWN 982868 — "Standards for use of unsafe Rust in the kernel"

Benno Lossin proposed documentation standards for `unsafe` usage. Key distinctions:

**Two categories of `unsafe`:**
1. **Unsafe operations** — code relying on guarantees the compiler cannot verify. Documentation must explain "why the operation is safe in this case" (context-specific justification).
2. **Unsafe functions** — functions where the compiler cannot fully understand safety conditions. Documentation must explain "what the requirements are to use the function safely" (preconditions).

**Critical distinction (Alice Ryhl):** Safety comments must explain "why the preconditions are satisfied, *not* what the preconditions are." Conflating these is "a really really common mistake."

**Enforcement:** Daniel Almeida proposed using linters (Clippy already supports this) to enforce documentation presence. A follow-up patch would formalize the requirement.

**Relevance to Ori:** This is exactly the problem Ori's contracts solve. `pre()` and `post()` are machine-readable replacements for prose safety comments. They can be runtime-checked today and potentially statically verified later. The distinction between "what the preconditions are" (the contract itself) and "why they are satisfied" (the call-site justification) maps to the difference between the function's contract declaration and the caller's capability evidence.

#### LWN 985848 — "Banning unsafe in Rust for Linux device drivers"

**The policy:** "Don't use unsafe when there's a suitable safe abstraction you can use at no performance cost." In the long run, this would be equivalent to a **complete ban** on unsafe in driver code.

**Key points:**
- The plan has always been to make it possible to write drivers without the need for `unsafe`
- Performance parity is a core requirement — safe alternatives must match unsafe equivalents in efficiency
- Safe abstractions are expected to emerge gradually
- When complete, any `unsafe` in a driver would be noteworthy during code review

**Can drivers be written with ZERO unsafe?** The consensus is **yes**, provided safe abstractions exist for all the C subsystem interfaces the driver uses. The Asterinas project (see 8.10) demonstrates this is achievable for an entire OS kernel.

**What abstractions are still needed:**
- Many subsystems lack safe Rust abstractions: clocks, pin control, runtime power management, regulators, and others
- As of 2025, the development of safe abstractions has been "a significant challenge" in Rust adoption across kernel subsystems

### 8.7 Klint and Sparse — Kernel Static Analysis

#### Klint — Compile-Time Atomic Context Checking for Rust

Klint tracks preemption count at the function level. Every function gets two properties:

| Property | Meaning | Example |
|----------|---------|---------|
| **Adjustment** | How the function changes preempt_count | `spin_lock`: +1, `spin_unlock`: -1 |
| **Expected range** | What preempt_count values are valid when calling | `mutex_lock`: expects 0 (task context only), `spin_lock`: any |

**Rules enforced:**
- Sleepable functions can only be called with preemption count = 0
- Holding a spinlock (preempt_count > 0) prevents calling any function that might sleep
- RCU read-side critical sections (preempt_count > 0) prevent sleeping — violation causes use-after-free

**Annotations:**
- `#[klint::preempt_count]` — marks function behavior with adjustment and expectation
- `#[klint::drop_preempt_count]` — annotates Drop implementation's preemption impact
- `unchecked` modifier — skip inference validation

**Effectiveness:** Successfully identified bugs in experimental RFL code where drop implementations violated preemption count expectations through complex call chains.

**Limitations:**
- Cannot handle `try_lock`-style operations (conditionally adjust preempt_count)
- Cannot handle conditional lock acquisition/release patterns
- Indirect function calls default to "sleepable" (conservative)
- Trait objects assumed sleepable unless annotated

**Relevance to Ori:** Klint's preemption count tracking is a **specific instance** of what Ori's capability system generalizes. In Ori's model:
- `uses InterruptCtx` ≈ "preempt_count > 0"
- `InterruptCtx` prohibits `Allocator` and `Suspend` ≈ Klint's "cannot sleep with preempt_count > 0"
- The difference: Klint is a separate external tool with its own annotation system; Ori bakes context tracking into the language's effect system

#### Sparse — C Semantic Checker

Sparse provides compile-time annotations for C code:

**Address space annotations:**
- `__user` — user-space pointer (cannot be dereferenced directly in kernel)
- `__kernel` — kernel-space pointer
- `__iomem` — I/O memory-mapped pointer (requires volatile access)
- `__bitwise` — strict bitwise type checking (prevents endianness mixing)

**Lock annotations:**
- `__must_hold(lock)` — lock is held on entry and exit
- `__acquires(lock)` — lock is held on exit but not entry
- `__releases(lock)` — lock is held on entry but not exit

**How it works:** Invoked during kernel build with `make C=1` (recompiled files only) or `make C=2` (entire tree). Defines `__CHECKER__` preprocessor symbol for conditional compilation.

**Limitations:**
- C-only (no Rust support)
- Annotations are optional and not universally applied
- Cannot track complex lock ordering across multiple locks
- No data-flow analysis — purely syntactic/semantic matching
- The annotations are undefined under GCC, so they serve as documentation more than enforcement

**Relevance to Ori:** Sparse's address space annotations (`__user`, `__kernel`, `__iomem`) map directly to Ori's proposed typed address newtypes (`UserPtr`, `VirtAddr`, `PhysAddr`). The difference: Sparse annotations are post-hoc documentation; Ori's types are structural — you literally cannot pass a `UserPtr` where a `VirtAddr` is expected.

### 8.8 DMA Patterns Across Real Drivers

#### Coherent vs Streaming DMA

| Property | Coherent | Streaming |
|----------|----------|-----------|
| Lifetime | Driver init to shutdown | Per-transfer |
| Cache coherency | Hardware-guaranteed | Requires explicit sync |
| Use case | Ring descriptors, mailboxes, firmware | Packet buffers, I/O data |
| API | `dma_alloc_coherent()` / `dma_free_coherent()` | `dma_map_single()` / `dma_unmap_single()` |
| Direction | Implicit bidirectional | Must specify: `DMA_TO_DEVICE`, `DMA_FROM_DEVICE`, `DMA_BIDIRECTIONAL` |

#### Scatter-Gather Lists

`dma_map_sg()` maps a scatter-gather list of non-contiguous pages into a set of DMA-capable bus addresses. Critical invariant: the function may **merge** consecutive entries, returning fewer entries than submitted. The unmap call must use the **original** entry count, not the mapped count.

#### Bounce Buffers

When a buffer's physical address is outside the device's DMA mask (e.g., above 4GB for a 32-bit DMA device), the DMA subsystem automatically allocates a bounce buffer within the addressable range, copies data, and performs the DMA from the bounce buffer. This is transparent to the driver but has performance implications.

#### IOMMU Interaction

IOMMUs translate bus addresses to physical addresses, providing:
- **DMA address space isolation** — devices can only access memory explicitly mapped for them
- **Address translation** — virtual DMA addresses can map to any physical page
- **Scatter-gather coalescing** — physically non-contiguous pages can appear contiguous to the device

Drivers provide virtual addresses to mapping functions; the IOMMU programs translations and returns bus addresses for device consumption.

#### Critical Driver Invariants

1. **Check `dma_mapping_error()`** after every `dma_map_single()` or `dma_map_page()` — failure means the mapping was not created
2. **Synchronize CPU and device access**: `dma_sync_single_for_cpu()` before CPU reads DMA-written data; `dma_sync_single_for_device()` before device reads CPU-written data
3. **Match map/unmap pairs** — every `dma_map_*()` must have a corresponding `dma_unmap_*()` to prevent DMA address space exhaustion
4. **Use correct direction flags** — `DMA_TO_DEVICE` data must not be read by device as if it were `DMA_FROM_DEVICE`
5. **Only DMA-safe memory**: `kmalloc()`, page allocators, `kmem_cache_alloc()`. Stack, kernel image, module, and `vmalloc()` memory must **never** be used for DMA
6. **Alignment**: `dma_alloc_coherent()` returns page-aligned memory. Streaming DMA buffers must not share cache lines with non-DMA data (minimum alignment: `ARCH_DMA_MINALIGN`, up to 128 bytes on non-coherent platforms)
7. **Memory barriers**: Even coherent DMA requires explicit `wmb()` / `rmb()` if ordering matters — the CPU may reorder stores to coherent memory

**Relevance to Ori:** These invariants are exactly what contracts can express:
- `pre(dma_mask_set(device))` before mapping
- `post(result.is_ok())` after mapping (replaces `dma_mapping_error()` check)
- Direction flags become an enum type — cannot pass wrong direction
- `DmaBuffer<T>` newtype prevents using stack/vmalloc memory for DMA
- Alignment requirements encoded in `DmaBuffer<T>` constructor contracts

### 8.9 Interrupt and Context Patterns

#### Context Hierarchy

| Context | Preemptible | Can Sleep | Can Allocate GFP_KERNEL | Can Hold Mutex |
|---------|-------------|-----------|------------------------|----------------|
| User (process) context | Yes | Yes | Yes | Yes |
| Softirq / Tasklet | No | **No** | **No** (GFP_ATOMIC only) | **No** |
| Hardware IRQ (top-half) | No | **No** | **No** (GFP_ATOMIC only) | **No** |
| Spinlock held | No | **No** | **No** (GFP_ATOMIC only) | **No** |
| RCU read-side | No | **No** | **No** (GFP_ATOMIC only) | **No** |

**Each context can only be preempted by those above it in the hierarchy.**

#### Top-Half vs Bottom-Half

**Top-half (hardirq handler):**
- Runs with interrupts partially disabled
- Must execute quickly — acknowledge interrupt, copy essential data, schedule bottom-half
- Cannot sleep, allocate with GFP_KERNEL, acquire mutexes, or access user space
- Registered via `request_irq()` with a function pointer

**Bottom-half mechanisms:**

| Mechanism | Context | Can Sleep | Use Case |
|-----------|---------|-----------|----------|
| Softirq | Atomic | No | High-frequency, performance-critical (networking, block) |
| Tasklet | Atomic | No | Per-device deferred work, serialized per-tasklet |
| Workqueue | Process | **Yes** | Long-running, may sleep (firmware loading, device config) |

- **Softirqs** are statically allocated at compile time, cannot be created/destroyed dynamically. Same softirq type CAN run on multiple CPUs simultaneously.
- **Tasklets** are dynamic, built on top of softirqs. Same tasklet type CANNOT run on multiple CPUs simultaneously (serialized).
- **Workqueues** run in kernel thread context. Can sleep, allocate, acquire mutexes. Include `lockdep_map` for lock dependency tracking.

#### Forbidden Operations by Context

| Operation | User Context | Softirq/Tasklet | Hardware IRQ | Spinlock Held |
|-----------|-------------|-----------------|-------------|---------------|
| Sleep/schedule | OK | **FORBIDDEN** | **FORBIDDEN** | **FORBIDDEN** |
| `kmalloc(GFP_KERNEL)` | OK | **FORBIDDEN** | **FORBIDDEN** | **FORBIDDEN** |
| `kmalloc(GFP_ATOMIC)` | OK | OK | OK | OK |
| `mutex_lock()` | OK | **FORBIDDEN** | **FORBIDDEN** | **FORBIDDEN** |
| `spin_lock()` | OK | OK | OK (irqsave variant) | Nested OK |
| `copy_from_user()` | OK | **FORBIDDEN** | **FORBIDDEN** | **FORBIDDEN** |
| `copy_to_user()` | OK | **FORBIDDEN** | **FORBIDDEN** | **FORBIDDEN** |

**Debugging:** `CONFIG_DEBUG_ATOMIC_SLEEP` warns at runtime when sleeping rules are violated.

#### How lockdep Detects Context Violations

lockdep tracks lock usage states along four dimensions:
- Ever held in **hardirq context**
- Ever held in **softirq context**
- Ever held with **hardirq enabled**
- Ever held with **softirq enabled**

**Single-lock rules:** A lock cannot be both irq-safe (used in interrupt context) AND irq-unsafe (acquired with interrupts enabled). Same for softirq-safe/unsafe.

**Multi-lock rules:**
1. No self-acquisition (same lock class acquired twice)
2. No circular ordering (if L1->L2 observed, L2->L1 is forbidden)
3. No context mixing (hardirq-safe lock cannot depend on hardirq-unsafe lock)

**Detection algorithm:** lockdep builds a dependency graph of lock classes. At each acquisition, it checks for strong circular paths using four edge types: ER (exclusive->recursive reader), EN (exclusive->non-recursive), SR (shared reader->recursive), SN (shared reader->non-recursive). A closed strong path proves a deadlock combination exists.

**Performance:** Hash-based — each unique lock chain is validated only once. "100% certainty that no combination and timing of these locking sequences can cause any class of lock related deadlock."

**Relevance to Ori:** lockdep's context tracking is runtime-only and C-only. Ori's capability system could enforce these constraints statically:
- `uses InterruptCtx` functions cannot call `uses Allocator` or `uses Suspend` functions — compile-time check
- `uses Synchronization` with ordered lock types (e.g., `Lock<Level1>`, `Lock<Level2>`) could prevent inversion — const generic lock levels
- The "cannot hold mutex in interrupt context" rule becomes a type error, not a runtime warning

### 8.10 The "Safe Driver" Boundary Question

#### Is There a Clean Separation?

**Yes, architecturally.** The Rust-for-Linux project defines a clear three-layer architecture:

```
Layer 3: DRIVERS          — target 0% unsafe, pure business logic
Layer 2: SAFE ABSTRACTIONS (rust/kernel/) — concentrated unsafe, wraps C APIs
Layer 1: FFI BINDINGS     (rust/bindings/) — 100% unsafe, auto-generated
```

**In practice, the separation is incomplete.** Many subsystem abstractions do not yet exist, forcing driver developers to use `unsafe` directly. Missing abstractions include: clocks, pin control, runtime power management, regulators, and many subsystem-specific interfaces.

#### Asterinas: Proof That 100% Safe Drivers Are Achievable

The Asterinas project provides the strongest empirical evidence:

| Metric | Value |
|--------|-------|
| Total kernel LoC | 100,000+ Rust |
| TCB (OSTD framework) | ~15,000 LoC (~14% of kernel) |
| TCB unsafe density | High (concentrated) |
| Everything else (drivers, filesystems, networking) | **100% safe Rust** |
| Linux syscall coverage | 210+ syscalls |
| Performance vs Linux | Competitive (on par) |

Asterinas uses a "framekernel" architecture:
- **OSTD** (OS Development Toolkit): ~15K LoC of carefully reviewed safe+unsafe Rust that wraps low-level operations (context switching, user-mode CPU state, virtual memory management, page tables) into safe abstractions
- **Everything above OSTD**: 100% safe Rust — drivers, filesystems, networking, all of it

This is comparable to verified microkernels like seL4 (~10K LoC TCB), while commodity monolithic kernels (including Rust-for-Linux) have TCBs orders of magnitude larger.

**Key insight:** The OSTD framework demonstrates that the "unsafe hardware boundary" can be confined to ~14% of the kernel. The remaining 86% — including all drivers — operates in 100% safe Rust.

#### What Percentage of a Typical Driver is Hardware-Boundary Code vs Logic?

Based on analysis of VirtIO and NVMe driver structures:

| Component | Approximate % | Unsafe? |
|-----------|--------------|---------|
| MMIO register access | 5-10% | Yes (raw pointer dereference) |
| DMA buffer management | 10-15% | Yes (physical address, cache coherency) |
| Interrupt handler registration | 2-5% | Yes (callback registration, context rules) |
| Queue/ring buffer management | 15-25% | Partially (descriptor manipulation) |
| Protocol state machine | 25-35% | **No** — pure logic |
| Error handling / recovery | 10-15% | **No** — pure logic |
| Configuration / setup / teardown | 10-15% | Partially (FFI for C subsystem calls) |

**Bottom line:** Roughly 20-35% of a typical driver involves hardware-boundary operations that require some form of unsafe/capability-gated code. The remaining 65-80% is pure logic that should be entirely safe.

This supports Ori's thesis: graduated capabilities can cover the 20-35% hardware boundary with specific, tracked permissions rather than blanket `unsafe`, while the 65-80% logic portion remains fully compiler-verified.

#### The Unsafe Pervading vs Contained Debate

There are two models:

1. **Rust-for-Linux model (contained):** Unsafe is concentrated in the safe abstraction layer. Drivers themselves should be 100% safe. Works well when abstractions exist; breaks down when they don't.

2. **Reality today (pervading):** Because many abstractions are missing, drivers must use `unsafe` directly. This means `unsafe` pervades driver code in practice, even though the architecture says it shouldn't.

3. **Asterinas model (maximally contained):** A small, carefully audited framework (~14% of kernel) contains ALL unsafe code. Everything above it, including drivers, is provably safe.

Ori's Deep Safety proposal targets an outcome between models 1 and 3: the capability system replaces blanket `unsafe` with specific capabilities (`VolatileIO`, `DMA`, `InterruptCtx`) that are tracked, composed, and contracted — even the 20-35% hardware-boundary code gets compiler assistance.

---

## Part 9: Implications for Ori's Deep Safety Design

### What the Data Supports

1. **The safe-driver goal is achievable.** Asterinas proves 100% safe drivers work in practice. RFL's architecture targets the same. Ori should design for this.

2. **The real enemy is not `unsafe` — it's missing abstractions.** CVE-2025-68260 happened because developers used `unsafe` intrusive list operations instead of a safe abstraction. The RFL developer dissatisfaction is primarily about missing safe abstractions, not about the Rust language.

3. **Rust materially reduces the safety-bug surface, but not the protocol/semantic surface.** The
   ACSAC 2024 study argues Rust can prevent or constrain most memory-safety-class cases in the
   studied corpus. The remaining protocol and semantic violations still need something beyond a
   conventional ownership/type system — which is where capabilities, contracts, and domain
   abstractions could add value.

4. **Context rules are the highest-value enforcement target.** Klint, lockdep, sparse, and `CONFIG_DEBUG_ATOMIC_SLEEP` all exist to enforce "you cannot do X in context Y" — exactly what `InterruptCtx` prohibiting `Allocator` and `Suspend` expresses. These are currently runtime checks or external tools; making them compile-time through the capability system is a clear win.

5. **DMA invariants are contract-shaped.** Every DMA invariant (check mapping error, match map/unmap, correct direction, correct alignment, safe memory source) is a precondition or postcondition. They are currently unchecked or runtime-checked. Contracts make them explicit and testable.

6. **Typed addresses eliminate real bug classes.** Sparse's `__user`, `__kernel`, `__iomem` annotations exist because mixing address types is a real bug. Making these types rather than annotations (as Ori proposes) catches the bugs at compile time.

### What the Data Challenges

1. **The negative-effect system is non-negotiable.** Context rules ("InterruptCtx prohibits Allocator") require expressing what is *forbidden*, not just what is *required*. No existing part of Ori's spec supports this. Pony's deny-capability model provides the design precedent.

2. **Contracts are not proofs.** The scorecard in Part 4 must be honest: runtime `pre()`/`post()` checks are better than comments but are not static guarantees. Until Ori has SMT-backed static contract verification (like SPARK/F*), contract-based safety is strictly less than compile-time enforcement.

3. **Lock ordering is genuinely hard.** lockdep's approach (runtime graph, detect at first occurrence) is effective but fundamentally runtime. Static lock ordering (e.g., leveled lock types) restricts flexibility. This is an unresolved design question.

4. **Value semantics help enormously — but not universally.** They reduce whole classes of aliasing
   and move-related hazards, and they make some non-intrusive designs much easier. They do not by
   themselves remove the need for safe intrusive abstractions, DMA-specific types, MMIO wrappers,
   or context-sensitive rules for shared device state.

5. **Unsafe concentration is achievable but requires massive abstraction effort.** Rust-for-Linux
   and Asterinas both show the boundary can be concentrated. They also show that doing so requires
   years of subsystem-specific abstraction work. Ori should plan for that cost, not wave it away.

6. **Current Ori is several prerequisite layers below Deep Safety.** Before negative effects or
   kernel capabilities can matter, Ori still needs end-to-end capability support, end-to-end FFI,
   at least a minimal task/context model, and AOT/backend support for all of the above.

### Recommended Next Steps

1. **Close baseline compiler gaps first** — finish positive capability propagation to callees,
   decide whether stateful handlers are required on the critical path, and add LLVM/AOT support for
   capabilities and `with...in`.

2. **Build baseline FFI and `Unsafe` end to end** — parser support already exists, but the type
   checker, evaluator/runtime, `CPtr`, codegen, and minimal ABI support do not. Deep Safety cannot
   replace blanket `Unsafe` until baseline `Unsafe` and FFI exist.

3. **Decide the concurrency baseline explicitly** — either finish a minimal `Sendable` + task
   context substrate, or scope the first proof to polling-only / single-context device code that
   avoids interrupts and deferred work. Do not leave this implicit.

4. **Design the negative-effect system** — this remains the single highest-value Deep Safety
   mechanism once the baseline exists. Without it, context rules cannot be expressed.

5. **Write capability proposals** for the 4 highest-impact capabilities:
   - `InterruptCtx` (context rules — highest value per lockdep/Klint analysis)
   - `VolatileIO` (MMIO — every driver needs it)
   - `DMA` (DMA buffer management — second-most-common unsafe category in drivers)
   - `Synchronization` (lock management — CVE-2025-68260's direct cause)

6. **Design `DmaBuffer<T>` and typed address newtypes** as library types, not language features.
   They need contracts and backend/runtime support, but not new syntax.

7. **Validate against Asterinas's OSTD** — Asterinas's TCB is the best available reference for
   what a minimal unsafe framework looks like. Compare Ori's proposed capability set against OSTD's
   actual unsafe operations to check coverage.

---

## Part 10: The Negative-Effect Solution — Boolean Effect Algebra with `without`

> **See `negative-effects-research.md` for the complete 672-line analysis of 8 approaches.**
> **Draft proposal:** `docs/ori_lang/proposals/drafts/negative-effect-without-proposal.md`
>
> **This is the single most important design decision in the deep-safety initiative.**

### 10.1 The Problem

Ori's capability system can express "this function REQUIRES capability X" (`uses X`). For kernel safety, we need "this context FORBIDS capability Y":

- Interrupt handlers cannot allocate memory (`without Allocator`)
- RCU read-side critical sections cannot sleep (`without Suspend`)
- Per-CPU access requires preemption disabled (`without Suspend`)
- Hardirq context cannot acquire sleeping locks (`without SleepingLock`)

### 10.2 Approaches Evaluated

| Approach | Can express "not X"? | Polymorphic? | Sound proof? | Composable? |
|----------|---------------------|-------------|-------------|------------|
| Pony deny caps | Yes (matrix) | No (fixed 6) | Yes | Limited |
| Koka closed rows | Indirectly | No | Yes | Poor |
| Links presence/absence flags | Yes | Yes | Yes | Medium |
| Scala 3 capture bounds | Positive only | Limited | Partial | Medium |
| Typestate | By absence | No | Yes | Poor |
| **Boolean algebra (Flix)** | **Yes (direct)** | **Yes** | **Yes (strongest)** | **Good** |
| Ocap attenuation | Positive only | N/A | Informal | Good |
| Kernel tools (Klint/sparse) | Integer ranges | No | No | Poor |

### 10.3 Recommended Design: `without` Clause

Based on the Flix/ICFP 2023 Boolean effect algebra (Lutze et al.), with the strongest theoretical guarantees of any approach:

#### Syntax

```ori
// Denial in function signature
@irq_handler () -> void
    uses InterruptCtx
    without Allocator, Suspend, SleepingLock = ...

// Polymorphic denial: "any context that doesn't allocate"
@process_fast<E> (data: [byte]) -> void
    uses E without Allocator = ...

// Context entry establishes denial — all callees inherit it
@handle_interrupt (ctx: InterruptFrame) -> void
    uses InterruptCtx
    without Allocator, Suspend, SleepingLock = {
    acknowledge_irq(ctx:)
    process_data(buffer:)  // inherits all denials
}
```

#### Semantics

1. `without X` means: within this function and all transitive callees, capability X is **forbidden**
2. Calling a function that `uses X` from `without X` context → **compile error**
3. `with X = impl in expr` inside `without X` context → **compile error** (denial cannot be overridden)
4. Denials propagate downward through call chains automatically
5. A function can add new denials but never remove them (monotonic restriction)

#### Critical Design Decision: Denial Cannot Be Overridden

The difference between capability *absence* and capability *denial*:
- **Absence**: Capability not currently available. Can be provided via `with X = impl in`.
- **Denial**: Capability explicitly forbidden. Cannot be provided. Ever. Within the denial scope.

For kernel safety, denial (not absence) is required. The whole point is that interrupt handlers CANNOT allocate, period. A `with Allocator = ... in` inside an interrupt handler must be a compile error.

#### Typing Rules (Informal)

```
[DENY-INTRO]  If f declares "without E", then E is in the denied set of f's body.
[DENY-PROP]   Calling g that "uses E" where E is denied → type error.
[DENY-INHERIT] Callees inherit caller's denied set. Callee denied = caller denied ∪ callee's own "without".
[DENY-NO-OVERRIDE] "with E = impl in expr" where E is denied → type error.
[DENY-POLYMORPHIC] If f has effect variable e "without E", instantiation of e must not include E.
```

#### Error Messages

```
error[E1260]: capability `Allocator` is denied in this context
  --> drivers/net/e1000/irq.ori:42:5
   |
42 |     let buffer = allocate(size: 1024);
   |                  ^^^^^^^^ requires `Allocator` capability
   |
note: `Allocator` is denied because this function is in interrupt context
  --> drivers/net/e1000/irq.ori:30:1
   |
30 | @handle_rx_irq () -> void
31 |     uses InterruptCtx
32 |     without Allocator, Suspend, SleepingLock = {
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ denial established here
   |
help: use pre-allocated buffers from the driver's buffer pool instead
```

#### Complementary Mechanisms

1. **Capset integration**: `capset InterruptDenials = Allocator, Suspend, SleepingLock` — reusable denial sets
2. **Context types with automatic denials**: `InterruptCtx` automatically establishes its denial set
3. **Typestate for lock protocols**: `rcu_read_lock()` transitions context, automatically adding `without Suspend`

#### Implementation Roadmap

| Phase | What | Complexity |
|-------|------|-----------|
| 1 | Parse `without` clause, represent denied set in function types | Low |
| 2 | Check denial violations (calling `uses X` from `without X`) | Low |
| 3 | Propagate denials through call chains (simple set union) | Low |
| 4 | Prevent `with X = impl in` from overriding denials | Low |
| 5 | Boolean unification for polymorphic denial inference | Medium |
| 6 | Automatic denial derivation from context types | Medium |

Phases 1-4 provide the core kernel safety mechanism with low implementation complexity. Phase 5 adds polymorphism. Phase 6 adds ergonomics.

---

## Part 11: Lock Management Without RAII & Zero-Copy Without Lifetimes

> **See `01-lock-and-zerocopy-research.md` for the complete 800-line analysis.**

### 11.1 Lock Management: Scoped APIs on `with()`

**Primary pattern**: `with_lock(mutex, body:)` — holds lock for callback duration, guaranteed release.

```ori
@with_lock<T, R> (mutex: Mutex<T>, body: (T) -> R) -> R
    uses Synchronization = {
    with(acquire: mutex.lock(), action: guard -> body(guard.data()), release: guard -> guard.unlock())
}

// Multiple locks: nesting composes naturally (LIFO release order)
with_lock(mutex_a, body: a -> {
    with_lock(mutex_b, body: b -> {
        a.value = b.value  // both locks held
    })
})
```

**Lock ordering enforcement**: Type-level lock levels with `LockBefore<L>` trait:

```ori
type Level1 = {}
type Level2 = {}
impl Level1: LockBefore<Level2>

@acquire<Current, Next, T> (lock: OrderedLock<Next, T>, held: LockToken<Current>)
    -> (LockGuard<Next, T>, LockToken<Next>)
    where Current: LockBefore<Next>
```

Out-of-order acquisition is a **compile error**.

**Lock categories via capabilities**:
- `uses Synchronization` — sleeping locks (mutex, semaphore). Cannot be held in interrupt context.
- `uses SpinLockCtx` — spinning locks. Automatically denies `Suspend`, `Allocator`, `SleepingLock` within scope.
- `uses RcuRead` — RCU read-side. Automatically denies `Suspend`.

**What kernel code actually needs**: Most functions hold 1-3 locks. Linux `MAX_LOCK_DEPTH=96` exists for pathological cases. The scoped pattern handles all common patterns.

### 11.2 Zero-Copy: Three-Layer Approach

| Layer | Mechanism | Scope | Status |
|-------|-----------|-------|--------|
| **Application** | Seamless slices | Strings, lists | Existing |
| **Kernel** | Callback-scoped views | DMA, MMIO, mmap | Proposed |
| **Hot path** | Second-class borrows (`view T`) | Function params | Future |

#### Callback-scoped views (kernel zero-copy)

```ori
@with_buffer_view<T, R> (buf: DmaBuffer<T>, body: (BufferView<T>) -> R) -> R
    uses DMA = {
    let view = buf.map_readable()
    let result = body(view)
    buf.unmap(view)
    result
}
```

`BufferView<T>` cannot escape the callback because:
1. Not `Clone` (cannot be duplicated)
2. Return type `R` constrained to `Value` (cannot contain view)
3. `with` pattern guarantees cleanup

#### What is achievable vs what requires lifetimes

| Pattern | Without lifetimes? | Mechanism |
|---------|-------------------|-----------|
| Substring views | **Yes** | Seamless slices (existing) |
| DMA buffer access | **Yes** | Callback-scoped views |
| MMIO register access | **Yes** | Callback-scoped views |
| Parameter passing without RC | **Yes** | ARC borrow inference (existing) |
| Returning a view from a function | **No** | Requires lifetimes |
| Storing a view in a struct field | **No** | Requires lifetimes |
| Iterator over borrowed data | **Partially** | Scoped OK; stored iterator needs lifetimes |

The "no" cases are rare in kernel code — all real DMA/MMIO/mmap patterns follow acquire-use-release.

---

## Part 12: Path to Static Contract Verification

> **See `static-verification-research.md` for the complete 700-line analysis.**

### 12.1 Why Ori Is a Better Verification Target Than Rust

Ori's language design makes static verification significantly easier:

| Property | Ori | Rust | Impact on verification |
|----------|-----|------|----------------------|
| Aliasing | No shared mutable refs | Complex borrow checker | Eliminates aliasing proofs |
| Evaluation | Strict | Strict | Standard WP calculus |
| Memory model | ARC (no GC, no lifetimes) | Ownership + lifetimes | Simpler heap model |
| Side effects | Explicit via capabilities | Implicit | Effect isolation simplifies proofs |
| Closures | Capture by value | Capture by reference | No closure lifetime complications |
| Self-referential types | Forbidden | Possible (Pin) | No cyclic heap reasoning |
| Mutation | Copy-on-write, value semantics | In-place mutation | Functional reasoning applies |

### 12.2 Recommended Path: Liquid Haskell Model

Start with refinement types in a decidable logic fragment, with graceful degradation to runtime checks:

**Year 1 — Refinement types on parameters**:
- Reuse existing `pre()` syntax: `pre(index >= 0 && index < len(list))`
- Abstract interpretation for simple cases (bounds, non-null, sign)
- Z3 for linear arithmetic fragment (decidable, fast)
- Target: statically eliminate bounds checks, null checks, alignment checks
- Expected compile overhead: 1-10ms per annotated function

**Year 2 — Full contract verification**:
- `post()` verification via weakest precondition calculus
- Refinement type inference (derive bounds automatically)
- `ori check --verify` command (opt-in, not blocking)
- Target: Tier 2 contract verification (runtime → static)

**Year 3 — Kernel-level verification**:
- AI-assisted annotation (Dafny's dafny-annotator achieves 86% on DafnyBench)
- Cross-function modular verification
- Capability interaction proofs (e.g., `without Allocator` is never violated)
- FFI boundary verification via bounded model checking (CBMC/Kani-style)

### 12.3 Key Numbers

| Metric | Value | Source |
|--------|-------|--------|
| Verus proof:code ratio | 5:1 (avg), 7.5:1 (kernel) | SOSP 2024/2025 |
| Prusti annotation overhead | 14% avg, 24% max | ETH Zurich |
| Liquid Haskell basic overhead | ~1 hint per 100 LoC | LiquidHaskell tutorial |
| SPARK Silver auto-proof rate | 95-98% of runtime checks | AdaCore |
| Z3 per-query target | 1-10ms for decidable fragments | Dafny benchmarks |
| Dafny verification instability | "Butterfly effect" — CI fragile | OOPSLA 2025 study |

### 12.4 SPARK Graduated Model for Ori

Inspired by SPARK/Ada's 5-level system, but adapted for Ori:

| Level | Name | What it guarantees | Annotation cost |
|-------|------|-------------------|-----------------|
| 0 | **Safe** | Type-safe, ARC-managed, capability-tracked | Zero (default) |
| 1 | **Contracted** | `pre()`/`post()` present, runtime-checked | Low (contracts on boundary functions) |
| 2 | **Verified** | Contracts statically proven (SMT/Z3) | Moderate (loop invariants, lemmas) |
| 3 | **Certified** | Full functional correctness proofs | High (proof functions, ghost code) |

Teams adopt incrementally. Levels 0-1 are accessible today. Level 2 requires Year 1-2 infrastructure. Level 3 is research-grade (far future).

The key insight from SPARK: **value at every level**. Level 0 already eliminates memory bugs. Level 1 catches contract violations at runtime. Level 2 catches them at compile time. Each level is worth the investment independently.

---

## Part 13: Concrete Architecture — The Ori Deep Safety System

### 13.1 The Complete Capability Taxonomy

**14 low-level capabilities in 4 domains + 1 blanket escape:**

#### Memory Domain
| Capability | What it gates | Proof obligation | Evidence |
|-----------|--------------|-----------------|---------|
| `VolatileIO` | Memory-mapped register access | Address in declared range | **PROPOSED** + **RUNTIME** |
| `RawMemory` | Pointer arithmetic, raw allocation | Alignment, bounds | **PROPOSED** + **RUNTIME** |
| `DMA` | DMA buffer management | Cache coherency, alignment, direction | **PROPOSED** + **RUNTIME** |

#### Context Domain
| Capability | What it gates | Automatic denials | Evidence |
|-----------|--------------|------------------|---------|
| `InterruptCtx` | Interrupt handler context | `without Allocator, Suspend, SleepingLock` | **PROPOSED** |
| `PerCpuAccess` | Per-CPU variable access | `without Suspend` | **PROPOSED** |
| `RCU` | RCU read-side critical sections | `without Suspend` | **PROPOSED** |

#### Resource Domain
| Capability | What it gates | Proof obligation | Evidence |
|-----------|--------------|-----------------|---------|
| `Allocator` | Custom memory allocation | Size/alignment | **PROPOSED** |
| `Synchronization` | Lock/unlock operations | Matched acquire/release | **PROPOSED** |
| `DeferredWork` | Workqueue/timer scheduling | Sendable closure | **PROPOSED** |
| `PageMgmt` | Page allocation and mapping | Alignment, map/unmap matching | **PROPOSED** |

#### Low-Level Domain
| Capability | What it gates | Evidence |
|-----------|--------------|---------|
| `InlineAsm` | CPU instructions | **PROPOSED** — `asm` reserved in spec |
| `StaticMut` | Mutable static state | **PROPOSED** + **RUNTIME** |
| `Transmute` | Type reinterpretation | **PROPOSED** + **RUNTIME** |

#### Blanket Escape
| Capability | What it gates | Evidence |
|-----------|--------------|---------|
| `Unsafe` | Everything above + unclassifiable | **IN SPEC** — approved proposal |

### 13.2 The Denial Matrix

Context capabilities automatically establish denial sets:

| Context | Denies Allocator | Denies Suspend | Denies SleepingLock | Denies PageMgmt |
|---------|-----------------|---------------|--------------------|--------------------|
| `InterruptCtx` | **Yes** | **Yes** | **Yes** | **Yes** |
| `RCU` (read-side) | No | **Yes** | **Yes** | No |
| `PerCpuAccess` | No | **Yes** | No | No |
| `SpinLockCtx` | No | **Yes** | **Yes** | No |
| Normal context | No | No | No | No |

### 13.3 Typed Address System

Library types (not language features), using existing newtypes + contracts:

```ori
type PhysAddr: Value, Eq, Comparable = int
type VirtAddr: Value, Eq, Comparable = int
type BusAddr: Value, Eq, Comparable = int
type UserPtr: Value = int

type MmioRegion: Value = { base: PhysAddr, size: Size }
type Register<T: Value>: Value = { region: MmioRegion, offset: int }
type DmaBuffer<T: Value + Sendable>: Value = { bus_addr: BusAddr, size: int }
```

Typed addresses prevent mixing — `PhysAddr` cannot be passed where `VirtAddr` is expected. Contracts verify bounds:

```ori
@read_register<T: Value> (reg: Register<T>) -> T
    uses VolatileIO
    pre(reg.offset >= 0 && reg.offset + size_of<T>() <= reg.region.size) = ...
```

### 13.4 Scoped Resource Pattern (Universal)

All resource management follows the same pattern — scoped access via callbacks:

```ori
// Lock + DMA buffer access composed
@process_dma_locked<R> (
    mutex: Mutex<DmaState>,
    buffer: DmaBuffer<Packet>,
    body: (DmaState, BufferView<Packet>) -> R,
) -> R uses Synchronization, DMA = {
    with_lock(mutex, body: state -> {
        with_buffer_view(buffer, body: view -> {
            body(state, view)
        })
    })
}
```

### 13.5 How a VirtIO Driver Would Look

```ori
// VirtIO network driver — illustrative sketch
use std.kernel { InterruptFrame, DmaBuffer, MmioRegion, Register }
use std.kernel.sync { Mutex, SpinLock }

type VirtQueue: Value = {
    desc_ring: DmaBuffer<VirtqDesc>,
    avail_ring: DmaBuffer<VirtqAvail>,
    used_ring: DmaBuffer<VirtqUsed>,
    notify_reg: Register<int>,
    free_head: int,
    num_free: int,
}

// Submit a buffer to the device
@virtq_submit (queue: VirtQueue, buf: DmaBuffer<[byte]>) -> VirtQueue
    uses VolatileIO, DMA
    pre(queue.num_free > 0) = {
    let idx = queue.free_head
    // Write descriptor (scoped DMA view)
    with_buffer_view(queue.desc_ring, body: descs -> {
        let desc = VirtqDesc {
            addr: buf.bus_addr,
            len: buf.size,
            flags: VIRTQ_DESC_F_NEXT,
            next: 0,
        }
        descs.write_at(index: idx, value: desc)
    })
    // Update available ring
    with_buffer_view(queue.avail_ring, body: avail -> {
        avail.write_idx(value: idx)
    })
    // Notify device (MMIO write)
    write_register(reg: queue.notify_reg, value: idx)
    { ...queue, free_head: (idx + 1) % queue_size, num_free: queue.num_free - 1 }
}

// Interrupt handler — cannot allocate, cannot sleep
@handle_rx_irq (frame: InterruptFrame, queue: VirtQueue) -> VirtQueue
    uses InterruptCtx, VolatileIO
    without Allocator, Suspend, SleepingLock = {
    // Process completions from used ring (zero-copy view)
    with_buffer_view(queue.used_ring, body: used -> {
        let completed = used.read_entries()
        // Schedule bottom-half for packet processing (deferred work)
        schedule_softirq(packets: completed)
    })
    queue
}
```

**Key observations:**
- No `unsafe` blocks anywhere — specific capabilities replace blanket trust
- `without Allocator, Suspend` in interrupt handler is a **compile-time check**
- DMA buffer access is scoped — views cannot escape callbacks
- When Ori owns the abstraction boundary, value semantics encourage non-intrusive designs
- Contracts (`pre(queue.num_free > 0)`) replace prose safety comments

---

## Part 14: Implementation Strategy & Proof of Concept

> **Draft proposals:**
> - `docs/ori_lang/proposals/drafts/capability-propagation-completion-proposal.md` (Phase 0A)
> - `docs/ori_lang/proposals/drafts/unsafe-operation-gating-proposal.md` (Phase 0B)
> - `docs/ori_lang/proposals/drafts/negative-effect-without-proposal.md` (Phase 1)

### 14.1 Implementation Phases

| Phase | Deliverable | Dependencies | Effort |
|-------|------------|-------------|--------|
| **Phase 0A** | Close current capability gaps: propagation to callees, marker-capability semantics, stateful-handler decision, LLVM/AOT support for `uses`/`with...in` | existing Section 6 work | 4-8 weeks |
| **Phase 0B** | Baseline FFI + `Unsafe`: type checking, `CPtr`, runtime/evaluator behavior, LLVM/AOT support, minimal C ABI path | roadmap Sections 6.9, 11, 21A.13 | 6-12 weeks |
| **Phase 0C** | Concurrency baseline decision: either minimal `Sendable`/task substrate or an explicitly polling-only first prototype | roadmap Section 17 or narrowed scope | 4-10 weeks |
| **Phase 1** | `without` clause prototype — parse, represent, check | Phases 0A and an agreed effect-model design | 4-8 weeks |
| **Phase 2** | First Deep Safety slice: `InterruptCtx`, `VolatileIO`, `DMA`, `Synchronization` + typed addresses + scoped APIs | Phases 0B-1 | 8-14 weeks |
| **Phase 3** | VM device proof in QEMU (prefer a narrow virtio proof before broad driver ambition) | Phase 2 | 8-16 weeks |
| **Phase 4** | Broaden capability coverage, compile-fail suite, audit packet, and implementation hardening | Phase 3 | 6-10 weeks |
| **Phase 5** | Static contract verification research prototype | separate research track, not critical path | multi-quarter |

These are best-case estimates for focused engineering work. They are intentionally longer than the
earlier schedule because the repository still lacks prerequisite capability, FFI, concurrency, and
AOT infrastructure.

### 14.2 Proof of Concept: VM NIC Driver

**Target**: Port core logic of a small VM NIC driver (virtio-net in QEMU) to Ori.

**Must demonstrate**:
1. MMIO register access through typed register abstractions
2. DMA descriptor-ring setup and update paths
3. Interrupt handler constraints + deferred work processing
4. FFI interaction via Deep FFI or a clearly-audited residual unsafe boundary
5. **Compile-fail examples** showing rejected invalid operations:
   - Allocation in interrupt context → E1260
   - Invalid register range → contract violation
   - DMA buffer type mismatch → type error
   - Sleep in RCU read-side → E1260

**Success criteria**:
- VM boots, device initializes through Ori driver
- Driver can transmit and receive packets
- The proof artifact includes a trusted-boundary map separating compile-time checks, runtime
  contracts, and any residual manual trust
- Adversarial negative cases produce either clear compile errors (for statically modeled rules) or
  explicit contract failures (for dynamic invariants)
- Zero blanket `unsafe` in driver-facing Ori code is a stretch goal, not a prerequisite for a
  first credible prototype

### 14.3 Validation Strategy

1. **Compare against Asterinas OSTD** — map Ori's 14 capabilities against OSTD's ~15K LoC of unsafe operations. Every OSTD unsafe operation should map to exactly one Ori capability.

2. **Compare against the Rust-for-Linux kernel crate** — map the unsafe footprint in
   `rust/kernel/` to Ori capabilities. Check coverage completeness rather than relying on a single
   snapshot-specific line count.

3. **Compare against CVE-2025-68260** — verify the specific failure pattern (premature lock release + unguarded list removal) would be a compile error under Ori's design.

4. **Benchmark annotation burden** — count capability annotations per function in the proof-of-concept driver. Target: <1% of code is capability annotations.

5. **External audit** — as described in Part 6. Schedule a pre-implementation design audit after
   the baseline prerequisites and negative-effect design exist, then a post-prototype audit once a
   concrete proof artifact exists.

---

## Supplementary Research Files

| File | Lines | Content |
|------|-------|---------|
| `negative-effects-research.md` | 672 | 8 approaches to effect denial; Boolean algebra solution; typing rules; error design |
| `01-lock-and-zerocopy-research.md` | 800 | 5 approaches to lock management; 5 approaches to zero-copy; kernel data |
| `static-verification-research.md` | 700 | Dafny, Creusot, Prusti, Verus, Liquid Haskell; realistic path for Ori |
| `failed-approaches.md` | 925 | 12 failed approaches; quantitative data; 12 universal design principles |

---

## Research Status Summary

| Question | Status | Answer |
|----------|--------|--------|
| Can Rust's binary unsafe be decomposed? | **SELECTED DIRECTION** | Probably yes — 14 capabilities in 4 domains is the leading design |
| Can negative effects be expressed? | **SELECTED DIRECTION** | Yes in theory — Boolean effect algebra with `without` is the strongest candidate |
| Can locks work without RAII? | **SELECTED DIRECTION** | Probably — scoped APIs on `with()` are the most plausible fit |
| Can zero-copy work without lifetimes? | **SELECTED DIRECTION** | Partially — callback-scoped views look realistic; general escaping borrows do not |
| Can contracts become static proofs? | **RESEARCH PATH IDENTIFIED** | Yes as a research direction, but not on the near-term critical path |
| What's the right capability granularity? | **WORKING HYPOTHESIS** | 14 caps, max 3-4 per domain, pending prototype validation |
| Is the safe-driver goal achievable? | **EXTERNALLY DEMONSTRATED** | Yes in purpose-built safe-Rust kernels such as Asterinas, not yet in Ori |
| What causes the most kernel bugs? | **SUPPORTED BY EVIDENCE** | Memory-safety classes remain large; context/protocol violations are high-value targets |
| LKMM compatibility? | **PARTIALLY RESOLVED** | Residual capability is the current best answer; formal interaction with Ori still needs work |

**Bottom line:** the research is strong enough to justify continued design and a narrow proof
prototype. It is not strong enough to skip prerequisite compiler work, empirical prototype
validation, or external audit.
