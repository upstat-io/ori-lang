# IBAN

**Problem:** The International Bank Account Number (IBAN) is an internationally agreed means of identifying bank accounts across national borders with a reduced risk of propagating transcription errors. The IBAN consists of up to 34 alphanumeric characters: The check digits enable a sanity check of the bank account number to confirm its integrity even before submitting a transaction.

**Requirements:**
- first the two-letter ISO 3166-1 alpha-2 country code,
- then two check digits, and
- finally a country-specific Basic Bank Account Number (BBAN).

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
