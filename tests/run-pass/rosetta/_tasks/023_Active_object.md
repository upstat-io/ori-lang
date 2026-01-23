# Active object

**Problem:** Implement an active integrator object that continuously updates using trapezoid integration.

**Requirements:**
- Object continuously integrates: `S = S + (K(t₁) + K(t₀)) × (t₁ - t₀) / 2`
- Methods: `Input(K)` to set input function, `Output()` to get accumulated value
- Test sequence:
  1. Set input to `sin(2πft)` where f = 0.5 Hz
  2. Wait 2 seconds
  3. Set input to 0
  4. Wait 0.5 seconds
  5. Verify output ≈ 0

**Success Criteria:**
- Output approximately 0 (within ±0.001)
- Integrating full sine cycle yields zero
