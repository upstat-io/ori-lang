import os
import re
import glob

rules_dir = ".claude/rules"
files = glob.glob(f"{rules_dir}/*.md")
content = {}
for f in files:
    with open(f, "r") as fh:
        content[os.path.basename(f)] = fh.read()

def normalize_header(h):
    # remove markdown formatting and non-alphanumeric chars for comparison
    h = re.sub(r'[*`_]', '', h)
    return h.strip()

headers = {}
for fname, text in content.items():
    headers[fname] = []
    # Find headers
    for match in re.finditer(r'^#+\s+(.*)', text, re.M):
        headers[fname].append(normalize_header(match.group(1)))
    # Find explicit anchors like §AB-1
    for match in re.finditer(r'§([A-Z0-9-]+)', text):
        headers[fname].append("§" + match.group(1))

# Check for broken references
for fname, text in content.items():
    # look for references like "file.md §Section"
    refs = re.findall(r'([a-z-]+\.md)(?:[\s,]*§([A-Za-z0-9-\.]+))?', text)
    for ref_file, ref_sec in set(refs):
        if ref_file not in content:
            if ref_file not in ["CLAUDE.md", "operator-rules.md", "grammar.ebnf", "versioning.md"]:
                pass # print(f"FILE REF MIGHT BE BROKEN in {fname}: {ref_file}")
        elif ref_sec:
            # try to find ref_sec in the headers of ref_file
            found = False
            # direct exact match
            if ref_sec in content[ref_file]:
                found = True
            else:
                for h in headers[ref_file]:
                    if ref_sec in h or h.startswith(ref_sec):
                        found = True
                        break
            if not found:
                print(f"BROKEN SEC REF in {fname}: {ref_file} §{ref_sec}")

print("Checking finished")
