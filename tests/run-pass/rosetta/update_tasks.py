#!/usr/bin/env python3
"""
Script to fetch Rosetta Code task details and update markdown files.

Usage:
    python update_tasks.py [start_num] [end_num] [--force] [--dry-run]
"""

import json
import re
import sys
import time
import urllib.request
import urllib.parse
from pathlib import Path

TASKS_DIR = Path(__file__).parent / "_tasks"
API_BASE = "https://rosettacode.org/w/api.php"


def fetch_wiki_text(title: str) -> str | None:
    """Fetch raw wiki text using MediaWiki API."""
    params = {
        "action": "query",
        "titles": title,
        "prop": "revisions",
        "rvprop": "content",
        "rvslots": "main",
        "format": "json",
    }

    url = API_BASE + "?" + urllib.parse.urlencode(params)

    try:
        req = urllib.request.Request(
            url,
            headers={"User-Agent": "Mozilla/5.0 (compatible; RosettaCodeFetcher/1.0)"}
        )
        with urllib.request.urlopen(req, timeout=30) as response:
            data = json.loads(response.read().decode("utf-8"))

        pages = data.get("query", {}).get("pages", {})
        for page_id, page_data in pages.items():
            if page_id != "-1":
                revisions = page_data.get("revisions", [])
                if revisions:
                    return revisions[0].get("slots", {}).get("main", {}).get("*", "")
        return None
    except Exception as e:
        print(f"  API error: {e}")
        return None


def clean_wiki_text(text: str) -> str:
    """Clean wiki markup from text."""
    # Remove templates like {{...}}
    text = re.sub(r'\{\{[^}]*\}\}', '', text)
    # Remove wiki links, keep text: [[link|text]] -> text, [[link]] -> link
    text = re.sub(r'\[\[(?:[^|\]]*\|)?([^\]]+)\]\]', r'\1', text)
    # Remove external links [url text] -> text
    text = re.sub(r'\[https?://[^\s\]]+ ([^\]]+)\]', r'\1', text)
    text = re.sub(r'\[https?://[^\]]+\]', '', text)
    # Remove bold/italic
    text = re.sub(r"'{2,}", '', text)
    # Remove HTML tags
    text = re.sub(r'<[^>]+>', '', text)
    # Remove math tags content
    text = re.sub(r'<math>.*?</math>', '', text, flags=re.DOTALL)
    # Remove LaTeX-style math
    text = re.sub(r'\\mathit\{([^}]*)\}', r'\1', text)
    text = re.sub(r'\\[a-z]+\{[^}]*\}', '', text)
    text = re.sub(r'\\[a-z]+', '', text)
    # Remove wiki definition markers
    text = re.sub(r'^;+\s*', '', text, flags=re.MULTILINE)
    text = re.sub(r'^:+\s*', '', text, flags=re.MULTILINE)
    # Remove category tags
    text = re.sub(r'Category:[^\n]+', '', text)
    # Remove &nbsp; and similar
    text = re.sub(r'&[a-z]+;', ' ', text)
    # Clean whitespace
    text = re.sub(r'\n{3,}', '\n\n', text)
    text = re.sub(r' +', ' ', text)
    return text.strip()


def extract_task_section(wiki_text: str) -> str:
    """Extract just the task description section (before language implementations)."""
    # Task content is before the first ==Language== header
    # Look for common section markers
    patterns = [
        r'^==\s*\w',  # ==SomeLanguage or ==See also
        r'^=={{header',  # =={{header|Language}}
    ]

    lines = wiki_text.split('\n')
    task_lines = []

    for line in lines:
        # Stop at first language header or see also
        if re.match(r'^==', line):
            break
        task_lines.append(line)

    return '\n'.join(task_lines)


def parse_task_content(task_text: str) -> dict:
    """Parse task text to extract problem, requirements, and test cases."""
    cleaned = clean_wiki_text(task_text)
    lines = cleaned.split('\n')

    problem_parts = []
    requirements = []
    test_cases = []

    for line in lines:
        line = line.strip()
        if not line or line.lower() in ('task', 'task:', 'related task:'):
            continue

        # Skip section headers
        if line.startswith('='):
            continue

        # Detect list items (wiki uses * and #)
        if line.startswith('*') or line.startswith('#'):
            item = line.lstrip('*#:').strip()
            if item and len(item) > 5:
                # Skip "See also" style links and references
                if re.search(r'^(Wikipedia|OEIS|See also|Related)', item, re.I):
                    continue
                if re.search(r'\(.*\)$', item) and len(item) < 50:  # Likely a reference
                    continue
                # Check if it looks like a test case
                if re.search(r'[→=]>|should|expect|result|output|returns?', item, re.I):
                    test_cases.append(item)
                elif re.search(r'^\d+\s*[+\-*/×÷]|^\(?\d+,\s*\d+\)?.*[=→]', item):
                    test_cases.append(item)
                else:
                    requirements.append(item)
        else:
            # Regular paragraph - part of problem description
            if len(' '.join(problem_parts)) < 500 and len(line) > 10:
                problem_parts.append(line)

    # Build problem statement
    problem = ' '.join(problem_parts)

    # Clean up problem text
    problem = re.sub(r'\s+', ' ', problem).strip()

    # Truncate if too long
    if len(problem) > 500:
        problem = problem[:500]
        last_period = problem.rfind('.')
        if last_period > 250:
            problem = problem[:last_period + 1]

    # If we didn't find bullet requirements, try to extract from problem
    if not requirements and problem:
        # Look for sentences that sound like requirements
        sentences = re.split(r'(?<=[.!])\s+', problem)
        for s in sentences[1:]:  # Skip first sentence (usually the main description)
            if re.search(r'must|should|need|require|implement|write|create|show|display|demonstrate', s, re.I):
                requirements.append(s)

    return {
        "problem": problem or "Implement the task as described.",
        "requirements": requirements[:10],
        "test_cases": test_cases[:8],
    }


def format_markdown(task_name: str, info: dict) -> str:
    """Format task info as markdown."""
    lines = [f"# {task_name}", ""]

    problem = info.get("problem", "Implement the task as described.")
    lines.append(f"**Problem:** {problem}")
    lines.append("")

    lines.append("**Requirements:**")
    requirements = info.get("requirements", [])
    if requirements:
        for req in requirements:
            lines.append(f"- {req}")
    else:
        lines.append("- Implement the task according to the specification")
    lines.append("")

    lines.append("**Success Criteria:**")
    test_cases = info.get("test_cases", [])
    if test_cases:
        for tc in test_cases:
            lines.append(f"- {tc}")
    else:
        # Generate generic criteria based on problem
        lines.append("- Program produces correct output for test cases")
        lines.append("- Implementation matches Rosetta Code specification")
    lines.append("")

    return "\n".join(lines)


def is_file_incomplete(filepath: Path) -> bool:
    """Check if a task file needs updating."""
    try:
        content = filepath.read_text()
        # Consider incomplete if very short or has placeholder text
        if len(content.strip()) < 100:
            return True
        if "Complete the task as specified on Rosetta Code" in content:
            return True
        if "**Problem:**" not in content:
            return True
        return False
    except Exception:
        return True


def get_task_name_from_file(filepath: Path) -> str:
    """Extract task name from the file's first heading."""
    try:
        content = filepath.read_text()
        match = re.match(r"#\s*(.+)", content)
        if match:
            return match.group(1).strip()
    except Exception:
        pass
    name = filepath.stem
    name = re.sub(r"^\d+_", "", name)
    return name.replace("_", " ")


def process_file(filepath: Path, dry_run: bool = False) -> bool:
    """Process a single task file."""
    task_name = get_task_name_from_file(filepath)
    print(f"Processing: {filepath.name}")

    # Try different title variations
    titles = [
        task_name.replace(" ", "_"),
        task_name.replace(" ", "_").replace("-", "_"),
        task_name.replace("-", " ").replace(" ", "_"),
    ]

    wiki_text = None
    for title in titles:
        wiki_text = fetch_wiki_text(title)
        if wiki_text and len(wiki_text) > 100:
            break
        time.sleep(0.2)

    if not wiki_text:
        print(f"  Could not fetch: {task_name}")
        info = {"problem": f"Implement the {task_name} task."}
    else:
        task_section = extract_task_section(wiki_text)
        info = parse_task_content(task_section)
        print(f"  Found: {len(info.get('requirements', []))} reqs, {len(info.get('test_cases', []))} tests")

    markdown = format_markdown(task_name, info)

    if dry_run:
        print(f"---\n{markdown}\n---")
    else:
        filepath.write_text(markdown)
        print(f"  Updated")

    return True


def main():
    args = sys.argv[1:]

    nums = [a for a in args if a.isdigit()]
    start_num = int(nums[0]) if len(nums) > 0 else None
    end_num = int(nums[1]) if len(nums) > 1 else None
    dry_run = "--dry-run" in args
    force = "--force" in args

    task_files = sorted(TASKS_DIR.glob("*.md"))

    # Filter by number range
    if start_num is not None:
        filtered = []
        for f in task_files:
            match = re.match(r"(\d+)_", f.name)
            if match:
                num = int(match.group(1))
                if start_num <= num <= (end_num or 9999):
                    filtered.append(f)
        task_files = filtered

    # Filter to incomplete unless --force
    if not force:
        task_files = [f for f in task_files if is_file_incomplete(f)]

    print(f"Processing {len(task_files)} files")
    if dry_run:
        print("(DRY RUN)")
    print()

    success = 0
    for filepath in task_files:
        try:
            if process_file(filepath, dry_run):
                success += 1
        except Exception as e:
            print(f"  Error: {e}")
        time.sleep(0.5)

    print(f"\nDone: {success}/{len(task_files)}")


if __name__ == "__main__":
    main()
