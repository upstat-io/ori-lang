//! Interpreter (tree-walk evaluator) micro-benchmarks for Ori.
//!
//! Measures `evaluated()` — the same Salsa-tracked query the production `ori run`
//! entry calls — over a simple->complex kernel ladder. Each kernel isolates ONE
//! eval dispatch construct so a regression attributes to a single lever.
//!
//! Timed-region discipline (mirrors the corrected split-bench in `parser.rs`):
//! construct the `CompilerDb` + `SourceFile` ONCE per kernel, warm the query, then
//! per-iter toggle the input between two equivalent-cost source variants and
//! `black_box` only the re-`evaluated()` recompute. Folding DB/SourceFile
//! construction into the timed region times the whole front-end, not the
//! interpreter (the parser-perf input-setup confound). Each kernel does heavy
//! looped work so the interpreter dominates the fixed lex+parse+typecheck of the
//! tiny program. Runnable under callgrind for instruction-count baselines.

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{criterion_group, Criterion};
use oric::query::evaluated;
use oric::{CompilerDb, SourceFile};
use salsa::Setter;

// --- Kernel ladder (simple -> complex) over the live eval dispatch surface ---
// Every kernel is a standalone Ori program with a heavy-work `@main`; all are
// smoke-verified to run on the interpreter (`ori run`).

/// Integer arithmetic accumulation in a loop — `eval_can_binary` + loop dispatch.
const ARITHMETIC_LOOP: &str = r#"@main () -> int = {
    let total = 0;
    for i in 0..2000 do {
        total = total + i * 2 - 1;
    };
    total
}"#;

/// Free-function call in a loop — `eval_call` (the dominant hotspot) + `prepare_call_env`.
const FUNCTION_CALL_LOOP: &str = r#"@inc (x: int) -> int = x + 1;

@main () -> int = {
    let total = 0;
    for _ in 0..2000 do {
        total = inc(x: total);
    };
    total
}"#;

/// Method call in a loop — `eval_method_body` (shares the function-call allocation shape).
const METHOD_CALL_LOOP: &str = r#"type Counter = { value: int }

impl Counter {
    @bumped (self, amt: int) -> Counter = Counter { value: self.value + amt }
}

@main () -> int = {
    let c = Counter { value: 0 };
    for i in 0..2000 do {
        c = c.bumped(amt: i);
    };
    c.value
}"#;

/// List comprehension + iteration — `eval_can_for` + `eval_iter_next`.
const FOR_YIELD: &str = r#"@main () -> int = {
    let total = 0;
    for _ in 0..200 do {
        let xs = for i in 0..100 yield i * i;
        for x in xs do {
            total = total + x;
        };
    };
    total
}"#;

/// List element update in a loop — list COW `make_mut` in-place-when-unique path.
const LIST_INDEX_ASSIGN: &str = r#"@main () -> int = {
    let xs = for _ in 0..100 yield 0;
    for _ in 0..200 do {
        for i in 0..100 do {
            xs = xs.set(index: i, value: i * 2);
        };
    };
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}"#;

/// List grow/shrink in a loop — `ListData` push/pop + COW.
const LIST_APPEND_POP: &str = r#"@main () -> int = {
    let total = 0;
    for _ in 0..200 do {
        let xs = [0];
        for i in 1..40 do {
            xs = xs.push(i);
        };
        for _ in 0..40 do {
            let v = xs.pop();
            if v.is_none() then break;
            xs = xs.take(xs.length() - 1);
        };
        total = total + xs.length();
    };
    total
}"#;

/// Map insert + lookup in a loop — `dispatch_map_method` + map COW.
const MAP_INSERT_LOOKUP: &str = r#"@main () -> int = {
    let total = 0;
    for _ in 0..200 do {
        let m: {int: int} = {};
        for i in 0..50 do {
            m = m.insert(i, i * 2);
        };
        for i in 0..50 do {
            total = total + m.get(i).unwrap_or(default: 0);
        };
    };
    total
}"#;

/// String concatenation in a loop — `dispatch_string_method` + string COW.
const STRING_CONCAT: &str = r#"@main () -> int = {
    let total = 0;
    for _ in 0..200 do {
        let s = "";
        for _ in 0..50 do {
            s = s + "ab";
        };
        total = total + s.len();
    };
    total
}"#;

/// Closure capturing an outer binding, called in a loop — `bind_captures`.
const CLOSURE_CAPTURE: &str = r#"@main () -> int = {
    let total = 0;
    for i in 0..2000 do {
        let base = i;
        let add_base = (x: int) -> x + base;
        total = total + add_base(10);
    };
    total
}"#;

/// Sum-type construction + `match` dispatch in a loop — `eval_can_match`.
const PATTERN_MATCH_DISPATCH: &str = r#"type Shape = Circle(r: int) | Square(s: int) | Tri(a: int, b: int);

@area2 (sh: Shape) -> int = match sh {
    Circle(r) -> r * r * 3,
    Square(s) -> s * s,
    Tri(a, b) -> a * b / 2,
}

@main () -> int = {
    let total = 0;
    for i in 0..2000 do {
        let sh = match i % 3 {
            0 -> Circle(r: i),
            1 -> Square(s: i),
            _ -> Tri(a: i, b: i + 1),
        };
        total = total + area2(sh: sh);
    };
    total
}"#;

/// Nested list build + traversal — integration kernel exercising multiple dispatch arms.
const NESTED_COLLECTIONS: &str = r#"@main () -> int = {
    let total = 0;
    for _ in 0..100 do {
        let grid = for i in 0..30 yield (for j in 0..30 yield i * j);
        for row in grid do {
            for v in row do {
                total = total + v;
            };
        };
    };
    total
}"#;

/// Salsa-recompute split-bench: build the `CompilerDb` + `SourceFile` ONCE, warm the
/// query, then per-iter toggle the input between two equivalent-cost variants so each
/// iteration registers a genuine change and forces an `evaluated()` recompute of equal
/// work — with NO DB/SourceFile reconstruction in the timed region. Variant B prepends
/// an uncalled top-level fn: it changes the parsed module (so Salsa does not backdate
/// `evaluated`) without changing what `@main` evaluates.
fn bench_kernel(c: &mut Criterion, name: &str, body: &str) {
    let src_a = body.to_string();
    let src_b = format!("@_alt_variant () -> int = 1;\n{body}");
    let mut db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src_a.clone());
    let _ = evaluated(&db, file); // warm (cold compile out of the timed region)

    let mut toggle = false;
    c.bench_function(name, |b| {
        b.iter(|| {
            // Alternate between two equivalent-cost sources so each iter is a real change.
            let next = if toggle { src_a.clone() } else { src_b.clone() };
            toggle = !toggle;
            file.set_text(&mut db).to(next);
            black_box(evaluated(&db, file));
        });
    });
}

/// The kernel ladder, in simple->complex order. Single source consumed by both the
/// Criterion wall-time path and the callgrind single-shot instruction-count path.
const KERNELS: &[(&str, &str)] = &[
    ("arithmetic_loop", ARITHMETIC_LOOP),
    ("function_call_loop", FUNCTION_CALL_LOOP),
    ("method_call_loop", METHOD_CALL_LOOP),
    ("for_yield", FOR_YIELD),
    ("list_index_assign", LIST_INDEX_ASSIGN),
    ("list_append_pop", LIST_APPEND_POP),
    ("map_insert_lookup", MAP_INSERT_LOOKUP),
    ("string_concat", STRING_CONCAT),
    ("closure_capture", CLOSURE_CAPTURE),
    ("pattern_match_dispatch", PATTERN_MATCH_DISPATCH),
    ("nested_collections", NESTED_COLLECTIONS),
];

fn bench_interpreter(c: &mut Criterion) {
    for (name, body) in KERNELS {
        bench_kernel(c, &format!("interpreter/{name}"), body);
    }
}

/// Callgrind single-shot capture: run ONE kernel's `evaluated()` recompute a FIXED
/// number of times so the total instruction count is deterministic (the per-commit
/// regression gate diffs current-vs-baseline; the constant startup + cold-compile
/// offset cancels). Same warm-once-then-toggle-recompute discipline as the Criterion
/// path. Gated by `ORI_INTERP_CALLGRIND_KERNEL`; `ORI_INTERP_CALLGRIND_ITERS` sets
/// the fixed recompute count (default 50).
fn run_single_shot(name: &str, iters: usize) {
    let Some((_, body)) = KERNELS.iter().find(|(n, _)| *n == name) else {
        eprintln!("unknown kernel: {name}");
        std::process::exit(2);
    };
    let src_a = body.to_string();
    let src_b = format!("@_alt_variant () -> int = 1;\n{body}");
    let mut db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src_a.clone());
    let _ = evaluated(&db, file); // warm (cold compile out of the fixed-count region)

    let mut toggle = false;
    for _ in 0..iters {
        let next = if toggle { src_a.clone() } else { src_b.clone() };
        toggle = !toggle;
        file.set_text(&mut db).to(next);
        black_box(evaluated(&db, file));
    }
}

criterion_group!(benches, bench_interpreter);

fn main() {
    if let Ok(name) = std::env::var("ORI_INTERP_CALLGRIND_KERNEL") {
        let iters = std::env::var("ORI_INTERP_CALLGRIND_ITERS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(50);
        run_single_shot(&name, iters);
        return;
    }
    benches();
    Criterion::default().configure_from_args().final_summary();
}
