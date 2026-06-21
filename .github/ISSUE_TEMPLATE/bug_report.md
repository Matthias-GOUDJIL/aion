---
name: Bug Report
about: Report a compiler/lexer/parser/codegen/runtime bug
title: "[Area] Brief description in imperative mood"
labels: ["type-bug"]
---

## Problem
<!-- What is broken? Be specific. -->

## Repro
<!--
Minimum code that triggers the bug.

```aion
// code here
```

Then describe the expected vs actual output.
-->

## Impact
<!-- Who is affected? What breaks? Is there a workaround? -->

## Proposed Fix
<!-- Sketch the fix — file path, approach, alternative solutions considered. -->

## Acceptance
<!--
Checklist of conditions that must be true for the issue to be closed.
- [ ] ...
- [ ] ...
-->

## Tests required
<!--
Per AGENTS.md Test Coverage rule: every behavior change needs fixtures
covering nominal + edge + error cases.
Describe the test(s) to add under tests/fixtures/.
- [ ] Nominal: ...
- [ ] Edge cases: empty inputs, boundary indices (0, len-1, len),
      out-of-bounds, type variations, no-op conditions.
- [ ] Error cases: invalid inputs produce documented errors, not crashes.
-->

## Related
<!--
Issues: #NN
PRs: #NN
ROADMAP phase: vX.Y
-->
