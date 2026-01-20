# Rosetta Code Tasks for Sigil

This folder contains implementations of [Rosetta Code](https://rosettacode.org) programming tasks in Sigil.

## Folder Structure

Each task has its own folder with source and test files:

```
rosetta/
├── README.md
├── ALL_TASKS.md
├── hello_world/
│   ├── hello_world.si
│   └── _test/
│       └── hello_world.test.si
├── fizzbuzz/
│   ├── fizzbuzz.si
│   └── _test/
│       └── fizzbuzz.test.si
└── ...
```

## Running Tasks

Run a task:
```bash
cargo run -- run rosetta/hello_world/hello_world.si
```

Run tests for a task:
```bash
cargo run -- test rosetta/hello_world/_test/hello_world.test.si
```

## Implementation Progress

See `ALL_TASKS.md` for the complete task list. We're starting with the 20 essential tasks that cover core language features.

### Basics
- [x] Hello world/Text
- [x] A+B
- [x] FizzBuzz
- [x] Factorial
- [x] Fibonacci sequence

### Data Structures
- [x] Arrays
- [ ] Associative array/Creation
- [x] Stack
- [x] Queue/Definition

### Strings
- [x] String concatenation
- [x] String length
- [x] Reverse a string

### Control Flow
- [ ] Loops/For
- [ ] Loops/While
- [ ] Conditional structures

### Functions
- [ ] Function definition
- [x] Higher-order functions
- [ ] Closures/Value capture

### I/O & Error Handling
- [ ] Read a file line by line
- [ ] Exceptions
