# Concurrency

Guidelines for concurrent programming in Rust based on official documentation.

## Quick Reference

- [ ] Use `std::thread` for spawning threads
- [ ] Use `Arc<T>` for shared ownership across threads
- [ ] Use `Mutex<T>` for shared mutable state
- [ ] Use channels (`std::sync::mpsc`) for message passing
- [ ] Understand `Send` and `Sync` traits
- [ ] Prefer message passing over shared state

## Creating Threads

### Basic Thread Spawning

```rust
use std::thread;
use std::time::Duration;

fn main() {
    let handle = thread::spawn(|| {
        for i in 1..10 {
            println!("spawned thread: {i}");
            thread::sleep(Duration::from_millis(1));
        }
    });

    for i in 1..5 {
        println!("main thread: {i}");
        thread::sleep(Duration::from_millis(1));
    }

    handle.join().unwrap();  // Wait for thread to finish
}
```

### Moving Data into Threads

Use `move` to transfer ownership to the spawned thread:

```rust
use std::thread;

fn main() {
    let v = vec![1, 2, 3];

    let handle = thread::spawn(move || {
        println!("vector: {:?}", v);
    });

    handle.join().unwrap();
}
```

## Thread Safety Traits

### Send and Sync

| Trait | Meaning |
|-------|---------|
| `Send` | Type can be transferred to another thread |
| `Sync` | Type can be referenced from multiple threads (`&T` is `Send`) |

Most types are `Send` and `Sync` automatically. Notable exceptions:
- `Rc<T>` is neither `Send` nor `Sync`
- `RefCell<T>` is not `Sync`
- Raw pointers are neither `Send` nor `Sync`

```rust
use std::thread;

// T: Send means we can move T to another thread
fn spawn_with<T: Send + 'static>(value: T, f: fn(T)) {
    thread::spawn(move || f(value));
}
```

## Shared State with Mutex

### Basic Mutex Usage

```rust
use std::sync::Mutex;

fn main() {
    let m = Mutex::new(5);

    {
        let mut num = m.lock().unwrap();
        *num = 6;
    }  // Lock is released when MutexGuard goes out of scope

    println!("m = {:?}", m);
}
```

### Sharing Mutex Between Threads with Arc

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            let mut num = counter.lock().unwrap();
            *num += 1;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Result: {}", *counter.lock().unwrap());
}
```

### RwLock for Read-Heavy Workloads

Use `RwLock` when reads are more common than writes:

```rust
use std::sync::RwLock;

let lock = RwLock::new(5);

// Multiple readers allowed simultaneously
{
    let r1 = lock.read().unwrap();
    let r2 = lock.read().unwrap();
    println!("readers: {} {}", *r1, *r2);
}

// Only one writer, blocks all readers
{
    let mut w = lock.write().unwrap();
    *w += 1;
}
```

## Message Passing with Channels

### Basic Channel Usage

```rust
use std::sync::mpsc;
use std::thread;

fn main() {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let val = String::from("hello");
        tx.send(val).unwrap();
        // val is moved, can't use it here
    });

    let received = rx.recv().unwrap();
    println!("Got: {received}");
}
```

### Sending Multiple Values

```rust
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn main() {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let vals = vec!["hi", "from", "thread"];
        for val in vals {
            tx.send(val).unwrap();
            thread::sleep(Duration::from_millis(200));
        }
    });

    for received in rx {
        println!("Got: {received}");
    }
}
```

### Multiple Producers

Clone the transmitter for multiple senders:

```rust
use std::sync::mpsc;
use std::thread;

fn main() {
    let (tx, rx) = mpsc::channel();

    let tx1 = tx.clone();
    thread::spawn(move || {
        tx1.send("from thread 1").unwrap();
    });

    thread::spawn(move || {
        tx.send("from thread 2").unwrap();
    });

    for received in rx {
        println!("Got: {received}");
    }
}
```

## Atomic Types

For simple counters and flags, use atomic types:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

fn main() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Result: {}", counter.load(Ordering::SeqCst));
}
```

## Common Patterns

### Arc + Mutex Pattern

For shared mutable state across threads:

```rust
use std::sync::{Arc, Mutex};

// Shared state type
type SharedData = Arc<Mutex<Vec<i32>>>;

fn process(data: SharedData) {
    let mut locked = data.lock().unwrap();
    locked.push(42);
}
```

### Scoped Threads

Use `thread::scope` for threads that borrow local data (Rust 1.63+):

```rust
use std::thread;

fn main() {
    let mut data = vec![1, 2, 3];

    thread::scope(|s| {
        s.spawn(|| {
            println!("data: {:?}", data);  // Can borrow data
        });
    });

    data.push(4);  // Can use data after scope ends
}
```

## Avoiding Deadlocks

### Lock Ordering

Always acquire locks in the same order:

```rust
// Bad: potential deadlock if another thread locks in opposite order
fn process_both(a: &Mutex<Data>, b: &Mutex<Data>) {
    let _lock_a = a.lock().unwrap();
    let _lock_b = b.lock().unwrap();  // Deadlock risk!
}

// Good: consistent ordering (e.g., by memory address)
fn process_both_safe(a: &Mutex<Data>, b: &Mutex<Data>) {
    let (first, second) = if std::ptr::eq(a, b) {
        return;  // Same mutex
    } else if (a as *const _) < (b as *const _) {
        (a, b)
    } else {
        (b, a)
    };

    let _lock1 = first.lock().unwrap();
    let _lock2 = second.lock().unwrap();
}
```

### Minimize Lock Scope

```rust
// Bad: lock held too long
let mut data = mutex.lock().unwrap();
expensive_computation();  // Lock held during computation
*data = result;

// Good: minimize lock scope
let result = expensive_computation();
{
    let mut data = mutex.lock().unwrap();
    *data = result;
}
```

## When to Use What

| Scenario | Solution |
|----------|----------|
| Simple background task | `std::thread::spawn` |
| Share read-only data | `Arc<T>` |
| Share mutable data | `Arc<Mutex<T>>` |
| Read-heavy shared data | `Arc<RwLock<T>>` |
| Thread communication | `std::sync::mpsc` channels |
| Simple counter/flag | `AtomicUsize`, `AtomicBool` |
| Borrow local data in threads | `std::thread::scope` |

## Guidelines

### Do

- Prefer message passing over shared state
- Use `Arc<Mutex<T>>` when sharing mutable state is necessary
- Release locks as soon as possible
- Use atomic types for simple counters and flags
- Use `thread::scope` when borrowing local data

### Don't

- Don't use `Rc` in multi-threaded code (use `Arc`)
- Don't hold locks across long operations
- Don't ignore lock poisoning (`unwrap` vs proper handling)
- Don't create complex lock hierarchies

## Resources

- [Fearless Concurrency - The Rust Book](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Using Threads - The Rust Book](https://doc.rust-lang.org/book/ch16-01-threads.html)
- [Shared-State Concurrency - The Rust Book](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Message Passing - The Rust Book](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [std::sync Module](https://doc.rust-lang.org/std/sync/)
- [std::thread Module](https://doc.rust-lang.org/std/thread/)
