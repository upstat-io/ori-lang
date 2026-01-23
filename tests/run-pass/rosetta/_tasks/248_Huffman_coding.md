# Huffman coding

**Problem:** Huffman encoding is a way to assign binary codes to symbols that reduces the overall number of bits used to encode a typical string of those symbols. For example, if you use letters as symbols and have details of the frequency of occurrence of those letters in typical strings, then you could just encode each letter with a fixed number of bits, such as in ASCII codes.

**Requirements:**
- Create a leaf node for each symbol and add it to the priority queue.
- While there is more than one node in the queue:
- Remove the node of highest priority (lowest probability) twice to get two nodes.
- Create a new internal node with these two nodes as children and with probability equal to the sum of the two nodes' probabilities.
- Add the new node to the queue.
- The remaining node is the root node and the tree is complete.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
