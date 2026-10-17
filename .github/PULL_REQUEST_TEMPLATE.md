## Summary
<!-- One paragraph: what does this PR do and why? -->

## Changes
<!-- Bullet list of changes, grouped by area. -->

### <!-- Area: e.g. Lexer / Parser / Checker / Codegen / Stdlib / Docs / CI -->
- ...

## Tests
<!--
Per AGENTS.md Test Coverage rule: every behavior change needs a fixture.
List the test fixtures added or updated. If no tests are needed, explain why.
-->

- [ ] Fixture added/updated: `tests/fixtures/...`
- [ ] All tests pass: `cargo test` (or Docker equivalent)
- [ ] Snapshots reviewed (not blindly accepted via INSTA_UPDATE=always)

## Docs
<!--
Per AGENTS.md Doc Freshness rule: any PR modifying src/, compiler/, or stdlib/
MUST either update the corresponding docs/*.md file in the same PR OR check
"No doc impact" below with a one-line justification (enforced by CI #207).
-->

- [ ] `docs/SPEC.md` updated (or `docs/SPEC_*.md`)
- [ ] `docs/architecture.md` updated (if files/invariants changed)
- [ ] `docs/STDLIB.md` / `stdlib/.../SPEC.md` updated (if stdlib changed)
- [ ] `ROADMAP.md` & `AGENTS.md` updated (if closing a tracked ROADMAP issue)
- [ ] No doc impact: <!-- required if no .md file was modified: write 1-line reason here -->

## Closes
<!-- Reference the issues this PR closes. Remove if none. -->

Closes #NN
