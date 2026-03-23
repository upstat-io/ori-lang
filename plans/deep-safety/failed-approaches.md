# Deep Safety Research: Failed and Troubled Safety Approaches

Research into programming language safety approaches that failed, stalled, or had serious
adoption/design problems. Compiled for the Ori compiler's deep-safety initiative, which aims
to replace Rust's binary `unsafe` with graduated capabilities for kernel/systems programming.

---

## 1. Cyclone — Region-Based Memory Safety for C

**Period**: 2001-2006 (active), abandoned ~2008
**Authors**: Dan Grossman (UW), Greg Morrisett (Cornell), Trevor Jim (AT&T Labs)
**Goal**: Safe dialect of C with region-based memory management

### What It Was

Cyclone introduced three pointer types to eliminate C's memory unsafety:
- **Thin pointers** (`*`/`@thin`): single machine word, no bounds info
- **Fat pointers** (`?`/`@fat`): three-word structure with bounds, enabling safe arithmetic
- **Null-safe pointers** (`@`/`@notnull`): compile-time verified non-null

Region annotations tracked pointer lifetimes. Functions were region-polymorphic:
`fact<rho>(int*rho result, int n)`. Flow-sensitive analysis tracked region liveness
via capabilities (access rights) and effects (region modifications).

### The Annotation Burden (Quantified)

- Porting legacy C to Cyclone required altering **~8% of total code**
- Of those changes, only **6% of the 8%** (i.e., ~0.48% of total code) were region annotations
- Boa web server port: modifications to only **~5% of codebase**
- Default annotations + local type inference handled most cases automatically

This is actually a surprisingly low burden. The annotation problem was not catastrophic.

### Why It Actually Failed

The annotation burden was NOT the primary killer. The real problems were:

1. **Fat pointers broke kernel/systems use**: Fat pointers required 3 words per pointer.
   Kernel code needs to preserve data structure layouts where pointers are 1 word. This
   made Cyclone unsuitable for its target domain (systems programming).

2. **LIFO region limitation**: Object lifetime is fixed at allocation time. Subsequent
   computation cannot extend or shorten it. This made many common patterns impossible.

3. **No 64-bit support**: The reference tooling never supported 64-bit platforms.

4. **Research goals met**: The core research project finished. Developers moved on.
   Several ideas made their way into Rust.

### Failure Type: Contingent (wrong execution + timing)

The ideas were sound. The implementation hit practical walls (fat pointer layouts,
platform support). Rust absorbed the good ideas (ownership, lifetimes, non-null references)
and avoided the bad (fat pointers for all bounds checking, LIFO regions).

### Lesson for Ori

- Keep pointer/reference representations as thin as possible. Safety metadata should
  be tracked at compile time, not as runtime fat pointers.
- Region-based lifetimes work but LIFO is too restrictive. Ori's ARC-based approach
  avoids this entirely.
- ~0.5% annotation burden is the target to beat. If Ori's capability annotations exceed
  this, we have a problem.

**Sources**:
- [Region-Based Memory Management in Cyclone (PLDI 2002)](https://www.cs.umd.edu/projects/cyclone/papers/cyclone-regions.pdf)
- [Cyclone: A Type-Safe Dialect of C](https://homes.cs.washington.edu/~djg/papers/cyclone-cuj.pdf)
- [Safe Manual Memory Management in Cyclone](https://www.cs.umd.edu/projects/PL/cyclone/scp.pdf)
- [Cyclone Project Site](https://cyclone.thelanguage.org/)

---

## 2. CCured — Pointer Classification for C

**Period**: 2002-2005
**Authors**: George Necula, Scott McPeak, Westley Weimer (UC Berkeley)
**Goal**: Retrofit memory safety onto legacy C via automatic pointer classification

### What It Was

CCured automatically classified every pointer in a C program into one of three kinds:
- **SAFE**: No bounds info needed (single word, like normal pointer)
- **SEQ** (sequence): Pointer + base + end = 3 words. Bounds-checked on arithmetic.
- **WILD**: Pointer + base + tags. Full runtime tracking for unsafe cast patterns.

Static analysis determined which pointers could be SAFE (most), which needed SEQ
(array-like), and which required WILD (arbitrarily cast).

### Quantified Data

- **Pointer classification**: <1% of pointers are WILD, <10% are SEQ in typical programs
- **Performance overhead**: 0-150% runtime slowdown (varies per benchmark)
- **Source changes required**: Non-trivial. Restructuring to avoid WILD pointers requires
  runtime type information annotations, tagged unions, and code reorganization.
- **Separate compilation**: NOT supported. Whole-program analysis required.

### Why It Failed

1. **Binary compatibility destroyed**: SEQ pointers are 3 words. WILD pointers include
   tags. Any struct containing a SEQ or WILD pointer has a different memory layout than
   the original C struct. This breaks ABI compatibility with every existing C library.

2. **Source modifications required**: Despite being "automatic," CCured required non-trivial
   source changes to avoid WILD pointer classification. A WILD pointer anywhere in a
   reachable type contaminates the entire connected component.

3. **No separate compilation**: CCured needs whole-program analysis. You cannot compile
   one file at a time. This is fatal for large systems.

4. **Successor obsoleted it**: SoftBound achieved similar safety with NO source
   modifications and full separate compilation support, by storing bounds metadata
   in a separate shadow space rather than widening pointers.

### Failure Type: Fundamental (wrong mechanism)

Fat pointers are fundamentally incompatible with C's memory layout contracts.
Any approach that changes pointer width breaks binary compatibility.

### Lesson for Ori

- **Never change data layout for safety**. Safety metadata must be tracked out-of-band
  (compile-time types, shadow metadata, or separate runtime structures).
- Whole-program analysis is acceptable for optimization but not for basic safety.
  Safety must work with separate compilation.
- Ori's approach (ARC + compile-time ownership tracking) is correct: the safety
  mechanism does not alter the memory representation of user types.

**Sources**:
- [CCured: Type-Safe Retrofitting of Legacy Software (TOPLAS)](https://people.eecs.berkeley.edu/~necula/Papers/ccured_toplas.pdf)
- [CCured in the Real World (PLDI 2003)](https://people.eecs.berkeley.edu/~necula/Papers/ccured_pldi03.pdf)
- [SoftBound: Highly Compatible Spatial Memory Safety](https://people.cs.rutgers.edu/~santosh.nagarakatte/papers/pldi09_softbound.pdf)

---

## 3. ATS (Applied Type System) — Dependent Types for Systems Programming

**Period**: Early 2000s - present (ATS3 alpha released April 2025)
**Author**: Hongwei Xi (Boston University)
**Goal**: Combine programming with theorem proving at C-level performance

### What It Is

ATS unifies implementation with formal specification. It has:
- **Dependent types**: Types parameterized by values (`arrayref(int, n)`)
- **Linear types**: Track resource ownership (no GC needed)
- **Proof functions** (`prfun`): Total recursive functions erased at compile time
- **Propositions** (`dataprop`): Inductive proof types encoding invariants

ATS compiles to C, matching C performance because proofs are erased.

### The Proof Burden (Quantified)

- **Simple properties**: ~1:1 ratio (1 line proof per 1 line implementation)
- **Moderate complexity**: ~2:1 ratio (type signature + proof constructor)
- **Complex recursive proofs**: ~2:3 ratio (4 lines proof, 6 lines implementation)
  but with a "termination clause" and explicit type parameters that substantially
  expand signatures
- **Real programs**: The burden scales with logical complexity, not code size.
  A verified quicksort requires ~40 lines with mutable arrays, imperative loops,
  AND dependent type annotations (`{n:nat}`, `&arrayref(int, n) >> _`).

The ATS2 compiler itself is 180,000+ lines of ATS1, demonstrating the cumulative
weight of the approach.

### Why Adoption Is Near-Zero

1. **Requires two mental models simultaneously**: Programmers must think in both a
   theorem/proof layer and an implementation layer. This dual-mode thinking is
   fundamentally different from how most programmers work.

2. **Mathematical sophistication required**: Dependent types and linearity demand
   background in type theory uncommon outside PL research.

3. **No package manager**: Manual dependency management via `staload`.

4. **Limited tooling**: Primarily Emacs/Vim. No LSP, no modern IDE support.

5. **Tiny community**: Fewer than 200 GitHub repositories contain ATS code as of 2024.
   Community consists primarily of researchers and academics.

6. **Full type inference is undecidable**: Programmers must supply annotations for
   complex constraints, since the type system is too expressive for full inference.

### Failure Type: Fundamental (wrong target audience / wrong abstraction level)

The approach is mathematically correct but targets an audience that barely exists.
The number of programmers who can simultaneously write systems code AND construct
formal proofs is vanishingly small. The proof burden does not "go to zero" with
practice -- it is inherent in the approach.

### Lesson for Ori

- **Proofs must be invisible to the common case**. Ori's contracts (`pre()`/`post()`)
  are the right level: they look like assertions, not theorems. Runtime-checked today,
  statically verifiable later.
- **Never require the programmer to operate in two separate mental frameworks**.
  Capabilities should feel like annotations, not proof obligations.
- **Tooling and ecosystem matter as much as type system power**. A language with 200
  GitHub repos has failed regardless of its theoretical properties.
- ATS proves that dependent types CAN achieve zero runtime overhead with full
  verification. The question is whether the proof burden can be hidden.

**Sources**:
- [ATS: A Language That Combines Programming with Theorem Proving](https://link.springer.com/chapter/10.1007/11559306_19)
- [Applied Type System (arXiv)](https://arxiv.org/abs/1703.08683)
- [Constructing Proofs with dataprop in ATS](https://bluishcoder.co.nz/2013/07/01/constructing-proofs-with-dataprop-in-ats.html)
- [Why isn't ATS more popular? (Quora)](https://www.quora.com/Why-isnt-the-ATS-programming-language-more-popular)

---

## 4. Java Checked Exceptions — Effect Tracking That Failed

**Period**: 1996 - present (but effectively abandoned by the ecosystem)
**Design**: James Gosling (Sun Microsystems)
**Goal**: Force callers to handle recoverable errors via type system

### What It Was

Every Java method must declare which checked exceptions it can throw:
```java
void readFile(String path) throws IOException, SecurityException { ... }
```
Callers must either catch or re-declare each exception. This is, in essence,
a **mandatory effect system** -- the type system tracks one specific effect
(error paths) through the call chain.

### Why It Failed (Quantified)

1. **Exponential declaration explosion** (Hejlsberg's scalability argument):
   When integrating subsystems each throwing 4-10 exceptions, throws clauses grow
   exponentially up the call tree. "You end up having to declare 40 exceptions
   that you might throw." At system integration level: **80+ throw statements**.

2. **Viral annotation burden**: Every method in the call chain between thrower and
   handler must declare every exception. Adding a new exception to a library method
   is a **breaking change** for all callers (Hejlsberg's versionability argument).

3. **The `throws Exception` anti-pattern**: Developers escape the burden by declaring
   `throws Exception` on everything, completely defeating the purpose. This became
   the dominant pattern in real codebases.

4. **Massive boilerplate**: Projects required **2000+ non-functional catch-throw blocks**
   just to re-wrap exceptions at layer boundaries. **600+ coding errors per project**
   in exception handling code.

5. **Composition failure with lambdas**: Java 8's functional interfaces (Stream.map,
   etc.) **cannot declare checked exceptions**. This forced Java's own standard library
   to abandon checked exceptions for its most modern API.

6. **10:1 ratio of finally to catch**: Hejlsberg observed that in well-written code,
   resource cleanup (finally) outnumbers exception handling (catch) 10:1, making the
   checked exception machinery irrelevant for most error handling.

7. **Industry verdict**: Every JVM language after Java (Kotlin, Scala, Groovy) rejected
   checked exceptions. All major frameworks (Spring, Hibernate) switched to unchecked.

### Failure Type: Fundamental (wrong granularity for effect tracking)

The core idea -- tracking effects in the type system -- is sound. The failure was in
making the tracking **mandatory, non-composable, and viral**. Every intermediate
function in the call chain pays the annotation cost even if it has nothing to do
with the error.

### Lesson for Ori

This is the **single most important lesson** for Ori's capability system:

- **Capabilities must compose without intermediate annotation burden**. If using
  `VolatileIO` in a leaf function forces every caller up the chain to declare
  `uses VolatileIO`, we have recreated Java checked exceptions.
- **Capabilities need a discharge mechanism**. Java had no way to "handle" an exception
  type and remove it from the signature. Ori's `with Cap = handler in expr` is
  critical -- it discharges the capability.
- **Adding a new capability to a library must NOT break callers**. This means capability
  propagation must have some form of inference or default handling.
- **The escape hatch anti-pattern is inevitable**. If the burden is too high,
  programmers will write `uses Unsafe` on everything, recreating Rust's problem.
  The system must make the CORRECT annotation cheaper than the escape hatch.

**Sources**:
- [The Trouble with Checked Exceptions (Anders Hejlsberg, Artima 2003)](https://www.artima.com/intv/handcuffs.html)
- [Checked Exceptions: Java's Biggest Mistake](https://literatejava.com/exceptions/checked-exceptions-javas-biggest-mistake/)
- [Java's Checked Exceptions: The 20-Year Experiment That Failed](https://www.javacodegeeks.com/2026/01/javas-checked-exceptions-the-20-year-experiment-that-failed.html)
- [Bruce Eckel on Checked Exceptions](https://www.artima.com/intv/handcuffs.html)

---

## 5. D Language @safe/@trusted/@system — Three-Level Safety

**Period**: 2007 - present
**Author**: Walter Bright
**Goal**: Graduated safety levels for systems programming

### What It Is

D has three function-level safety annotations:
- **`@system`** (default): All operations legal, including pointer casts and arithmetic
- **`@trusted`**: Claims safe interface but allows unsafe internals. Can be called from `@safe`.
- **`@safe`**: Compiler-enforced memory safety. No pointer arithmetic, no casts, etc.

### Why Wrong Defaults Killed It

1. **Default is `@system` (unsafe)**: Unlike Rust (safe by default) or C# (safe by default),
   D chose unsafe as the default. Every function without annotation is `@system`.
   This means safety is **opt-in**, requiring active effort for every function.

2. **DIP1028 (safe-by-default) was accepted then REVERSED**: The proposal to make `@safe`
   the default was initially accepted by Walter Bright, then reversed after community
   backlash due to:
   - Would break most existing unannotated code
   - `@safe` functions can override `@system` but not vice versa -- subclasses break
   - Template inference destroyed by adding `@system:` file headers
   - No tooling to automate migration of existing codebases
   - Extern(D) functions would need recompilation due to mangling changes

3. **`@trusted` is an unverifiable escape hatch**: `@trusted` functions are called from
   `@safe` code but contain arbitrary unsafe operations. The programmer must manually
   verify correctness, but:
   - "It is impossible for the programmer to uphold this assumption in @trusted code
     in all cases without making unacceptable feature, performance, and
     implementation-quality tradeoffs"
   - Invalid pointers CAN be created and returned from `@trusted` functions
   - `private` doesn't protect potentially-invalid data from same-module `@safe` code
   - No formal verification of the `@trusted` contract

4. **Asymmetric protections**: Built-in arrays have special safety protections that
   user-defined types cannot access, undermining the model's comprehensiveness.

### Adoption Data

No published statistics on what percentage of D code uses `@safe`. The DUB package
registry contains ~2,648 packages but no safety metrics are tracked. The D community
acknowledged that trying to turn `@safe` on by default "had been a disaster."

### Failure Type: Contingent (wrong default, could be fixed but wasn't)

The three-level model is reasonable. The failure was choosing the wrong default and
then being unable to change it due to backwards compatibility.

### Lesson for Ori

- **Safe must be the default. Period.** The opt-in vs opt-out decision determines
  whether safety is the norm or the exception.
- **The escape hatch (`@trusted`) must be verifiable**. Ori's contracts (`pre()`/`post()`)
  provide this -- they are runtime-checked assertions, not comments.
- **Plan for the default from day one**. D cannot change its default after 15+ years
  of code. Ori must ship with safe-by-default.
- **Never create an unverifiable trust boundary**. D's `@trusted` proves that
  "the programmer asserts correctness" is not a safety model.

**Sources**:
- [What Does Memory Safety Really Mean in D?](https://pbackus.github.io/blog/what-does-memory-safety-really-mean-in-d.html)
- [DIP1028 (rejected)](https://github.com/dlang/DIPs/blob/master/DIPs/rejected/DIP1028.md)
- [DIP1035 (accepted replacement)](https://github.com/dlang/DIPs/blob/master/DIPs/accepted/DIP1035.md)
- [Memory Safety in D Part 3](https://dlang.org/blog/2023/01/05/memory-safety-in-a-systems-programming-language-part-3/)

---

## 6. Safe Haskell — Binary Safe/Unsafe Modules

**Period**: GHC 7.2 (2011) - present
**Authors**: David Terei, Simon Marlow, Simon Peyton Jones, David Mazieres
**Goal**: Allow untrusted Haskell code to be securely included in trusted codebases

### What It Is

Three module-level flags:
- **`-XSafe`**: Restricts to safe subset (no `unsafePerformIO`, no Template Haskell,
  no pure FFI, no RULES, restricted overlapping instances)
- **`-XTrustworthy`**: Module author claims safe API despite internal unsafe operations
- **`-XUnsafe`**: Explicitly unsafe module

### Why Module-Level Granularity Is Too Coarse

1. **All-or-nothing per module**: A module is entirely Safe or entirely Trustworthy.
   If one function needs `unsafePerformIO`, the entire module must be Trustworthy,
   even if 99% of it is safe. This forces splitting modules artificially.

2. **No symbol-level tracking**: Under the alternative design (rejected for pragmatic
   reasons), better error messages would be possible. Currently, calling an unsafe
   function from a Safe module gives "not in scope" rather than "unsafe function."

3. **Template Haskell bypass**: TH can subvert module boundaries entirely, and there
   is no way to restrict which modules unsafe code imports, giving a "very large
   attack surface -- essentially any package currently installed on the system."

4. **Compilation-time bypass**: Custom preprocessors can launch arbitrary processes
   during compilation. Safe Haskell cannot prevent this.

5. **`-XTrustworthy` is honor system**: Like D's `@trusted`, the module author claims
   safety but the compiler cannot verify it. Other code relies on these claims.

6. **Practical adoption**: Safe Haskell is rarely used in practice. The Haskell ecosystem
   does not enforce it, and most library authors do not annotate their modules.

### Failure Type: Contingent (too coarse, but fixable in principle)

The binary classification at module level is too blunt. A function-level or
expression-level safety system would be more useful but was rejected as
"a more invasive change to the Haskell language."

### Lesson for Ori

- **Safety tracking must be at the function level, not the module level**. Ori's
  `uses Capability` on individual functions is the right granularity.
- **Trust boundaries need verification, not honor system**. Ori's contracts provide
  runtime-verified assertions at trust boundaries.
- **Compile-time code execution is a safety hole**. Any macro/metaprogramming system
  needs its own safety model.

**Sources**:
- [Safe Haskell (Haskell Symposium 2012)](https://www.scs.stanford.edu/~dm/home/papers/terei:safe-haskell.pdf)
- [GHC Safe Haskell User Guide](https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/safe_haskell.html)

---

## 7. Pony Reference Capabilities — Six-Capability Concurrency Safety

**Period**: 2014 - present (active but very small community)
**Authors**: Sylvan Clebsch et al. (Imperial College London)
**Goal**: Data-race freedom via reference capability annotations

### What It Is

Every reference in Pony carries one of six capabilities:
- **iso** (isolated): Mutable, exclusive, transferable between actors
- **val** (value): Deeply immutable, shareable across actors
- **ref** (reference): Mutable, not shareable (local to actor)
- **box**: Read-only locally (might be mutable elsewhere or immutable)
- **trn** (transition): Writable, but others only see it as `box`
- **tag**: Identity only -- cannot read or write data

These form a subtyping lattice. The compiler enforces that no two actors can see
the same object as mutable, guaranteeing data-race freedom.

### The Learning Curve Problem (Specific)

1. **Six capabilities is too many for initial learning**: Developers must understand
   all six kinds, their subtyping relationships, aliasing rules, and when to use each.
   The official tutorial explicitly warns: "Generics and reference capabilities are the
   hardest things to get a handle on while learning Pony so don't get frustrated.
   It's not just you. We all go through this."

2. **`trn` is rarely used**: The transition capability exists primarily for type system
   completeness but is "seldomly used in code," adding complexity without proportional
   benefit.

3. **Error messages are cryptic**: "val is not a subtype of iso" gives no guidance on
   the FIX. Error messages only mention the right-hand side when the left-hand side's
   capability is the actual problem.

4. **Capability subtyping matrix is confusing**: "Local" and "global" in terms of
   references are "not clear concepts for people coming from other programming
   backgrounds."

5. **`consume` and `recover` add more concepts**: Beyond the six capabilities,
   developers must understand destructive read (`consume`) and capability lifting
   (`recover`), adding two more operations to the mental model.

### Adoption Data

- **Wallaroo** (the most prominent Pony user) migrated to Rust, citing ecosystem
  limitations and hiring difficulty.
- Pony's GitHub organization has 91 repositories but the community is very small.
- No published adoption statistics. The language is effectively a research artifact
  with a small community of enthusiasts.

### Failure Type: Contingent (right idea, wrong cognitive load distribution)

Pony's THEORY is correct -- the six capabilities precisely characterize all useful
aliasing/sharing patterns. The PRACTICE is that programmers cannot hold six
capability distinctions in working memory while also solving their domain problem.

### Lesson for Ori

- **The number of distinct safety levels must be small** (2-4, not 6+). Ori's
  graduated capabilities must group into a small number of conceptual levels
  that are easy to reason about.
- **Capability names must map to programmer intuition**, not type theory.
  "VolatileIO" is better than "trn" because it describes WHAT, not HOW.
- **Error messages must explain the FIX**, not just the violation. "Cannot use
  RawMemory in this context; consider adding `uses RawMemory` or use `PhysAddr`
  instead" is mandatory.
- **Rarely-used capabilities should be removed or merged**. If a capability
  exists only for type system completeness, it is an implementation detail
  that should not be user-visible.

**Sources**:
- [Reference Capabilities - Pony Tutorial](https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html)
- [Why Wallaroo Moved From Pony To Rust](https://wallarooai.medium.com/why-wallaroo-moved-from-pony-to-rust-292e7339fc34)
- [Reference Capabilities in Pony for Everybody](https://zartstrom.github.io/pony/2016/08/28/reference-capabilities-in-pony.html)
- [Pony Tutorial: Error Messages](https://tutorial.ponylang.io/appendices/error-messages.html)

---

## 8. Vault (Microsoft Research) — Linear Types for Imperative Programming

**Period**: ~2001-2004
**Authors**: Robert DeLine, Manuel Fahndrich (Microsoft Research)
**Goal**: Practical linear types for tracking resource protocols in imperative code

### What It Was

Vault used linear types and typestates to enforce resource protocols (e.g., "file must be
opened before reading, closed after use"). Key innovations:
- **Adoption**: Safely alias a linear object by "adopting" it into a data structure
- **Focus**: Temporarily recover linear access to an adopted object for protocol checking

### Why It Stayed in Research

1. **Linear-nonlinear divide is too rigid**: "The hard division between linear and
   nonlinear types forces the programmer to make a trade-off between checking a
   protocol on an object and aliasing the object." You can either track a resource's
   state OR have multiple references to it, but not both.

2. **Linearity is infectious**: "Any type with a linear component must itself be linear."
   A struct containing a linear field becomes linear, and any struct containing THAT
   struct becomes linear. This cascading restriction makes practical programming painful.

3. **Adoption/focus solves the wrong problem**: The constructs reduce restrictions but
   add conceptual complexity. Programmers must now understand linear types AND
   adoption AND focus.

4. **No path to mainstream**: Linear and affine type systems "have had relatively little
   effect on programming practice" and "lacked a clear path forward to integrate with
   existing languages such as OCaml or Haskell."

### Failure Type: Fundamental (linearity too restrictive for general imperative programming)

Linear types are the right tool for a narrow domain (resource tracking) but too
restrictive for general programming. Rust's solution (borrowing with lifetimes)
succeeded where Vault failed by making the restrictions more flexible.

### Lesson for Ori

- **Do not require linear types for general programming**. Ori's ARC handles the
  common case. Linear/unique types should only appear in specialized low-level APIs
  (DMA buffers, MMIO regions) where the restriction is inherent in the hardware.
- **Avoid infectious type properties**. A capability on a struct field should not
  force the entire struct (and all containers of that struct) to carry the capability.
- **Resource tracking via capabilities is more practical than linear types**. `uses DMA`
  is less restrictive than making `DmaBuffer<T>` linear, because the capability
  constrains WHERE the buffer is used, not HOW it is referenced.

**Sources**:
- [Adoption and Focus: Practical Linear Types for Imperative Programming (PLDI 2002)](https://www.microsoft.com/en-us/research/wp-content/uploads/2002/05/pldi02.pdf)
- [Typestates for Objects](https://link.springer.com/chapter/10.1007/978-3-540-24851-4_21)

---

## 9. Sing# / Singularity OS — Microsoft's Safe OS Language

**Period**: 2003-2008 (Singularity), evolved to Midori (~2009-2015)
**Authors**: Galen Hunt, James Larus, Joe Duffy, and teams at Microsoft Research
**Goal**: Build an entire OS in a memory-safe managed language

### What It Provided

Singularity was an experimental OS with extraordinary safety guarantees:
- **Over 90% of kernel in Sing#** (memory-safe C# dialect), only ~6% in unsafe C++/assembly
- **Software-Isolated Processes (SIPs)**: Processes in same address space, isolated by
  language safety rather than hardware memory protection
- **Contract-based channels**: IPC defined by state-machine contracts, compiler-verified
- **Zero-copy message passing**: Data ownership transferred via exchange heap
- **Performance**: SIP creation ~388K cycles (far less than traditional processes),
  thread switch ~394 cycles, message latency ~1,040 cycles for ping-pong

### Safety Overhead (Quantified)

- **Runtime bounds/type checking**: 4.5-4.7% CPU overhead
- **Software isolation vs hardware isolation**: <5% overhead (vs 25-33% for hardware MMU)
- **Contract runtime monitoring**: ~6 microseconds per operation

### Why Sing#/Singularity Was Abandoned

The project evolved to Midori rather than being simply abandoned:

1. **Required maintaining an advanced compiler**: "Integrating the system and the language
   was powerful, but meant that the Singularity team had to maintain an advanced
   compiler -- making it much harder for others within and outside of Microsoft to use
   and build on the Singularity system."

2. **Organizational politics**: Joe Duffy noted "decisions around the destiny of Midori's
   core technology weren't entirely technology-driven, and sadly, not even entirely
   business-driven." The Windows team was skeptical even with Midori running in front
   of them.

3. **Ecosystem incompatibility**: A safe OS requires a safe language requires a safe
   compiler requires a safe standard library. The entire stack must be rebuilt from
   scratch, incompatible with existing software.

### Midori's Three Safeties (Key Design Insight)

Joe Duffy identified three essential safety properties:
- **Memory safety**: No buffer overflows, use-after-free, double-frees
- **Type safety**: No type confusion, casting errors, uninitialized variables
- **Concurrency safety**: No read-write, write-read, or write-write hazards

All three are necessary. Concurrency violations "frequently cascaded into type and
memory safety failures." The minimal trusted computing base (TCB) with unsafe code
handled hardware, while "all application-level and library code was 100% safe."

**Key performance finding**: "Compiler technology has advanced tremendously" and safety
overheads are "within the noise for most interesting programs." Architectural decisions
(async everywhere, zero-copy IO) "far outweighed the minor costs of safety."

### Failure Type: Contingent (organizational/ecosystem, not technical)

Sing#/Singularity/Midori WORKED TECHNICALLY. The 4.5% safety overhead is negligible.
The failure was that building a safe OS requires rebuilding the entire software
ecosystem, which is a business problem, not a technical one.

### Lesson for Ori

- **4.5% runtime overhead for safety is the benchmark to target**. Singularity proved
  this is achievable.
- **Software isolation (language-enforced) beats hardware isolation (MMU)**. This
  validates Ori's approach of compile-time safety over runtime sandboxing.
- **Contract-based verification works for systems code**. Singularity's channel
  contracts are directly analogous to Ori's `pre()`/`post()` capability contracts.
- **The three safeties (memory, type, concurrency) must all be addressed**. Ori has
  memory safety (ARC) and type safety (HM inference). Concurrency safety via
  capabilities (`uses Suspend`, `Sendable`) needs to be equally rigorous.
- **Ecosystem compatibility is non-negotiable**. Ori must interop with C/Rust/LLVM
  ecosystems, not require rebuilding everything from scratch.

**Sources**:
- [Singularity: Rethinking the Software Stack](https://courses.cs.washington.edu/courses/cse551/15sp/papers/singularity-osr07.pdf)
- [A Tale of Three Safeties (Joe Duffy)](https://joeduffyblog.com/2015/11/03/a-tale-of-three-safeties/)
- [Safe Native Code (Joe Duffy)](https://joeduffyblog.com/2015/12/19/safe-native-code/)
- [The Error Model (Joe Duffy)](https://joeduffyblog.com/2016/02/07/the-error-model/)

---

## 10. Rust's `unsafe` — The Binary Safety Boundary in Practice

### Empirical Studies

Three major studies characterize unsafe Rust usage:

**Study 1: Evans et al. (ICSE 2020) — "Is Rust Used Safely by Software Developers?"**
- **29% of all crates** on crates.io use `unsafe` directly
- **Over 50%** of the most downloaded crates use `unsafe`
- **~60% of popular/highly downloaded libraries** include unsafe Rust
- Majority of unsafe uses: calling other Rust functions marked `unsafe` (not FFI)
- Only **22% of unsafe function calls** are to external C libraries
- **Over half of all crates** cannot be entirely statically checked because unsafe
  Rust is hidden somewhere in a library's call chain

**Study 2: Astrauskas et al. (OOPSLA 2020) — "How Do Programmers Use Unsafe Rust?"**
- **75% of crates** do not use `unsafe` themselves (but may depend on crates that do)
- **20% of crates** use unsafe blocks
- **13% of crates** declare unsafe functions
- **~90% of unsafe code** calls unsafe functions (the dominant pattern)
- **5% of unsafe usages** are unnecessary (removing `unsafe` causes no compile errors)

**Study 3: Xu et al. (TOSEM 2021) — "Memory-Safety Challenge Considered Solved?"**
- **186 total memory-safety bugs** analyzed (all Rust CVEs through 2020-12-31)
- **All 185 non-compiler bugs require `unsafe` code** — Rust keeps its promise
- Bug distribution: 1 compiler, 33 stdlib, 142 third-party libraries, 10 executables
- **Bug consequences**: 82 use-after-free, 40 buffer overflow/over-read, 12 double-free,
  12 uninitialized memory, 39 other undefined behaviors
- **Culprit categories**: 52 unsound generic/trait, 35 automatic memory reclaim,
  29 unsound function, 69 other errors
- **Largest single category**: "Insufficient bound of generic" (36 bugs) -- missing
  trait bounds on generic parameters in unsafe code

**Study 4: ICSE 2024 — "Is unsafe an Achilles' Heel?"**
- Defined **19 safety properties** for unsafe APIs
- Analyzed **416 unsafe APIs** in the standard library
- Found "inconsistency and insufficiency" in safety requirement documentation
- 50 experienced Rust programmers surveyed confirmed the classification

### Android/Chromium Production Data

- Android memory safety vulnerabilities: dropped from **76% (2019)** to **<20% (2024)**
  with Rust adoption
- Google reports: **1 near-memory-safety issue per 5M lines of Rust** vs
  **~1000 vulnerabilities per 1M lines of C/C++** (1000x improvement)
- Chromium: **~70% of high-severity issues** are memory safety issues (in C++)

### The Fundamental Problem with Binary `unsafe`

1. **Unsafe is viral through call chains**: >50% of crates transitively depend on unsafe
2. **Unsafe blocks are not granular**: Everything inside `unsafe {}` loses ALL compiler
   guarantees, even if only one specific operation needs to be unsafe
3. **Safety comments are unverifiable**: `// SAFETY: ...` is a comment, not a contract
4. **The largest CVE category (unsound generics) is a DESIGN problem**: Missing trait
   bounds cannot be caught by "be more careful in unsafe code"
5. **In kernel code, unsafe is everywhere**: Every FFI call, every MMIO, every global --
   the safety boundary dissolves

### Lesson for Ori

- **29% of crates using unsafe is too high**. Ori's graduated capabilities should make
  the "needs raw access" percentage much smaller because most uses of unsafe are
  calling already-verified unsafe functions, not doing fundamentally unsafe things.
- **The transitive dependency problem is critical**. Ori capabilities propagate through
  call chains by design, making the safety boundary visible and auditable.
- **Generic bounds are where safety breaks down**. Ori must ensure that capability
  requirements on generic parameters are enforced (not just documented).
- **Contracts replace safety comments**. `pre(addr.is_aligned())` is verifiable.
  `// SAFETY: addr is aligned` is not.
- **The 1000x safety improvement from safe code is the prize**. The goal is not
  to match Rust but to make MORE code expressible without any escape hatch.

**Sources**:
- [Is Rust Used Safely? (Evans et al., ICSE 2020)](https://arxiv.org/pdf/2007.00752)
- [How Do Programmers Use Unsafe Rust? (OOPSLA 2020)](https://dl.acm.org/doi/10.1145/3428204)
- [Memory-Safety Challenge Considered Solved? (Xu et al., TOSEM)](https://arxiv.org/abs/2003.03296)
- [Is unsafe an Achilles' Heel? (ICSE 2024)](https://dl.acm.org/doi/10.1145/3597503.3639136)
- [Rust Drives Android Memory Safety Below 20%](https://thehackernews.com/2025/11/rust-adoption-drives-android-memory.html)

---

## 11. Effect Systems That Didn't Ship

### The Landscape

Algebraic effects have been proposed for many languages but remain largely experimental:

- **Koka** (Daan Leijen, MSR): Full row-polymorphic effect system, research language
- **OCaml 5**: Added effect handler primitives (5.0, 2022) and high-level syntax (5.3)
  but **effects are NOT tracked at the type level** as of OCaml 5.4
- **Eff**: Research language purpose-built for algebraic effects
- **Haskell**: Effects encoded via monads (not true algebraic effects)
- **Unison**: Effects tracked in types (research-oriented)

### Why They Stay Experimental

1. **Compilation is hard**: "Compiling algebraic effects efficiently is not straightforward"
   because the operational semantics require capturing a delimited execution context
   (one-shot continuations). The compiler cannot manipulate the system stack directly.

2. **Three fundamental problems** (identified at Dagstuhl Seminar 18172):
   - **Reasoning**: How to reason about programs with effects
   - **Performance**: Handler dispatch and continuation capture overhead
   - **Typing**: Combining polymorphism with effect subtyping makes inference undecidable

3. **OCaml's compromise**: OCaml 5 shipped effects WITHOUT type-level tracking because
   typing effects was too hard. A performed-but-unhandled effect fails at RUNTIME,
   not compile time. This defeats the purpose of a type-and-effect system.

4. **Koka's performance challenges**: Evidence vectors passed to every function create
   runtime overhead. "Open floating" optimization improves performance by 2.5x but
   the baseline overhead exists. Row polymorphism on effects was adopted because
   polymorphism + subtyping made inference undecidable.

5. **Handler matching is dynamic**: "The main technical difficulty arises from the
   dynamic nature of coupling an effectful operation with the right handler during
   execution." This is analogous to virtual dispatch -- it works but is hard to
   optimize.

6. **Monad transformer overhead in Haskell**: Stacking monad transformers for
   multiple effects creates O(n) dispatch overhead per effect and requires
   mechanical lifting that is error-prone.

### Failure Type: Still open (compilation + inference barriers not yet solved)

No one has demonstrated a production-quality, high-performance, fully-typed effect
system in a mainstream language. The closest is Koka, which is a research language.

### Lesson for Ori

- **Ori's capability system IS an effect system**, but it takes a different approach
  than algebraic effects. Capabilities are tracked as function annotations, not as
  row types. They propagate through call chains but do not require capturing
  continuations.
- **Avoid the OCaml trap**: If capabilities aren't tracked at the type level, they
  become documentation, not enforcement. Ori MUST enforce capabilities at compile time.
- **Avoid the Koka trap**: If every function call passes an evidence vector, the
  runtime overhead is too high for systems programming. Ori capabilities should be
  ERASED at runtime (they are compile-time constraints, not runtime values).
- **Avoid the Haskell trap**: If capabilities require manual composition (like monad
  transformers), the cognitive burden kills adoption. Ori capabilities must compose
  automatically through `uses` declarations.
- **The typing/inference problem is real**. Ori should use simple capability propagation
  (union of required capabilities through call chain) rather than row polymorphism
  or subtyping, which make inference undecidable.

**Sources**:
- [Algebraic Effects for Functional Programming (Leijen, MSR)](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/08/algeff-tr-2016-v2.pdf)
- [Koka: Programming with Row-polymorphic Effect Types](https://arxiv.org/pdf/1406.2061)
- [Algebraic Effect Handlers Go Mainstream (Dagstuhl Report)](https://kcsrk.info/papers/effects_dagstuhl18.pdf)
- [Generalized Evidence Passing (Koka compilation)](https://dl.acm.org/doi/10.1145/3473576)
- [Effective Programming: Adding an Effect System to OCaml (Jane Street)](https://www.janestreet.com/tech-talks/effective-programming/)

---

## 12. Capability-Based Security Systems — E, Joe-E, Caja

### E Language (Mark Miller, 1997)

**What it was**: Object-capability programming language for secure distributed computing.
Every object reference is a capability. "Only connectivity begets connectivity" --
you can only access objects you hold references to.

**What worked**:
- Theoretically sound security model
- Influenced Pony, SES (Secure ECMAScript), and capability-based security research
- Mark Miller's thesis established the formal foundations
- Capability Myths Demolished (2003) addressed common misconceptions

**What didn't work**:
- Near-zero production adoption
- Small research community only
- The security model was hard to explain to application developers
- No ecosystem, no libraries, no tooling

### Joe-E (UC Berkeley, ~2008)

**What it was**: A subset of Java enforcing object-capability discipline. By restricting
Java to a capability-safe subset, existing Java code could be incrementally made safe.

**What worked**:
- Subsetting an existing language avoids the "new language" adoption barrier
- Influenced later subset languages (ADsafe, Cajita)

**What didn't work**:
- "Taming a library is unfortunately a time-consuming and difficult task, and a place
  where a mistake could violate soundness of security goals." Every Java standard
  library method had to be manually reviewed for capability safety.
- The DarpaBrowser security review found "methods violating capability discipline had
  been inadvertently allowed" -- manual taming is error-prone.

### Google Caja (2008-2021, archived)

**What it was**: Compiler that sanitized untrusted HTML/CSS/JavaScript for safe embedding
in web pages. Used the object-capability model to isolate third-party code.

**What worked**:
- Used by MySpace, iGoogle, Orkut for gadget sandboxing
- Formal proofs of capability safety for the Cajita JavaScript subset
- Demonstrated practical capability-based isolation

**What failed catastrophically**:
- **Unicode escape bypass**: `\u0077indow` bypassed identifier recognition to access the
  real `window` object, escaping the sandbox entirely. The code "did not take into account
  different ways of storing the same identifier."
- **DOM clobbering**: Global variables could be clobbered via HTML `name` attributes.
- **Repeated bypasses**: The same researcher got bounties THREE TIMES for nearly the
  same class of bypass, showing the attack surface was too large to secure.
- **Archived January 2021**: Google archived the project "due to known vulnerabilities
  and lack of maintenance to keep up with the latest web security research."

**Root cause**: Language-based sandboxing of a language as complex as JavaScript is
fundamentally hard. The semantic gap between the sandbox model and the implementation
is too large to secure completely. Browser-native isolation (iframes + postMessage)
won because it uses hardware/OS-level isolation that cannot be bypassed by language tricks.

### Failure Type: Mixed

- E: Contingent (right ideas, no ecosystem). Ideas live on in SES and Agoric.
- Joe-E: Fundamental for subsetting (taming existing libraries is intractable).
  Contingent for capability model itself.
- Caja: Fundamental (language-level sandboxing of a complex language is too brittle).

### Lesson for Ori

- **Capabilities in the language, not as a sandbox layer**. Ori's capabilities are
  part of the type system from day one, not retrofitted onto an existing language.
  This avoids the taming problem (Joe-E) and the semantic gap problem (Caja).
- **Capability names must be meaningful to developers**. E's success was in making
  "object reference = capability" intuitive. Ori's `uses Http`, `uses FileSystem`
  follow this principle.
- **The attack surface of capability enforcement must be small**. Caja failed because
  JavaScript has too many escape hatches. Ori's compiler enforces capabilities at the
  type-checker level where the attack surface is the type checker itself (not the
  entire language runtime).
- **Capability composition must be automatic, not manual review**. Joe-E's manual
  library taming is unsustainable. Ori capabilities propagate through the type system
  automatically.
- **Don't try to subset an unsafe language for safety**. Build safety in from the start.

**Sources**:
- [Capability Myths Demolished (Miller et al., 2003)](https://srl.cs.jhu.edu/pubs/SRL2003-02.pdf)
- [Joe-E: A Security-Oriented Subset of Java (NDSS)](https://www.ndss-symposium.org/wp-content/uploads/2017/09/met.pdf)
- [Google Caja Project Archive](https://code.google.com/archive/p/google-caja)
- [Object Capabilities and Isolation of Untrusted Web Applications](https://theory.stanford.edu/~ataly/Papers/sp10.pdf)
- [Google Caja XSS Bypasses](https://blog.bentkowski.info/2017/11/yet-another-google-caja-bypasses-hat.html)

---

## Summary: Design Principles for Ori's Deep Safety

### Universal Lessons (from ALL 12 failures)

| # | Principle | Violated By |
|---|-----------|-------------|
| 1 | **Safe must be the default** | D (@system default), Safe Haskell (module-level opt-in) |
| 2 | **Annotation burden must stay <1% of code** | Cyclone (0.5% -- achieved!), Java checked exceptions (2000+ blocks), ATS (proof functions) |
| 3 | **Safety metadata must not change data layout** | CCured (fat pointers), Cyclone (fat pointers in kernel code) |
| 4 | **Escape hatches must be verifiable** | D (@trusted), Safe Haskell (-XTrustworthy), Rust (// SAFETY comments) |
| 5 | **Capabilities must compose without viral propagation** | Java checked exceptions (throws clauses cascade), Vault (linear types are infectious) |
| 6 | **Tracking granularity: function level, not module level** | Safe Haskell (module-level), Rust (block-level -- too fine for audit) |
| 7 | **Cognitive load: max 3-4 safety concepts** | Pony (6 capabilities + consume + recover = 8 concepts) |
| 8 | **Must work with separate compilation** | CCured (whole-program only) |
| 9 | **Must interop with existing ecosystems** | Singularity (required full-stack rebuild), E (no ecosystem) |
| 10 | **Runtime overhead of safety: target <5%** | Singularity achieved 4.5%; CCured 0-150% |
| 11 | **Effect tracking must be enforced at compile time** | OCaml 5 (runtime-only), Rust unsafe (comment-only) |
| 12 | **Never require formal proofs for common operations** | ATS (proof functions), Vault (linear type obligations) |

### Ori's Position

Ori's capability system avoids the specific failure modes of all 12 approaches:

- **Not binary** (unlike Rust unsafe, Safe Haskell): graduated `uses` capabilities
- **Not opt-in safety** (unlike D @safe): safe by default, capabilities are opt-in for power
- **Not module-level** (unlike Safe Haskell): per-function `uses` declarations
- **Not viral** (unlike Java checked exceptions): `with Cap = handler in expr` discharges
- **Not layout-changing** (unlike CCured): capabilities are compile-time types, erased at runtime
- **Not proof-requiring** (unlike ATS): contracts are runtime-checked assertions
- **Not unverifiable** (unlike D @trusted, Rust // SAFETY): contracts are executable code
- **Not too many levels** (unlike Pony's 6): graduated but grouped into intuitive categories
- **Not ecosystem-breaking** (unlike Singularity): C FFI via existing Deep FFI proposal

The key remaining risk is **annotation burden**. Java checked exceptions are the warning:
if capability annotations propagate virally through call chains, developers will write
`uses Unsafe` everywhere and the system collapses. The design must ensure that:

1. Common code needs ZERO capability annotations (safe by default)
2. Low-level code needs AT MOST one or two capability annotations per function
3. `with...in` discharge ensures intermediate callers don't accumulate capabilities
4. Adding a capability to a library function does NOT break existing callers
