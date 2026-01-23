# Quaternion

**Problem:** Quaternions are an extension of the idea of complex numbers. A complex number has a real and complex part, sometimes written as a + bi, where a and b stand for real numbers, and i stands for the square root of minus 1. An example of a complex number might be -3 + 2i, where the real part, a is -3.0 and the complex part, b is +2.0. A quaternion has one real part and three imaginary parts, i, j, and k. A quaternion might be written as a + bi + cj + dk.

**Requirements:**
- i∙i = j∙j = k∙k = i∙j∙k = -1, or more simply,
- ii = jj = kk = ijk = -1.
- The norm of a quaternion: =
- Addition of a real number r and a quaternion q: r + q = q + r = (a+r, b, c, d)
- Addition of two quaternions: q1 + q2 = (a1+a2, b1+b2, c1+c2, d1+d2)
- Multiplication of a real number and a quaternion: qr = rq = (ar, br, cr, dr)
- Multiplication of two quaternions q1 and q2 is given by: ( a1a2 − b1b2 − c1c2 − d1d2, a1b2 + b1a2 + c1d2 − d1c2, a1c2 − b1d2 + c1a2 + d1b2, a1d2 + b1c2 − c1b2 + d1a2 )
- Show that, for the two quaternions q1 and q2: q1q2 q2q1
- On Infinitesimal rotation matrix relationship to skew-symmetric matrices;
- rigidgeometricalgebra.org/Quaternion

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
