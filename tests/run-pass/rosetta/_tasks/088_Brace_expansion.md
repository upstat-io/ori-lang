# Brace expansion

**Problem:** Implement brace expansion as used in Unix shells.

**Requirements:**
- Parse balanced brace pairs containing commas as alternations
- Unescaped backslashes escape the following character
- Brace pairs without commas are literal
- Multiple alternations create Cartesian product
- Preserve all alternatives including duplicates and empty ones
- Maintain lexicographic ordering

**Success Criteria:**
- `~/{Downloads,Pictures}/*.{jpg,gif,png}` → 6 results
- `It{{em,alic}iz,erat}e{d,}, please.` → 6 results
- `enable_{audio,video}` → `enable_audio`, `enable_video`
