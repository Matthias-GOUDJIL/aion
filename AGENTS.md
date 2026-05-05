# Aion — Agent Instructions

## Core Instructions & AI Communication Rules

- **Strict Conciseness**: Respond directly and without unnecessary politeness to save output tokens. Get straight to the point (no "hello", "I will do it", or "here is").
- **Quiet Shell Commands**: Always use quiet flags (`-q`, `--quiet`, or `> /dev/null`) for shell commands when detailed output is not needed (e.g. dependency installation, successful builds).
- **Doc Freshness**: When modifying code behavior, architecture, or workflow, update the corresponding `docs/*.md` file in the same commit. Never leave stale docs — they will mislead future sessions.

## Project Overview

Aion is a system programming language for AI-native apps. The compiler (`aionc`) is written in **Rust** and targets **LLVM 15** IR. The runtime links against **Boehm GC** (`libgc`) and `pthread` via a C runtime (`src/runtime.c`).

## Project Tracking

- **Issues**: https://github.com/Matthias-GOUDJIL/aion/issues
- **Branches**: Feature branches → PR → merge to `main`
- **Commits**: Conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`)
- **CI**: GitHub Actions runs tests on every push/PR

## Extended Context (Read as needed)

The detailed project context has been split into thematic files to preserve token context. **You must read these files using the `read_file` tool only if the current task requires it.**

- **[Architecture & Debugging](docs/architecture.md)**: Compiler pipeline, invariants, toolchain quirks, and debugging guide.
- **[Testing](docs/testing.md)**: How to run tests, fixture conventions, and test creation rules.
- **[Conventions & Standards](docs/conventions.md)**: Coding standards, language rules, and Git conventions.
- **[Workflow & Commands](docs/workflow.md)**: Developer commands, Docker cache gotcha, and workflow commands.
