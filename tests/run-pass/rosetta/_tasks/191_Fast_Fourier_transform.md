# Fast Fourier transform

**Problem:** Calculate the FFT (Fast Fourier Transform) of an input sequence. The most general case allows for complex numbers at the input and results in a sequence of equal length, again of complex numbers. If you need to restrict yourself to real numbers, the output should be the magnitude (i.e.: sqrt(re2 + im2)) of the complex result. The classic version is the recursive Cooley–Tukey FFT. Wikipedia has pseudo-code for that. Further optimizations are possible but not required.

**Requirements:**
- If you need to restrict yourself to real numbers, the output should be the magnitude (i.e.: sqrt(re2 + im2)) of the complex result.
- Further optimizations are possible but not required.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
