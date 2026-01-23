# Adding Rosetta Code Task Requirements

Instructions for continuing to add task requirements to ALL_TASKS.md.

## Process

1. **Find the task** in ALL_TASKS.md (listed alphabetically under letter sections)

2. **Fetch requirements** from Rosetta Code:
   ```
   https://rosettacode.org/wiki/Task_Name
   ```
   - Replace spaces with underscores in URL
   - Example: "Almost prime" → `https://rosettacode.org/wiki/Almost_prime`

3. **Add entry** using this format:
   ```markdown
   #### Task name
   **Problem:** One-line description of what to solve.

   **Requirements:**
   - Bullet points of what the program must do
   - Input/output specifications
   - Any constraints or rules

   **Success Criteria:**
   - Expected outputs or test cases
   - How to verify correctness

   ---
   ```

4. **Replace the bullet point** (`- Task name`) with the full entry

## Format Guidelines

- Keep descriptions concise
- Include concrete test cases when available
- Use code blocks for expected output or formulas
- Add `---` separator after each entry

## Progress

Completed sections:
- 0-9 (all 11 tasks)
- A (partial - through "Animate a pendulum")

Next task to add: **Animation**

## Example Entry

```markdown
#### Factorial
**Problem:** Compute the factorial of a non-negative integer.

**Requirements:**
- Input: non-negative integer n
- Output: n! = n × (n-1) × ... × 1
- 0! = 1 by definition

**Success Criteria:**
- factorial(0) = 1
- factorial(5) = 120
- factorial(10) = 3628800

---
```
