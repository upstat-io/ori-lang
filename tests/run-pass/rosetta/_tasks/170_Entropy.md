# Entropy

**Problem:** Task Calculate the Shannon entropy H of a given input string. Given the discrete random variable X } that is a string of N } "symbols" (total characters) consisting of n } different characters (n=2 for binary), the Shannon entropy of X in bits/symbol is : H 2 ( X ) = − ∑ i = 1 n c o u n t i N log 2 ⁡ ( c o u n t i N ) (X)=- { } ({ })}} where c o u n t i }} is the count of character n i }} .

**Requirements:**
- For this task, use X="1223334444" as an example. The result should be 1.84644... bits/symbol. This assumes X was a random variable, which may not be the case, or it may depend on the observer.
- where N=number of molecules. Boltzmann's H is the same equation as Shannon's H, and it gives the specific entropy H on a "per molecule" basis.
- The "total", "absolute", or "extensive" information entropy is
- bits of entropy. The total entropy in bits of the example above is S= 10*18.4644 = 18.4644 bits.
- Two other "entropies" are useful:
- Normalized specific entropy:
- which varies from 0 to 1 and it has units of "entropy/symbol" or just 1/symbol. For this example, Hn<>= 0.923.
- Normalized total (extensive) entropy:

**Success Criteria:**
- Task completed according to Rosetta Code specification
