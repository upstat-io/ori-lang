import os
import re
import glob

# regex to find links like `file.md` or `file.md#section`
# Also look for phrases like `file.md §1`
link_re = re.compile(r'\[.*?\]\((.*?\.md)(#.*?)?\)')
ref_re = re.compile(r'([a-zA-Z0-9_-]+\.md)\s*(§[a-zA-Z0-9_-]+)?')

files = glob.glob('.claude/rules/*.md')
files.append('CLAUDE.md')

valid_files = set(os.path.basename(f) for f in files)

for filepath in files:
    with open(filepath, 'r') as f:
        content = f.read()
        
        # Check standard markdown links
        for match in link_re.finditer(content):
            target_file = os.path.basename(match.group(1))
            if target_file not in valid_files and not target_file.startswith('http'):
                print(f"{filepath}: broken link to {target_file}")
                
        # Check text references
        for match in ref_re.finditer(content):
            target_file = match.group(1)
            if target_file not in valid_files and target_file != 'CLAUDE.md':
                # filter out some false positives if any
                if "README" not in target_file and "SKILL" not in target_file and "envelope-format" not in target_file and "findings-schema" not in target_file:
                    print(f"{filepath}: potential broken ref to {target_file}")

