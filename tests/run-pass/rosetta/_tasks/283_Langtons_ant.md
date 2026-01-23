# Langton's ant

**Problem:** Langton's ant is a cellular automaton that models an ant sitting on a plane of cells, all of which are white initially, the ant facing in one of four directions. Each cell can either be black or white. The ant moves according to the color of the cell it is currently sitting in, with the following rules: This rather simple ruleset leads to an initially chaotic movement pattern, and after about 10000 steps, a cycle appears where the ant moves steadily away from the starting location in a diagonal 

**Requirements:**
- If the cell is black, it changes to white and the ant turns left;
- If the cell is white, it changes to black and the ant turns right;
- The ant then moves forward to the next cell, and repeat from step 1.
- Conway's Game of Life.
- Elementary cellular automaton

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
