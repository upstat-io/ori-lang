# Associative array/Merging

**Problem:** Merge two associative arrays where update values take precedence.

**Requirements:**
- Create base array: {"name": "Rocket Skates", "price": 12.75, "color": "yellow"}
- Create update array: {"price": 15.25, "color": "red", "year": 1974}
- Merge arrays with update values overriding base values
- Do not mutate the original arrays if possible

**Success Criteria:**
- Result contains: {"name": "Rocket Skates", "price": 15.25, "color": "red", "year": 1974}
- Keys only in base are preserved
- Keys only in update are added
- Conflicting keys use update value
