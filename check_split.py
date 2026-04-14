import glob
import re

for file_path in glob.glob('plans/bug-tracker/section-*.md'):
    with open(file_path, 'r') as f:
        lines = f.readlines()
    
    for i, line in enumerate(lines):
        if re.match(r'^\s*(Repro|Subsystem|Found|Note):', line):
            # Check previous lines to see if it's within a bullet point
            # We look backwards for the start of the bullet point
            found_header = False
            for j in range(i-1, -1, -1):
                prev_line = lines[j]
                if re.match(r'^\s*$', prev_line):
                    # Blank line
                    continue
                if re.match(r'^-\s+\[[ x]\]', prev_line):
                    found_header = True
                    break
                if not prev_line.startswith(' ') and not prev_line.startswith('\t'):
                    # Found a non-indented line that is not a bullet point
                    break
            if not found_header:
                print(f"Orphaned entry found in {file_path}:{i+1}")
                print(f"Line: {line.strip()}")
                print(f"Prev line: {lines[i-1].strip() if i > 0 else ''}")
                print("---")
