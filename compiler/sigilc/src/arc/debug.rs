// Debug Support for ARC Memory Management
//
// Provides leak detection and allocation tracking for debugging
// memory management issues.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::ids::TypeId;
use super::traits::{AllocationEntry, AllocationTracker};

/// Default implementation of AllocationTracker
///
/// Thread-safe allocation tracker using a mutex-protected hash map.
pub struct DefaultAllocationTracker {
    /// Allocations indexed by type ID
    allocations: Mutex<HashMap<TypeId, AllocationEntry>>,

    /// Next allocation ID
    next_id: Mutex<u32>,
}

impl Default for DefaultAllocationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultAllocationTracker {
    /// Create a new tracker
    pub fn new() -> Self {
        DefaultAllocationTracker {
            allocations: Mutex::new(HashMap::new()),
            next_id: Mutex::new(0),
        }
    }

    /// Get the number of current allocations
    pub fn allocation_count(&self) -> usize {
        self.allocations.lock().unwrap().len()
    }

    /// Check if there are any allocations (potential leaks)
    pub fn has_leaks(&self) -> bool {
        !self.allocations.lock().unwrap().is_empty()
    }

    /// Clear all tracked allocations
    pub fn clear(&self) {
        self.allocations.lock().unwrap().clear();
    }
}

impl AllocationTracker for DefaultAllocationTracker {
    fn record_alloc(&self, entry: AllocationEntry) {
        let mut allocations = self.allocations.lock().unwrap();
        allocations.insert(entry.type_id, entry);
    }

    fn record_release(&self, type_id: TypeId) -> bool {
        let mut allocations = self.allocations.lock().unwrap();
        if let Some(entry) = allocations.get_mut(&type_id) {
            if entry.refcount > 1 {
                entry.refcount -= 1;
                return false;
            }
            allocations.remove(&type_id);
            return true;
        }
        false
    }

    fn current_allocations(&self) -> Vec<AllocationEntry> {
        self.allocations.lock().unwrap().values().cloned().collect()
    }

    fn leak_report(&self) -> String {
        let allocations = self.allocations.lock().unwrap();

        if allocations.is_empty() {
            return "No memory leaks detected.".to_string();
        }

        let mut report = String::new();
        report.push_str("=== MEMORY LEAK REPORT ===\n");
        report.push_str(&format!("Leaked allocations: {}\n\n", allocations.len()));

        for (id, entry) in allocations.iter() {
            report.push_str(&format!(
                "  {}: {} (refcount: {})",
                id, entry.type_name, entry.refcount
            ));
            if let Some(loc) = &entry.source_location {
                report.push_str(&format!(" at {}", loc));
            }
            report.push('\n');
        }

        report
    }
}

/// Helper for tracking allocations during testing
pub struct TestTracker {
    tracker: DefaultAllocationTracker,
}

impl TestTracker {
    /// Create a new test tracker
    pub fn new() -> Self {
        TestTracker {
            tracker: DefaultAllocationTracker::new(),
        }
    }

    /// Simulate an allocation
    pub fn alloc(&self, type_name: &str) -> TypeId {
        let type_id = {
            let mut next_id = self.tracker.next_id.lock().unwrap();
            let id = TypeId::new(*next_id);
            *next_id += 1;
            id
        };

        self.tracker.record_alloc(AllocationEntry {
            type_id,
            type_name: type_name.to_string(),
            refcount: 1,
            source_location: None,
        });

        type_id
    }

    /// Simulate a retain
    pub fn retain(&self, type_id: TypeId) {
        let mut allocations = self.tracker.allocations.lock().unwrap();
        if let Some(entry) = allocations.get_mut(&type_id) {
            entry.refcount += 1;
        }
    }

    /// Simulate a release
    pub fn release(&self, type_id: TypeId) -> bool {
        self.tracker.record_release(type_id)
    }

    /// Check for leaks
    pub fn has_leaks(&self) -> bool {
        self.tracker.has_leaks()
    }

    /// Get leak report
    pub fn leak_report(&self) -> String {
        self.tracker.leak_report()
    }

    /// Get allocation count
    pub fn allocation_count(&self) -> usize {
        self.tracker.allocation_count()
    }
}

impl Default for TestTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard that checks for leaks when dropped
pub struct LeakCheckGuard {
    tracker: Arc<DefaultAllocationTracker>,
    check_on_drop: bool,
}

impl LeakCheckGuard {
    /// Create a new guard with a shared tracker
    pub fn new(tracker: Arc<DefaultAllocationTracker>) -> Self {
        LeakCheckGuard {
            tracker,
            check_on_drop: true,
        }
    }

    /// Disable leak checking on drop
    pub fn disable_check(&mut self) {
        self.check_on_drop = false;
    }
}

impl Drop for LeakCheckGuard {
    fn drop(&mut self) {
        if self.check_on_drop && self.tracker.has_leaks() {
            eprintln!("{}", self.tracker.leak_report());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_creation() {
        let tracker = DefaultAllocationTracker::new();
        assert_eq!(tracker.allocation_count(), 0);
        assert!(!tracker.has_leaks());
    }

    #[test]
    fn test_allocation_tracking() {
        let tracker = TestTracker::new();

        let id1 = tracker.alloc("String");
        assert_eq!(tracker.allocation_count(), 1);

        let id2 = tracker.alloc("List");
        assert_eq!(tracker.allocation_count(), 2);

        assert!(tracker.release(id1));
        assert_eq!(tracker.allocation_count(), 1);

        assert!(tracker.release(id2));
        assert_eq!(tracker.allocation_count(), 0);
        assert!(!tracker.has_leaks());
    }

    #[test]
    fn test_refcount_tracking() {
        let tracker = TestTracker::new();

        let id = tracker.alloc("String");
        tracker.retain(id);  // refcount = 2

        assert!(!tracker.release(id));  // refcount = 1, not freed
        assert_eq!(tracker.allocation_count(), 1);

        assert!(tracker.release(id));  // refcount = 0, freed
        assert_eq!(tracker.allocation_count(), 0);
    }

    #[test]
    fn test_leak_detection() {
        let tracker = TestTracker::new();

        let _id = tracker.alloc("LeakedString");

        assert!(tracker.has_leaks());
        let report = tracker.leak_report();
        assert!(report.contains("MEMORY LEAK"));
        assert!(report.contains("LeakedString"));
    }

    #[test]
    fn test_no_leaks_report() {
        let tracker = TestTracker::new();
        let report = tracker.leak_report();
        assert!(report.contains("No memory leaks"));
    }

    #[test]
    fn test_leak_check_guard() {
        let tracker = Arc::new(DefaultAllocationTracker::new());
        let mut guard = LeakCheckGuard::new(tracker.clone());

        // Record an allocation
        tracker.record_alloc(AllocationEntry {
            type_id: TypeId::new(0),
            type_name: "Test".to_string(),
            refcount: 1,
            source_location: None,
        });

        // Disable check so test doesn't fail
        guard.disable_check();

        // Clean up
        tracker.record_release(TypeId::new(0));
    }
}
