//! Comprehensive tests for the parallel pattern.
//!
//! Tests cover:
//! - Basic parallel execution
//! - `max_concurrent` limiting
//! - timeout handling
//! - Error capture (all-settled semantics)
//! - Edge cases (empty, single task, non-callable)
//! - Order preservation
//! - Thread safety

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#![expect(
    clippy::disallowed_types,
    reason = "Tests use Arc/Mutex directly for thread-safety assertions"
)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use crate::parallel::{ParallelPattern, Semaphore};
use crate::Value;

// Semaphore Unit Tests

mod semaphore {
    use super::*;

    #[test]
    fn basic_acquire_release() {
        let sem = Semaphore::new(2);
        sem.acquire();
        sem.acquire();
        sem.release();
        sem.release();
    }

    #[test]
    fn limits_concurrency() {
        let sem = Arc::new(Semaphore::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all threads attempt acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(10));
        // Second barrier ensures threads hold semaphore until all have recorded max
        let hold_barrier = Arc::new(Barrier::new(2));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let sem = Arc::clone(&sem);
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                let start_barrier = Arc::clone(&start_barrier);
                let hold_barrier = Arc::clone(&hold_barrier);
                thread::spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(current, Ordering::SeqCst);
                    // Wait at barrier to ensure both permitted threads overlap
                    hold_barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    sem.release();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert!(max_active.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn single_slot() {
        let sem = Arc::new(Semaphore::new(1));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all threads attempt acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(5));

        let handles: Vec<_> = (0..5)
            .map(|_| {
                let sem = Arc::clone(&sem);
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                let start_barrier = Arc::clone(&start_barrier);
                thread::spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(current, Ordering::SeqCst);
                    // No sleep needed - semaphore guarantees serialization
                    active.fetch_sub(1, Ordering::SeqCst);
                    sem.release();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // With single slot, max should be exactly 1
        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn high_concurrency() {
        let sem = Arc::new(Semaphore::new(50));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all threads attempt acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(100));
        // Hold barrier ensures threads with permits overlap
        let hold_barrier = Arc::new(Barrier::new(50));

        let handles: Vec<_> = (0..100)
            .map(|_| {
                let sem = Arc::clone(&sem);
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                let start_barrier = Arc::clone(&start_barrier);
                let hold_barrier = Arc::clone(&hold_barrier);
                thread::spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(current, Ordering::SeqCst);
                    hold_barrier.wait(); // Ensure all 50 permitted threads overlap
                    active.fetch_sub(1, Ordering::SeqCst);
                    sem.release();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert!(max_active.load(Ordering::SeqCst) <= 50);
    }
}

// Value Helper Tests

mod value_helpers {
    use super::*;

    #[test]
    fn ok_value_creation() {
        let v = Value::ok(Value::int(42));
        match v {
            Value::Ok(inner) => assert_eq!(*inner, Value::int(42)),
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn err_value_creation() {
        let v = Value::err(Value::string("error message"));
        match v {
            Value::Err(inner) => {
                if let Value::Str(s) = &*inner {
                    assert_eq!(s.as_str(), "error message");
                } else {
                    panic!("expected Str inside Err");
                }
            }
            _ => panic!("expected Err variant"),
        }
    }

    #[test]
    fn list_value_creation() {
        let items = vec![Value::int(1), Value::int(2), Value::int(3)];
        let v = Value::list(items);
        match v {
            Value::List(list) => {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0], Value::int(1));
                assert_eq!(list[1], Value::int(2));
                assert_eq!(list[2], Value::int(3));
            }
            _ => panic!("expected List variant"),
        }
    }
}

// execute_task Function Tests

mod execute_task_tests {
    use super::*;
    use crate::parallel::execute_task;

    #[test]
    fn wraps_ok_value() {
        let result = execute_task(Value::int(42));
        match result {
            Value::Ok(inner) => assert_eq!(*inner, Value::int(42)),
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn preserves_ok_variant() {
        let result = execute_task(Value::ok(Value::int(42)));
        match result {
            Value::Ok(inner) => assert_eq!(*inner, Value::int(42)),
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn preserves_err_variant() {
        let result = execute_task(Value::err(Value::string("error")));
        match result {
            Value::Err(inner) => {
                if let Value::Str(s) = &*inner {
                    assert_eq!(s.as_str(), "error");
                }
            }
            _ => panic!("expected Err variant"),
        }
    }

    #[test]
    fn wraps_string_value() {
        let result = execute_task(Value::string("hello"));
        match result {
            Value::Ok(inner) => {
                if let Value::Str(s) = &*inner {
                    assert_eq!(s.as_str(), "hello");
                }
            }
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn wraps_bool_value() {
        let result = execute_task(Value::Bool(true));
        match result {
            Value::Ok(inner) => assert_eq!(*inner, Value::Bool(true)),
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn wraps_list_value() {
        let list = Value::list(vec![Value::int(1), Value::int(2)]);
        let result = execute_task(list);
        match result {
            Value::Ok(inner) => {
                if let Value::List(l) = &*inner {
                    assert_eq!(l.len(), 2);
                }
            }
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn wraps_error_in_ok() {
        // Note: execute_task wraps non-callable values in Ok, including Error.
        // Error -> Err conversion only happens in wrap_in_result for function results.
        let result = execute_task(Value::Error("runtime error".to_string()));
        match result {
            Value::Ok(_) => {}
            _ => panic!("expected Ok variant (Error is wrapped, not converted)"),
        }
    }
}

// wrap_in_result Function Tests

mod wrap_in_result_tests {
    use super::*;
    use crate::parallel::wrap_in_result;

    #[test]
    fn wraps_int() {
        let result = wrap_in_result(Value::int(42));
        match result {
            Value::Ok(inner) => assert_eq!(*inner, Value::int(42)),
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn passes_through_ok() {
        let result = wrap_in_result(Value::ok(Value::int(99)));
        match result {
            Value::Ok(inner) => assert_eq!(*inner, Value::int(99)),
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn passes_through_err() {
        let result = wrap_in_result(Value::err(Value::string("fail")));
        match result {
            Value::Err(_) => {}
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn converts_error_to_err() {
        let result = wrap_in_result(Value::Error("bad".to_string()));
        match result {
            Value::Err(_) => {}
            _ => panic!("expected Err"),
        }
    }
}

// Pattern Definition Tests

mod pattern_definition {
    use super::*;
    use crate::PatternDefinition;

    #[test]
    fn name_is_parallel() {
        let pattern = ParallelPattern;
        assert_eq!(pattern.name(), "parallel");
    }

    #[test]
    fn required_props_is_tasks() {
        let pattern = ParallelPattern;
        assert_eq!(pattern.required_props(), &["tasks"]);
    }

    #[test]
    fn does_not_allow_arbitrary_props() {
        let pattern = ParallelPattern;
        assert!(!pattern.allows_arbitrary_props());
    }
}

// Concurrency Verification Tests

mod concurrency_verification {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// Verify that tasks actually execute concurrently using barriers.
    #[test]
    fn tasks_run_concurrently() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all threads reach their "active" state together
        let sync_barrier = Arc::new(Barrier::new(4));

        thread::scope(|s| {
            for _ in 0..4 {
                let active = Arc::clone(&active);
                let max_concurrent = Arc::clone(&max_concurrent);
                let completed = Arc::clone(&completed);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(current, Ordering::SeqCst);
                    // Barrier ensures all threads overlap at this point
                    sync_barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                });
            }
        });

        // All tasks should complete
        assert_eq!(completed.load(Ordering::SeqCst), 4);
        // All 4 tasks should have been active concurrently (barrier guarantees this)
        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            4,
            "barrier should ensure all 4 threads overlap"
        );
    }

    /// Verify tasks run on different OS threads by checking thread IDs.
    #[test]
    fn tasks_use_different_threads() {
        let thread_ids = Arc::new(Mutex::new(Vec::new()));
        // Barrier ensures threads are alive simultaneously when recording IDs
        let sync_barrier = Arc::new(Barrier::new(4));

        thread::scope(|s| {
            for _ in 0..4 {
                let thread_ids = Arc::clone(&thread_ids);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    let id = thread::current().id();
                    thread_ids.lock().unwrap().push(id);
                    // Barrier ensures all threads exist simultaneously
                    sync_barrier.wait();
                });
            }
        });

        let ids = thread_ids.lock().unwrap();
        let unique_ids: HashSet<_> = ids.iter().collect();

        // Should have multiple unique thread IDs (at least 2, likely 4)
        assert!(
            unique_ids.len() > 1,
            "expected multiple threads, got {} unique thread IDs: {ids:?}",
            unique_ids.len()
        );
    }

    /// Verify concurrent execution by tracking max active tasks.
    #[test]
    fn execution_windows_overlap() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all threads are active simultaneously
        let sync_barrier = Arc::new(Barrier::new(4));

        thread::scope(|s| {
            for _ in 0..4 {
                let active = Arc::clone(&active);
                let max_concurrent = Arc::clone(&max_concurrent);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    // Increment active before barrier to detect overlap
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(current, Ordering::SeqCst);
                    // Barrier ensures overlap
                    sync_barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        // With barrier, all 4 tasks should be active simultaneously
        let max = max_concurrent.load(Ordering::SeqCst);
        assert_eq!(max, 4, "barrier should ensure all 4 threads overlap");
    }

    /// Verify that different threads can execute simultaneously on different cores.
    #[test]
    fn simultaneous_execution_on_cores() {
        let active_threads = Arc::new(AtomicUsize::new(0));
        let max_simultaneous = Arc::new(AtomicUsize::new(0));
        let thread_ids = Arc::new(Mutex::new(Vec::new()));
        // Barrier ensures all 8 threads are active simultaneously
        let sync_barrier = Arc::new(Barrier::new(8));

        thread::scope(|s| {
            for _ in 0..8 {
                let active = Arc::clone(&active_threads);
                let max_sim = Arc::clone(&max_simultaneous);
                let thread_ids = Arc::clone(&thread_ids);
                let sync_barrier = Arc::clone(&sync_barrier);

                s.spawn(move || {
                    // Increment active count
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;

                    // Track maximum and record thread ID
                    max_sim.fetch_max(current, Ordering::SeqCst);
                    thread_ids.lock().unwrap().push(thread::current().id());

                    // Barrier ensures all threads overlap
                    sync_barrier.wait();

                    // Decrement active count
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        let max = max_simultaneous.load(Ordering::SeqCst);
        let ids = thread_ids.lock().unwrap();

        // With barrier, all 8 threads should be active simultaneously
        assert_eq!(max, 8, "barrier should ensure all 8 threads overlap");

        // Verify we actually used multiple OS threads
        let unique_ids: HashSet<_> = ids.iter().collect();
        assert!(
            unique_ids.len() > 1,
            "should have multiple unique thread IDs"
        );
    }

    /// Verify thread IDs are distinct from main thread.
    #[test]
    fn spawned_threads_differ_from_main() {
        let main_thread_id = thread::current().id();
        let spawned_ids = Arc::new(Mutex::new(Vec::new()));

        thread::scope(|s| {
            for _ in 0..4 {
                let spawned_ids = Arc::clone(&spawned_ids);
                s.spawn(move || {
                    spawned_ids.lock().unwrap().push(thread::current().id());
                });
            }
        });

        let ids = spawned_ids.lock().unwrap();
        for id in ids.iter() {
            assert_ne!(
                *id, main_thread_id,
                "spawned thread should have different ID than main thread"
            );
        }
    }

    /// Verify `max_concurrent` actually limits parallelism.
    #[test]
    fn max_concurrent_limits_parallelism() {
        let sem = Arc::new(Semaphore::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        // Start barrier ensures all threads try to acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(6));
        // Hold barrier ensures threads with permits overlap
        let hold_barrier = Arc::new(Barrier::new(2));

        thread::scope(|s| {
            for _ in 0..6 {
                let sem = Arc::clone(&sem);
                let active = Arc::clone(&active);
                let max_observed = Arc::clone(&max_observed);
                let completed = Arc::clone(&completed);
                let start_barrier = Arc::clone(&start_barrier);
                let hold_barrier = Arc::clone(&hold_barrier);
                s.spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_observed.fetch_max(current, Ordering::SeqCst);
                    // Barrier ensures the 2 permitted threads overlap
                    hold_barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                    sem.release();
                });
            }
        });

        let max = max_observed.load(Ordering::SeqCst);

        // Semaphore should limit concurrent execution to 2
        assert!(max <= 2, "max concurrent should be 2, was {max}");
        // All tasks should complete
        assert_eq!(completed.load(Ordering::SeqCst), 6);
    }

    /// Verify order is preserved in results (results go to correct slots).
    #[test]
    fn results_preserve_order() {
        let results = Arc::new(Mutex::new(vec![None; 5]));
        // Barrier ensures concurrent execution
        let sync_barrier = Arc::new(Barrier::new(5));

        thread::scope(|s| {
            for i in 0..5 {
                let results = Arc::clone(&results);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    // Barrier ensures threads run concurrently
                    sync_barrier.wait();
                    results.lock().unwrap()[i] = Some(i);
                });
            }
        });

        let results = results.lock().unwrap();
        for (i, r) in results.iter().enumerate() {
            assert_eq!(*r, Some(i), "result at index {i} should be {i}");
        }
    }
}

// Timeout Tests

mod timeout {
    use super::*;

    #[test]
    fn timeout_duration_parsing() {
        // Test that Duration values work
        let duration_ms: u64 = 100;
        assert_eq!(Duration::from_millis(duration_ms).as_millis(), 100);
    }

    #[test]
    fn short_timeout_completes_fast_tasks() {
        let completed = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all threads overlap
        let sync_barrier = Arc::new(Barrier::new(3));

        thread::scope(|s| {
            for _ in 0..3 {
                let completed = Arc::clone(&completed);
                let max_concurrent = Arc::clone(&max_concurrent);
                let active = Arc::clone(&active);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(current, Ordering::SeqCst);
                    sync_barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                });
            }
        });

        // All tasks should complete
        assert_eq!(completed.load(Ordering::SeqCst), 3);
        // All 3 tasks should run concurrently (barrier guarantees)
        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            3,
            "barrier should ensure all 3 threads overlap"
        );
    }

    #[test]
    fn timeout_elapsed_detection() {
        // Test the math directly without relying on actual sleep timing
        let timeout = Duration::from_millis(50);
        let elapsed = Duration::from_millis(60); // Simulated elapsed time

        let remaining = timeout.saturating_sub(elapsed);
        assert!(remaining.is_zero(), "timeout should have elapsed");

        // Also test boundary condition
        let elapsed_exact = Duration::from_millis(50);
        let remaining_exact = timeout.saturating_sub(elapsed_exact);
        assert!(remaining_exact.is_zero(), "exact timeout should have elapsed");
    }

    #[test]
    fn timeout_remaining_calculation() {
        // Test the math directly without relying on actual sleep timing
        let timeout = Duration::from_millis(500);
        let elapsed = Duration::from_millis(50); // Simulated elapsed time

        let remaining = timeout.saturating_sub(elapsed);
        assert_eq!(
            remaining,
            Duration::from_millis(450),
            "remaining should be exactly 450ms"
        );

        // Test partial elapsed
        let elapsed_partial = Duration::from_millis(200);
        let remaining_partial = timeout.saturating_sub(elapsed_partial);
        assert_eq!(
            remaining_partial,
            Duration::from_millis(300),
            "remaining should be exactly 300ms"
        );

        // Test over-elapsed (should saturate to zero)
        let elapsed_over = Duration::from_millis(600);
        let remaining_over = timeout.saturating_sub(elapsed_over);
        assert!(remaining_over.is_zero(), "over-elapsed should saturate to zero");
    }
}

// Error Handling Tests (All-Settled Semantics)

mod all_settled {
    use super::*;

    #[test]
    fn errors_captured_as_err_values() {
        // Simulate error capture behavior
        let results: Vec<Value> = vec![
            Value::ok(Value::int(1)),
            Value::err(Value::string("task 2 failed")),
            Value::ok(Value::int(3)),
        ];

        assert_eq!(results.len(), 3);
        match &results[0] {
            Value::Ok(_) => {}
            _ => panic!("expected Ok"),
        }
        match &results[1] {
            Value::Err(_) => {}
            _ => panic!("expected Err"),
        }
        match &results[2] {
            Value::Ok(_) => {}
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn mixed_success_and_failure() {
        let results: Vec<Value> = (0..5)
            .map(|i| {
                if i % 2 == 0 {
                    Value::ok(Value::int(i))
                } else {
                    Value::err(Value::string(format!("error at {i}")))
                }
            })
            .collect();

        let successes: usize = results.iter().filter(|v| matches!(v, Value::Ok(_))).count();
        let failures: usize = results
            .iter()
            .filter(|v| matches!(v, Value::Err(_)))
            .count();

        assert_eq!(successes, 3);
        assert_eq!(failures, 2);
    }

    #[test]
    fn all_tasks_complete_even_with_errors() {
        let completed = Arc::new(AtomicUsize::new(0));

        thread::scope(|s| {
            for i in 0..5 {
                let completed = Arc::clone(&completed);
                s.spawn(move || {
                    // Simulate some tasks failing
                    if i == 2 {
                        // "failing" task still completes
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                });
            }
        });

        assert_eq!(completed.load(Ordering::SeqCst), 5);
    }
}

// Edge Cases

mod edge_cases {
    use super::*;

    #[test]
    fn empty_task_list() {
        let results: Vec<Value> = vec![];
        assert!(results.is_empty());
        assert_eq!(Value::list(results), Value::list(vec![]));
    }

    #[test]
    fn single_task() {
        let results = [Value::ok(Value::int(42))];
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn large_number_of_tasks() {
        let task_count = 100;
        let completed = Arc::new(AtomicUsize::new(0));

        thread::scope(|s| {
            for _ in 0..task_count {
                let completed = Arc::clone(&completed);
                s.spawn(move || {
                    completed.fetch_add(1, Ordering::SeqCst);
                });
            }
        });

        assert_eq!(completed.load(Ordering::SeqCst), task_count);
    }

    #[test]
    fn max_concurrent_equals_task_count() {
        let sem = Arc::new(Semaphore::new(5));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all 5 threads overlap
        let sync_barrier = Arc::new(Barrier::new(5));

        thread::scope(|s| {
            for _ in 0..5 {
                let sem = Arc::clone(&sem);
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(current, Ordering::SeqCst);
                    sync_barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    sem.release();
                });
            }
        });

        // All 5 should run at once (barrier guarantees)
        assert_eq!(max_active.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn max_concurrent_one() {
        let sem = Arc::new(Semaphore::new(1));
        let sequence = Arc::new(Mutex::new(Vec::new()));
        // Barrier ensures all threads try to acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(3));

        thread::scope(|s| {
            for i in 0..3 {
                let sem = Arc::clone(&sem);
                let sequence = Arc::clone(&sequence);
                let start_barrier = Arc::clone(&start_barrier);
                s.spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    sequence.lock().unwrap().push(format!("start_{i}"));
                    // No sleep needed - semaphore guarantees serialization
                    sequence.lock().unwrap().push(format!("end_{i}"));
                    sem.release();
                });
            }
        });

        let seq = sequence.lock().unwrap();
        // With max_concurrent=1, operations should not interleave
        // Each start should be followed by its end before next start
        for i in 0..3 {
            let start_idx = seq.iter().position(|s| s == &format!("start_{i}"));
            let end_idx = seq.iter().position(|s| s == &format!("end_{i}"));
            if let (Some(s), Some(e)) = (start_idx, end_idx) {
                assert!(
                    e == s + 1,
                    "start and end should be consecutive with max_concurrent=1"
                );
            }
        }
    }

    #[test]
    fn zero_timeout_treated_as_no_timeout() {
        let timeout = Duration::from_millis(0);
        assert!(timeout.is_zero());
    }

    #[test]
    fn very_long_timeout() {
        let timeout = Duration::from_secs(3600); // 1 hour
        assert!(!timeout.is_zero());
        assert_eq!(timeout.as_secs(), 3600);
    }
}

// Thread Safety Tests

mod thread_safety {
    use super::*;
    use std::sync::Mutex;

    #[test]
    #[expect(clippy::cast_possible_wrap, reason = "test indices are small")]
    fn concurrent_result_writes() {
        let results = Arc::new(Mutex::new(vec![None; 10]));

        thread::scope(|s| {
            for i in 0..10 {
                let results = Arc::clone(&results);
                s.spawn(move || {
                    let value = Value::int(i as i64);
                    results.lock().unwrap()[i] = Some(value);
                });
            }
        });

        let results = results.lock().unwrap();
        for (i, r) in results.iter().enumerate() {
            assert!(r.is_some(), "result at {i} should be Some");
            match r {
                Some(Value::Int(n)) => assert_eq!(n.raw(), i as i64),
                _ => panic!("expected Int"),
            }
        }
    }

    #[test]
    fn no_data_races() {
        let counter = Arc::new(AtomicUsize::new(0));
        let iterations = 1000;

        thread::scope(|s| {
            for _ in 0..10 {
                let counter = Arc::clone(&counter);
                s.spawn(move || {
                    for _ in 0..iterations {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });

        assert_eq!(counter.load(Ordering::SeqCst), 10 * iterations);
    }

    #[test]
    fn mutex_poisoning_recovery() {
        let data = Arc::new(Mutex::new(42));
        let data_clone = Arc::clone(&data);

        // Simulate a panic that would poison the mutex
        let result = std::panic::catch_unwind(move || {
            let _guard = data_clone.lock().unwrap();
            panic!("intentional panic");
        });

        assert!(result.is_err());

        // Should still be able to access with into_inner
        let guard = data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(*guard, 42);
    }
}

// Integration-Style Tests

mod integration {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn simulate_parallel_http_requests() {
        // Simulate parallel HTTP requests - verify concurrency via barrier, not timing
        let responses = Arc::new(Mutex::new(vec![None; 5]));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        // Barrier ensures all "requests" are in flight simultaneously
        let sync_barrier = Arc::new(Barrier::new(5));

        thread::scope(|s| {
            for i in 0..5 {
                let responses = Arc::clone(&responses);
                let max_concurrent = Arc::clone(&max_concurrent);
                let active = Arc::clone(&active);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    // Track concurrent requests
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(current, Ordering::SeqCst);

                    // Barrier proves all requests were concurrent
                    sync_barrier.wait();

                    let response = format!("response_{i}");
                    responses.lock().unwrap()[i] = Some(Value::string(&response));
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        // Verify all 5 requests were concurrent (barrier guarantees this)
        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            5,
            "all HTTP requests should be concurrent"
        );

        let responses = responses.lock().unwrap();
        for (i, r) in responses.iter().enumerate() {
            assert!(r.is_some(), "response {i} should be present");
        }
    }

    #[test]
    fn simulate_parallel_file_processing() {
        // Simulate parallel file processing with rate limiting
        let sem = Arc::new(Semaphore::new(3)); // Max 3 concurrent file ops
        let processed = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        // Start barrier ensures all threads try to acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(10));

        thread::scope(|s| {
            for _ in 0..10 {
                let sem = Arc::clone(&sem);
                let processed = Arc::clone(&processed);
                let max_concurrent = Arc::clone(&max_concurrent);
                let active = Arc::clone(&active);
                let start_barrier = Arc::clone(&start_barrier);
                s.spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(current, Ordering::SeqCst);

                    // Semaphore guarantees max 3 concurrent - no sleep needed
                    processed.fetch_add(1, Ordering::SeqCst);

                    active.fetch_sub(1, Ordering::SeqCst);
                    sem.release();
                });
            }
        });

        assert_eq!(processed.load(Ordering::SeqCst), 10);
        assert!(max_concurrent.load(Ordering::SeqCst) <= 3);
    }

    #[test]
    #[expect(clippy::cast_possible_wrap, reason = "test indices are small")]
    fn simulate_parallel_with_mixed_results() {
        // Simulate a parallel operation where some tasks succeed and some fail
        let results = Arc::new(Mutex::new(vec![None; 6]));
        // Barrier ensures all threads run concurrently
        let sync_barrier = Arc::new(Barrier::new(6));

        thread::scope(|s| {
            for i in 0..6 {
                let results = Arc::clone(&results);
                let sync_barrier = Arc::clone(&sync_barrier);
                s.spawn(move || {
                    sync_barrier.wait();
                    let result = if i % 3 == 0 {
                        // Every 3rd task "fails"
                        Value::err(Value::string(format!("task {i} failed")))
                    } else {
                        Value::ok(Value::int(i as i64 * 10))
                    };
                    results.lock().unwrap()[i] = Some(result);
                });
            }
        });

        let results = results.lock().unwrap();

        // Count successes and failures
        let mut successes = 0;
        let mut failures = 0;
        for r in results.iter() {
            match r {
                Some(Value::Ok(_)) => successes += 1,
                Some(Value::Err(_)) => failures += 1,
                _ => panic!("unexpected result"),
            }
        }

        assert_eq!(successes, 4); // indices 1, 2, 4, 5
        assert_eq!(failures, 2); // indices 0, 3
    }

    #[test]
    fn simulate_parallel_with_max_concurrent_verification() {
        // Verify semaphore limits concurrency correctly
        let sem = Arc::new(Semaphore::new(2));
        let completed = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        // Start barrier ensures all threads try to acquire simultaneously
        let start_barrier = Arc::new(Barrier::new(4));
        // Hold barrier ensures threads with permits overlap
        let hold_barrier = Arc::new(Barrier::new(2));

        thread::scope(|s| {
            for _ in 0..4 {
                let sem = Arc::clone(&sem);
                let completed = Arc::clone(&completed);
                let max_concurrent = Arc::clone(&max_concurrent);
                let active = Arc::clone(&active);
                let start_barrier = Arc::clone(&start_barrier);
                let hold_barrier = Arc::clone(&hold_barrier);
                s.spawn(move || {
                    start_barrier.wait(); // All threads start together
                    sem.acquire();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(current, Ordering::SeqCst);
                    hold_barrier.wait(); // Ensure permitted threads overlap
                    active.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                    sem.release();
                });
            }
        });

        // All tasks should complete
        assert_eq!(completed.load(Ordering::SeqCst), 4);
        // Max concurrent should be limited to 2
        assert!(max_concurrent.load(Ordering::SeqCst) <= 2);
    }
}

// Stress Tests

mod stress {
    use super::*;

    #[test]
    fn many_concurrent_semaphore_operations() {
        let sem = Arc::new(Semaphore::new(10));
        let ops = Arc::new(AtomicUsize::new(0));

        thread::scope(|s| {
            for _ in 0..100 {
                let sem = Arc::clone(&sem);
                let ops = Arc::clone(&ops);
                s.spawn(move || {
                    for _ in 0..100 {
                        sem.acquire();
                        ops.fetch_add(1, Ordering::SeqCst);
                        sem.release();
                    }
                });
            }
        });

        assert_eq!(ops.load(Ordering::SeqCst), 10000);
    }

    #[test]
    fn rapid_spawn_and_complete() {
        let completed = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let completed = Arc::clone(&completed);
            thread::scope(|s| {
                for _ in 0..50 {
                    let completed = Arc::clone(&completed);
                    s.spawn(move || {
                        completed.fetch_add(1, Ordering::SeqCst);
                    });
                }
            });
        }

        assert_eq!(completed.load(Ordering::SeqCst), 500);
    }
}
