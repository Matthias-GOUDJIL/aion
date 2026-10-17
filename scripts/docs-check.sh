#!/usr/bin/env bash
# Lint that guards against documentation drift (#75, #112, #207).
#
# Enforces the AGENTS.md "Doc Freshness" rule by failing when:
#   1. docs/SPEC.md header version != ROADMAP.md "Current State" version.
#   2. README.md revives the inaccurate "without GC" / "linear type system" claims.
#   3. docs/STDLIB.md is missing the implementation-status legend.
#   4. [stable] stdlib modules reference unimplemented intrinsics (#112).
#   5. Any file in src/**/*.{rs,c} or compiler/*.ai is missing from docs/architecture.md,
#      or docs/architecture.md references a deleted file (#207).
#   6. Any file in stdlib/**/*.ai is missing from docs/STDLIB.md (#207).
#   7. Enum payload byte size in src/codegen/compiler.rs (array_type(N)) differs
#      from [N x i8] in docs/SPEC.md, docs/architecture.md, or ROADMAP.md (#207).
#
# Run locally: bash scripts/docs-check.sh
# Run in CI: see the `docs-check` job in .github/workflows/ci.yml.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Auto-configure local git hooks path when running inside a git checkout (#207).
if [ -d .git ] && [ -d .githooks ]; then
    current_hooks="$(git config --get core.hooksPath || true)"
    if [ "$current_hooks" != ".githooks" ]; then
        git config core.hooksPath .githooks 2>/dev/null || true
    fi
fi

status=0

# --- Check 1: SPEC version matches ROADMAP Current State version. ---
spec_version=$(grep -m1 -E '^# Aion Specification v[0-9]+\.[0-9]+' docs/SPEC.md | grep -oE 'v[0-9]+\.[0-9]+' | head -1)
roadmap_version=$(grep -m1 -E 'Current State: v[0-9]+\.[0-9]+' ROADMAP.md | grep -oE 'v[0-9]+\.[0-9]+' | head -1)

if [ -z "$spec_version" ] || [ -z "$roadmap_version" ]; then
    echo "error: could not extract version from SPEC.md ('$spec_version') or ROADMAP.md ('$roadmap_version')"
    status=1
elif [ "$spec_version" != "$roadmap_version" ]; then
    echo "error: version mismatch — docs/SPEC.md is $spec_version, ROADMAP.md Current State is $roadmap_version"
    echo "       update the SPEC.md header or ROADMAP.md 'Current State' line so both match."
    status=1
else
    echo "ok: SPEC.md and ROADMAP.md versions match ($spec_version)"
fi

# --- Check 2: README must not revive disproven claims. ---
if grep -niE 'without GC|linear type system' README.md; then
    echo "error: README.md contains an inaccurate 'without GC' or 'linear type system' claim"
    echo "       Aion uses the Boehm GC; remove the stale wording."
    status=1
else
    echo "ok: README.md has no 'without GC' / 'linear type system' claims"
fi

# --- Check 3: STDLIB.md must keep the implementation-status legend. ---
if ! grep -q 'Implementation status legend' docs/STDLIB.md; then
    echo "error: docs/STDLIB.md is missing the 'Implementation status legend' block"
    echo "       the legend ([stable]/[partial]/[stub]/[skeleton]) must be present."
    status=1
else
    echo "ok: docs/STDLIB.md has the implementation-status legend"
fi

# --- Check 4: [stable] stdlib modules must not reference unimplemented intrinsics. #112. ---
if bash "$(dirname "${BASH_SOURCE[0]}")/docs-check-intrinsics.sh"; then
    :
else
    status=1
fi

# --- Check 5: src/**/*.{rs,c} and compiler/*.ai must match docs/architecture.md (#207). ---
arch_missing=0
for f in $(find src -type f \( -name '*.rs' -o -name '*.c' \) | sort) $(find compiler -maxdepth 1 -type f -name '*.ai' | sort); do
    base=$(basename "$f")
    if ! grep -qF "$base" docs/architecture.md; then
        echo "error: '$f' exists on disk but '$base' is not documented in docs/architecture.md"
        arch_missing=1
        status=1
    fi
done

# Also check that every *.rs / *.c file entry in the architecture.md tree exists under src/
while read -r entry; do
    [ -z "$entry" ] && continue
    if [ -z "$(find src -name "$entry" -print -quit)" ]; then
        echo "error: docs/architecture.md lists '$entry' in its tree, but no such file exists under src/"
        arch_missing=1
        status=1
    fi
done < <(awk '/^```/{in_tree=!in_tree; next} in_tree && match($0, /[a-z_]+\.(rs|c)[[:space:]]+—/) { print substr($0, RSTART, RLENGTH) }' docs/architecture.md | awk '{print $1}')

if [ "$arch_missing" -eq 0 ]; then
    echo "ok: all src/ and compiler/ files are synchronized with docs/architecture.md"
fi

# --- Check 6: Every stdlib/**/*.ai file must be documented in docs/STDLIB.md (#207). ---
stdlib_missing=0
for f in $(find stdlib -type f -name '*.ai' | sort); do
    mod=$(echo "$f" | sed -e 's|^stdlib/||' -e 's|\.ai$||' -e 's|/|.|g')
    leaf=$(basename "$f" .ai)
    if ! grep -qE "\`($mod|$leaf)\`" docs/STDLIB.md; then
        echo "error: '$f' (module '$mod') is not documented in docs/STDLIB.md"
        stdlib_missing=1
        status=1
    fi
done
if [ "$stdlib_missing" -eq 0 ]; then
    echo "ok: all stdlib/**/*.ai modules are documented in docs/STDLIB.md"
fi

# --- Check 7: Enum payload size in src/codegen/compiler.rs matches docs (#207). ---
enum_bytes=$(grep -oE 'i8_type\(\)\.array_type\([0-9]+\)' src/codegen/compiler.rs | grep -oE '[0-9]+' | tail -1)
if [ -n "$enum_bytes" ]; then
    enum_ok=1
    for docfile in docs/SPEC.md docs/architecture.md ROADMAP.md; do
        if ! grep -qF "[${enum_bytes} x i8]" "$docfile"; then
            echo "error: $docfile does not match enum payload layout '[${enum_bytes} x i8]' from src/codegen/compiler.rs"
            enum_ok=0
            status=1
        fi
    done
    if [ "$enum_ok" -eq 1 ]; then
        echo "ok: enum layout '{ i64, [${enum_bytes} x i8] }' matches across codegen and docs"
    fi
fi

exit $status
