# Abelian sandpile model

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
