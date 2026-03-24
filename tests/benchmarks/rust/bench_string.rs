// Benchmark: String-heavy
// Exercises: concatenation, searching, splitting, case conversion
// Measures: string allocation, UTF-8 handling

fn build_string(n: usize) -> String {
    let mut s = String::new();
    for _ in 0..n {
        s.push('x');
    }
    s
}

fn count_char(s: &str, target: char) -> i64 {
    let mut count: i64 = 0;
    for ch in s.chars() {
        if ch == target { count += 1; }
    }
    count
}

fn repeat_transform(s: &str, n: usize) -> String {
    let mut result = s.to_string();
    for _ in 0..n {
        result = result.to_uppercase();
        result = result.to_lowercase();
    }
    result
}

fn main() -> std::process::ExitCode {
    // String building
    let mut build_total: usize = 0;
    for _ in 0..100 {
        let s = build_string(1000);
        build_total += s.len();
    }

    // String searching
    let mut search_total: i64 = 0;
    let haystack = build_string(5000);
    for _ in 0..1000 {
        if haystack.contains("xxx") { search_total += 1; }
    }

    // Case conversion
    let mut case_total: usize = 0;
    for _ in 0..100 {
        let s = "Hello World Test String For Benchmark";
        let transformed = repeat_transform(s, 100);
        case_total += transformed.len();
    }

    // String concatenation patterns
    let mut concat_total: usize = 0;
    for _ in 0..1000 {
        let a = "hello";
        let b = " ";
        let c = "world";
        let combined = format!("{}{}{}", a, b, c);
        concat_total += combined.len();
    }

    if build_total > 0 && search_total > 0 && case_total > 0 && concat_total > 0 {
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(1)
    }
}
