# Deep Safety Research: Lock Management & Zero-Copy Without Lifetimes

Research into two critical design problems for Ori's deep safety initiative.

---

## Problem 1: Lock/Resource Management WITHOUT RAII

### The Core Challenge

Ori uses value semantics with ARC. Values are conceptually copied, not moved. This breaks the
RAII guard pattern because:

1. Copying a `MutexGuard` would create two "guards" for the same lock
2. Drop timing is ARC-controlled, not scope-controlled
3. Guards require affine/linear semantics (use exactly once, destroy at scope exit)

Rust's `MutexGuard` works because: (a) move semantics prevent duplication, (b) `Drop` runs
deterministically at scope exit, (c) the borrow checker prevents the guard from escaping its scope.

None of these hold in Ori's model. The question is: what does?

---

### Approach 1: Scoped APIs / Higher-Order Functions

**Pattern**: `with_lock(mutex, body:)` where the lock is held for the duration of the callback.

```ori
// Ori syntax
with_lock(mutex, body: data -> {
    data.count += 1
    data.name = "updated"
})
```

**Languages using this pattern:**

- **Go**: `sync.Mutex` + `defer mu.Unlock()`. Not truly scoped — `defer` runs at function exit,
  not block exit, so lock is held longer than necessary. Multiple locks use nested defers with
  manual ordering. No compiler enforcement of ordering.

- **Zig**: `m.lock(); defer m.unlock();` — same defer-at-function-exit pattern as Go. Explicit,
  visible, but no compiler-enforced pairing.

- **Java**: `synchronized(obj) { ... }` — block-scoped, but only for a single monitor. Multiple
  monitors require nested `synchronized` blocks with manual ordering discipline.

- **Koka**: Effect handlers with `finally` clauses provide guaranteed cleanup. Scoped handlers
  use polymorphic scope variables to prevent the resource from escaping:
  ```koka
  fun with-file(path, action)
    val f = open(path)
    finally({ close(f) }) { action(f) }
  ```
  The scope polymorphism (`forall<s>. (file<s>) -> e a`) ensures `f` cannot escape `action`.

- **Ori's `with(acquire:, action:, release:)`**: Already exists in the language. The `release`
  expression **always executes** if `acquire` succeeds, regardless of panic, error propagation,
  or early exit. This is the natural foundation for lock management.

**How Ori's `with...in` capability system handles this:**

```ori
// Scoped lock using existing with() pattern
@with_lock<T, R> (mutex: Mutex<T>, body: (T) -> R) -> R
    uses Synchronization = {
    with(
        acquire: mutex.lock(),
        action: guard -> body(guard.data()),
        release: guard -> guard.unlock(),
    )
}
```

**Multiple locks / nesting:**

Nested scoped APIs compose naturally:

```ori
with_lock(mutex_a, body: data_a -> {
    with_lock(mutex_b, body: data_b -> {
        // Both locks held
        data_a.value = data_b.value
    })
})
```

This is verbose but **safe by construction** — locks are released in reverse acquisition order
because the callbacks unwind in stack order. This is identical to how C++ `std::scoped_lock`
works, but enforced structurally rather than by destructor ordering.

**Lock ordering enforcement:**

The scoped pattern does NOT automatically enforce ordering. Two call sites could acquire
`(a, b)` and `(b, a)` respectively. Solutions:

1. **Type-level lock ordering** (from Fuchsia/Rust `lock_ordering` crate): Define lock levels as
   types with a `LockBefore<L>` trait relationship. Each acquisition requires a proof that the
   current held level precedes the requested level. In Ori:

   ```ori
   // Type-level lock ordering
   type Level1 = {}
   type Level2 = {}
   type Level3 = {}

   // Lock levels form a total order
   impl Level1: LockBefore<Level2>
   impl Level2: LockBefore<Level3>

   // Acquiring lock requires proof of correct ordering
   @acquire<Current, Next, T> (
       lock: OrderedLock<Next, T>,
       held: LockToken<Current>,
   ) -> (LockGuard<Next, T>, LockToken<Next>)
       where Current: LockBefore<Next>
   ```

   This makes out-of-order acquisition a **compile error**. The pattern was formalized in
   "A Type System for Preventing Data Races and Deadlocks in Java Programs" (Boyapati et al.)
   and implemented in Fuchsia's lock-ordering library.

2. **Multi-lock acquisition function**: Provide a `with_locks(a, b, body:)` that sorts by
   lock identity (pointer address or assigned ID) before acquiring, similar to
   `std::scoped_lock(mutex1, mutex2)` in C++17.

   ```ori
   @with_locks_2<A, B, R> (
       lock_a: Mutex<A>,
       lock_b: Mutex<B>,
       body: (A, B) -> R,
   ) -> R uses Synchronization
   ```

**Verdict**: Scoped APIs are the primary pattern for Ori. The `with()` built-in already provides
the foundation. Lock ordering can be layered on via the type system.

---

### Approach 2: Linear/Affine Types for Resources

**Key languages:**

- **Austral**: Linear types denoted with `!` (e.g., `File!`). Values must be used exactly once.
  Resource threading pattern:
  ```austral
  let f: File! := openFile("test.txt");
  let f1: File! := writeString(f, "First line");
  closeFile(f1);
  ```
  Each operation consumes the old handle and returns a new one. The compiler counts variable
  uses and rejects programs where a linear value is used zero times (leak) or more than once
  (double-use). Austral borrows Rust's borrow syntax for temporary non-linear views within
  scoped contexts.

- **Vale**: "Higher RAII" — linear structs must be explicitly destroyed by specific approved
  functions. Unlike traditional RAII (single zero-argument destructor), Vale allows multiple
  named destructors with parameters and return values:
  ```vale
  linear struct Transaction { ... }
  func commit(txn Transaction, db &DB) { destruct txn; }
  func rollback(txn Transaction, db &DB) { destruct txn; }
  ```
  The only way to get rid of a `LockGuard` is to hand it to `unlock()`. This is
  compiler-enforced. Downside: linearity is "upwardly viral" — linear members force containing
  types to be linear.

- **Clean**: Uniqueness types ensure single-owner semantics. A unique value can be mutated
  in-place because no other reference exists. This is conceptually similar to Ori's ARC
  uniqueness optimization but enforced statically.

**How linear types compose with value semantics:**

The fundamental tension: value semantics means values can be copied, but linear types must NOT
be copied. These are contradictory.

Resolution options:
1. **Dual type universe**: Some types are `Value` (copyable), others are `Linear` (not copyable).
   Austral does this. The cost is a split type system.
2. **Scoped linearity**: Types are normally copyable but become temporarily linear within a
   `focus` scope (Vault pattern — see below).
3. **Capability-gated linearity**: Linear behavior is an effect. `uses LinearResource` marks
   functions that handle non-copyable resources. This fits Ori's capability model.

**For Ori**: Full linear types would be a massive language change — a second kind of type that
doesn't follow value semantics. The scoped API approach (Approach 1) achieves the same safety
guarantees without bifurcating the type system. Linear types are **not recommended** as the
primary mechanism.

However, a **limited linear capability** could work: certain capability-gated types (locks,
DMA buffers) could be marked as non-copyable within their `with` scope. The type checker would
reject attempts to copy them. This is similar to Vault's focus pattern.

---

### Approach 3: Region-Based Resource Management

**MLKit regions**: The ML Kit compiler divides the heap into regions. Objects are allocated into
regions. Regions are deallocated as a unit (stack discipline). Region inference is automatic.
This works well for memory but has limitations for locks:

- Regions follow LIFO (stack) discipline — a lock's lifetime is fixed at allocation time
- Cannot extend a lock's lifetime based on runtime conditions
- Cannot transfer a lock across region boundaries
- Combining regions with non-LIFO resources (locks that must be released in arbitrary order)
  is unresolved

**Cyclone regions**: Extended ML regions with region subtyping and integration with stack
allocation. Limitations that caused Cyclone to fail:

1. **LIFO constraint**: Object lifetime fixed at allocation. Subsequent computation cannot
   shorten or extend it. This is fatal for locks, which need acquire/release at arbitrary
   points within a function.
2. **Annotation burden**: Separate compilation required explicit region annotations.
3. **Performance for small objects**: Region overhead (creation + deallocation) makes them
   expensive for single objects like lock guards.
4. **No 64-bit support**: Reference tooling never supported 64-bit platforms.

Cyclone's developers explicitly note that Rust integrated many of the same ideas more
successfully.

**For Ori**: Regions are not suitable for lock management. The LIFO constraint directly conflicts
with the flexibility kernel code needs (e.g., acquiring lock A, then B, then releasing A while
keeping B). Scoped APIs already provide the "region-like" scoping without the LIFO limitation.

---

### Approach 4: Typestate for Lock Protocols

**Plaid language**: Objects have typestates that change over time. The type system tracks these
changes. A lock has states `Unlocked` and `Locked`. Acquiring transitions
`Unlocked -> Locked`, releasing transitions `Locked -> Unlocked`. The compiler rejects:
- Double-lock (already in `Locked` state)
- Use-without-lock (in `Unlocked` state)
- Forgetting to unlock (linear obligation to transition back)

Plaid uses permissions (unique, immutable, shared) to control access. Concurrency is
"by default" — the runtime executes operations concurrently up to permission constraints.

**Session types for locks**: Model lock acquire/release as a protocol:
```
Lock = !Acquire.?GuardToken.!Release.Lock
```
The `acquire-release` discipline in shared session types enforces mutual exclusion. Deadlock
prevention uses a partial order on resources — locks must be acquired in ascending order.

**Vault language (Microsoft Research, 2002)**: Combined typestate with linear types via two key
constructs:

- **Adoption**: Temporarily allows aliases to a linear object. The linear object "adopts"
  non-linear children.
- **Focus**: Temporarily provides a linear view of a non-linear (aliased) object. Within the
  focus scope, all other aliases are invalidated. Guards prevent access to potential aliases.

This is directly relevant to Ori: `focus` is essentially a scoped borrow that provides
temporary exclusive access without a full lifetime system.

**For Ori**: Pure typestate is too complex for the general case, but Ori can incorporate
typestate-like patterns through its capability system:

```ori
// Typestate-inspired lock protocol via capabilities
@critical_section<T, R> (mutex: Mutex<T>, body: (LockedView<T>) -> R) -> R
    uses Synchronization = {
    // LockedView<T> is only constructible inside this function
    // It cannot escape because it's not Clone and the callback is scoped
    let view = mutex.acquire()
    let result = body(view)
    mutex.release(view)
    result
}
```

The `LockedView<T>` type acts as a typestate token — proof that the lock is held. It exists
only within the callback scope.

---

### Approach 5: What Kernel Code Actually Needs

**How many locks does a typical kernel function hold simultaneously?**

- Linux kernel's `MAX_LOCK_DEPTH` has been increased over time: 30 -> 40 -> 48 -> 96.
  Currently set to 96 (the maximum number of locks a single task can hold at once).
- Typical desktop systems have fewer than 1,000 lock classes total (`MAX_LOCKDEP_KEYS`
  defaults to 8,191).
- **In practice**: Most kernel functions hold 1-3 locks. Complex paths (filesystem, networking)
  may hold 4-6. The 96-deep limit exists for pathological paths through the scheduler and
  memory allocator. The Linux locking guide states: "the best locks are encapsulated: they
  never get exposed in headers, and are never held around calls to non-trivial functions
  outside the same file."

**Lock ordering in Linux (lock dependency classes):**

Lockdep tracks lock ordering via dependency graphs:
- Same lock-class must not be acquired twice (no recursive locking)
- No inverse ordering: if L1->L2 appears anywhere, L2->L1 must never appear
- Hardirq-safe locks cannot depend on hardirq-unsafe locks
- Softirq-safe locks cannot depend on softirq-unsafe locks

These are **runtime checks** in Linux. Ori's capability system could make many of them
**compile-time checks**.

**Interrupt-safe locks vs. sleeping locks:**

Three categories:
1. **Sleeping locks**: `mutex`, `rt_mutex`, `semaphore`, `rw_semaphore` — can only be held in
   preemptible task context. May sleep on contention.
2. **Spinning locks**: `spinlock_t`, `raw_spinlock_t` — disable preemption. Cannot sleep,
   allocate memory, or call most kernel functions while held.
3. **CPU-local locks**: `local_lock` — per-CPU, prevent preemption on the local CPU.

Nesting rules:
- Sleeping cannot nest inside spinning or CPU-local
- Spinning can nest inside anything
- CPU-local can nest inside sleeping

On PREEMPT_RT kernels, `spinlock_t` is remapped to a sleeping lock based on `rt_mutex`.
Only `raw_spinlock_t` remains truly non-preemptible.

**For Ori**: These categories map directly to capabilities:

```ori
// Capability-enforced lock categories
capset InterruptContext = InterruptCtx
capset ProcessContext = Allocator, Suspend

// Sleeping lock — requires process context
@with_mutex<T, R> (m: Mutex<T>, body: (T) -> R) -> R
    uses Synchronization
// Cannot be called from interrupt context because Synchronization
// is incompatible with InterruptCtx

// Spinlock — compatible with interrupt context but restricts body
@with_spinlock<T, R> (s: SpinLock<T>, body: (T) -> R) -> R
    uses SpinLockCtx
// body cannot use Allocator, Suspend, or any sleeping lock
```

**RCU as lock-free alternative:**

RCU (Read-Copy-Update) provides lock-free read access to shared data:
- Readers enter/exit RCU critical sections (zero overhead on non-preemptive kernels)
- Writers create a new version, atomically swap the pointer, then wait for all readers to finish
  before freeing the old version
- Over 9,000 uses in the Linux kernel (as of 2014)
- Standardized in C++26 (P2545R4)

RCU maps well to Ori's value semantics — readers get an immutable snapshot, writers create
a new value. The "grace period" waiting could be modeled as a capability effect:

```ori
@rcu_read<T, R> (data: RcuProtected<T>, body: (T) -> R) -> R
    uses RcuRead = {
    let snapshot = data.read_lock()
    let result = body(snapshot)
    data.read_unlock()
    result
}

@rcu_update<T> (data: RcuProtected<T>, new_value: T) -> void
    uses RcuWrite = {
    let old = data.swap(new_value:)
    rcu_synchronize()  // Wait for all readers to finish
    drop_early(value: old)
}
```

**Can Ori's `with...in` naturally handle nested locks?**

Yes. The callback structure enforces LIFO ordering:

```ori
with Synchronization = kernel_sync in {
    with_lock(lock_a, body: a -> {
        // lock_a held
        with_lock(lock_b, body: b -> {
            // lock_a and lock_b held
            // lock_b released first (inner callback returns first)
        })
        // only lock_a held
    })
    // no locks held
}
```

This is exactly the correct LIFO ordering. Non-LIFO patterns (release A while keeping B)
would require a different API — but such patterns are rare in well-structured kernel code
and are often a design smell even in C.

---

### Recommendation for Ori

**Primary mechanism: Scoped APIs built on `with()`**

1. `with_lock(mutex, body:)` — holds lock for callback duration
2. `with_locks(a, b, body:)` — multi-lock with deterministic ordering
3. Lock categories enforced via capabilities (`uses SpinLockCtx` prohibits sleeping)
4. Lock ordering enforced via type-level lock levels (compile-time deadlock prevention)
5. RCU provided as a lock-free primitive for read-heavy patterns

**No RAII guards, no linear types, no regions.** The scoped API is simpler, safer (guaranteed
release), and fits naturally into Ori's existing `with()` and capability infrastructure.

---

## Problem 2: Zero-Copy WITHOUT Lifetimes

### The Core Challenge

Kernel code needs guaranteed zero-copy views:
- DMA buffer views: device writes to physical memory, CPU reads from a mapped view
- SKB data pointers: network packet data accessed without copying
- Memory-mapped device registers: volatile reads/writes to fixed addresses
- File page cache: mmap'd file pages shared between kernel and userspace

Ori has no lifetime system. References are always copies (value semantics). How do you have
a "view" into data without copying it and without lifetimes to prevent the view from outliving
the source?

---

### Approach 1: Ori's Seamless Slices — How Far Do They Go?

**Current implementation:**

Ori's seamless slices are zero-copy views into list/string buffers:
- The `SLICE_FLAG` (bit 63 of `cap`) marks a value as a slice
- A slice's `data` pointer points **into** another allocation's data region
- The slice increments the original allocation's RC (the slice is an additional reference)
- Slice-of-slice accumulates offsets to always reference the true original allocation
- Mutations on a slice trigger materialization (copy-on-write)

This is directly inspired by Roc's seamless slices and Go's slice model.

**What they handle well:**
- Substring views: `str.substring(start:, end:)` returns zero-copy slice
- List slicing: `list.slice(start, end)` returns zero-copy view
- Iterator `.take()` / `.skip()` return slices
- No observable difference between slice and owned value (by design)

**Limitations for kernel zero-copy:**

1. **ARC overhead**: Every slice increments RC on the original buffer. In kernel hot paths
   (networking, block I/O), RC operations may be unacceptable.

2. **Cannot represent hardware memory**: A seamless slice must point into an ARC-managed
   allocation. DMA buffers, MMIO regions, and mmap'd pages are NOT ARC-managed — they're
   managed by the hardware or kernel memory subsystem.

3. **COW on mutation**: If the slice is the only reference, mutation is in-place. But if shared,
   mutation triggers a copy. Kernel code often needs guaranteed in-place writes to device
   memory (MMIO), where COW would be catastrophically wrong.

4. **Slice cannot outlive source**: Slices hold an RC reference to the source, so the source
   cannot be freed while slices exist. This is correct for memory safety but means the source
   buffer stays alive as long as any slice exists. In kernel contexts, this could prevent
   timely buffer reclamation.

**Verdict**: Seamless slices work for application-level zero-copy (substrings, list views) but
are **insufficient for kernel-level zero-copy** where hardware memory, MMIO regions, and DMA
buffers are involved.

---

### Approach 2: Region-Scoped Borrows (Without Full Lifetimes)

**Core idea**: Provide zero-copy views that are scope-limited without a full lifetime system.
The view exists only within a callback scope and cannot escape.

```ori
@with_buffer_view<T, R> (
    buf: DmaBuffer<T>,
    body: (BufferView<T>) -> R,
) -> R uses DMA = {
    let view = buf.map_readable()
    let result = body(view)
    buf.unmap(view)
    result
}
```

`BufferView<T>` provides read access to the buffer's contents without copying. It cannot
escape the callback because:
1. It is not `Clone` (cannot be duplicated)
2. The callback's return type `R` cannot contain `BufferView<T>` (enforced by a trait bound
   like `R: Value` or a negative bound `R: !ContainsBorrow`)
3. The `with` pattern guarantees `unmap` runs after the callback

**Hylo's subscript/projection model:**

Hylo (formerly Val) achieves zero-copy through **subscripts** — functions that `yield` a value
rather than `return` it:

```hylo
subscript view(_ buf: Buffer): Data {
    yield buf.mapped_data
    // cleanup runs after caller finishes with yielded value
}
```

The yielded value is a **projection** — the caller gets temporary access without ownership.
The subscript resumes after the caller is done, running cleanup code. This is a coroutine-based
scoped borrow. Key insight: the scope boundary is determined by control flow, not lifetime
annotations.

Hylo has four subscription modes:
- `let`: immutable projection (shared read)
- `inout`: mutable projection (exclusive write)
- `set`: write-only (initialize without reading current value)
- `sink`: consuming (return, not project)

This maps well to kernel access patterns:
- `let` = read DMA buffer
- `inout` = write MMIO register
- `set` = initialize buffer before DMA transfer

**Lean 4's borrowed parameters:**

Lean 4 marks parameters as `@& T` (borrowed). The compiler does not increment RC when passing
borrowed values. The callee cannot store the value or pass ownership — it can only read. This
is a **second-class reference**: it exists only as a parameter-passing mode, not as a
storable type.

Ori already does something similar through ARC borrow inference — parameters classified as
`Borrowed` skip RC operations. Extending this to provide explicit zero-copy views is natural.

**For Ori**: This is the recommended approach. Region-scoped borrows via callback APIs provide
zero-copy without lifetimes. The key constraint is: **borrowed views cannot escape their
scope**.

---

### Approach 3: Mutable Value Semantics (MVS) — The Hylo/Swift Path

**The MVS model** (Racordon et al., 2022):

- References are **second-class**: they can exist only as function parameters (`in`, `inout`),
  never stored in variables or fields
- The compiler verifies exclusivity at function boundaries
- No lifetime annotations needed because borrows cannot escape function calls
- Copy-on-write for dynamically sized containers
- Fixed-size values live on the stack

**Swift's Law of Exclusivity**:

If an access to a variable is in progress, no other access to the same variable may overlap
unless both are reads. `inout` parameters get exclusive access; `borrowing` parameters get
shared access. Enforced both statically (for local variables) and dynamically (for properties
accessed through computed paths).

**How MVS achieves zero-copy:**

```swift
// Swift: inout gives exclusive zero-copy access
func process(data: inout [Int]) {
    data[0] = 42  // In-place mutation, no copy
}

var buffer = [1, 2, 3]
process(data: &buffer)  // Zero-copy — buffer is mutated in place
```

The `inout` parameter is not a copy — it's a scoped exclusive borrow. The compiler ensures
no other access to `buffer` occurs during the call.

**Ori's `self` in methods already works this way:**

From the Ori syntax reference: "self (mutable in methods — mutations propagate to caller)".
When a method receives `self`, mutations to `self` are visible to the caller. This is
effectively an implicit `inout` parameter.

**Extending to explicit projections:**

Ori could add an explicit `view` or `borrow` parameter mode:

```ori
// Hypothetical: explicit borrowed parameter
@process_buffer (data: view [int]) -> void = {
    // data is a zero-copy view, cannot be stored or returned
    let sum = data.fold(initial: 0, op: (a, b) -> a + b)
    print(msg: `Sum: {sum}`)
}
```

The `view` parameter mode means:
1. No RC increment on entry
2. No RC decrement on exit
3. The parameter cannot be stored in a field or returned
4. The parameter cannot be captured by a closure that escapes

This is exactly Lean 4's `@&` / Hylo's `let` subscript / Swift's `borrowing` — second-class
references that exist only at function call boundaries.

---

### Approach 4: Memory-Mapped Views in Safe Languages

**Java's `MappedByteBuffer`:**

`MappedByteBuffer` wraps `mmap()` to provide direct access to file data:
- Zero-copy: reads go directly to page cache
- Safety issues: no way to explicitly unmap. The only way to request unmapping is `System.gc()`.
  Using the buffer after unmap crashes the JVM.
- Size limited to 2GB (int indexing)
- Known bug (JDK-4724038) since 2002: no `unmap()` method

**.NET's `MemoryMappedFile`:**

Provides `CreateViewAccessor()` for mapped regions. Getting direct pointer access requires
unsafe code (`AcquirePointer` / `ReleasePointer`). There is an active proposal (dotnet/runtime
#57330) for safe `Span<T>` access, but the core challenge remains: preventing the `Span` from
being used after the underlying file mapping is disposed.

**The fundamental problem in GC'd languages**: The GC controls object lifetime, but
memory-mapped regions have externally-controlled lifetime (the file mapping). There's a
mismatch between GC-managed references and OS-managed memory.

**For Ori**: ARC has the same mismatch. A memory-mapped region's lifetime is controlled by the
OS, not by RC. The solution is the same as for locks: **scoped APIs**.

```ori
@with_mmap_view<R> (
    file: FileHandle,
    offset: int,
    length: int,
    body: (MmapView) -> R,
) -> R uses FileSystem = {
    let view = mmap(file, offset, length)
    let result = body(view)
    munmap(view)
    result
}
```

`MmapView` is only valid within the callback. It cannot escape. This is strictly safer than
Java's approach (where the `MappedByteBuffer` can leak) and doesn't require unsafe code
like .NET.

---

### Approach 5: The Fundamental Tension

**Can you have both value semantics AND zero-copy?**

The MVS research (Racordon et al.) answers this definitively: **yes, with second-class
references.** The key insight is that references do not need to be first-class (storable,
returnable) to be useful. If references exist only at function call boundaries (as parameter
modes), the compiler can verify exclusivity and prevent dangling without lifetime annotations.

**What subset of zero-copy patterns are achievable without lifetimes?**

| Pattern | Achievable? | Mechanism |
|---------|-------------|-----------|
| Substring views | Yes | Seamless slices (existing) |
| List slice views | Yes | Seamless slices (existing) |
| Parameter passing without RC | Yes | Borrow inference (existing ARC) |
| Scoped DMA buffer access | Yes | Callback-scoped views |
| Scoped MMIO access | Yes | Callback-scoped views |
| Scoped mmap access | Yes | Callback-scoped views |
| Returning a view from a function | **No** | Requires lifetimes |
| Storing a view in a struct field | **No** | Requires lifetimes |
| Iterator over borrowed data | **Partially** | Scoped iteration OK; stored iterator needs lifetimes |
| Zero-copy deserialization | **Partially** | Scoped parsing OK; persisted parsed data needs copy |

The "no" cases are the fundamental limitation of second-class references. They require either:
(a) full lifetime system, (b) arena/region allocation where the arena outlives all references,
or (c) accepting a copy at the boundary.

**Is there a "scoped borrow" that doesn't import full lifetimes?**

Yes. Multiple approaches have been demonstrated:

1. **Callback-scoped borrows** (Koka, Ori's `with()`): The borrow exists only within a
   callback. The type system prevents escape by restricting the callback's return type.

2. **Second-class references** (Hylo, Swift `inout`/`borrowing`): References exist only as
   parameter modes. Cannot be stored or returned.

3. **Subscript projections** (Hylo): Coroutine-based yields that provide temporary access.
   The compiler verifies that the yielded reference doesn't escape.

4. **Place-based borrow checking** (Niko Matsakis, "Borrow Checking Without Lifetimes"):
   Instead of abstract lifetime variables, track concrete "places" (variable paths like
   `a.b.c`). The compiler computes which places are "live" (potentially used later) and
   prevents mutation of borrowed places. This provides Rust-level safety without explicit
   `'a` annotations, though the analysis is similarly complex internally.

All of these work for Ori's needs. The callback pattern is simplest and already exists.
Second-class references could be added as an optimization for hot paths.

---

### What Kernel Code Actually Needs

**DMA buffer patterns:**

The Linux dma-buf framework enables zero-copy sharing between kernel drivers:
- `dma_buf_map_attachment()` → scoped access to buffer
- `dma_buf_unmap_attachment()` → release access
- Always paired acquire/release — naturally maps to callback pattern

**Network buffer (SKB) patterns:**

SKB data access in Linux:
- `skb->data` points to packet data (potentially DMA'd)
- `skb_header_pointer()` returns a view into packet headers
- Views are always scoped to the current processing function

**MMIO patterns:**

- `ioremap()` → `readl()`/`writel()` → `iounmap()`
- Always scoped to the driver's lifetime
- Volatile access is inherently scoped (no caching)

**Common pattern**: All three follow acquire-use-release. The "use" phase is always bounded.
This maps directly to Ori's callback-scoped view pattern.

---

### Recommendation for Ori

**Three-layer approach:**

1. **Seamless slices** (existing): Application-level zero-copy for strings and lists.
   No changes needed.

2. **Callback-scoped views**: For DMA buffers, MMIO, mmap, and other hardware resources.
   Built on `with()`. The view type cannot escape the callback. Enforced by:
   - The view type not implementing `Clone`
   - The callback's return type constrained (e.g., `R: Value`)
   - The capability system tracking which views are active

3. **Second-class borrowed parameters** (future optimization): For hot-path functions where
   the callback overhead is measurable. Parameter mode `view T` or `borrow T` that:
   - Skips RC increment/decrement (already done by ARC borrow inference)
   - Is explicitly non-storable and non-returnable
   - Provides compiler-verified zero-copy access
   - Modeled after Lean 4's `@&`, Hylo's `let` subscript, Swift's `borrowing`

**No full lifetime system.** The scoped patterns cover all kernel zero-copy needs. The
cases that require first-class references (returning views, storing views in fields) are
rare in kernel code and can use owned copies at the boundary.

---

## Cross-Cutting Design: How Both Solutions Compose

Both lock management and zero-copy views share the same fundamental pattern: **scoped access
via callbacks**. This is not coincidental — both are instances of the same problem: temporary
access to a resource that has externally-managed lifetime.

```ori
// Combined pattern: hold lock AND access DMA buffer
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

The capability system ensures:
- `Synchronization` capability is required for lock operations
- `DMA` capability is required for buffer access
- `SpinLockCtx` prohibits sleeping operations within spinlock-held scopes
- `InterruptCtx` prohibits sleeping locks and memory allocation

The scoped API pattern is the unifying abstraction. It works for locks, DMA buffers, MMIO
regions, mmap views, RCU critical sections, and any other acquire-use-release resource.

---

## Summary

| Design Decision | Recommendation | Rationale |
|----------------|---------------|-----------|
| Lock management | Scoped APIs on `with()` | Already exists, guaranteed release, composable |
| Lock ordering | Type-level lock levels | Compile-time deadlock prevention |
| Lock categories | Capabilities | `uses SpinLockCtx` prevents sleeping |
| Linear types for locks | No | Bifurcates type system; scoped APIs achieve same safety |
| Regions for locks | No | LIFO constraint too restrictive |
| Zero-copy (app-level) | Seamless slices | Already exists, transparent |
| Zero-copy (kernel) | Callback-scoped views | Same pattern as locks; cannot escape scope |
| Zero-copy (hot path) | Second-class borrows | Future: `view T` parameter mode |
| Full lifetime system | No | Scoped patterns cover kernel needs |
| RCU | Capability-tracked primitive | Natural fit for value semantics |

---

## Sources

### Lock Management
- [Introducing Austral](https://borretti.me/article/introducing-austral)
- [Austral Linear Types Tutorial](https://austral-lang.org/tutorial/linear-types)
- [Higher RAII and Linear Types (Vale)](https://verdagon.dev/blog/higher-raii-uses-linear-types)
- [Plaid Programming Language](https://www.cs.cmu.edu/~aldrich/plaid/plaid-intro.pdf)
- [Adoption and Focus (Vault)](https://www.microsoft.com/en-us/research/publication/adoption-and-focus-practical-linear-types-for-imperative-programming/)
- [Linux Kernel Lock Types](https://docs.kernel.org/locking/locktypes.html)
- [Linux Kernel Lockdep Design](https://docs.kernel.org/locking/lockdep-design.html)
- [Linux Kernel Locking Guide](https://docs.kernel.org/kernel-hacking/locking.html)
- [Lock Ordering (Linus Torvalds)](https://yarchive.net/comp/linux/lock_ordering.html)
- [Compile-Time Lock Ordering in Rust (lock_ordering crate)](https://docs.rs/lock_ordering/latest/lock_ordering/)
- [Compile-Time Lock Ordering in Rust (locks crate)](https://github.com/YurBoiRene/locks)
- [Koka Effect Handlers](https://koka-lang.github.io/koka/doc/book.html)
- [Algebraic Effect Handlers with Resources](https://www.microsoft.com/en-us/research/wp-content/uploads/2018/04/resource-v1.pdf)
- [Session Types for Deadlock Freedom](https://www.cs.cmu.edu/~fp/papers/esop19.pdf)
- [Go Mutex Patterns](https://oneuptime.com/blog/post/2026-01-23-go-mutex/view)
- [Zig Defer Patterns](https://matklad.github.io/2024/03/21/defer-patterns.html)
- [Fuchsia Runtime Lock Validation](https://fuchsia.dev/fuchsia-src/concepts/kernel/lockdep-design)
- [RCU on Wikipedia](https://en.wikipedia.org/wiki/Read-copy-update)
- [RCU in Linux Kernel](https://www.kernel.org/doc/Documentation/RCU/whatisRCU.txt)

### Zero-Copy
- [Mutable Value Semantics (Racordon et al.)](https://www.jot.fm/issues/issue_2022_02/article2.pdf)
- [Second-Class References (Borretti)](https://borretti.me/article/second-class-references)
- [Hylo Language Subscripts](https://docs.hylo-lang.org/language-tour/subscripts)
- [Hylo Language Overview](https://hylo-lang.org/)
- [Ruminating About Mutable Value Semantics](https://www.scattered-thoughts.net/writing/ruminating-about-mutable-value-semantics/)
- [Swift Exclusivity Enforcement](https://www.swift.org/blog/swift-5-exclusivity/)
- [Swift Ownership Manifesto](https://github.com/swiftlang/swift/blob/main/docs/OwnershipManifesto.md)
- [Lean 4 FFI and Borrowed Parameters](https://gist.github.com/ydewit/7ab62be1bd0fea5bd53b48d23914dd6b)
- [Borrow Checking Without Lifetimes (Niko Matsakis)](https://smallcultfollowing.com/babysteps/blog/2024/03/04/borrow-checking-without-lifetimes/)
- [Memory Safety Without Lifetime Parameters (Safe C++)](https://safecpp.org/draft-lifetimes.html)
- [Roc Language](https://www.roc-lang.org/)
- [Cyclone Regions](https://cyclone.thelanguage.org/wiki/Introduction%20to%20Regions/)
- [Cyclone Region Paper](https://www.cs.umd.edu/projects/cyclone/papers/cyclone-regions.pdf)
- [MLKit Region-Based Memory Management](https://www.semanticscholar.org/paper/Region-based-Memory-Management-Tofte-Talpin/9117c75f62162b0bcf8e1ab91b7e25e0acc919a8)
- [DMA Buffer Sharing (Linux)](https://docs.kernel.org/driver-api/dma-buf.html)
- [Java MappedByteBuffer](https://howtodoinjava.com/java/nio/memory-mapped-files-mappedbytebuffer/)
- [.NET Safe Memory-Mapped Files Proposal](https://github.com/dotnet/runtime/issues/57330)
- [Zero-Copy in Rust (Manish Goregaokar)](https://manishearth.github.io/blog/2022/08/03/zero-copy-1-not-a-yoking-matter/)
- [Uniqueness Typing for Resource Management](https://www.arxiv.org/abs/1003.5513v1)
