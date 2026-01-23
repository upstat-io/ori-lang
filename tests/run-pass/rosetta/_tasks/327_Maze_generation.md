# Maze generation

**Problem:** 350px||right|a maze Generate and show a maze, using the simple Depth-first search algorithm. Related tasks

**Requirements:**
- Start at a random cell.
- Mark the current cell as visited, and get a list of its neighbors. For each neighbor, starting with a randomly selected neighbor:
- If that neighbor hasn't been visited, remove the wall between this cell and that neighbor, and then recurse with that neighbor as the current cell.
- Maze solving.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
