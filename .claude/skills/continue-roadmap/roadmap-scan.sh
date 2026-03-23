#!/usr/bin/env bash
# roadmap-scan.sh — Fast roadmap status scanner with blocker awareness
# Scans plans/roadmap/section-*.md files sequentially.
# Outputs: per-section status line + detail block for first incomplete section.
# Detects frontmatter/body mismatches at both section and subsection level.
# Tracks blocker dependencies via <!-- blocked-by:X --> tags.
set -euo pipefail

ROADMAP_DIR="${1:-plans/roadmap}"
FOCUS_SECTION="${2:-}"
first_incomplete=""

# ── Portable key-value store (bash 3 compatible) ──
# Uses temp directory: each "map" is a subdirectory, keys are filenames, values are file contents.
_kvdir=$(mktemp -d)
trap 'rm -rf "$_kvdir"' EXIT

_kv_init() { mkdir -p "$_kvdir/$1"; }
_kv_set() { printf '%s' "$3" > "$_kvdir/$1/$2"; }
_kv_get() { cat "$_kvdir/$1/$2" 2>/dev/null || printf '%s' "${3:-}"; }
_kv_keys() { local d="$_kvdir/$1"; if [[ -d "$d" ]]; then find "$d" -maxdepth 1 -type f -exec basename {} \; 2>/dev/null | sort; fi; }
_kv_len() { local d="$_kvdir/$1"; if [[ -d "$d" ]]; then find "$d" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' '; else echo 0; fi; }
_kv_inc() {
    local cur
    cur=$(_kv_get "$1" "$2" 0)
    _kv_set "$1" "$2" "$((cur + 1))"
}

# ── Reroute detection from plan frontmatter ──
# Scans plans/*/index.md for reroute/parallel plans with active/queued status
has_active_reroute=false
active_reroutes=()
queued_reroutes=()
for plan_index in plans/*/index.md; do
    [[ -f "$plan_index" ]] || continue
    plan_dir=$(dirname "$plan_index")
    plan_name=$(basename "$plan_dir")
    [[ "$plan_name" == "roadmap" ]] && continue

    # Parse frontmatter fields
    fm_type=$(awk '/^---$/{n++; next} n==1 && /^(reroute|parallel):/{sub(/^[a-z]+: */,""); print; exit}' "$plan_index")
    fm_status=$(awk '/^---$/{n++; next} n==1 && /^status:/{sub(/^status: */,""); print; exit}' "$plan_index")
    fm_full_name=$(awk '/^---$/{n++; next} n==1 && /^full_name:/{sub(/^full_name: */,""); gsub(/"/, ""); print; exit}' "$plan_index")
    fm_name=$(awk '/^---$/{n++; next} n==1 && /^name:/{sub(/^name: */,""); gsub(/"/, ""); print; exit}' "$plan_index")
    fm_is_reroute=$(awk '/^---$/{n++; next} n==1 && /^reroute:/{sub(/^reroute: */,""); print; exit}' "$plan_index")
    fm_order=$(awk '/^---$/{n++; next} n==1 && /^order:/{sub(/^order: */,""); print; exit}' "$plan_index")
    fm_order="${fm_order:-999}"

    display_name="${fm_full_name:-${fm_name:-$plan_name}}"

    if [[ "$fm_status" == "active" ]]; then
        # Count progress in plan sections
        plan_checked=$({ grep -r -c '\- \[x\]' "$plan_dir"/section-*.md 2>/dev/null || true; } | awk -F: '{s+=$NF} END{print s+0}')
        plan_unchecked=$({ grep -r -c '\- \[ \]' "$plan_dir"/section-*.md 2>/dev/null || true; } | awk -F: '{s+=$NF} END{print s+0}')
        plan_total=$((plan_checked + plan_unchecked))
        plan_pct=0
        [[ "$plan_total" -gt 0 ]] && plan_pct=$((plan_checked * 100 / plan_total))
        type_label="reroute"
        [[ "$fm_is_reroute" != "true" ]] && type_label="parallel"
        active_reroutes+=("${type_label}|${display_name}|${plan_dir}|${plan_checked}/${plan_total} (${plan_pct}%)")
        [[ "$fm_is_reroute" == "true" ]] && has_active_reroute=true
    elif [[ "$fm_status" == "queued" ]]; then
        type_label="reroute"
        [[ "$fm_is_reroute" != "true" ]] && type_label="parallel"
        queued_reroutes+=("${fm_order}|${type_label}|${display_name}|${plan_dir}")
    fi
done

# Display reroute status
if [[ ${#active_reroutes[@]} -gt 0 || ${#queued_reroutes[@]} -gt 0 ]]; then
    echo "=== REROUTES ==="
    for entry in "${active_reroutes[@]}"; do
        IFS='|' read -r rtype rname rdir rprog <<< "$entry"
        echo "[ACTIVE ${rtype}] ${rname} — ${rdir} — ${rprog}"
    done
    # Sort queued reroutes by order field (numeric, ascending)
    IFS=$'\n' sorted_queued=($(printf '%s\n' "${queued_reroutes[@]}" | sort -t'|' -k1 -n))
    unset IFS
    for entry in "${sorted_queued[@]}"; do
        IFS='|' read -r rorder rtype rname rdir <<< "$entry"
        echo "[queued ${rtype}] ${rname} — ${rdir} (order: ${rorder})"
    done
    echo ""
fi

# ── Helper: find section file by section number ──
find_section_file() {
    local sid="$1"
    if [[ "$sid" =~ ^[0-9]+$ ]]; then
        local padded
        padded=$(printf "%02d" "$sid")
        for candidate in "$ROADMAP_DIR"/section-${padded}-*.md "$ROADMAP_DIR"/section-${padded}.md; do
            if [[ -f "$candidate" ]]; then
                echo "$candidate"
                return
            fi
        done
    fi
    for candidate in "$ROADMAP_DIR"/section-${sid}-*.md "$ROADMAP_DIR"/section-${sid}.md; do
        if [[ -f "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    done
}

# ── Pre-parse dependency graph from overview ──
_kv_init dep_of
if [[ -f "$ROADMAP_DIR/00-overview.md" ]]; then
    while read -r child parent; do
        [[ -z "$child" ]] && continue
        _kv_set dep_of "$child" "$parent"
    done < <(awk '
        BEGIN { last_sec = ""; in_graph = 0; in_code = 0 }
        /^## Dependency Graph/ { in_graph = 1; next }
        in_graph && /^## / { exit }
        !in_graph { next }
        /^```/ { in_code = !in_code; next }
        !in_code { next }
        /^$/ { next }
        {
            line = $0
            is_cont = (line ~ /^[[:space:]]/)
            prev = ""
            if (is_cont && last_sec != "") prev = last_sec
            while (match(line, /Section [0-9]+/)) {
                sec = substr(line, RSTART + 8, RLENGTH - 8)
                if (prev != "" && sec != prev) {
                    printf "%s %s\n", sec, prev
                }
                prev = sec
                line = substr(line, RSTART + RLENGTH)
            }
            if (prev != "") last_sec = prev
        }
    ' "$ROADMAP_DIR/00-overview.md")
fi

for f in "$ROADMAP_DIR"/section-*.md; do
    # Extract top-level frontmatter fields (between first and second --- lines)
    status=$(awk '/^---$/{n++; next} n==1 && /^status:/{sub(/^status: */,""); print; exit}' "$f")
    title=$(awk '/^---$/{n++; next} n==1 && /^title:/{sub(/^title: */,""); print; exit}' "$f")
    section=$(awk '/^---$/{n++; next} n==1 && /^section:/{sub(/^section: */,""); print; exit}' "$f")

    # Count checkboxes in file body (after frontmatter)
    checked=$(grep -c '\- \[x\]' "$f" 2>/dev/null || true)
    unchecked=$(grep -c '\- \[ \]' "$f" 2>/dev/null || true)
    checked=${checked:-0}
    unchecked=${unchecked:-0}
    total=$((checked + unchecked))

    # Section-level frontmatter/body mismatch detection (both directions)
    mismatch=""
    if [[ "$status" == "complete" && "$unchecked" -gt 0 ]]; then
        mismatch=" !! MISMATCH: frontmatter=complete but ${unchecked} unchecked"
    elif [[ "$status" == "not-started" && "$checked" -gt 0 ]]; then
        mismatch=" !! MISMATCH: frontmatter=not-started but ${checked} checked"
    fi

    if [[ "$unchecked" -eq 0 ]]; then
        echo "[done] Section ${section}: ${title} (${checked}/${total})${mismatch}"
    else
        pct=0
        if [[ "$total" -gt 0 ]]; then
            pct=$((checked * 100 / total))
        fi
        echo "[open] Section ${section}: ${title} (${checked}/${total}, ${pct}%)${mismatch}"

        # Detail block for focused section (if specified) or first incomplete
        if [[ -n "$FOCUS_SECTION" && "${section//\"/}" == "$FOCUS_SECTION" ]] || \
           [[ -z "$FOCUS_SECTION" && -z "$first_incomplete" ]]; then
            first_incomplete="$f"
            echo ""
            echo "=== FOCUS: Section ${section} — ${title} ==="
            echo "File: $(basename "$f")"
            echo "Progress: ${checked}/${total} (${pct}%)"

            # ── Recently completed items ──
            echo ""
            echo "Recently completed:"
            recently=$(awk '
                /^---$/ { n++; next }
                n < 2 { next }
                /^- \[x\]/ { printf "  L%d: %s\n", NR, $0 }
            ' "$f" | tail -3)
            if [[ -n "$recently" ]]; then
                echo "$recently"
            else
                echo "  (none)"
            fi

            # ── Blocker extraction ──
            # Parse all - [ ] lines: line number, indent, effective blockers, content
            # Parent inheritance: indent-0 items set parent blocker,
            # indent>0 items inherit if no own blocker. Reset at ## boundaries.
            blocker_data=$(awk '
                BEGIN { n = 0; parent_bl = ""; cur_sub = "?" }
                /^---$/ { n++; next }
                n < 2 { next }
                /^## / {
                    parent_bl = ""
                    header = $0
                    sub(/^## /, "", header)
                    split(header, parts, " ")
                    cur_sub = parts[1]
                    gsub(/:$/, "", cur_sub)
                    next
                }
                /^###/ { parent_bl = ""; next }
                /\- \[ \]/ {
                    line = $0
                    indent = 0
                    while (substr(line, indent+1, 1) == " ") indent++
                    own = ""
                    rest = line
                    while (match(rest, /blocked-by:[0-9A-Za-z.]+/)) {
                        tag = substr(rest, RSTART + 11, RLENGTH - 11)
                        if (own != "") own = own ","
                        own = own tag
                        rest = substr(rest, RSTART + RLENGTH)
                    }
                    if (indent == 0) parent_bl = own
                    eff = own
                    if (indent > 0 && eff == "" && parent_bl != "") eff = parent_bl
                    if (eff == "") eff = "-"
                    printf "%d\t%d\t%s\t%s\t%s\n", NR, indent, eff, cur_sub, line
                }
            ' "$f")

            # Count blocked vs unblocked, collect blocker section IDs and affected subsections
            total_blocked=0
            total_unblocked=0
            _kv_init blocker_item_counts
            _kv_init blocker_subs
            while IFS=$'\t' read -r lineno indent blockers subsection content; do
                [[ -z "$lineno" ]] && continue
                if [[ "$blockers" != "-" ]]; then
                    total_blocked=$((total_blocked + 1))
                    IFS=',' read -ra bids <<< "$blockers"
                    for bid in "${bids[@]}"; do
                        bsec="${bid%%.*}"
                        _kv_inc blocker_item_counts "$bsec"
                        _kv_set blocker_subs "${bsec}:${subsection}" 1
                    done
                else
                    total_unblocked=$((total_unblocked + 1))
                fi
            done <<< "$blocker_data"

            num_blocker_sections=$(_kv_len blocker_item_counts)
            if [[ "$total_blocked" -gt 0 ]]; then
                echo "Actionable: ${total_unblocked} unblocked, ${total_blocked} blocked (by ${num_blocker_sections} sections)"
            fi
            echo ""

            # ── Subsection statuses with blocked counts ──
            # Pre-compute blocked count per subsection (## header)
            _kv_init sub_blocked_counts
            while IFS=$'\t' read -r sid sbc; do
                [[ -z "$sid" ]] && continue
                _kv_set sub_blocked_counts "$sid" "$sbc"
            done < <(awk '
                BEGIN { fm = 0; in_body = 0; cur_id = ""; blocked = 0; parent_bl = "" }
                /^---$/ { fm++; next }
                fm >= 2 { in_body = 1 }
                in_body && /^## / {
                    if (cur_id != "") printf "%s\t%d\n", cur_id, blocked
                    header = $0
                    sub(/^## /, "", header)
                    split(header, parts, " ")
                    cur_id = parts[1]
                    gsub(/:$/, "", cur_id)
                    blocked = 0
                    parent_bl = ""
                    next
                }
                in_body && /^### / { parent_bl = "" }
                cur_id != "" && /\- \[ \]/ {
                    line = $0
                    indent = 0
                    while (substr(line, indent+1, 1) == " ") indent++
                    has_own = (line ~ /blocked-by:/)
                    if (indent == 0) parent_bl = (has_own ? "y" : "")
                    if (has_own || (indent > 0 && parent_bl == "y")) blocked++
                }
                END { if (cur_id != "") printf "%s\t%d\n", cur_id, blocked }
            ' "$f")

            echo "Subsections:"
            while IFS=$'\t' read -r sub_id sub_title sub_status; do
                body_counts=$(awk -v sid="$sub_id" '
                    BEGIN { in_body = 0; in_section = 0; cx = 0; co = 0 }
                    /^---$/ { n++; next }
                    n >= 2 { in_body = 1 }
                    in_body && /^## / {
                        header = $0
                        if (header ~ "^## " sid "[ :]" || header ~ "^## " sid "$") {
                            in_section = 1
                            next
                        } else if (in_section) {
                            exit
                        }
                    }
                    in_section && /\- \[x\]/ { cx++ }
                    in_section && /\- \[ \]/ { co++ }
                    END { printf "%d %d", cx, co }
                ' "$f")
                sub_cx=${body_counts%% *}
                sub_co=${body_counts##* }
                sub_total=$((sub_cx + sub_co))

                sub_mismatch=""
                if [[ "$sub_status" == "complete" && "$sub_co" -gt 0 ]]; then
                    sub_mismatch=" !! frontmatter=complete but ${sub_co} unchecked"
                elif [[ "$sub_status" == "not-started" && "$sub_cx" -gt 0 ]]; then
                    sub_mismatch=" !! frontmatter=not-started but ${sub_cx} checked"
                elif [[ "$sub_total" -eq 0 ]]; then
                    sub_mismatch=" (no checkboxes found under ## header)"
                fi

                blocked_suffix=""
                bc=$(_kv_get sub_blocked_counts "$sub_id" 0)
                if [[ "$bc" -gt 0 ]]; then
                    blocked_suffix=" [${bc} blocked]"
                fi

                echo "  ${sub_id} ${sub_title} — ${sub_status} (${sub_cx}/${sub_total})${blocked_suffix}${sub_mismatch}"
            done < <(awk '
                /^---$/ { n++; next }
                n == 1 && /^  - id:/ { id = $NF; gsub(/"/, "", id) }
                n == 1 && /^    title:/ { sub(/^    title: */, ""); t = $0 }
                n == 1 && /^    status:/ { sub(/^    status: */, ""); printf "%s\t%s\t%s\n", id, t, $0 }
            ' "$f")
            echo ""

            # ── Unblocked items (grouped by subsection) ──
            if [[ "$total_unblocked" -gt 0 ]]; then
                echo "Unblocked items:"
                last_sub=""
                while IFS=$'\t' read -r lineno indent _blockers subsection content; do
                    [[ -z "$lineno" ]] && continue
                    if [[ "$subsection" != "$last_sub" ]]; then
                        sub_title=$(awk -v sid="$subsection" '
                            /^---$/ { n++; next }
                            n == 1 && /^  - id:/ { id = $NF; gsub(/"/, "", id) }
                            n == 1 && /^    title:/ { if (id == sid) { sub(/^    title: */, ""); print; exit } }
                        ' "$f")
                        echo "  ## ${subsection}: ${sub_title}"
                        last_sub="$subsection"
                    fi
                    content="${content#"${content%%[![:space:]]*}"}"
                    echo "    L${lineno}: ${content}"
                done < <(echo "$blocker_data" | awk -F'\t' '$3 == "-"')
                echo ""
            fi

            # ── Blocker tree with readiness classification ──
            if [[ "$(_kv_len blocker_item_counts)" -gt 0 ]]; then
                echo "Blocker tree:"
                sorted_blockers=($(_kv_keys blocker_item_counts | sort -n))
                last_idx=$(( ${#sorted_blockers[@]} - 1 ))
                for i in "${!sorted_blockers[@]}"; do
                    bsec="${sorted_blockers[$i]}"
                    bf=$(find_section_file "$bsec")
                    if [[ -n "$bf" && -f "$bf" ]]; then
                        bstatus=$(awk '/^---$/{n++; next} n==1 && /^status:/{sub(/^status: */,""); print; exit}' "$bf")
                        btitle=$(awk '/^---$/{n++; next} n==1 && /^title:/{sub(/^title: */,""); print; exit}' "$bf")
                        bchecked=$(grep -c '\- \[x\]' "$bf" 2>/dev/null || true)
                        bunchecked=$(grep -c '\- \[ \]' "$bf" 2>/dev/null || true)
                        btotal=$((${bchecked:-0} + ${bunchecked:-0}))
                        bpct=0
                        if [[ "$btotal" -gt 0 ]]; then
                            bpct=$((${bchecked:-0} * 100 / btotal))
                        fi

                        # Classify readiness
                        readiness=""
                        if [[ "$bstatus" == "complete" ]]; then
                            readiness="DONE"
                        elif [[ "$bstatus" == "in-progress" ]]; then
                            readiness="IN PROGRESS (${bpct}%)"
                        else
                            # Walk dependency chain to determine READY vs WAITING
                            current="$bsec"
                            depth=0
                            all_deps_ok=true
                            waiting_chain=""
                            while [[ "$depth" -lt 5 ]]; do
                                dep=$(_kv_get dep_of "$current" "")
                                [[ -z "$dep" ]] && break
                                df=$(find_section_file "$dep")
                                [[ -z "$df" || ! -f "$df" ]] && break
                                dstatus=$(awk '/^---$/{n++; next} n==1 && /^status:/{sub(/^status: */,""); print; exit}' "$df")
                                if [[ "$dstatus" != "complete" ]]; then
                                    all_deps_ok=false
                                    waiting_chain="${waiting_chain:+$waiting_chain <- }Section ${dep} [${dstatus}]"
                                fi
                                [[ "$dstatus" == "complete" ]] && break
                                current="$dep"
                                depth=$((depth + 1))
                            done
                            if $all_deps_ok; then
                                dep=$(_kv_get dep_of "$bsec" "")
                                readiness="READY${dep:+ (deps satisfied)}"
                                [[ -z "$dep" ]] && readiness="READY (no deps)"
                            else
                                readiness="WAITING on ${waiting_chain}"
                            fi
                        fi

                        # Collect affected subsections (sorted)
                        affected=$(_kv_keys blocker_subs | while read -r key; do
                            if [[ "${key%%:*}" == "$bsec" ]]; then echo "${key#*:}"; fi
                        done | sort -V | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')
                        [[ -z "$affected" ]] && affected="?"

                        # Tree connectors
                        count=$(_kv_get blocker_item_counts "$bsec" 0)
                        item_word="items"
                        [[ "$count" -eq 1 ]] && item_word="item"
                        if [[ "$i" -eq "$last_idx" ]]; then
                            connector="└─"
                            sub_prefix="   "
                        else
                            connector="├─"
                            sub_prefix="│  "
                        fi
                        echo "  ${connector} Section ${bsec}: ${btitle} [${bstatus}, ${bpct}%] — ${readiness}"
                        echo "  ${sub_prefix} └─ blocks ${count} ${item_word} in ${affected}"
                    fi
                done
                echo ""
            fi
        fi
    fi
done

if [[ -z "$first_incomplete" ]]; then
    echo ""
    echo "ALL SECTIONS COMPLETE"
fi
