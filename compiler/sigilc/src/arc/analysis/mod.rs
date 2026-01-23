// Analysis module for ARC Memory Management
//
// This module contains compile-time analysis components:
// - Type classification (value vs reference types)
// - Cycle detection (Tarjan's SCC algorithm)
// - Size calculation (platform-aware type sizes)

pub mod classification;
pub mod cycle_detection;
pub mod size_calculator;

// Re-export main types and functions
pub use classification::{classify, is_value, requires_destruction, DefaultTypeClassifier};
pub use cycle_detection::{
    check_module_cycles, check_type_cycles, find_cycles_tarjan, DefaultCycleDetector,
};
pub use size_calculator::{PlatformSizes, TypeSizeCalculator};
