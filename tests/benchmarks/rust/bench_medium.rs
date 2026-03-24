// Benchmark: Medium (~100 lines)
// Exercises: structs, closures, iterators, pattern matching, string ops, error handling
// Expected: balanced mix of allocation and computation

struct Point {
    x: i64,
    y: i64,
}

struct Stats {
    count: i64,
    sum: i64,
    min_val: i64,
    max_val: i64,
}

fn distance_squared(a: &Point, b: &Point) -> i64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn make_point(i: i64) -> Point {
    Point { x: i * 3 + 1, y: i * 7 - 2 }
}

fn classify(n: i64) -> &'static str {
    match n % 4 {
        0 => "zero",
        1 => "one",
        2 => "two",
        _ => "three",
    }
}

fn safe_divide(a: i64, b: i64) -> Result<i64, &'static str> {
    if b == 0 { Err("division by zero") } else { Ok(a / b) }
}

fn chain_divide(a: i64, b: i64, c: i64) -> Result<i64, &'static str> {
    let first = safe_divide(a, b)?;
    let second = safe_divide(first, c)?;
    Ok(second)
}

fn compute_stats(values: &[i64]) -> Stats {
    let mut count: i64 = 0;
    let mut sum: i64 = 0;
    let mut min_val: i64 = 999999;
    let mut max_val: i64 = -999999;

    for &v in values {
        count += 1;
        sum += v;
        if v < min_val { min_val = v; }
        if v > max_val { max_val = v; }
    }

    Stats { count, sum, min_val, max_val }
}

fn transform_pipeline(n: i64) -> i64 {
    let values: Vec<i64> = (0..n).map(|i| i * i).collect();
    values.iter()
        .filter(|&&x| x % 2 == 0)
        .map(|&x| x + 1)
        .take(50)
        .fold(0i64, |acc, x| acc + x)
}

fn build_points(n: i64) -> Vec<Point> {
    (0..n).map(|i| make_point(i)).collect()
}

fn closest_to_origin(points: &[Point]) -> i64 {
    let mut best: i64 = 999999999;
    for p in points {
        let d = p.x * p.x + p.y * p.y;
        if d < best { best = d; }
    }
    best
}

fn string_work() -> i64 {
    let s = "hello";
    let t = " world";
    let combined = format!("{}{}", s, t);
    let len = combined.len();
    let upper = combined.to_uppercase();
    let has_hello = upper.contains("HELLO");
    if has_hello && len == 11 { 1 } else { 0 }
}

fn recursive_sum(n: i64, acc: i64) -> i64 {
    if n == 0 { acc } else { recursive_sum(n - 1, acc + n) }
}

fn main() -> std::process::ExitCode {
    // Struct operations
    let points = build_points(100);
    let closest = closest_to_origin(&points);

    // Iterator pipeline
    let pipeline_result = transform_pipeline(200);

    // Stats computation
    let data: Vec<i64> = (0..500).map(|i| i % 100).collect();
    let stats = compute_stats(&data);

    // Recursion
    let rsum = recursive_sum(1000, 0);

    // Error handling
    let div_ok = chain_divide(100, 5, 2);
    let div_err = chain_divide(100, 0, 2);

    // Pattern matching
    let class_result = classify(42);

    // String work
    let str_ok = string_work();

    // Validate results
    let ok = closest > 0
        && pipeline_result > 0
        && stats.count == 500
        && rsum == 500500
        && div_ok.is_ok()
        && div_err.is_err()
        && class_result == "two"
        && str_ok == 1;

    if ok {
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(1)
    }
}
