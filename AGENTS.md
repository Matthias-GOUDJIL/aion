# Aion — Agent Instructions

## Core Instructions & AI Communication Rules

- **Strict Conciseness**: Respond directly and without unnecessary politeness to save output tokens. Get straight to the point (no "hello", "I will do it", or "here is").
- **Quiet Shell Commands**: Always use quiet flags (`-q`, `--quiet`, or `> /dev/null`) for shell commands when detailed output is not needed (e.g. dependency installation, successful builds).
- **Doc Freshness**: When modifying code behavior, architecture, or workflow, update the corresponding `docs/*.md` file in the same commit. Never leave stale docs — they will mislead future sessions.
- **Test Coverage**: When adding or changing compiler/lexer/parser/codegen behavior, add or update test fixtures under `tests/fixtures/` covering the new behavior (nominal case + edge cases). Run `INSTA_UPDATE=always cargo test` to generate snapshots, then verify their content before committing. Never ship a behavior change without an accompanying test.

## Project Overview

Aion is a system programming language for AI-native apps. The compiler (`aionc`) is written in **Rust** and targets **LLVM 15** IR. The runtime links against **Boehm GC** (`libgc`) and `pthread` via a C runtime (`src/runtime.c`).

## Project Tracking

- **Issues**: https://github.com/Matthias-GOUDJIL/aion/issues
- **Branches**: Feature branches → PR → merge to `main`
- **Commits**: Conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`)
- **CI**: GitHub Actions runs tests on every push/PR

### Issue & PR Templates
- New issues use GitHub templates (`.github/ISSUE_TEMPLATE/`): bug report or feature request. Both enforce sections: Problem/Motivation, Proposed Fix/API, Acceptance, Tests required, Related.
- PRs use `.github/PULL_REQUEST_TEMPLATE.md`: Summary, Changes (by area), Tests (checkboxes), Docs (checkboxes), Closes #N.
- **Always fill every section** — do not leave Acceptance or Tests empty. If a section is N/A, write "N/A" with a one-line reason.

### Label Taxonomy
See `docs/workflow.md` for the full table. Rules:
- Every issue: exactly one `priority-*` + exactly one `type-*` + at least one `area-*`.
- `phase-*` labels are optional (apply only when the issue maps to a ROADMAP phase).
- When creating an issue, apply labels at creation time — do not leave unlabeled.

### Triage Guide for AI Agents
When asked to "pick the next task" or "what should I work on":
1. **Sort by priority**: `priority-critical` > `priority-high` > `priority-medium` > `priority-low`.
2. **Check dependencies**: read the "Related" / "Depends on" section. If an issue depends on an open issue, the dependency must be closed first.
3. **Prefer bugs over features** at equal priority — a `type-bug` at `priority-high` is more urgent than a `type-feature` at `priority-high`.
4. **Prefer smaller scope** when priorities are equal — a fix touching one file is preferable to a refactor touching the whole codegen, because it lands faster and unblocks others.
5. **Never start a new feature if existing ones are incomplete or untested** (see `docs/conventions.md` — Completion first).
6. **Check the "Status" section** in the issue body — some issues are already FIXED and waiting for a PR merge (e.g. #29, #62). Do not re-implement.

### ROADMAP ↔ Issue Cross-Reference
The ROADMAP (`ROADMAP.md`) lists phases. Each unchecked ROADMAP item has a corresponding GitHub issue:

| ROADMAP item | Issue | Phase |
|---|---|---|
| Self-Hosting Path (identify blockers) | (not yet opened) | Phase 1.6 |
| Self-Runtime (rewrite aion_spawn in Aion) | #72 | Phase 1.6 |
| GCC/Clang Removal | #73 | Phase 1.6 |
| SQL Backend & Interoperability | #74 | Phase 1.5 |
| Phase 2 v0.9 — LLVM Backend in Aion | #9 | Phase 2 |
| Phase 3 — LSP server | #10 | Phase 3 |
| Phase 3 — Full self-hosting (Ouroboros) | (blocked by #9) | Phase 3 |

When a ROADMAP item has no issue yet and you are asked to work on it, **open the issue first** (using the template), then start.

## Extended Context (Read as needed)

The detailed project context has been split into thematic files to preserve token context. **You must read these files using the `read_file` tool only if the current task requires it.**

- **[Architecture & Debugging](docs/architecture.md)**: Compiler pipeline, invariants, toolchain quirks, and debugging guide.
- **[Testing](docs/testing.md)**: How to run tests, fixture conventions, and test creation rules.
- **[Conventions & Standards](docs/conventions.md)**: Coding standards, language rules, and Git conventions.
- **[Workflow & Commands](docs/workflow.md)**: Developer commands, Docker cache gotcha, and workflow commands.

## Stdlib Documentation

Each stdlib module has a `SPEC.md` co-located with its source code. This documents purpose, API, language requirements, known limitations, and design decisions. Always read the module's `SPEC.md` before modifying it.

Current specs:
- `stdlib/std/json/SPEC.md` — JSON parser/serializer (in progress)
