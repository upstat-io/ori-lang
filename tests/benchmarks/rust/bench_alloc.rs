// Benchmark: Allocation-heavy
// Exercises: Vec creation, struct allocation, iteration, collection ops
// Measures: allocation patterns, collection codegen

struct Point {
    x: i64,
    y: i64,
}

fn make_points(n: i64) -> Vec<Point> {
    (0..n).map(|i| Point { x: i * 3 + 1, y: i * 7 - 2 }).collect()
}

fn sum_points(points: &[Point]) -> i64 {
    let mut total: i64 = 0;
    for p in points {
        total += p.x + p.y;
    }
    total
}

fn filter_points(points: &[Point], threshold: i64) -> Vec<Point> {
    points.iter()
        .filter(|p| p.x + p.y > threshold)
        .map(|p| Point { x: p.x, y: p.y })
        .collect()
}

fn transform_list(n: i64) -> Vec<i64> {
    (0..n).map(|i| i * i + i + 1).collect()
}

fn nested_lists(n: i64) -> i64 {
    let mut total: i64 = 0;
    for i in 0..n {
        let inner: Vec<i64> = (0..100).map(|j| i * 100 + j).collect();
        for v in &inner {
            total += v;
        }
    }
    total
}

fn list_operations(n: i64) -> i64 {
    let list: Vec<i64> = (0..n).collect();
    let doubled: Vec<i64> = list.iter().map(|v| v * 2).collect();
    let filtered: Vec<i64> = doubled.iter().filter(|&&v| v % 3 != 0).copied().collect();
    let mut total: i64 = 0;
    for v in &filtered {
        total += v;
    }
    total
}

fn main() -> std::process::ExitCode {
    // Build and sum many point lists
    let mut point_sum: i64 = 0;
    for _ in 0..1000 {
        let pts = make_points(500);
        point_sum += sum_points(&pts);
    }

    // Filter operations
    let mut filter_sum: i64 = 0;
    for _ in 0..1000 {
        let pts = make_points(500);
        let filtered = filter_points(&pts, 100);
        filter_sum += sum_points(&filtered);
    }

    // List transformations
    let mut transform_sum: i64 = 0;
    for _ in 0..1000 {
        let t = transform_list(1000);
        for v in &t {
            transform_sum += v;
        }
    }

    // Nested list creation
    let nested = nested_lists(500);

    // Chained list operations
    let mut chained: i64 = 0;
    for _ in 0..1000 {
        chained += list_operations(1000);
    }

    if point_sum != 0 && filter_sum != 0 && transform_sum != 0 && nested != 0 && chained != 0 {
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(1)
    }
}
