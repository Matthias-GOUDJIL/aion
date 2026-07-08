#!/usr/bin/env bash
# Guard against stdlib/doc drift: a [stable] module in docs/STDLIB.md must
# not reference an @intrinsic whose C runtime function is missing or a known
# placeholder. #112.
#
# For every module tagged [stable] in docs/STDLIB.md, collect the
# @intrinsic(...) references in its source file, classify each as a
# declaration decorator (form 1 — attribute value is the C function name) or
# an expression call (form 2 — normalize via the rules in Expression::Intrinsic
# in src/codegen/compiler.rs), then verify the resolved C function is defined
# and non-placeholder in src/runtime.c (or is a recognized libc/inline intrinsic).
#
# Run locally: scripts/docs-check-intrinsics.sh
# Invoked by scripts/docs-check.sh (see the `docs-check` CI job).

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

stdlib="docs/STDLIB.md"
runtime="src/runtime.c"

# Intrinsics handled inline by codegen (no C call): no runtime.c symbol needed.
inline_set="sizeof mem_is_null mem_zero str_ptr"

# C symbols provided by libc / the linked C runtime, not by src/runtime.c.
# Extended as new libc-backed intrinsics are added.
external_set="strlen exit memcpy memset memcmp strcmp strncmp strcpy strncpy time localtime malloc free printf sprintf snprintf"

errors=()

is_in_set() { # $1 = value, $2 = space-separated set
    local v="$1"
    local e
    for e in $2; do [ "$e" = "$v" ] && return 0; done
    return 1
}

# Normalize an @intrinsic expression name to its C resolution.
# Echoes "inline", "external <name>", or "runtime <name>".
normalize_expr() { # $1 = intrinsic name as written in the .ai
    local an="$1"
    if is_in_set "$an" "$inline_set"; then echo "inline"; return; fi
    case "$an" in
        str_len)            echo "external strlen" ;;
        exit)               echo "external exit" ;;
        fs_read_to_string)  echo "runtime aion_read_file" ;;
        fs_write)           echo "runtime aion_write_file" ;;
        fs_append)          echo "runtime aion_append_file" ;;
        mem_zero_ptr)       echo "runtime aion_memzero" ;;
        libc.*)             echo "external ${an#libc.}" ;;
        *)                  echo "runtime aion_${an}" ;;
    esac
}

# Normalize a declaration-decorator @intrinsic name (attribute value is the C
# function name directly, e.g. "aion_malloc"). libc.* still strips.
normalize_decl() { # $1 = intrinsic name as written in the .ai
    local an="$1"
    case "$an" in
        libc.*) echo "external ${an#libc.}" ;;
        *)      echo "runtime ${an}" ;;
    esac
}

defined_in_runtime() { # $1 = C function name
    grep -nE "^[A-Za-z_][A-Za-z_0-9 *]*[[:space:]]+[*]*\b$1\b *\(" "$runtime" \
        | grep -v ';' >/dev/null
}

runtime_body() { # $1 = C function name -> echoes body lines
    awk -v fn="$1" '
        $0 !~ /;/ && $0 ~ ("[^A-Za-z_0-9]" fn "[[:space:]]*\\(") { printing=1 }
        printing { print }
        printing && $0 ~ /^}/ { exit }
    ' "$runtime"
}

add_error() { errors+=("$1"); }

# --- 1. Parse docs/STDLIB.md for [stable] modules, map each to a file. ------
# Output: "<module_dotted_path>\t<file_path>".
mapfile -t stable_records < <(
    awk '
        function emit(path,    file) {
            file = "stdlib/" path; gsub(/\./, "/", file); file = file ".ai"
            print path "\t" file
        }
        /^### / {
            # A heading can list several `name` [status] segments split by "&".
            # The FIRST segment anchors sub-bullets even if the heading line
            # itself carries no [stable] mark (e.g. `### std.collections` where
            # the status lives on the `- **vector**` sub-bullets).
            head = $0
            nb = split(head, segs, "&")
            lastbase = ""
            for (i=1;i<=nb;i++) {
                seg = segs[i]
                if (match(seg, /`[^`]+`/)) {
                    nm = substr(seg, RSTART+1, RLENGTH-2)
                    if (lastbase == "") lastbase = nm
                    if (seg ~ /\[stable\]/) emit(nm)
                }
            }
            next
        }
        /^- \*\*`[^`]+`\*\*/ {
            line = $0
            if (!match(line, /`[^`]+`/)) next
            nm = substr(line, RSTART+1, RLENGTH-2)
            if (line ~ /\[stable\]/ && lastbase != "") emit(lastbase "." nm)
        }
    ' "$stdlib"
)

# --- 2. For each [stable] module file, emit "cname\tform" records. -----------
validate_records=()
for rec in "${stable_records[@]}"; do
    IFS=$'\t' read -r mod file <<< "$rec"
    if [ ! -f "$file" ]; then
        add_error "$mod: source file $file not found"
        continue
    fi
    while IFS=$'\t' read -r cname form; do
        [ -z "$cname" ] && continue
        validate_records+=("$mod"$'\t'"$cname"$'\t'"$form")
    done < <(
        awk '
            { lines[NR]=$0 }
            END {
                for (i=1;i<=NR;i++) {
                    line = lines[i]
                    if (!match(line, /@intrinsic[[:space:]]*\(/)) continue
                    rest = substr(line, RSTART+RLENGTH)
                    if (!match(rest, /"[^"]*"/)) continue
                    an = substr(rest, RSTART+1, RLENGTH-2)
                    nxt = ""
                    for (j=i+1;j<=NR;j++) {
                        if (lines[j] ~ /^[[:space:]]*$/) continue
                        nxt = lines[j]; break
                    }
                    hasargs = (rest ~ /,[[:space:]]*[^)]/)?1:0
                    # form 2 with args is always expression form; form 1 has no
                    # args and is followed by a `fn` declaration.
                    if (!hasargs && nxt ~ /^[[:space:]]*(pub[[:space:]]+)?(unsafe[[:space:]]+)?fn[[:space:]]/)
                        print an "\tdecl"
                    else
                        print an "\texpr"
                }
            }
        ' "$file"
    )
done

# --- 3. Validate each intrinsic reference. -----------------------------------
for rec in "${validate_records[@]}"; do
    IFS=$'\t' read -r mod cname form <<< "$rec"
    if [ "$form" = "decl" ]; then norm="$(normalize_decl "$cname")"
    else norm="$(normalize_expr "$cname")"; fi
    kind="${norm%% *}"
    name="${norm#* }"
    if [ "$kind" = "inline" ]; then
        continue
    fi
    if [ "$kind" = "external" ]; then
        if ! is_in_set "$name" "$external_set"; then
            add_error "$mod: @intrinsic(\"$cname\") -> external symbol '$name' not in the libc allowlist (scripts/docs-check-intrinsics.sh:external_set)"
        fi
        continue
    fi
    if ! defined_in_runtime "$name"; then
        add_error "$mod: @intrinsic(\"$cname\") -> C function '$name' NOT defined in src/runtime.c (downgrade the module to [partial]/[stub] in docs/STDLIB.md OR add the runtime impl)"
        continue
    fi
    body="$(runtime_body "$name")"
    if echo "$body" | grep -qiE '//[[:space:]]*placeholder|placeholder:|return zeros for now'; then
        add_error "$mod: @intrinsic(\"$cname\") -> '$name' is a placeholder in src/runtime.c (downgrade the module to [partial]/[stub] OR implement it)"
    fi
done

# --- Report. ----------------------------------------------------------------
if [ ${#errors[@]} -ne 0 ]; then
    echo "error: [stable] stdlib modules reference unimplemented intrinsic(s):" >&2
    printf '  - %s\n' "${errors[@]}" >&2
    echo >&2
    echo "Per AGENTS.md Doc Freshness, a [stable] module must not advertise" >&2
    echo "behavior the runtime does not implement. Downgrade the module to" >&2
    echo "[partial]/[stub] in docs/STDLIB.md, or add the missing runtime impl." >&2
    exit 1
fi
echo "ok: all [stable] stdlib intrinsics resolve to implemented runtime functions"