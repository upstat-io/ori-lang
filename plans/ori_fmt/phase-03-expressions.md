# Phase 3: Expressions

**Goal**: Implement formatting for all expression types: calls, chains, conditionals, lambdas, and binary expressions.

> **DESIGN**: `docs/tooling/formatter/design/02-constructs/expressions.md`

## Phase Status: 🔶 In Progress

## 3.1 Function Calls

- [x] **Implement**: Simple call (inline)
  - [x] **Golden Tests**: `tests/fmt/expressions/calls/simple.ori`
  ```ori
  print(msg: "Hello")
  ```
- [x] **Implement**: Multi-argument call (inline if fits)
  - [x] **Golden Tests**: `tests/fmt/expressions/calls/multi_arg.ori`
- [ ] **Implement**: Multi-argument call (broken when >100 chars)
  - [ ] **Golden Tests**: `tests/fmt/expressions/calls/broken.ori`
  ```ori
  send_email(
      to: recipient_address,
      subject: email_subject,
      body: email_content,
  )
  ```
- [x] **Implement**: Nested call (independent breaking)
  - [x] **Golden Tests**: `tests/fmt/expressions/calls/nested.ori`
- [x] **Implement**: Call with lambda argument (positional single-param)
  - [x] **Golden Tests**: `tests/fmt/expressions/calls/lambda_arg.ori`
- [ ] **Implement**: Generic call
  - [ ] **Golden Tests**: `tests/fmt/expressions/calls/generic.ori`

## 3.2 Method Chains

- [x] **Implement**: Short chain (inline if fits)
  - [x] **Golden Tests**: `tests/fmt/expressions/chains/short.ori`
  ```ori
  items.filter(x -> x > 0).map(x -> x * 2)
  ```
- [ ] **Implement**: Long chain (all-or-nothing breaking)
  - [ ] **Golden Tests**: `tests/fmt/expressions/chains/long.ori`
  ```ori
  items
      .filter(predicate: is_valid)
      .map(transform: process)
      .fold(initial: 0, op: sum)
  ```
- [ ] **Implement**: Chain with complex receiver
  - [ ] **Golden Tests**: `tests/fmt/expressions/chains/complex_receiver.ori`
- [ ] **Implement**: Chain indentation (4 spaces from receiver)
  - [ ] **Golden Tests**: Proper chain alignment
- [x] **Implement**: Mixed field access and method calls
  - [x] **Golden Tests**: `tests/fmt/expressions/chains/mixed.ori`

## 3.3 Conditionals

- [x] **Implement**: Simple if-then-else (inline)
  - [x] **Golden Tests**: `tests/fmt/expressions/conditionals/simple.ori`
  ```ori
  if x > 0 then x else -x
  ```
- [x] **Implement**: If-then without else
  - [x] **Golden Tests**: `tests/fmt/expressions/conditionals/no_else.ori`
- [ ] **Implement**: Multi-line conditional (break at else)
  - [ ] **Golden Tests**: `tests/fmt/expressions/conditionals/multiline.ori`
  ```ori
  if condition then
      result_when_true
  else
      result_when_false
  ```
- [x] **Implement**: Chained if-else-if
  - [x] **Golden Tests**: `tests/fmt/expressions/conditionals/chained.ori`
- [ ] **Implement**: Complex condition (breaking)
  - [ ] **Golden Tests**: `tests/fmt/expressions/conditionals/complex_cond.ori`
- [ ] **Implement**: Complex branches (independent breaking)
  - [ ] **Golden Tests**: `tests/fmt/expressions/conditionals/complex_branch.ori`

## 3.4 Lambdas

- [x] **Implement**: Single parameter lambda
  - [x] **Golden Tests**: `tests/fmt/expressions/lambdas/single.ori`
  ```ori
  x -> x + 1
  ```
- [x] **Implement**: Multi-parameter lambda
  - [x] **Golden Tests**: `tests/fmt/expressions/lambdas/multi.ori`
  ```ori
  (a, b) -> a + b
  ```
- [x] **Implement**: No-parameter lambda
  - [x] **Golden Tests**: `tests/fmt/expressions/lambdas/no_param.ori`
  ```ori
  () -> 42
  ```
- [ ] **Implement**: Typed lambda
  - [ ] **Golden Tests**: `tests/fmt/expressions/lambdas/typed.ori`
  ```ori
  (x: int) -> int = x * 2
  ```
- [ ] **Implement**: Lambda with complex body (inline)
  - [ ] **Golden Tests**: `tests/fmt/expressions/lambdas/complex_inline.ori`
- [ ] **Implement**: Lambda with always-stacked body (run/try/match)
  - [ ] **Golden Tests**: `tests/fmt/expressions/lambdas/stacked_body.ori`
  ```ori
  x -> run(
      let result = process(x),
      result
  )
  ```
- [ ] **Implement**: Lambda breaking (only for always-stacked patterns)
  - [ ] **Golden Tests**: Lambda body formatting rules

## 3.5 Binary Expressions

- [x] **Implement**: Simple binary (inline)
  - [x] **Golden Tests**: `tests/fmt/expressions/binary/simple.ori`
  ```ori
  a + b * c
  ```
- [ ] **Implement**: Long binary (break before operator)
  - [ ] **Golden Tests**: `tests/fmt/expressions/binary/long.ori`
  ```ori
  very_long_variable_name
      + another_very_long_name
      + yet_another_long_name
  ```
- [ ] **Implement**: Operator precedence preservation
  - [ ] **Golden Tests**: `tests/fmt/expressions/binary/precedence.ori`
- [x] **Implement**: Logical operators (short-circuit)
  - [x] **Golden Tests**: `tests/fmt/expressions/binary/logical.ori`
- [x] **Implement**: Comparison chains
  - [x] **Golden Tests**: `tests/fmt/expressions/binary/comparison.ori`
- [x] **Implement**: Range expressions (`..`, `..=`)
  - [x] **Golden Tests**: `tests/fmt/expressions/binary/range.ori`
  - Note: `by` range stepping not implemented in parser

## 3.6 Bindings

- [x] **Implement**: Simple let binding
  - [x] **Golden Tests**: `tests/fmt/expressions/bindings/simple.ori`
  ```ori
  let x = 42
  ```
- [x] **Implement**: Immutable binding (module-level only)
  - [x] **Golden Tests**: `tests/fmt/expressions/bindings/immutable.ori`
  - Note: `$` prefix only valid at module level, not in run patterns
- [ ] **Implement**: Typed binding
  - [ ] **Golden Tests**: `tests/fmt/expressions/bindings/typed.ori`
- [x] **Implement**: Struct destructuring
  - [x] **Golden Tests**: `tests/fmt/expressions/bindings/destructure_struct.ori`
  ```ori
  let { x, y } = point
  ```
- [x] **Implement**: Tuple destructuring
  - [x] **Golden Tests**: `tests/fmt/expressions/bindings/destructure_tuple.ori`
- [ ] **Implement**: List destructuring
  - [ ] **Golden Tests**: `tests/fmt/expressions/bindings/destructure_list.ori`
- [ ] **Implement**: Nested destructuring
  - [ ] **Golden Tests**: `tests/fmt/expressions/bindings/nested.ori`
- [ ] **Implement**: Long value (breaking)
  - [ ] **Golden Tests**: `tests/fmt/expressions/bindings/long_value.ori`

## 3.7 Indexing and Access

- [x] **Implement**: List indexing
  - [x] **Golden Tests**: `tests/fmt/expressions/access/list.ori`
  ```ori
  list[0]
  list[# - 1]
  ```
- [ ] **Implement**: Map indexing
  - [ ] **Golden Tests**: `tests/fmt/expressions/access/map.ori`
- [x] **Implement**: Field access
  - [x] **Golden Tests**: `tests/fmt/expressions/access/field.ori`
- [ ] **Implement**: Complex index expression
  - [ ] **Golden Tests**: `tests/fmt/expressions/access/complex.ori`

## 3.8 Type Conversions

- [ ] **Implement**: Infallible conversion (`as`)
  - [x] **Golden Tests**: `tests/fmt/expressions/conversions/as.ori` (placeholder)
  - Note: `as` syntax not yet implemented in parser
- [ ] **Implement**: Fallible conversion (`as?`)
  - [ ] **Golden Tests**: `tests/fmt/expressions/conversions/try_as.ori`

## 3.9 Error Propagation

- [ ] **Implement**: Question mark operator
  - [x] **Golden Tests**: `tests/fmt/expressions/errors/propagate.ori` (placeholder)
  - Note: `?` inside try patterns has parsing issues

## Known Parser Limitations

The following features are specified but not yet implemented in the parser:
- `as` / `as?` type conversion syntax
- `by` range stepping (`0..10 by 2`)
- `$` immutable bindings in local scopes (only valid at module level)
- Parenthesis preservation in AST (affects range precedence in method chains)

## Completion Checklist

- [x] Basic function call tests pass
- [x] Basic method chain tests pass
- [x] Basic conditional tests pass
- [x] Basic lambda tests pass
- [x] Basic binary expression tests pass
- [x] Basic binding tests pass
- [ ] All edge case tests pass
- [ ] Round-trip verification for all expression types
