---
section: "05"
title: "Exploring Perceus for OCaml — Evaluation Methodology"
status: complete
goal: "Establish evaluation discipline for old-vs-AIMS comparisons and define hard regression gates vs observed metrics"
paper:
  title: "Exploring Perceus for OCaml"
  url: "https://www.microsoft.com/en-us/research/publication/exploring-perceus-for-ocaml/"
  venue: "ML Workshop 2023"
  authors: "Pinto & Leijen"
depends_on: ["01", "02", "03", "04"]
sections:
  - id: "05.1"
    title: "Paper Thesis"
    status: complete
  - id: "05.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "05.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "05.4"
    title: "Plan Edits"
    status: complete
  - id: "05.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "05.6"
    title: "Lens Shift"
    status: complete
  - id: "05.7"
    title: "Open Risk"
    status: complete
---

# Section 05: Exploring Perceus for OCaml — Evaluation Methodology

**Status:** Complete
**Goal:** Establish rigorous evaluation discipline for old-vs-AIMS comparisons. Define
what should become a hard regression gate versus an observed metric. Ensure backend-swap
effects are measured cleanly.

**Paper:** Pinto & Leijen, "Exploring Perceus for OCaml," ML Workshop 2023.
[Full paper](https://www.microsoft.com/en-us/research/publication/exploring-perceus-for-ocaml/)

**Why read this fifth:** This is mostly a methodology paper. It tells you how to evaluate
the AIMS branch without cheating — same compiler, same source, only switch the memory-
management backend. It is the gold standard for AIMS Section 08 evaluation doctrine.

**Pause questions:**
- Are your old-vs-AIMS comparisons isolated enough?
- What should become a hard regression gate vs an observed metric?
- Are you measuring backend swap effects cleanly enough?

**AIMS context:**
- `aims-shadow` feature runs both pipelines and compares results
- `ShadowComparisonReport` tracks 5 dimensions of comparison
- `diagnostics/aims-compare.sh` compares behavioral + RC output
- Section 08 (Verification & Validation) handles evaluation
- Current gates: behavioral equivalence, Valgrind clean, verify pass

---

## 05.1 Paper Thesis

The paper's core claim is methodological, not algorithmic: **no fair comparison between
reference counting and garbage collection existed within a single compiler system.** All
prior Perceus evaluations compared across language boundaries (Koka vs OCaml vs Haskell
vs Java), which conflates the memory-management strategy with differences in compiler
quality, optimization pipelines, calling conventions, allocator implementations, and
language semantics. Pinto and Leijen implemented Perceus reference counting inside the
OCaml 4.14.0 compiler itself, producing a system where "we can directly compare the
performance of programs compiled with the exact same compiler, where we only switch
the backend."

The paper demonstrates that this isolation is achievable and that the results differ
materially from cross-system comparisons. Specifically:

- **Execution time.** Perceus with drop specialization is faster than OCaml's
  generational GC on 4 of 5 benchmarks (cfold: 0.72x, nqueens: 0.87x, rbtree-ck:
  0.76x, deriv: 0.96x). Only rbtree (2.76x for naive, 1.52x with drop specialization)
  is significantly slower, because balanced tree insertion creates many short-lived
  objects ideal for OCaml's nursery/copying collector.

- **Peak memory.** Perceus uses less memory on all benchmarks, with 40%+ reduction
  on cfold and nqueens (0.54x and 0.58x peak RSS). The other three benchmarks show
  near-parity (0.98-0.99x), indicating OCaml's GC manages heap size well in those
  cases.

- **Optimization incrementality matters.** The paper reports three configurations:
  GC (baseline), Perceus (naive), Perceus+opt (drop specialization). The delta
  between naive and drop-specialized Perceus is large (rbtree: 2.76x to 1.52x),
  establishing that intermediate optimization states must be measured independently.
  Reuse analysis is explicitly flagged as not-yet-implemented and expected to close
  the remaining rbtree gap.

The methodological contribution is the template: *same source, same frontend, same
middle-end, same backend optimizer, same code generator, same allocator integration
strategy* (they embed mimalloc into OCaml's runtime for fair allocation parity) *--
only switch the memory-management insertion pass and runtime.* This is the strictest
isolation achievable without a formal proof of equivalence.

The paper is explicit about its own limitations: "The benchmarks used in the evaluation
are quite limited -- too limited to draw any firm conclusions." This honesty is itself
the methodology lesson. Five benchmarks on allocation-intensive functional workloads
are informative but not definitive. The paper's value is the experimental design, not
the specific numbers.

---

## 05.2 What AIMS Should Adopt

### Keep

**K1. Same-compiler, feature-flag isolation is correct and AIMS already does it.**
The `--features aims` flag selects the AIMS pipeline while keeping identical lowering,
LLVM codegen, runtime, and allocator. This directly maps to the paper's approach of
switching the backend within OCaml 4.14.0. AIMS's feature-flag approach is actually
*cleaner* than the paper's prototype because both pipelines share the same `ArcFunction`
IR type, same `ori_llvm` emitter, and same `ori_rt` runtime. The paper had to integrate
mimalloc and modify the calling convention; AIMS changes nothing outside `ori_arc`.

**K2. Behavioral equivalence as the one non-negotiable hard gate.** The paper's
implicit assumption is that RC and GC produce identical observable behavior -- any
semantic difference would invalidate the entire comparison. AIMS Section 08.1 already
treats behavioral equivalence as a hard gate (exit code 1 in `aims-compare.sh` for
any output difference). This is correct and must remain the first gate, before any
performance comparison is meaningful.

**K3. Multiple optimization tiers must be measured independently.** The paper's
three-tier comparison (GC / Perceus / Perceus+opt) reveals that drop specialization
accounts for most of the performance improvement. AIMS should track analogous tiers:

| Tier | What it measures | AIMS equivalent |
|------|-----------------|-----------------|
| Legacy (baseline) | Current multi-pass pipeline | `cargo build` (default features) |
| AIMS core | Unified lattice, basic RC emission | `--features aims` (Stage 1C) |
| AIMS + coalescing | With RC coalescing pass | `--features aims` (Stage 1D) |
| AIMS + reuse | With reuse emission active | `--features aims` (Stage 1D+) |
| AIMS + dimensional fusion | Full 7-dimension cross-talk | `--features aims` (Stage 2) |

Currently `aims-compare.sh` only compares legacy vs AIMS-current. There is no
mechanism to record and compare across optimization tiers as AIMS matures. This is
a gap (see 05.4 Plan Edits).

**K4. Peak memory (RSS) as a tracked metric.** The paper measures peak working set
and finds 40%+ reductions for garbage-free RC. AIMS Section 08.5 already measures
peak RSS (`/usr/bin/time -v`) but only for one program. The paper's methodology
suggests measuring RSS for every benchmark program, not just compilation RSS.

**K5. Mean-of-N-runs with explicit hardware context.** The paper reports "mean over
five runs" on specified hardware (Core i7 @2.5GHz, 16GiB, macOS Ventura, compiled
with `-O2` via Clambda middle-end). AIMS Section 08.5 uses `hyperfine --warmup 3
--min-runs 10`, which is better (more runs, warmup). The hardware context (WSL2,
specific CPU) is noted but not formalized. This should be recorded in a stable
metadata format alongside benchmark results.

**K6. Benchmarks must be allocation-intensive.** The paper deliberately selects
"medium sized examples that are allocation intensive" because memory management is
irrelevant to compute-bound code. AIMS's golden corpus (`tests/aims/`) and benchmark
suite (`tests/benchmarks/`) include allocation-heavy programs (tree construction,
list COW, closure capture). The spec suite (`tests/spec/`) is less allocation-
intensive by nature (many small test functions with trivial allocations). RC count
comparisons on the spec suite are useful for regression detection but should not be
treated as performance evidence.

### New Invariants

**N1. Confounding-variable isolation principle.** Any metric comparison between
AIMS and legacy must satisfy: the two binaries were built from the same commit, with
the same `--release`/`--debug` profile, the same LLVM version, and the same `ori_rt`
runtime. The only permitted difference is the `aims` feature flag. If any other
variable changes (e.g., a runtime change lands between measurements), the comparison
is invalid. `aims-compare.sh` already enforces same-commit by building sequentially
from the same working tree. This invariant should be documented as a precondition
in the script header and in Section 08.

**N2. Optimization-tier tracking.** Each measurable AIMS improvement should be
attributed to a specific mechanism: unified lattice (access/consumption), borrow
improvement, immortal skipping, reuse, drop specialization, dimensional fusion.
The shadow comparison already tracks `immortal_skips` separately from RC count
improvements (see `AimsSnapshot.immortal_count` and `FunctionComparison.immortal_skips`
in `pipeline/shadow.rs`). This attribution model should extend to each new
optimization tier: when RC count improves, the report should attribute the delta to
the specific mechanism(s) responsible.

**N3. Benchmark stability contract.** The paper uses "the same benchmark suite used
in the original Perceus paper." AIMS golden corpus programs (`tests/aims/`) must be
treated as frozen after Section 08 baselines are established. Modifying a golden
corpus program invalidates all historical comparisons. If a program must change,
the old version is archived and a new baseline is established. This is not currently
enforced anywhere.

**N4. Compilation-speed isolation.** The paper does not measure compilation speed
(its comparison is purely runtime), but AIMS Section 08.5 gates on compilation time
(10% threshold). The paper's methodology implies that compilation speed comparisons
must also be isolated: same input, same LLVM version, same optimization level. AIMS
should additionally break down compilation time into phases (interprocedural analysis,
intraprocedural analysis, RC emission, reuse emission, LLVM codegen) to attribute
any regression to the specific phase. Section 08.7 lists "compile-time breakdown"
as not-yet-measured.

**N5. Distinguish static metrics from dynamic metrics.** The paper reports execution
time and peak RSS -- both are dynamic (runtime) metrics. AIMS currently mixes static
metrics (RC operation count in ARC IR) with dynamic metrics (execution time, memory).
Static RC count is a proxy for dynamic RC overhead, but the relationship is nonlinear:
an RC operation inside a hot loop matters 10,000x more than one in initialization
code. Section 08.2 should document this distinction explicitly and note that static
RC count is an optimization-quality signal, not a performance measurement.

---

## 05.3 What AIMS Should Not Adopt

### Reject

**R1. OCaml's calling-convention workarounds are irrelevant to Ori.** The paper
devotes Section 3 to modifying OCaml's custom calling convention (caller-save
registers, r14/r15 reserved for runtime) to integrate mimalloc for RC allocation.
This required inlining mimalloc's fast path into generated code and reserving r15
for the mimalloc thread-local heap. Ori uses LLVM's standard calling conventions
and links against `ori_rt` (which uses the system allocator). There is no calling-
convention impedance mismatch to solve. None of Section 3's engineering applies.

**R2. The specific benchmark suite is too OCaml-centric.** The five benchmarks
(cfold, deriv, nqueens, rbtree, rbtree-ck) are from the original Perceus paper and
emphasize OCaml-idiomatic patterns (immutable trees, symbolic derivatives, constraint
solving). Ori's allocation patterns differ: COW semantics on mutable collections,
closure environments, string SSO, enum-based ADTs with pattern matching. AIMS should
use its own benchmark suite derived from Ori-idiomatic programs, not port OCaml
benchmarks.

**R3. The "no reuse analysis" limitation does not apply.** The paper explicitly
notes that reuse analysis is not implemented and that rbtree performance would
improve with it. AIMS already has reuse emission infrastructure
(`aims/emit_reuse/`). The paper's rbtree regression (2.76x naive, 1.52x with drop
specialization) is not predictive of AIMS's behavior because AIMS's pipeline
architecture differs fundamentally (unified lattice vs sequential passes).

**R4. The paper's three-configuration comparison is too coarse for AIMS.** GC /
Perceus / Perceus+opt is a useful structure, but AIMS has finer-grained optimization
tiers (see K3 above). AIMS should not limit itself to three tiers; it should measure
at each significant optimization stage.

**R5. Single-threaded-only evaluation scope.** The paper evaluates OCaml 4.14.0
(single-threaded, no OCaml 5 multicore). The authors note they generate concurrent-
safe RC code (atomic increment for negative refcounts) to prepare for OCaml 5 but
do not evaluate concurrent workloads. Ori is currently single-threaded but has
`Sendable` and channel types for future concurrency. Concurrent RC evaluation
methodology is deferred to Stage 5.

---

## 05.4 Plan Edits

### Section 08 (Verification & Validation)

**E1. Add confounding-variable documentation to 08.1.** The evaluation doctrine
paragraph (lines 50-54 of `plans/aims/section-08-verification.md`) correctly cites
this paper but should be expanded with the explicit isolation requirements from N1:
same commit, same profile, same LLVM version, same runtime. Currently it says
"same compiler, same frontend, same optimizer, same LLVM backend" but does not
mention same commit or same build profile as requirements.
<!-- reviewed: completeness fix — The existing doctrine text (Section 08 line 51-53)
says: "Same compiler, same frontend, same optimizer, same LLVM backend -- only switch
old ARC pipeline vs AIMS pipeline." The proposed additions (same commit, same build
profile) are legitimate refinements of the methodology. Sound edit. -->

**E2. Formalize the static-vs-dynamic metric distinction in 08.2.** Add a note
after the RC counting methodology explaining that static RC count is a proxy metric,
not a direct performance measurement. Reference N5. The current text treats RC count
reduction as evidence of "less runtime overhead" (line 263 of `compare.rs`: <!-- reviewed: accuracy fix, was line 264 -->
"Fewer total RC ops = better (less runtime overhead)") -- this is approximately
true but should be qualified.
<!-- reviewed: completeness fix — PARTIALLY PRESENT. Section 08.2a (Allocation Count
Comparison) already has a careful static-vs-dynamic distinction: "Static allocation-site
counts are a secondary metric, not the main allocation story" and "What this does NOT
measure: Runtime allocations." Section 08.2 (RC counts) lacks an equivalent disclaimer.
The proposed edit is sound — add a similar qualifier to the RC counting section. -->

**E3. Add tier-tracking infrastructure to 08.5.** Section 08.5 measures compilation
speed for a single program (cow_chain.ori) and codegen quality via test pass rates.
It should also define the optimization-tier comparison matrix from K3 and track
metrics across tiers as AIMS matures from Stage 1C through Stage 2. Specifically:
add a `build/aims-history/` directory where each measurement run records a JSON
file with `{commit, profile, tier, program, metrics: {rc_count, exec_time_ms,
peak_rss_kb, compile_time_ms}}`.
<!-- reviewed: completeness fix — This proposes new infrastructure (build/aims-history/
directory, JSON recording). This is a tooling enhancement, not a plan document edit.
The plan edit portion (defining the tier comparison matrix) is sound. The infrastructure
should be documented in the plan as a future tooling item, not as a Section 08 checklist
item. -->

**E4. Add phase-level compile-time breakdown to 08.7 checklist.** The checklist
item "Compile-time breakdown documented" is present but not-yet-measured. Given the
paper's methodology of isolating the memory-management pass, AIMS should instrument
`run_aims_pipeline()` in `pipeline/aims_pipeline.rs` with `tracing::info_span!()`
around each phase (interprocedural, intraprocedural, RC emission, reuse emission,
COW/drop hints) and report durations as a percentage of total ARC pipeline time.
<!-- reviewed: completeness fix — ALREADY PRESENT as checklist item. Section 08.7 line
469: "Compile-time breakdown documented (interprocedural, intraprocedural, emission
percentages)". Section 08.5 line 362: "Compile-time breakdown: Not yet measured."
What is NEW: the specific instrumentation approach (tracing::info_span! in
run_aims_pipeline). This is useful implementation guidance for the existing checklist
item. -->

### `diagnostics/aims-compare.sh`

**E5. Record hardware/environment context.** The script should emit a metadata
header with: hostname, CPU model (`/proc/cpuinfo` or `sysctl`), memory, OS version,
Rust version (`rustc --version`), LLVM version, commit hash, build profile. This
makes results reproducible and comparable across runs. Currently none of this is
recorded.
<!-- reviewed: completeness fix — Tooling enhancement, not a plan document edit. Sound
proposal. Should be tracked as a diagnostics/ improvement item. -->

**E6. Measure per-program peak RSS.** The script already captures behavioral output
by running AOT binaries. It should additionally capture peak RSS via `/usr/bin/time
-v` (Linux) and record it alongside RC counts. This implements K4.
<!-- reviewed: completeness fix — Section 08.5 already measures peak RSS (line 386:
"bench_medium peak RSS: old=80,400 KB, AIMS=80,400 KB"). The proposal is to add this
to aims-compare.sh as automated per-program capture. Sound tooling enhancement. -->

**E7. Add `--baseline` mode for tier tracking.** Add a `--save-baseline NAME` flag
that writes results to `build/aims-baselines/NAME.json` and a `--compare-baseline
NAME` flag that compares current results against a saved baseline. This enables the
tier-tracking workflow from K3.
<!-- reviewed: completeness fix — Tooling enhancement. Not a plan document edit. -->

### `pipeline/shadow/` infrastructure

**E8. Document the isolation guarantee in `shadow.rs` module doc.** The module doc
(lines 1-18 of `shadow.rs`) explains what is compared but not why the comparison is
valid. Add a paragraph explaining the confounding-variable isolation: both pipelines
consume the same `ArcFunction` IR (produced by the same lowering), emit to the same
LLVM backend, and use the same runtime. The only variable is the analysis and RC
emission logic.
<!-- reviewed: completeness fix — Sound documentation enhancement. The isolation
guarantee is implicit in the current code but should be explicit. -->

### Golden corpus

**E9. Freeze golden corpus programs.** Add a note to Section 08.1 golden corpus
definition (lines 97-110 of `section-08-verification.md`) that golden corpus programs
are frozen after baseline establishment. Any modification requires archiving the old
version and re-establishing baselines. This implements N3.
<!-- reviewed: completeness fix — Sound proposal. The golden corpus definition (Section
08.1 lines 97-110) lists the programs but does not specify a freeze policy. Adding one
prevents silent baseline drift. -->

---

## 05.5 Code Changes (Later)

### `pipeline/shadow/compare.rs`

**C1. Add dynamic metric attribution to `FunctionComparison`.** Currently
`immortal_skips` is the only attribution field. Extend the comparison to attribute
RC count improvements to specific mechanisms: borrow improvement (AIMS borrows where
legacy owned), uniqueness improvement (AIMS proves unique where legacy said
MaybeShared), coalescing (inc/dec pairs eliminated), reuse (Construct replaced by
Reuse). This requires adding fields to `FunctionComparison`:
```
pub borrow_attributed_savings: usize,
pub uniqueness_attributed_savings: usize,
pub coalesce_attributed_savings: usize,
pub reuse_attributed_savings: usize,
```
These would be populated by analyzing the per-instruction state map during
the comparison phase.

**C2. Add `compare_execution_metrics()` function.** The shadow pipeline currently
compares static analysis artifacts only. For dynamic comparison, add an optional
runtime comparison mode that compiles both pipelines' output, executes them, and
compares wall-clock time and peak RSS. This is too expensive for default shadow
mode but could be triggered by a `--bench` flag in `aims-compare.sh`.

### `pipeline/shadow.rs`

**C3. Add isolation assertion to `run_shadow_pipeline_all()`.** At the top of the
function, assert that the functions being compared are unmodified (no RC ops
present). This validates the precondition that both pipelines start from the same
IR. The assertion already holds implicitly (the functions come from lowering before
any analysis), but making it explicit prevents future regressions if pipeline
ordering changes.

### `diagnostics/aims-compare.sh`

**C4. Implement E5-E7 from Plan Edits.** Hardware context header, per-program RSS
measurement, and baseline save/compare mode.

**C5. Add per-benchmark breakdown output.** Currently the script reports aggregate
totals and per-file deltas. Add a summary table at the end (similar to the paper's
Figure 2) showing each golden corpus program's metrics side-by-side:
```
Program           | Old RC | AIMS RC | Delta | Old Time | AIMS Time | Delta
closure_capture   |     48 |      17 |  -65% |          |           |
cow_chain         |     74 |      22 |  -70% |          |           |
...
```
This makes the output directly comparable to the paper's presentation format.

### `pipeline/aims_pipeline.rs`

**C6. Add phase-level timing instrumentation.** Wrap each pipeline phase in
`tracing::info_span!()` and emit phase durations. When `ORI_LOG=ori_arc=info`,
the output should show:
```
AIMS pipeline: interprocedural 1.2ms, intraprocedural 0.8ms, rc_emission 0.3ms,
reuse_emission 0.2ms, cow_annotations 0.1ms, drop_hints 0.1ms, total 2.7ms
```
This implements E4 and enables compile-time regression attribution.

---

## 05.6 Lens Shift

This paper changes how we read the remaining theory papers (06-09) in two ways:

**L1. Every paper's claimed improvement must be testable under isolation.** When
Paper 06 (Linearity vs Uniqueness) argues that separating linearity from uniqueness
enables better optimization, the question becomes: can AIMS demonstrate this
improvement with a feature-flag comparison where the only variable is whether
linearity-uniqueness separation is active? If the improvement cannot be isolated
to a single mechanism, it is not proven -- it might be an artifact of other changes.
This is the same rigor the paper applies to RC-vs-GC.

**L2. Cross-system benchmark comparisons in theory papers should be read with
skepticism.** Several papers in the AIMS lineage (Perceus, FP2, Counting Immutable
Beans) report cross-language benchmarks (Koka vs OCaml vs Lean vs Haskell). This
paper's core thesis is that such comparisons are confounded. When reading benchmark
tables in Papers 06-09, focus on: (a) the algorithmic insight being demonstrated,
(b) whether the improvement can be isolated within AIMS's single-system framework,
and (c) what AIMS-specific benchmarks would test the claim fairly.

**L3. Incomplete prototypes are informative.** The paper honestly states its
prototype lacks exceptions, mutable references, and reuse analysis. It reports
results anyway, with appropriate caveats. This is relevant to AIMS Stage 1C: the
current +4% RC regression on the spec suite is analogous to the paper's rbtree
regression without reuse. Both are expected to improve with future optimization
tiers. The lesson is to report intermediate results with clear attribution of what
is and is not yet implemented, rather than waiting for a complete system.

**L4. Runtime integration quality matters independently of analysis quality.**
The paper's Section 3 (adapting OCaml for RC) shows that mimalloc integration,
fast-path inlining, and calling-convention adaptation have performance effects
independent of the RC algorithm's quality. For AIMS, this means that even a perfect
analysis producing optimal RC placement can be undermined by poor codegen (e.g.,
unnecessary register spills around RC calls, failure to inline fast paths). AIMS
evaluation should eventually include codegen-quality metrics (instruction count per
RC operation, branch prediction success rate) alongside static RC counts.

---

## 05.7 Open Risk

**O1. AIMS's shadow comparison (`aims-shadow`) measures static artifacts, not
runtime behavior.** The paper compares execution time and peak RSS. AIMS's
`ShadowComparisonReport` compares param ownership, return uniqueness, COW
annotations, RC counts, and arg ownership -- all static properties of the ARC IR.
The `aims-compare.sh` script does compare behavioral output (exit code + stdout)
but does not compare execution time or memory. There is currently no mechanism to
detect a case where AIMS produces fewer static RC ops but *slower* runtime
performance (e.g., because the remaining RC ops are in hotter code paths, or because
AIMS's placement defeats branch prediction or instruction cache locality).

**Mitigation:** C2 proposes an optional runtime comparison mode. Until implemented,
AIMS should run `hyperfine` comparisons on golden corpus programs periodically
(not just cow_chain.ori as currently reported in Section 08.5).

**O2. The `aims-compare.sh` script rebuilds between passes, introducing build
variability.** The script builds the old pipeline, captures data, then rebuilds
with `--features aims` and captures data. If the machine's thermal state, background
load, or filesystem cache state differs between passes, runtime comparisons would be
confounded. For static metrics (RC counts, ARC IR dumps) this is not a problem.
For any future runtime comparison, the script should randomize measurement order
or interleave measurements.

**Mitigation:** For now, static comparisons are safe. E5 (recording hardware context)
partially addresses this. Full mitigation requires interleaved measurement, which
is architecturally difficult with the current build-then-measure approach.

**O3. The shadow pipeline clones all functions for AIMS analysis.** In
`run_shadow_pipeline_all()` (line 180 of `shadow.rs`), all functions are cloned
(`functions.to_vec()`) so that AIMS can mutate its copies while legacy uses the
originals. This means the shadow comparison is measuring AIMS running on a deep
copy, not on the same memory. For correctness comparison this is fine, but for
compile-time comparison it adds clone overhead. The shadow pipeline should not be
used for compile-time benchmarking.

**O4. Golden corpus size is small.** Five programs in `tests/aims/` is comparable
to the paper's five benchmarks, but the paper acknowledges this is "too limited to
draw any firm conclusions." AIMS should grow the golden corpus to at least 15-20
programs covering: (a) allocation-heavy loops, (b) deep recursion with ADTs,
(c) closure-heavy code, (d) COW-intensive mutations, (e) mixed borrowed/owned
call patterns, (f) programs where GC would outperform naive RC (many short-lived
objects, sharing-heavy). Category (f) is the AIMS equivalent of the paper's rbtree
benchmark -- the hardest case for RC, and therefore the most informative.

**O5. No regression-gate escalation path.** Section 08 defines gates (behavioral
equivalence, Valgrind, compilation speed, codegen quality) but does not define
what happens when a gate fails intermittently. The paper implicitly assumes
deterministic execution (mean of five runs). If an AIMS measurement shows a 12%
compilation slowdown on one run and 8% on another, is the 10% gate passed or
failed? Section 08.5 should specify: use median of N runs (not mean), N >= 10,
and the gate applies to the median. This is more robust to outliers than mean-
of-5 as used in the paper.
