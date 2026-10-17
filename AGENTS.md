# Aion — Agent Instructions

## Core Instructions & AI Communication Rules

- **Strict Conciseness**: Respond directly and without unnecessary politeness to save output tokens. Get straight to the point (no "hello", "I will do it", or "here is").
- **Quiet Shell Commands**: Always use quiet flags (`-q`, `--quiet`, or `> /dev/null`) for shell commands when detailed output is not needed (e.g. dependency installation, successful builds).
- **Doc Freshness**: When modifying code behavior, architecture, or workflow, update the corresponding `docs/*.md` file in the same commit. Never leave stale docs — they will mislead future sessions.
- **Test Coverage**: When adding or changing compiler/lexer/parser/codegen behavior, add or update test fixtures under `tests/fixtures/` covering the new behavior. Run `INSTA_UPDATE=always cargo test` to generate snapshots, then verify their content before committing. Never ship a behavior change without an accompanying test. Each fixture must cover:
  - **Nominal case**: the happy path — normal inputs producing expected output.
  - **Edge cases**: empty inputs, boundary indices (0, len-1, len), out-of-bounds, type variations (i64 vs String vs struct), no-op conditions (e.g. `swap(i, i)`), and any overflow/truncation scenarios relevant to the feature.
  - **Error cases**: where applicable — invalid inputs should produce the documented error, not a crash. Snapshot the stderr for expected-failure tests.
- **Never run git add, commit, or push without explicit user approval**.

## Project Overview

Aion is a system programming language for AI-native apps. The compiler (`aionc`) is written in **Rust** and targets **LLVM 15** IR. The runtime links against **Boehm GC** (`libgc`) and `pthread` via a C runtime (`src/runtime.c`).

## Coding Standards

- **Completion first**: never start a new feature if existing ones are incomplete or untested.
- **No dead code**: remove commented-out blocks and unused code immediately.
- **No debug prints**: use `eprintln!` with feature flags, not `println!` for debugging.
- **SPEC alignment**: code behavior must match `docs/SPEC.md`. If code changes, update SPEC first.
- **Zero-cost abstractions**: verify generated IR is optimal, isolate `unsafe` blocks, use `Result<T, E>` extensively.
- **Robustness First**: when multiple approaches exist, always propose the most robust, professional, and maintainable solution. If the language lacks features needed for the robust approach, document the blockers in the library's `SPEC.md` and propose what needs to be fixed upstream rather than working around it with inferior patterns.
- All code, comments, variable names, and commit messages in **English only**.
- Read `docs/SPEC.md` before contributing to the compiler.

## Testing

- Test framework: **insta** (snapshot testing) + **assert_cmd** (CLI testing).
- Run tests: `docker run --rm -v "$(pwd)":/workspace -w /workspace aion-compiler cargo test -- --test-threads=1`.
- Test fixtures: `tests/fixtures/{language,stdlib,compiler}/*.ai`.
- Snapshots: `tests/snapshots/*.snap` (auto-generated, committed to git).
- **Execution**: AIs may run tests on their own when needed to verify changes.
- **Workflow**: After a modification, run tests to verify. Fix failures before moving on.
- **Creating new tests**: Add fixture in the appropriate subdirectory, run tests with `INSTA_UPDATE=always` to generate snapshot, commit the `.snap` file.
- **Updating snapshots**: Run `cargo insta review` to accept/reject changes interactively, or `INSTA_UPDATE=always cargo test` to auto-accept.
- **Expected failures**: Tests that test compiler errors should snapshot the stderr output.
- **Directory structure**:
  - `tests/fixtures/language/` — Language features (if, while, match, generics, etc.).
  - `tests/fixtures/stdlib/` — Standard library tests (io, fs, collections, etc.).
  - `tests/fixtures/compiler/` — Compiler tests (self-hosting, optimization, FFI, etc.).

## Project Tracking

- **Issues**: https://github.com/Matthias-GOUDJIL/aion/issues
- **Branches**: Feature branches → PR → merge to `main`
- **Commits**: Conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`, `style:`, `perf:`, `build:`, `ci:`)
- **CI**: GitHub Actions runs tests on every push/PR

### GitHub Workflow (mandatory)
1. **Never commit directly on `main`** — create a feature branch (`type/issue-N-short-desc`, e.g. `refactor/match-lowering-dedup`) from `main` first.
2. Push the branch, then open a PR with the template filled; the body must include `Closes #N`.
3. **Watch the CI checks**; merge **only when all checks are green** (squash merge — repo history is linear).
4. **Never close an issue manually** — issues close automatically when the `Closes #N` PR merges. If an issue has no merged PR, it stays open.
5. After merge: pull `main`, delete the local and remote feature branch.

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
5. **Never start a new feature if existing ones are incomplete or untested** (see Coding Standards — Completion first).
6. **Check the "Status" section** in the issue body before starting work so you do not re-implement an already-fixed issue.

### ROADMAP ↔ Issue Cross-Reference
The ROADMAP (`ROADMAP.md`) lists phases. Each unchecked ROADMAP item has a corresponding GitHub issue:

| ROADMAP item | Issue | Phase |
|---|---|---|
| Self-Runtime (rewrite aion_spawn in Aion) | #72 | Phase 1.6 |
| `?` operator for `Result<T, E>` | #138 | Phase 1.8 |
| Phase 3 — LSP server | #10 | Phase 3 |
| Phase 3 — MCP server (LLM agent tooling) | #82 | Phase 3 |
| Phase 3 — Full self-hosting loop (Ouroboros standalone binary) | (next after #9/#135) | Phase 3 |

When a ROADMAP item has no issue yet and you are asked to work on it, **open the issue first** (using the template), then start.

## Documentation Structure

### Hierarchy (read in this order)

| Priority | File | Scope | When to read |
|---|---|---|---|
| 1 | `AGENTS.md` (this file) | Rules, conventions, triage | Always — loaded by AI tools automatically |
| 2 | `docs/SPEC.md` | Language specification (current implementation) | Before any compiler change |
| 3 | `docs/architecture.md` | Compiler pipeline, invariants, debugging | Before codegen/checker changes |
| 4 | `docs/STDLIB.md` | Stdlib overview with impl status markers | Before stdlib changes |
| 5 | `docs/workflow.md` | Commands, Docker, Git workflow, label taxonomy | Before any contribution |

### SPEC files

`docs/SPEC.md` is the **general** language spec. Feature-specific specs live alongside it:

| File | Feature |
|---|---|
| `docs/SPEC.md` | Core language (types, memory, codegen, robustness) |
| `docs/SPEC_TIME.md` | `Duration` and `Date` primitives |
| `docs/SPEC_SANDBOX.md` | `unsafe` keyword and sandbox enforcement |
| `docs/SPEC_SQL.md` | PostgreSQL (`PL/pgSQL`) transpiler |

**Naming convention**: `docs/SPEC_{FEATURE}.md` — uppercase, underscore-separated, one feature per file.

### Stdlib module specs

Each stdlib module has a `SPEC.md` co-located with its source code. This documents purpose, API, language requirements, known limitations, and design decisions. Always read the module's `SPEC.md` before modifying it.

Current module specs:
- `stdlib/std/json_SPEC.md` — JSON parser/serializer (in progress)

**Naming convention**: `stdlib/std/{module}/SPEC.md` (e.g. `stdlib/std/collections/SPEC.md`). If a module is a single file at the parent level (`stdlib/std/{module}.ai`), its spec lives at `stdlib/std/{module}_SPEC.md` (see issue #41 for the json layout inconsistency).

## Extended Context

The detailed project context has been split into thematic files to preserve token context. **You must read these files using the `read_file` tool only if the current task requires it.**

- **[Architecture & Debugging](docs/architecture.md)**: Compiler pipeline, invariants, toolchain quirks, and debugging guide.
- **[Workflow & Commands](docs/workflow.md)**: Developer commands, Docker cache gotcha, and label taxonomy.
