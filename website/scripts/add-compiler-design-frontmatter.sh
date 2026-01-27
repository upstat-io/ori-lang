#!/bin/bash
# Add frontmatter to compiler design docs

DESIGN_DIR="../docs/compiler/design"

# Section names for display
declare -A SECTION_NAMES=(
    ["01-architecture"]="Architecture"
    ["02-intermediate-representation"]="Intermediate Representation"
    ["03-lexer"]="Lexer"
    ["04-parser"]="Parser"
    ["05-type-system"]="Type System"
    ["06-pattern-system"]="Pattern System"
    ["07-evaluator"]="Evaluator"
    ["08-diagnostics"]="Diagnostics"
    ["09-testing"]="Testing"
    ["appendices"]="Appendices"
)

add_frontmatter() {
    local file="$1"
    local section="$2"
    local order="$3"

    # Check if frontmatter already exists
    if head -1 "$file" | grep -q '^---'; then
        echo "Skipping $file (already has frontmatter)"
        return
    fi

    # Get title from first heading
    local title=$(grep -m1 '^# ' "$file" | sed 's/^# //')

    echo "Adding frontmatter to $file (section: $section, order: $order)"

    # Create temp file with frontmatter
    {
        echo "---"
        echo "title: \"$title\""
        echo "description: \"Ori Compiler Design — $title\""
        echo "order: $order"
        if [ -n "$section" ]; then
            echo "section: \"$section\""
        fi
        echo "---"
        echo ""
        cat "$file"
    } > "$file.tmp"

    mv "$file.tmp" "$file"
}

# Add frontmatter to main index
add_frontmatter "$DESIGN_DIR/index.md" "" 0

# Process each section directory
section_order=1
for section_dir in "$DESIGN_DIR"/[0-9][0-9]-* "$DESIGN_DIR"/appendices; do
    if [ -d "$section_dir" ]; then
        section_name=$(basename "$section_dir")
        display_name="${SECTION_NAMES[$section_name]:-$section_name}"

        # Section index gets base order
        if [ -f "$section_dir/index.md" ]; then
            add_frontmatter "$section_dir/index.md" "$display_name" $((section_order * 100))
        fi

        # Sub-pages get incremented order
        page_order=1
        for page in "$section_dir"/*.md; do
            if [ -f "$page" ] && [ "$(basename "$page")" != "index.md" ]; then
                add_frontmatter "$page" "$display_name" $((section_order * 100 + page_order))
                ((page_order++))
            fi
        done

        ((section_order++))
    fi
done

echo ""
echo "Done!"
