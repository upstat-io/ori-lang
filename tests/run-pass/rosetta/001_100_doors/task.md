# 100 doors

**Problem:** Simulate toggling 100 doors through multiple passes and determine their final states.

**Requirements:**
- Start with 100 doors, all initially closed
- Make 100 passes:
  - Pass 1: Toggle every door
  - Pass 2: Toggle every 2nd door
  - Pass 3: Toggle every 3rd door
  - ...continue through Pass 100
- Toggle means: if closed, open it; if open, close it

**Success Criteria:**
- Output which doors remain open after all 100 passes
- Expected result: Doors 1, 4, 9, 16, 25, 36, 49, 64, 81, 100 (perfect squares)
