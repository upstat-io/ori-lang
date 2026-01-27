# Rosetta Code Tasks for Ori

This folder contains implementations of [Rosetta Code](https://rosettacode.org) programming tasks in Ori.

## Folder Structure

Each task has its own folder with source and test files:

```
rosetta/
├── README.md
├── hello_world/
│   ├── hello_world.ori
│   └── _test/
│       └── hello_world.test.ori
├── fizzbuzz/
│   ├── fizzbuzz.ori
│   └── _test/
│       └── fizzbuzz.test.ori
└── ...
```

## Running Tasks

Run a task:
```bash
cargo run -- run rosetta/hello_world/hello_world.ori
```

Run tests for a task:
```bash
cargo run -- test rosetta/hello_world/_test/hello_world.test.ori
```

## Implementation Progress

See `_tasks` for the complete task list. We're starting with the 20 essential tasks that cover core language features.
