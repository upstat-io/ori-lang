# 2048

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
