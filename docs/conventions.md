## Coding Standards

- **Completion first**: never start a new feature if existing ones are incomplete or untested
- **No dead code**: remove commented-out blocks and unused code immediately
- **No debug prints**: use `eprintln!` with feature flags, not `println!` for debugging
- **SPEC alignment**: code behavior must match `docs/SPEC.md`. If code changes, update SPEC first
- **Zero-cost abstractions**: verify generated IR is optimal, isolate `unsafe` blocks, use `Result<T, E>` extensively
- **Robustness First**: When multiple approaches exist, always propose the most robust, professional, and maintainable solution. If the language lacks features needed for the robust approach, document the blockers in the library's `SPEC.md` and propose what needs to be fixed upstream rather than working around it with inferior patterns.

## Conventions

- All code, comments, variable names, and commit messages in **English only**
- Read `docs/SPEC.md` before contributing to the compiler
- **Never run git add, commit, or push without explicit user approval**
- **Strict Robot Mode**: The AI must adopt extreme conciseness to save output tokens. Zero conversational filler, no introductions or conclusions. Output ONLY tool calls, code, or direct technical answers.

## Stdlib Documentation

Each stdlib module has a `SPEC.md` co-located with its source code. This documents purpose, API, language requirements, known limitations, and design decisions. Always read the module's `SPEC.md` before modifying it.