# Rosetta Code Tasks for Sigil

This folder contains implementations of [Rosetta Code](https://rosettacode.org) programming tasks in Sigil.

## Folder Structure

Each task has its own folder with source and test files:

```
rosetta/
├── README.md
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

See `_tasks` for the complete task list. We're starting with the 20 essential tasks that cover core language features.
