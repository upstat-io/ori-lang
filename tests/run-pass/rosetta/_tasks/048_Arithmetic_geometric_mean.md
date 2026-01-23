# Arithmetic-geometric mean

**Problem:** Compute the arithmetic-geometric mean (AGM) of two numbers through iterative convergence.

**Requirements:**
- Implement the AGM algorithm with iterations:
  - a_{n+1} = (a_n + g_n) / 2 (arithmetic mean)
  - g_{n+1} = sqrt(a_n × g_n) (geometric mean)
- Continue until the difference between a_n and g_n is negligibly small
- Use an appropriate tolerance threshold (e.g., 1e-10 to 1e-16)

**Success Criteria:**
- agm(1, 1/sqrt(2)) ≈ 0.8472130847939792
- Algorithm exhibits quadratic convergence
