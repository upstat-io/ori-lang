export const EXAMPLES: Record<string, { label: string; code: string }> = {
  hello: {
    label: 'Hello World',
    code: `// Hello World in Ori
@main () -> void = print(msg: "Hello, World!")`,
  },
  fibonacci: {
    label: 'Fibonacci',
    code: `// Memoized - O(n) instead of O(2^n)
@fib (n: int) -> int = recurse(
    condition: n < 2,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,
)

@main () -> void = run(
    print(msg: "fib(30) = " + str(fib(n: 30)))
)`,
  },
  factorial: {
    label: 'Factorial',
    code: `// Factorial with recursion
@factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n: n - 1)

@main () -> void = run(
    print(msg: "5! = " + str(factorial(n: 5)))
)`,
  },
  'list-ops': {
    label: 'List Operations',
    code: `// List operations
@main () -> void = run(
    let numbers = [1, 2, 3, 4, 5],
    let doubled = numbers.map(transform: x -> x * 2),
    let evens = doubled.filter(predicate: x -> x % 2 == 0),
    let sum = evens.fold(initial: 0, op: (acc, x) -> acc + x),
    print(msg: "Sum of doubled evens: " + str(sum))
)`,
  },
  structs: {
    label: 'Structs',
    code: `// Structs and methods
type Point = { x: int, y: int }

impl Point {
    @sum (self) -> int = self.x + self.y
    @scale (self, factor: int) -> Point = Point { x: self.x * factor, y: self.y * factor }
}

@main () -> void = run(
    let p = Point { x: 3, y: 4 },
    print(msg: "Point sum: " + str(p.sum())),
    let scaled = p.scale(factor: 2),
    print(msg: "Scaled: (" + str(scaled.x) + ", " + str(scaled.y) + ")")
)`,
  },
};

export const DEFAULT_CODE = EXAMPLES.hello.code;
