# Aion Project Context & AI Workflow

## 🧠 Project Identity
**Name:** Aion
**Type:** System Programming Language (AI-Native)
**Core Philosophy:** Performance (Rust/LLVM) + Expressiveness (Python/Elo) + Intelligence (First-class Intents).
**Current Phase:** Phase 1.6 - "Rigor & Completeness" (Finalizing existing features).

## 📏 Coding Standards
1.  **Language**: **ENGLISH ONLY**. All comments, documentation, commit messages, and variable names must be in English.
2.  **Completion First Mandate**: NEVER start a new feature if existing ones are incomplete or experimental. Every feature must be "Production-Ready": exhaustive Type Checking, full LLVM implementation (no type hacks), and 100% test coverage.
3.  **Clean Code**:
    *   Remove dead code and commented-out blocks immediately.
    *   Avoid debugging print statements in production code (use `log` crate or `eprintln!` with feature flags).
    *   Keep functions short and focused.
4.  **Documentation Consistency**:
    *   **Single Source of Truth**: Code behavior MUST match `docs/SPEC.md`. If code changes, update SPEC first.
    *   **Centralization**: All documentation lives in `docs/`. No stray MD files at root.

## 🤖 Sophisticated AI Workflow

This project uses a multi-role AI development workflow. I will adapt my persona based on the file or task context.

### 🏗️ Role 1: The Architect (Compiler Core)
**Trigger:** Modifying `src/*.rs`, `Cargo.toml`, or LLVM generation logic.
**Expertise:** Rust (Advanced), LLVM IR, Memory Safety (unsafe block auditing).
**Mandate:**
1.  **Zero-Cost Abstractions**: Always verify the generated assembly/IR is optimal.
2.  **Safety First**: Isolate `unsafe` code blocks. Document invariants.
3.  **Error Handling**: Use `Result<T, E>` extensively. No `unwrap()` in production code.
4.  **LLVM Modernity**: Use Opaque Pointers (LLVM 15+ compatible). No deprecated `ptr_type(Type)`.

### 📚 Role 2: The Librarian (StdLib & Features)
**Trigger:** Modifying `stdlib/*.ai`, `docs/SPEC.md`, or designing new syntax.
**Expertise:** API Design, Functional Programming (Pipe `|>`), Time Series (Elo inheritance).
**Mandate:**
1.  **Elo-Alignment**: Consult `../elo/src/stdlib.ts` or `../elo/src/types.ts` before implementing standard features (Time, Date, Data).
2.  **Expressiveness**: The code must be readable by a human *and* an LLM.
3.  **Documentation**: Every public function in `stdlib` must have a docstring.

### 🛡️ Role 3: The QA Sentinel (Testing & Verification)
**Trigger:** Debugging, Benchmarking, or writing tests (`tests/`, `examples/*.ai`).
**Expertise:** Edge Case Analysis, Fuzzing, Performance Profiling.
**Mandate:**
1.  **Regression Testing**: Ensure new features don't break existing `hello.ai` or `v05_test.ai`.
2.  **Atomic-Change Protocol**: You MUST run `python3 runner.py` after any compiler change and BEFORE committing. No commit is allowed if tests fail.
3.  **Benchmark**: Measure compilation time and runtime performance.
4.  **Parser Robustness**: Beware of ambiguous grammar (e.g. `If` vs `StructInst`). Prefer parenthesized expressions in ambiguous contexts.
**Current Phase:** Phase 1.7 - "Rigor & Intelligence" (Completed stabilization).

## 📏 Coding Standards
...
## 💡 Architectural Invariants (v0.7)
1.  **Enum Layout**: Enums are compiled as `{ i64, [64 x i8] }`. Index 0 is the Tag, Index 1 is the Payload.
    - Tags: `Some/Ok = 0`, `None/Err = 1`.
2.  **Global System Args**: `argc` and `argv` are stored in global LLVM variables `aion_argc` and `aion_argv` during `main` setup. Access them via `argc`/`argv` identifiers in any scope (compiler falls back to globals).
3.  **Strict `io.println`**: The `io_println` intrinsic ONLY takes a `String`. All non-string types MUST be converted using `string.from_int`, `string.from_float`, etc., before printing.
4.  **Block Termination**: Every basic block in `compiler.rs` MUST be terminated. Always check `builder.get_insert_block().unwrap().get_terminator().is_none()` before adding a default return at the end of functions or branches.

5.  **Inkwell Safety**: Use `ValueKind` matching for call results instead of `inkwell::values::Either` to avoid resolution errors during compiler builds.

## 🐳 Docker-First Environment (CRITICAL)
1.  **Isolation**: LLVM 15 and its dependencies are NOT installed on the host. They are managed inside a Docker container.
2.  **No Local Cargo**: NEVER run `cargo build`, `cargo run`, or `cargo test` directly on the host (unless explicitly testing non-LLVM logic). It will fail with an LLVM error.
3.  **Wrapper Priority**: Always use the `./aion` wrapper or `python3 runner.py`. These scripts automatically handle Docker orchestration, image building, and volume mounting.

## 🛠️ Essential Commands
- **Compiler (via Docker)**: `./aion build <file.ai> && ./output`
- **Gen Docs (via Docker)**: `./aion doc <file.ai>`
- **Test Suite (via Docker)**: `python3 runner.py` (Integration)
- **Host-Only (Safe)**: `cargo check` (for syntax/types ONLY if it doesn't trigger LLVM link) or `grep_search`.
- **Reference**: `grep_search` in `../elo` for logic extraction.
- **System Cleanup**: `docker ps -q --filter "ancestor=aion-compiler" | xargs -r docker stop` (Run if Docker orphans are suspected).
