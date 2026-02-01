#!/bin/bash
# Generate changelog.json from git log during build
# Filters to meaningful commits and extracts conventional commit info

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEBSITE_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(dirname "$WEBSITE_DIR")"
OUTPUT="$WEBSITE_DIR/public/changelog.json"

# How many meaningful commits to show
LIMIT=100

cd "$REPO_ROOT"

echo "Generating changelog from git log..."

# Start JSON array
echo "[" > "$OUTPUT"

count=0
first=true

# Read commits line by line
while IFS='|' read -r full_hash date subject; do
    # Stop at limit
    if [ "$count" -ge "$LIMIT" ]; then
        break
    fi

    hash="${full_hash:0:7}"

    # Skip merge commits
    case "$subject" in
        Merge*) continue ;;
    esac

    # Extract type using sed
    # Pattern: type(scope): message or type: message
    if echo "$subject" | grep -qE '^[a-z]+(\([^)]+\))?!?: '; then
        type=$(echo "$subject" | sed -E 's/^([a-z]+)(\([^)]+\))?!?: .*/\1/')
        scope=$(echo "$subject" | sed -E 's/^[a-z]+(\(([^)]+)\))?!?: .*/\2/')
        message=$(echo "$subject" | sed -E 's/^[a-z]+(\([^)]+\))?!?: //')
    else
        # Not a conventional commit, skip
        continue
    fi

    # Only keep meaningful types (whitelist approach)
    case "$type" in
        feat|fix|refactor|perf|docs)
            # Keep these
            ;;
        *)
            # Skip everything else (chore, style, ci, build, test, and non-standard)
            continue
            ;;
    esac

    # Skip "approve X" docs commits (proposal folder moves)
    if [[ "$type" == "docs" ]]; then
        lower_msg=$(echo "$message" | tr '[:upper:]' '[:lower:]')
        case "$lower_msg" in
            approve*) continue ;;
        esac
    fi

    # Escape for JSON
    message=$(echo "$message" | sed 's/\\/\\\\/g; s/"/\\"/g')

    # Add comma before entries (except first)
    if [ "$first" = true ]; then
        first=false
    else
        echo "," >> "$OUTPUT"
    fi

    # Write entry
    if [ -n "$scope" ]; then
        printf '  {"date": "%s", "type": "%s", "scope": "%s", "message": "%s", "hash": "%s"}' \
            "$date" "$type" "$scope" "$message" "$hash" >> "$OUTPUT"
    else
        printf '  {"date": "%s", "type": "%s", "message": "%s", "hash": "%s"}' \
            "$date" "$type" "$message" "$hash" >> "$OUTPUT"
    fi

    count=$((count + 1))
done < <(git log --pretty=format:'%H|%ad|%s' --date=short -n 500)

# Close JSON array
echo "" >> "$OUTPUT"
echo "]" >> "$OUTPUT"

echo "Generated changelog with $count entries -> $OUTPUT"
