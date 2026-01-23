# Angle difference between two bearings

**Problem:** Calculate the difference between two compass bearings, normalized to [-180, +180].

**Requirements:**
- Input: two bearings (degrees)
- Output: b2 - b1, normalized to -180 to +180 range
- Handle circular arithmetic (360° = full rotation)

**Test Cases:**
- (20, 45), (-45, 45), (-85, 90), (-95, 90)
- (-45, 125), (-45, 145), (29.4803, -88.6381)

**Success Criteria:**
- Results always in [-180, 180] range
- Bonus: handle large values outside standard range
