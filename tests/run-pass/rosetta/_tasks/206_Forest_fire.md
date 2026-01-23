# Forest fire

**Problem:** Implement the Drossel and Schwabl definition of the forest-fire model. It is basically a 2D cellular automaton where each cell can be in three distinct states (empty, tree and burning) and evolves according to the following rules (as given by Wikipedia) Neighborhood is the Moore neighborhood; boundary conditions are so that on the boundary the cells are always empty ("fixed" boundary condition).

**Requirements:**
- A burning cell turns into an empty cell
- A tree will burn if at least one neighbor is burning
- A tree ignites with probability f even if no neighbor is burning
- An empty space fills with a tree with probability p
- See Conway's Game of Life
- See Wireworld.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
