# Bifid cipher

**Problem:** Implement encryption and decryption for the Bifid cipher using a 5x5 Polybius square.

**Requirements:**
- Convert characters to x,y coordinates from Polybius square
- Write coordinates vertically (x-row, y-row), then read horizontally
- Divide into pairs and convert back using the square
- I and J share same position in standard square
- Ignore spaces; convert to uppercase
- Implement both encryption and decryption

**Success Criteria:**
- "ATTACKATDAWN" → "DQBDAXDQPDQH" (with standard square)
- "FLEEATONCE" → "UAEOLWRINS" (with Wikipedia square)
- Decryption reverses encryption correctly
