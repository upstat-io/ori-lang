# 15 puzzle solver

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
