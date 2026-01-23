# Updating Rosetta Code Task Files

Instructions for filling in details for task files that only contain a heading.

## Background

Task files in `_tasks/` are numbered `001_Task_Name.md` through `597_Task_Name.md`.
- Files 001-037 have full details (Problem, Requirements, Success Criteria)
- Files 038-597 only have the task name as a heading and need to be filled in

## Process

1. **Find an incomplete task file** in `_tasks/` (any file with just a `# Task Name` heading)

2. **Fetch requirements** from Rosetta Code:
   ```
   https://rosettacode.org/wiki/Task_Name
   ```
   - Replace spaces with underscores in URL
   - Example: "Almost prime" → `https://rosettacode.org/wiki/Almost_prime`

3. **Update the file** using this format:
   ```markdown
   # Task Name

   **Problem:** One-line description of what to solve.

   **Requirements:**
   - Bullet points of what the program must do
   - Input/output specifications
   - Any constraints or rules

   **Success Criteria:**
   - Expected outputs or test cases
   - How to verify correctness
   ```

## Format Guidelines

- Keep descriptions concise
- Include concrete test cases when available
- Use code blocks for expected output or formulas
- Focus on what the task requires, not implementation details

## Example

Before (incomplete):
```markdown
# Factorial
```

After (complete):
```markdown
# Factorial

**Problem:** Compute the factorial of a non-negative integer.

**Requirements:**
- Input: non-negative integer n
- Output: n! = n × (n-1) × ... × 1
- 0! = 1 by definition

**Success Criteria:**
- factorial(0) = 1
- factorial(5) = 120
- factorial(10) = 3628800
```

## Progress Tracking

Files needing details: 038-597 (560 files)

To find incomplete files:
```bash
# List files with only a heading (small file size)
find _tasks -name "*.md" -size -50c
```
