// Test result types

/// Result of running a single test file
pub struct TestFileResult {
    pub path: String,
    pub passed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}
