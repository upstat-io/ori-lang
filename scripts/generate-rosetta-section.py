#!/usr/bin/env python3
"""
Generate a rosetta-stress-test plan section file from a list of task files.

Folder names match task file names (minus .md):
  _tasks/001_100_doors.md → rosetta/001_100_doors/
                           → rosetta/001_100_doors/task.md (copy)
                           → rosetta/001_100_doors/001_100_doors.ori
                           → rosetta/001_100_doors/_test/001_100_doors.test.ori

Usage:
    # First 15 tasks by index order:
    python3 scripts/generate-rosetta-section.py --section 01 --first 15

    # Specific tasks by index numbers:
    python3 scripts/generate-rosetta-section.py --section 02 --tasks 050,079,093,179,227

    # Range of tasks:
    python3 scripts/generate-rosetta-section.py --section 02 --range 038-060

    # Add --create-folders to also create directories and copy task.md:
    python3 scripts/generate-rosetta-section.py --section 01 --first 15 --create-folders

Outputs the full section file to stdout (redirect to plan file).
"""

import argparse
import re
import shutil
import sys
from pathlib import Path

ROSETTA_DIR = Path("tests/run-pass/rosetta")
TASKS_DIR = ROSETTA_DIR / "_tasks"


def get_all_tasks_sorted() -> list[Path]:
    """Get all task files sorted by numeric index."""
    if not TASKS_DIR.is_dir():
        return []
    tasks = list(TASKS_DIR.glob("*.md"))
    def sort_key(p):
        m = re.match(r"(\d+)_", p.name)
        return int(m.group(1)) if m else 999
    return sorted(tasks, key=sort_key)


def task_stem(task_path: Path) -> str:
    """Get the folder/program name from a task file: 001_100_doors.md → 001_100_doors"""
    return task_path.stem


def task_index(task_path: Path) -> str:
    """Get just the numeric index: 001_100_doors.md → 001"""
    m = re.match(r"(\d+)_", task_path.name)
    return m.group(1) if m else "???"


def task_human_name(task_path: Path) -> str:
    """Get human-readable name: 001_100_doors.md → 100 doors"""
    stem = task_path.stem
    m = re.match(r"\d+_(.*)", stem)
    return m.group(1).replace("_", " ") if m else stem


def discover_program_state(name: str) -> dict:
    """Discover the current state of a rosetta program by inspecting its folder."""
    folder = ROSETTA_DIR / name
    source = folder / f"{name}.ori"
    test_dir = folder / "_test"
    test_file = test_dir / f"{name}.test.ori"

    state = {
        "name": name,
        "exists": folder.is_dir(),
        "has_source": source.is_file(),
        "has_main": False,
        "has_test_dir": test_dir.is_dir(),
        "has_test_file": test_file.is_file(),
        "has_inline_tests": False,
        "has_task_md": (folder / "task.md").is_file(),
        "skip_reasons": [],
    }

    if state["has_source"]:
        content = source.read_text()
        state["has_main"] = "@main" in content
        state["has_inline_tests"] = "use std.testing" in content or "@test" in content

    if state["has_test_file"]:
        test_content = test_file.read_text()
        skips = re.findall(r'#\[?skip\("([^"]+)"\)', test_content)
        state["skip_reasons"] = list(set(skips))

    return state


def format_current_state(state: dict) -> str:
    """Format the current state line for a program."""
    parts = []
    if not state["exists"]:
        return "**New — folder not yet created**"
    if state["has_source"]:
        if state["has_main"]:
            parts.append("Has `@main`")
        else:
            parts.append("No `@main`")
        if state["has_test_file"]:
            parts.append("has `_test/`")
        elif state["has_inline_tests"]:
            parts.append("inline tests (needs extraction)")
        else:
            parts.append("no tests")
        if state["skip_reasons"]:
            parts.append(f"`#skip` ({', '.join(state['skip_reasons'])})")
    else:
        parts.append("Folder exists but no `.ori` source yet")
    if state["has_task_md"]:
        parts.append("has `task.md`")
    return ", ".join(parts) if parts else "Empty"


def generate_program_subsection(section_num: str, sub_num: int, task_path: Path, state: dict) -> str:
    """Generate the full subsection for one program with EVERY checkbox."""
    name = state["name"]
    task_file = task_path.name
    idx = task_index(task_path)
    human = task_human_name(task_path)
    sub_id = f"{section_num}.{sub_num}"
    current = format_current_state(state)
    path = f"tests/run-pass/rosetta/{name}"
    ori = f"{path}/{name}.ori"
    test = f"{path}/_test/{name}.test.ori"

    return f"""
## {sub_id} {name}

**#{idx} — {human}** | **Task file:** `_tasks/{task_file}` | **Current state:** {current}

### Setup
- [ ] Create folder `{path}/` if it does not exist: `mkdir -p {path}/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/{task_file} {path}/task.md`
- [ ] Read `{path}/task.md` — understand the problem requirements, success criteria, and expected outputs

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available)
- [ ] Write `{ori}` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `{test}` with `use std.testing {{ assert_eq }}` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check {ori}` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check {ori}` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check {ori}` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check {ori}` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test {test}` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run {ori}` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build {ori} -o /tmp/rosetta_{name}_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release {ori} -o /tmp/rosetta_{name}_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build {ori} -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build {ori} -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_{name}_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_{name}_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh {ori}` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh {ori}` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_{name}_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_{name}_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_{name}_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build {ori} -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build {ori} -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build {ori} -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh {ori}` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level {ori}` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh {ori}` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict {ori}` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh {ori}` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_{name}_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_{name}_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_{name}_debug /tmp/rosetta_{name}_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_{name}_stripped /tmp/rosetta_{name}_release && ls -la /tmp/rosetta_{name}_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run {ori}` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_{name}_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_{name}_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build {ori} -o /tmp/rosetta_{name}_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release {ori} -o /tmp/rosetta_{name}_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "{human} <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: {name} ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### {sub_id} Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out ({sub_id})** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### {sub_id} Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---"""


def generate_section(section_num: str, title: str, task_paths: list[Path]) -> str:
    """Generate a complete section file."""
    # Discover state for each program
    programs = []
    for tp in task_paths:
        name = task_stem(tp)
        state = discover_program_state(name)
        state["task_path"] = tp
        programs.append(state)

    # Build frontmatter sections list
    sections_yaml = ""
    if section_num == "01":
        sections_yaml += '  - id: "01.PRE"\n    title: "Infrastructure"\n    status: not-started\n'
    for i, p in enumerate(programs, 1):
        sections_yaml += f'  - id: "{section_num}.{i}"\n    title: "{p["name"]}"\n    status: not-started\n'
    sections_yaml += f'  - id: "{section_num}.R"\n    title: "Third Party Review Findings"\n    status: not-started\n'
    sections_yaml += f'  - id: "{section_num}.N"\n    title: "Completion Checklist"\n    status: not-started\n'

    # Build program selection table (sorted by _tasks/ index)
    table_rows = ""
    for i, p in enumerate(programs, 1):
        tp = p["task_path"]
        idx = task_index(tp)
        table_rows += f"| {i} | #{idx} | {p['name']} | `_tasks/{tp.name}` | {format_current_state(p)} |\n"

    # Build all subsections
    subsections = ""
    for i, p in enumerate(programs, 1):
        subsections += generate_program_subsection(section_num, i, p["task_path"], p)

    # Infrastructure subsection (only for section 01)
    infra = ""
    if section_num == "01":
        infra = """
## 01.PRE Infrastructure

- [ ] Create `tests/run-pass/rosetta/rosetta-manifest.json` with per-program schema (status, features, has_main, has_tests, aot_eligible, bugs_filed, language_findings, perf)
- [ ] Update `tests/run-pass/rosetta/README.md` documenting the per-program pipeline, manifest, and folder structure

- [ ] **Subsection close-out (01.PRE)**:
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

---"""

    depends = "[]" if section_num == "01" else f'["{int(section_num) - 1:02d}"]'
    count = len(programs)

    return f"""---
section: "{section_num}"
title: "{title}"
status: not-started
reviewed: true
goal: "Methodically implement/evaluate {count} Rosetta programs — each running every diagnostic tool, verification flag, and benchmark in the full pipeline."
success_criteria:
  - "rosetta-manifest.json describes all {count} programs with status, findings, perf data"
  - "Each task folder has task.md, source.ori with @main, _test/ with assertions"
  - "All {count} programs pass every step of the per-program pipeline (Phases A-J)"
  - "Every bug discovered filed or fixed"
  - "Every language/syntax gap documented with roadmap cross-reference"
  - "Performance baselines captured for all {count} programs"
  - "/tpr-review passed for every subsection"
inspired_by:
  - "Rosetta Code (deterministic, well-defined problems)"
  - "Rust compiletest (per-test expected outcomes)"
  - "Swift SIL tests (positive + negative ARC pairing)"
depends_on: {depends}
third_party_review:
  status: none
  updated: null
sections:
{sections_yaml}---

# Section {section_num}: {title}

**Status:** Not Started
**Goal:** Work through {count} Rosetta programs — each running every single diagnostic tool, verification flag, and benchmark.

**Program selection (ordered by `_tasks/` index):**

| # | Index | Program | Task File | Current State |
|---|-------|---------|-----------|--------------|
{table_rows}
---
{infra}{subsections}
## {section_num}.R Third Party Review Findings

- None.

---

## {section_num}.N Completion Checklist

- [ ] `rosetta-manifest.json` has accurate entries for all {count} programs (status, bugs, findings, perf)
- [ ] All {count} programs have: `task.md`, `<name>.ori` with `@main`, `_test/` with tests
- [ ] All {count} programs ran EVERY step of Phases A-J (no shortcuts, no abbreviations)
- [ ] `/tpr-review` passed for every subsection
- [ ] Passing programs: zero dual-exec mismatches, zero leaks, clean codegen audit, clean `--strict`, clean `ORI_VERIFY_ARC`, clean `ORI_VERIFY_EACH`, clean `ORI_LLVM_LINT`
- [ ] DWARF symbols verified on all AOT debug binaries
- [ ] Performance baselines recorded for all {count} programs (interpreter, debug, release, compile times, speedup ratios)
- [ ] Every bug filed (`/add-bug`) or fixed (`/fix-bug`)
- [ ] Every language/syntax gap documented in manifest with roadmap/bug-tracker cross-reference
- [ ] Blocked programs have explicit cross-references
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] Plan annotation cleanup
- [ ] Plan sync — update plan metadata
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
- [ ] **Run `/create-plan` to add next section** — task selection informed by this section's findings

**Exit Criteria:** All {count} programs fully evaluated through every step of the pipeline. Manifest complete with status, bugs, language findings, and performance data. Every blocked program has a concrete cross-reference. Primary deliverable = findings, fixes, and language insights.
"""


def create_folder_structure(task_paths: list[Path]) -> None:
    """Create folder structure and copy task.md for each program."""
    for tp in task_paths:
        name = task_stem(tp)
        folder = ROSETTA_DIR / name
        test_dir = folder / "_test"

        # Create directories
        test_dir.mkdir(parents=True, exist_ok=True)
        print(f"  mkdir -p {test_dir}", file=sys.stderr)

        # Copy task definition
        dst = folder / "task.md"
        if tp.is_file() and not dst.is_file():
            shutil.copy2(tp, dst)
            print(f"  cp {tp.name} → {dst}", file=sys.stderr)
        elif dst.is_file():
            print(f"  skip {dst} (already exists)", file=sys.stderr)
        else:
            print(f"  WARN: {tp} not found!", file=sys.stderr)


def resolve_tasks(args) -> list[Path]:
    """Resolve command-line arguments to a sorted list of task file paths."""
    all_tasks = get_all_tasks_sorted()

    if args.first:
        return all_tasks[:args.first]
    elif args.tasks:
        # Match by index prefix (e.g., "001" matches "001_100_doors.md")
        indices = [idx.strip().zfill(3) for idx in args.tasks.split(",")]
        result = []
        for tp in all_tasks:
            idx = task_index(tp)
            if idx.zfill(3) in indices:
                result.append(tp)
        return result
    elif args.range:
        m = re.match(r"(\d+)-(\d+)", args.range)
        if not m:
            print(f"ERROR: invalid range '{args.range}', use NNN-NNN", file=sys.stderr)
            sys.exit(1)
        lo, hi = int(m.group(1)), int(m.group(2))
        return [tp for tp in all_tasks if lo <= int(task_index(tp)) <= hi]
    elif args.from_existing:
        existing = sorted([
            d.name for d in ROSETTA_DIR.iterdir()
            if d.is_dir() and d.name != "_tasks" and not d.name.startswith(".")
        ])
        # Map existing folder names back to task paths
        stem_to_path = {task_stem(tp): tp for tp in all_tasks}
        return [stem_to_path[name] for name in existing if name in stem_to_path][:15]
    else:
        return []


def main():
    parser = argparse.ArgumentParser(description="Generate rosetta-stress-test plan section")
    parser.add_argument("--section", required=True, help="Section number (e.g., 01, 02)")
    parser.add_argument("--title", default=None, help="Section title (auto-generated if omitted)")
    parser.add_argument("--first", type=int, default=None, help="Use first N tasks by index order")
    parser.add_argument("--tasks", default=None, help="Comma-separated task index numbers (e.g., 001,012,022)")
    parser.add_argument("--range", default=None, help="Task index range (e.g., 038-060)")
    parser.add_argument("--from-existing", action="store_true", help="Use existing program folders")
    parser.add_argument("--create-folders", action="store_true", help="Create folder structure + copy task.md")
    args = parser.parse_args()

    task_paths = resolve_tasks(args)
    if not task_paths:
        print("ERROR: no tasks selected. Use --first, --tasks, --range, or --from-existing", file=sys.stderr)
        sys.exit(1)

    if args.create_folders:
        print(f"Creating folder structure for {len(task_paths)} programs:", file=sys.stderr)
        create_folder_structure(task_paths)
        print("Done.", file=sys.stderr)

    title = args.title or f"Programs Batch {args.section} ({len(task_paths)} programs)"
    if args.section == "01":
        title = f"Infrastructure + First {len(task_paths)} Programs"

    print(generate_section(args.section, title, task_paths))


if __name__ == "__main__":
    main()
