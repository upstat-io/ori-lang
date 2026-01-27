# Rosetta Code Programming Tasks

Complete list of ~1,200 programming tasks for language validation.

## Essential Tasks for Ori (Recommended Starting Set)

These 20 tasks cover the core language features:

### Basics
1. **Hello world/Text** - Output
2. **A+B** - Input, arithmetic
3. **FizzBuzz** - Conditionals, loops
4. **Factorial** - Recursion
5. **Fibonacci sequence** - Recursion, iteration

### Data Structures
6. **Arrays** - List creation/access
7. **Associative array/Creation** - Maps/dictionaries
8. **Stack** - LIFO structure
9. **Queue/Definition** - FIFO structure

### Strings
10. **String concatenation** - Basic string ops
11. **String length** - String functions
12. **Reverse a string** - String manipulation

### Control Flow
13. **Loops/For** - Iteration
14. **Loops/While** - Conditional loops
15. **Conditional structures** - If/else

### Functions
16. **Function definition** - Basic functions
17. **Higher-order functions** - Functions as values
18. **Closures/Value capture** - Lexical scope

### I/O & Error Handling
19. **Read a file line by line** - File I/O
20. **Exceptions** - Error handling

---

## Full Task List (A-Z)

### 0-9

#### 100 doors
**Problem:** Simulate toggling 100 doors through multiple passes and determine their final states.

**Requirements:**
- Start with 100 doors, all initially closed
- Make 100 passes:
  - Pass 1: Toggle every door
  - Pass 2: Toggle every 2nd door
  - Pass 3: Toggle every 3rd door
  - ...continue through Pass 100
- Toggle means: if closed, open it; if open, close it

**Success Criteria:**
- Output which doors remain open after all 100 passes
- Expected result: Doors 1, 4, 9, 16, 25, 36, 49, 64, 81, 100 (perfect squares)

---

#### 100 prisoners
**Problem:** Simulate the 100 prisoners problem to compare survival strategies.

**Setup:**
- 100 prisoners numbered 1-100
- 100 drawers, each containing a randomly shuffled card numbered 1-100
- Each prisoner can open maximum 50 drawers
- All prisoners must find their own number for everyone to be pardoned

**Requirements:**
- Implement two strategies:
  1. **Random:** Each prisoner randomly selects 50 drawers
  2. **Optimal:** Prisoner opens drawer matching their number, then follows the chain
- Simulate thousands of game instances with each strategy
- Calculate and display success probabilities

**Success Criteria:**
- Random strategy: ~0% success rate
- Optimal strategy: ~30-31% success rate

---

#### 15 puzzle game
**Problem:** Implement the classic 15 Puzzle sliding tile game.

**Requirements:**
- Create a 4×4 grid with tiles numbered 1-15 and one empty space
- Display the current puzzle state
- Accept player input to slide tiles into the empty space
- Only allow valid moves (tiles adjacent to empty space)
- Generate a shuffled but solvable initial configuration
- Track move count
- Detect solved state (tiles 1-15 in order, empty space last)

**Success Criteria:**
- Displays shuffled puzzle board
- Players can move tiles via input
- Invalid moves are rejected
- Recognizes and announces solved state
- Shows move count upon victory

---

#### 15 puzzle solver
**Problem:** Write a program that solves the 15 Puzzle optimally or near-optimally.

**Requirements:**
- Solve this specific configuration:
  ```
  15 14  1  6
   9 11  4 12
   0 10  7  3
  13  8  5  2
  ```
- Reach goal state:
  ```
   1  2  3  4
   5  6  7  8
   9 10 11 12
  13 14 15  0
  ```
- Output move sequence (e.g., "rrrulddluuuldr...")

**Success Criteria:**
- Find a solution (optimal is 52 moves)
- Output move directions as a sequence
- Bonus: Solve extra puzzle starting with 0 in top-left

---

#### 2048
**Problem:** Implement the 2048 sliding block puzzle game.

**Requirements:**
- 4×4 grid with numbered tiles
- Player chooses direction (up/down/left/right) each turn
- All tiles slide as far as possible in chosen direction
- Matching adjacent tiles combine (sum their values)
- Tiles created by combining cannot combine again same turn
- After each valid move, spawn new tile (90% chance: 2, 10% chance: 4)
- A move is valid only if at least one tile moves or combines

**Success Criteria:**
- Win condition: Create a 2048 tile
- Lose condition: Board full with no valid moves
- Properly handle edge cases (e.g., `[2][2][2][2]` → `[4][4]` not `[8]`)

---

#### 21 game
**Problem:** Implement the "21" two-player counting game.

**Requirements:**
- Running total starts at 0
- Players alternate adding 1, 2, or 3 to the total
- Player who reaches exactly 21 wins
- One player is human, one is computer
- Computer should play intelligently (winning strategy exists)
- Validate input (only 1, 2, or 3 allowed)
- Display running total after each move
- Allow quitting and replaying

**Success Criteria:**
- Correctly identifies winner at exactly 21
- Prevents invalid moves
- Computer opponent functions properly
- Clear winner announcement

---

#### 24 game
**Problem:** Implement the 24 Game where players form expressions equaling 24.

**Requirements:**
- Randomly choose and display four digits (1-9, repetitions allowed)
- Player enters an arithmetic expression using those digits
- Validate the expression:
  - Must use all four digits exactly once
  - No forming multi-digit numbers (can't combine 1,2 into 12)
  - Only +, -, *, / operators allowed
  - Parentheses permitted
- Use floating point division (preserve remainders)
- Evaluate and check if result equals 24

**Success Criteria:**
- Correctly validates digit usage
- Properly evaluates expressions
- Accepts valid solutions, rejects invalid ones
- Note: Program validates player input, does NOT generate solutions

---

#### 24 game/Solve
**Problem:** Write a solver for the 24 Game that finds valid expressions.

**Requirements:**
- Accept four digits (1-9) from user or random generation
- Find arithmetic expressions that evaluate to exactly 24
- Use each digit exactly once
- Only +, -, *, / operators
- Consider all permutations of digits
- Consider all operator combinations
- Consider all parenthesization patterns
- Handle division by zero cases
- Use proper floating-point comparison

**Success Criteria:**
- Displays valid solution(s) if they exist
- Indicates when no solution exists
- Expressions shown evaluate correctly to 24

---

#### 4-rings or 4-squares puzzle
**Problem:** Find digit assignments where sums of overlapping squares are equal.

**Requirements:**
- Assign digits to variables a, b, c, d, e, f, g
- Constraint: `a + b = b + c + d = d + e + f = f + g`
- Solve three test cases:
  1. Digits 1-7, unique values, display all solutions
  2. Digits 3-9, unique values, display all solutions
  3. Digits 0-9, repeats allowed, report count only

**Success Criteria:**
- Test 1: Find 8 unique solutions
- Test 2: Find 4 unique solutions
- Test 3: Find 2860 non-unique solutions

---

#### 9 billion names of God the integer
**Problem:** Generate a number triangle representing integer partitions.

**Requirements:**
- Display first 25 rows of the partition triangle
- Row n shows partition counts beginning with each possible value
- Implement G(n) returning sum of n-th row (equals partition function P(n))
- Compute G() for: 23, 123, 1234, 12345

**Triangle structure (first rows):**
```
1
1 1
1 1 1
1 2 1 1
1 2 2 1 1
1 3 3 2 1 1
```

**Success Criteria:**
- Accurate 25-row triangle
- Correct row sums matching partition function
- Handle large integers (results exceed 64-bit for larger inputs)

---

#### 99 bottles of beer
**Problem:** Display the complete lyrics to "99 Bottles of Beer on the Wall."

**Requirements:**
- Countdown from 99 to 0
- Each verse follows pattern:
  - "X bottles of beer on the wall, X bottles of beer"
  - "Take one down, pass it around"
  - "(X-1) bottles of beer on the wall"
- Handle grammar: "bottle" (singular) vs "bottles" (plural)
- Verses should be separated appropriately

**Success Criteria:**
- Complete lyrics from 99 down to 0
- Grammatically correct throughout
- Proper singular/plural handling for "1 bottle"

---

### A

#### A+B
**Problem:** Classic programming contest problem - read two integers and output their sum.

**Requirements:**
- Read two integers from input (space-separated)
- Constraints: -1000 ≤ A, B ≤ +1000
- Output their sum

**Success Criteria:**
- Input: `2 2` → Output: `4`
- Input: `3 2` → Output: `5`

---

#### Abbreviations, automatic
**Problem:** Find minimum abbreviation length to uniquely identify each word in a list.

**Requirements:**
- Read lines of space-separated words
- For each line, find minimum prefix length where all abbreviations are unique
- Example: "Sunday Monday Tuesday..." needs length 2 ("Su", "Mo", "Tu"... all distinct)

**Output:**
- Display minimum length (right-aligned, width 2) followed by the line
- Handle blank lines (return 0 or empty)
- Support Unicode/accented characters

**Success Criteria:**
- Correctly computes minimum unique abbreviation length per line
- Handles edge cases (empty lines, special characters)

---

#### Abbreviations, easy
**Problem:** Validate user input against a command table with abbreviation rules.

**Requirements:**
- Command table uses capitals to show minimum abbreviation (e.g., "ALTer" needs 3+ chars)
- Valid abbreviation must:
  - Be at least as long as the capital letter count
  - Match leading characters (case-insensitive)
  - Not exceed full command length
- Return full uppercase command name for valid matches
- Return `*error*` for invalid inputs

**Example:**
- Input: `riG rePEAT copies put mo rest`
- Output: `RIGHT REPEAT *error* PUT MOVE RESTORE`

**Success Criteria:**
- Correctly matches abbreviations to commands
- Handles case-insensitivity
- Returns `*error*` for non-matches

---

#### Abbreviations, simple
**Problem:** Validate words against a command table with explicit minimum lengths.

**Requirements:**
- Command table has commands with optional minimum abbreviation lengths
- If no number follows command, no abbreviation is permitted
- Valid abbreviation: length ≥ minimum, length ≤ full, matches prefix (case-insensitive)
- Non-alphabetic input is invalid
- Return uppercase command for valid, `*error*` for invalid

**Example:**
- Input: `riG rePEAT copies put mo rest`
- Output: `RIGHT REPEAT *error* PUT MOVE RESTORE`

**Success Criteria:**
- Correctly validates against explicit length requirements
- Handles case-insensitivity
- Rejects non-alphabetic input

---

#### ABC problem
**Problem:** Determine if a word can be spelled using letter blocks (each block has 2 letters).

**Requirements:**
- 20 blocks with two letters each (e.g., (B,O), (X,K), (D,Q)...)
- Each block can only be used once
- Case-insensitive matching
- Test words: A, BARK, BOOK, TREAT, COMMON, SQUAD, CONFUSE

**Success Criteria:**
- A → True
- BARK → True
- BOOK → False (need two O's, only one O block)
- TREAT → True
- COMMON → False
- SQUAD → True
- CONFUSE → True

---

#### Abelian sandpile model
**Problem:** Implement the Abelian sandpile cellular automaton (Bak–Tang–Wiesenfeld model).

**Requirements:**
- Create a 2D grid of arbitrary size
- Place sand particles at any location
- Collapse rule: when cell has ≥4 grains, it loses 4 and distributes 1 to each neighbor (up/down/left/right)
- Cascade collapses until all cells have <4 grains
- Display results (image format preferred, terminal OK for small grids)

**Success Criteria:**
- Correctly simulates sandpile dynamics
- Reaches stable equilibrium (no cell ≥4)
- Handles arbitrary initial configurations

---

#### Abstract type
**Problem:** Demonstrate how to declare an abstract type in the language.

**Requirements:**
- Show syntax for declaring an abstract type
- Abstract type = type without instances or without full definition
- If language distinguishes interfaces vs partially-implemented types, show both:
  - Interface: no implementation, only signatures
  - Abstract class: mix of abstract and concrete methods
- Show how to create concrete implementations

**Success Criteria:**
- Demonstrates abstract type declaration
- Shows prevention of direct instantiation
- Includes concrete subclass fulfilling requirements

---

#### Abundant odd numbers
**Problem:** Find odd numbers where sum of proper divisors exceeds the number itself.

**Requirements:**
- Abundant number: sum of proper divisors > number (e.g., 945: divisors sum to 975 > 945)
- Display first 25 abundant odd numbers with their divisor sums
- Find and display the 1000th abundant odd number
- Find first abundant odd number > 1,000,000,000

**Success Criteria:**
- Correctly calculates proper divisor sums
- Identifies all three targets
- Handles large numbers efficiently

---

#### Accumulator factory
**Problem:** Create a function that returns an accumulator function (closure with mutable state).

**Requirements:**
- `foo(n)` returns accumulator function `g`
- `g(i)` adds `i` to running total and returns new sum
- Must work with both integers and floats
- State persists across calls, no global variables

**Example:**
```
x = foo(1)
x(5)     # returns 6
x(2.3)   # returns 8.3
```

**Success Criteria:**
- Demonstrates closures with mutable state
- Each accumulator is independent
- Handles numeric types correctly

---

#### Achilles numbers
**Problem:** Find Achilles numbers (powerful but imperfect numbers).

**Definitions:**
- Powerful: for every prime factor p, p² also divides n
- Achilles: powerful but not a perfect power (can't be written as m^k, m,k > 1)
- Strong Achilles: Achilles number whose totient is also Achilles

**Requirements:**
- Display first 50 Achilles numbers
- Display first 20 strong Achilles numbers
- Count Achilles numbers by digit length (2-6 digits)

**Success Criteria:**
- First 50 starts: 72, 108, 200, 288, 392...
- Digit counts: 2→1, 3→12, 4→47, 5→192, 6→664

---

#### Ackermann function
**Problem:** Implement the Ackermann function (classic non-primitive recursive function).

**Definition:**
```
A(m, n) = n + 1                  if m = 0
A(m, n) = A(m-1, 1)              if m > 0, n = 0
A(m, n) = A(m-1, A(m, n-1))      if m > 0, n > 0
```

**Requirements:**
- Handle non-negative integer arguments
- Arbitrary precision preferred (grows very quickly)

**Success Criteria:**
- A(3, 4) = 125
- A(4, 1) = 65533
- A(3, 5) = 253

---

#### Active object
**Problem:** Implement an active integrator object that continuously updates using trapezoid integration.

**Requirements:**
- Object continuously integrates: `S = S + (K(t₁) + K(t₀)) × (t₁ - t₀) / 2`
- Methods: `Input(K)` to set input function, `Output()` to get accumulated value
- Test sequence:
  1. Set input to `sin(2πft)` where f = 0.5 Hz
  2. Wait 2 seconds
  3. Set input to 0
  4. Wait 0.5 seconds
  5. Verify output ≈ 0

**Success Criteria:**
- Output approximately 0 (within ±0.001)
- Integrating full sine cycle yields zero

---

#### Add a variable to a class instance at runtime
**Problem:** Demonstrate adding variables to an object instance after creation.

**Requirements:**
- Create a class/object instance
- Add a new variable/attribute to that specific instance at runtime
- Access the newly added variable
- Static languages may use Maps/Dictionaries to simulate

**Success Criteria:**
- Shows mechanism for dynamic attribute addition
- Demonstrates accessing the new attribute
- Works within language's type system

---

#### Additive primes
**Problem:** Find primes whose digit sum is also prime.

**Requirements:**
- Find all additive primes less than 500
- Additive prime: number is prime AND sum of its digits is prime
- Display all qualifying numbers
- Optionally show count

**Success Criteria:**
- Expected: 54 additive primes below 500
- Examples: 2, 3, 5, 7, 11, 23, 29, 41, 43, 47...

---

#### Address of a variable
**Problem:** Demonstrate getting and setting a variable's memory address.

**Requirements:**
- Get address: retrieve memory location of a variable
- Set address: direct a variable to a specific memory location
- Show language-appropriate mechanisms (pointers, references, etc.)

**Success Criteria:**
- Demonstrates obtaining a variable's address
- Shows directing a variable to a specific location
- Note: Some high-level languages don't expose raw addresses

---

#### AKS test for primes
**Problem:** Implement AKS primality test using polynomial coefficients.

**Theory:** p is prime iff all coefficients of (x-1)^p - (x^p - 1) are divisible by p.

**Requirements:**
- Create coefficient generator for (x-1)^p expansion
- Display polynomial expansions for p = 0 to 7
- Implement primality test using coefficient divisibility
- Find all primes under 35
- Stretch: primes under 50 (needs >31-bit integers)

**Success Criteria:**
- Correct binomial coefficients
- Readable polynomial format
- Accurate prime identification

---

#### Algebraic data types
**Problem:** Demonstrate algebraic data types by implementing red-black tree insertion.

**Red-Black Tree Properties:**
- Each node is red or black
- No red node has a red child
- All paths root→empty have same black count

**Requirements:**
- Show language support for algebraic data types
- Use pattern matching on tree structures
- Implement insertion with automatic rebalancing
- Handle four rebalancing cases

**Success Criteria:**
- Creates valid red-black tree from insertions
- Pattern matching identifies rebalancing needs
- Maintains tree balance properties

---

#### Align columns
**Problem:** Align dollar-delimited text into formatted columns.

**Requirements:**
- Parse text with `$` as field delimiter
- Calculate max width per column from data
- Output three versions: left-justified, right-justified, center-justified
- At least one space between columns
- Compute spacing from data (not hard-coded)

**Success Criteria:**
- Handles varying field counts per line
- All columns use same alignment method
- Produces readable columnar output

---

#### Aliquot sequence classifications
**Problem:** Classify aliquot sequences by their termination/repetition patterns.

**Sequence:** Each term = sum of proper divisors of previous term.

**Classifications:**
1. Terminating - reaches 0
2. Perfect - returns to start immediately (period 1)
3. Amicable - returns to start on 3rd term (period 2)
4. Sociable - returns to start after N>3 terms
5. Aspiring - settles into repeating non-start number
6. Cyclic - enters cycle with non-start number
7. Non-terminating - doesn't classify after 16 terms or exceeds 2^47

**Test:** Numbers 1-10, plus 11, 12, 28, 496, 220, 1184, 12496, 1264460, 790, 909, 562, 1064, 1488

**Success Criteria:** Show classification and full sequence for each number

---

#### Almost prime
**Problem:** Find k-almost primes (numbers with exactly k prime factors, with multiplicity).

**Requirements:**
- Generate k-almost primes for given k
- Display first 10 k-almost primes for k = 1 to 5

**Success Criteria:**
- k=1: 2, 3, 5, 7, 11, 13, 17, 19, 23, 29 (primes)
- k=2: 4, 6, 9, 10, 14, 15, 21, 22, 25, 26 (semiprimes)
- k=3: 8, 12, 18, 20, 27, 28, 30, 42, 44, 45
- k=4: 16, 24, 36, 40, 54, 56, 60, 81, 84, 88
- k=5: 32, 48, 72, 80, 108, 112, 120, 162, 168, 176

---

#### Amb
**Problem:** Implement the Amb (ambiguous) operator for nondeterministic choice with backtracking.

**Behavior:**
- `amb()` with no args fails
- `amb(v1, v2, ...)` explores all possibilities
- Backtracks when constraints fail, succeeds when all constraints met

**Test Case:** Select one word from each set where last char of each word equals first char of next:
- Set 1: "the", "that", "a"
- Set 2: "frog", "elephant", "thing"
- Set 3: "walked", "treaded", "grows"
- Set 4: "slowly", "quickly"

**Success Criteria:**
- Output: "that thing grows slowly"
- Must use actual backtracking, not just nested loops

---

#### Amicable pairs
**Problem:** Find all amicable number pairs below 20,000.

**Definition:** N and M are amicable if:
- N ≠ M
- Sum of proper divisors of N = M
- Sum of proper divisors of M = N

**Example:** (220, 284) - divisors of 220 sum to 284, divisors of 284 sum to 220

**Success Criteria:** Find all 8 pairs:
```
220 284
1184 1210
2620 2924
5020 5564
6232 6368
10744 10856
12285 14595
17296 18416
```

---

#### Anagrams
**Problem:** Find the largest sets of anagrams from a dictionary.

**Requirements:**
- Use word list from `http://wiki.puzzlers.org/pub/wordlists/unixdict.txt`
- Group words by sorted letters (canonical form)
- Find group(s) with maximum word count

**Success Criteria:** Find 6 groups of 5 anagrams each:
- abel, able, bale, bela, elba
- caret, carte, cater, crate, trace
- angel, angle, galen, glean, lange
- alger, glare, lager, large, regal
- elan, lane, lean, lena, neal
- evil, levi, live, veil, vile

---

#### Anagrams/Deranged anagrams
**Problem:** Find the longest deranged anagram pair from a dictionary.

**Definition:** Deranged anagram = two words that:
- Are anagrams (same letters)
- Have NO character in the same position

**Requirements:**
- Use `http://wiki.puzzlers.org/pub/wordlists/unixdict.txt`
- Find longest pair where no positions match

**Success Criteria:**
- Output: "excitation" and "intoxicate" (11 chars)

---

#### Angle difference between two bearings
**Problem:** Calculate the difference between two compass bearings, normalized to [-180, +180].

**Requirements:**
- Input: two bearings (degrees)
- Output: b2 - b1, normalized to -180 to +180 range
- Handle circular arithmetic (360° = full rotation)

**Test Cases:**
- (20, 45), (-45, 45), (-85, 90), (-95, 90)
- (-45, 125), (-45, 145), (29.4803, -88.6381)

**Success Criteria:**
- Results always in [-180, 180] range
- Bonus: handle large values outside standard range

---

#### Animate a pendulum
**Problem:** Create a physical simulation of a swinging pendulum with animation.

**Requirements:**
- Implement physics: acceleration = -(g/L) × sin(θ)
- Parameters: gravity (~9.81 m/s²), length, initial angle
- Update position based on velocity and angle over time
- Render graphically: pivot, rod, bob

**Success Criteria:**
- Smooth, continuous swinging motion
- Realistic physics behavior
- Stable animation (no divergence)

---
- Animation
- Anonymous recursion
- Anti-primes
- Append a record to the end of a text file
- Apply a callback to an array
- Approximate equality
- Arbitrary-precision integers
- Archimedean spiral
- Arithmetic derivative
- Arithmetic evaluation
- Arithmetic-geometric mean
- Arithmetic/Complex
- Arithmetic/Integer
- Arithmetic/Rational
- Array concatenation
- Array length
- Arrays
- Ascending primes
- ASCII art diagram converter
- Assertions
- Associative array/Creation
- Associative array/Iteration
- Associative array/Merging
- Atomic updates
- Attractive numbers
- Average loop length
- Averages/Arithmetic mean
- Averages/Median
- Averages/Mode
- Averages/Root mean square
- AVL tree

### B
- Babbage problem
- Balanced brackets
- Balanced ternary
- Base64 decode data
- Bell numbers
- Benford's law
- Bernoulli numbers
- Best shuffle
- Bifid cipher
- Binary digits
- Binary search
- Binary strings
- Bitmap
- Bitmap/Bresenham's line algorithm
- Bitmap/Flood fill
- Bitwise operations
- Boolean values
- Box the compass
- Boyer-Moore string search
- Brace expansion
- Brazilian numbers
- Brownian tree
- Bulls and cows
- Burrows–Wheeler transform

### C
- Caesar cipher
- Calculating the value of e
- Calendar
- Call a function
- Camel case and snake case
- Cartesian product of two or more lists
- Case-sensitivity of identifiers
- Casting out nines
- Catalan numbers
- Catamorphism
- Character codes
- Chat server
- Check that file exists
- Chinese remainder theorem
- Cholesky decomposition
- Church numerals
- Circular primes
- Classes
- Closest-pair problem
- Closures/Value capture
- Collections
- Color quantization
- Combinations
- Combinations with repetitions
- Comma quibbling
- Command-line arguments
- Comments
- Compare a list of strings
- Compile-time calculation
- Compiler/lexical analyzer
- Compiler/syntax analyzer
- Compound data type
- Concurrent computing
- Conditional structures
- Continued fraction
- Convert decimal number to rational
- Convert seconds to compound duration
- Convex hull
- Conway's Game of Life
- Copy a string
- Count in factors
- Count occurrences of a substring
- Count the coins
- Cramer's rule
- CRC-32
- Create a file
- Create a two-dimensional array at runtime
- CSV data manipulation
- Currying

### D
- Damm algorithm
- Date format
- Date manipulation
- Day of the week
- De Bruijn sequences
- Deepcopy
- Define a primitive data type
- Delegates
- Delete a file
- Determinant and permanent
- Determine if a string has all unique characters
- Determine if a string is numeric
- Dijkstra's algorithm
- Dinesman's multiple-dwelling problem
- Dining philosophers
- Dot product
- Doubly-linked list/Definition
- Doubly-linked list/Traversal
- Dragon curve
- Draw a clock
- Dutch national flag problem
- Dynamic variable names

### E
- Echo server
- Egyptian division
- Element-wise operations
- Empty program
- Empty string
- Enforced immutability
- Entropy
- Enumerations
- Environment variables
- Equilibrium index
- Ethiopian multiplication
- Euclidean rhythm
- Euler method
- Euler's identity
- Evaluate binomial coefficients
- Even or odd
- Events
- Evolutionary algorithm
- Exceptions
- Exceptions/Catch an exception thrown in a nested call
- Executable library
- Execute a system command
- Extend your language
- Extensible prime generator

### F
- Factorial
- Factors of an integer
- Farey sequence
- Fast Fourier transform
- Fibonacci sequence
- File input/output
- File modification time
- File size
- Filter
- Find common directory path
- Find duplicate files
- Find limit of recursion
- First-class functions
- Five weekends
- FizzBuzz
- Flatten a list
- Floyd's triangle
- Floyd-Warshall algorithm
- Forest fire
- Fork
- Formatted numeric output
- Forward difference
- Four bit adder
- Fractal tree
- Fractran
- Function composition
- Function definition
- Functional coverage tree

### G
- Gamma function
- Gauss-Jordan matrix inversion
- Gaussian elimination
- General FizzBuzz
- Generate lower case ASCII alphabet
- Generator/Exponential
- Generic swap
- Get system command output
- Globally replace text in several files
- Golden ratio/Convergence
- Gray code
- Greatest common divisor
- Greatest element of a list
- Greatest subsequential sum
- Greedy algorithm for Egyptian fractions

### H
- Hailstone sequence
- Hamming numbers
- Handle a signal
- Happy numbers
- Harmonic series
- Hash from two arrays
- Hash join
- Haversine formula
- Hello world/Text
- Here document
- Heronian triangles
- Higher-order functions
- Hilbert curve
- History variables
- Hofstadter Q sequence
- Hostname
- HTTP
- Huffman coding

### I
- I before E except after C
- IBAN
- Identity matrix
- Include a file
- Increment a numerical string
- Index finite lists of positive integers
- Infinity
- Inheritance/Multiple
- Inheritance/Single
- Inner classes
- Input loop
- Integer comparison
- Integer overflow
- Integer sequence
- Interactive programming (repl)
- Introspection
- Inverted index
- ISBN13 check digit
- Iterated digits squaring

### J-K
- Jacobi symbol
- Jensen's Device
- Jewels and stones
- Josephus problem
- JSON
- Julia set
- Jump anywhere
- K-d tree
- Kaprekar numbers
- Kernighan's large earthquake problem
- Keyboard input/Keypress check
- Knapsack problem
- Knight's tour
- Knuth shuffle
- Knuth's algorithm S

### L
- Langton's ant
- Largest int from concatenated ints
- Last Friday of each month
- Latin Squares in reduced form
- Leap year
- Least common multiple
- Leonardo numbers
- Letter frequency
- Levenshtein distance
- Linear congruential generator
- List comprehensions
- Literals/Floating point
- Literals/Integer
- Literals/String
- Logical operations
- Long multiplication
- Longest common subsequence
- Longest common substring
- Longest increasing subsequence
- Look-and-say sequence
- Loop over multiple arrays simultaneously
- Loops/Break
- Loops/Continue
- Loops/Do-while
- Loops/For
- Loops/Foreach
- Loops/Infinite
- Loops/Nested
- Loops/While
- LU decomposition
- Lucas-Lehmer test
- Luhn test of credit card numbers
- LZW compression

### M
- MAC vendor lookup
- Mad Libs
- Magic squares of odd order
- Main step of GOST 28147-89
- Make directory path
- Man or boy test
- Mandelbrot set
- Map range
- Matrix multiplication
- Matrix transposition
- Maximum triangle path sum
- Maze generation
- Maze solving
- MD5
- Median filter
- Memory allocation
- Menu
- Merge and aggregate datasets
- Metaprogramming
- Miller-Rabin primality test
- Minesweeper game
- Modular arithmetic
- Modular exponentiation
- Modular inverse
- Monte Carlo methods
- Monty Hall problem
- Morse code
- Multifactorial
- Multiple distinct objects
- Multiplication tables
- Mutual recursion

### N
- N-queens problem
- Named parameters
- Naming conventions
- Narcissistic decimal number
- Natural sorting
- Negative base numbers
- Nested function
- Nim game
- Non-decimal radices/Convert
- Non-decimal radices/Input
- Non-decimal radices/Output
- Nth root
- Null object
- Number names
- Number reversal game
- Numeric error propagation
- Numerical integration

### O
- Object serialization
- Odd word problem
- Old lady swallowed a fly
- One of n lines in a file
- One-dimensional cellular automata
- Operator precedence
- Optional parameters
- Ordered words

### P
- Palindrome detection
- Pangram checker
- Parallel calculations
- Parse an IP Address
- Parsing/RPN calculator algorithm
- Parsing/Shunting-yard algorithm
- Partial function application
- Pascal's triangle
- Password generator
- Perfect numbers
- Perfect shuffle
- Permutations
- Permutations by swapping
- Phrase reversals
- Pi
- Pig the dice game
- Playfair cipher
- Playing cards
- Poker hand analyser
- Polymorphism
- Polynomial long division
- Population count
- Power set
- Prime decomposition
- Primorial numbers
- Priority queue
- Probabilistic choice
- Problem of Apollonius
- Program termination
- Proper divisors
- Pythagorean triples

### Q
- QR decomposition
- Quaternion
- Queue/Definition
- Queue/Usage
- Quickselect algorithm
- Quine

### R
- Random number generator (included)
- Random numbers
- Range consolidation
- Range expansion
- Range extraction
- Ranking methods
- Ray-casting algorithm
- Read a configuration file
- Read a file line by line
- Read a specific line from a file
- Read entire file
- Real constants and functions
- Recaman's sequence
- Reduced row echelon form
- Regular expressions
- Remove duplicate elements
- Remove lines from a file
- Rename a file
- Rep-string
- Repeat a string
- Return multiple values
- Reverse a string
- Reverse words in a string
- Roman numerals/Decode
- Roman numerals/Encode
- Roots of a function
- Roots of a quadratic function
- Roots of unity
- Rot-13
- Run-length encoding
- Runge-Kutta method
- Runtime evaluation

### S
- S-expressions
- Safe primes and unsafe primes
- Same fringe
- Scope modifiers
- Search a list
- Secure temporary file
- SEDOLs
- Self numbers
- Self-describing numbers
- Semiprime
- Send email
- Sequence of non-squares
- Sequence of primes by trial division
- Set
- Set consolidation
- Seven-sided dice from five-sided dice
- SHA-1
- SHA-256
- Shell one-liner
- Shoelace formula for polygonal area
- Short-circuit evaluation
- Shortest common supersequence
- Show ASCII table
- Sierpinski carpet
- Sierpinski triangle
- Sieve of Eratosthenes
- Simple database
- Singleton
- Singly-linked list/Definition
- Singly-linked list/Traversal
- Sleep
- Smith numbers
- SOAP
- Sockets
- Sokoban
- Sort a list of object identifiers
- Sort an array of composite structures
- Sort an integer array
- Sort using a custom comparator
- Sorting algorithms/Bubble sort
- Sorting algorithms/Heapsort
- Sorting algorithms/Insertion sort
- Sorting algorithms/Merge sort
- Sorting algorithms/Quicksort
- Sorting algorithms/Selection sort
- Soundex
- Special characters
- Speech synthesis
- Spiral matrix
- Split a character string based on change of character
- Square-free integers
- Stable marriage problem
- Stack
- Stack traces
- Start from a main routine
- Statistics/Basic
- Stem-and-leaf plot
- Stern-Brocot sequence
- Stirling numbers of the first kind
- String append
- String case
- String comparison
- String concatenation
- String interpolation (included)
- String length
- String matching
- String prepend
- Strip a set of characters from a string
- Strip block comments
- Strip comments from a string
- Strip whitespace from a string/Top and tail
- Substring
- Substring/Top and tail
- Sudoku
- Sum and product of an array
- Sum digits of an integer
- Sum multiples of 3 and 5
- Sum of a series
- Sum of squares
- Symmetric difference
- System time

### T
- Take notes on the command line
- Tau function
- Taxicab numbers
- Temperature conversion
- Terminal control/Clear the screen
- Terminal control/Cursor movement
- Ternary logic
- Test a function
- Test integerness
- Text processing/1
- Text processing/2
- Textonyms
- The Name Game
- The sieve of Sundaram
- The Twelve Days of Christmas
- Thue-Morse
- Tic-tac-toe
- Time a function
- Tokenize a string
- Tokenize a string with escaping
- Topological sort
- Topswops
- Totient function
- Towers of Hanoi
- Tree datastructures
- Tree from nesting levels
- Tree traversal
- Trigonometric functions
- Truncatable primes
- Truncate a file
- Truth table

### U
- Ulam spiral (for primes)
- Undefined values
- Unicode strings
- Universal Turing machine
- Unix/ls
- URL decoding
- URL encoding
- URL parser
- User input/Text
- UTF-8 encode and decode

### V
- Validate International Securities Identification Number
- Vampire number
- Van der Corput sequence
- Van Eck sequence
- Variable declaration reset
- Variable size/Get
- Variables
- Variadic function
- Vector
- Vector products
- Verify distribution uniformity/Naive
- Vigenère cipher

### W
- Walk a directory/Non-recursively
- Walk a directory/Recursively
- Water collected between towers
- Web scraping
- Weird numbers
- Window creation
- Word frequency
- Word ladder
- Word search
- Word wrap
- Write entire file
- Write float arrays to a text file

### X-Y-Z
- XML/Input
- XML/Output
- XML/XPath
- Y combinator
- Yellowstone sequence
- Yin and yang
- Zebra puzzle
- Zeckendorf number representation
- Zero to the zero power
- Zig-zag matrix
- Zumkeller numbers

---

## Task Count by Category

| Category | Count |
|----------|-------|
| String manipulation | ~50 |
| Mathematics | ~200 |
| Data structures | ~80 |
| Algorithms | ~150 |
| I/O & Files | ~40 |
| Graphics | ~60 |
| Puzzles & Games | ~100 |
| Cryptography | ~40 |
| Other | ~480 |
| **Total** | **~1,200** |
