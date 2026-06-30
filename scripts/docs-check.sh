#!/usr/bin/env bash
# Lint that guards against documentation drift. #75.
#
# Enforces the AGENTS.md "Doc Freshness" rule by failing when:
#   1. docs/SPEC.md header version != ROADMAP.md "Current State" version.
#   2. README.md revives the inaccurate "without GC" / "linear type system" claims.
#   3. docs/STDLIB.md is missing the implementation-status legend.
#
# Run locally: scripts/docs-check.sh
# Run in CI: see the `docs-check` job in .github/workflows/ci.yml.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

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

exit $status
