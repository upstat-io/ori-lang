# ASCII art diagram converter

**Problem:** Parse an ASCII art diagram to extract bit-field metadata and decode binary data.

**Requirements:**
- Parse ASCII tables using +, -, |, and whitespace characters
- Support tables with 8, 16, 32, or 64 columns representing bits
- Extract field names and their bit widths from the diagram
- Calculate start/end bit positions for each field
- Decode binary data according to the extracted structure

**Success Criteria:**
- Parse RFC 1035 DNS message format diagram
- For test hex "78477bbf5496e12e1bf169a4", extract fields:
  - ID: 16 bits
  - QR, Opcode, AA, TC, RD, RA: varying bit widths
  - Z: 3 bits, RCODE: 4 bits
  - QDCOUNT, ANCOUNT, NSCOUNT, ARCOUNT: 16 bits each
