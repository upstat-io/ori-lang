---
section: "01"
title: "Performance Baselines"
status: not-started
reviewed: true
goal: "Establish reproducible performance baselines for lexer, parser, and Salsa query throughput using Criterion benchmarks."
inspired_by:
  - "Chumsky benches/ (json.rs, lex.rs, backtrack.rs) — benchmark organization"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Run Existing Benchmarks"
    status: not-started
  - id: "01.2"
    title: "Add Missing Benchmark Workloads"
    status: not-started
  - id: "01.3"
    title: "Record Baselines"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Performance Baselines

**Status:** Not Started
**Goal:** All existing lexer, parser, and Salsa benchmarks run cleanly, results recorded as baseline JSON files, and any missing workload coverage identified and added. Future sections measure against these baselines to prove improvement or detect regressions.

**Context:** Ori already has a comprehensive Criterion benchmark suite (8 files, ~2,673 lines) covering lexer throughput, parser throughput (raw + Salsa), incremental parsing, and scaling. However, baselines need to be formally captured and the benchmark suite has gaps — no string-heavy workloads, no deeply nested expression benchmarks targeting the Pratt parser specifically, and no benchmark that isolates Salsa query overhead from raw parsing.

**Reference implementations:**
- **Chumsky** `benches/json.rs`: Real-world format parsing benchmark — shows the value of realistic workloads vs synthetic scaling tests.
- **Chumsky** `benches/backtrack.rs`: Heavy backtracking scenario (1000x5 patterns) — tests worst-case performance.

**Depends on:** None.

---

## 01.1 Run Existing Benchmarks

**File(s):** `compiler/oric/benches/lexer.rs`, `compiler/oric/benches/lexer_core.rs`, `compiler/oric/benches/parser.rs`

Run the full benchmark suite and verify all benchmarks produce stable results. Establish the measurement environment and methodology.

- [ ] Run lexer core benchmarks and record results:
  ```bash
  cargo bench -p oric --bench lexer_core -- "raw/throughput"
  ```

- [ ] Run cooked lexer benchmarks and record results:
  ```bash
  cargo bench -p oric --bench lexer -- "raw/throughput"
  ```

- [ ] Run parser benchmarks (both Salsa and raw) and record results:
  ```bash
  cargo bench -p oric --bench parser -- "parser/raw"
  ```

- [ ] Verify benchmark stability: run each benchmark 3 times, confirm variance < 5%. If variance exceeds 5%, identify the cause (CPU throttling, background processes) and stabilize before recording baselines.

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.2 Add Missing Benchmark Workloads

**File(s):** `compiler/oric/benches/parser.rs`, `compiler/oric/benches/lexer.rs`

Current benchmarks use `generate_n_functions()` which produces simple `@funcN (x: int) -> int = x + N` lines. This under-represents:
- **String-heavy source** (template literals, escape sequences — exercises cooker hot paths)
- **Expression-heavy source** (deeply nested binary ops — exercises Pratt parser loop)
- **Real-world Ori code** (mix of types, traits, impls, patterns — exercises full grammar)

- [ ] Add string-heavy benchmark workload to `lexer.rs`:
  ```rust
  fn generate_string_heavy(n: usize) -> String {
      (0..n)
          .map(|i| format!(r#"let $msg{i} = "hello \n world \t {i} \u{{0041}}""#))
          .collect::<Vec<_>>()
          .join("\n")
  }
  ```

- [ ] Add expression-heavy benchmark workload to `parser.rs`:
  ```rust
  fn generate_expr_heavy(depth: usize) -> String {
      let mut expr = "x".to_string();
      for _ in 0..depth {
          expr = format!("({expr} + y * z - w)");
      }
      format!("@compute (x: int, y: int, z: int, w: int) -> int = {expr}")
  }
  ```

- [ ] Add Salsa-isolation benchmark to `parser.rs` that measures the delta between `parser/raw` and Salsa-mediated `parser/` throughput for the same workload, to quantify Salsa overhead precisely.
  - **Verified:** `CompilerDb::new()`, `SourceFile::new(&db, PathBuf, String)`, and `parsed(&db, file)` are the correct API signatures (confirmed from existing benchmarks in `parser.rs:74-187`). Required imports already present: `use oric::{CompilerDb, SourceFile}; use oric::query::parsed;`.
  - **Verified:** `ori_lexer::lex()` and `ori_parse::parse()` are the correct free-function APIs for the raw path. Required imports: `use ori_ir::StringInterner;`.
  ```rust
  fn bench_salsa_overhead(c: &mut Criterion) {
      let interner = StringInterner::new();
      let db = CompilerDb::new();
      let source = generate_n_functions(500);
      let bytes = source.len() as u64;
      let mut group = c.benchmark_group("parser/salsa_overhead");
      group.throughput(Throughput::Bytes(bytes));

      group.bench_function("raw", |b| {
          b.iter(|| {
              let tokens = ori_lexer::lex(&source, &interner);
              black_box(ori_parse::parse(&tokens, &interner));
          });
      });

      group.bench_function("via_salsa", |b| {
          b.iter(|| {
              let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), source.clone());
              black_box(parsed(&db, file));
          });
      });

      group.finish();
  }
  ```

- [ ] Add all new benchmarks to the `criterion_group!` macro in their respective files.

**Matrix dimensions (test coverage):**
- Workload types: simple functions, string-heavy, expression-heavy, realistic mixed
- Sizes: small (1KB), medium (10KB), large (50KB)
- Modes: raw (no Salsa), Salsa query, incremental (cached)

**Semantic pin:** The Salsa overhead benchmark isolates query system cost. If Salsa overhead exceeds 50% of raw parse time, Section 05 has clear evidence for granularity work.

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.3 Record Baselines

**File(s):** New file `plans/parser-perf/baselines.json`

Capture all benchmark results in a structured format for comparison in Section 06.

- [ ] Run complete benchmark suite with `--save-baseline parser-perf-v0`:
  ```bash
  cargo bench -p oric --bench lexer_core -- --save-baseline parser-perf-v0
  cargo bench -p oric --bench lexer -- --save-baseline parser-perf-v0
  cargo bench -p oric --bench parser -- --save-baseline parser-perf-v0
  ```

- [ ] Record key metrics in `plans/parser-perf/baselines.json`:
  ```json
  {
    "date": "YYYY-MM-DD",
    "commit": "<hash>",
    "environment": "<cpu, os, rust version>",
    "results": {
      "lexer_core_raw_throughput_mib_s": null,
      "lexer_cooked_throughput_mib_s": null,
      "parser_raw_throughput_mib_s": null,
      "parser_salsa_throughput_mib_s": null,
      "salsa_overhead_percent": null
    }
  }
  ```

- [ ] Verify baselines match expected ranges from memory (lexer core: ~720-1020 MiB/s, cooked: ~208-240 MiB/s, parser: ~95-128 MiB/s). If results differ significantly, investigate before proceeding.

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] All existing benchmarks run cleanly (0 failures)
- [ ] Benchmark variance < 5% across 3 runs for each benchmark
- [ ] New string-heavy and expression-heavy workloads added
- [ ] Salsa overhead isolation benchmark added
- [ ] Baselines recorded in `plans/parser-perf/baselines.json`
- [ ] Baseline throughput values within expected ranges
- [ ] `./test-all.sh` green (no regressions from benchmark additions)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `cargo bench -p oric --bench lexer_core -- "lexer_core/raw"`, `cargo bench -p oric --bench lexer -- "lexer/raw"`, and `cargo bench -p oric --bench parser -- "parser/raw"` all run cleanly. `baselines.json` contains populated results for all metric fields. Results are stable (< 5% variance across 3 runs).
