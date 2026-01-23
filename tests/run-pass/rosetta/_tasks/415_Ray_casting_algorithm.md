# Ray-casting algorithm

**Problem:** Given a point and a polygon, check if the point is inside or outside the polygon using the ray-casting algorithm. A pseudocode can be simply: foreach side in polygon: if ray_intersects_segment(P,side) then count ← count + 1 if is_odd(count) then return inside return outside Where the function ray_intersects_segment return true if the horizontal ray starting from the point P intersects the side (segment), false otherwise.

**Requirements:**
- Implement the task according to the specification

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
