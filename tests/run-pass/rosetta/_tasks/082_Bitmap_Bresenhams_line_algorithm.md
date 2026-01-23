# Bitmap/Bresenham's line algorithm

**Problem:** Draw lines on a bitmap using Bresenham's line algorithm.

**Requirements:**
- Use bitmap data structure from Bitmap task
- Accept two endpoints (x0, y0) and (x1, y1)
- Draw pixels along the line connecting the points
- Support arbitrary line directions and slopes
- Use integer-only arithmetic with error accumulation

**Success Criteria:**
- Lines render without gaps or artifacts
- Works for all octants (any direction)
- Can draw shapes like diamonds by connecting multiple lines
