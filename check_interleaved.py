import glob
import re

for file_path in glob.glob('plans/bug-tracker/section-*.md'):
    with open(file_path, 'r') as f:
        lines = f.readlines()
    
    current_bug = None
    for i, line in enumerate(lines):
        match_bug = re.match(r'^-\s+\[[ x]\]\s+`\[([^\]]+)\]', line)
        if match_bug:
            current_bug = match_bug.group(1)
        
        match_field = re.match(r'^\s*(Repro|Subsystem|Found|Note):\s*(.*)', line)
        if match_field:
            # Does this field mention a bug ID? (Sometimes they do, sometimes they don't, but we can check if it feels like it belongs to current_bug)
            # Actually, just print the current bug and the field to manually review.
            pass

        # Let's detect if there's a field immediately after a bug header that seems unrelated.
        # But wait, if BUG-04-079 was inserted BETWEEN BUG-04-069 header and body, the body would appear under BUG-04-079!
        # Let's just print all bug headers and their fields to see if any fields appear orphaned.
        
