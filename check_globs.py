import glob
import yaml
import os
from pathlib import Path

rules_dir = Path('.claude/rules')
for md_file in rules_dir.glob('*.md'):
    content = md_file.read_text()
    if content.startswith('---'):
        end_idx = content.find('---', 3)
        if end_idx != -1:
            frontmatter = content[3:end_idx]
            try:
                data = yaml.safe_load(frontmatter)
                paths = data.get('paths', [])
                for p in paths:
                    # check if glob matches anything
                    # Note: python's glob.glob with recursive=True
                    # doesn't handle ** exactly like gitignore, but close enough.
                    # We can use pathlib's rglob if we replace ** appropriately.
                    # Let's just use a quick check:
                    matches = list(Path('.').glob(p))
                    if not matches:
                        # Try ignoring leading **/
                        if p.startswith('**/'):
                            matches = list(Path('.').rglob(p[3:]))
                        elif '**' in p:
                            parts = p.split('**')
                            # Simple approximation for finding dead triggers
                            if parts[0]:
                                d = Path(parts[0])
                                if not d.exists():
                                    print(f"Dead trigger in {md_file.name}: {p} (dir {d} does not exist)")
                                    continue
                            # Just use glob.glob
                            matches = glob.glob(p, recursive=True)
                            
                    if not matches:
                        # fall back to bash glob
                        import subprocess
                        res = subprocess.run(f"shopt -s globstar; ls -d {p} 2>/dev/null | head -n 1", shell=True, capture_output=True, text=True)
                        if not res.stdout.strip():
                            print(f"Dead trigger in {md_file.name}: {p}")
            except Exception as e:
                print(f"Error parsing {md_file}: {e}")
