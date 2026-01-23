# Dragon curve

**Problem:** Create and display a dragon curve fractal. (You may either display the curve directly or write it to an image file.) Algorithms Here are some brief notes the algorithms used and how they might suit various languages. Recursively a right curling dragon is a right dragon followed by a left dragon, at 90-degree angle.

**Requirements:**
- e. And a left dragon is a left followed by a right.
- *---R----* expands to * *
- *---L---* expands to * *
- The co-routines dcl and dcr in various examples do this recursively to a desired expansion level.
- The curl direction right or left can be a parameter instead of two separate routines.
- Recursively, a curl direction can be eliminated by noting the dragon consists of two copies of itself drawn towards a central point at 45-degrees.
- *------->* becomes * * Recursive copies drawn
- / from the ends towards

**Success Criteria:**
- Task completed according to Rosetta Code specification
