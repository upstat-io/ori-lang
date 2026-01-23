# Knuth's algorithm S

**Problem:** This is a method of randomly sampling n items from a set of M items, with equal probability; where M >= n and M, the number of items is unknown until the end. This means that the equal probability sampling should be maintained for all successive items > n as they become available (although the content of successive samples can change). The algorithm: Note: A class taking n and generating a callable instance/function might also be used. Related tasks:

**Requirements:**
- Select the first n items as the sample as they become available;
- For the i-th item where i > n, have a random chance of n/i of keeping it. If failing this chance, the sample remains the same. If not, have it randomly (1/n) replace one of the previously selected n items of the sample.
- Repeat 2nd step for any subsequent items.
- Test your functions by printing and showing the frequency of occurrences of the selected digits from 100,000 repetitions of:
- Use the s_of_n_creator with n == 3 to generate an s_of_n.
- The Art of Computer Programming, Vol 2, 3.4.2 p.142
- One of n lines in a file
- Accumulator factory

**Success Criteria:**
- Create a function s_of_n_creator that given n the maximum sample size, returns a function s_of_n that takes one parameter, item.
- Function s_of_n when called with successive items returns an equi-weighted random sample of up to n of its items so far, each time it is called, calculated using Knuths Algorithm S.
- call s_of_n with each of the digits 0 to 9 in order, keeping the returned three digits of its random sampling from its last call with argument item=9.
