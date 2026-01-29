# Phase 4: Patterns

**Goal**: Implement formatting for compiler-recognized pattern constructs: run, try, match, recurse, parallel, spawn, nursery, and other control flow patterns.

> **DESIGN**: `docs/tooling/formatter/design/02-constructs/patterns.md`

## Phase Status: ⏳ Not Started

## 4.1 Run Pattern

Always stacked, never inline.

- [ ] **Implement**: Simple run block
  - [ ] **Golden Tests**: `tests/fmt/patterns/run/simple.ori`
  ```ori
  run(
      let x = compute(),
      let y = process(x),
      result(x, y)
  )
  ```
- [ ] **Implement**: Run with pre_check
  - [ ] **Golden Tests**: `tests/fmt/patterns/run/pre_check.ori`
  ```ori
  run(
      pre_check: x > 0,
      body
  )
  ```
- [ ] **Implement**: Run with post_check
  - [ ] **Golden Tests**: `tests/fmt/patterns/run/post_check.ori`
  ```ori
  run(
      body,
      post_check: r -> r >= 0
  )
  ```
- [ ] **Implement**: Run with both checks
  - [ ] **Golden Tests**: `tests/fmt/patterns/run/both_checks.ori`
- [ ] **Implement**: Run with custom error message
  - [ ] **Golden Tests**: `tests/fmt/patterns/run/custom_msg.ori`
  ```ori
  run(
      pre_check: x > 0 | "x must be positive",
      body
  )
  ```

## 4.2 Try Pattern

Always stacked, never inline.

- [ ] **Implement**: Simple try block
  - [ ] **Golden Tests**: `tests/fmt/patterns/try/simple.ori`
  ```ori
  try(
      let x = fallible()?,
      let y = another()?,
      Ok(x + y)
  )
  ```
- [ ] **Implement**: Try with error propagation
  - [ ] **Golden Tests**: `tests/fmt/patterns/try/propagation.ori`
- [ ] **Implement**: Try with pre_check/post_check
  - [ ] **Golden Tests**: `tests/fmt/patterns/try/checks.ori`

## 4.3 Match Pattern

Arms always stacked, one per line.

- [ ] **Implement**: Simple match
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/simple.ori`
  ```ori
  match(value,
      Some(x) -> x,
      None -> 0,
  )
  ```
- [ ] **Implement**: Match with complex patterns
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/complex.ori`
- [ ] **Implement**: Match with guards
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/guards.ori`
  ```ori
  match(n,
      x if x < 0 -> "negative",
      0 -> "zero",
      x -> "positive",
  )
  ```
- [ ] **Implement**: Match with or-patterns
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/or.ori`
- [ ] **Implement**: Match with struct patterns
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/struct.ori`
- [ ] **Implement**: Match with list patterns
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/list.ori`
- [ ] **Implement**: Match arm with complex body
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/complex_body.ori`
- [ ] **Implement**: Wildcard pattern
  - [ ] **Golden Tests**: `tests/fmt/patterns/match/wildcard.ori`

## 4.4 Recurse Pattern

Always stacked with named parameters.

- [ ] **Implement**: Simple recurse
  - [ ] **Golden Tests**: `tests/fmt/patterns/recurse/simple.ori`
  ```ori
  recurse(
      condition: n <= 1,
      base: 1,
      step: n * self(n: n - 1),
  )
  ```
- [ ] **Implement**: Recurse with memo
  - [ ] **Golden Tests**: `tests/fmt/patterns/recurse/memo.ori`
  ```ori
  recurse(
      condition: n <= 1,
      base: 1,
      step: self(n: n - 1) + self(n: n - 2),
      memo: true,
  )
  ```
- [ ] **Implement**: Recurse with parallel
  - [ ] **Golden Tests**: `tests/fmt/patterns/recurse/parallel.ori`

## 4.5 Parallel Pattern

Always stacked, tasks follow list rules.

- [ ] **Implement**: Simple parallel
  - [ ] **Golden Tests**: `tests/fmt/patterns/parallel/simple.ori`
  ```ori
  parallel(
      tasks: [
          fetch(url: url1),
          fetch(url: url2),
          fetch(url: url3),
      ],
  )
  ```
- [ ] **Implement**: Parallel with max_concurrent
  - [ ] **Golden Tests**: `tests/fmt/patterns/parallel/max_concurrent.ori`
- [ ] **Implement**: Parallel with timeout
  - [ ] **Golden Tests**: `tests/fmt/patterns/parallel/timeout.ori`

## 4.6 Spawn Pattern

Always stacked, fire-and-forget.

- [ ] **Implement**: Simple spawn
  - [ ] **Golden Tests**: `tests/fmt/patterns/spawn/simple.ori`
  ```ori
  spawn(
      tasks: [
          log_event(event: e1),
          log_event(event: e2),
      ],
  )
  ```
- [ ] **Implement**: Spawn with max_concurrent
  - [ ] **Golden Tests**: `tests/fmt/patterns/spawn/max_concurrent.ori`

## 4.7 Nursery Pattern

Always stacked, structured concurrency.

- [ ] **Implement**: Simple nursery
  - [ ] **Golden Tests**: `tests/fmt/patterns/nursery/simple.ori`
  ```ori
  nursery(
      body: n -> run(
          n.spawn(task1),
          n.spawn(task2),
          collect_results()
      ),
  )
  ```
- [ ] **Implement**: Nursery with on_error
  - [ ] **Golden Tests**: `tests/fmt/patterns/nursery/on_error.ori`
- [ ] **Implement**: Nursery with timeout
  - [ ] **Golden Tests**: `tests/fmt/patterns/nursery/timeout.ori`

## 4.8 Timeout Pattern

Stacked when multiple parameters.

- [ ] **Implement**: Simple timeout
  - [ ] **Golden Tests**: `tests/fmt/patterns/timeout/simple.ori`
  ```ori
  timeout(
      op: slow_operation(),
      after: 5s,
  )
  ```

## 4.9 Cache Pattern

Stacked when multiple parameters.

- [ ] **Implement**: Simple cache
  - [ ] **Golden Tests**: `tests/fmt/patterns/cache/simple.ori`
  ```ori
  cache(
      key: user_id,
      op: fetch_user(id: user_id),
  )
  ```
- [ ] **Implement**: Cache with TTL
  - [ ] **Golden Tests**: `tests/fmt/patterns/cache/ttl.ori`

## 4.10 With Pattern

- [ ] **Implement**: Simple with (inline)
  - [ ] **Golden Tests**: `tests/fmt/patterns/with/simple.ori`
  ```ori
  with Http = MockHttp {} in fetch("/data")
  ```
- [ ] **Implement**: With broken at `in`
  - [ ] **Golden Tests**: `tests/fmt/patterns/with/broken.ori`
  ```ori
  with Http = RealHttp { base_url: "https://api.example.com" }
  in fetch_all_data()
  ```
- [ ] **Implement**: With acquire/use/release
  - [ ] **Golden Tests**: `tests/fmt/patterns/with/resource.ori`

## 4.11 For Pattern

- [ ] **Implement**: For-do (imperative)
  - [ ] **Golden Tests**: `tests/fmt/patterns/for/do.ori`
  ```ori
  for item in items do
      print(msg: item)
  ```
- [ ] **Implement**: For-yield (collection)
  - [ ] **Golden Tests**: `tests/fmt/patterns/for/yield.ori`
  ```ori
  for x in items yield x * 2
  ```
- [ ] **Implement**: For with guard
  - [ ] **Golden Tests**: `tests/fmt/patterns/for/guard.ori`
- [ ] **Implement**: For with pattern matching
  - [ ] **Golden Tests**: `tests/fmt/patterns/for/pattern.ori`
- [ ] **Implement**: Labeled for
  - [ ] **Golden Tests**: `tests/fmt/patterns/for/labeled.ori`

## 4.12 Loop Pattern

- [ ] **Implement**: Simple loop
  - [ ] **Golden Tests**: `tests/fmt/patterns/loop/simple.ori`
- [ ] **Implement**: Loop with break
  - [ ] **Golden Tests**: `tests/fmt/patterns/loop/break.ori`
- [ ] **Implement**: Loop with continue
  - [ ] **Golden Tests**: `tests/fmt/patterns/loop/continue.ori`
- [ ] **Implement**: Labeled loop
  - [ ] **Golden Tests**: `tests/fmt/patterns/loop/labeled.ori`

## 4.13 Catch Pattern

- [ ] **Implement**: Catch expression
  - [ ] **Golden Tests**: `tests/fmt/patterns/catch/simple.ori`
  ```ori
  catch(expr: risky_operation())
  ```

## 4.14 Channel Constructors

- [ ] **Implement**: Simple channel
  - [ ] **Golden Tests**: `tests/fmt/patterns/channel/simple.ori`
  ```ori
  let (producer, consumer) = channel<int>(buffer: 10)
  ```
- [ ] **Implement**: Channel variants (channel_in, channel_out, channel_all)
  - [ ] **Golden Tests**: `tests/fmt/patterns/channel/variants.ori`

## Completion Checklist

- [ ] All run/try tests pass
- [ ] All match tests pass
- [ ] All recurse tests pass
- [ ] All parallel/spawn/nursery tests pass
- [ ] All timeout/cache tests pass
- [ ] All with tests pass
- [ ] All for/loop tests pass
- [ ] Round-trip verification for all pattern types
