## Testing

- Test framework: **insta** (snapshot testing) + **assert_cmd** (CLI testing)
- Run tests: `docker run --rm -v "$(pwd)":/workspace -w /workspace aion-compiler cargo test -- --test-threads=1`
- Test fixtures: `tests/fixtures/{language,stdlib,compiler}/*.ai`
- Snapshots: `tests/snapshots/*.snap` (auto-generated, committed to git)
- **Execution**: AIs may run tests on their own when needed to verify changes.
- **Workflow**: After a modification, run tests to verify. Fix failures before moving on.
- **Creating new tests**: Add fixture in the appropriate subdirectory, run tests with `INSTA_UPDATE=always` to generate snapshot, commit the `.snap` file.
- **Updating snapshots**: Run `cargo insta review` to accept/reject changes interactively, or `INSTA_UPDATE=always cargo test` to auto-accept.
- **Expected failures**: Tests like `005_unsafe_check` that test compiler errors should snapshot the stderr output.
- **Directory structure**:
  - `tests/fixtures/language/` — Language features (if, while, match, generics, etc.)
  - `tests/fixtures/stdlib/` — Standard library tests (io, fs, collections, etc.)
  - `tests/fixtures/compiler/` — Compiler tests (self-hosting, optimization, FFI, etc.)